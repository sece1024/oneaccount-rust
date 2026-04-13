pub mod migrations;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub type DbPool = Pool<SqliteConnectionManager>;

/// 创建连接池，配置 WAL 模式和外键支持
pub fn create_pool(db_path: &str) -> crate::error::Result<DbPool> {
    // 确保父目录存在
    if let Some(parent) = std::path::Path::new(db_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;
             PRAGMA cache_size = -8000;",
        )
    });

    let pool = Pool::builder().max_size(16).build(manager)?;

    // 运行迁移（建表 + 种子数据）
    {
        let conn = pool.get()?;
        migrations::run_migrations(&conn)?;
        migrations::seed_default_data(&conn)?;
    }

    Ok(pool)
}
