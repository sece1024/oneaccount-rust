# OneAccount 重构方案（聚焦月结核心）

## 重构目标

1. **极简月结输入** - 智能默认值 + 快速跳过 + 撤销支持
2. **数据结构优化** - 为同比/环比分析添加冗余列和聚合表
3. **后端分析增强** - 前端只需渲染后端计算好的数据

---

## Phase 1: 数据模型优化

### 1.1 优化 balance_snapshots 表

当前问题：缺少时间维度冗余列，聚合查询效率低；无审计日志，无法撤销。

**变更：**
```sql
-- 添加冗余时间列，加速聚合查询
ALTER TABLE balance_snapshots ADD COLUMN year_month INTEGER NOT NULL DEFAULT 0;
ALTER TABLE balance_snapshots ADD COLUMN balance_delta REAL; -- 与上月的差额
ALTER TABLE balance_snapshots ADD COLUMN prev_balance REAL; -- 上月余额

-- 创建快照历史表（支持撤销）
CREATE TABLE IF NOT EXISTS snapshot_history (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id   INTEGER NOT NULL REFERENCES balance_snapshots(id) ON DELETE CASCADE,
    old_balance   REAL,
    new_balance   REAL NOT NULL,
    changed_at    TEXT NOT NULL,
    change_reason TEXT -- 'initial', 'edit', 'undo'
);

CREATE INDEX idx_snap_ym ON balance_snapshots(year_month);
CREATE INDEX idx_snap_history_sid ON snapshot_history(snapshot_id);
```

**迁移脚本：**
```rust
// 在 apply_incremental_migrations 中添加
let _ = conn.execute_batch("
    ALTER TABLE balance_snapshots ADD COLUMN year_month INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE balance_snapshots ADD COLUMN balance_delta REAL;
    ALTER TABLE balance_snapshots ADD COLUMN prev_balance REAL;
");

// 回填历史数据
conn.execute_batch("
    UPDATE balance_snapshots SET
        year_month = year * 100 + month;
")?;

// 计算 delta 和 prev_balance
conn.execute_batch("
    UPDATE balance_snapshots SET
        prev_balance = (
            SELECT bs2.balance FROM balance_snapshots bs2
            WHERE bs2.account_id = balance_snapshots.account_id
            AND bs2.year * 100 + bs2.month = (
                SELECT MAX(year * 100 + month) FROM balance_snapshots bs3
                WHERE bs3.account_id = balance_snapshots.account_id
                AND bs3.year * 100 + bs3.month < balance_snapshots.year_month
            )
        ),
        balance_delta = balance - COALESCE(
            (SELECT bs2.balance FROM balance_snapshots bs2
             WHERE bs2.account_id = balance_snapshots.account_id
             AND bs2.year * 100 + bs2.month = (
                 SELECT MAX(year * 100 + month) FROM balance_snapshots bs3
                 WHERE bs3.account_id = balance_snapshots.account_id
                 AND bs3.year * 100 + bs3.month < balance_snapshots.year_month
             )),
            0.0
        );
")?;
```

### 1.2 新增 monthly_analytics 聚合表

```sql
CREATE TABLE IF NOT EXISTS monthly_analytics (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    year                INTEGER NOT NULL,
    month               INTEGER NOT NULL,
    -- 总资产/负债
    total_assets        REAL NOT NULL DEFAULT 0.0,
    total_liabilities   REAL NOT NULL DEFAULT 0.0,
    net_worth           REAL NOT NULL DEFAULT 0.0,
    -- 按流动性分类
    liquid_assets       REAL NOT NULL DEFAULT 0.0,
    illiquid_assets     REAL NOT NULL DEFAULT 0.0,
    -- 环比/同比
    assets_mom_change   REAL, -- 环比变化率
    assets_yoy_change   REAL, -- 同比变化率
    net_worth_mom_change REAL,
    net_worth_yoy_change REAL,
    -- 储蓄率（如果有收入数据）
    savings_rate        REAL,
    -- 元数据
    account_count       INTEGER NOT NULL DEFAULT 0,
    updated_at          TEXT NOT NULL,
    UNIQUE(year, month)
);

CREATE INDEX idx_manalytics_ym ON monthly_analytics(year, month);
```

