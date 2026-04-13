mod handlers;
mod routes;

use std::sync::Arc;

use axum::Router;
use tokio::net::TcpListener;

use crate::db::DbPool;
use crate::error::Result;

pub async fn run_server(pool: Arc<DbPool>, host: &str, port: u16) -> Result<()> {
    let app: Router = routes::build_router(pool);
    let addr = format!("{host}:{port}");
    tracing::info!("HTTP 服务启动: http://{addr}");
    let listener = TcpListener::bind(&addr).await.map_err(crate::error::AppError::Io)?;
    axum::serve(listener, app).await.map_err(crate::error::AppError::Io)?;
    Ok(())
}
