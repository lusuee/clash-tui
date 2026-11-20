mod api;
mod app;
mod appupdate;
mod backup;
mod core;
mod diagnostics;
mod dnsconf;
mod envproxy;
mod installer;
mod kernelupdate;
mod overrides;
mod providers;
mod rules;
mod subscriptions;
mod sysproxy;
mod theme;
mod ui;

use api::{ClashClient, Configs, ConnectionsResponse, ProxiesResponse, VersionInfo};
use app::{ActiveTab, App};
use core::CoreManager;
use crossterm::{
    cursor::Show,
    event::{self, DisableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Write};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    Traffic(u64, u64),
    Proxies(Result<ProxiesResponse, String>),
    Configs(Result<Configs, String>),
    Version(Result<VersionInfo, String>),
    Connections(Result<ConnectionsResponse, String>),
    DelayResult { node: String, result: Result<u64, String> },
    SubUpdated {
        sub_id: String,
        result: Result<subscriptions::SubUpdateResult, String>,
    },
    SubActivated { msg: String },
    CoreActionDone(String),
    TunToggled(Result<bool, String>),
    PortChanged(Result<u16, String>),
    KernelProgress(u8),
    KernelUpdateDone(Result<kernelupdate::KernelUpdateReport, String>),
    AppUpdateChecked(Result<String, String>),
    AppUpdateProgress(u8),
    AppUpdateDone(Result<appupdate::AppUpdateReport, String>),
    /// 内核体检结果（Settings → H）
    HealthChecked(Vec<diagnostics::HealthItem>),
    /// 内核日志尾部读取结果（Settings → L）
    LogsLoaded(Result<Vec<String>, String>),
    /// 本地配置备份创建结果（Settings → B，`n`）
    BackupCreated(Result<String, String>),
    /// 本地配置恢复结果（Settings → B，Enter×2）
    BackupRestored(Result<(String, usize), String>),
    /// Providers 列表载入（Settings → V）
    ProvidersLoaded(Result<crate::providers::ProvidersSnapshot, String>),
    /// provider 更新结果（名称 + 类别）
    ProviderUpdated(Result<(String, crate::providers::ProviderKind), String>),
}

