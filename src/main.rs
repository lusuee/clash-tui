mod api;
mod app;
mod core;
mod envproxy;
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
use std::time::Duration;
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
    SubUpdated { msg: String, node_count: usize },
    SubActivated { msg: String },
    CoreActionDone(String),
    TunToggled(Result<bool, String>),
    PortChanged(Result<u16, String>),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse arguments
    let mut api_url = "http://127.0.0.1:9090".to_string();
    let mut secret: Option<String> = None;

    let args: Vec<String> = std::env::args().collect();
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
                println!("Clash TUI - Modern Cross-Platform Mihomo/Clash Terminal Client");
                println!();
                println!("Usage: clash-tui [OPTIONS]");
                println!();
                println!("Options:");
                println!("  -u, --url <URL>        Clash REST API URL (default: http://127.0.0.1:9090)");
                println!("      --secret <SECRET>  Clash external controller secret");
                println!("  -h, --help             Show help information");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    // 2. Ensure Mihomo Core daemon is active
    let core_mgr = CoreManager::new();
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
                AppEvent::SubUpdated { msg, .. } => {
                    app.set_status(format!("✔ {}", msg));
                    // Trigger core reload & refresh proxies
                    let client = app.client.clone();
                    let tx_refresh = tx.clone();
                    let full_path = get_clean_config_path();
                    tokio::spawn(async move {
                        let _ = client.reload_config(&full_path).await;
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        if let Ok(proxies) = client.get_proxies().await {
                            let _ = tx_refresh.send(AppEvent::Proxies(Ok(proxies)));
                        }
                    });
                }
                AppEvent::SubActivated { msg } => {
                    app.set_status(msg);
                    app.selected_group_idx = 0;
                    app.selected_node_idx = 0;
                }
                AppEvent::CoreActionDone(msg) => {
                    app.set_status(msg);
                }
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
                AppEvent::Tick => {}
            }
        }
    }

    // 6. Cleanup terminal on exit
    drop(terminal);
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, Show);
    let _ = io::stdout().flush();

    println!("Clash TUI closed. Mihomo core continues running in the background.");
    std::process::exit(0);
}

async fn handle_key_event(app: &mut App, key: KeyEvent, tx: &mpsc::UnboundedSender<AppEvent>) {
    if key.kind != KeyEventKind::Press {
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
                        let _sub = app.sub_mgr.add(name.clone(), url.clone());
                        app.set_status(format!("✔ Added subscription '{}'. Fetching...", name));

                        // Spawn download
                        let mut sub_mgr = app.sub_mgr.clone();
                        let tx_sub = tx.clone();
                        let sub_idx = app.sub_mgr.subscriptions.len().saturating_sub(1);
                        tokio::spawn(async move {
                            match sub_mgr.update_subscription(sub_idx).await {
                                Ok((msg, count)) => {
                                    let _ = tx_sub.send(AppEvent::SubUpdated { msg, node_count: count });
                                }
                                Err(e) => {
                                    let _ = tx_sub.send(AppEvent::SubUpdated {
                                        msg: format!("Failed to update: {}", e),
                                        node_count: 0,
                                    });
                                }
                            }
                        });
                    }
                }
            }
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
            app.toggle_env_proxy();
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
            }
        }
        KeyCode::Char('u') => {
            if app.active_tab == ActiveTab::Subscriptions && !app.sub_mgr.subscriptions.is_empty() {
                let idx = app.selected_sub_idx;
                let mut sub_mgr = app.sub_mgr.clone();
                let tx_sub = tx.clone();
                app.set_status("Updating subscription in background...");
                tokio::spawn(async move {
                    match sub_mgr.update_subscription(idx).await {
                        Ok((msg, count)) => {
                            let _ = tx_sub.send(AppEvent::SubUpdated { msg, node_count: count });
                        }
                        Err(e) => {
                            let _ = tx_sub.send(AppEvent::SubUpdated {
                                msg: format!("Update failed: {}", e),
                                node_count: 0,
                            });
                        }
                    }
                });
            }
        }
        KeyCode::Char('U') => {
            if app.active_tab == ActiveTab::Subscriptions && !app.sub_mgr.subscriptions.is_empty() {
                let total = app.sub_mgr.subscriptions.len();
                app.set_status(format!("Updating all {} subscriptions...", total));
                for idx in 0..total {
                    let mut sub_mgr = app.sub_mgr.clone();
                    let tx_sub = tx.clone();
                    tokio::spawn(async move {
                        if let Ok((msg, count)) = sub_mgr.update_subscription(idx).await {
                            let _ = tx_sub.send(AppEvent::SubUpdated { msg, node_count: count });
                        }
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
            }
        }
        KeyCode::Char('d') => {
            if app.active_tab == ActiveTab::Connections && !app.connections.is_empty() {
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
