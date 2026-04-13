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
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    year       INTEGER NOT NULL,
    month      INTEGER NOT NULL,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    balance    REAL    NOT NULL,
    note       TEXT,
    created_at TEXT    NOT NULL,
    UNIQUE(year, month, account_id)
);

CREATE INDEX IF NOT EXISTS idx_tx_date         ON transactions(date);
CREATE INDEX IF NOT EXISTS idx_tx_account_id   ON transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_tx_category_id  ON transactions(category_id);
CREATE INDEX IF NOT EXISTS idx_snap_ym         ON balance_snapshots(year, month);
CREATE INDEX IF NOT EXISTS idx_snap_account    ON balance_snapshots(account_id);
";

/// 对可能已存在的旧 DB 进行安全的增量迁移（忽略"列已存在"错误）
fn apply_incremental_migrations(conn: &Connection) -> Result<()> {
    // 为旧版 transactions 表添加 is_large 列（如已存在则忽略）
    let _ = conn.execute_batch(
        "ALTER TABLE transactions ADD COLUMN is_large INTEGER NOT NULL DEFAULT 0;",
    );
    // is_large 列存在后才能建索引（同样忽略已存在错误）
    let _ = conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_tx_is_large ON transactions(is_large);",
    );
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
