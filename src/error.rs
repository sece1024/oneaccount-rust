use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("连接池错误: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV 错误: {0}")]
    Csv(#[from] csv::Error),

    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("输入无效: {0}")]
    InvalidInput(String),
}

pub type Result<T> = std::result::Result<T, AppError>;
