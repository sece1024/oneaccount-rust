# OneAccount — Copilot Instructions

跨平台终端个人记账软件，使用 Rust 编写，支持 TUI 界面、HTTP REST API、CSV 导入/导出。数据存储在本地 SQLite 数据库中。

## Build & Test

```bash
cargo build --release          # 发布构建
cargo build                    # 开发构建
cargo test                     # 运行全部测试
cargo test <test_name>         # 运行单个测试，例如: cargo test test_atomic_create_transaction
cargo test -- --nocapture      # 显示测试输出

make run-tui                   # 启动 TUI 界面
make run-server                # 启动 HTTP 服务（端口 8080）
make build-all                 # 交叉编译所有目标平台（需要 cross + Docker）
```

## Architecture

五层结构，数据单向流动：`models` → `dao` → `service` → `tui` / `server` / `csv`

```
src/
├── models/      # 纯数据结构：Account, Transaction, Category, AccountSnapshot 等
├── dao/         # 数据访问层：直接操作 rusqlite Connection，每个实体一个文件
├── service/     # 业务逻辑层：通过 r2d2 Pool 获取连接，所有余额变动在这里原子提交
├── tui/
│   ├── app.rs   # App 状态结构体、MonthlyForm、Tab 枚举
│   ├── events.rs# 键盘事件处理（handle_key 及各 Tab 的子处理函数）
│   └── ui.rs    # 纯渲染（无副作用），所有 ratatui widget 在这里构建
├── server/      # Axum HTTP 服务：routes.rs 注册路由，handlers.rs 实现处理函数
├── csv/         # CSV 导入/导出，使用 FieldMapping 做表头适配
├── db/
│   ├── mod.rs       # create_pool：建立 r2d2 连接池，配置 WAL/外键/busy_timeout
│   └── migrations.rs# run_migrations（IF NOT EXISTS 幂等）+ apply_incremental_migrations（ALTER TABLE，忽略错误）
└── error.rs     # AppError（thiserror）+ Result<T> 类型别名
```

**关键数据流**：`AppService` 是 TUI 和 Server 的唯一数据入口。DAO 函数接受 `&Connection` 或 `&Transaction`（rusqlite），不持有连接。Service 层负责开启 `conn.transaction()` 并 `commit()`，DAO 层不感知事务边界。

**月结快照逻辑**：月结（`balance_snapshots` 表）保存每个账户每月的余额快照，`save_monthly_snapshot` 在同一个数据库事务中同时 upsert 快照和调用 `AccountDao::set_balance`，保证账户余额与快照一致。

## Key Conventions

### 错误处理
- 统一使用 `crate::error::Result<T>`，所有层都不 `.unwrap()`（测试除外）
- `AppError` 通过 `#[from]` 自动从 rusqlite/r2d2/io/csv/serde_json 错误转换

### 数据库
- 所有日期以 `TEXT` 类型存储，格式为 `chrono::Local::now().to_rfc3339()`（账目/账户）或 `"YYYY-MM-DD"`（交易日期）
- `AccountType` 在数据库中存为小写字符串（如 `"bank"`、`"credit_card"`），通过 `FromStr`/`Display` 双向转换
- 增量迁移在 `apply_incremental_migrations` 中用 `let _ = conn.execute_batch(...)` 忽略"列已存在"错误，不能使用 `?`
- 测试用 `create_pool(":memory:")` 创建内存数据库，每个测试函数调用 `test_service()` 获得独立实例

### TUI
- `App` 结构体（`tui/app.rs`）是所有 UI 状态的唯一来源，不直接持有 DB 连接，只持有 `AppService`
- 渲染函数（`tui/ui.rs`）是纯函数：接受 `&App` 和 `&mut Frame`，无副作用
- 余额显示统一用 `fmt_balance(b: f64)` 辅助函数（定义在 `ui.rs`），负数格式为 `-¥1234.56`，同理 `snapshot.rs` 中有同名私有函数
- 月结余额输入支持负号：`-` 只能作为第一个字符输入（信用卡/花呗等负债账户）

### 账户类型（AccountType）
在 `models/account.rs` 中以枚举维护，新增类型需同步修改：`Display`、`FromStr`、`display_name()`、`all()` 四个位置，以及 `db/migrations.rs` 的种子数据（如需要）。

### HTTP API
- 路由前缀统一为 `/api/v1`
- handler 通过 `axum::extract::State<Arc<AppService>>` 获取服务实例
- 响应直接返回 `Json(value)` 或 `StatusCode`

### 浮点数比较
测试中余额断言使用 `(a - b).abs() < 1e-9`，不用 `==`。
