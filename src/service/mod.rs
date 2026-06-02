use std::sync::Arc;

use chrono::Datelike;
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

#[derive(Serialize)]
pub struct MonthSnapshotSummary {
    pub year: i32,
    pub month: u32,
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub net_worth: f64,
    pub liquid_assets: f64,
    pub illiquid_assets: f64,
    pub account_count: usize,
}

#[derive(Serialize)]
pub struct MonthlyComparison {
    pub year: i32,
    pub month: u32,
    pub current: MonthSnapshotSummary,
    pub prev_month: MonthSnapshotSummary,
    pub last_year: MonthSnapshotSummary,
    pub assets_mom: Option<f64>,
    pub assets_yoy: Option<f64>,
    pub net_worth_mom: Option<f64>,
    pub net_worth_yoy: Option<f64>,
}

#[derive(Serialize)]
pub struct TypeRatio {
    pub name: String,
    pub amount: f64,
    pub percentage: f64,
}

#[derive(Serialize)]
pub struct LiquidRatio {
    pub category: String,
    pub amount: f64,
    pub percentage: f64,
}

#[derive(Serialize)]
pub struct CurrencyRatio {
    pub currency: String,
    pub amount: f64,
    pub percentage: f64,
}

#[derive(Serialize)]
pub struct AssetStructure {
    pub year: i32,
    pub month: u32,
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub net_worth: f64,
    pub by_type: Vec<TypeRatio>,
    pub by_liquid: Vec<LiquidRatio>,
    pub by_currency: Vec<CurrencyRatio>,
}

#[derive(Serialize)]
pub struct FinancialHealth {
    pub year: i32,
    pub month: u32,
    pub debt_ratio: f64,
    pub liquidity_ratio: f64,
    pub asset_growth_mom: Option<f64>,
    pub asset_growth_yoy: Option<f64>,
    pub net_worth_growth_mom: Option<f64>,
    pub net_worth_growth_yoy: Option<f64>,
    pub health_score: f64,
    pub health_level: String,
}

#[derive(Serialize)]
pub struct TrendPoint {
    pub year: i32,
    pub month: u32,
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub net_worth: f64,
    pub assets_mom: Option<f64>,
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

    /// 获取单月快照摘要（内部辅助）
    fn get_month_snapshot_summary(
        &self,
        conn: &rusqlite::Connection,
        year: i32,
        month: u32,
    ) -> Result<MonthSnapshotSummary> {
        use crate::models::account::AccountType;

        let snapshots = SnapshotDao::find_month(conn, year, month)?;
        let mut total_assets = 0.0_f64;
        let mut total_liabilities = 0.0_f64;
        let mut liquid_assets = 0.0_f64;
        let mut illiquid_assets = 0.0_f64;

        for snap in &snapshots {
            if let Some(acc) = AccountDao::find_by_id(conn, snap.account_id)? {
                match acc.account_type {
                    AccountType::CreditCard => {
                        total_liabilities += snap.balance.abs();
                    }
                    _ => {
                        total_assets += snap.balance;
                        if acc.is_liquid {
                            liquid_assets += snap.balance;
                        } else {
                            illiquid_assets += snap.balance;
                        }
                    }
                }
            }
        }

        Ok(MonthSnapshotSummary {
            year,
            month,
            total_assets,
            total_liabilities,
            net_worth: total_assets - total_liabilities,
            liquid_assets,
            illiquid_assets,
            account_count: snapshots.len(),
        })
    }

    /// 获取月度对比数据（环比 + 同比）
    pub fn get_monthly_comparison(&self, year: i32, month: u32) -> Result<MonthlyComparison> {
        let conn = self.pool.get()?;
        let current = self.get_month_snapshot_summary(&conn, year, month)?;
        let (prev_y, prev_m) = prev_month(year, month);
        let prev = self.get_month_snapshot_summary(&conn, prev_y, prev_m)?;
        let last_year = self.get_month_snapshot_summary(&conn, year - 1, month)?;

        Ok(MonthlyComparison {
            year,
            month,
            assets_mom: calc_growth_rate(current.total_assets, prev.total_assets),
            assets_yoy: calc_growth_rate(current.total_assets, last_year.total_assets),
            net_worth_mom: calc_growth_rate(current.net_worth, prev.net_worth),
            net_worth_yoy: calc_growth_rate(current.net_worth, last_year.net_worth),
            current,
            prev_month: prev,
            last_year,
        })
    }