### 1.3 新增 account_monthly_stats 表

```sql
CREATE TABLE IF NOT EXISTS account_monthly_stats (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    year            INTEGER NOT NULL,
    month           INTEGER NOT NULL,
    account_id      INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    balance         REAL NOT NULL DEFAULT 0.0,
    balance_delta   REAL, -- 与上月差额
    growth_rate     REAL, -- 增长率
    -- 占比
    asset_ratio     REAL, -- 占总资产比例
    type_ratio      REAL, -- 占同类型资产比例
    updated_at      TEXT NOT NULL,
    UNIQUE(year, month, account_id)
);

CREATE INDEX idx_amstats_ym ON account_monthly_stats(year, month);
CREATE INDEX idx_amstats_aid ON account_monthly_stats(account_id);
```

---

## Phase 2: 月结流程优化

### 2.1 智能默认值

当前问题：每次都要逐个账户输入余额，大部分账户变化不大。

**新增服务方法：**
```rust
impl AppService {
    /// 获取智能默认值（基于上月快照）
    pub fn get_smart_defaults_for_month(&self, year: i32, month: u32) -> Result<SmartMonthDefaults> {
        let conn = self.pool.get()?;

        // 获取上月快照
        let (prev_y, prev_m) = prev_month(year, month);
        let prev_snapshots = SnapshotDao::find_month(&conn, prev_y, prev_m)?;

        // 获取所有账户
        let accounts = AccountDao::find_all(&conn)?;

        let entries: Vec<SmartEntry> = accounts.iter().map(|acc| {
            let prev = prev_snapshots.iter()
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
        }).collect();

        Ok(SmartMonthDefaults {
            year,
            month,
            entries,
            total_accounts: accounts.len(),
        })
    }
}

#[derive(Serialize)]
pub struct SmartMonthDefaults {
    pub year: i32,
    pub month: u32,
    pub entries: Vec<SmartEntry>,
    pub total_accounts: usize,
}

#[derive(Serialize)]
pub struct SmartEntry {
    pub account_id: i64,
    pub account_name: String,
    pub account_type: String,
    pub currency: String,
    pub last_balance: Option<f64>,
    pub suggested_balance: Option<f64>,
}
```

### 2.2 快速输入模式

当前问题：逐个账户输入太慢，大部分账户余额不变。

**新增 TUI 交互：**
```
┌─────────────────────────────────────────────────────┐
│  2024-03 月结 (3/10 个账户需要确认)                   │
│                                                     │
│  ▶ 银行卡    ¥15,000.00  (上次: ¥15,000.00)  [确认] │
│    微信      ¥3,200.50   (上次: ¥3,200.50)   [跳过] │
│    支付宝    ¥8,500.00   (上次: ¥8,500.00)   [跳过] │
│    信用卡    -¥2,300.00  (上次: -¥2,300.00)  [确认] │
│    股票      ¥50,000.00  (上次: ¥45,000.00)  [修改] │
│                                                     │
│  [Enter]确认  [Space]跳过  [e]编辑  [u]撤销  [Ctrl+S]保存 │
└─────────────────────────────────────────────────────┘
```

**交互逻辑：**
- **批量确认**：按 `Space` 跳过当前账户，保持上月余额
- **快速编辑**：按 `e` 进入编辑模式，修改余额
- **撤销支持**：按 `u` 撤销上一步操作（恢复到修改前的值）

### 2.3 撤销支持

当前问题：输错了无法撤销，只能重新输入。

