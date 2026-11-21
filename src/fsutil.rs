use std::fs;
use std::path::Path;

/// 将内容原子写入目标文件：先写入同目录下的临时文件，再原子替换（rename）目标文件。
/// 避免写入中途进程被杀、系统断电或磁盘满导致原文件被截断为 0 字节损坏文件。
pub fn atomic_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, content: C) -> std::io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let tmp_path = path.with_file_name(format!(".{}.tmp.{}", file_name, std::process::id()));

    fs::write(&tmp_path, content)?;

    #[cfg(windows)]
    {
        // Windows 下若目标文件已存在，直接 rename 可能会报已存在或拒绝访问
        if path.exists() {
            let backup_tmp =
                path.with_file_name(format!(".{}.old.{}", file_name, std::process::id()));
            let _ = fs::remove_file(&backup_tmp);
            if fs::rename(path, &backup_tmp).is_ok() {
                if let Err(e) = fs::rename(&tmp_path, path) {
                    let _ = fs::rename(&backup_tmp, path);
                    let _ = fs::remove_file(&tmp_path);
                    return Err(e);
                }
                let _ = fs::remove_file(&backup_tmp);
                return Ok(());
            }
        }
    }

    if let Err(e) = fs::rename(&tmp_path, path) {
        let _ = fs::remove_file(&tmp_path);
        return Err(e);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_write_new_and_overwrite() {
        let dir = std::env::temp_dir().join(format!("clash-tui-fsutil-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let target = dir.join("sub/config.txt");

        // 1. 首次写入（自动创建父目录）
        atomic_write(&target, "first content").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "first content");

        // 2. 覆盖写入
        atomic_write(&target, "second content").unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "second content");

        let _ = fs::remove_dir_all(&dir);
    }
}
