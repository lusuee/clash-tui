//! 本地配置备份与恢复（Settings → `B`）。
//!
//! 只备份“由用户配置出来、订阅更新会覆盖”的小文件：`data/config.yaml`、
//! `data/dns.json`、`data/overrides.json`、`rules.json`。`profiles/` 与
//! `subscriptions.json` 属于订阅数据，体积可能很大，且恢复后需要重新激活订阅，
//! 不在此模块范围内。
//!
//! 备份目录固定为 `data/backups/`（已在 `data/` 的 gitignore 覆盖内），每个备份
//! 是一个以 `backup-<时间戳>` 命名的子目录。恢复前会在 UI 上二次确认，恢复后由
//! 调用方重新执行 `apply_rules_to_config()` 并热重载。

use std::fs;
use std::path::{Path, PathBuf};

/// 备份根目录（相对工作目录）。
pub const BACKUP_DIR: &str = "data/backups";

/// (工作目录内的源文件, 备份目录内的文件名)
const BACKED_UP: &[(&str, &str)] = &[
    ("data/config.yaml", "config.yaml"),
    ("data/dns.json", "dns.json"),
    ("data/overrides.json", "overrides.json"),
    ("rules.json", "rules.json"),
];

/// 备份列表项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupEntry {
    pub name: String,
    pub files: usize,
    pub bytes: u64,
}

fn backup_dir(root: &Path) -> PathBuf {
    root.join(BACKUP_DIR)
}

/// 备份名只允许 `backup-` 前缀 + ASCII 字母数字/`-`/`_`，防止路径穿越。
pub fn is_safe_name(name: &str) -> bool {
    name.starts_with("backup-")
        && !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn format_bytes(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let b = bytes as f64;
    if b < K {
        format!("{} B", bytes)
    } else if b < K * K {
        format!("{:.1} KB", b / K)
    } else {
        format!("{:.1} MB", b / K / K)
    }
}

/// 列出全部备份，按名称（即时间）倒序。
pub fn list_backups(backup_dir: &Path) -> Vec<BackupEntry> {
    let mut entries = Vec::new();
    let Ok(read) = fs::read_dir(backup_dir) else {
        return entries;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_safe_name(&name) {
            continue;
        }
        let mut files = 0usize;
        let mut bytes = 0u64;
        if let Ok(read) = fs::read_dir(&path) {
            for f in read.flatten() {
                if f.path().is_file() {
                    files += 1;
                    bytes += f.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }
        entries.push(BackupEntry { name, files, bytes });
    }
    entries.sort_by(|a, b| b.name.cmp(&a.name));
    entries
}

/// 创建一次备份，返回备份目录名。没有可备份的文件时返回错误。
pub fn create_backup(root: &Path) -> Result<String, String> {
    let dir = backup_dir(root);
    let sources: Vec<(&str, &str)> = BACKED_UP
        .iter()
        .copied()
        .filter(|(rel, _)| root.join(rel).is_file())
        .collect();
    if sources.is_empty() {
        return Err("没有可备份的数据文件（先激活订阅生成 data/config.yaml）".to_string());
    }

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = format!("backup-{stamp}");
    let target = dir.join(&name);
    if target.exists() {
        return Err("备份过于频繁：同一秒内已创建过备份，请稍后再试".to_string());
    }
    fs::create_dir_all(&target).map_err(|e| format!("创建备份目录失败: {e}"))?;

    for (rel, file_name) in &sources {
        let src = root.join(rel);
        let dst = target.join(file_name);
        fs::copy(&src, &dst)
            .map_err(|e| format!("备份 {rel} 失败: {e}"))?;
    }
    Ok(name)
}

/// 恢复指定备份：把备份内的文件覆盖回工作目录。返回恢复的文件数。
pub fn restore_backup(root: &Path, name: &str) -> Result<usize, String> {
    if !is_safe_name(name) {
        return Err("非法的备份名".to_string());
    }
    let src_dir = backup_dir(root).join(name);
    if !src_dir.is_dir() {
        return Err(format!("备份 {name} 不存在"));
    }

    let mut restored = 0usize;
    for (target_rel, file_name) in BACKED_UP {
        let src = src_dir.join(file_name);
        if !src.is_file() {
            continue;
        }
        let dst = root.join(target_rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }
        fs::copy(&src, &dst).map_err(|e| format!("恢复 {target_rel} 失败: {e}"))?;
        restored += 1;
    }
    if restored == 0 {
        return Err(format!("备份 {name} 内没有可恢复的文件"));
    }
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "clash-tui-backup-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("data")).unwrap();
        dir
    }

    fn write(path: &Path, text: &str) {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(text.as_bytes()).unwrap();
    }

    #[test]
    fn create_list_and_restore_roundtrip() {
        let root = temp_root("roundtrip");
        write(&root.join("data/config.yaml"), "a: 1\n");
        write(&root.join("rules.json"), "[]\n");
        write(&root.join("data/dns.json"), "{\"direct_use_system_dns\": true}\n");

        let name = create_backup(&root).unwrap();
        assert!(name.starts_with("backup-"));

        let entries = list_backups(&backup_dir(&root));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, name);
        assert_eq!(entries[0].files, 3);

        // 改动现场，再恢复
        write(&root.join("data/config.yaml"), "b: 2\n");
        let count = restore_backup(&root, &name).unwrap();
        assert_eq!(count, 3);
        assert_eq!(
            fs::read_to_string(root.join("data/config.yaml")).unwrap(),
            "a: 1\n"
        );
        assert_eq!(fs::read_to_string(root.join("rules.json")).unwrap(), "[]\n");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn empty_data_dir_cannot_be_backed_up() {
        let root = temp_root("empty");
        assert!(create_backup(&root).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_unsafe_names() {
        assert!(is_safe_name("backup-20261120-101500"));
        assert!(!is_safe_name("../backup-20261120-101500"));
        assert!(!is_safe_name("backup-a/b"));
        assert!(!is_safe_name("backup-a\\b"));
        assert!(!is_safe_name("backup-.."));
        assert!(!is_safe_name("other-20261120"));

        let root = temp_root("unsafe");
        write(&root.join("data/config.yaml"), "a: 1\n");
        assert!(restore_backup(&root, "../backup-x").is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn lists_backups_newest_first() {
        let root = temp_root("order");
        let dir = backup_dir(&root);
        fs::create_dir_all(dir.join("backup-20260101-000000")).unwrap();
        fs::create_dir_all(dir.join("backup-20260102-000000")).unwrap();
        write(&dir.join("backup-20260101-000000/config.yaml"), "x: 1\n");
        let entries = list_backups(&dir);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "backup-20260102-000000");
        assert_eq!(entries[0].files, 0);
        assert_eq!(entries[1].name, "backup-20260101-000000");
        assert_eq!(entries[1].files, 1);
        let _ = fs::remove_dir_all(&root);
    }
}