**实现方案：**
```rust
pub struct MonthlyForm {
    pub year: i32,
    pub month: u32,
    pub entries: Vec<MonthlyEntryItem>,
    pub selected: usize,
    pub editing: bool,
    pub input_buf: String,
    pub saved: bool,
    // 新增：撤销栈
    pub undo_stack: Vec<UndoAction>,
}

#[derive(Clone)]
pub enum UndoAction {
    /// 编辑前的状态
    Edit {
        account_id: i64,
        old_balance: Option<f64>,
        new_balance: Option<f64>,
    },
    /// 确认前的状态
    Confirm {
        account_id: i64,
        old_confirmed: Option<f64>,
    },
}

impl MonthlyForm {
    /// 记录编辑操作（用于撤销）
    pub fn push_undo(&mut self, action: UndoAction) {
        self.undo_stack.push(action);
        // 限制栈大小，避免内存占用过多
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    /// 撤销上一步操作
    pub fn undo(&mut self) -> bool {
        if let Some(action) = self.undo_stack.pop() {
            match action {
                UndoAction::Edit { account_id, old_balance, .. } => {
                    if let Some(item) = self.entries.iter_mut().find(|e| e.account_id == account_id) {
                        item.confirmed_balance = old_balance;
                        item.input = old_balance.map(|b| format!("{b:.2}")).unwrap_or_default();
                    }
                }
                UndoAction::Confirm { account_id, old_confirmed } => {
                    if let Some(item) = self.entries.iter_mut().find(|e| e.account_id == account_id) {
                        item.confirmed_balance = old_confirmed;
                    }
                }
            }
            true
        } else {
            false
        }
    }
}
```

### 2.4 手动保存草稿

当前问题：切换 Tab 或退出时，未保存的数据会丢失。

**实现方案（用户手动保存）：**
```rust
impl App {
    /// 保存当前月结草稿到本地文件
    fn save_draft(&self) -> Result<()> {
        let draft = MonthlyDraft {
            year: self.monthly_form.year,
            month: self.monthly_form.month,
            entries: self.monthly_form.entries.iter()
                .filter(|e| e.confirmed_balance.is_some())
                .map(|e| DraftEntry {
                    account_id: e.account_id,
                    balance: e.confirmed_balance.unwrap(),
                })
                .collect(),
            saved_at: chrono::Local::now().to_rfc3339(),
        };

        let draft_path = self.draft_path();
        let file = std::fs::File::create(&draft_path)?;
        serde_json::to_writer(file, &draft)?;
        Ok(())
    }

    /// 加载草稿
    fn load_draft(&mut self) -> Result<()> {
        let draft_path = self.draft_path();
        if !draft_path.exists() {
            return Ok(());
        }

        let file = std::fs::File::open(&draft_path)?;
        let draft: MonthlyDraft = serde_json::from_reader(file)?;

        // 验证是否是当前月份的草稿
        if draft.year == self.monthly_form.year && draft.month == self.monthly_form.month {
            for entry in &draft.entries {
                if let Some(item) = self.monthly_form.entries.iter_mut()
                    .find(|e| e.account_id == entry.account_id) {
                    item.confirmed_balance = Some(entry.balance);
                    item.input = format!("{:.2}", entry.balance);
                }
            }
            self.status_msg = Some("已恢复草稿".into());
        }

        Ok(())
    }

    fn draft_path(&self) -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("oneaccount")
            .join("drafts")
            .join(format!("draft_{}_{}.json", self.monthly_form.year, self.monthly_form.month))
    }
}
```

---

## Phase 3: 分析引擎

### 3.1 同比/环比增长率

