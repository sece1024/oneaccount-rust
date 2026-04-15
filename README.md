# OneAccount

跨平台个人记账软件，支持 Windows / macOS / Linux / 树莓派。数据存储在本地 SQLite 数据库，无需联网、无需注册账号。

## 功能

- **TUI 终端界面** — 资产总览、月结记录、大额支出管理、账户管理
- **HTTP REST API** — 可对接第三方客户端或脚本自动化
- **CSV 导入/导出** — 兼容常见银行账单格式
- **月结快照** — 每月记录各账户余额，自动计算资产趋势
- **负债账户** — 支持信用卡、花呗、美团月付等负余额账户

## 安装

### 从源码构建（推荐）

需要 Rust 工具链（[安装 Rust](https://rustup.rs)）：

```bash
git clone https://github.com/yourname/oneaccount.git
cd oneaccount
make build          # 输出: target/release/oneaccount
# 或安装到 ~/.cargo/bin/
make install
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

### TUI 终端界面（默认）

```bash
./oneaccount
# 或显式指定
./oneaccount tui
```

| 快捷键 | 功能 |
|--------|------|
| `Tab` / `1`~`4` | 切换页面 |
| `↑` `↓` | 选择列表项 |
| `a` | 新建账户 / 新增大额支出 |
| `d` | 删除选中项 |
| `Enter` | 编辑当前行余额（月结页） |
| `-` | 输入负数余额（负债账户） |
| `Ctrl+S` | 保存月结快照 |
| `Ctrl+E` | 导出 CSV |
| `Ctrl+I` | 导入 CSV |
| `q` | 退出 |

### HTTP 服务

```bash
./oneaccount server --port 8080
```

API 前缀：`/api/v1`

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/accounts` | 账户列表 |
| `POST` | `/accounts` | 创建账户 |
| `DELETE` | `/accounts/:id` | 删除账户 |
| `GET` | `/categories` | 分类列表 |
| `POST` | `/categories` | 创建分类 |
| `GET` | `/transactions` | 账目列表（支持过滤） |
| `POST` | `/transactions` | 创建账目 |
| `DELETE` | `/transactions/:id` | 删除账目 |
| `GET` | `/stats/monthly` | 月度收支统计 |
| `GET` | `/stats/by-category` | 按分类统计 |

**账目查询参数**：`start_date`、`end_date`、`account_id`、`category_id`、`tx_type`（income/expense/transfer）、`limit`、`offset`

### CSV 导入/导出

```bash
# 导出所有账单
./oneaccount export output.csv

# 导入（使用默认字段映射：金额/日期/备注/类型/分类）
./oneaccount import bill.csv --account-id 1

# 自定义字段映射（CSV表头与默认不同时）
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

| 类型 | 说明 |
|------|------|
| 现金 | 实体现金 |
| 银行卡 | 储蓄卡 |
| 微信 | 微信钱包 |
| 支付宝 | 支付宝余额 |
| 股票 | 股票账户 |
| 虚拟货币 | 加密资产 |
| 社保 | 社保/公积金 |
| 基金 | 基金账户 |
| 信用/负债 | 信用卡、花呗、美团月付等（支持负余额） |

## 开发

```bash
cargo test                          # 运行全部测试
cargo test test_atomic_create       # 运行单个测试
cargo test -- --nocapture           # 显示测试输出
RUST_LOG=debug ./oneaccount tui     # 开启调试日志
```

## License

MIT
