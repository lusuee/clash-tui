//! Mihomo 内核下载与更新。
//!
//! 全部走官方 GitHub Release（`MetaCubeX/mihomo`）：
//! 1. 请求 `/releases/latest`，从重定向后的 tag 里拿到最新版本号；
//! 2. 按当前平台拼出资产名（Windows 为 `.zip`，其他平台为 `.gz`）并下载；
//! 3. 与本地内核 `mihomo -v` 的版本比较，新版本才安装；
//! 4. 安装时先停内核、备份旧文件、原子替换，再按需要重启内核。
//!
//! GitHub 直连失败时会自动回退到本地混合端口代理（127.0.0.1:mixed-port）。

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::core::CoreManager;

const RELEASES_LATEST: &str = "https://github.com/MetaCubeX/mihomo/releases/latest";
const DOWNLOAD_BASE: &str = "https://github.com/MetaCubeX/mihomo/releases/download";

/// 一次“检查并更新”的结果。
#[derive(Debug, Clone)]
pub struct KernelUpdateReport {
    /// 更新后（或已是最新时）的版本号，不含 `v` 前缀。
    pub latest: String,
    /// 更新前的本地版本（内核原本不存在时为 None）。
    pub previous: Option<String>,
    /// 是否真的下载并替换了内核文件。
    pub applied: bool,
    /// 是否在更新后重新启动了内核。
    pub restarted: bool,
    /// 面向状态栏的完整提示文案。
    pub message: String,
}

/// 从任意文本中提取第一个版本号（`v1.19.34` / `1.19.34`），返回不含 `v` 的形式。
///
/// 只接受前后为“非字母数字”边界的版本串，避免把 `go1.23.4` 之类的依赖版本误认。
pub fn extract_version(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let boundary = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        let start = if bytes[i] == b'v' || bytes[i] == b'V' {
            if !boundary {
                i += 1;
                continue;
            }
            i + 1
        } else if bytes[i].is_ascii_digit() {
            if !boundary {
                i += 1;
                continue;
            }
            i
        } else {
            i += 1;
            continue;
        };

        let mut j = start;
        let mut dots = 0usize;
        while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b'.') {
            if bytes[j] == b'.' {
                dots += 1;
            }
            j += 1;
        }
        if dots >= 2 && j > start && bytes[j - 1] != b'.' {
            return Some(text[start..j].to_string());
        }
        i = if j > i { j } else { i + 1 };
    }
    None
}

fn parse_tuple(version: &str) -> Option<(u64, u64, u64)> {
    let v = version.trim();
    let v = v
        .strip_prefix('v')
        .or_else(|| v.strip_prefix('V'))
        .unwrap_or(v);
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

/// `latest` 是否比 `current` 新。本地版本缺失或无法解析时视为需要更新。
pub fn is_newer(latest: &str, current: Option<&str>) -> bool {
    match (parse_tuple(latest), current.and_then(parse_tuple)) {
        (Some(latest), Some(current)) => latest > current,
        (Some(_), None) => true,
        _ => false,
    }
}

/// 按当前平台选择官方 Release 资产名。
pub fn asset_name(version: &str) -> Result<String, String> {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let name = match (os, arch) {
        ("windows", "x86_64") => format!("mihomo-windows-amd64-v{version}.zip"),
        ("windows", "aarch64") => format!("mihomo-windows-arm64-v{version}.zip"),
        ("windows", "x86") => format!("mihomo-windows-386-v{version}.zip"),
        ("linux", "x86_64") => format!("mihomo-linux-amd64-v{version}.gz"),
        ("linux", "aarch64") => format!("mihomo-linux-arm64-v{version}.gz"),
        ("linux", "x86") => format!("mihomo-linux-386-v{version}.gz"),
        ("macos", "x86_64") => format!("mihomo-darwin-amd64-v{version}.gz"),
        ("macos", "aarch64") => format!("mihomo-darwin-arm64-v{version}.gz"),
        (os, arch) => return Err(format!("暂不支持自动下载 {os}/{arch} 平台的 Mihomo 内核")),
    };
    Ok(name)
}

fn build_client(proxy_port: Option<u16>) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .user_agent(format!("clash-tui/{}", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(900));
    if let Some(port) = proxy_port {
        let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))
            .map_err(|e| format!("构造本地代理失败: {e}"))?;
        builder = builder.proxy(proxy);
    }
    builder.build().map_err(|e| format!("初始化 HTTP 客户端失败: {e}"))
}

