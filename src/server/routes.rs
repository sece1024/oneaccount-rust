use std::sync::Arc;
use std::path::PathBuf;

use axum::Router;
use axum::http::Method;
use axum::response::IntoResponse;
use tower_http::cors::{CorsLayer, Any};

use crate::db::DbPool;
use crate::service::AppService;

use super::handlers;

async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let raw_path = uri.path().trim_start_matches('/');
    // Block path traversal attempts and normalize SPA root.
    let requested = if raw_path.is_empty() || raw_path.contains("..") || raw_path.contains('\\') {
        "index.html"
    } else {
        raw_path
    };

    let dist_dir = PathBuf::from("frontend").join("dist");
    let requested_file = dist_dir.join(requested);

    if let Ok(content) = std::fs::read(&requested_file) {
        let mime = mime_guess::from_path(&requested_file)
            .first_or_octet_stream()
            .to_string();
        return (
            [(axum::http::header::CONTENT_TYPE, mime)],
            content,
        )
            .into_response();
    }

    let index_file = dist_dir.join("index.html");
    match std::fs::read(&index_file) {
        Ok(content) => {
            let mime = mime_guess::from_path("index.html")
                .first_or_octet_stream()
                .to_string();
            (
                [(axum::http::header::CONTENT_TYPE, mime)],
                content,
            )
                .into_response()
        }
        Err(_) => (
            axum::http::StatusCode::NOT_FOUND,
            "frontend assets not found; build frontend first (cd frontend && npm run build)",
        )
            .into_response(),
    }
}

pub fn build_router(pool: Arc<DbPool>, db_path: String) -> Router {
    let service = Arc::new(AppService::new(pool));

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);

    Router::new()
        .nest("/api/v1", api_routes(service, db_path))
        .fallback(static_handler)
        .layer(cors)
}

fn api_routes(service: Arc<AppService>, db_path: String) -> Router {
    use axum::routing::{delete, get, post, put};

    let service_routes = Router::new()
        // Accounts
        .route("/accounts", get(handlers::list_accounts).post(handlers::create_account))
        .route("/accounts/:id", delete(handlers::delete_account))
        .route("/accounts/:id/liquid", post(handlers::update_account_liquid))
        // Categories
        .route("/categories", get(handlers::list_categories).post(handlers::create_category))
        // Transactions
        .route(
            "/transactions",
            get(handlers::list_transactions).post(handlers::create_transaction),
        )
        .route("/transactions/:id", put(handlers::update_transaction).delete(handlers::delete_transaction))
        // Stats
        .route("/stats/monthly", get(handlers::monthly_stats))
        .route("/stats/by-category", get(handlers::category_stats))
        // Analytics
        .route("/analytics/summary", get(handlers::analytics_summary))
        .route("/analytics/assets", get(handlers::analytics_assets))
        .route("/analytics/health", get(handlers::analytics_health))
        .route("/analytics/trend", get(handlers::analytics_trend))
        // Smart defaults
        .route("/smart/defaults", get(handlers::smart_defaults))
        // Snapshots
        .route("/snapshots/grid", get(handlers::get_snapshot_grid))
        .route("/snapshots/entry-items", get(handlers::get_entry_items))
        .route("/snapshots", post(handlers::save_snapshot))
        .with_state(service);

    let backup_routes = Router::new()
        .route("/backup", get(handlers::create_backup))
        .route("/backup/list", get(handlers::list_backups))
        .with_state(db_path);

    service_routes.merge(backup_routes)
}
