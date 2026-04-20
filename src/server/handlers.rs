use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::models::account::NewAccount;
use crate::models::category::NewCategory;
use crate::models::transaction::{NewTransaction, TransactionFilter, TransactionType};
use crate::service::{AppService, SnapshotGridRow};

type AppState = Arc<AppService>;

fn internal_error(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
}

// ── Accounts ──────────────────────────────────────────────────────────────────

pub async fn list_accounts(State(svc): State<AppState>) -> impl IntoResponse {
    match svc.list_accounts() {
        Ok(accounts) => Json(accounts).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn create_account(
    State(svc): State<AppState>,
    Json(req): Json<NewAccount>,
) -> impl IntoResponse {
    match svc.create_account(&req) {
        Ok(acc) => (StatusCode::CREATED, Json(acc)).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn delete_account(
    State(svc): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match svc.delete_account(id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

#[derive(Deserialize)]
pub struct UpdateLiquidReq {
    pub is_liquid: bool,
}

pub async fn update_account_liquid(
    State(svc): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<UpdateLiquidReq>,
) -> impl IntoResponse {
    match svc.update_account_liquid(id, req.is_liquid) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

// ── Categories ────────────────────────────────────────────────────────────────

pub async fn list_categories(State(svc): State<AppState>) -> impl IntoResponse {
    match svc.list_categories() {
        Ok(cats) => Json(cats).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn create_category(
    State(svc): State<AppState>,
    Json(req): Json<NewCategory>,
) -> impl IntoResponse {
    match svc.create_category(&req) {
        Ok(cat) => (StatusCode::CREATED, Json(cat)).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

// ── Transactions ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TxQuery {
    start_date: Option<String>,
    end_date: Option<String>,
    category_id: Option<i64>,
    account_id: Option<i64>,
    tx_type: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

pub async fn list_transactions(
    State(svc): State<AppState>,
    Query(q): Query<TxQuery>,
) -> impl IntoResponse {
    let filter = TransactionFilter {
        start_date: q.start_date,
        end_date: q.end_date,
        category_id: q.category_id,
        account_id: q.account_id,
        transaction_type: q
            .tx_type
            .as_deref()
            .and_then(|t| t.parse::<TransactionType>().ok()),
        limit: q.limit.or(Some(50)),
        offset: q.offset,
        is_large_only: false,
    };
    match svc.list_transactions(&filter) {
        Ok(txs) => Json(txs).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn create_transaction(
    State(svc): State<AppState>,
    Json(req): Json<NewTransaction>,
) -> impl IntoResponse {
    match svc.create_transaction(&req) {
        Ok(tx) => (StatusCode::CREATED, Json(tx)).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

pub async fn delete_transaction(
    State(svc): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match svc.delete_transaction(id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

// ── Stats ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct MonthlyQuery {
    year: Option<i32>,
    month: Option<u32>,
}

#[derive(Serialize)]
pub struct MonthlyStatsResponse {
    year: i32,
    month: u32,
    income: f64,
    expense: f64,
    net: f64,
    total_assets: f64,
}

pub async fn monthly_stats(
    State(svc): State<AppState>,
    Query(q): Query<MonthlyQuery>,
) -> impl IntoResponse {
    let now = chrono::Local::now();
    let year = q.year.unwrap_or_else(|| now.year());
    let month = q.month.unwrap_or_else(|| now.month());
    match svc.monthly_stats(year, month) {
        Ok((income, expense, net, total)) => Json(MonthlyStatsResponse {
            year,
            month,
            income,
            expense,
            net,
            total_assets: total,
        })
        .into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

// ── Snapshots ─────────────────────────────────────────────────────────────────

/// GET /api/v1/snapshots/grid?months=18
#[derive(Deserialize)]
pub struct GridQuery {
    months: Option<i64>,
}

pub async fn get_snapshot_grid(
    State(svc): State<AppState>,
    Query(q): Query<GridQuery>,
) -> impl IntoResponse {
    let months = q.months.unwrap_or(18);
    match svc.snapshot_grid(months) {
        Ok(rows) => Json(rows).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

/// GET /api/v1/snapshots/entry-items
#[derive(Serialize)]
pub struct EntryItem {
    pub account_id: i64,
    pub account_name: String,
    pub account_type: String,
    pub last_balance: Option<f64>,
}

pub async fn get_entry_items(State(svc): State<AppState>) -> impl IntoResponse {
    match svc.build_monthly_entry_items() {
        Ok(items) => {
            let resp: Vec<EntryItem> = items
                .into_iter()
                .map(|i| EntryItem {
                    account_id: i.account_id,
                    account_name: i.account_name,
                    account_type: i.account_type,
                    last_balance: i.last_balance,
                })
                .collect();
            Json(resp).into_response()
        }
        Err(e) => internal_error(e).into_response(),
    }
}

/// POST /api/v1/snapshots
#[derive(Deserialize)]
pub struct SaveSnapshotReq {
    pub year: i32,
    pub month: u32,
    pub balances: Vec<AccountBalance>,
    pub note: Option<String>,
}

#[derive(Deserialize)]
pub struct AccountBalance {
    pub account_id: i64,
    pub balance: f64,
}

pub async fn save_snapshot(
    State(svc): State<AppState>,
    Json(req): Json<SaveSnapshotReq>,
) -> impl IntoResponse {
    use crate::models::snapshot::MonthlyEntryItem;
    let items: Vec<MonthlyEntryItem> = req
        .balances
        .iter()
        .map(|b| MonthlyEntryItem {
            account_id: b.account_id,
            account_name: String::new(),
            account_type: String::new(),
            last_balance: None,
            input: String::new(),
            confirmed_balance: Some(b.balance),
        })
        .collect();
    match svc.save_monthly_snapshot(req.year, req.month, &items, req.note.as_deref()) {
        Ok(total) => Json(serde_json::json!({ "total": total })).into_response(),
        Err(e) => internal_error(e).into_response(),
    }
}

#[derive(Deserialize)]
pub struct DateRangeQuery {
    start_date: Option<String>,
    end_date: Option<String>,
}

#[derive(Serialize)]
pub struct CategoryStatItem {
    category: String,
    amount: f64,
}

pub async fn category_stats(
    State(svc): State<AppState>,
    Query(q): Query<DateRangeQuery>,
) -> impl IntoResponse {
    let now = chrono::Local::now();
    let start = q
        .start_date
        .unwrap_or_else(|| format!("{}-{:02}-01", now.year(), now.month()));
    let end = q.end_date.unwrap_or_else(|| now.format("%Y-%m-%d").to_string());
    match svc.category_stats(&start, &end) {
        Ok(stats) => {
            let items: Vec<_> = stats
                .into_iter()
                .map(|(category, amount)| CategoryStatItem { category, amount })
                .collect();
            Json(items).into_response()
        }
        Err(e) => internal_error(e).into_response(),
    }
}