fn version_from_url(url: &str) -> Option<String> {
    let segment = url.trim_end_matches('/').rsplit('/').next()?;
    if segment.starts_with('v') || segment.starts_with('V') {
        extract_version(segment)
    } else {
        None
    }
}

fn version_from_html(html: &str) -> Option<String> {
    let marker = "/releases/tag/";
    let start = html.find(marker)? + marker.len();
    extract_version(&html[start..])
}

/// 查询 GitHub 上的最新 Mihomo 版本号（不含 `v`）。
pub async fn latest_version(mixed_port: u16) -> Result<String, String> {
    let mut last_err = String::from("未知错误");
    for proxy in [None, Some(mixed_port)] {
        let client = match build_client(proxy) {
            Ok(client) => client,
            Err(e) => {
                last_err = e;
                continue;
            }
        };
        match client.get(RELEASES_LATEST).send().await {
            Ok(resp) => {
                let status = resp.status();
                let final_url = resp.url().to_string();
                if !status.is_success() {
                    last_err = format!("GitHub 返回 HTTP {status}");
                    continue;
                }
                let body = resp.text().await.unwrap_or_default();
                if let Some(version) =
                    version_from_url(&final_url).or_else(|| version_from_html(&body))
                {
                    return Ok(version);
                }
                last_err = "无法从 GitHub 响应中解析版本号".to_string();
            }
            Err(e) => last_err = format!("{e}"),
        }
    }
    Err(format!(
        "获取最新内核版本失败: {last_err}（已尝试直连与 127.0.0.1:{mixed_port} 代理）"
    ))
}

/// 读取本地内核二进制版本：执行 `mihomo -v` 并解析。文件不存在时返回 None。
pub fn installed_version(bin: &Path) -> Option<String> {
    if !bin.is_file() {
        return None;
    }
    let mut cmd = std::process::Command::new(bin);
    cmd.arg("-v");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let output = cmd.output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    extract_version(&stdout).or_else(|| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        extract_version(&stderr)
    })
}

async fn download_asset(
    url: &str,
    dest: &Path,
    mixed_port: u16,
    on_progress: &(dyn Fn(u8) + Sync),
) -> Result<u64, String> {
    let mut last_err = String::from("未知错误");
    for proxy in [None, Some(mixed_port)] {
        let client = match build_client(proxy) {
            Ok(client) => client,
            Err(e) => {
                last_err = e;
                continue;
            }
        };
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => match write_response(resp, dest, on_progress).await {
                Ok(size) => return Ok(size),
                Err(e) => last_err = e,
            },
            Ok(resp) => last_err = format!("HTTP {}", resp.status()),
            Err(e) => last_err = format!("{e}"),
        }
    }
    let _ = std::fs::remove_file(dest);
    Err(format!("下载内核失败: {last_err}"))
}

async fn write_response(
    mut resp: reqwest::Response,
    dest: &Path,
    on_progress: &(dyn Fn(u8) + Sync),
) -> Result<u64, String> {
    use tokio::io::AsyncWriteExt;

    let total = resp.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("创建下载文件失败: {e}"))?;
    let mut written: u64 = 0;
    let mut last_pct = 255u8;

    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => {
                file.write_all(&chunk)
                    .await
                    .map_err(|e| format!("写入下载文件失败: {e}"))?;
                written += chunk.len() as u64;
                if total > 0 {
                    let pct = ((written.saturating_mul(100) / total).min(100)) as u8;
                    if pct != last_pct {
                        on_progress(pct);
                        last_pct = pct;
                    }
                }
            }
            Ok(None) => break,
            Err(e) => return Err(format!("下载连接中断: {e}")),
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("写入下载文件失败: {e}"))?;
    if total > 0 && written != total {
        return Err(format!("下载不完整: {written}/{total} 字节"));
    }
    Ok(written)
}

