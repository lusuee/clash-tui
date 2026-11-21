//! clash-tui 自身的版本信息与自更新。
//!
//! - 版本号由 `build.rs` 在编译期注入（CI 用 Release tag，本地回退到 `git describe`），
//!   因此发布出来的二进制显示的版本与 GitHub Release 一致；
//! - 启动时静默检查一次 GitHub latest，只把“有新版本”提示到状态栏/设置页；
//! - 用户在 Settings 按 `U` 才下载、解压并替换当前 exe，随后自动重启 TUI（不影响内核）。

use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::Client;

pub const RELEASES_LATEST: &str = "https://github.com/lusuee/clash-tui/releases/latest";
pub const DOWNLOAD_BASE: &str = "https://github.com/lusuee/clash-tui/releases/download";

/// 默认的 GitHub API 最新 Release 地址。
///
/// 优先用它而不是 `releases/latest` 重定向：第三方 GitHub 镜像（如 Apple 镜像）
/// 会把 `user/repo` 段替换成镜像自己的 NAS 路径，导致拼出的资产 URL 404/超时。
/// 环境变量 `CLASH_TUI_GITHUB_API_BASE` 仅供测试注入本地 mock。
pub const GITHUB_API_LATEST: &str =
    "https://api.github.com/repos/lusuee/clash-tui/releases/latest";

fn api_latest_url() -> String {
    match std::env::var("CLASH_TUI_GITHUB_API_BASE") {
        Ok(base) if !base.trim().is_empty() => {
            format!("{}/repos/lusuee/clash-tui/releases/latest", base.trim_end_matches('/'))
        }
        _ => GITHUB_API_LATEST.to_string(),
    }
}

/// 编译期注入的版本号（`build.rs` 保证该变量存在）。
pub const VERSION: &str = env!("CLASH_TUI_VERSION");

#[cfg(target_os = "windows")]
const BIN_NAME: &str = "clash-tui.exe";
#[cfg(not(target_os = "windows"))]
const BIN_NAME: &str = "clash-tui";

/// 供 UI 展示的版本号：纯数字补 `v` 前缀，其余（如 `v0.2.5-1-gabc-dirty`）原样显示。
pub fn version_label() -> String {
    let v = VERSION.trim();
    let starts_with_digit = v
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false);
    if starts_with_digit {
        format!("v{v}")
    } else {
        v.to_string()
    }
}

/// 解析 `v0.2.5` / `0.2.5` 这样的三段式版本；带后缀的开发版返回 None（不参与自动升级）。
pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let text = text.trim();
    let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
    let mut parts = text.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    let patch = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let parse = |part: &str| -> Option<u64> {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        part.parse().ok()
    };
    Some((parse(major)?, parse(minor)?, parse(patch)?))
}

/// `latest` 是否比 `current` 新；任一方无法解析时视为“不更新”（保护开发版）。
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// 当前平台在 GitHub Release 中的资产文件名。
pub fn asset_name() -> Result<String, String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    match (os, arch) {
        ("windows", "x86_64") => Ok("clash-tui-windows-x64.zip".to_string()),
        ("linux", "x86_64") => Ok("clash-tui-linux-x64.tar.gz".to_string()),
        ("macos", "x86_64") => Ok("clash-tui-darwin-x64.tar.gz".to_string()),
        ("macos", "aarch64") => Ok("clash-tui-darwin-arm64.tar.gz".to_string()),
        _ => Err(format!(
            "当前平台 {os}/{arch} 暂无预编译包，请到 GitHub Releases 手动下载"
        )),
    }
}

fn build_client(proxy_port: Option<u16>) -> Result<Client, String> {
    let mut builder = Client::builder()
        .user_agent(format!("clash-tui/{}", VERSION))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600));
    if let Some(port) = proxy_port.filter(|p| *p > 0) {
        let proxy = reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))
            .map_err(|e| format!("构造本地代理失败: {e}"))?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|e| format!("初始化 HTTP 客户端失败: {e}"))
}

