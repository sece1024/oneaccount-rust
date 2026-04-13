use std::sync::Arc;

use crate::dao::{AccountDao, CategoryDao, SnapshotDao, TransactionDao};
use crate::db::DbPool;
use crate::error::Result;
use crate::models::account::{Account, NewAccount};
use crate::models::category::{Category, CategoryType, NewCategory};
use crate::models::snapshot::{AccountSnapshot, MonthlyEntryItem, MonthlyTotal};
use crate::models::transaction::{NewTransaction, Transaction, TransactionFilter, TransactionType};

pub struct AppService {
    pool: Arc<DbPool>,
}

impl AppService {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    // ── Accounts ─────────────────────────────────────────────────────────────

    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        let conn = self.pool.get()?;
        AccountDao::find_all(&conn)
    }

    pub fn create_account(&self, req: &NewAccount) -> Result<Account> {
        let conn = self.pool.get()?;
        AccountDao::create(&conn, req)
    }

    pub fn total_balance(&self) -> Result<f64> {
        let conn = self.pool.get()?;
        AccountDao::total_balance(&conn)
    }

    pub fn delete_account(&self, id: i64) -> Result<()> {
        let conn = self.pool.get()?;
        AccountDao::delete(&conn, id)
    }

    // ── Categories ────────────────────────────────────────────────────────────

    pub fn list_categories(&self) -> Result<Vec<Category>> {
        let conn = self.pool.get()?;
        CategoryDao::find_all(&conn)
    }

    pub fn list_categories_by_type(&self, cat_type: &CategoryType) -> Result<Vec<Category>> {
        let conn = self.pool.get()?;
        CategoryDao::find_by_type(&conn, cat_type)
    }

    pub fn create_category(&self, req: &NewCategory) -> Result<Category> {
        let conn = self.pool.get()?;
        CategoryDao::create(&conn, req)
    }

    // ── Transactions ──────────────────────────────────────────────────────────

    /// 原子化创建账目，同时更新相关账户余额
    pub fn create_transaction(&self, req: &NewTransaction) -> Result<Transaction> {
        let mut conn = self.pool.get()?;
        let db_tx = conn.transaction()?;

        let tx = TransactionDao::create(&db_tx, req)?;

        match req.transaction_type {
            TransactionType::Expense => {
                AccountDao::adjust_balance(&db_tx, req.account_id, -req.amount)?;
            }
            TransactionType::Income => {
                AccountDao::adjust_balance(&db_tx, req.account_id, req.amount)?;
            }
            TransactionType::Transfer => {
                AccountDao::adjust_balance(&db_tx, req.account_id, -req.amount)?;
                if let Some(to_id) = req.to_account_id {
                    AccountDao::adjust_balance(&db_tx, to_id, req.amount)?;
                }
            }
        }

        db_tx.commit()?;
        Ok(tx)
    }

    /// 原子化删除账目，同时回滚余额
    pub fn delete_transaction(&self, id: i64) -> Result<()> {
        let mut conn = self.pool.get()?;
        let db_tx = conn.transaction()?;

        if let Some(tx) = TransactionDao::find_by_id(&db_tx, id)? {
            match tx.transaction_type {
                TransactionType::Expense => {
                    AccountDao::adjust_balance(&db_tx, tx.account_id, tx.amount)?;
                }
                TransactionType::Income => {
                    AccountDao::adjust_balance(&db_tx, tx.account_id, -tx.amount)?;
                }
                TransactionType::Transfer => {
                    AccountDao::adjust_balance(&db_tx, tx.account_id, tx.amount)?;
                    if let Some(to_id) = tx.to_account_id {
                        AccountDao::adjust_balance(&db_tx, to_id, -tx.amount)?;
                    }
                }
            }
            TransactionDao::delete(&db_tx, id)?;
        }

        db_tx.commit()?;
        Ok(())
    }

    pub fn list_transactions(&self, filter: &TransactionFilter) -> Result<Vec<Transaction>> {
        let conn = self.pool.get()?;
        TransactionDao::find_with_filter(&conn, filter)
    }

    pub fn list_large_expenses(&self) -> Result<Vec<Transaction>> {
        let conn = self.pool.get()?;
        TransactionDao::find_with_filter(
            &conn,
            &TransactionFilter {
                is_large_only: true,
                limit: Some(200),
                ..Default::default()
            },
        )
    }

    pub fn recent_transactions(&self, limit: i64) -> Result<Vec<Transaction>> {
        let conn = self.pool.get()?;
        TransactionDao::recent(&conn, limit)
    }

    // ── Statistics ────────────────────────────────────────────────────────────

    pub fn monthly_stats(&self, year: i32, month: u32) -> Result<(f64, f64, f64, f64)> {
        let conn = self.pool.get()?;
        let (income, expense) = TransactionDao::monthly_summary(&conn, year, month)?;
        let total = AccountDao::total_balance(&conn)?;
        Ok((income, expense, income - expense, total))
    }

    pub fn category_stats(&self, start_date: &str, end_date: &str) -> Result<Vec<(String, f64)>> {
        let conn = self.pool.get()?;
        TransactionDao::category_stats(&conn, start_date, end_date)
    }

    // ── 月结快照 ──────────────────────────────────────────────────────────────

    /// 为月结表单构建各账户的输入项（附带上次快照余额）
    pub fn build_monthly_entry_items(&self) -> Result<Vec<MonthlyEntryItem>> {
        let conn = self.pool.get()?;
        let accounts = AccountDao::find_all(&conn)?;
        let latest = SnapshotDao::latest_per_account(&conn)?;

        let items = accounts
            .into_iter()
            .map(|acc| {
                let last_balance = latest
                    .iter()
                    .find(|(aid, _, _, _)| *aid == acc.id)
                    .map(|(_, bal, _, _)| *bal);
                MonthlyEntryItem {
                    account_id: acc.id,
                    account_name: acc.name.clone(),
                    account_type: acc.account_type.display_name().to_string(),
                    last_balance,
                    input: String::new(),
                    confirmed_balance: None,
                }
            })
            .collect();
        Ok(items)
    }

    /// 批量保存月结快照并同步更新账户余额（原子操作）
    pub fn save_monthly_snapshot(
        &self,
        year: i32,
        month: u32,
        items: &[MonthlyEntryItem],
        note: Option<&str>,
    ) -> Result<f64> {
        let mut conn = self.pool.get()?;
        let db_tx = conn.transaction()?;
        let mut total = 0.0;

        for item in items {
            if let Some(balance) = item.confirmed_balance {
                SnapshotDao::upsert(&db_tx, year, month, item.account_id, balance, note)?;
                AccountDao::set_balance(&db_tx, item.account_id, balance)?;
                total += balance;
            }
        }

        db_tx.commit()?;
        Ok(total)
    }

    /// 最近 N 个月的总资产趋势
    pub fn asset_trend(&self, months: i64) -> Result<Vec<MonthlyTotal>> {
        let conn = self.pool.get()?;
        SnapshotDao::monthly_totals(&conn, months)
    }

    /// 某月所有账户快照
    pub fn month_snapshots(&self, year: i32, month: u32) -> Result<Vec<AccountSnapshot>> {
        let conn = self.pool.get()?;
        SnapshotDao::find_month(&conn, year, month)
    }

    pub fn has_snapshot(&self, year: i32, month: u32) -> Result<bool> {
        let conn = self.pool.get()?;
        SnapshotDao::has_snapshot(&conn, year, month)
    }

    // ── CSV ───────────────────────────────────────────────────────────────────

    pub fn export_csv(&self, path: &str) -> Result<usize> {
        let file = std::fs::File::create(path)?;
        crate::csv::export_csv(file, self)
    }

    pub fn import_csv(&self, path: &str, account_id: i64) -> Result<usize> {
        let file = std::fs::File::open(path)?;
        let mapping = crate::csv::FieldMapping::with_default();
        crate::csv::import_csv(file, self, &mapping, account_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_pool;
    use crate::models::account::{AccountType, NewAccount};

    fn test_service() -> AppService {
        let pool = create_pool(":memory:").unwrap();
        AppService::new(Arc::new(pool))
    }

    #[test]
    fn test_atomic_create_transaction_updates_balance() {
        let svc = test_service();

        let acc = svc
            .create_account(&NewAccount {
                name: "工资卡".into(),
                account_type: AccountType::Bank,
                currency: "CNY".into(),
                initial_balance: 1000.0,
            })
            .unwrap();

        svc.create_transaction(&NewTransaction {
            amount: 300.0,
            transaction_type: TransactionType::Expense,
            category_id: None,
            account_id: acc.id,
            to_account_id: None,
            date: "2024-01-10".into(),
            note: Some("购物".into()),
            is_large: true,
        })
        .unwrap();

        let accounts = svc.list_accounts().unwrap();
        let updated = accounts.iter().find(|a| a.id == acc.id).unwrap();
        assert!((updated.balance - 700.0).abs() < 1e-9);
    }

    #[test]
    fn test_atomic_delete_transaction_reverses_balance() {
        let svc = test_service();

        let acc = svc
            .create_account(&NewAccount {
                name: "零钱".into(),
                account_type: AccountType::Cash,
                currency: "CNY".into(),
                initial_balance: 500.0,
            })
            .unwrap();

        let tx = svc
            .create_transaction(&NewTransaction {
                amount: 100.0,
                transaction_type: TransactionType::Expense,
                category_id: None,
                account_id: acc.id,
                to_account_id: None,
                date: "2024-02-01".into(),
                note: None,
                is_large: false,
            })
            .unwrap();

        svc.delete_transaction(tx.id).unwrap();

        let accounts = svc.list_accounts().unwrap();
        let restored = accounts.iter().find(|a| a.id == acc.id).unwrap();
        assert!((restored.balance - 500.0).abs() < 1e-9);
    }

    #[test]
    fn test_save_monthly_snapshot() {
        let svc = test_service();

        let acc1 = svc
            .create_account(&NewAccount {
                name: "工资卡".into(),
                account_type: AccountType::Bank,
                currency: "CNY".into(),
                initial_balance: 0.0,
            })
            .unwrap();
        let acc2 = svc
            .create_account(&NewAccount {
                name: "社保".into(),
                account_type: AccountType::SocialInsurance,
                currency: "CNY".into(),
                initial_balance: 0.0,
            })
            .unwrap();

        let mut items = svc.build_monthly_entry_items().unwrap();
        for item in &mut items {
            if item.account_id == acc1.id {
                item.confirmed_balance = Some(8000.0);
            } else if item.account_id == acc2.id {
                item.confirmed_balance = Some(15000.0);
            }
        }

        let total = svc.save_monthly_snapshot(2024, 3, &items, None).unwrap();
        assert!((total - 23000.0).abs() < 1e-9);

        // 验证账户余额已同步更新
        let accounts = svc.list_accounts().unwrap();
        let a1 = accounts.iter().find(|a| a.id == acc1.id).unwrap();
        assert!((a1.balance - 8000.0).abs() < 1e-9);

        // 验证趋势数据
        let trend = svc.asset_trend(12).unwrap();
        assert_eq!(trend.len(), 1);
        assert!((trend[0].total - 23000.0).abs() < 1e-9);
    }
}
