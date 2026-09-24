use crate::app::{format_bytes, format_speed, ActiveTab, App, ModalField, ProxyFocus};
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Sparkline, Table, Tabs,
    },
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header & status bar
            Constraint::Length(3), // Tabs
            Constraint::Min(10),   // Content
            Constraint::Length(3), // Footer / Keymap
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);

    match app.active_tab {
        ActiveTab::Proxies => render_proxies_tab(f, app, chunks[2]),
        ActiveTab::Connections => render_connections_tab(f, app, chunks[2]),
        ActiveTab::Subscriptions => render_subscriptions_tab(f, app, chunks[2]),
        ActiveTab::Settings => render_settings_tab(f, app, chunks[2]),
    }

    render_footer(f, app, chunks[3]);

    // Render modal if open
    if let Some(modal) = &app.show_add_sub_modal {
        render_add_sub_modal(f, modal);
    }
    if let Some(port_input) = &app.show_edit_port_modal {
        render_edit_port_modal(f, port_input);
    }
}

// ----------------------------------------------------------------------------
// 1. Header Bar
// ----------------------------------------------------------------------------
fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(18), // Logo & Online badge
            Constraint::Length(15), // Mode badge
            Constraint::Length(16), // SysProxy badge
            Constraint::Length(15), // EnvProxy badge
            Constraint::Length(14), // TUN badge
            Constraint::Length(14), // Port badge
            Constraint::Min(24),   // Sparkline & Traffic
        ])
        .split(area);

    // 1. Title & Online
    let (conn_sym, conn_style) = if app.is_connected {
        (" ● Online ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
    } else {
        (" ○ Offline ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD))
    };
    let title_line = Line::from(vec![
        Span::styled("⚡ CLASH ", Theme::title_style()),
        Span::styled(conn_sym, conn_style),
    ]);
    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SURFACE1));
    f.render_widget(Paragraph::new(title_line).block(title_block), chunks[0]);

    // 2. Mode Pill
    let mode_color = Theme::mode_color(&app.mode);
    let mode_line = Line::from(vec![
        Span::styled("󰘳 Mode: ", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(
            format!("[{}]", app.mode),
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
    ]);
    let mode_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SURFACE1));
    f.render_widget(Paragraph::new(mode_line).block(mode_block), chunks[1]);

    // 3. SysProxy Pill
    let (sys_text, sys_color) = if app.sys_proxy_enabled {
        ("ON", Theme::GREEN)
    } else {
        ("OFF", Theme::OVERLAY0)
    };
    let sys_line = Line::from(vec![
        Span::styled("󰖟 Sys: ", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(
            format!("[{}]", sys_text),
            Style::default().fg(sys_color).add_modifier(Modifier::BOLD),
        ),
    ]);
    let sys_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SURFACE1));
    f.render_widget(Paragraph::new(sys_line).block(sys_block), chunks[2]);

    // 4. EnvProxy Pill
    let (env_text, env_color) = if app.env_proxy_enabled {
        ("ON", Theme::TEAL)
    } else {
        ("OFF", Theme::OVERLAY0)
    };
    let env_line = Line::from(vec![
        Span::styled("󰌢 Env: ", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(
            format!("[{}]", env_text),
            Style::default().fg(env_color).add_modifier(Modifier::BOLD),
        ),
    ]);
    let env_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SURFACE1));
    f.render_widget(Paragraph::new(env_line).block(env_block), chunks[3]);

    // 5. TUN Pill
    let (tun_text, tun_color) = if app.tun_enabled {
        ("ON", Theme::GREEN)
    } else {
        ("OFF", Theme::OVERLAY0)
    };
    let tun_line = Line::from(vec![
        Span::styled("󰖩 TUN: ", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(
            format!("[{}]", tun_text),
            Style::default().fg(tun_color).add_modifier(Modifier::BOLD),
        ),
    ]);
    let tun_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SURFACE1));
    f.render_widget(Paragraph::new(tun_line).block(tun_block), chunks[4]);

    // 6. Port Pill
    let port_line = Line::from(vec![
        Span::styled("󰛳 Port: ", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(
            format!("{}", app.mixed_port),
            Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
        ),
    ]);
    let port_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SURFACE1));
    f.render_widget(Paragraph::new(port_line).block(port_block), chunks[5]);

    // 7. Traffic Stats + Sparkline
    let traffic_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[6]);

    let sparkline_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SURFACE1))
        .title(Span::styled(" 󰑓 Speed ", Style::default().fg(Theme::SUBTEXT0)));
    let spark = Sparkline::default()
        .block(sparkline_block)
        .data(&app.down_history)
        .style(Style::default().fg(Theme::TEAL));
    f.render_widget(spark, traffic_chunks[0]);

    let traffic_text = Line::from(vec![
        Span::styled("▲ ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
        Span::styled(format_speed(app.up_speed), Style::default().fg(Theme::TEXT)),
        Span::raw(" "),
        Span::styled("▼ ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
        Span::styled(format_speed(app.down_speed), Style::default().fg(Theme::TEXT)),
    ]);
    let traffic_box = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SURFACE1));
    f.render_widget(
        Paragraph::new(traffic_text)
            .block(traffic_box)
            .alignment(Alignment::Center),
        traffic_chunks[1],
    );
}

