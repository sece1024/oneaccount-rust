use std::sync::Arc;

use serde::Serialize;

use crate::dao::{AccountDao, CategoryDao, SnapshotDao, TransactionDao};
use crate::db::DbPool;
use crate::error::Result;
use crate::models::account::{Account, NewAccount};
use crate::models::category::{Category, CategoryType, NewCategory};
use crate::models::snapshot::{AccountSnapshot, MonthlyEntryItem, MonthlyTotal};
use crate::models::transaction::{NewTransaction, Transaction, TransactionFilter, TransactionType};

#[derive(Serialize)]
pub struct SnapshotGridRow {
    pub year: i32,
    pub month: u32,
    pub balances: std::collections::HashMap<i64, f64>,
    /// CNY 账户合计
    pub total: f64,
    /// CNY 活动资产合计
    pub liquid_total: f64,
    /// CNY 非活动资产合计
    pub illiquid_total: f64,
    /// 各币种余额合计（含 CNY），用于多货币展示
    pub currency_totals: std::collections::HashMap<String, f64>,
}

/// 月结智能默认值
#[derive(Serialize)]
pub struct SmartMonthDefaults {
    pub year: i32,
    pub month: u32,
    pub entries: Vec<SmartEntry>,
    pub total_accounts: usize,
}