/// 从 URL/HTML 中抽取三段式版本号（不含 `v`），并确保前后不是字母数字
/// （避免把 `go1.26.11` 之类误认成版本）。
fn extract_version(text: &str) -> Option<String> {
    let start = text.find(|c: char| c.is_ascii_digit())?;
    if let Some(prev) = text[..start].chars().last() {
        // 允许紧邻的 `v` 前缀，但拒绝 `go1.26.11`、`x1.2.3` 之类的前缀。
        if prev.is_ascii_alphanumeric() && prev != 'v' && prev != 'V' {
            return None;
        }
    }
    let rest = &text[start..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    let candidate = rest[..end].trim_end_matches('.');
    parse_version(candidate)?;
    Some(candidate.to_string())
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

/// 查询 GitHub 上的最新 clash-tui 版本（不含 `v`）。
///
/// 顺序：GitHub API → Releases 页重定向。正式环境只有 `api.github.com` 能成功，
/// 第三方 mirror 通过环境变量或镜像解析无法冒充官方 API，避免拼出错误的资产 URL。
pub async fn check_latest(proxy_port: u16) -> Result<String, String> {
    match check_latest_api(proxy_port).await {
        Ok(version) => Ok(version),
        Err(api_err) => match check_latest_redirect(proxy_port).await {
            Ok(version) => Ok(version),
            Err(redirect_err) => Err(format!(
                "获取 clash-tui 最新版本失败: {api_err}; {redirect_err}"
            )),
        },
    }
}

/// GitHub API 路径：读取 `tag_name`。
async fn check_latest_api(proxy_port: u16) -> Result<String, String> {
    let url = api_latest_url();
    let proxies = [None, Some(proxy_port).filter(|p| *p > 0)];
    let mut last_err = String::from("未知错误");
    for proxy in proxies {
        let client = match build_client(proxy) {
            Ok(client) => client,
            Err(e) => {
                last_err = e;
                continue;
            }
        };
        match client.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    last_err = format!("GitHub API 返回 HTTP {status}");
                    continue;
                }
                let body = resp.text().await.unwrap_or_default();
                if let Some(version) = tag_name_from_release_json(&body) {
                    return Ok(version);
                }
                last_err = "无法从 GitHub API 响应中解析版本号".to_string();
            }
            Err(e) => last_err = format!("{e}"),
        }
    }
    Err(last_err)
}

/// Releases 页重定向路径（API 被限流或不可用时兜底）。
async fn check_latest_redirect(proxy_port: u16) -> Result<String, String> {
    let proxies = [None, Some(proxy_port).filter(|p| *p > 0)];
    let mut last_err = String::from("未知错误");
    for proxy in proxies {
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
    Err(last_err)
}

/// 从 GitHub Release JSON 里提取 `"tag_name": "v0.2.9"`。
fn tag_name_from_release_json(body: &str) -> Option<String> {
    let marker = "\"tag_name\"";
    let start = body.find(marker)? + marker.len();
    let rest = &body[start..];
    let colon = rest.find(':')?;
    let rest = &rest[colon + 1..];
    let quote = rest.find('"')?;
    let rest = &rest[quote + 1..];
    let end = rest.find('"')?;
    extract_version(&rest[..end])
}

async fn download_asset(
    url: &str,
    dest: &Path,
    proxy_port: u16,
    on_progress: &(dyn Fn(u8) + Sync),
) -> Result<(), String> {
    let proxies = [None, Some(proxy_port).filter(|p| *p > 0)];
    let mut last_err = String::from("未知错误");
    for proxy in proxies {
        let client = match build_client(proxy) {
            Ok(client) => client,
            Err(e) => {
                last_err = e;
                continue;
            }
        };
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match write_response(resp, dest, on_progress).await {
                    Ok(()) => return Ok(()),
                    Err(e) => last_err = e,
                }
            }
            Ok(resp) => last_err = format!("HTTP {}", resp.status()),
            Err(e) => last_err = format!("{e}"),
        }
    }
    let _ = std::fs::remove_file(dest);
    Err(format!("下载更新包失败: {last_err}"))
}

