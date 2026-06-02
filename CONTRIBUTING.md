# 贡献指南

感谢你有意为 OneAccount 贡献代码！本文档说明开发环境搭建、代码规范和提交流程。

## 开发环境

### 依赖

| 工具 | 版本要求 | 用途 |
|------|----------|------|
| Rust | stable (edition 2024) | 主程序 |
| Node.js | 18+ | 前端构建 |
| pnpm | 8+ | 前端包管理 |
| SQLite | 3.x | 运行时依赖（由 rusqlite 静态链接） |

安装 Rust：https://rustup.rs  
安装 pnpm：`npm install -g pnpm`

### 克隆与构建

```bash
git clone https://github.com/sece1024/oneaccount-rust.git
cd oneaccount-rust

# 后端开发构建
cargo build

# 启动 TUI（仅后端）
make run-tui

# 启动 HTTP 服务
make run-server

# 同时启动后端 + 前端 dev server（带热更新）
make dev
```

## 项目结构

```
src/
├── models/      # 纯数据结构（Account, Transaction, Category 等）
├── dao/         # 数据访问层（rusqlite，每个实体一个文件）
├── service/     # 业务逻辑层（r2d2 连接池，原子操作）
├── tui/
│   ├── app.rs   # App 状态结构体、MonthlyForm、Tab 枚举
│   ├── events.rs# 键盘事件处理
│   └── ui.rs    # 纯渲染（无副作用）
├── server/      # Axum HTTP 服务
│   ├── routes.rs# 路由注册
│   └── handlers.rs # 请求处理函数
├── csv/         # CSV 导入/导出
├── db/
│   ├── mod.rs       # 连接池（WAL / 外键 / busy_timeout）
│   └── migrations.rs# 建表迁移 + 增量迁移
└── error.rs     # AppError + Result<T> 类型别名

frontend/        # Svelte 5 + Vite 前端（嵌入二进制）
```

**数据流**：`models` → `dao` → `service` → `tui` / `server` / `csv`

`AppService` 是 TUI 和 Server 的唯一数据入口，DAO 层不感知事务边界。

## 开发规范

### 错误处理

- 统一使用 `crate::error::Result<T>`，禁止在非测试代码中使用 `.unwrap()`
- 新增错误类型在 `src/error.rs` 的 `AppError` 枚举中添加，通过 `#[from]` 自动转换

### 数据库

- 所有建表语句放在 `db/migrations.rs` 的 `SCHEMA` 常量中，使用 `IF NOT EXISTS`
- 对已有数据库的列/表变更放在 `apply_incremental_migrations` 中，使用 `let _ = conn.execute_batch(...)` 忽略"列已存在"错误
- 日期统一用 `TEXT` 存储：账目/账户用 RFC3339，快照日期用 `YYYY-MM-DD`
- `AccountType` 在数据库中存为小写字符串，通过 `Display`/`FromStr` 双向转换

### 新增账户类型

需同步修改 `src/models/account.rs` 中的四处：

1. `AccountType` 枚举变体
2. `impl Display` — 数据库存储格式（小写下划线）
3. `impl FromStr` — 解析（含中文别名）
4. `display_name()` — TUI 显示名称
5. `all()` — 枚举全量列表（用于 UI 选择）

### TUI

- `App` 结构体（`tui/app.rs`）是所有 UI 状态的唯一来源
- 渲染函数（`tui/ui.rs`）是纯函数：只接受 `&App` 和 `&mut Frame`，不产生副作用
- 余额显示使用 `fmt_balance(b: f64)` 辅助函数，负数格式为 `-¥1234.56`

### HTTP API

- 路由前缀统一为 `/api/v1`
- Handler 通过 `State<Arc<AppService>>` 获取服务实例
- 新接口先在 `service/mod.rs` 实现方法，再在 `handlers.rs` 添加 handler，最后在 `routes.rs` 注册路由

### 浮点数比较

测试中余额断言使用 `(a - b).abs() < 1e-9`，不使用 `==`。

## 测试

```bash
cargo test                    # 运行全部测试
cargo test <test_name>        # 运行单个测试
cargo test -- --nocapture     # 显示 println! 输出
```

测试惯例：

- 每个测试调用 `test_service()` 获得独立的内存数据库实例（互不干扰）
- 业务逻辑测试放在 `src/service/mod.rs` 的 `#[cfg(test)]` 模块中
- DAO 测试放在对应的 `src/dao/*.rs` 文件中

新功能应附带测试，覆盖正常路径和边界条件。

## 提交规范

使用语义化提交信息：

```
<type>: <简短描述>

[可选正文]

[可选脚注，如 Co-authored-by]
```

常用 type：

| type | 用途 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `refactor` | 重构（不改变行为） |
| `test` | 增加或修改测试 |
| `docs` | 文档变更 |
| `chore` | 构建/工具链/配置 |

示例：

```
feat: 新增电子钱包账户类型

支持 EWallet 类型（e_wallet），用于小米钱包等电子账户。
默认为流动资产。
```

## Pull Request

1. 从 `main` 分支创建功能分支：`git checkout -b feat/your-feature`
2. 确保 `cargo test` 全部通过
3. 确保 `cargo build` 无 error（warning 可接受但应尽量消除）
4. 提交 PR 并描述变更内容和测试方法