#[cfg(target_os = "windows")]
fn extract_archive(archive: &Path, target: &Path) -> Result<(), String> {
    let file =
        std::fs::File::open(archive).map_err(|e| format!("打开压缩包失败: {e}"))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| format!("解析压缩包失败: {e}"))?;

    let mut kernel_index = None;
    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|e| format!("读取压缩包条目失败: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        // 官方 Windows 资产里的文件名是 `mihomo-windows-amd64.exe`，
        // 也可能被重命名为 `mihomo.exe`，因此按 “.exe + mihomo 前缀” 匹配。
        let name = entry.name().to_ascii_lowercase();
        if name.ends_with(".exe") && name.contains("mihomo") {
            kernel_index = Some(i);
            break;
        }
    }
    let index = kernel_index.ok_or_else(|| "压缩包内未找到 mihomo.exe".to_string())?;

    let mut entry = zip
        .by_index(index)
        .map_err(|e| format!("读取压缩包条目失败: {e}"))?;
    let mut out = std::fs::File::create(target).map_err(|e| format!("创建临时内核失败: {e}"))?;
    std::io::copy(&mut entry, &mut out).map_err(|e| format!("解压内核失败: {e}"))?;
    Ok(())
}

#[cfg(unix)]
fn extract_archive(archive: &Path, target: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| format!("打开压缩包失败: {e}"))?;
    let mut decoder = flate2::read::GzDecoder::new(file);
    let mut out = std::fs::File::create(target).map_err(|e| format!("创建临时内核失败: {e}"))?;
    std::io::copy(&mut decoder, &mut out).map_err(|e| format!("解压内核失败: {e}"))?;
    Ok(())
}

#[cfg(not(any(target_os = "windows", unix)))]
fn extract_archive(_archive: &Path, _target: &Path) -> Result<(), String> {
    Err("当前平台暂不支持自动解压 Mihomo 内核".to_string())
}

async fn rename_with_retry(from: &Path, to: &Path) -> std::io::Result<()> {
    let mut last_err = None;
    for _ in 0..6 {
        match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("rename failed")))
}