fn print_help() {
    println!("Clash TUI - Modern Cross-Platform Mihomo/Clash Terminal Client");
    println!();
    println!("Usage:");
    println!("  clash-tui [OPTIONS]                 Launch interactive TUI (or double-click clash-tui.exe)");
    println!("  clash-tui install [-a|--autostart]  Add clash-tui to User PATH (optional: enable autostart)");
    println!("  clash-tui uninstall                 Remove clash-tui from PATH and cleanup autostart");
    println!("  clash-tui autostart <on|off|status> Manage background kernel autostart on boot");
    println!();
    println!("Options:");
    println!("  -u, --url <URL>        Clash REST API URL (default: http://127.0.0.1:9090)");
    println!("      --secret <SECRET>  Clash external controller secret");
    println!("  -h, --help             Show this help information");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse arguments
    let mut api_url = "http://127.0.0.1:9090".to_string();
    let mut secret: Option<String> = None;

    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "install" => {
                resolve_working_dir();
                let autostart = args.iter().any(|a| a == "-a" || a == "--autostart");
                match installer::install(autostart) {
                    Ok(msg) => println!("✔ {}", msg),
                    Err(e) => eprintln!("✘ {}", e),
                }
                return Ok(());
            }
            "uninstall" => {
                resolve_working_dir();
                match installer::uninstall() {
                    Ok(msg) => println!("✔ {}", msg),
                    Err(e) => eprintln!("✘ {}", e),
                }
                return Ok(());
            }
            "autostart" => {
                resolve_working_dir();
                let core_mgr = CoreManager::new();
                let action = args.get(2).map(|s| s.as_str()).unwrap_or("status");
                match action {
                    "on" | "enable" | "start" => match core_mgr.enable_autostart() {
                        Ok(msg) => println!("✔ {}", msg),
                        Err(e) => eprintln!("✘ {}", e),
                    },
                    "off" | "disable" | "stop" => match core_mgr.disable_autostart() {
                        Ok(msg) => println!("✔ {}", msg),
                        Err(e) => eprintln!("✘ {}", e),
                    },
                    "status" => {
                        let enabled = core_mgr.is_autostart_enabled();
                        if enabled {
                            println!("✔ Autostart is currently [ENABLED]");
                        } else {
                            println!("○ Autostart is currently [DISABLED]");
                        }
                    }
                    _ => {
                        println!("Usage: clash-tui autostart <on|off|status>");
                    }
                }
                return Ok(());
            }
            "-h" | "--help" | "help" => {
                print_help();
                return Ok(());
            }
            _ => {}
        }
    }

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-s" | "--server" | "-u" | "--url" => {
                if i + 1 < args.len() {
                    api_url = args[i + 1].clone();
                    i += 1;
                }
            }
            "--secret" => {
                if i + 1 < args.len() {
                    secret = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    // 2. Resolve working directory (portable layout for global install)
    resolve_working_dir();

    // 3. Ensure Mihomo Core daemon is active
    let core_mgr = CoreManager::new();
    core_mgr.ensure_default_config();
    let is_running = core_mgr.is_running(&api_url, secret.as_deref()).await;
    if !is_running && core_mgr.is_installed() {
        let _ = core_mgr.start_core();
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // 3. Setup panic hook to restore terminal on panic
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture, Show);
        let _ = std::io::stdout().flush();
        default_hook(panic_info);
    }));

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 4. Channel for application events
    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();

    // Input listener task
    let tx_key = tx.clone();
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    if key.kind == KeyEventKind::Press {
                        let _ = tx_key.send(AppEvent::Key(key));
                    }
                }
            } else {
                let _ = tx_key.send(AppEvent::Tick);
            }
        }
    });

    let client = ClashClient::new(&api_url, secret.clone());

    // Background poller task for traffic and connections
    let tx_poll = tx.clone();
    let client_poll = client.clone();
    tokio::spawn(async move {
        let mut last_upload = 0u64;
        let mut last_download = 0u64;
        let mut first = true;

        loop {
            tokio::time::sleep(Duration::from_millis(1000)).await;

            // Fetch connections & compute instant bandwidth
            if let Ok(conns) = client_poll.get_connections().await {
                let up_speed = if first { 0 } else { conns.upload_total.saturating_sub(last_upload) };
                let down_speed = if first { 0 } else { conns.download_total.saturating_sub(last_download) };
                last_upload = conns.upload_total;
                last_download = conns.download_total;
                first = false;

                let _ = tx_poll.send(AppEvent::Traffic(up_speed, down_speed));
                let _ = tx_poll.send(AppEvent::Connections(Ok(conns)));
            } else {
                let _ = tx_poll.send(AppEvent::Traffic(0, 0));
            }
        }
    });

    // Background poller task for core info and proxies
    let tx_slow = tx.clone();
    let client_slow = client.clone();
    tokio::spawn(async move {
        loop {
            let ver = client_slow.get_version().await;
            let _ = tx_slow.send(AppEvent::Version(ver));

            let configs = client_slow.get_configs().await;
            let _ = tx_slow.send(AppEvent::Configs(configs));

            let proxies = client_slow.get_proxies().await;
            let _ = tx_slow.send(AppEvent::Proxies(proxies));

            tokio::time::sleep(Duration::from_millis(2500)).await;
        }
    });

    // 5. Application loop
    let mut app = App::new(&api_url, secret);

    // 内核缺失时启动即可见提示，避免只按 [S] 才发现找不到内核
    if !app.core_mgr.is_installed() {
        app.set_status(format!("✘ {}", app.core_mgr.missing_hint()));
    }

    // 上一次自更新可能留下改名后的旧二进制（Windows 下运行中无法直接删除）。
    appupdate::cleanup_stale_backup();

    // 静默检查 clash-tui 自身是否有新版本：只提示，安装由用户在 Settings 按 U 触发。
    {
        let tx_update = tx.clone();
        let mixed_port = app.mixed_port;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let result = appupdate::check_latest(mixed_port).await;
            let _ = tx_update.send(AppEvent::AppUpdateChecked(result));
        });
    }

    while !app.should_quit {
        terminal.draw(|f| ui::render(f, &mut app))?;

        if let Some(event) = rx.recv().await {
            match event {
                AppEvent::Key(key) => {
                    handle_key_event(&mut app, key, &tx).await;
                }
                AppEvent::Traffic(up, down) => {
                    app.update_traffic(up, down);
                }
                AppEvent::Proxies(Ok(resp)) => {
                    app.update_proxies(resp.proxies);
                    app.is_connected = true;
                }
                AppEvent::Proxies(Err(_)) => {
                    app.is_connected = false;
                }
                AppEvent::Configs(Ok(cfg)) => {
                    if !cfg.mode.is_empty() {
                        app.mode = cfg.mode;
                    }
                    if let Some(p) = cfg.mixed_port {
                        app.mixed_port = p;
                    }
                    if let Some(tun) = cfg.tun {
                        app.tun_enabled = tun.enable;
                    }
                }
                AppEvent::Configs(Err(_)) => {}
                AppEvent::Version(Ok(ver)) => {
                    app.core_version = ver.version;
                    app.is_core_running = true;
                }
                AppEvent::Version(Err(_)) => {
                    app.is_core_running = false;
                }
                AppEvent::Connections(Ok(conns)) => {
                    app.connections = conns.connections;
                    if app.selected_conn_idx >= app.connections.len() {
                        app.selected_conn_idx = app.connections.len().saturating_sub(1);
                    }
                    app.total_upload = conns.upload_total;
                    app.total_download = conns.download_total;
                }
                AppEvent::Connections(Err(_)) => {}
                AppEvent::DelayResult { node, result } => {
                    app.testing_nodes.remove(&node);
                    match result {
                        Ok(delay) => {
                            app.delays.insert(node.clone(), delay);
                            app.set_status(format!("✔ Ping [{}] => {} ms", node, delay));
                        }
                        Err(e) => {
                            app.set_status(format!("✘ Ping [{}] failed: {}", node, e));
                        }
                    }
                }
                AppEvent::SubUpdated { sub_id, result } => {
                    app.updating_subs.remove(&sub_id);
                    match result {
                        Ok(res) => {
                            app.sub_mgr.apply_update(&res);
                            app.set_status(format!("✔ Updated '{}' ({} nodes)", res.sub_name, res.node_count));
                            if res.is_active {
                                let client = app.client.clone();
                                let tx_refresh = tx.clone();
                                let full_path = get_clean_config_path();
                                tokio::spawn(async move {
                                    if let Ok(()) = client.reload_config(&full_path).await {
                                        tokio::time::sleep(Duration::from_millis(300)).await;
                                        if let Ok(proxies) = client.get_proxies().await {
                                            let _ = tx_refresh.send(AppEvent::Proxies(Ok(proxies)));
                                        }
                                        if let Ok(configs) = client.get_configs().await {
                                            let _ = tx_refresh.send(AppEvent::Configs(Ok(configs)));
                                        }
                                    }
                                });
                            }
                        }
                        Err(e) => {
                            app.set_status(format!("✘ {}", e));
                        }
                    }
                }
                AppEvent::SubActivated { msg } => {
                    app.set_status(msg);
                    app.selected_group_idx = 0;
                    app.selected_node_idx = 0;
                }
                AppEvent::CoreActionDone(msg) => {
                    app.set_status(msg);
                }
                AppEvent::KernelProgress(pct) => {
                    app.kernel_progress = pct.min(100);
                }
                AppEvent::KernelUpdateDone(res) => {
                    app.kernel_busy = false;
                    match res {
                        Ok(report) => {
                            app.kernel_latest = Some(report.latest.clone());
                            if report.applied {
                                app.kernel_bin_version = report.latest.clone();
                            }
                            if report.restarted {
                                app.core_version = format!("v{}", report.latest);
                            }
                            app.set_status(report.message);
                        }
                        Err(e) => {
                            app.kernel_progress = 0;
                            app.set_status(format!("✘ 内核更新失败: {e}"));
                        }
                    }
                }
                AppEvent::AppUpdateChecked(res) => {
                    match res {
                        Ok(latest) => {
                            let newer = appupdate::is_newer(&latest, appupdate::VERSION);
                            app.app_latest = Some(latest.clone());
                            if newer {
                                app.set_status(format!(
                                    "⬆ 发现 clash-tui 新版本 v{}（当前 {}），在设置页按 U 更新",
                                    latest,
                                    app.app_version
                                ));
                            }
                        }
                        // 启动时的静默检查：失败不打扰用户，按 U 时可重新检查并看到错误
                        Err(_) => {}
                    }
                }
                AppEvent::AppUpdateProgress(pct) => {
                    app.app_progress = pct.min(100);
                }
                AppEvent::AppUpdateDone(res) => {
                    app.app_busy = false;
                    match res {
                        Ok(report) => {
                            app.app_latest = Some(report.latest.clone());
                            if report.applied {
                                app.app_progress = 100;
                                app.restart_after_exit = true;
                                app.should_quit = true;
                            }
                            app.set_status(report.message);
                        }
                        Err(e) => {
                            app.app_progress = 0;
                            app.set_status(format!("✘ clash-tui 更新失败: {e}"));
                        }
                    }
                }
                AppEvent::HealthChecked(items) => {
                    let bad = items.iter().filter(|i| i.ok == Some(false)).count();
                    app.set_health_items(items);
                    if bad == 0 {
                        app.set_status("✔ 内核体检完成：未发现异常");
                    } else {
                        app.set_status(format!("✘ 内核体检完成：{} 项需要关注", bad));
                    }
                }
                AppEvent::LogsLoaded(result) => {
                    match &result {
                        Ok(lines) => app.set_status(format!(
                            "✔ 已载入内核日志 {} 行 ({} 之后的尾部)",
                            lines.len(),
                            diagnostics::LOG_PATH
                        )),
                        Err(e) => app.set_status(format!("✘ {e}")),
                    }
                    app.set_log_lines(result);
                }
                AppEvent::BackupCreated(result) => {
                    match &result {
                        Ok(name) => {
                            app.set_status(format!("✔ 已创建备份 {name}"));
                            let entries = crate::backup::list_backups(std::path::Path::new(
                                crate::backup::BACKUP_DIR,
                            ));
                            app.set_backup_entries(entries);
                        }
                        Err(e) => app.set_status(format!("✘ 创建备份失败: {e}")),
                    }
                }
                AppEvent::BackupRestored(result) => match result {
                    Ok((name, restored)) => {
                        app.reload_managers_from_disk();
                        let (_, target) = rules::apply_rules_to_config();
                        app.rule_mgr.target_hint = target;
                        app.set_status(format!(
                            "✔ 已恢复备份 {name}（{restored} 个文件），热重载中..."
                        ));
                        app.close_backup_modal();
                        let client = app.client.clone();
                        let full_path = get_clean_config_path();
                        tokio::spawn(async move {
                            let _ = client.reload_config(&full_path).await;
                        });
                    }
                    Err(e) => app.set_status(format!("✘ 恢复备份失败: {e}")),
                },
                AppEvent::ProvidersLoaded(result) => match result {
                    Ok(snapshot) => {
                        let total = snapshot.total();
                        app.set_providers_snapshot(snapshot);
                        app.providers_modal_set_message(None);
                        if total == 0 {
                            app.set_status("✔ providers 已载入：当前配置未声明 proxy/rule providers");
                        } else {
                            app.set_status(format!("✔ providers 已载入（共 {total} 个）"));
                        }
                    }
                    Err(e) => {
                        app.providers_modal_set_message(Some(format!("载入失败: {e}")));
                        app.set_status(format!("✘ 载入 providers 失败: {e}"));
                    }
                },
                AppEvent::ProviderUpdated(result) => match result {
                    Ok((name, kind)) => {
                        app.set_status(format!(
                            "✔ 已请求更新 {} provider {name}，正在重新载入...",
                            kind.label()
                        ));
                        spawn_providers_load(&mut app, &tx);
                    }
                    Err(e) => app.set_status(format!("✘ 更新 provider 失败: {e}")),
                },
                AppEvent::TunToggled(res) => {
                    match res {
                        Ok(active) => {
                            app.tun_enabled = active;
                            if active {
                                app.set_status("✔ TUN 虚拟网卡模式已启动");
                            } else {
                                app.set_status("✔ TUN 虚拟网卡模式已关闭");
                            }
                        }
                        Err(e) => {
                            app.set_status(format!("✘ TUN 切换失败: {}", e));
                        }
                    }
                }
                AppEvent::PortChanged(res) => {
                    match res {
                        Ok(new_port) => {
                            app.mixed_port = new_port;
                            app.set_status(format!("✔ 代理端口已更新为 [{}]", new_port));
                            if app.sys_proxy_enabled {
                                let _ = sysproxy::SysProxy::set_proxy(true, &format!("127.0.0.1:{}", new_port));
                            }
                            if app.env_proxy_enabled {
                                let _ = envproxy::EnvProxy::set_env_proxy(true, new_port);
                            }
                        }
                        Err(e) => {
                            app.set_status(format!("✘ 端口修改失败: {}", e));
                        }
                    }
                }
                AppEvent::Tick => {
                    // 订阅页的自动更新：每 60 秒检查一次，仅刷新已到期的订阅
                    if app.sub_mgr.auto_update_minutes > 0
                        && app.last_sub_auto_check.elapsed() >= Duration::from_secs(60)
                    {
                        app.last_sub_auto_check = Instant::now();
                        let due: Vec<_> = app
                            .sub_mgr
                            .due_subscriptions()
                            .into_iter()
                            .filter(|sub| !app.updating_subs.contains(&sub.id))
                            .collect();
                        if !due.is_empty() {
                            app.set_status(format!(
                                "自动更新 {} 个到期订阅 (间隔 {})...",
                                due.len(),
                                subscriptions::auto_update_label(app.sub_mgr.auto_update_minutes)
                            ));
                            for sub in due {
                                spawn_sub_download(&mut app, &tx, sub);
                            }
                        }
                    }
                }
            }
        }
    }

    // 6. Cleanup terminal on exit
    drop(terminal);
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, Show);
    let _ = io::stdout().flush();

    if app.restart_after_exit {
        // 自动更新已替换 exe：用同一套命令行参数与工作目录拉起新版本。
        match appupdate::restart_into_new_binary() {
            Ok(()) => println!("Clash TUI 已更新，正在以新版本重新启动..."),
            Err(e) => println!("✘ {e}"),
        }
        std::process::exit(0);
    }

    if app.core_stopped_on_exit {
        println!("Clash TUI closed. Mihomo core has been stopped.");
    } else {
        println!("Clash TUI closed. Mihomo core continues running in the background.");
    }
    std::process::exit(0);
}

