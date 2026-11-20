//! 内核体检与日志查看（Settings → `H` / `L`）。
//!
//! 检查项尽量用零副作用手段：进程存在性、REST /version、磁盘二进制 `-v`、
//! TCP 端口连通性、配置文件与日志文件状态。

use crate::api::ClashClient;
use crate::core::CoreManager;
use crate::kernelupdate;
use std::io::{Read, Seek, SeekFrom};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const LOG_PATH: &str = "data/mihomo.log";
const LOG_TAIL_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthItem {
    pub label: String,
    pub detail: String,
    /// Some(true)=通过、Some(false)=异常、None=无法判断/仅供参考
    pub ok: Option<bool>,
}

impl HealthItem {
    pub fn ok(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { label: label.into(), detail: detail.into(), ok: Some(true) }
    }

    pub fn warn(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { label: label.into(), detail: detail.into(), ok: Some(false) }
    }

    pub fn info(label: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { label: label.into(), detail: detail.into(), ok: None }
    }
}

fn tcp_open(host: &str, port: u16) -> bool {
    let Ok(addr) = format!("{}:{}", host, port).parse::<SocketAddr>() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(600)).is_ok()
}

fn file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}

pub fn format_file_size(bytes: Option<u64>) -> String {
    match bytes {
        None => "缺失".to_string(),
        Some(b) if b < 1024 => format!("{} B", b),
        Some(b) => format!("{:.1} KB", b as f64 / 1024.0),
    }
}

fn config_path() -> PathBuf {
    Path::new("data").join("config.yaml")
}

/// 收集一次体检结果。端口探测与 `-v` 会阻塞，放到 blocking 线程里跑。
pub async fn collect_health(
    core: &CoreManager,
    api_url: &str,
    secret: Option<String>,
    mixed_port: u16,
) -> Vec<HealthItem> {
    let mut items = Vec::new();

    let alive = CoreManager::process_alive();
    items.push(if alive {
        HealthItem::ok("内核进程", "mihomo 正在运行")
    } else {
        HealthItem::warn("内核进程", "未检测到 mihomo 进程（代理不会工作）")
    });

    let client = ClashClient::new(api_url, secret);
    let rest_version = match tokio::time::timeout(Duration::from_secs(2), client.get_version()).await {
        Ok(Ok(v)) => {
            items.push(HealthItem::ok(
                "REST 控制器",
                format!("{} 可达 · {}", api_url, display_version(&v.version)),
            ));
            Some(v.version)
        }
        Ok(Err(e)) => {
            items.push(HealthItem::warn("REST 控制器", format!("{} 不可达：{}", api_url, e)));
            None
        }
        Err(_) => {
            items.push(HealthItem::warn("REST 控制器", format!("{} 请求超时", api_url)));
            None
        }
    };

    let bin_path = core.bin_path.clone();
    let disk_version = tokio::task::spawn_blocking(move || {
        if bin_path.is_file() {
            Some((bin_path.clone(), kernelupdate::installed_version(&bin_path)))
        } else {
            None
        }
    })
    .await
    .ok()
    .flatten();

    match &disk_version {
        Some((path, Some(ver))) => items.push(HealthItem::ok(
            "磁盘二进制",
            format!("{} · {}", shorten_path(path), display_version(ver)),
        )),
        Some((path, None)) => items.push(HealthItem::warn(
            "磁盘二进制",
            format!("{} 存在但无法读取版本（可能损坏）", shorten_path(path)),
        )),
        None => items.push(HealthItem::warn("磁盘二进制", "未安装内核（可在设置页按 K 下载）")),
    }

    match (&rest_version, disk_version.as_ref().and_then(|(_, v)| v.as_deref())) {
        (Some(run), Some(disk)) => {
            let (run, disk) = (normalize_version(run), normalize_version(disk));
            if run == disk {
                items.push(HealthItem::ok("版本一致性", format!("运行与磁盘一致：{}", disk)));
            } else {
                items.push(HealthItem::warn(
                    "版本一致性",
                    format!("运行 {} ≠ 磁盘 {}（需重启内核，见设置页 S）", run, disk),
                ));
            }
        }
        _ => items.push(HealthItem::info("版本一致性", "缺少运行或磁盘版本，无法比较")),
    }

    let port = mixed_port;
    let mixed_open = tokio::task::spawn_blocking(move || tcp_open("127.0.0.1", port))
        .await
        .unwrap_or(false);
    items.push(if mixed_open {
        HealthItem::ok("混合代理端口", format!("127.0.0.1:{} 可连接", mixed_port))
    } else {
        HealthItem::warn(
            "混合代理端口",
            format!("127.0.0.1:{} 无法连接（内核未启动或端口被占用）", mixed_port),
        )
    });

    let cfg = config_path();
    let cfg_size = file_size(&cfg);
    items.push(match cfg_size {
        Some(size) if size > 0 => HealthItem::ok("生效配置", format!("data/config.yaml · {}", format_file_size(Some(size)))),
        _ => HealthItem::warn("生效配置", "data/config.yaml 缺失或为空（订阅未激活？）"),
    });

    let log_size = file_size(Path::new(LOG_PATH));
    items.push(HealthItem::info(
        "内核日志",
        format!("{} · {}", LOG_PATH, format_file_size(log_size)),
    ));

    items
}

