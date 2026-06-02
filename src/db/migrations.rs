use rusqlite::Connection;

use crate::error::Result;

/// 建表 + 索引（幂等，使用 IF NOT EXISTS）
pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    // 增量迁移：对已有 DB 安全地追加列/表
    apply_incremental_migrations(conn)?;
    Ok(())
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS accounts (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    name         TEXT    NOT NULL,
    account_type TEXT    NOT NULL,
    currency     TEXT    NOT NULL DEFAULT 'CNY',
    balance      REAL    NOT NULL DEFAULT 0.0,
    is_liquid    INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT    NOT NULL,
    updated_at   TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS categories (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    category_type TEXT NOT NULL,
    icon          TEXT,
    parent_id     INTEGER REFERENCES categories(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS transactions (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    amount           REAL    NOT NULL,
    transaction_type TEXT    NOT NULL,
    category_id      INTEGER REFERENCES categories(id) ON DELETE SET NULL,
    account_id       INTEGER NOT NULL REFERENCES accounts(id) ON DELETE RESTRICT,
    to_account_id    INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    date             TEXT    NOT NULL,
    note             TEXT,
    is_large         INTEGER NOT NULL DEFAULT 0,
    created_at       TEXT    NOT NULL,
    updated_at       TEXT    NOT NULL
);

-- 月结快照：记录每个账户每月底余额
CREATE TABLE IF NOT EXISTS balance_snapshots (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    year           INTEGER NOT NULL,
    month          INTEGER NOT NULL,
    account_id     INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    balance        REAL    NOT NULL,
    year_month     INTEGER NOT NULL DEFAULT 0, -- 冗余列: year * 100 + month，加速聚合
    balance_delta  REAL,                       -- 与上月的差额
    prev_balance   REAL,                       -- 上月余额
    note           TEXT,
    created_at     TEXT    NOT NULL,
    UNIQUE(year, month, account_id)
);

-- 快照历史表：支持撤销操作
CREATE TABLE IF NOT EXISTS snapshot_history (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id   INTEGER NOT NULL REFERENCES balance_snapshots(id) ON DELETE CASCADE,
    old_balance   REAL,
    new_balance   REAL NOT NULL,
    changed_at    TEXT NOT NULL,
    change_reason TEXT -- 'initial', 'edit', 'undo'
);

-- 月度分析聚合表：预计算同比/环比数据
CREATE TABLE IF NOT EXISTS monthly_analytics (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    year                  INTEGER NOT NULL,
    month                 INTEGER NOT NULL,
    total_assets          REAL NOT NULL DEFAULT 0.0,
    total_liabilities     REAL NOT NULL DEFAULT 0.0,
    net_worth             REAL NOT NULL DEFAULT 0.0,
    liquid_assets         REAL NOT NULL DEFAULT 0.0,
    illiquid_assets       REAL NOT NULL DEFAULT 0.0,
    assets_mom_change     REAL, -- 环比变化率
    assets_yoy_change     REAL, -- 同比变化率
    net_worth_mom_change  REAL,
    net_worth_yoy_change  REAL,
    account_count         INTEGER NOT NULL DEFAULT 0,
    updated_at            TEXT NOT NULL,
    UNIQUE(year, month)
);

-- 账户月度统计表：按账户维度预计算
CREATE TABLE IF NOT EXISTS account_monthly_stats (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    year            INTEGER NOT NULL,
    month           INTEGER NOT NULL,
    account_id      INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    balance         REAL NOT NULL DEFAULT 0.0,
    balance_delta   REAL, -- 与上月差额
    growth_rate     REAL, -- 增长率
    asset_ratio     REAL, -- 占总资产比例
    updated_at      TEXT NOT NULL,
    UNIQUE(year, month, account_id)
);

CREATE INDEX IF NOT EXISTS idx_tx_date         ON transactions(date);
CREATE INDEX IF NOT EXISTS idx_tx_account_id   ON transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_tx_category_id  ON transactions(category_id);
CREATE INDEX IF NOT EXISTS idx_snap_ym         ON balance_snapshots(year, month);
CREATE INDEX IF NOT EXISTS idx_snap_account    ON balance_snapshots(account_id);
CREATE INDEX IF NOT EXISTS idx_snap_year_month ON balance_snapshots(year_month);
CREATE INDEX IF NOT EXISTS idx_snap_history_sid ON snapshot_history(snapshot_id);
CREATE INDEX IF NOT EXISTS idx_manalytics_ym   ON monthly_analytics(year, month);
CREATE INDEX IF NOT EXISTS idx_amstats_ym      ON account_monthly_stats(year, month);
CREATE INDEX IF NOT EXISTS idx_amstats_aid     ON account_monthly_stats(account_id);
";

/// 对可能已存在的旧 DB 进行安全的增量迁移（忽略"列已存在"错误）
fn apply_incremental_migrations(conn: &Connection) -> Result<()> {
    // 为旧版 transactions 表添加 is_large 列（如已存在则忽略）
    let _ = conn.execute_batch(
        "ALTER TABLE transactions ADD COLUMN is_large INTEGER NOT NULL DEFAULT 0;",
    );
    let _ = conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_tx_is_large ON transactions(is_large);",
    );
    // 为旧版 accounts 表添加 is_liquid 列（活动/非活动资金）
    let _ = conn.execute_batch(
        "ALTER TABLE accounts ADD COLUMN is_liquid INTEGER NOT NULL DEFAULT 1;",
    );
    // 根据账户类型自动设置默认值：社保/基金/股票/虚拟货币 默认为非活动资金
    let _ = conn.execute_batch(
        "UPDATE accounts SET is_liquid = 0
         WHERE is_liquid = 1
           AND account_type IN ('social_insurance','fund','stock','crypto');",
    );

    // ── Phase 1: 月结快照优化 ──────────────────────────────────────────────
    // 添加冗余时间列 year_month
    let _ = conn.execute_batch(
        "ALTER TABLE balance_snapshots ADD COLUMN year_month INTEGER NOT NULL DEFAULT 0;",
    );
    // 添加 balance_delta（与上月差额）
    let _ = conn.execute_batch(
        "ALTER TABLE balance_snapshots ADD COLUMN balance_delta REAL;",
    );
    // 添加 prev_balance（上月余额）
    let _ = conn.execute_batch(
        "ALTER TABLE balance_snapshots ADD COLUMN prev_balance REAL;",
    );

    // 回填历史数据：year_month
    conn.execute_batch(
        "UPDATE balance_snapshots SET year_month = year * 100 + month WHERE year_month = 0;",
    )?;

    // 回填历史数据：prev_balance 和 balance_delta
    conn.execute_batch(
        "UPDATE balance_snapshots SET
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
                balance
            )
        WHERE prev_balance IS NULL;",
    )?;

    Ok(())
}

/// 首次运行时写入默认分类和账户
pub fn seed_default_data(conn: &Connection) -> Result<()> {
    let cat_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0))?;

    if cat_count == 0 {
        conn.execute_batch(
            "INSERT INTO categories (name, category_type, icon) VALUES
                ('餐饮',     'expense', '🍜'),
                ('交通',     'expense', '🚌'),
                ('购物',     'expense', '🛒'),
                ('娱乐',     'expense', '🎮'),
                ('医疗',     'expense', '💊'),
                ('住房',     'expense', '🏠'),
                ('通讯',     'expense', '📱'),
                ('教育',     'expense', '📚'),
                ('旅行',     'expense', '✈️'),
                ('其他支出', 'expense', '💸'),
                ('工资',     'income',  '💼'),
                ('奖金',     'income',  '🎁'),
                ('投资收益', 'income',  '📈'),
                ('其他收入', 'income',  '💰');",
        )?;
    }

    let acc_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get(0))?;

    if acc_count == 0 {
        let now = chrono::Local::now().to_rfc3339();
        for (name, atype) in &[
            ("现金", "cash"),
            ("银行卡", "bank"),
            ("微信", "wechat"),
            ("支付宝", "alipay"),
            ("社保", "social_insurance"),
        ] {
            conn.execute(
                "INSERT INTO accounts (name, account_type, currency, balance, created_at, updated_at)
                 VALUES (?1, ?2, 'CNY', 0.0, ?3, ?3)",
                rusqlite::params![name, atype, now],
            )?;
        }
    }

    Ok(())
}