```rust
impl AppService {
    /// 获取月度对比数据（环比 + 同比）
    pub fn get_monthly_comparison(&self, year: i32, month: u32) -> Result<MonthlyComparison> {
        let conn = self.pool.get()?;

        // 当月快照
        let current = self.get_month_snapshot_summary(&conn, year, month)?;

        // 上月（环比）
        let (prev_y, prev_m) = prev_month(year, month);
        let prev_month = self.get_month_snapshot_summary(&conn, prev_y, prev_m)?;

        // 去年同月（同比）
        let last_year = self.get_month_snapshot_summary(&conn, year - 1, month)?;

        // 计算增长率
        let assets_mom = calc_growth_rate(current.total_assets, prev_month.total_assets);
        let assets_yoy = calc_growth_rate(current.total_assets, last_year.total_assets);
        let net_worth_mom = calc_growth_rate(current.net_worth, prev_month.net_worth);
        let net_worth_yoy = calc_growth_rate(current.net_worth, last_year.net_worth);

        Ok(MonthlyComparison {
            year,
            month,
            current,
            prev_month,
            last_year,
            assets_mom,
            assets_yoy,
            net_worth_mom,
            net_worth_yoy,
        })
    }

    /// 获取单月快照摘要
    fn get_month_snapshot_summary(&self, conn: &Connection, year: i32, month: u32) -> Result<MonthSnapshotSummary> {
        let snapshots = SnapshotDao::find_month(conn, year, month)?;

        let mut total_assets = 0.0;
        let mut total_liabilities = 0.0;
        let mut liquid_assets = 0.0;
        let mut illiquid_assets = 0.0;

        for snap in &snapshots {
            let account = AccountDao::find_by_id(conn, snap.account_id)?;
            if let Some(acc) = account {
                match acc.account_type {
                    AccountType::CreditCard | AccountType::EWallet => {
                        // 负债账户（余额为负数）
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
}

#[derive(Serialize)]
pub struct MonthlyComparison {
    pub year: i32,
    pub month: u32,
    pub current: MonthSnapshotSummary,
    pub prev_month: MonthSnapshotSummary,
    pub last_year: MonthSnapshotSummary,
    pub assets_mom: Option<f64>,      // 资产环比增长率
    pub assets_yoy: Option<f64>,      // 资产同比增长率
    pub net_worth_mom: Option<f64>,   // 净资产环比增长率
    pub net_worth_yoy: Option<f64>,   // 净资产同比增长率
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
```

### 3.2 资产结构分析

```rust
impl AppService {
    /// 获取资产结构分析
    pub fn get_asset_structure(&self, year: i32, month: u32) -> Result<AssetStructure> {
        let conn = self.pool.get()?;
        let snapshots = SnapshotDao::find_month(&conn, year, month)?;

        let mut by_type: HashMap<String, f64> = HashMap::new();
        let mut by_liquid: HashMap<String, f64> = HashMap::new();
        let mut by_currency: HashMap<String, f64> = HashMap::new();

        for snap in &snapshots {
            if let Some(account) = AccountDao::find_by_id(&conn, snap.account_id)? {
                let type_name = account.account_type.display_name().to_string();
                *by_type.entry(type_name).or_insert(0.0) += snap.balance;

                let liquid_key = if account.is_liquid { "流动资产" } else { "非流动资产" }.to_string();
                *by_liquid.entry(liquid_key).or_insert(0.0) += snap.balance;

                *by_currency.entry(account.currency.clone()).or_insert(0.0) += snap.balance;
            }
        }

        // 计算占比
        let total_assets: f64 = by_type.values().filter(|&&v| v > 0.0).sum();
        let total_liabilities: f64 = by_type.values().filter(|&&v| v < 0.0).map(|v| v.abs()).sum();

        Ok(AssetStructure {
            year,
            month,
            total_assets,
            total_liabilities,
            net_worth: total_assets - total_liabilities,
            by_type: by_type.into_iter().map(|(k, v)| TypeRatio {
                name: k,
                amount: v,
                percentage: if total_assets > 0.0 { v / total_assets } else { 0.0 },
            }).collect(),
            by_liquid: by_liquid.into_iter().map(|(k, v)| LiquidRatio {
                category: k,
                amount: v,
                percentage: if total_assets > 0.0 { v / total_assets } else { 0.0 },
            }).collect(),
            by_currency: by_currency.into_iter().map(|(k, v)| CurrencyRatio {
                currency: k,
                amount: v,
                percentage: if total_assets > 0.0 { v / total_assets } else { 0.0 },
            }).collect(),
        })
    }
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
```