fn normalize_version(v: &str) -> String {
    v.trim().trim_start_matches('v').to_string()
}

fn display_version(v: &str) -> String {
    let v = v.trim();
    if v.starts_with('v') {
        v.to_string()
    } else {
        format!("v{}", v)
    }
}

fn shorten_path(path: &Path) -> String {
    let s = path.to_string_lossy().to_string();
    s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
}

// ----------------------------------------------------------------------------
// 日志
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Other,
}

pub fn classify_log_line(line: &str) -> LogLevel {
    let lower = line.to_ascii_lowercase();
    if lower.contains("level=error")
        || lower.contains("level=fatal")
        || lower.contains("error")
        || lower.contains("panic")
    {
        LogLevel::Error
    } else if lower.contains("level=warn") || lower.contains("warn") {
        LogLevel::Warn
    } else if lower.contains("level=info") || lower.contains("info") {
        LogLevel::Info
    } else {
        LogLevel::Other
    }
}

/// 读取日志文件末尾若干行（最多读尾部 256KB，避免把大日志整个读进内存）。
pub fn read_log_tail(max_lines: usize) -> Result<Vec<String>, String> {
    let path = Path::new(LOG_PATH);
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("无法打开 {}：{}", LOG_PATH, e))?;
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if len > LOG_TAIL_BYTES {
        file.seek(SeekFrom::Start(len - LOG_TAIL_BYTES))
            .map_err(|e| format!("读取日志失败：{}", e))?;
    }
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| format!("读取日志失败：{}", e))?;
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    if lines.len() > max_lines {
        lines.drain(0..lines.len() - max_lines);
    }
    Ok(lines)
}

/// 按关键字过滤（大小写不敏感），保留最后 `limit` 条匹配，保持原有顺序。
pub fn filter_lines(lines: &[String], filter: &str, limit: usize) -> Vec<String> {
    let needle = filter.trim().to_ascii_lowercase();
    let mut matched: Vec<String> = lines
        .iter()
        .filter(|line| needle.is_empty() || line.to_ascii_lowercase().contains(&needle))
        .cloned()
        .collect();
    if matched.len() > limit {
        matched.drain(0..matched.len() - limit);
    }
    matched
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_log_levels() {
        assert_eq!(classify_log_line("time=1 level=info msg=start"), LogLevel::Info);
        assert_eq!(classify_log_line("level=warning msg=deprecated"), LogLevel::Warn);
        assert_eq!(classify_log_line("level=error msg=listen udp :53: bind"), LogLevel::Error);
        assert_eq!(classify_log_line("panic: boom"), LogLevel::Error);
        assert_eq!(classify_log_line("plain output"), LogLevel::Other);
    }

    #[test]
    fn filters_case_insensitively_and_keeps_last_lines() {
        let lines: Vec<String> = vec![
            "start DNS".into(),
            "ERROR listen udp".into(),
            "warn: slow".into(),
            "error again".into(),
        ];
        let matched = filter_lines(&lines, "error", 1);
        assert_eq!(matched, vec!["error again".to_string()]);
        assert_eq!(filter_lines(&lines, "  ", 10).len(), 4);
    }

    #[test]
    fn formats_sizes() {
        assert_eq!(format_file_size(None), "缺失");
        assert_eq!(format_file_size(Some(512)), "512 B");
        assert_eq!(format_file_size(Some(2048)), "2.0 KB");
    }
}