/// 后台拉取订阅并回报 `SubUpdated`，供手动/自动更新共用
fn spawn_sub_download(
    app: &mut App,
    tx: &mpsc::UnboundedSender<AppEvent>,
    sub: subscriptions::SubscriptionItem,
) {
    let sub_id = sub.id.clone();
    let sub_name = sub.name.clone();
    let sub_url = sub.url.clone();
    let is_active = sub.active;
    let mixed_port = app.mixed_port;
    let profiles_dir = app.sub_mgr.profiles_dir.clone();
    app.updating_subs.insert(sub_id.clone());

    let tx_sub = tx.clone();
    tokio::spawn(async move {
        let result = subscriptions::SubscriptionManager::download_and_update(
            sub_id.clone(),
            sub_name,
            sub_url,
            is_active,
            mixed_port,
            profiles_dir,
        )
        .await;
        let _ = tx_sub.send(AppEvent::SubUpdated { sub_id, result });
    });
}

/// 上移/下移选中的自定义规则，随后重注入并热重载
fn move_selected_rule(app: &mut App, up: bool) {
    let idx = app.selected_rule_idx;
    let Some(new_idx) = (if up {
        app.rule_mgr.move_up(idx)
    } else {
        app.rule_mgr.move_down(idx)
    }) else {
        let edge = if up { "已是最高优先级" } else { "已是最低优先级" };
        app.set_status(format!("✘ {edge}"));
        return;
    };
    app.selected_rule_idx = new_idx;
    let (count, target) = rules::apply_rules_to_config();
    app.rule_mgr.target_hint = target;
    app.set_status(format!(
        "✔ 规则已{}至第 {} 位 (共 {} 条，热重载中...)",
        if up { "上移" } else { "下移" },
        new_idx + 1,
        count
    ));
    let client = app.client.clone();
    let full_path = get_clean_config_path();
    tokio::spawn(async move {
        let _ = client.reload_config(&full_path).await;
    });
}