async fn write_response(
    mut resp: reqwest::Response,
    dest: &Path,
    on_progress: &(dyn Fn(u8) + Sync),
) -> Result<(), String> {
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
                if let Some(pct) = written.saturating_mul(100).checked_div(total) {
                    let pct = pct.min(100) as u8;
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
    Ok(())
}

#[cfg(target_os = "windows")]
fn extract_binary(archive: &Path, target: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| format!("打开更新包失败: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("解析更新包失败: {e}"))?;

    let mut index = None;
    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|e| format!("读取更新包条目失败: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        // 发布物形如 `clash-tui-windows-x64/clash-tui.exe`，这里按 “.exe + clash-tui” 匹配。
        let name = entry.name().to_ascii_lowercase();
        let file_name = name.rsplit('/').next().unwrap_or(&name);
        if file_name.ends_with(".exe") && file_name.starts_with("clash-tui") {
            index = Some(i);
            break;
        }
    }
    let index = index.ok_or_else(|| "更新包内未找到 clash-tui.exe".to_string())?;
    let mut entry = zip
        .by_index(index)
        .map_err(|e| format!("读取更新包条目失败: {e}"))?;
    let mut out =
        std::fs::File::create(target).map_err(|e| format!("创建新版本文件失败: {e}"))?;
    std::io::copy(&mut entry, &mut out).map_err(|e| format!("解压新版本失败: {e}"))?;
    Ok(())
}

#[cfg(unix)]
fn extract_binary(archive: &Path, target: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| format!("打开更新包失败: {e}"))?;
    let mut tar =
        tar::Archive::new(flate2::read::GzDecoder::new(file));
    let entries = tar
        .entries()
        .map_err(|e| format!("解析更新包失败: {e}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("读取更新包条目失败: {e}"))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .map_err(|e| format!("读取更新包条目失败: {e}"))?
            .to_path_buf();
        if path.file_name().and_then(|n| n.to_str()) != Some(BIN_NAME) {
            continue;
        }
        let mut out =
            std::fs::File::create(target).map_err(|e| format!("创建新版本文件失败: {e}"))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("解压新版本失败: {e}"))?;
        return Ok(());
    }
    Err(format!("更新包内未找到 {BIN_NAME}"))
}

#[cfg(not(any(target_os = "windows", unix)))]
fn extract_binary(_archive: &Path, _target: &Path) -> Result<(), String> {
    Err("当前平台暂不支持自动更新".to_string())
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// 去掉 Windows `\\?\` 长路径前缀（canonicalize 会带回来）。
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy().to_string();
    let cleaned = if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        text
    };
    PathBuf::from(cleaned)
}

/// 用新二进制替换当前正在运行的 exe。
///
/// Windows 上不能直接覆盖运行中的 exe，但可以先把自身改名（旧文件仍被运行中的进程持有），
/// 再把新文件放到原路径；新进程启动后由 [`cleanup_stale_backup`] 清理 `.old`。
fn install_running_binary(new_binary: &Path) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("定位当前程序失败: {e}"))?;
    let exe = std::fs::canonicalize(&exe)
        .map(strip_verbatim)
        .unwrap_or_else(|_| strip_verbatim(exe));
    install_binary_at(&exe, new_binary)
}

/// 把新二进制装到 `target`（`target` 可能正是当前运行中的 exe）。
///
/// 两步落地：先完整复制到同目录的 `.new`（复制失败时旧文件完全没动），再把旧 exe
/// 改名成 `.old`，最后把 `.new` 原子改名为目标名。Windows 允许重命名运行中的文件，
/// 但不允许覆盖它——这也是之前“解压阶段直接写 exe 失败、把旧备份删掉”的修复点。
fn install_binary_at(target: &Path, new_binary: &Path) -> Result<PathBuf, String> {
    let backup = with_suffix(target, ".old");
    let staged = with_suffix(target, ".new");
    let _ = std::fs::remove_file(&backup);
    let _ = std::fs::remove_file(&staged);

    match std::fs::copy(new_binary, &staged) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            request_elevated_update(new_binary, target)?;
            return Ok(backup);
        }
        Err(e) => {
            let _ = std::fs::remove_file(&staged);
            return Err(format!(
                "写入新版本失败: {e}（安装目录可能需要管理员权限）"
            ));
        }
    }

    let mut moved_old = false;
    if target.exists() {
        if let Err(e) = std::fs::rename(target, &backup) {
            let _ = std::fs::remove_file(&staged);
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                return request_elevated_update(new_binary, target).map(|_| backup);
            }
            return Err(format!(
                "无法备份当前程序（安装目录可能需要管理员权限）: {e}"
            ));
        }
        moved_old = true;
    }

    if let Err(e) = std::fs::rename(&staged, target) {
        let _ = std::fs::remove_file(&staged);
        if moved_old {
            let _ = std::fs::rename(&backup, target);
        }
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return request_elevated_update(new_binary, target).map(|_| backup);
        }
        return Err(format!(
            "写入新版本失败: {e}（安装目录可能需要管理员权限）"
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o755));
    }
    Ok(backup)
}

