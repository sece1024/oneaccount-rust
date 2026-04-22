use std::path::{Path, PathBuf};

use crate::error::Result;

/// 获取备份目录（数据库同级 backups/ 子目录）
pub fn backup_dir(db_path: &str) -> PathBuf {
    Path::new(db_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups")
}

/// 创建数据库备份，返回备份文件路径
pub fn create_backup(db_path: &str) -> Result<PathBuf> {
    let dir = backup_dir(db_path);
    std::fs::create_dir_all(&dir)?;

    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup_name = format!("oneaccount_{ts}.db");
    let backup_path = dir.join(&backup_name);

    std::fs::copy(db_path, &backup_path)?;
    tracing::info!("备份已创建: {}", backup_path.display());

    Ok(backup_path)
}

/// 列出所有备份文件（按时间倒序）
pub fn list_backups(db_path: &str) -> Result<Vec<PathBuf>> {
    let dir = backup_dir(db_path);
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().map_or(false, |ext| ext == "db")
                && p.file_name()
                    .map_or(false, |n| n.to_string_lossy().starts_with("oneaccount_"))
        })
        .collect();

    files.sort_by(|a, b| b.cmp(a)); // newest first
    Ok(files)
}

/// 从备份文件恢复数据库（先备份当前数据库）
pub fn restore_backup(db_path: &str, backup_file: &Path) -> Result<PathBuf> {
    if !backup_file.exists() {
        return Err(crate::error::AppError::NotFound(format!(
            "备份文件不存在: {}",
            backup_file.display()
        )));
    }

    // 先备份当前数据库
    let safety_backup = create_backup(db_path)?;
    tracing::info!(
        "恢复前已备份当前数据库: {}",
        safety_backup.display()
    );

    // 恢复
    std::fs::copy(backup_file, db_path)?;
    tracing::info!("已从备份恢复: {}", backup_file.display());

    Ok(safety_backup)
}

/// 清理旧备份，仅保留最近 keep 个
pub fn prune_backups(db_path: &str, keep: usize) -> Result<usize> {
    let files = list_backups(db_path)?;
    if files.len() <= keep {
        return Ok(0);
    }

    let mut removed = 0;
    for old in &files[keep..] {
        if std::fs::remove_file(old).is_ok() {
            removed += 1;
        }
    }
    if removed > 0 {
        tracing::info!("已清理 {removed} 个旧备份（保留最近 {keep} 个）");
    }
    Ok(removed)
}