/// 打开体检弹窗并异步收集检查项
fn spawn_health_check(app: &mut App, tx: &mpsc::UnboundedSender<AppEvent>) {
    app.open_health_modal();
    let api_url = app.api_url.clone();
    let secret = app.core_secret.clone();
    let mixed_port = app.mixed_port;
    let tx_health = tx.clone();
    tokio::spawn(async move {
        let core = CoreManager::new();
        let items = diagnostics::collect_health(&core, &api_url, secret, mixed_port).await;
        let _ = tx_health.send(AppEvent::HealthChecked(items));
    });
}

/// 重新读取内核日志尾部（不重置弹窗里的过滤关键字）
fn spawn_logs_reload(tx: &mpsc::UnboundedSender<AppEvent>) {
    let tx_logs = tx.clone();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(|| diagnostics::read_log_tail(400))
            .await
            .unwrap_or_else(|e| Err(format!("读取日志失败：{}", e)));
        let _ = tx_logs.send(AppEvent::LogsLoaded(result));
    });
}

/// 后台创建本地配置备份
fn spawn_backup_create(tx: &mpsc::UnboundedSender<AppEvent>) {
    let tx_b = tx.clone();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(|| {
            crate::backup::create_backup(std::path::Path::new("."))
        })
        .await
        .unwrap_or_else(|e| Err(format!("备份任务失败：{}", e)));
        let _ = tx_b.send(AppEvent::BackupCreated(result));
    });
}

/// 后台恢复指定备份（会覆盖当前配置）
fn spawn_backup_restore(tx: &mpsc::UnboundedSender<AppEvent>, name: String) {
    let tx_b = tx.clone();
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            let restored = crate::backup::restore_backup(std::path::Path::new("."), &name)?;
            Ok((name.clone(), restored))
        })
        .await
        .unwrap_or_else(|e| Err(format!("恢复任务失败：{}", e)));
        let _ = tx_b.send(AppEvent::BackupRestored(result));
    });
}

/// 读取配置里的 provider 声明 + REST 数据，载入弹窗列表
fn spawn_providers_load(app: &mut App, tx: &mpsc::UnboundedSender<AppEvent>) {
    let client = app.client.clone();
    let tx_p = tx.clone();
    tokio::spawn(async move {
        let proxies = client.get_proxy_providers().await;
        let rules = client.get_rule_providers().await;
        let result = match (proxies, rules) {
            (Ok(p), Ok(r)) => {
                let config = tokio::task::spawn_blocking(|| {
                    std::fs::read_to_string(crate::providers::CONFIG_PATH).unwrap_or_default()
                })
                .await
                .unwrap_or_default();
                Ok(crate::providers::parse_snapshot(&config, &p, &r))
            }
            (Err(e), _) | (_, Err(e)) => Err(e),
        };
        let _ = tx_p.send(AppEvent::ProvidersLoaded(result));
    });
}

/// 让内核重新拉取指定 provider
fn spawn_provider_update(
    client: ClashClient,
    tx: &mpsc::UnboundedSender<AppEvent>,
    kind: crate::providers::ProviderKind,
    name: String,
) {
    let tx_p = tx.clone();
    tokio::spawn(async move {
        let res = match kind {
            crate::providers::ProviderKind::Proxy => client.update_proxy_provider(&name).await,
            crate::providers::ProviderKind::Rule => client.update_rule_provider(&name).await,
        };
        let payload = res.map(|_| (name, kind));
        let _ = tx_p.send(AppEvent::ProviderUpdated(payload));
    });
}