### 3.3 财务健康指标

```rust
impl AppService {
    /// 获取财务健康指标
    pub fn get_financial_health(&self, year: i32, month: u32) -> Result<FinancialHealth> {
        let conn = self.pool.get()?;

        // 当月资产/负债
        let current = self.get_month_snapshot_summary(&conn, year, month)?;

        // 上月（计算变化）
        let (prev_y, prev_m) = prev_month(year, month);
        let prev = self.get_month_snapshot_summary(&conn, prev_y, prev_m)?;

        // 去年同月（计算同比）
        let last_year = self.get_month_snapshot_summary(&conn, year - 1, month)?;

        // 负债率 = 总负债 / 总资产
        let debt_ratio = if current.total_assets > 0.0 {
            current.total_liabilities / current.total_assets
        } else {
            0.0
        };

        // 流动性比率 = 流动资产 / 总资产
        let liquidity_ratio = if current.total_assets > 0.0 {
            current.liquid_assets / current.total_assets
        } else {
            0.0
        };

        // 资产增长率（环比）
        let asset_growth_mom = calc_growth_rate(current.total_assets, prev.total_assets);

        // 资产增长率（同比）
        let asset_growth_yoy = calc_growth_rate(current.total_assets, last_year.total_assets);

        // 净资产增长率（环比）
        let net_worth_growth_mom = calc_growth_rate(current.net_worth, prev.net_worth);

        // 净资产增长率（同比）
        let net_worth_growth_yoy = calc_growth_rate(current.net_worth, last_year.net_worth);

        // 财务健康评分（简单规则）
        let health_score = self.calculate_health_score(
            debt_ratio,
            liquidity_ratio,
            asset_growth_mom,
            net_worth_growth_mom,
        );

        Ok(FinancialHealth {
            year,
            month,
            debt_ratio,
            liquidity_ratio,
            asset_growth_mom,
            asset_growth_yoy,
            net_worth_growth_mom,
            net_worth_growth_yoy,
            health_score,
            health_level: self.score_to_level(health_score),
        })
    }

    fn calculate_health_score(&self, debt_ratio: f64, liquidity_ratio: f64, asset_growth: f64, net_worth_growth: f64) -> f64 {
        let mut score = 100.0;

        // 负债率扣分（负债率 > 50% 开始扣分）
        if debt_ratio > 0.5 {
            score -= (debt_ratio - 0.5) * 100.0;
        }

        // 流动性加分（流动性 > 30% 加分）
        if liquidity_ratio > 0.3 {
            score += (liquidity_ratio - 0.3) * 20.0;
        }

        // 增长加分
        if let Some(growth) = asset_growth {
            if growth > 0.0 {
                score += growth * 10.0;
            } else {
                score += growth * 5.0; // 下降扣分较少
            }
        }

        score.max(0.0).min(100.0)
    }

    fn score_to_level(&self, score: f64) -> String {
        if score >= 80.0 { "优秀".into() }
        else if score >= 60.0 { "良好".into() }
        else if score >= 40.0 { "一般".into() }
        else { "需改善".into() }
    }
}

#[derive(Serialize)]
pub struct FinancialHealth {
    pub year: i32,
    pub month: u32,
    pub debt_ratio: f64,           // 负债率
    pub liquidity_ratio: f64,      // 流动性比率
    pub asset_growth_mom: Option<f64>,  // 资产环比增长率
    pub asset_growth_yoy: Option<f64>,  // 资产同比增长率
    pub net_worth_growth_mom: Option<f64>,
    pub net_worth_growth_yoy: Option<f64>,
    pub health_score: f64,         // 健康评分 0-100
    pub health_level: String,      // 健康等级
}
```

### 3.4 多维度分析

