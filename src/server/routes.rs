use std::sync::Arc;

use axum::Router;
use tower_http::cors::CorsLayer;

use crate::db::DbPool;
use crate::service::AppService;

use super::handlers;

pub fn build_router(pool: Arc<DbPool>) -> Router {
    let service = Arc::new(AppService::new(pool));

    Router::new()
        .nest("/api/v1", api_routes(service))
        .layer(CorsLayer::permissive())
}

fn api_routes(service: Arc<AppService>) -> Router {
    use axum::routing::{delete, get, post};

    Router::new()
        // Accounts
        .route("/accounts", get(handlers::list_accounts).post(handlers::create_account))
        .route("/accounts/:id", delete(handlers::delete_account))
        // Categories
        .route("/categories", get(handlers::list_categories).post(handlers::create_category))
        // Transactions
        .route(
            "/transactions",
            get(handlers::list_transactions).post(handlers::create_transaction),
        )
        .route("/transactions/:id", delete(handlers::delete_transaction))
        // Stats
        .route("/stats/monthly", get(handlers::monthly_stats))
        .route("/stats/by-category", get(handlers::category_stats))
        // Snapshots
        .route("/snapshots/grid", get(handlers::get_snapshot_grid))
        .route("/snapshots/entry-items", get(handlers::get_entry_items))
        .route("/snapshots", post(handlers::save_snapshot))
        .with_state(service)
}