async fn handle_key_event(app: &mut App, key: KeyEvent, tx: &mpsc::UnboundedSender<AppEvent>) {
    if key.kind != KeyEventKind::Press {
        return;
    }

    // 快捷键帮助浮层：只响应关闭键，避免误触后台操作
    if app.show_help {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                app.show_help = false;
            }
            _ => {}
        }
        return;
    }

    // 流量历史弹窗（Settings → Y）
    if app.show_traffic_modal {
        match key.code {
            KeyCode::Esc => app.close_traffic_modal(),
            KeyCode::Char('c') => {
                app.clear_traffic_history();
                app.set_status("✔ 流量历史已清空");
            }
            _ => {}
        }
        return;
    }

    // Providers 弹窗（Settings → V）
    if app.show_providers_modal.is_some() {
        match key.code {
            KeyCode::Esc => app.close_providers_modal(),
            KeyCode::Tab => app.providers_modal_toggle_kind(),
            KeyCode::Char('j') | KeyCode::Down => app.providers_modal_next(),
            KeyCode::Char('k') | KeyCode::Up => app.providers_modal_prev(),
            KeyCode::Char('r') => {
                app.providers_modal_set_message(Some("重新载入中...".to_string()));
                spawn_providers_load(app, tx);
            }
            KeyCode::Char('u') => {
                if let Some((name, kind)) = app.providers_modal_selected() {
                    app.providers_modal_set_message(Some(format!("正在更新 {name}...")));
                    spawn_provider_update(app.client.clone(), tx, kind, name);
                } else {
                    app.set_status("没有可更新的 provider");
                }
            }
            _ => {}
        }
        return;
    }

    // 本地配置备份弹窗（Settings → B）
    if app.show_backup_modal.is_some() {
        match key.code {
            KeyCode::Esc => {
                if app.backup_modal_pending().is_some() {
                    app.backup_modal_clear_pending();
                    app.set_status("已取消恢复");
                } else {
                    app.close_backup_modal();
                }
            }
            KeyCode::Char('j') | KeyCode::Down => app.backup_modal_next(),
            KeyCode::Char('k') | KeyCode::Up => app.backup_modal_prev(),
            KeyCode::Char('n') => {
                app.set_status("正在创建本地配置备份...");
                spawn_backup_create(tx);
            }
            KeyCode::Enter => {
                if let Some(name) = app.backup_modal_pending() {
                    app.backup_modal_clear_pending();
                    app.set_status(format!("正在恢复备份 {name}..."));
                    spawn_backup_restore(tx, name);
                } else if let Some(name) = app.backup_modal_selected_name() {
                    app.backup_modal_set_pending(name.clone());
                    app.set_status(format!(
                        "⚠ 将用备份 {name} 覆盖当前配置 / DNS / 覆盖 / 规则，再按 Enter 确认，Esc 取消"
                    ));
                } else {
                    app.set_status("没有可恢复的备份，先按 n 创建");
                }
            }
            _ => {}
        }
        return;
    }

    // 运行配置覆盖弹窗（Settings → C）
    if app.show_overrides_modal.is_some() {
        match key.code {
            KeyCode::Esc => app.close_overrides_modal(),
            KeyCode::Tab | KeyCode::Down => app.overrides_modal_next_field(),
            KeyCode::BackTab | KeyCode::Up => app.overrides_modal_prev_field(),
            KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
                app.overrides_modal_cycle_value()
            }
            KeyCode::Enter => match app.save_overrides_from_modal() {
                Ok(settings) => {
                    let (_, target) = rules::apply_rules_to_config();
                    app.rule_mgr.target_hint = target;
                    app.set_status(format!(
                        "✔ 运行时配置已保存并注入: {} (热重载中...)",
                        settings.summary()
                    ));
                    let client = app.client.clone();
                    let full_path = get_clean_config_path();
                    tokio::spawn(async move {
                        let _ = client.reload_config(&full_path).await;
                    });
                }
                Err(e) => app.set_status(format!("✘ {e}")),
            },
            _ => {}
        }
        return;
    }

    // 体检 / 日志弹窗（Settings → H / L）
    if app.show_health_modal.is_some() {
        match key.code {
            KeyCode::Esc => app.close_health_modal(),
            KeyCode::Char('H') | KeyCode::Char('r') => spawn_health_check(app, tx),
            _ => {}
        }
        return;
    }
    if app.show_logs_modal.is_some() {
        match key.code {
            KeyCode::Esc => app.close_logs_modal(),
            KeyCode::Enter => {
                if let Some(modal) = &mut app.show_logs_modal {
                    modal.loading = true;
                    modal.error = None;
                }
                spawn_logs_reload(tx);
            }
            KeyCode::Backspace => app.log_modal_backspace(),
            KeyCode::Char(c) => app.log_modal_handle_char(c),
            _ => {}
        }
        return;
    }

    // If Modal is open, redirect keystrokes to modal form
    if app.show_edit_port_modal.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.close_edit_port_modal();
            }
            KeyCode::Backspace => {
                app.edit_port_backspace();
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                app.edit_port_handle_char(c);
            }
            KeyCode::Enter => {
                if let Some(port_str) = app.show_edit_port_modal.take() {
                    if let Ok(new_port) = port_str.trim().parse::<u16>() {
                        if (1..=65535).contains(&new_port) {
                            app.set_status(format!("Updating mixed-port to {}...", new_port));
                            let client = app.client.clone();
                            let tx_p = tx.clone();
                            tokio::spawn(async move {
                                match client.set_mixed_port(new_port).await {
                                    Ok(()) => {
                                        let _ = tx_p.send(AppEvent::PortChanged(Ok(new_port)));
                                    }
                                    Err(e) => {
                                        let _ = tx_p.send(AppEvent::PortChanged(Err(e)));
                                    }
                                }
                            });
                        } else {
                            app.set_status("✘ 端口范围必须在 1 ~ 65535 之间");
                        }
                    } else {
                        app.set_status("✘ 请输入有效的数字端口");
                    }
                }
            }
            _ => {}
        }
        return;
    }

    if app.show_add_sub_modal.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.close_modal();
            }
            KeyCode::Tab => {
                app.modal_toggle_field();
            }
            KeyCode::Backspace => {
                app.modal_backspace();
            }
            KeyCode::Char(c) => {
                app.modal_handle_char(c);
            }
            KeyCode::Enter => {
                if let Some(modal) = app.show_add_sub_modal.take() {
                    let name = modal.name_input.trim().to_string();
                    let url = modal.url_input.trim().to_string();
                    if !name.is_empty() && !url.is_empty() {
                        if let Some(id) = modal.editing_id {
                            let old_url = app
                                .sub_mgr
                                .subscriptions
                                .iter()
                                .find(|s| s.id == id)
                                .map(|s| s.url.clone())
                                .unwrap_or_default();
                            if let Some(updated) = app.sub_mgr.update_meta(&id, name.clone(), url) {
                                if updated.url != old_url {
                                    app.set_status(format!(
                                        "✔ 已更新订阅 '{}'，URL 变化，重新下载...",
                                        name
                                    ));
                                    spawn_sub_download(app, tx, updated);
                                } else {
                                    app.set_status(format!("✔ 已更新订阅 '{}'", name));
                                }
                            }
                        } else {
                            let sub = app.sub_mgr.add(name.clone(), url);
                            app.set_status(format!("✔ Added subscription '{}'. Fetching...", name));
                            spawn_sub_download(app, tx, sub);
                        }
                    }
                }
            }
            _ => {}
        }
        return;
    }

    // Add Domain Rule modal
    if app.show_add_rule_modal.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.close_modal();
            }
            KeyCode::Tab | KeyCode::Left | KeyCode::Right => {
                app.rule_modal_cycle_destination();
            }
            KeyCode::Backspace => {
                app.rule_modal_backspace();
            }
            KeyCode::Char(c) => {
                app.rule_modal_handle_char(c);
            }
            KeyCode::Enter => {
                if let Some(modal) = app.show_add_rule_modal.take() {
                    let text = modal.input.trim().to_string();
                    if text.is_empty() {
                        app.set_status("✘ 域名不能为空");
                    } else {
                        match app.rule_mgr.add(&text, modal.target, modal.group.as_deref()) {
                            Some(item) => {
                                let (count, target) = rules::apply_rules_to_config();
                                app.rule_mgr.target_hint = target;
                                app.rule_mgr.groups = rules::config_proxy_groups();
                                let destination = match &item.group {
                                    Some(g) if item.target == crate::rules::RuleTarget::Proxy => g.clone(),
                                    _ => item.target.as_str().to_string(),
                                };
                                app.set_status(format!(
                                    "✔ 已添加规则 {} {} → {} (共 {} 条，热重载中...)",
                                    item.rule_type, item.value, destination, count
                                ));
                                let client = app.client.clone();
                                let full_path = get_clean_config_path();
                                tokio::spawn(async move {
                                    let _ = client.reload_config(&full_path).await;
                                });
                            }
                            None => {
                                app.set_status("✘ 无效域名或规则已存在");
                                // 保留弹窗继续编辑
                                app.show_add_rule_modal = Some(modal);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        return;
    }

    // Rule probe (命中推演) modal
    if app.show_rule_test_modal.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.close_rule_test_modal();
            }
            KeyCode::Enter => {
                app.rule_test_run();
            }
            KeyCode::Backspace => {
                app.rule_test_backspace();
            }
            KeyCode::Char(c) => {
                app.rule_test_handle_char(c);
            }
            _ => {}
        }
        return;
    }

    // Settings → DNS override modal
    if app.show_dns_modal.is_some() {
        match key.code {
            KeyCode::Esc => {
                app.close_dns_modal();
            }
            KeyCode::Tab | KeyCode::Down => {
                app.dns_modal_next_field();
            }
            KeyCode::BackTab | KeyCode::Up => {
                app.dns_modal_prev_field();
            }
            KeyCode::Char(' ') => {
                app.dns_modal_toggle();
            }
            KeyCode::Backspace => {
                app.dns_modal_backspace();
            }
            KeyCode::Char(c) => {
                app.dns_modal_handle_char(c);
            }
            KeyCode::Enter => match app.dns_modal_to_settings() {
                Ok(settings) => {
                    app.dns_mgr.settings = settings.clone();
                    app.dns_mgr.save();
                    let (count, target) = rules::apply_rules_to_config();
                    app.rule_mgr.target_hint = target;
                    app.show_dns_modal = None;
                    app.set_status(format!(
                        "✔ DNS 配置已保存并注入 ({} 条策略 / {} 条规则，热重载中...)",
                        settings.policies.len(),
                        count
                    ));
                    let client = app.client.clone();
                    let full_path = get_clean_config_path();
                    tokio::spawn(async move {
                        let _ = client.reload_config(&full_path).await;
                    });
                }
                Err(e) => {
                    app.set_status(format!("✘ {}", e));
                }
            },
            _ => {}
        }
        return;
    }

    // Normal navigation keystrokes
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.should_quit = true;
        }
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Char('?') => {
            app.show_help = true;
        }
        KeyCode::Tab => {
            app.next_tab();
        }
        KeyCode::BackTab => {
            app.prev_tab();
        }
        KeyCode::Char('1') => {
            app.set_tab(ActiveTab::Proxies);
        }
        KeyCode::Char('2') => {
            app.set_tab(ActiveTab::Connections);
        }
        KeyCode::Char('3') => {
            app.set_tab(ActiveTab::Subscriptions);
        }
        KeyCode::Char('4') => {
            app.set_tab(ActiveTab::Settings);
        }
        KeyCode::Char('5') => {
            app.set_tab(ActiveTab::Rules);
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.on_up();
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.on_down();
        }
        KeyCode::PageUp => {
            app.on_page_up();
        }
        KeyCode::PageDown => {
            app.on_page_down();
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.on_home();
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.on_end();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            app.on_left();
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.on_right();
        }
        KeyCode::Char('m') => {
            if let Some(next_mode) = app.cycle_mode() {
                let client = app.client.clone();
                let mode_to_set = next_mode.clone();
                tokio::spawn(async move {
                    let _ = client.set_mode(&mode_to_set).await;
                });
                app.mode = next_mode.clone();
                app.set_status(format!("✔ Switched mode to [{}]", next_mode));
            }
        }
        KeyCode::Char('p') => {
            app.toggle_sys_proxy();
        }
        KeyCode::Char('e') => {
            if app.active_tab == ActiveTab::Subscriptions {
                app.open_edit_sub_modal();
            } else {
                app.toggle_env_proxy();
            }
        }
        KeyCode::Char('n') => {
            let target = !app.tun_enabled;
            app.set_status(if target { "Enabling TUN mode..." } else { "Disabling TUN mode..." });
            let client = app.client.clone();
            let tx_tun = tx.clone();
            tokio::spawn(async move {
                match client.set_tun(target).await {
                    Ok(()) => {
                        tokio::time::sleep(Duration::from_millis(300)).await;
                        if let Ok(cfg) = client.get_configs().await {
                            let actual = cfg.tun.map(|t| t.enable).unwrap_or(false);
                            if target && !actual {
                                let _ = tx_tun.send(AppEvent::TunToggled(Err("缺少管理员权限 (可在设置按 A 提权)".to_string())));
                            } else {
                                let _ = tx_tun.send(AppEvent::TunToggled(Ok(actual)));
                            }
                        } else {
                            let _ = tx_tun.send(AppEvent::TunToggled(Ok(target)));
                        }
                    }
                    Err(e) => {
                        let _ = tx_tun.send(AppEvent::TunToggled(Err(e)));
                    }
                }
            });
        }
        KeyCode::Char('P') => {
            app.open_edit_port_modal();
        }
        KeyCode::Char('o') => {
            if app.active_tab == ActiveTab::Proxies {
                app.cycle_node_sort();
                app.set_status(format!("Node sort: [{}]", app.node_sort.label()));
            }
        }
        KeyCode::Char('Q') => {
            // 退出并停止内核（q / Ctrl+C 仅退出 TUI，内核常驻）
            let core_mgr = CoreManager::new();
            match core_mgr.stop_core() {
                Ok(msg) => {
                    app.core_stopped_on_exit = true;
                    app.set_status(format!("✔ {} | Exiting...", msg));
                }
                Err(e) => {
                    app.set_status(format!("✘ {} | Exiting anyway...", e));
                }
            }
            app.should_quit = true;
        }
        KeyCode::Enter => {
            match app.active_tab {
                ActiveTab::Proxies => {
                    if let Some(group) = app.current_group_name() {
                        if let Some(node) = app.current_selected_node_name() {
                            let client = app.client.clone();
                            let g_name = group.to_string();
                            let n_name = node.clone();
                            tokio::spawn(async move {
                                let _ = client.select_proxy(&g_name, &n_name).await;
                            });
                            app.set_status(format!("✔ Switched [{}] to node [{}]", group, node));
                        }
                    }
                }
                ActiveTab::Subscriptions => {
                    let idx = app.selected_sub_idx;
                    match app.sub_mgr.set_active(idx, app.mixed_port) {
                        Ok(sub) => {
                            app.set_status(format!("Activating profile '{}' & reloading core...", sub.name));
                            let client = app.client.clone();
                            let tx_sub = tx.clone();
                            let full_path = get_clean_config_path();
                            let sub_name = sub.name.clone();

                            tokio::spawn(async move {
                                match client.reload_config(&full_path).await {
                                    Ok(()) => {
                                        tokio::time::sleep(Duration::from_millis(500)).await;
                                        if let Ok(proxies) = client.get_proxies().await {
                                            let _ = tx_sub.send(AppEvent::Proxies(Ok(proxies)));
                                            let _ = tx_sub.send(AppEvent::SubActivated {
                                                msg: format!("✔ Activated profile '{}' & refreshed nodes", sub_name),
                                            });
                                        } else {
                                            let _ = tx_sub.send(AppEvent::SubActivated {
                                                msg: format!("✔ Activated profile '{}' (reloaded)", sub_name),
                                            });
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx_sub.send(AppEvent::SubActivated {
                                            msg: format!("✘ Core reload failed: {}", e),
                                        });
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            app.set_status(format!("✘ {}", e));
                        }
                    }
                }
                _ => {}
            }
        }
        KeyCode::Char('t') => {
            if app.active_tab == ActiveTab::Proxies {
                if let Some(node) = app.current_selected_node_name() {
                    app.testing_nodes.insert(node.clone());
                    let client = app.client.clone();
                    let tx_delay = tx.clone();
                    let test_url = app.test_url.clone();
                    let node_clone = node.clone();
                    tokio::spawn(async move {
                        let result = client.test_delay(&node_clone, &test_url, 4000).await;
                        let _ = tx_delay.send(AppEvent::DelayResult {
                            node: node_clone,
                            result,
                        });
                    });
                    app.set_status(format!("Testing ping for [{}]...", node));
                }
            } else if app.active_tab == ActiveTab::Rules && !app.rule_mgr.rules.is_empty() {
                let idx = app.selected_rule_idx;
                if let Some(item) = app.rule_mgr.cycle_destination(idx) {
                    let (count, target) = rules::apply_rules_to_config();
                    app.rule_mgr.target_hint = target;
                    let destination = match &item.group {
                        Some(g) if item.target == crate::rules::RuleTarget::Proxy => g.clone(),
                        _ => item.target.as_str().to_string(),
                    };
                    app.set_status(format!(
                        "✔ 规则 {} {} 已切换为 {} (共 {} 条，热重载中...)",
                        item.rule_type, item.value, destination, count
                    ));
                    let client = app.client.clone();
                    let full_path = get_clean_config_path();
                    tokio::spawn(async move {
                        let _ = client.reload_config(&full_path).await;
                    });
                }
            }
        }
        KeyCode::Char('T') => {
            if app.active_tab == ActiveTab::Proxies {
                let nodes = app.current_group_nodes();
                for node in nodes {
                    app.testing_nodes.insert(node.clone());
                    let client = app.client.clone();
                    let tx_delay = tx.clone();
                    let test_url = app.test_url.clone();
                    tokio::spawn(async move {
                        let result = client.test_delay(&node, &test_url, 4000).await;
                        let _ = tx_delay.send(AppEvent::DelayResult { node, result });
                    });
                }
                app.set_status("Testing latency for all nodes in group...");
            }
        }
        KeyCode::Char('a') => {
            if app.active_tab == ActiveTab::Subscriptions {
                app.open_add_sub_modal();
            } else if app.active_tab == ActiveTab::Rules {
                app.open_add_rule_modal();
            }
        }
        KeyCode::Char('i') => {
            if app.active_tab == ActiveTab::Subscriptions {
                let minutes = app.sub_mgr.cycle_auto_update();
                app.set_status(format!(
                    "✔ 订阅自动更新间隔已设为 [{}]",
                    subscriptions::auto_update_label(minutes)
                ));
            }
        }
        KeyCode::Char('u') => {
            if app.active_tab == ActiveTab::Subscriptions && !app.sub_mgr.subscriptions.is_empty() {
                let sub = app.sub_mgr.subscriptions[app.selected_sub_idx].clone();
                app.set_status(format!("Updating subscription '{}' in background...", sub.name));
                spawn_sub_download(app, tx, sub);
            }
        }
        KeyCode::Char('U') => {
            if app.active_tab == ActiveTab::Subscriptions && !app.sub_mgr.subscriptions.is_empty() {
                let subs = app.sub_mgr.subscriptions.clone();
                app.set_status(format!("Updating all {} subscriptions...", subs.len()));
                for sub in subs {
                    spawn_sub_download(app, tx, sub);
                }
            } else if app.active_tab == ActiveTab::Settings {
                if app.app_busy {
                    app.set_status("clash-tui 更新正在进行中...");
                } else {
                    app.app_busy = true;
                    app.app_progress = 0;
                    app.set_status("检查 clash-tui 最新版本...");
                    let mixed_port = app.mixed_port;
                    let tx_update = tx.clone();
                    tokio::spawn(async move {
                        let tx_progress = tx_update.clone();
                        let result = appupdate::update_app(mixed_port, move |pct| {
                            let _ = tx_progress.send(AppEvent::AppUpdateProgress(pct));
                        })
                        .await;
                        let _ = tx_update.send(AppEvent::AppUpdateDone(result));
                    });
                }
            }
        }
        KeyCode::Char('x') => {
            if app.active_tab == ActiveTab::Subscriptions && !app.sub_mgr.subscriptions.is_empty() {
                let idx = app.selected_sub_idx;
                if let Some(removed) = app.sub_mgr.delete(idx) {
                    if app.selected_sub_idx >= app.sub_mgr.subscriptions.len() {
                        app.selected_sub_idx = app.sub_mgr.subscriptions.len().saturating_sub(1);
                    }
                    app.set_status(format!("✔ Deleted subscription '{}'", removed.name));
                }
            } else if app.active_tab == ActiveTab::Rules && !app.rule_mgr.rules.is_empty() {
                let idx = app.selected_rule_idx;
                if let Some(removed) = app.rule_mgr.remove(idx) {
                    if app.selected_rule_idx >= app.rule_mgr.rules.len() {
                        app.selected_rule_idx = app.rule_mgr.rules.len().saturating_sub(1);
                    }
                    let (count, target) = rules::apply_rules_to_config();
                    app.rule_mgr.target_hint = target;
                    app.set_status(format!(
                        "✔ 已删除规则 {} {} (剩余 {} 条，热重载中...)",
                        removed.rule_type, removed.value, count
                    ));
                    let client = app.client.clone();
                    let full_path = get_clean_config_path();
                    tokio::spawn(async move {
                        let _ = client.reload_config(&full_path).await;
                    });
                }
            }
        }
        KeyCode::Char('d') => {
            if app.active_tab == ActiveTab::Settings {
                app.open_dns_modal();
            } else if app.active_tab == ActiveTab::Connections && !app.connections.is_empty() {
                if let Some(conn) = app.connections.get(app.selected_conn_idx) {
                    let client = app.client.clone();
                    let id = conn.id.clone();
                    tokio::spawn(async move {
                        let _ = client.close_connection(&id).await;
                    });
                    app.set_status("Closed selected connection");
                }
            }
        }
        KeyCode::Char('D') => {
            if app.active_tab == ActiveTab::Connections {
                let client = app.client.clone();
                tokio::spawn(async move {
                    let _ = client.close_all_connections().await;
                });
                app.set_status("Closed all active connections");
            }
        }
        KeyCode::Char('H') => {
            if app.active_tab == ActiveTab::Settings {
                spawn_health_check(app, tx);
            }
        }
        KeyCode::Char('B') => {
            if app.active_tab == ActiveTab::Settings {
                app.open_backup_modal();
                app.set_status("按 n 新建备份，Enter 恢复选中备份");
            }
        }
        KeyCode::Char('V') => {
            if app.active_tab == ActiveTab::Settings {
                app.open_providers_modal();
                app.set_status("载入 providers...");
                spawn_providers_load(app, tx);
            }
        }
        KeyCode::Char('Y') => {
            if app.active_tab == ActiveTab::Settings {
                app.open_traffic_modal();
            }
        }
        KeyCode::Char('C') => {
            if app.active_tab == ActiveTab::Settings {
                app.open_overrides_modal();
            }
        }
        KeyCode::Char('L') => {
            if app.active_tab == ActiveTab::Settings {
                app.open_logs_modal();
                spawn_logs_reload(tx);
            }
        }
        KeyCode::Char('s') => {
            if app.active_tab == ActiveTab::Settings {
                let core_mgr = CoreManager::new();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = match core_mgr.stop_core() {
                        Ok(msg) => format!("✔ {}", msg),
                        Err(e) => format!("✘ {}", e),
                    };
                    let _ = tx_c.send(AppEvent::CoreActionDone(res));
                });
            }
        }
        KeyCode::Char('S') => {
            if app.active_tab == ActiveTab::Settings {
                let core_mgr = CoreManager::new();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let _ = core_mgr.stop_core();
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    let res = match core_mgr.start_core() {
                        Ok(msg) => format!("✔ {}", msg),
                        Err(e) => format!("✘ {}", e),
                    };
                    let _ = tx_c.send(AppEvent::CoreActionDone(res));
                });
            }
        }
        KeyCode::Char('A') => {
            if app.active_tab == ActiveTab::Settings {
                let core_mgr = CoreManager::new();
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = match core_mgr.start_core_as_admin() {
                        Ok(msg) => format!("✔ {}", msg),
                        Err(e) => format!("✘ {}", e),
                    };
                    let _ = tx_c.send(AppEvent::CoreActionDone(res));
                });
            }
        }
        KeyCode::Char('K') => {
            if app.active_tab == ActiveTab::Settings {
                if app.kernel_busy {
                    app.set_status("内核更新正在进行中...");
                } else {
                    app.kernel_busy = true;
                    app.kernel_progress = 0;
                    app.set_status("检查 Mihomo 内核最新版本...");
                    let core_mgr = CoreManager::new();
                    let api_url = app.api_url.clone();
                    let secret = app.core_secret.clone();
                    let ui_says_running = app.is_core_running;
                    let mixed_port = app.mixed_port;
                    let tx_k = tx.clone();
                    tokio::spawn(async move {
                        // REST 可能滞后（刚启动、secret 错误）：以实时探测为准，避免停掉内核后忘了重启
                        let was_running = core_mgr.is_running(&api_url, secret.as_deref()).await
                            || ui_says_running;
                        let tx_progress = tx_k.clone();
                        let result = kernelupdate::update_kernel(
                            &core_mgr,
                            mixed_port,
                            was_running,
                            move |pct| {
                                let _ = tx_progress.send(AppEvent::KernelProgress(pct));
                            },
                        )
                        .await;
                        let _ = tx_k.send(AppEvent::KernelUpdateDone(result));
                    });
                }
            } else if app.active_tab == ActiveTab::Rules {
                move_selected_rule(app, true);
            }
        }
        KeyCode::Char('J') => {
            if app.active_tab == ActiveTab::Rules {
                move_selected_rule(app, false);
            }
        }
        KeyCode::Char('f') => {
            if app.active_tab == ActiveTab::Rules {
                app.open_rule_test_modal();
            }
        }
        KeyCode::Char('r') => {
            app.sub_mgr.load();
            let client = app.client.clone();
            let tx_sub = tx.clone();
            let full_path = get_clean_config_path();
            tokio::spawn(async move {
                let _ = client.reload_config(&full_path).await;
                tokio::time::sleep(Duration::from_millis(500)).await;
                if let Ok(proxies) = client.get_proxies().await {
                    let _ = tx_sub.send(AppEvent::Proxies(Ok(proxies)));
                }
            });
            app.set_status("Reloaded local subscriptions and refreshed data");
        }
        _ => {}
    }
}