/// 检查并安装最新 Mihomo 内核。
///
/// - `was_running`：调用前内核是否在运行（决定更新后是否自动重启）；
/// - `on_progress`：下载进度 0..=100。
pub async fn update_kernel<P>(
    core: &CoreManager,
    mixed_port: u16,
    was_running: bool,
    on_progress: P,
) -> Result<KernelUpdateReport, String>
where
    P: Fn(u8) + Send + Sync,
{
    let latest = latest_version(mixed_port).await?;
    let previous = installed_version(&core.bin_path);

    if let Some(current) = previous.clone() {
        if !is_newer(&latest, Some(&current)) {
            on_progress(100);
            return Ok(KernelUpdateReport {
                latest: current.clone(),
                previous,
                applied: false,
                restarted: false,
                message: format!("✔ 当前内核 v{current} 已是最新（GitHub latest v{latest}）"),
            });
        }
    }

    let asset = asset_name(&latest)?;
    let download_dir = core.data_dir.join("kernel-download");
    std::fs::create_dir_all(&download_dir)
        .map_err(|e| format!("创建下载目录失败: {e}"))?;
    let archive_path = download_dir.join(&asset);
    let url = format!("{DOWNLOAD_BASE}/v{latest}/{asset}");

    on_progress(0);
    let size = download_asset(&url, &archive_path, mixed_port, &on_progress).await?;
    on_progress(100);

    let target = core.bin_path.clone();
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&parent).map_err(|e| format!("创建内核目录失败: {e}"))?;

    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("mihomo.exe")
        .to_string();
    let tmp_path = parent.join(format!("{file_name}.new"));
    let _ = std::fs::remove_file(&tmp_path);
    extract_archive(&archive_path, &tmp_path)?;

    let was_installed = target.is_file();
    let mut backup: Option<PathBuf> = None;

    if was_installed {
        // Windows 上替换被占用的 exe 前必须先停内核。
        let _ = core.stop_core();
        tokio::time::sleep(Duration::from_millis(300)).await;

        let backup_path = parent.join(format!("{file_name}.old"));
        let _ = std::fs::remove_file(&backup_path);
        if let Err(e) = std::fs::rename(&target, &backup_path) {
            let _ = std::fs::remove_file(&tmp_path);
            let _ = std::fs::remove_file(&archive_path);
            if was_running {
                let _ = core.start_core();
            }
            return Err(format!(
                "无法替换内核 {}: {e}（可能仍被占用或缺少权限）",
                target.display()
            ));
        }
        backup = Some(backup_path);
    }

    if let Err(e) = rename_with_retry(&tmp_path, &target).await {
        if let Some(backup_path) = &backup {
            let _ = std::fs::rename(backup_path, &target);
        }
        let _ = std::fs::remove_file(&tmp_path);
        let _ = std::fs::remove_file(&archive_path);
        if was_running {
            let _ = core.start_core();
        }
        return Err(format!(
            "写入内核失败: {e}（安装目录可能需要管理员权限，试试以管理员身份运行 clash-tui）"
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755));
    }

    let _ = std::fs::remove_file(&archive_path);
    let _ = std::fs::remove_dir(&download_dir);

    let installed = installed_version(&target).unwrap_or_else(|| latest.clone());
    let size_mb = size as f64 / 1_048_576.0;
    let mut message = match &previous {
        Some(prev) => format!("✔ Mihomo 内核已更新 v{prev} → v{installed}（{size_mb:.1} MB）"),
        None => format!(
            "✔ Mihomo 内核 v{installed} 已下载并安装到 {}（{size_mb:.1} MB）",
            target.display()
        ),
    };

    let mut restarted = false;
    if was_running {
        match core.start_core() {
            Ok(_) => {
                restarted = true;
                message.push_str("，内核已重新启动");
            }
            Err(e) => message.push_str(&format!("；内核重启失败: {e}（按 S 重试）")),
        }
    } else if previous.is_none() && !was_installed {
        message.push_str("（内核未运行，按 S 启动）");
    }

    if let Some(backup_path) = backup {
        let _ = std::fs::remove_file(backup_path);
    }

    Ok(KernelUpdateReport {
        latest: installed,
        previous,
        applied: true,
        restarted,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_version_from_mihomo_banner() {
        let banner = "Mihomo Meta v1.19.31 windows amd64 with go1.23.4 Load: 0.12";
        assert_eq!(extract_version(banner).as_deref(), Some("1.19.31"));
    }

    #[test]
    fn extracts_version_from_release_url() {
        let url = "https://github.com/MetaCubeX/mihomo/releases/tag/v1.19.34";
        assert_eq!(version_from_url(url).as_deref(), Some("1.19.34"));
        assert!(
            version_from_url("https://github.com/MetaCubeX/mihomo/releases/latest").is_none()
        );
    }

    #[test]
    fn compares_versions() {
        assert!(is_newer("1.19.34", Some("1.19.31")));
        assert!(!is_newer("1.19.31", Some("1.19.31")));
        assert!(!is_newer("1.19.31", Some("v1.19.34")));
        assert!(is_newer("1.19.34", None));
        assert!(is_newer("1.20.0", Some("1.19.99")));
    }

    #[test]
    fn builds_platform_asset_name() {
        let name = asset_name("1.19.34").expect("当前平台应支持自动下载");
        assert!(name.starts_with("mihomo-"), "{name}");
        assert!(name.contains("1.19.34"), "{name}");
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        assert_eq!(name, "mihomo-windows-amd64-v1.19.34.zip");
    }

    /// 手动跑一次的端到端验证（会联网下载真实内核到临时目录，不碰仓库里的 bin/）：
    /// `cargo test -- --ignored end_to_end_download_to_temp_dir --nocapture`
    #[tokio::test]
    #[ignore = "network: 从 GitHub 下载最新内核到临时目录"]
    async fn end_to_end_download_to_temp_dir() {
        let dir = std::env::temp_dir().join(format!("clash-tui-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let core = crate::core::CoreManager {
            bin_path: dir.join("mihomo.exe"),
            data_dir: dir.join("data"),
        };

        let report = update_kernel(&core, 7897, false, |_| {})
            .await
            .expect("下载/安装内核应成功");
        assert!(report.applied, "{}", report.message);
        assert!(core.bin_path.is_file());
        assert_eq!(
            installed_version(&core.bin_path).as_deref(),
            Some(report.latest.as_str())
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(all(test, target_os = "windows"))]
mod zip_tests {
    use super::*;

    #[test]
    fn extracts_mihomo_exe_from_zip() {
        use std::io::Write;

        let dir = std::env::temp_dir().join(format!(
            "clash-tui-kernel-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let zip_path = dir.join("kernel.zip");
        {
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
            let options = zip::write::SimpleFileOptions::default();
            writer.start_file("mihomo.exe", options).unwrap();
            writer.write_all(b"fake-kernel-bytes").unwrap();
            writer.finish().unwrap();
        }

        let out = dir.join("mihomo.exe");
        extract_archive(&zip_path, &out).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "fake-kernel-bytes");
        let _ = std::fs::remove_dir_all(&dir);
    }
}