    /// 获取资产结构分析
    pub fn get_asset_structure(&self, year: i32, month: u32) -> Result<AssetStructure> {
        use crate::models::account::AccountType;
        use std::collections::HashMap;

        let conn = self.pool.get()?;
        let snapshots = SnapshotDao::find_month(&conn, year, month)?;

        let mut by_type: HashMap<String, f64> = HashMap::new();
        let mut by_liquid: HashMap<String, f64> = HashMap::new();
        let mut by_currency: HashMap<String, f64> = HashMap::new();
        let mut total_assets = 0.0_f64;
        let mut total_liabilities = 0.0_f64;

        for snap in &snapshots {
            if let Some(acc) = AccountDao::find_by_id(&conn, snap.account_id)? {
                let type_name = acc.account_type.display_name().to_string();
                *by_type.entry(type_name).or_insert(0.0) += snap.balance;

                match acc.account_type {
                    AccountType::CreditCard => {
                        total_liabilities += snap.balance.abs();
                    }
                    _ => {
                        total_assets += snap.balance;
                        let liquid_key = if acc.is_liquid { "流动资产" } else { "非流动资产" }.to_string();
                        *by_liquid.entry(liquid_key).or_insert(0.0) += snap.balance;
                    }
                }
                *by_currency.entry(acc.currency.clone()).or_insert(0.0) += snap.balance;
            }
        }

        let by_type_vec = by_type
            .into_iter()
            .map(|(name, amount)| TypeRatio {
                percentage: if total_assets > 1e-9 { amount / total_assets } else { 0.0 },
                name,
                amount,
            })
            .collect();

        let by_liquid_vec = by_liquid
            .into_iter()
            .map(|(category, amount)| LiquidRatio {
                percentage: if total_assets > 1e-9 { amount / total_assets } else { 0.0 },
                category,
                amount,
            })
            .collect();

        let by_currency_vec = by_currency
            .into_iter()
            .map(|(currency, amount)| CurrencyRatio {
                percentage: if total_assets > 1e-9 { amount / total_assets } else { 0.0 },
                currency,
                amount,
            })
            .collect();

        Ok(AssetStructure {
            year,
            month,
            total_assets,
            total_liabilities,
            net_worth: total_assets - total_liabilities,
            by_type: by_type_vec,
            by_liquid: by_liquid_vec,
            by_currency: by_currency_vec,
        })
    }

    /// 获取财务健康指标
    pub fn get_financial_health(&self, year: i32, month: u32) -> Result<FinancialHealth> {
        let conn = self.pool.get()?;
        let current = self.get_month_snapshot_summary(&conn, year, month)?;
        let (prev_y, prev_m) = prev_month(year, month);
        let prev = self.get_month_snapshot_summary(&conn, prev_y, prev_m)?;
        let last_year = self.get_month_snapshot_summary(&conn, year - 1, month)?;

        let debt_ratio = if current.total_assets > 1e-9 {
            current.total_liabilities / current.total_assets
        } else {
            0.0
        };

        let liquidity_ratio = if current.total_assets > 1e-9 {
            current.liquid_assets / current.total_assets
        } else {
            0.0
        };

        let asset_growth_mom = calc_growth_rate(current.total_assets, prev.total_assets);
        let asset_growth_yoy = calc_growth_rate(current.total_assets, last_year.total_assets);
        let net_worth_growth_mom = calc_growth_rate(current.net_worth, prev.net_worth);
        let net_worth_growth_yoy = calc_growth_rate(current.net_worth, last_year.net_worth);

        let mut score = 100.0_f64;
        if debt_ratio > 0.5 { score -= (debt_ratio - 0.5) * 100.0; }
        if liquidity_ratio > 0.3 { score += (liquidity_ratio - 0.3) * 20.0; }
        if let Some(g) = asset_growth_mom {
            if g > 0.0 { score += g * 10.0; } else { score += g * 5.0; }
        }
        let health_score = score.clamp(0.0, 100.0);
        let health_level = if health_score >= 80.0 { "优秀" }
            else if health_score >= 60.0 { "良好" }
            else if health_score >= 40.0 { "一般" }
            else { "需改善" }.to_string();

        Ok(FinancialHealth {
            year, month,
            debt_ratio, liquidity_ratio,
            asset_growth_mom, asset_growth_yoy,
            net_worth_growth_mom, net_worth_growth_yoy,
            health_score, health_level,
        })
    }