fn get_clean_config_path() -> String {
    let config_path = std::path::Path::new("data").join("config.yaml");
    if let Ok(abs) = std::fs::canonicalize(&config_path) {
        let s = abs.to_string_lossy().to_string();
        s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join("data").join("config.yaml").to_string_lossy().to_string()
    } else {
        config_path.to_string_lossy().to_string()
    }
}

/// 判断目录是否为一个含内核的便携布局根目录。
/// 只认内核二进制：`data/config.yaml`、`rules.json` 等是该程序自己生成的产物，
/// 若把它们当作“这是数据目录”的证据，任何被误写入一次的空目录都会永久劫持工作目录
/// （表现就是在该目录里启动时永远提示找不到 mihomo）。
fn dir_has_kernel(dir: &std::path::Path) -> bool {
    const NAMES: [&str; 2] = ["mihomo", "mihomo.exe"];
    NAMES.iter().any(|n| dir.join(n).is_file())
        || NAMES.iter().any(|n| dir.join("bin").join(n).is_file())
        || NAMES
            .iter()
            .any(|n| dir.join("bin").join("mihomo").join(n).is_file())
}

/// 判断目录是否已经存在 clash-tui 的数据产物（不含内核）。
fn looks_like_data_dir(dir: &std::path::Path) -> bool {
    ["subscriptions.json", "rules.json"]
        .iter()
        .any(|f| dir.join(f).exists())
        || dir.join("data").join("config.yaml").exists()
}