// ----------------------------------------------------------------------------
// 2. Tabs Bar
// ----------------------------------------------------------------------------
fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles = vec![
        " [1] 󰒍 Proxies ",
        " [2] 󰈀 Connections ",
        " [3] 󰑓 Subscriptions ",
        " [4] 󰒓 Settings ",
    ];
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Theme::SURFACE1)),
        )
        .select(app.active_tab as usize)
        .style(Style::default().fg(Theme::OVERLAY0))
        .highlight_style(
            Style::default()
                .fg(Theme::LAVENDER)
                .bg(Theme::SURFACE0)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, area);
}

// ----------------------------------------------------------------------------
// 3. Tab 1: Proxies View
// ----------------------------------------------------------------------------
fn render_proxies_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(32), Constraint::Percentage(68)])
        .split(area);

    let is_group_focused = app.proxy_focus == ProxyFocus::Groups;
    let is_node_focused = app.proxy_focus == ProxyFocus::Nodes;

    // Groups column
    let group_border_style = if is_group_focused {
        Theme::active_border()
    } else {
        Theme::inactive_border()
    };
    let group_title = format!(
        " 󰒍 Proxy Groups ({}/{}) ",
        if app.proxy_groups.is_empty() { 0 } else { app.selected_group_idx + 1 },
        app.proxy_groups.len()
    );
    let group_block = Block::default()
        .title(Span::styled(group_title, Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(group_border_style);

    let mut group_rows = Vec::new();
    for (idx, group_name) in app.proxy_groups.iter().enumerate() {
        let is_selected = idx == app.selected_group_idx;
        let group_item = app.proxies.get(group_name);
        let now_node = group_item
            .and_then(|g| g.now.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("");
        let count = group_item
            .and_then(|g| g.all.as_ref())
            .map(|a| a.len())
            .unwrap_or(0);

        let row_style = if is_selected && is_group_focused {
            Theme::selected_row_focused()
        } else if is_selected {
            Theme::selected_row_unfocused()
        } else {
            Theme::normal_row()
        };

        let pointer = if is_selected { "▶ " } else { "  " };
        let cells = vec![
            Cell::from(format!("{}{}", pointer, group_name)),
            Cell::from(Span::styled(format!("({})", count), Style::default().fg(Theme::MUTED))),
            Cell::from(Span::styled(now_node, Style::default().fg(Theme::LAVENDER))),
        ];
        group_rows.push(Row::new(cells).style(row_style));
    }

    let groups_table = Table::new(
        group_rows,
        [
            Constraint::Percentage(45),
            Constraint::Percentage(15),
            Constraint::Percentage(40),
        ],
    )
    .header(
        Row::new(vec!["Group", "Nodes", "Active Node"])
            .style(Style::default().fg(Theme::SUBTEXT0).add_modifier(Modifier::BOLD)),
    )
    .block(group_block);

    app.groups_table_state.select(if app.proxy_groups.is_empty() { None } else { Some(app.selected_group_idx) });
    f.render_stateful_widget(groups_table, chunks[0], &mut app.groups_table_state);

    if app.proxy_groups.len() > 1 {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█");
        let mut scrollbar_state = ScrollbarState::new(app.proxy_groups.len()).position(app.selected_group_idx);
        f.render_stateful_widget(scrollbar, chunks[0].inner(Margin { vertical: 1, horizontal: 0 }), &mut scrollbar_state);
    }

    // Nodes column
    let node_border_style = if is_node_focused {
        Theme::active_border()
    } else {
        Theme::inactive_border()
    };
    let curr_group_name = app.current_group_name().unwrap_or("None");
    let nodes = app.current_group_nodes();
    let current_active = app
        .proxies
        .get(curr_group_name)
        .and_then(|g| g.now.as_ref())
        .map(|s| s.as_str());

    let node_title = format!(
        " 󰈀 Nodes in [{}] ({}/{}) ",
        curr_group_name,
        if nodes.is_empty() { 0 } else { app.selected_node_idx + 1 },
        nodes.len()
    );

    let node_block = Block::default()
        .title(Span::styled(
            node_title,
            Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(node_border_style);

    let mut node_rows = Vec::new();
    for (idx, node_name) in nodes.iter().enumerate() {
        let is_cursor = idx == app.selected_node_idx;
        let is_now = current_active == Some(node_name.as_str());
        let node_item = app.proxies.get(node_name);
        let proto = node_item.map(|n| n.proxy_type.as_str()).unwrap_or("Unknown");

        // Latency
        let (delay_text, delay_color) = if app.testing_nodes.contains(node_name) {
            ("testing...".to_string(), Theme::SAPPHIRE)
        } else if let Some(&d) = app.delays.get(node_name) {
            let color = Theme::latency_color(d);
            (format!("{} ms", d), color)
        } else {
            ("-".to_string(), Theme::OVERLAY0)
        };

        let pointer = if is_cursor { "▶ " } else { "  " };
        let active_mark = if is_now { "●" } else { "○" };
        let active_style = if is_now {
            Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::OVERLAY0)
        };

        let row_style = if is_cursor && is_node_focused {
            Theme::selected_row_focused()
        } else if is_cursor {
            Theme::selected_row_unfocused()
        } else {
            Theme::normal_row()
        };

        let proto_color = Theme::protocol_color(proto);
        let cells = vec![
            Cell::from(format!("{}{}", pointer, active_mark)).style(active_style),
            Cell::from(node_name.clone()),
            Cell::from(Span::styled(format!("[{}]", proto), Style::default().fg(proto_color))),
            Cell::from(Span::styled(delay_text, Style::default().fg(delay_color))),
        ];
        node_rows.push(Row::new(cells).style(row_style));
    }

    let nodes_table = Table::new(
        node_rows,
        [
            Constraint::Length(5),
            Constraint::Percentage(55),
            Constraint::Length(14),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new(vec!["", "Node Name", "Protocol", "Latency"])
            .style(Style::default().fg(Theme::SUBTEXT0).add_modifier(Modifier::BOLD)),
    )
    .block(node_block);

    app.nodes_table_state.select(if nodes.is_empty() { None } else { Some(app.selected_node_idx) });
    f.render_stateful_widget(nodes_table, chunks[1], &mut app.nodes_table_state);

    if nodes.len() > 1 {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█");
        let mut scrollbar_state = ScrollbarState::new(nodes.len()).position(app.selected_node_idx);
        f.render_stateful_widget(scrollbar, chunks[1].inner(Margin { vertical: 1, horizontal: 0 }), &mut scrollbar_state);
    }
}

// ----------------------------------------------------------------------------
// 4. Tab 2: Connections View
// ----------------------------------------------------------------------------
fn render_connections_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            format!(
                " 󰈀 Active Connections ({}/{}) | Total Up: {} | Total Down: {} ",
                if app.connections.is_empty() { 0 } else { app.selected_conn_idx + 1 },
                app.connections.len(),
                format_bytes(app.total_upload),
                format_bytes(app.total_download)
            ),
            Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border());

    let mut rows = Vec::new();
    for (idx, conn) in app.connections.iter().enumerate() {
        let is_selected = idx == app.selected_conn_idx;
        let row_style = if is_selected {
            Theme::selected_row_focused()
        } else {
            Theme::normal_row()
        };

        let pointer = if is_selected { "▶ " } else { "  " };
        let host = conn
            .metadata
            .host
            .as_deref()
            .or(conn.metadata.destination_ip.as_deref())
            .unwrap_or("Unknown");
        let port = conn.metadata.destination_port.as_deref().unwrap_or("");
        let host_port = if port.is_empty() {
            host.to_string()
        } else {
            format!("{}:{}", host, port)
        };

        let network = conn.metadata.network.as_deref().unwrap_or("tcp").to_uppercase();
        let chains = conn.chains.join(" -> ");
        let transfer = format!("▲ {} / ▼ {}", format_bytes(conn.upload), format_bytes(conn.download));

        let cells = vec![
            Cell::from(format!("{}{}", pointer, host_port)),
            Cell::from(Span::styled(network, Style::default().fg(Theme::SAPPHIRE))),
            Cell::from(transfer),
            Cell::from(Span::styled(conn.rule.clone(), Style::default().fg(Theme::YELLOW))),
            Cell::from(chains),
        ];
        rows.push(Row::new(cells).style(row_style));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Length(8),
            Constraint::Length(22),
            Constraint::Percentage(15),
            Constraint::Percentage(25),
        ],
    )
    .header(
        Row::new(vec!["Destination", "Net", "Transfer", "Rule", "Chains"])
            .style(Style::default().fg(Theme::SUBTEXT0).add_modifier(Modifier::BOLD)),
    )
    .block(block);

    app.connections_table_state.select(if app.connections.is_empty() { None } else { Some(app.selected_conn_idx) });
    f.render_stateful_widget(table, area, &mut app.connections_table_state);

    if app.connections.len() > 1 {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█");
        let mut scrollbar_state = ScrollbarState::new(app.connections.len()).position(app.selected_conn_idx);
        f.render_stateful_widget(scrollbar, area.inner(Margin { vertical: 1, horizontal: 0 }), &mut scrollbar_state);
    }
}

// ----------------------------------------------------------------------------
// 5. Tab 3: Subscriptions View
// ----------------------------------------------------------------------------
fn render_subscriptions_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .title(Span::styled(
            format!(
                " 󰑓 Subscriptions Management ({}/{}) ",
                if app.sub_mgr.subscriptions.is_empty() { 0 } else { app.selected_sub_idx + 1 },
                app.sub_mgr.subscriptions.len()
            ),
            Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border());

    let mut rows = Vec::new();
    for (idx, sub) in app.sub_mgr.subscriptions.iter().enumerate() {
        let is_selected = idx == app.selected_sub_idx;
        let row_style = if is_selected {
            Theme::selected_row_focused()
        } else {
            Theme::normal_row()
        };

        let pointer = if is_selected { "▶ " } else { "  " };
        let (active_badge, active_style) = if sub.active {
            (" ● Active ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
        } else {
            (" - ", Style::default().fg(Theme::OVERLAY0))
        };

        let cells = vec![
            Cell::from(Span::styled(format!("{}{}", pointer, active_badge), active_style)),
            Cell::from(sub.name.clone()),
            Cell::from(Span::styled(
                format!("{} nodes", sub.node_count),
                Style::default().fg(Theme::SAPPHIRE),
            )),
            Cell::from(Span::styled(sub.last_updated.clone(), Style::default().fg(Theme::MUTED))),
            Cell::from(sub.url.clone()),
        ];
        rows.push(Row::new(cells).style(row_style));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Percentage(25),
            Constraint::Length(12),
            Constraint::Length(18),
            Constraint::Percentage(45),
        ],
    )
    .header(
        Row::new(vec!["Status", "Subscription Name", "Count", "Last Updated", "Subscription URL"])
            .style(Style::default().fg(Theme::SUBTEXT0).add_modifier(Modifier::BOLD)),
    )
    .block(block);

    app.subs_table_state.select(if app.sub_mgr.subscriptions.is_empty() { None } else { Some(app.selected_sub_idx) });
    f.render_stateful_widget(table, area, &mut app.subs_table_state);

    if app.sub_mgr.subscriptions.len() > 1 {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█");
        let mut scrollbar_state = ScrollbarState::new(app.sub_mgr.subscriptions.len()).position(app.selected_sub_idx);
        f.render_stateful_widget(scrollbar, area.inner(Margin { vertical: 1, horizontal: 0 }), &mut scrollbar_state);
    }
}

// ----------------------------------------------------------------------------
// 6. Tab 4: Settings View
// ----------------------------------------------------------------------------
fn render_settings_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Length(8), Constraint::Min(4)])
        .split(area);

    // Card 1: Mihomo Core Daemon
    let core_block = Block::default()
        .title(Span::styled(" ⚙ Mihomo Core Daemon ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::inactive_border());
    let core_lines = vec![
        Line::from(vec![
            Span::styled("Binary Path:       ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(app.core_mgr.bin_path.to_string_lossy(), Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("Core Version:      ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(&app.core_version, Style::default().fg(Theme::LAVENDER)),
        ]),
        Line::from(vec![
            Span::styled("Daemon Status:     ", Style::default().fg(Theme::SUBTEXT0)),
            if app.is_connected || app.is_core_running {
                Span::styled("Active Running (background)", Style::default().fg(Theme::GREEN))
            } else {
                Span::styled("Stopped", Style::default().fg(Theme::RED))
            },
        ]),
        Line::from(vec![
            Span::styled("Core Hotkeys:      ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled("[s] Stop Core Process    [S] Restart Core Daemon", Style::default().fg(Theme::YELLOW)),
        ]),
    ];
    f.render_widget(Paragraph::new(core_lines).block(core_block), chunks[0]);

    // Card 2: Network & Proxy Controls
    let net_block = Block::default()
        .title(Span::styled(" 󰖟 System Proxy & Network Controls ", Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::inactive_border());
    let (sys_status_str, sys_color) = if app.sys_proxy_enabled {
        (format!("Enabled (127.0.0.1:{})", app.mixed_port), Theme::GREEN)
    } else {
        ("Disabled".to_string(), Theme::OVERLAY0)
    };

    let (env_status_str, env_color) = if app.env_proxy_enabled {
        (format!("Enabled (HKCU + env.nu/bat/ps1, port {})", app.mixed_port), Theme::TEAL)
    } else {
        ("Disabled".to_string(), Theme::OVERLAY0)
    };

    let (tun_status_str, tun_color) = if app.tun_enabled {
        ("Enabled (Virtual Interface Active)".to_string(), Theme::GREEN)
    } else {
        ("Disabled (Requires Admin / Root)".to_string(), Theme::OVERLAY0)
    };

    #[cfg(target_os = "windows")]
    let platform_name = "Windows (WinINet + Registry)";
    #[cfg(target_os = "macos")]
    let platform_name = "macOS (networksetup CLI)";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let platform_name = "Linux/Other";

    let net_lines = vec![
        Line::from(vec![
            Span::styled("Platform Backend:  ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(platform_name, Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("System Proxy:      ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(sys_status_str, Style::default().fg(sys_color).add_modifier(Modifier::BOLD)),
            Span::styled("  [p] Toggle", Style::default().fg(Theme::YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("Environment Proxy: ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(env_status_str, Style::default().fg(env_color).add_modifier(Modifier::BOLD)),
            Span::styled("  [e] Toggle", Style::default().fg(Theme::YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("TUN Mode:          ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(tun_status_str, Style::default().fg(tun_color).add_modifier(Modifier::BOLD)),
            Span::styled("  [n] Toggle", Style::default().fg(Theme::YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("Mixed Proxy Port:  ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(format!("{}", app.mixed_port), Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("  [P] Change Port", Style::default().fg(Theme::PEACH)),
        ]),
        Line::from(vec![
            Span::styled("Elevated Core:     ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled("[A] Restart Mihomo with Administrator Rights (UAC)", Style::default().fg(Theme::RED)),
        ]),
    ];
    f.render_widget(Paragraph::new(net_lines).block(net_block), chunks[1]);

    // Card 3: Quick Info
    let help_block = Block::default()
        .title(Span::styled(" 󰋖 About Clash TUI ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::inactive_border());
    let help_lines = vec![
        Line::from("Built with Rust + Ratatui + Crossterm + Tokio."),
        Line::from("Ultra-lightweight, cross-platform terminal proxy manager with zero web engine overhead."),
    ];
    f.render_widget(Paragraph::new(help_lines).block(help_block), chunks[2]);
}

// ----------------------------------------------------------------------------
// 7. Modal: Add Subscription Dialog
// ----------------------------------------------------------------------------
fn render_add_sub_modal(f: &mut Frame, modal: &crate::app::AddSubModal) {
    let area = centered_rect(65, 40, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " 󰑓 Add New Subscription ",
            Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border());

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Name input
            Constraint::Length(3), // URL input
            Constraint::Length(2), // Hints
        ])
        .split(inner);

    // Name Field
    let name_border_style = if modal.active_field == ModalField::Name {
        Theme::active_border()
    } else {
        Theme::inactive_border()
    };
    let name_block = Block::default()
        .title(" Subscription Name ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(name_border_style);
    let name_text = Paragraph::new(modal.name_input.as_str()).block(name_block);
    f.render_widget(name_text, chunks[0]);

    // URL Field
    let url_border_style = if modal.active_field == ModalField::Url {
        Theme::active_border()
    } else {
        Theme::inactive_border()
    };
    let url_block = Block::default()
        .title(" Subscription URL ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(url_border_style);
    let url_text = Paragraph::new(modal.url_input.as_str()).block(url_block);
    f.render_widget(url_text, chunks[1]);

    // Action Hints
    let hint_line = Line::from(vec![
        Span::styled("[Tab] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
        Span::raw("Switch Field   "),
        Span::styled("[Enter] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
        Span::raw("Save & Fetch   "),
        Span::styled("[Esc] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
        Span::raw("Cancel"),
    ]);
    f.render_widget(Paragraph::new(hint_line).alignment(Alignment::Center), chunks[2]);
}

fn render_edit_port_modal(f: &mut Frame, port_input: &str) {
    let area = centered_rect(50, 30, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " 󰛳 Change Mixed Proxy Port ",
            Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border());

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // Hint
            Constraint::Length(3), // Input box
            Constraint::Length(1), // Help
        ])
        .split(inner);

    let hint = Paragraph::new("Enter new mixed proxy port (1 ~ 65535):")
        .style(Style::default().fg(Theme::SUBTEXT0));
    f.render_widget(hint, chunks[0]);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::YELLOW));
    let input_text = Line::from(vec![
        Span::raw(" "),
        Span::styled(port_input, Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD)),
        Span::styled("█", Style::default().fg(Theme::YELLOW)),
    ]);
    f.render_widget(Paragraph::new(input_text).block(input_block), chunks[1]);

    let action_hints = Line::from(vec![
        Span::styled("[Enter] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
        Span::raw("Apply Port   "),
        Span::styled("[Esc] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
        Span::raw("Cancel"),
    ]);
    f.render_widget(Paragraph::new(action_hints).alignment(Alignment::Center), chunks[2]);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// ----------------------------------------------------------------------------
// 8. Footer & Status Toast
// ----------------------------------------------------------------------------
fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Toast bar
    if let Some(toast) = app.get_active_status() {
        let toast_style = if toast.starts_with('✔') {
            Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)
        } else if toast.starts_with('✘') {
            Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)
        };
        let toast_p = Paragraph::new(Line::from(vec![
            Span::styled(" 󰍡 ", Style::default().fg(Theme::MAUVE)),
            Span::styled(toast, toast_style),
        ]));
        f.render_widget(toast_p, chunks[0]);
    } else {
        let hint_line = match app.active_tab {
            ActiveTab::Proxies => Line::from(vec![
                Span::styled(" [Enter] ", Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD)),
                Span::raw("Select  "),
                Span::styled(" [t/T] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Ping  "),
                Span::styled(" [←/→] ", Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD)),
                Span::raw("Col  "),
                Span::styled(" [PgUp/PgDn] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("Page Scroll"),
            ]),
            ActiveTab::Connections => Line::from(vec![
                Span::styled(" [d/D] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::raw("Close Conn  "),
                Span::styled(" [PgUp/PgDn] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("Page Scroll"),
            ]),
            ActiveTab::Subscriptions => Line::from(vec![
                Span::styled(" [a] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
                Span::raw("Add  "),
                Span::styled(" [u/U] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Update  "),
                Span::styled(" [Enter] ", Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD)),
                Span::raw("Apply  "),
                Span::styled(" [x] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::raw("Del  "),
                Span::styled(" [PgUp/PgDn] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("Scroll"),
            ]),
            ActiveTab::Settings => Line::from(vec![
                Span::styled(" [p] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
                Span::raw("SysProxy  "),
                Span::styled(" [e] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("EnvProxy  "),
                Span::styled(" [n] ", Style::default().fg(Theme::SAPPHIRE).add_modifier(Modifier::BOLD)),
                Span::raw("TUN  "),
                Span::styled(" [P] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Change Port  "),
                Span::styled(" [A] ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
                Span::raw("Admin Core  "),
                Span::styled(" [s/S] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::raw("Stop/Restart"),
            ]),
        };
        f.render_widget(Paragraph::new(hint_line), chunks[0]);
    }

    // Global Keymap
    let keys = vec![
        Span::styled("[q] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
        Span::raw("Quit  "),
        Span::styled("[Tab] ", Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD)),
        Span::raw("Next Tab  "),
        Span::styled("[p] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
        Span::raw("SysProxy  "),
        Span::styled("[e] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
        Span::raw("EnvProxy  "),
        Span::styled("[n] ", Style::default().fg(Theme::SAPPHIRE).add_modifier(Modifier::BOLD)),
        Span::raw("TUN  "),
        Span::styled("[P] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
        Span::raw("Port  "),
        Span::styled("[m] ", Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD)),
        Span::raw("Mode"),
    ];
    f.render_widget(
        Paragraph::new(Line::from(keys)).style(Style::default().fg(Theme::MUTED)),
        chunks[1],
    );
}