/// 智能预填项
#[derive(Serialize)]
pub struct SmartEntry {
    pub account_id: i64,
    pub account_name: String,
    pub account_type: String,
    pub currency: String,
    pub last_balance: Option<f64>,
    pub suggested_balance: Option<f64>,
}

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

    #[allow(dead_code)]
    pub fn total_balance(&self) -> Result<f64> {
        let conn = self.pool.get()?;
        AccountDao::total_balance(&conn)
    }

    pub fn delete_account(&self, id: i64) -> Result<()> {
        let conn = self.pool.get()?;
        AccountDao::delete(&conn, id)
    }

    pub fn update_account_liquid(&self, id: i64, is_liquid: bool) -> Result<()> {
        let conn = self.pool.get()?;
        AccountDao::update_liquid(&conn, id, is_liquid)
    }

    // ── Categories ────────────────────────────────────────────────────────────

    pub fn list_categories(&self) -> Result<Vec<Category>> {
        let conn = self.pool.get()?;
        CategoryDao::find_all(&conn)
    }

    #[allow(dead_code)]
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
        if req.amount <= 0.0 {
            return Err(crate::error::AppError::InvalidInput(
                "交易金额必须为正数".into(),
            ));
        }
        if req.transaction_type == TransactionType::Transfer {
            let to_id = req.to_account_id.ok_or_else(|| {
                crate::error::AppError::InvalidInput(
                    "转账交易必须指定目标账户".into(),
                )
            })?;
            if req.account_id == to_id {
                return Err(crate::error::AppError::InvalidInput(
                    "转账的源账户和目标账户不能相同".into(),
                ));
            }
        }

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
                // to_account_id 已在上方校验过非空
                let to_id = req.to_account_id.unwrap();
                AccountDao::adjust_balance(&db_tx, to_id, req.amount)?;
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

    /// 原子化更新账目：先回滚旧余额变动，再应用新的
    pub fn update_transaction(&self, id: i64, req: &NewTransaction) -> Result<Transaction> {
        if req.amount <= 0.0 {
            return Err(crate::error::AppError::InvalidInput(
                "交易金额必须为正数".into(),
            ));
        }
        if req.transaction_type == TransactionType::Transfer {
            let to_id = req.to_account_id.ok_or_else(|| {
                crate::error::AppError::InvalidInput("转账交易必须指定目标账户".into())
            })?;
            if req.account_id == to_id {
                return Err(crate::error::AppError::InvalidInput(
                    "转账的源账户和目标账户不能相同".into(),
                ));
            }
        }

        let mut conn = self.pool.get()?;
        let db_tx = conn.transaction()?;

        let old = TransactionDao::find_by_id(&db_tx, id)?
            .ok_or_else(|| crate::error::AppError::NotFound("Transaction".into()))?;

        // 回滚旧交易的余额影响
        match old.transaction_type {
            TransactionType::Expense => {
                AccountDao::adjust_balance(&db_tx, old.account_id, old.amount)?;
            }
            TransactionType::Income => {
                AccountDao::adjust_balance(&db_tx, old.account_id, -old.amount)?;
            }
            TransactionType::Transfer => {
                AccountDao::adjust_balance(&db_tx, old.account_id, old.amount)?;
                if let Some(to_id) = old.to_account_id {
                    AccountDao::adjust_balance(&db_tx, to_id, -old.amount)?;
                }
            }
        }

        // 应用新交易的余额影响
        match req.transaction_type {
            TransactionType::Expense => {
                AccountDao::adjust_balance(&db_tx, req.account_id, -req.amount)?;
            }
            TransactionType::Income => {
                AccountDao::adjust_balance(&db_tx, req.account_id, req.amount)?;
            }
            TransactionType::Transfer => {
                AccountDao::adjust_balance(&db_tx, req.account_id, -req.amount)?;
                let to_id = req.to_account_id.unwrap();
                AccountDao::adjust_balance(&db_tx, to_id, req.amount)?;
            }
        }

        let updated = TransactionDao::update(&db_tx, id, req)?;
        db_tx.commit()?;
        Ok(updated)
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

    #[allow(dead_code)]
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

    /// 获取月结智能默认值（基于上月快照预填）
    pub fn get_smart_defaults_for_month(&self, year: i32, month: u32) -> Result<SmartMonthDefaults> {
        let conn = self.pool.get()?;

        // 获取上月快照
        let (prev_y, prev_m) = prev_month(year, month);
        let prev_snapshots = SnapshotDao::find_month(&conn, prev_y, prev_m)?;

        // 获取所有账户
        let accounts = AccountDao::find_all(&conn)?;

        let entries: Vec<SmartEntry> = accounts
            .iter()
            .map(|acc| {
                let prev = prev_snapshots
                    .iter()
                    .find(|s| s.account_id == acc.id)
                    .map(|s| s.balance);

                SmartEntry {
                    account_id: acc.id,
                    account_name: acc.name.clone(),
                    account_type: acc.account_type.display_name().to_string(),
                    currency: acc.currency.clone(),
                    last_balance: prev,
                    // 智能预填：如果有上月余额，直接预填
                    suggested_balance: prev,
                }
            })
            .collect();

        Ok(SmartMonthDefaults {
            year,
            month,
            entries,
            total_accounts: accounts.len(),
        })
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

    /// 获取最近 N 个月所有账户快照，结构化为行列格式（含活动/非活动分类合计）
    pub fn snapshot_grid(&self, months: i64) -> Result<Vec<SnapshotGridRow>> {
        let conn = self.pool.get()?;
        let accounts = AccountDao::find_all(&conn)?;
        let liquid_map: std::collections::HashMap<i64, bool> =
            accounts.iter().map(|a| (a.id, a.is_liquid)).collect();
        // 记录每个账户的货币类型，用于多货币分组统计
        let currency_map: std::collections::HashMap<i64, String> =
            accounts.iter().map(|a| (a.id, a.currency.clone())).collect();

        // 单次查询获取所有快照，按 (year, month) 分组
        let all_snaps = SnapshotDao::find_recent_months(&conn, months)?;

        let mut rows: Vec<SnapshotGridRow> = Vec::new();
        let mut current_key: Option<(i32, u32)> = None;
        let mut balances = std::collections::HashMap::new();
        let mut currency_totals: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        let mut liquid_total = 0.0f64;
        let mut illiquid_total = 0.0f64;

        for s in &all_snaps {
            let key = (s.year, s.month);
            if current_key != Some(key) {
                if let Some((y, m)) = current_key {
                    rows.push(SnapshotGridRow {
                        year: y,
                        month: m,
                        total: liquid_total + illiquid_total,
                        liquid_total,
                        illiquid_total,
                        balances: std::mem::take(&mut balances),
                        currency_totals: std::mem::take(&mut currency_totals),
                    });
                }
                current_key = Some(key);
                liquid_total = 0.0;
                illiquid_total = 0.0;
            }
            balances.insert(s.account_id, s.balance);
            let currency = currency_map
                .get(&s.account_id)
                .map(|c| c.as_str())
                .unwrap_or("CNY");
            *currency_totals.entry(currency.to_string()).or_insert(0.0) += s.balance;
            // total / liquid_total / illiquid_total 只统计 CNY，避免跨币种混算
            if currency == "CNY" {
                if *liquid_map.get(&s.account_id).unwrap_or(&true) {
                    liquid_total += s.balance;
                } else {
                    illiquid_total += s.balance;
                }
            }
        }
        if let Some((y, m)) = current_key {
            rows.push(SnapshotGridRow {
                year: y,
                month: m,
                total: liquid_total + illiquid_total,
                liquid_total,
                illiquid_total,
                balances,
                currency_totals,
            });
        }
        Ok(rows)
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

/// 计算上月的年月
fn prev_month(year: i32, month: u32) -> (i32, u32) {
    if month == 1 {
        (year - 1, 12)
    } else {
        (year, month - 1)
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
            is_liquid: true,
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
            is_liquid: true,
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
            is_liquid: true,
            })
            .unwrap();
        let acc2 = svc
            .create_account(&NewAccount {
                name: "社保".into(),
                account_type: AccountType::SocialInsurance,
                currency: "CNY".into(),
                initial_balance: 0.0,
            is_liquid: true,
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