/// 解析工作目录，支持全局命令安装（便携布局）：
/// - 当前目录含内核（bin/mihomo）时保持当前目录不变（开发与就地运行的既有行为）；
/// - 否则切换到 exe 所在目录，使通过 PATH 全局启动时数据、内核与配置跟随安装目录，
///   而不是散落在用户随意的 cwd 中；
/// - exe 目录也没有内核时（尚未放置内核），不要被 cwd 中残留的 data/config.yaml
///   等生成物留下，仍然跟随安装目录，保持数据位置唯一。
fn resolve_working_dir() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    if dir_has_kernel(&cwd) {
        return;
    }

    let exe_dir = match std::env::current_exe() {
        Ok(p) => {
            let p = std::fs::canonicalize(&p).unwrap_or(p);
            match p.parent() {
                Some(dir) => strip_verbatim_prefix(dir.to_path_buf()),
                None => return,
            }
        }
        Err(_) => return,
    };
    if exe_dir == cwd {
        return;
    }

    if dir_has_kernel(&exe_dir) || !looks_like_data_dir(&cwd) {
        let _ = std::env::set_current_dir(&exe_dir);
    }
}

/// 去掉 Windows `\\?\` 长路径前缀。
///
/// 把带前缀的目录设为进程工作目录后，mihomo 用它拼出来的路径是
/// `\\?\D:\...\data/ASN.mmdb` 这种形式：前缀会禁用分隔符归一化，反斜杠+正斜杠
/// 混用会被 Windows 判为非法路径，表现为 ASN/cache.db 删除失败等一连串错误。
fn strip_verbatim_prefix(path: std::path::PathBuf) -> std::path::PathBuf {
    let s = path.to_string_lossy().to_string();
    let s = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s
    };
    std::path::PathBuf::from(s)
}