    /// 获取资产趋势（最近 N 个月，含增长率）
    pub fn get_asset_trend(&self, months: u32) -> Result<Vec<TrendPoint>> {
        let now = chrono::Local::now();
        let conn = self.pool.get()?;
        let mut result = Vec::new();
        let mut prev_assets: Option<f64> = None;

        let mut points = Vec::new();
        let mut y = now.year();
        let mut m = now.month();
        for _ in 0..months {
            points.push((y, m));
            let (py, pm) = prev_month(y, m);
            y = py;
            m = pm;
        }
        points.reverse();

        for (year, month) in points {
            let summary = self.get_month_snapshot_summary(&conn, year, month)?;
            let assets_mom = prev_assets.and_then(|prev| calc_growth_rate(summary.total_assets, prev));
            prev_assets = Some(summary.total_assets);
            result.push(TrendPoint {
                year,
                month,
                total_assets: summary.total_assets,
                total_liabilities: summary.total_liabilities,
                net_worth: summary.net_worth,
                assets_mom,
            });
        }

        Ok(result)
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

    /// 清空所有业务数据（账户、账目、分类、月结快照及关联表），操作不可撤销。
    pub fn clear_all_data(&self) -> Result<()> {
        let mut conn = self.pool.get()?;
        let tx = conn.transaction()?;
        tx.execute_batch(
            "DELETE FROM snapshot_history;
             DELETE FROM balance_snapshots;
             DELETE FROM account_monthly_stats;
             DELETE FROM monthly_analytics;
             DELETE FROM transactions;
             DELETE FROM accounts;
             DELETE FROM categories;",
        )?;
        tx.commit()?;
        Ok(())
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

fn calc_growth_rate(current: f64, previous: f64) -> Option<f64> {
    if previous.abs() < 1e-9 {
        None
    } else {
        Some((current - previous) / previous.abs())
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
    }

    #[test]
    fn test_clear_all_data() {
        let svc = test_service();

        // 创建账户、分类、账目、月结快照
        let acc = svc
            .create_account(&NewAccount {
                name: "工资卡".into(),
                account_type: AccountType::Bank,
                currency: "CNY".into(),
                initial_balance: 5000.0,
                is_liquid: true,
            })
            .unwrap();

        let cat = svc
            .create_category(&crate::models::category::NewCategory {
                name: "餐饮".into(),
                category_type: crate::models::category::CategoryType::Expense,
                icon: None,
                parent_id: None,
            })
            .unwrap();

        svc.create_transaction(&NewTransaction {
            amount: 100.0,
            transaction_type: TransactionType::Expense,
            category_id: Some(cat.id),
            account_id: acc.id,
            to_account_id: None,
            date: "2024-01-01".into(),
            note: None,
            is_large: false,
        })
        .unwrap();

        let items = vec![crate::models::snapshot::MonthlyEntryItem {
            account_id: acc.id,
            account_name: acc.name.clone(),
            account_type: "bank".into(),
            last_balance: None,
            input: "5000".into(),
            confirmed_balance: Some(5000.0),
        }];
        svc.save_monthly_snapshot(2024, 1, &items, None).unwrap();

        // 清空数据
        svc.clear_all_data().unwrap();

        // 验证所有表都为空
        assert!(svc.list_accounts().unwrap().is_empty());
        assert!(svc.list_categories().unwrap().is_empty());
        let filter = crate::models::transaction::TransactionFilter {
            start_date: None,
            end_date: None,
            category_id: None,
            account_id: None,
            transaction_type: None,
            limit: None,
            offset: None,
            is_large_only: false,
        };
        assert!(svc.list_transactions(&filter).unwrap().is_empty());
        assert!(!svc.has_snapshot(2024, 1).unwrap());
    }
}
