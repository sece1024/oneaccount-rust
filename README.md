# OneAccount

跨平台个人记账软件，支持 Windows / macOS / Linux / 树莓派。数据存储在本地 SQLite 数据库，无需联网、无需注册账号。

## 功能

- **TUI 终端界面** — 资产总览、月结记录、大额支出管理、账户管理、财务分析
- **HTTP REST API** — 可对接前端或脚本自动化，内嵌 Web 界面（Svelte 5）
- **分析引擎** — 月度同比/环比增长率、资产结构、财务健康评分
- **CSV 导入/导出** — 兼容常见银行账单格式
- **月结快照** — 每月记录各账户余额，支持智能预填/快速跳过/撤销
- **负债账户** — 支持信用卡、花呗、美团月付等负余额账户
- **数据安全** — 本地 SQLite 存储，支持备份/恢复

## 安装

### 从源码构建

需要 Rust 工具链（[安装 Rust](https://rustup.rs)）：

```bash
git clone https://github.com/sece1024/oneaccount-rust.git
cd oneaccount-rust
make build          # 输出: target/release/oneaccount
```

若需内嵌 Web 界面（需 Node.js 18+ 及 pnpm）：

```bash
make build-release  # 构建前端 + 后端一体化可执行文件
```

### 交叉编译（其他平台）

需要先安装 [cross](https://github.com/cross-rs/cross) 和 Docker：

```bash
make install-cross      # 安装 cross 工具
make add-targets        # 添加 Rust 目标平台

make build-rpi-arm64    # 树莓派 3B+/4/5（64-bit）
make build-rpi-armv7    # 树莓派 Zero/1/2（32-bit）
make build-linux-x86    # Linux x86_64
make build-windows      # Windows x64
make build-all          # 所有平台，输出到 dist/
```

## 使用

### TUI 终端界面

```bash
./oneaccount         # 启动 TUI（默认）
./oneaccount tui     # 等效写法
```

**页面导航**

| 快捷键 | 功能 |
|--------|------|
| `Tab` / `1`~`5` | 切换页面（总览 / 月结 / 大额支出 / 账户 / 分析） |
| `↑` `↓` | 选择列表项 |
| `q` | 退出 |

**月结页快捷键**

| 快捷键 | 功能 |
|--------|------|
| `Enter` | 编辑当前行余额 |
| `Space` | 跳过当前账户（保持上月余额） |
| `u` | 撤销上一步操作 |
| `-` | 输入负数余额（负债账户） |
| `Ctrl+S` | 保存月结快照 |
| `Ctrl+D` | 保存草稿（切换页面不丢失） |
| `←` `→` | 切换月份 |

**其他快捷键**

| 快捷键 | 功能 |
|--------|------|
| `a` | 新建账户 / 新增大额支出 |
| `d` | 删除选中项 |
| `Ctrl+E` | 导出 CSV |
| `Ctrl+I` | 导入 CSV |

### HTTP 服务

```bash
./oneaccount server --port 8080
```

浏览器访问 `http://localhost:8080` 可使用内嵌 Web 界面（需先执行 `make build-release`）。

API 前缀：`/api/v1`

**账户 & 分类**

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/accounts` | 账户列表 |
| `POST` | `/accounts` | 创建账户 |
| `DELETE` | `/accounts/:id` | 删除账户 |
| `POST` | `/accounts/:id/liquid` | 更新流动性标记 |
| `GET` | `/categories` | 分类列表 |
| `POST` | `/categories` | 创建分类 |

**账目**

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/transactions` | 账目列表 |
| `POST` | `/transactions` | 创建账目 |
| `PUT` | `/transactions/:id` | 更新账目 |
| `DELETE` | `/transactions/:id` | 删除账目 |

账目查询参数：`start_date`、`end_date`、`account_id`、`category_id`、`tx_type`（income/expense/transfer）、`limit`、`offset`

**统计 & 快照**

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/stats/monthly` | 月度收支统计 |
| `GET` | `/stats/by-category` | 按分类统计 |
| `GET` | `/snapshots/grid` | 快照网格（近 N 月） |
| `GET` | `/snapshots/entry-items` | 月结预填项 |
| `POST` | `/snapshots` | 保存月结快照 |

**分析接口**

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/analytics/summary` | 月度对比（环比 + 同比） |
| `GET` | `/analytics/assets` | 资产结构（按类型/流动性/币种） |
| `GET` | `/analytics/health` | 财务健康指标（负债率/评分） |
| `GET` | `/analytics/trend` | 资产趋势（近 N 月） |
| `GET` | `/smart/defaults` | 月结智能预填默认值 |

分析接口通用查询参数：`year`、`month`（默认当前月）；`/analytics/trend` 使用 `months`（默认 12）。

**备份**

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/backup` | 创建备份 |
| `GET` | `/backup/list` | 备份列表 |

### CSV 导入/导出

```bash
# 导出所有账单
./oneaccount export output.csv

# 导入（使用默认字段映射）
./oneaccount import bill.csv --account-id 1

# 自定义字段映射（CSV 表头与默认不同时）
./oneaccount import bill.csv --account-id 1 \
  --map amount=交易金额 \
  --map date=交易时间 \
  --map note=商品说明
```

默认 CSV 字段映射：

| 标准字段 | 默认对应表头 |
|----------|-------------|
| amount | 金额 |
| date | 日期 |
| note | 备注 |
| type | 类型 |
| category | 分类 |

### 自定义数据库路径

```bash
./oneaccount -d /path/to/mydata.db tui
```

默认路径（按平台）：

| 平台 | 路径 |
|------|------|
| macOS | `~/Library/Application Support/oneaccount/oneaccount.db` |
| Linux / 树莓派 | `~/.local/share/oneaccount/oneaccount.db` |
| Windows | `%APPDATA%\oneaccount\oneaccount.db` |

## 账户类型

| 内部标识 | 显示名称 | 说明 |
|----------|----------|------|
| `cash` | 现金 | 实体现金 |
| `bank` | 银行卡 | 储蓄卡 |
| `wechat` | 微信 | 微信钱包 |
| `alipay` | 支付宝 | 支付宝余额 |
| `stock` | 股票 | 股票账户（默认非流动） |
| `crypto` | 虚拟货币 | 加密资产（默认非流动） |
| `social_insurance` | 社保 | 社保/公积金（默认非流动） |
| `fund` | 基金 | 基金账户（默认非流动） |
| `credit_card` | 信用/负债 | 信用卡、花呗等（支持负余额） |
| `e_wallet` | 电子钱包 | 小米钱包等 |
| `other` | 其他 | 其他资产 |

流动性（`is_liquid`）决定该账户是否计入「流动资产」分析维度，可通过 API 或 TUI 修改。

## 开发

```bash
cargo build                         # 开发构建
cargo test                          # 运行全部测试
cargo test test_atomic_create       # 运行单个测试
cargo test -- --nocapture           # 显示测试输出

make run-tui                        # 启动 TUI
make run-server                     # 启动 HTTP 服务（端口 8080）
make dev                            # 同时启动后端 + 前端 dev server

RUST_LOG=debug ./oneaccount tui     # 开启调试日志
```

详见 [CONTRIBUTING.md](CONTRIBUTING.md)。

## License

MIT