```rust
impl AppService {
    /// 获取多维度分析
    pub fn get_multi_dimension_analysis(&self, year: i32, month: u32) -> Result<MultiDimensionAnalysis> {
        let conn = self.pool.get()?;

        // 按账户类型分析
        let by_account_type = self.get_analysis_by_account_type(&conn, year, month)?;

        // 按币种分析
        let by_currency = self.get_analysis_by_currency(&conn, year, month)?;

        // 按流动性分析
        let by_liquidity = self.get_analysis_by_liquidity(&conn, year, month)?;

        // 趋势分析（最近12个月）
        let trend = self.get_trend_analysis(&conn, 12)?;

        Ok(MultiDimensionAnalysis {
            year,
            month,
            by_account_type,
            by_currency,
            by_liquidity,
            trend,
        })
    }

    fn get_analysis_by_account_type(&self, conn: &Connection, year: i32, month: u32) -> Result<Vec<AccountTypeAnalysis>> {
        let snapshots = SnapshotDao::find_month(conn, year, month)?;
        let mut by_type: HashMap<String, Vec<AccountAnalysis>> = HashMap::new();

        for snap in &snapshots {
            if let Some(account) = AccountDao::find_by_id(conn, snap.account_id)? {
                let type_name = account.account_type.display_name().to_string();
                let analysis = AccountAnalysis {
                    account_id: snap.account_id,
                    account_name: snap.account_name.clone(),
                    balance: snap.balance,
                    // 计算与上月的差额和增长率
                    delta: snap.balance_delta,
                    growth_rate: if let Some(prev) = snap.prev_balance {
                        calc_growth_rate(snap.balance, prev)
                    } else {
                        None
                    },
                };
                by_type.entry(type_name).or_default().push(analysis);
            }
        }

        Ok(by_type.into_iter().map(|(type_name, accounts)| {
            let total: f64 = accounts.iter().map(|a| a.balance).sum();
            AccountTypeAnalysis {
                type_name,
                accounts,
                total,
            }
        }).collect())
    }
}

#[derive(Serialize)]
pub struct MultiDimensionAnalysis {
    pub year: i32,
    pub month: u32,
    pub by_account_type: Vec<AccountTypeAnalysis>,
    pub by_currency: Vec<CurrencyAnalysis>,
    pub by_liquidity: Vec<LiquidityAnalysis>,
    pub trend: Vec<TrendAnalysis>,
}

#[derive(Serialize)]
pub struct AccountTypeAnalysis {
    pub type_name: String,
    pub accounts: Vec<AccountAnalysis>,
    pub total: f64,
}

#[derive(Serialize)]
pub struct AccountAnalysis {
    pub account_id: i64,
    pub account_name: String,
    pub balance: f64,
    pub delta: Option<f64>,
    pub growth_rate: Option<f64>,
}
```

---

## Phase 4: API 增强

### 4.1 结构化错误响应

```rust
#[derive(Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, "VALIDATION_ERROR", msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "NOT_FOUND", msg.clone()),
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR", "数据库错误".into()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "服务器错误".into()),
        };

        let body = Json(ApiError {
            code: code.into(),
            message,
            details: None,
        });

        (status, body).into_response()
    }
}
```

### 4.2 新增分析接口

| Method | Endpoint | 说明 |
|--------|----------|------|
| `GET` | `/analytics/summary?year=&month=` | 月度摘要（含环比/同比） |
| `GET` | `/analytics/assets?year=&month=` | 资产结构分析 |
| `GET` | `/analytics/health?year=&month=` | 财务健康指标 |
| `GET` | `/analytics/multi?year=&month=` | 多维度分析 |
| `GET` | `/analytics/trend?months=12` | 资产趋势（含增长率） |
| `GET` | `/smart/defaults?year=&month=` | 智能默认值 |

