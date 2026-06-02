# AGENTS.md

## Build & Test

```bash
cargo build --release          # release build
cargo test                     # run all tests
cargo test <test_name>         # run single test, e.g.: cargo test test_atomic_create_transaction
cargo test -- --nocapture      # show test output

make run-tui                   # start TUI interface
make run-server                # start HTTP server (port 8080)
make dev                       # start backend + frontend dev servers (Ctrl-C to stop both)
make build-frontend            # build frontend assets (required before `make build-release`)
```

## Architecture

Five-layer structure, data flows one way: `models` → `dao` → `service` → `tui` / `server` / `csv`

```
src/
├── models/      # Pure data structs: Account, Transaction, Category, AccountSnapshot
├── dao/         # Data access: direct rusqlite Connection operations, one file per entity
├── service/     # Business logic: r2d2 Pool, all balance changes commit atomically here
├── tui/
│   ├── app.rs   # App state struct, MonthlyForm, Tab enum
│   ├── events.rs# Keyboard event handling (handle_key + sub-handlers per Tab)
│   └── ui.rs    # Pure rendering (no side effects), all ratatui widgets built here
├── server/      # Axum HTTP server: routes.rs registers routes, handlers.rs implements them
├── csv/         # CSV import/export, FieldMapping adapts headers
├── db/
│   ├── mod.rs       # create_pool: r2d2 connection pool with WAL/foreign_keys/busy_timeout
│   └── migrations.rs# run_migrations (IF NOT EXISTS idempotent) + apply_incremental_migrations
└── error.rs     # AppError (thiserror) + Result<T> type alias
```

**Key data flow**: `AppService` is the sole entry point for TUI and Server. DAO functions take `&Connection` or `&Transaction` (rusqlite), do not hold connections. Service layer manages `conn.transaction()` and `commit()`.

**Monthly snapshot logic**: `balance_snapshots` table stores per-account monthly balances. `save_monthly_snapshot` upserts snapshot and calls `AccountDao::set_balance` in the same database transaction, ensuring consistency.

## Key Conventions

### Error handling
- Use unified `crate::error::Result<T>` everywhere
- `AppError` auto-converts from rusqlite/r2d2/io/csv/serde_json via `#[from]`
- No `.unwrap()` except in tests

### Database
- Dates stored as TEXT: `chrono::Local::now().to_rfc3339()` (accounts/transactions) or `"YYYY-MM-DD"` (transaction date)
- `AccountType` stored as lowercase strings (`"bank"`, `"credit_card"`) via `FromStr`/`Display`
- Incremental migrations in `apply_incremental_migrations` use `let _ = conn.execute_batch(...)` to ignore "column already exists" errors — cannot use `?`
- Tests use `create_pool(":memory:")` for in-memory database, each test calls `test_service()` for an independent instance

### TUI
- `App` struct (`tui/app.rs`) is the single source of UI state, holds `AppService` only (no direct DB connection)
- Render functions (`tui/ui.rs`) are pure: take `&App` and `&mut Frame`, no side effects
- Balance display uses `fmt_balance(b: f64)` helper (defined in `ui.rs`), negative format: `-¥1234.56`
- Monthly balance input supports negative sign: `-` only as first character (for credit/liability accounts)

### AccountType
Enum in `models/account.rs`. Adding a new type requires changes in 4 places: `Display`, `FromStr`, `display_name()`, `all()`. Also check `db/migrations.rs` seed data if needed.

### HTTP API
- Route prefix: `/api/v1`
- Handler extracts `State<Arc<AppService>>` via axum extract
- Response: return `Json(value)` or `StatusCode`

### Float comparison in tests
Use `(a - b).abs() < 1e-9`, not `==`.

## Frontend

Svelte 5 + Vite, embedded into the Rust binary via `rust-embed`. Dev server proxies `/api` to `http://127.0.0.1:8080`.

```bash
cd frontend && pnpm run dev    # start frontend dev server only
cd frontend && pnpm run build  # build frontend assets
```

## Reference

For detailed architecture and conventions, see `.github/copilot-instructions.md`.
