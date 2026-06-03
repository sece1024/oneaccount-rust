# AGENTS.md

This guide contains essential project-specific rules, quirks, and commands to help agents avoid mistakes and develop features safely.

## Build & Test Workflow

### Essential Commands
```bash
# Build the backend release binary
cargo build --release

# Run all backend unit and integration tests
cargo test

# Build frontend and embed it into the backend release binary
make build-release

# Run backend server + frontend dev server concurrently (hot-reloading)
make dev
```

### Gotchas & Quirks
- **Unified Build Requirement:** Always build the frontend assets (`make build-frontend` or `cd frontend && pnpm run build`) before compiling a backend release that embeds frontend pages via `rust-embed`.
- **Pre-existing Clippy Warnings:** Running `cargo clippy --all-targets -- -D warnings` will fail on pre-existing code due to unused fields (`new_balance`), collapsible ifs, and redundant closures. Ensure new code does not introduce new warnings, but do not enforce a strict `-D warnings` on pre-existing files.
- **Pre-existing Svelte Check Errors:** Running `cd frontend && pnpm run check` fails with TS errors in `src/pages/Transactions.svelte` due to an incompatible event handler parameter in `load(reset?: boolean)`. However, `pnpm run build` compiles successfully. Do not block PRs or releases on these pre-existing frontend TS check failures.

## Architecture & Data Flow

Data flows strictly in one direction: `models` → `dao` → `service` → `tui` / `server` / `csv`.

- **Entrypoint Constraint:** `AppService` is the single source of truth and data entry point for TUI, Server handlers, and CSV import/export.
- **Connection Management:** DAO modules (`src/dao/*`) do not hold database connections. They must accept `&Connection` or `&Transaction` passed from `AppService`.
- **Transaction Boundaries:** `AppService` is solely responsible for initiating database transactions (`conn.transaction()`) and committing them. Do not manage transactions inside DAO.

## Key Conventions

### 1. Database & Migrations
- **Database File Location:** Automatically located by platform (`dirs::data_dir()`), but can be overridden by passing `-d <path>` as a CLI argument.
- **Date Storage Format:**
  - Standard dates (accounts, transactions): RFC3339 string (`chrono::Local::now().to_rfc3339()`).
  - Snapshot dates: `"YYYY-MM-DD"` format.
- **Idempotent Migrations:** In `src/db/migrations.rs`, incremental migrations inside `apply_incremental_migrations` must use `let _ = conn.execute_batch(...)` and ignore potential "column already exists" errors. Do NOT use `?` on these queries.

### 2. TUI Rendering (Pure & State-driven)
- **UI State Source:** `App` (`src/tui/app.rs`) holds all UI states and `AppService`. It has no direct DB connection.
- **Pure Render Functions:** All render functions inside `src/tui/ui.rs` must be completely pure, taking `&App` and `&mut Frame` with absolutely no state mutations or side effects.
- **Currency and Balance Formatting:** Negative balances must be formatted using the `fmt_balance(b: f64)` helper as `-¥1234.56` (never `¥-1234.56`).
- **Negative Input in TUI:** Monthly balance input allows `-` only as the first character (for credit or liability accounts).

### 3. Adding a New Account Type
When adding a new type to the `AccountType` enum in `src/models/account.rs`, you must update precisely 5 places:
1. `AccountType` enum variants.
2. `impl Display` (snake_case representations for DB storage).
3. `impl FromStr` (parsing strings, including Chinese aliases).
4. `display_name(&self)` (UI-facing Chinese names).
5. `all()` (list of all variants used for selection lists).
*Also update `seed_default_data` or migrations in `src/db/migrations.rs` if seed data is needed.*

### 4. Float Comparisons in Tests
Never assert float balances with `==`. Always use `(a - b).abs() < 1e-9`.
