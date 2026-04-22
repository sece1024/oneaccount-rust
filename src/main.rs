mod backup;
mod csv;
mod dao;
mod db;
mod error;
mod models;
mod server;
mod service;
mod tui;

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};

use error::Result;

#[derive(Parser)]
#[command(
    name = "oneaccount",
    about = "跨平台个人记账软件",
    version,
    long_about = "OneAccount — 支持 Windows / macOS / 树莓派的终端记账工具"
)]
struct Cli {
    /// SQLite 数据库路径（默认: ~/.local/share/oneaccount/oneaccount.db）
    #[arg(long, short = 'd', global = true)]
    db_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 启动 TUI 终端界面（默认）
    Tui,
    /// 启动 HTTP REST 服务
    Server {
        /// 监听地址
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// 监听端口
        #[arg(long, short, default_value_t = 8080)]
        port: u16,
    },
    /// 从 CSV 文件导入账单
    Import {
        /// CSV 文件路径
        file: PathBuf,
        /// 目标账户 ID
        #[arg(long)]
        account_id: i64,
        /// 字段映射（格式: 标准字段=CSV表头，可多次使用）
        #[arg(long = "map", value_parser = parse_mapping)]
        mappings: Vec<(String, String)>,
    },
    /// 将账单导出到 CSV 文件
    Export {
        /// 输出文件路径（默认: ./oneaccount_export.csv）
        #[arg(default_value = "oneaccount_export.csv")]
        output: PathBuf,
    },
    /// 备份数据库
    Backup,
    /// 从备份恢复数据库
    Restore {
        /// 备份文件路径
        file: PathBuf,
    },
    /// 列出所有备份
    Backups,
}

fn parse_mapping(s: &str) -> std::result::Result<(String, String), String> {
    let (k, v) = s.split_once('=').ok_or_else(|| format!("映射格式错误（应为 key=value）: {s}"))?;
    Ok((k.trim().to_string(), v.trim().to_string()))
}

fn default_db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("oneaccount")
        .join("oneaccount.db")
}

fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("oneaccount=info".parse().unwrap()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    let db_path = cli
        .db_path
        .unwrap_or_else(default_db_path)
        .to_string_lossy()
        .to_string();

    tracing::info!("数据库路径: {db_path}");

    let pool = Arc::new(db::create_pool(&db_path)?);

    match cli.command.unwrap_or(Commands::Tui) {
        Commands::Tui => {
            tui::run_tui(pool)?;
        }

        Commands::Server { host, port } => {
            // 启动前自动备份，保留最近 10 个
            if let Err(e) = backup::create_backup(&db_path) {
                tracing::warn!("自动备份失败: {e}");
            } else {
                let _ = backup::prune_backups(&db_path, 10);
            }
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(server::run_server(pool, &host, port, &db_path))?;
        }

        Commands::Import { file, account_id, mappings } => {
            let svc = service::AppService::new(pool);
            let mut mapping = csv::FieldMapping::with_default();
            for (k, v) in mappings {
                mapping.map.insert(k, v);
            }
            let f = std::fs::File::open(&file)?;
            let count = csv::import_csv(f, &svc, &mapping, account_id)?;
            println!("✅ 成功导入 {count} 条账单（来源: {}）", file.display());
        }

        Commands::Export { output } => {
            let svc = service::AppService::new(pool);
            let f = std::fs::File::create(&output)?;
            let count = csv::export_csv(f, &svc)?;
            println!("✅ 已导出 {count} 条账单 → {}", output.display());
        }

        Commands::Backup => {
            let path = backup::create_backup(&db_path)?;
            backup::prune_backups(&db_path, 10)?;
            println!("✅ 备份完成: {}", path.display());
        }

        Commands::Restore { file } => {
            let safety = backup::restore_backup(&db_path, &file)?;
            println!("✅ 已恢复数据库（恢复前备份: {}）", safety.display());
            println!("⚠️  请重启应用以加载恢复后的数据");
        }

        Commands::Backups => {
            let files = backup::list_backups(&db_path)?;
            if files.is_empty() {
                println!("暂无备份");
            } else {
                println!("备份列表（{}）:", backup::backup_dir(&db_path).display());
                for f in &files {
                    let meta = std::fs::metadata(f)?;
                    let size = meta.len();
                    let name = f.file_name().unwrap_or_default().to_string_lossy();
                    println!("  {name}  ({:.1} KB)", size as f64 / 1024.0);
                }
                println!("\n共 {} 个备份", files.len());
            }
        }
    }

    Ok(())
}