/// 提权助手模式的环境变量标记（用环境变量传路径，避免 Start-Process 参数转义问题）。
pub const ELEVATED_MARKER: &str = "CLASH_TUI_APPLY_UPDATE";
pub const ELEVATED_FROM: &str = "CLASH_TUI_UPDATE_FROM";
pub const ELEVATED_TO: &str = "CLASH_TUI_UPDATE_TO";

/// 本进程是否由自更新以管理员身份拉起（是则返回待替换的文件路径）。
pub fn elevated_update_request() -> Option<(PathBuf, PathBuf)> {
    if std::env::var(ELEVATED_MARKER).ok().as_deref() != Some("1") {
        return None;
    }
    let from = std::env::var(ELEVATED_FROM).ok()?;
    let to = std::env::var(ELEVATED_TO).ok()?;
    if from.is_empty() || to.is_empty() {
        return None;
    }
    Some((PathBuf::from(from), PathBuf::from(to)))
}

/// 提权进程内执行的替换：先把新镜像写进 `.new` 暂存位，再改名落地（失败回滚）。
pub fn apply_update_elevated(update: &Path, target: &Path) -> Result<(), String> {
    let backup = with_suffix(target, ".old");
    let staged = with_suffix(target, ".new");
    let _ = std::fs::remove_file(&staged);
    std::fs::copy(update, &staged).map_err(|e| format!("写入新版本失败: {e}"))?;

    let mut moved_old = false;
    if target.exists() {
        let _ = std::fs::remove_file(&backup);
        if let Err(e) = std::fs::rename(target, &backup) {
            let _ = std::fs::remove_file(&staged);
            return Err(format!("备份旧版本失败: {e}"));
        }
        moved_old = true;
    }
    if let Err(e) = std::fs::rename(&staged, target) {
        let _ = std::fs::remove_file(&staged);
        if moved_old {
            let _ = std::fs::rename(&backup, target);
        }
        return Err(format!("写入新版本失败: {e}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

#[cfg(windows)]
fn request_elevated_update(update: &Path, target: &Path) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("定位当前程序失败: {e}"))?;
    let exe = std::fs::canonicalize(&exe)
        .map(strip_verbatim)
        .unwrap_or_else(|_| strip_verbatim(exe));
    // 单引号转义：PowerShell 里用 '' 表示一个 '。
    let ps_exe = exe.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$p = Start-Process -FilePath '{ps_exe}' -Verb RunAs -Wait -PassThru -WindowStyle Hidden; exit $p.ExitCode"
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &script])
        .env(ELEVATED_MARKER, "1")
        .env(ELEVATED_FROM, update)
        .env(ELEVATED_TO, target)
        .status()
        .map_err(|e| format!("无法发起管理员提权: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("提权被取消或失败：写入程序目录需要管理员权限，请以管理员身份运行 clash-tui 后再更新".to_string())
    }
}

#[cfg(not(windows))]
fn request_elevated_update(_update: &Path, target: &Path) -> Result<(), String> {
    Err(format!(
        "写入 {target:?} 需要更高权限，请以管理员身份运行 clash-tui 后再更新"
    ))
}

/// 删除上一次更新留下的 `.old`（旧镜像已不再运行，删除失败可忽略）。
pub fn cleanup_stale_backup() {
    if let Ok(exe) = std::env::current_exe() {
        let exe = std::fs::canonicalize(&exe)
            .map(strip_verbatim)
            .unwrap_or_else(|_| strip_verbatim(exe));
        let backup = with_suffix(&exe, ".old");
        if backup.exists() {
            let _ = std::fs::remove_file(backup);
        }
    }
}

/// 退出当前进程后，用同目录的新 exe 重新拉起 TUI（沿用原命令行参数与工作目录）。
/// 自更新重启参数：新进程先等待父进程退出、重置终端状态，再接管 TUI。
pub const RESTART_FLAG: &str = "--restart-after-exit";

pub fn restart_into_new_binary() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("定位当前程序失败: {e}"))?;
    let exe = std::fs::canonicalize(&exe)
        .map(strip_verbatim)
        .unwrap_or_else(|_| strip_verbatim(exe));
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|a| a == RESTART_FLAG) {
        args.push(RESTART_FLAG.to_string());
    }
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(args);
    if let Ok(cwd) = std::env::current_dir() {
        cmd.current_dir(cwd);
    }
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("启动新版本失败: {e}，请手动重新打开 clash-tui"))
}

#[derive(Debug, Clone)]
pub struct AppUpdateReport {
    /// GitHub latest 版本（不含 `v`）。
    pub latest: String,
    /// 更新前的本地版本标签。
    pub previous: String,
    /// 是否真的替换了可执行文件。
    pub applied: bool,
    pub message: String,
}