**响应示例：**
```json
// GET /analytics/summary?year=2024&month=3
{
  "year": 2024,
  "month": 3,
  "current": {
    "total_assets": 500000.0,
    "total_liabilities": 50000.0,
    "net_worth": 450000.0,
    "liquid_assets": 200000.0,
    "illiquid_assets": 300000.0,
    "account_count": 8
  },
  "prev_month": {
    "total_assets": 480000.0,
    "total_liabilities": 45000.0,
    "net_worth": 435000.0,
    "liquid_assets": 190000.0,
    "illiquid_assets": 290000.0,
    "account_count": 8
  },
  "last_year": {
    "total_assets": 400000.0,
    "total_liabilities": 60000.0,
    "net_worth": 340000.0,
    "liquid_assets": 150000.0,
    "illiquid_assets": 250000.0,
    "account_count": 7
  },
  "assets_mom": 0.042,
  "assets_yoy": 0.25,
  "net_worth_mom": 0.034,
  "net_worth_yoy": 0.324
}
```

---

## Phase 5: TUI 优化

### 5.1 新增"分析"Tab

```
┌─────────────────────────────────────────────────────┐
│  2024-03 财务分析                                    │
│                                                     │
│  总资产: ¥500,000  ↑4.2% 环比  ↑25% 同比            │
│  总负债: ¥50,000   ↑11% 环比   ↓17% 同比            │
│  净资产: ¥450,000  ↑3.4% 环比  ↑32% 同比            │
│                                                     │
│  资产结构:                                           │
│  流动资产: ¥200,000 (40%)                            │
│  非流动资产: ¥300,000 (60%)                          │
│                                                     │
│  负债率: 10%  |  流动性: 40%  |  健康评分: 85/100     │
│                                                     │
│  ← 上月  |  下月 →                                  │
└─────────────────────────────────────────────────────┘
```

### 5.2 优化月结 Tab

```
┌─────────────────────────────────────────────────────┐
│  2024-03 月结                                        │
│                                                     │
│  ▶ 银行卡    ¥15,000.00  (上次: ¥15,000.00)        │
│    微信      ¥3,200.50   (上次: ¥3,200.50)         │
│    支付宝    ¥8,500.00   (上次: ¥8,500.00)         │
│    信用卡    -¥2,300.00  (上次: -¥2,300.00)        │
│    股票      ¥50,000.00  (上次: ¥45,000.00)        │
│                                                     │
│  [Enter]编辑  [Space]跳过  [u]撤销  [Ctrl+S]保存     │
│  [Ctrl+D]保存草稿                                    │
└─────────────────────────────────────────────────────┘
```

### 5.3 快捷键增强

| 快捷键 | 功能 |
|--------|------|
| `Space` | 跳过当前账户，保持上月余额 |
| `e` | 编辑当前账户余额 |
| `u` | 撤销上一步操作 |
| `Ctrl+S` | 保存月结快照 |
| `Ctrl+D` | 保存草稿到本地文件 |

---

## 实施顺序

1. **Week 1**: Phase 1 - 数据模型优化
   - 添加冗余列
   - 创建历史表和聚合表
   - 数据迁移脚本

2. **Week 2**: Phase 2 - 月结流程优化
   - 智能默认值
   - 快速输入模式
   - 撤销支持
   - 草稿保存

3. **Week 3**: Phase 3 - 分析引擎
   - 同比/环比计算
   - 资产结构分析
   - 财务健康指标
   - 多维度分析

4. **Week 4**: Phase 4 - API 增强
   - 结构化错误
   - 分析接口

5. **Week 5**: Phase 5 - TUI 优化
   - 分析 Tab
   - 月结 Tab 优化
   - 快捷键增强

---

## 风险与注意事项

1. **数据迁移**: 需要确保旧数据不丢失，建议先备份再迁移
2. **性能**: 聚合表需要在快照保存时同步更新
3. **一致性**: `monthly_analytics` 和 `account_monthly_stats` 需要在同一个事务中更新
4. **向后兼容**: API 新增接口，不修改现有接口
5. **草稿管理**: 草稿保存在 `~/Library/Application Support/oneaccount/drafts/` 目录下，用户手动删除
