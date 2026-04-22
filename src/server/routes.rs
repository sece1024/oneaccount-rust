use std::sync::Arc;

use axum::Router;
use axum::http::Method;
use axum::response::IntoResponse;
use tower_http::cors::{CorsLayer, Any};
use rust_embed::Embed;

use crate::db::DbPool;
use crate::service::AppService;

use super::handlers;

#[derive(Embed)]
#[folder = "frontend/dist/"]
struct FrontendAssets;

async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    // Try the exact path first, then fall back to index.html (SPA)
    let path = if path.is_empty() { "index.html" } else { path };

    match FrontendAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();
            (
                [(axum::http::header::CONTENT_TYPE, mime)],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => {
            // SPA fallback: serve index.html for any unknown path
            match FrontendAssets::get("index.html") {
                Some(content) => (
                    [(axum::http::header::CONTENT_TYPE, "text/html".to_string())],
                    content.data.into_owned(),
                )
                    .into_response(),
                None => (axum::http::StatusCode::NOT_FOUND, "404").into_response(),
            }
        }
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