/// 检查并安装最新 clash-tui：若已是最新或本地为开发版则不做任何替换。
pub async fn update_app<P>(
    proxy_port: u16,
    on_progress: P,
) -> Result<AppUpdateReport, String>
where
    P: Fn(u8) + Send + Sync,
{
    let latest = check_latest(proxy_port).await?;
    let previous = version_label();

    if !is_newer(&latest, VERSION) {
        on_progress(100);
        return Ok(AppUpdateReport {
            latest: latest.clone(),
            previous: previous.clone(),
            applied: false,
            message: format!("✔ 当前已是最新版本 {previous}"),
        });
    }

    if parse_version(VERSION).is_none() {
        return Err(format!(
            "当前运行的是开发版 {previous}，自动更新只对正式发布版生效，请手动下载 v{latest}"
        ));
    }

    let asset = asset_name()?;
    let dir = PathBuf::from("data").join("app-update");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建更新目录失败: {e}"))?;
    let archive = dir.join(&asset);
    let new_binary = dir.join(BIN_NAME);
    let url = format!("{DOWNLOAD_BASE}/v{latest}/{asset}");

    on_progress(0);
    download_asset(&url, &archive, proxy_port, &on_progress).await?;
    on_progress(100);

    let _ = std::fs::remove_file(&new_binary);
    extract_binary(&archive, &new_binary)?;

    install_running_binary(&new_binary)?;
    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_file(&new_binary);

    Ok(AppUpdateReport {
        latest: latest.clone(),
        previous,
        applied: true,
        message: format!("✔ clash-tui 已更新到 v{latest}，正在重启..."),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_injected_at_build_time() {
        assert!(
            !VERSION.trim().is_empty(),
            "build.rs 必须注入 CLASH_TUI_VERSION"
        );
        println!("CLASH_TUI_VERSION={VERSION} (label: {})", version_label());
    }

    #[test]
    fn parses_release_style_versions() {
        assert_eq!(parse_version("v0.2.5"), Some((0, 2, 5)));
        assert_eq!(parse_version("0.2.5"), Some((0, 2, 5)));
        assert_eq!(parse_version("V12.34.56"), Some((12, 34, 56)));
        assert_eq!(parse_version("v0.2.5-dirty"), None);
        assert_eq!(parse_version("v0.2.5-1-g4d22d4b"), None);
        assert_eq!(parse_version("v0.2"), None);
        assert_eq!(parse_version("unknown"), None);
    }

    #[test]
    fn newer_versions_are_detected() {
        assert!(is_newer("v0.2.6", "v0.2.5"));
        assert!(is_newer("0.3.0", "v0.2.9"));
        assert!(!is_newer("v0.2.5", "v0.2.5"));
        assert!(!is_newer("v0.2.4", "v0.2.5"));
        assert!(!is_newer("v0.2.6", "v0.2.5-dirty"));
    }

    #[test]
    fn extracts_versions_from_url_and_html() {
        assert_eq!(
            version_from_url("https://github.com/lusuee/clash-tui/releases/tag/v0.2.5"),
            Some("0.2.5".to_string())
        );
        assert_eq!(
            version_from_url("https://github.com/lusuee/clash-tui/releases/latest"),
            None
        );
        assert_eq!(
            version_from_html(r#"<a href="/lusuee/clash-tui/releases/tag/v0.3.1">"#),
            Some("0.3.1".to_string())
        );
        assert_eq!(version_from_html("go1.26.11"), None);
    }

    #[test]
    fn current_platform_has_an_asset() {
        // 仅覆盖 Windows x64 开发机；其余平台在 CI 编译时也是合法资产。
        if cfg!(target_os = "windows") {
            assert_eq!(
                asset_name().unwrap(),
                "clash-tui-windows-x64.zip".to_string()
            );
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn extracts_exe_from_release_zip() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let dir = std::env::temp_dir().join(format!("clash-tui-appupdate-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let archive = dir.join("asset.zip");
        {
            let file = std::fs::File::create(&archive).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file(
                "clash-tui-windows-x64/clash-tui.exe",
                SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(b"fake-binary").unwrap();
            zip.finish().unwrap();
        }

        let target = dir.join("clash-tui.exe");
        extract_binary(&archive, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"fake-binary");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 提权助手只做文件替换：新文件就位、旧文件留成 `.old`。
    #[test]
    fn elevated_apply_replaces_binary_and_keeps_backup() {
        let dir = std::env::temp_dir().join(format!(
            "clash-tui-elev-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("clash-tui.exe");
        let update = dir.join("clash-tui-new.exe");
        std::fs::write(&target, b"old").unwrap();
        std::fs::write(&update, b"new").unwrap();

        apply_update_elevated(&update, &target).unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert_eq!(std::fs::read(dir.join("clash-tui.exe.old")).unwrap(), b"old");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 真网络验证：GitHub `releases/latest` 重定向 → 解析 tag（直连失败会自动走本地代理）。
    #[tokio::test]
    #[ignore = "需要网络：访问 GitHub 最新 Release"]
    async fn end_to_end_check_latest() {
        let latest = check_latest(7897).await.expect("check_latest 应当成功");
        assert!(
            parse_version(&latest).is_some(),
            "解析出的版本不合法: {latest}"
        );
        println!("latest clash-tui release: v{latest}");
    }

    /// 环境变量是进程级的：这些测试串行执行，避免互相踩到注入的 API 基址。
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[cfg(target_os = "windows")]
    fn release_zip(dir: &Path, entry: &str, bytes: &[u8]) -> PathBuf {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let archive = dir.join("release.zip");
        let file = std::fs::File::create(&archive).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file(entry, SimpleFileOptions::default())
            .unwrap();
        zip.write_all(bytes).unwrap();
        zip.finish().unwrap();
        archive
    }

    fn temp_case_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "clash-tui-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 复现线上问题：安装包被第三方镜像代理后文件名带 `.zip` 后缀时，
    /// 解压仍应写出可执行文件（改名逻辑保证 .exe 后缀）。
    #[cfg(target_os = "windows")]
    #[test]
    fn mirror_proxied_asset_still_extracts_executable() {
        let dir = temp_case_dir("mirror");
        let archive = release_zip(&dir, "mirror/dav/ClashTUI-windows-x64.zip/clash-tui-cli.exe", b"mirror-image");
        let target = dir.join("out.exe");
        extract_binary(&archive, &target).expect("镜像样式条目也应能解压");
        assert_eq!(std::fs::read(&target).unwrap(), b"mirror-image");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 解压必须先落到临时文件：target 指向运行中的 exe 时直接写会失败。
    #[cfg(target_os = "windows")]
    #[test]
    fn extraction_offloads_before_replacing_running_binary() {
        let dir = temp_case_dir("offload");
        let target = dir.join("clash-tui.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &target).unwrap();
        let archive = release_zip(&dir, "clash-tui-windows-x64/clash-tui.exe", b"new-image");
        let staged = with_suffix(&target, ".new");
        extract_binary(&archive, &staged).expect("写暂存文件不受运行中 exe 影响");
        std::fs::rename(&staged, &target).expect("暂存文件可改名落地");
        assert_eq!(std::fs::read(&target).unwrap(), b"new-image");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn api_base_follows_injected_environment() {
        let _guard = env_guard();
        std::env::set_var("CLASH_TUI_GITHUB_API_BASE", "http://127.0.0.1:4567/api");
        assert_eq!(
            api_latest_url(),
            "http://127.0.0.1:4567/api/repos/lusuee/clash-tui/releases/latest"
        );
        std::env::remove_var("CLASH_TUI_GITHUB_API_BASE");
        assert_eq!(api_latest_url(), GITHUB_API_LATEST);
    }

    #[tokio::test]
    async fn unreachable_api_base_fails_without_hitting_github() {
        let _guard = env_guard();
        std::env::set_var("CLASH_TUI_GITHUB_API_BASE", "http://127.0.0.1:9");
        let result = check_latest_api(0).await;
        std::env::remove_var("CLASH_TUI_GITHUB_API_BASE");
        assert!(result.is_err(), "不可达的 API 基址应当直接失败");
    }

    #[test]
    fn staged_install_replaces_target_and_keeps_backup() {
        let dir = temp_case_dir("install");
        let target = dir.join("clash-tui.exe");
        let incoming = dir.join("incoming.exe");
        std::fs::write(&target, b"old").unwrap();
        std::fs::write(&incoming, b"new").unwrap();

        let backup = install_binary_at(&target, &incoming).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert_eq!(std::fs::read(&backup).unwrap(), b"old");
        assert!(!with_suffix(&target, ".new").exists(), "暂存文件应被清理");
        let _ = std::fs::remove_dir_all(&dir);
    }
}