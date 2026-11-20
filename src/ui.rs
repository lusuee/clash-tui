use crate::app::{format_bytes, format_speed, ActiveTab, App, ModalField, ProxyFocus, TRAFFIC_HISTORY_CAP};
use crate::appupdate;
use crate::kernelupdate;
use crate::rules::{ProbeOutcome, RuleTarget};
use crate::subscriptions::{auto_update_label, SubscriptionItem};
use crate::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Sparkline, Table, Tabs, Wrap,
    },
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Cyber HUD Header & Telemetry
            Constraint::Length(3), // Workspace Navigator Tabs
            Constraint::Min(10),   // Active Workspace Panel
            Constraint::Length(3), // Cyber Command Deck & Status Toast
        ])
        .split(f.area());

    render_header(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);

    match app.active_tab {
        ActiveTab::Proxies => render_proxies_tab(f, app, chunks[2]),
        ActiveTab::Connections => render_connections_tab(f, app, chunks[2]),
        ActiveTab::Subscriptions => render_subscriptions_tab(f, app, chunks[2]),
        ActiveTab::Settings => render_settings_tab(f, app, chunks[2]),
        ActiveTab::Rules => render_rules_tab(f, app, chunks[2]),
    }

    render_footer(f, app, chunks[3]);

    // Render modal if open
    if let Some(modal) = &app.show_add_sub_modal {
        render_add_sub_modal(f, modal);
    }
    if let Some(modal) = &app.show_add_rule_modal {
        render_add_rule_modal(f, modal, &app.rule_mgr);
    }
    if let Some(modal) = &app.show_rule_test_modal {
        render_rule_test_modal(f, modal);
    }
    if let Some(modal) = &app.show_dns_modal {
        render_dns_modal(f, modal);
    }
    if let Some(modal) = &app.show_overrides_modal {
        render_overrides_modal(f, modal);
    }
    if let Some(modal) = &app.show_backup_modal {
        render_backup_modal(f, modal);
    }
    if let Some(modal) = &app.show_providers_modal {
        render_providers_modal(f, modal);
    }
    if app.show_traffic_modal {
        render_traffic_modal(f, app);
    }
    if let Some(modal) = &app.show_health_modal {
        render_health_modal(f, modal);
    }
    if let Some(modal) = &app.show_logs_modal {
        render_logs_modal(f, modal);
    }
    if let Some(port_input) = &app.show_edit_port_modal {
        render_edit_port_modal(f, port_input);
    }
    if app.show_help {
        let area = f.area();
        render_help_modal(f, area);
    }
}

// ----------------------------------------------------------------------------
// 1. Header Bar: Cyber Telemetry & Live Bandwidth
// ----------------------------------------------------------------------------
fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(64), // Telemetry & Badges
            Constraint::Percentage(36), // Bandwidth rates & Sparkline
        ])
        .split(area);

    // --- Left Panel: System Telemetry ---
    let (conn_sym, conn_style) = if app.is_connected {
        ("● ONLINE", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
    } else {
        ("○ OFFLINE", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD))
    };

    let mode_color = Theme::mode_color(&app.mode);

    let (sys_badge, sys_style) = if app.sys_proxy_enabled {
        ("[SYS:ON]", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
    } else {
        ("[SYS:--]", Style::default().fg(Theme::MUTED))
    };

    let (env_badge, env_style) = if app.env_proxy_enabled {
        ("[ENV:ON]", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD))
    } else {
        ("[ENV:--]", Style::default().fg(Theme::MUTED))
    };

    let (tun_badge, tun_style) = if app.tun_enabled {
        ("[TUN:ON]", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
    } else {
        ("[TUN:--]", Style::default().fg(Theme::MUTED))
    };

    let port_badge = format!("[:{}]", app.mixed_port);

    let telemetry_line = Line::from(vec![
        Span::styled(" ⚡ CLASH", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
        Span::styled("//", Style::default().fg(Theme::SURFACE2)),
        Span::styled("TUI ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{} ", app.app_version), Style::default().fg(Theme::SUBTEXT0)),
        Span::styled("│ ", Style::default().fg(Theme::SURFACE2)),
        Span::styled(conn_sym, conn_style),
        Span::styled(" │ ", Style::default().fg(Theme::SURFACE2)),
        Span::styled(format!("[{}] ", app.mode.to_uppercase()), Style::default().fg(mode_color).add_modifier(Modifier::BOLD)),
        Span::styled(sys_badge, sys_style),
        Span::raw(" "),
        Span::styled(env_badge, env_style),
        Span::raw(" "),
        Span::styled(tun_badge, tun_style),
        Span::styled(" │ ", Style::default().fg(Theme::SURFACE2)),
        Span::styled(port_badge, Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
    ]);

    let telemetry_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SURFACE2))
        .title(Span::styled(" ◈ SYSTEM TELEMETRY ◈ ", Style::default().fg(Theme::SUBTEXT0).add_modifier(Modifier::BOLD)));

    f.render_widget(Paragraph::new(telemetry_line).block(telemetry_block), header_chunks[0]);

    // --- Right Panel: Live Bandwidth ---
    let flow_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SURFACE2))
        .title(Span::styled(" ◈ LIVE BANDWIDTH ◈ ", Style::default().fg(Theme::SUBTEXT0).add_modifier(Modifier::BOLD)));

    let flow_inner = flow_block.inner(header_chunks[1]);
    f.render_widget(flow_block, header_chunks[1]);

    let flow_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(23), // Speed indicators
            Constraint::Min(8),     // Dynamic sparkline
        ])
        .split(flow_inner);

    let speed_line = Line::from(vec![
        Span::styled("▲ ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
        Span::styled(format_speed(app.up_speed), Style::default().fg(Theme::TEXT)),
        Span::styled(" │ ", Style::default().fg(Theme::SURFACE2)),
        Span::styled("▼ ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
        Span::styled(format_speed(app.down_speed), Style::default().fg(Theme::TEXT)),
    ]);
    f.render_widget(Paragraph::new(speed_line), flow_chunks[0]);

    let spark = Sparkline::default()
        .data(&app.down_history)
        .style(Style::default().fg(Theme::TEAL));
    f.render_widget(spark, flow_chunks[1]);
}

// ----------------------------------------------------------------------------
// 2. Tabs Bar: Cyber Navigation Deck
// ----------------------------------------------------------------------------
fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let tab_specs = [
        (ActiveTab::Proxies, "01", "󰒍 PROXIES"),
        (ActiveTab::Connections, "02", "󰈀 CONNECTIONS"),
        (ActiveTab::Subscriptions, "03", "󰑓 SUBSCRIPTIONS"),
        (ActiveTab::Settings, "04", "󰒓 SETTINGS"),
        (ActiveTab::Rules, "05", "󰃢 RULES"),
    ];

    let titles: Vec<Line> = tab_specs
        .iter()
        .map(|(tab, num, label)| {
            let is_selected = app.active_tab == *tab;
            if is_selected {
                Line::from(vec![
                    Span::styled(format!(" ◈ {} ", num), Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{} ", label), Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD)),
                ])
            } else {
                Line::from(vec![
                    Span::styled(format!("  {} ", num), Style::default().fg(Theme::MUTED)),
                    Span::styled(format!("{} ", label), Style::default().fg(Theme::SUBTEXT0)),
                ])
            }
        })
        .collect();

    let tab_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SURFACE2))
        .title(Span::styled(" ◈ WORKSPACE NAVIGATOR ◈ ", Style::default().fg(Theme::MUTED)));

    let tabs = Tabs::new(titles)
        .block(tab_block)
        .divider(Span::styled("│", Style::default().fg(Theme::SURFACE2)))
        .select(app.active_tab as usize)
        .style(Style::default().fg(Theme::OVERLAY0))
        .highlight_style(
            Style::default()
                .bg(Theme::SURFACE1)
                .fg(Theme::CYAN)
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

    // --- Groups Column ---
    let group_border_style = if is_group_focused {
        Theme::active_border()
    } else {
        Theme::inactive_border()
    };

    let group_title = Line::from(vec![
        Span::styled(
            " 󰒍 PROXY GROUPS ",
            Style::default()
                .fg(if is_group_focused { Theme::CYAN } else { Theme::SUBTEXT0 })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "⟨{}/{}⟩ ",
                if app.proxy_groups.is_empty() { 0 } else { app.selected_group_idx + 1 },
                app.proxy_groups.len()
            ),
            Style::default().fg(Theme::MUTED),
        ),
    ]);

    let group_block = Block::default()
        .title(group_title)
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
            .unwrap_or("-");
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

        let pointer = if is_selected { "❯ " } else { "  " };
        let cells = vec![
            Cell::from(format!("{}{}", pointer, format_display_name(group_name))),
            Cell::from(Span::styled(format!("⟨{}⟩", count), Style::default().fg(Theme::SAPPHIRE))),
            Cell::from(Span::styled(format!("➜ {}", format_display_name(now_node)), Style::default().fg(Theme::TEAL))),
        ];
        group_rows.push(Row::new(cells).style(row_style));
    }

    let groups_table = Table::new(
        group_rows,
        [
            Constraint::Percentage(46),
            Constraint::Percentage(16),
            Constraint::Percentage(38),
        ],
    )
    .header(
        Row::new(vec!["GROUP IDENTIFIER", "NODES", "ROUTED TARGET"])
            .style(Style::default().fg(Theme::MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(group_block);

    app.groups_table_state.select(if app.proxy_groups.is_empty() { None } else { Some(app.selected_group_idx) });
    f.render_stateful_widget(groups_table, chunks[0], &mut app.groups_table_state);

    if app.proxy_groups.len() > 1 {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(Style::default().fg(Theme::SURFACE2));
        let mut scrollbar_state = ScrollbarState::new(app.proxy_groups.len()).position(app.selected_group_idx);
        f.render_stateful_widget(scrollbar, chunks[0].inner(Margin { vertical: 1, horizontal: 0 }), &mut scrollbar_state);
    }

    // --- Nodes Column ---
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

    let node_title = Line::from(vec![
        Span::styled(
            format!(" 󰈀 NODES IN [{}] ", format_display_name(curr_group_name)),
            Style::default()
                .fg(if is_node_focused { Theme::CYAN } else { Theme::SUBTEXT0 })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "⟨{}/{}⟩ ",
                if nodes.is_empty() { 0 } else { app.selected_node_idx + 1 },
                nodes.len()
            ),
            Style::default().fg(Theme::MUTED),
        ),
        Span::styled("│ SORT: ", Style::default().fg(Theme::SURFACE2)),
        Span::styled(
            format!("[{}] ", app.node_sort.label()),
            Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
        ),
    ]);

    let node_block = Block::default()
        .title(node_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(node_border_style);

    let mut node_rows = Vec::new();
    for (idx, node_name) in nodes.iter().enumerate() {
        let is_cursor = idx == app.selected_node_idx;
        let is_now = current_active == Some(node_name.as_str());
        let node_item = app.proxies.get(node_name);
        let proto = node_item.map(|n| n.proxy_type.as_str()).unwrap_or("UNKNOWN");

        // High-tech Latency Gauge
        let (delay_span, _) = if app.testing_nodes.contains(node_name) {
            (
                Span::styled("⟳ testing...", Style::default().fg(Theme::SAPPHIRE)),
                Theme::SAPPHIRE,
            )
        } else if let Some(&d) = app.delays.get(node_name) {
            let (badge, color) = Theme::latency_badge(d);
            (
                Span::styled(
                    format!("{} {:>4} ms", badge, d),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                color,
            )
        } else {
            (
                Span::styled("┄   timeout", Style::default().fg(Theme::MUTED)),
                Theme::MUTED,
            )
        };

        let pointer = if is_cursor { "❯" } else { " " };
        let active_mark = if is_now { "●" } else { "○" };
        let active_style = if is_now {
            Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::MUTED)
        };

        let row_style = if is_cursor && is_node_focused {
            Theme::selected_row_focused()
        } else if is_cursor {
            Theme::selected_row_unfocused()
        } else {
            Theme::normal_row()
        };

        let proto_color = Theme::protocol_color(proto);
        let proto_badge = format!(" {} ", proto.to_uppercase());

        let cells = vec![
            Cell::from(format!("{} {}", pointer, active_mark)).style(active_style),
            Cell::from(format_display_name(node_name)),
            Cell::from(Span::styled(
                proto_badge,
                Style::default().fg(proto_color).add_modifier(Modifier::BOLD),
            )),
            Cell::from(delay_span),
        ];
        node_rows.push(Row::new(cells).style(row_style));
    }

    let nodes_table = Table::new(
        node_rows,
        [
            Constraint::Length(5),
            Constraint::Percentage(55),
            Constraint::Length(14),
            Constraint::Length(15),
        ],
    )
    .header(
        Row::new(vec!["ACT", "NODE IDENTIFIER", "PROTOCOL", "LATENCY GAUGE"])
            .style(Style::default().fg(Theme::MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(node_block);

    app.nodes_table_state.select(if nodes.is_empty() { None } else { Some(app.selected_node_idx) });
    f.render_stateful_widget(nodes_table, chunks[1], &mut app.nodes_table_state);

    if nodes.len() > 1 {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(Style::default().fg(Theme::SURFACE2));
        let mut scrollbar_state = ScrollbarState::new(nodes.len()).position(app.selected_node_idx);
        f.render_stateful_widget(scrollbar, chunks[1].inner(Margin { vertical: 1, horizontal: 0 }), &mut scrollbar_state);
    }
}

// ----------------------------------------------------------------------------
// 4. Tab 2: Connections View
// ----------------------------------------------------------------------------
fn render_connections_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let conn_title = Line::from(vec![
        Span::styled(" 󰈀 LIVE NETWORK TELEMETRY ", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
        Span::styled(format!("⟨{} active⟩ ", app.connections.len()), Style::default().fg(Theme::MUTED)),
        Span::styled("│ ▲ UP: ", Style::default().fg(Theme::PEACH)),
        Span::styled(format!("{} ", format_bytes(app.total_upload)), Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD)),
        Span::styled("│ ▼ DOWN: ", Style::default().fg(Theme::TEAL)),
        Span::styled(format!("{} ", format_bytes(app.total_download)), Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD)),
    ]);

    let block = Block::default()
        .title(conn_title)
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

        let pointer = if is_selected { "❯ " } else { "  " };
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
        let net_color = if network == "UDP" { Theme::PEACH } else { Theme::SAPPHIRE };
        let chains = conn
            .chains
            .iter()
            .map(|c| format_display_name(c))
            .collect::<Vec<_>>()
            .join(" ❯ ");
        let transfer_line = Line::from(vec![
            Span::styled("▲ ", Style::default().fg(Theme::PEACH)),
            Span::raw(format!("{:<8} ", format_bytes(conn.upload))),
            Span::styled("▼ ", Style::default().fg(Theme::TEAL)),
            Span::raw(format_bytes(conn.download)),
        ]);

        let process = conn
            .metadata
            .process_path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name().map(|n| n.to_string_lossy().to_string()))
            .map(|n| n.trim_end_matches(".exe").to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "—".to_string());
        let process = if process.chars().count() > 18 {
            format!("{}…", process.chars().take(17).collect::<String>())
        } else {
            process
        };

        let cells = vec![
            Cell::from(format!("{}{}", pointer, host_port)),
            Cell::from(Span::styled(format!(" {} ", network), Style::default().fg(net_color).add_modifier(Modifier::BOLD))),
            Cell::from(transfer_line),
            Cell::from(Span::styled(format!("[{}]", conn.rule), Style::default().fg(Theme::YELLOW))),
            Cell::from(chains),
            Cell::from(Span::styled(process, Style::default().fg(Theme::SUBTEXT0))),
        ];
        rows.push(Row::new(cells).style(row_style));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(28),
            Constraint::Length(8),
            Constraint::Length(22),
            Constraint::Percentage(15),
            Constraint::Percentage(18),
            Constraint::Percentage(12),
        ],
    )
    .header(
        Row::new(vec!["DESTINATION ENDPOINT", "NET", "DATA TRANSFER", "ROUTING RULE", "PROXY CHAIN", "PROCESS"])
            .style(Style::default().fg(Theme::MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(block);

    app.connections_table_state.select(if app.connections.is_empty() { None } else { Some(app.selected_conn_idx) });
    f.render_stateful_widget(table, area, &mut app.connections_table_state);

    if app.connections.len() > 1 {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(Style::default().fg(Theme::SURFACE2));
        let mut scrollbar_state = ScrollbarState::new(app.connections.len()).position(app.selected_conn_idx);
        f.render_stateful_widget(scrollbar, area.inner(Margin { vertical: 1, horizontal: 0 }), &mut scrollbar_state);
    }
}

// ----------------------------------------------------------------------------
// 5. Tab 3: Subscriptions View
// ----------------------------------------------------------------------------
fn render_subscriptions_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let sub_title = Line::from(vec![
        Span::styled(" 󰑓 SUBSCRIPTION PROFILES ", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(
                "⟨{}/{}⟩ ",
                if app.sub_mgr.subscriptions.is_empty() { 0 } else { app.selected_sub_idx + 1 },
                app.sub_mgr.subscriptions.len()
            ),
            Style::default().fg(Theme::MUTED),
        ),
        Span::styled("│ AUTO: ", Style::default().fg(Theme::SURFACE2)),
        Span::styled(
            format!("{} ", auto_update_label(app.sub_mgr.auto_update_minutes)),
            Style::default().fg(Theme::SAPPHIRE).add_modifier(Modifier::BOLD),
        ),
    ]);

    let block = Block::default()
        .title(sub_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border());

    let now_ts = chrono::Utc::now().timestamp();
    let mut rows = Vec::new();
    for (idx, sub) in app.sub_mgr.subscriptions.iter().enumerate() {
        let is_selected = idx == app.selected_sub_idx;
        let is_updating = app.updating_subs.contains(&sub.id);
        let row_style = if is_selected {
            Theme::selected_row_focused()
        } else {
            Theme::normal_row()
        };

        let pointer = if is_selected { "❯ " } else { "  " };
        let (active_badge, active_style) = if sub.active {
            (" ● ACTIVE ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
        } else {
            (" ○ IDLE   ", Style::default().fg(Theme::MUTED))
        };

        let node_cell = if is_updating && sub.node_count == 0 {
            Span::styled("⟳ fetch...", Style::default().fg(Theme::SAPPHIRE))
        } else {
            Span::styled(
                format!("◈ {} nodes", sub.node_count),
                Style::default().fg(Theme::SAPPHIRE),
            )
        };

        let sync_cell = if is_updating {
            Span::styled("⟳ updating...", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD))
        } else {
            Span::styled(format!("󰅐 {}", sub.last_updated), Style::default().fg(Theme::MUTED))
        };

        let (traffic_text, traffic_style) = traffic_cell(sub);
        let (expiry_text, expiry_style) = expiry_cell(sub, now_ts);

        let cells = vec![
            Cell::from(Span::styled(format!("{}{}", pointer, active_badge), active_style)),
            Cell::from(format_display_name(&sub.name)),
            Cell::from(node_cell),
            Cell::from(Span::styled(traffic_text, traffic_style)),
            Cell::from(Span::styled(expiry_text, expiry_style)),
            Cell::from(sync_cell),
            Cell::from(Span::styled(sub.url.clone(), Style::default().fg(Theme::SUBTEXT0))),
        ];
        rows.push(Row::new(cells).style(row_style));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Percentage(18),
            Constraint::Length(12),
            Constraint::Length(16),
            Constraint::Length(9),
            Constraint::Length(18),
            Constraint::Percentage(26),
        ],
    )
    .header(
        Row::new(vec!["STATE", "PROFILE IDENTIFIER", "NODES", "TRAFFIC", "EXPIRY", "SYNCHRONIZED", "REMOTE URL"])
            .style(Style::default().fg(Theme::MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(block);

    app.subs_table_state.select(if app.sub_mgr.subscriptions.is_empty() { None } else { Some(app.selected_sub_idx) });
    f.render_stateful_widget(table, area, &mut app.subs_table_state);

    if app.sub_mgr.subscriptions.len() > 1 {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(Style::default().fg(Theme::SURFACE2));
        let mut scrollbar_state = ScrollbarState::new(app.sub_mgr.subscriptions.len()).position(app.selected_sub_idx);
        f.render_stateful_widget(scrollbar, area.inner(Margin { vertical: 1, horizontal: 0 }), &mut scrollbar_state);
    }
}

/// 流量列的紧凑字节格式（1.2G / 950M / 12K）
fn compact_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if b >= 10.0 * GB {
        format!("{:.0}G", b / GB)
    } else if b >= GB {
        format!("{:.1}G", b / GB)
    } else if b >= 10.0 * MB {
        format!("{:.0}M", b / MB)
    } else if b >= MB {
        format!("{:.1}M", b / MB)
    } else if b >= KB {
        format!("{:.0}K", b / KB)
    } else {
        format!("{bytes}B")
    }
}

/// 订阅流量单元格：已用/总量，达到 75% / 90% 分别变黄 / 变红
fn traffic_cell(sub: &SubscriptionItem) -> (String, Style) {
    let used = sub.upload.saturating_add(sub.download);
    if sub.total > 0 {
        let pct = ((used as f64 / sub.total as f64) * 100.0).round() as u32;
        let style = if pct >= 90 {
            Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)
        } else if pct >= 75 {
            Style::default().fg(Theme::YELLOW)
        } else {
            Style::default().fg(Theme::SUBTEXT0)
        };
        (
            format!("{}/{}", compact_bytes(used), compact_bytes(sub.total)),
            style,
        )
    } else if sub.upload > 0 || sub.download > 0 {
        (
            format!("↑{}/↓{}", compact_bytes(sub.upload), compact_bytes(sub.download)),
            Style::default().fg(Theme::SUBTEXT0),
        )
    } else {
        ("—".to_string(), Style::default().fg(Theme::SURFACE2))
    }
}

/// 订阅到期单元格：剩余天数，7 天内变黄、已过期变红
fn expiry_cell(sub: &SubscriptionItem, now_ts: i64) -> (String, Style) {
    if sub.expire <= 0 {
        return ("—".to_string(), Style::default().fg(Theme::SURFACE2));
    }
    let days = (sub.expire - now_ts).div_euclid(86_400);
    if days < 0 {
        (
            "EXPIRED".to_string(),
            Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD),
        )
    } else if days == 0 {
        ("TODAY".to_string(), Style::default().fg(Theme::RED))
    } else if days <= 7 {
        (format!("{days}d"), Style::default().fg(Theme::YELLOW))
    } else if days <= 30 {
        (format!("{days}d"), Style::default().fg(Theme::SUBTEXT0))
    } else {
        (format!("{days}d"), Style::default().fg(Theme::MUTED))
    }
}

// ----------------------------------------------------------------------------
// 6. Tab 5: Domain Rules View
// ----------------------------------------------------------------------------
fn render_rules_tab(f: &mut Frame, app: &mut App, area: Rect) {
    let rules_title = Line::from(vec![
        Span::styled(" 󰃢 PROXY DOMAIN ROUTING MATRIX ", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!(
                "⟨{}/{}⟩ ",
                if app.rule_mgr.rules.is_empty() { 0 } else { app.selected_rule_idx + 1 },
                app.rule_mgr.rules.len()
            ),
            Style::default().fg(Theme::MUTED),
        ),
        Span::styled("│ PROXY GROUP: ", Style::default().fg(Theme::SURFACE2)),
        Span::styled(format!("[{}] ", app.rule_mgr.target_hint), Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
    ]);

    let block = Block::default()
        .title(rules_title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border());

    let mut rows = Vec::new();
    for (idx, rule) in app.rule_mgr.rules.iter().enumerate() {
        let is_selected = idx == app.selected_rule_idx;
        let row_style = if is_selected {
            Theme::selected_row_focused()
        } else {
            Theme::normal_row()
        };

        let pointer = if is_selected { "❯ " } else { "  " };
        let (rule_color, rule_desc) = match rule.rule_type.as_str() {
            "DOMAIN" => (Theme::MAUVE, "exact hostname"),
            "DOMAIN-SUFFIX" => (Theme::GREEN, "wildcard suffix"),
            "DOMAIN-KEYWORD" => (Theme::PEACH, "keyword match"),
            "IP-CIDR" => (Theme::TEAL, "ip / cidr range"),
            "GEOSITE" => (Theme::RED, "geosite code"),
            "RULE-SET" => (Theme::YELLOW, "ruleset name"),
            "PROCESS" => (Theme::BLUE, "process name"),
            _ => (Theme::TEXT, "generic"),
        };

        let (target_text, target_color) = match (&rule.target, &rule.group) {
            (RuleTarget::Direct, _) => ("➜ DIRECT".to_string(), Theme::YELLOW),
            (RuleTarget::Proxy, Some(group)) => (format!("➜ {}", group), Theme::TEAL),
            (RuleTarget::Proxy, None) => ("➜ PROXY".to_string(), Theme::TEAL),
        };

        let cells = vec![
            Cell::from(format!("{}{}", pointer, rule.rule_type))
                .style(Style::default().fg(rule_color).add_modifier(Modifier::BOLD)),
            Cell::from(rule.value.clone()),
            Cell::from(Span::styled(rule_desc, Style::default().fg(Theme::MUTED))),
            Cell::from(Span::styled(target_text, Style::default().fg(target_color).add_modifier(Modifier::BOLD))),
        ];
        rows.push(Row::new(cells).style(row_style));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(22),
            Constraint::Percentage(48),
            Constraint::Length(18),
            Constraint::Length(12),
        ],
    )
    .header(
        Row::new(vec!["RULE TYPE", "DOMAIN PATTERN / KEYWORD", "MATCH STRATEGY", "ROUTE TARGET"])
            .style(Style::default().fg(Theme::MUTED).add_modifier(Modifier::BOLD)),
    )
    .block(block);

    app.rules_table_state.select(if app.rule_mgr.rules.is_empty() {
        None
    } else {
        Some(app.selected_rule_idx)
    });
    f.render_stateful_widget(table, area, &mut app.rules_table_state);

    if app.rule_mgr.rules.len() > 1 {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("▲"))
            .end_symbol(Some("▼"))
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .style(Style::default().fg(Theme::SURFACE2));
        let mut scrollbar_state =
            ScrollbarState::new(app.rule_mgr.rules.len()).position(app.selected_rule_idx);
        f.render_stateful_widget(
            scrollbar,
            area.inner(Margin { vertical: 1, horizontal: 0 }),
            &mut scrollbar_state,
        );
    }
}

// ----------------------------------------------------------------------------
// 7. Tab 4: Settings View (Cyberpunk Hardware & Daemon Control Deck)
// ----------------------------------------------------------------------------
fn render_settings_tab(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Length(11), Constraint::Min(4)])
        .split(area);

    // Card 1: Mihomo Core Daemon Telemetry
    let core_block = Block::default()
        .title(Span::styled(" ⚙ CORE ENGINE & DAEMON TELEMETRY ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::inactive_border());

    let mut binary_line = vec![
        Span::styled("◈ Binary Image:      ", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(
            absolute_path_display(&app.core_mgr.bin_path),
            Style::default().fg(Theme::TEXT),
        ),
    ];
    if !app.core_mgr.is_installed() {
        binary_line.push(Span::styled(
            "  [✘ MISSING]",
            Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD),
        ));
    }

    let disk_version = if app.kernel_bin_version == "Unknown" {
        "未知版本".to_string()
    } else {
        format!("v{}", app.kernel_bin_version)
    };
    let (kernel_update_text, kernel_update_style) = if app.kernel_busy {
        (
            format!("[{:>3}%] 正在下载并更新...", app.kernel_progress),
            Style::default()
                .fg(Theme::YELLOW)
                .add_modifier(Modifier::BOLD),
        )
    } else if let Some(latest) = &app.kernel_latest {
        if kernelupdate::is_newer(latest, Some(&app.kernel_bin_version)) {
            (
                format!("v{latest} 可更新（本地 {disk_version}）  [K] 立即更新"),
                Style::default()
                    .fg(Theme::YELLOW)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                format!("v{latest} 已是最新  [K] 重新检查"),
                Style::default().fg(Theme::MUTED),
            )
        }
    } else if app.core_mgr.is_installed() {
        (
            format!("{disk_version} · [K] 检查更新"),
            Style::default().fg(Theme::SUBTEXT0),
        )
    } else {
        (
            "[K] 下载并安装 Mihomo 内核".to_string(),
            Style::default()
                .fg(Theme::YELLOW)
                .add_modifier(Modifier::BOLD),
        )
    };

    let (app_update_text, app_update_style) = if app.app_busy {
        (
            format!("[{:>3}%] 正在下载 clash-tui 更新...", app.app_progress),
            Style::default()
                .fg(Theme::YELLOW)
                .add_modifier(Modifier::BOLD),
        )
    } else if let Some(latest) = &app.app_latest {
        if appupdate::is_newer(latest, appupdate::VERSION) {
            (
                format!("v{latest} 可更新（当前 {}）  [U] 立即更新", app.app_version),
                Style::default()
                    .fg(Theme::YELLOW)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                format!("{} 已是最新  [U] 重新检查", app.app_version),
                Style::default().fg(Theme::MUTED),
            )
        }
    } else {
        (
            format!("{} · [U] 检查更新", app.app_version),
            Style::default().fg(Theme::SUBTEXT0),
        )
    };

    let core_lines = vec![
        Line::from(binary_line),
        Line::from(vec![
            Span::styled("◈ Engine Version:    ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(format!("Mihomo {}", app.core_version), Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("◈ Kernel Binary:    ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(kernel_update_text, kernel_update_style),
        ]),
        Line::from(vec![
            Span::styled("◈ Clash TUI:        ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(app_update_text, app_update_style),
        ]),
        Line::from(vec![
            Span::styled("◈ Daemon Lifecycle:  ", Style::default().fg(Theme::SUBTEXT0)),
            if app.is_connected || app.is_core_running {
                Span::styled("[● ACTIVE RUNNING] Background Daemon", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("[○ STOPPED / INACTIVE]", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD))
            },
        ]),
        Line::from(vec![
            Span::styled("◈ Autostart Service: ", Style::default().fg(Theme::SUBTEXT0)),
            if app.core_mgr.is_autostart_enabled() {
                Span::styled("[● ENABLED ] Active on system logon", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("[○ DISABLED] Inactive (Run: clash-tui autostart on)", Style::default().fg(Theme::MUTED))
            },
        ]),
        Line::from(vec![
            Span::styled("◈ Process Control:   ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled("[s] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
            Span::raw("Terminate Process   "),
            Span::styled("[S] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::raw("Restart Core Daemon"),
        ]),
    ];
    f.render_widget(Paragraph::new(core_lines).block(core_block), chunks[0]);

    // Card 2: Network Interface & Proxy Routing Controls
    let net_block = Block::default()
        .title(Span::styled(" 󰖟 NETWORK INTERFACE & PROXY SWITCHES ", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::inactive_border());

    let (sys_switch, sys_style) = if app.sys_proxy_enabled {
        (format!("[■ ENABLED ] ➜ 127.0.0.1:{}", app.mixed_port), Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
    } else {
        ("[  DISABLED] Offline".to_string(), Style::default().fg(Theme::MUTED))
    };

    let (env_switch, env_style) = if app.env_proxy_enabled {
        (format!("[■ ENABLED ] ➜ HKCU + env.bat/nu/ps1 (:{})", app.mixed_port), Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD))
    } else {
        ("[  DISABLED] Clean Environment".to_string(), Style::default().fg(Theme::MUTED))
    };

    let (tun_switch, tun_style) = if app.tun_enabled {
        ("[■ ENABLED ] ➜ Virtual NIC Active".to_string(), Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
    } else {
        ("[  DISABLED] Inactive (Requires Admin / Root)".to_string(), Style::default().fg(Theme::MUTED))
    };

    #[cfg(target_os = "windows")]
    let platform_name = "Windows Subsystem (WinINet API + Registry Hook)";
    #[cfg(target_os = "macos")]
    let platform_name = "macOS Subsystem (networksetup CLI Hook)";
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let platform_name = "Linux Generic (Desktop Environment Hooks)";

    let dns_settings = &app.dns_mgr.settings;
    let (dns_mode, dns_mode_style) = if dns_settings.has_overrides() {
        (
            "[■ CUSTOM ]",
            Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD),
        )
    } else {
        ("[○ INHERIT]", Style::default().fg(Theme::MUTED))
    };
    let dns_nameservers = if dns_settings.nameserver.is_empty() {
        "subscription nameservers".to_string()
    } else {
        dns_settings.nameserver.join(", ")
    };
    let auto_direct_count = app
        .rule_mgr
        .rules
        .iter()
        .filter(|r| {
            r.target == RuleTarget::Direct
                && (r.rule_type == "DOMAIN" || r.rule_type == "DOMAIN-SUFFIX")
        })
        .count();

    let overrides_settings = &app.overrides_mgr.settings;
    let (overrides_mode, overrides_mode_style) = if overrides_settings.has_overrides() {
        (
            "[■ CUSTOM ]",
            Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD),
        )
    } else {
        ("[○ INHERIT]", Style::default().fg(Theme::MUTED))
    };

    let net_lines = vec![
        Line::from(vec![
            Span::styled("◈ Platform Driver:   ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(platform_name, Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("◈ DNS Nameservers:   ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(dns_mode, dns_mode_style),
            Span::styled(format!(" {}", dns_nameservers), Style::default().fg(Theme::TEXT)),
            Span::styled("  [d] Edit DNS Config", Style::default().fg(Theme::YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("◈ DIRECT DNS Policy: ", Style::default().fg(Theme::SUBTEXT0)),
            if dns_settings.direct_use_system_dns {
                Span::styled(
                    "[SYSTEM DNS]",
                    Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("[INHERIT   ]", Style::default().fg(Theme::MUTED))
            },
            Span::styled(
                format!(
                    "  {} user policies · {} DIRECT auto (system)",
                    dns_settings.policies.len(),
                    auto_direct_count
                ),
                Style::default().fg(Theme::SUBTEXT0),
            ),
        ]),
        Line::from(vec![
            Span::styled("◈ Runtime Overrides: ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(overrides_mode, overrides_mode_style),
            Span::styled(
                format!(" {}", overrides_settings.summary()),
                Style::default().fg(Theme::TEXT),
            ),
            Span::styled("  [C] Edit Config", Style::default().fg(Theme::YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("◈ System Proxy:      ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(sys_switch, sys_style),
            Span::styled("  [p] Toggle", Style::default().fg(Theme::YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("◈ Environment Proxy: ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(env_switch, env_style),
            Span::styled("  [e] Toggle", Style::default().fg(Theme::YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("◈ TUN Virtual NIC:   ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(tun_switch, tun_style),
            Span::styled("  [n] Toggle", Style::default().fg(Theme::YELLOW)),
        ]),
        Line::from(vec![
            Span::styled("◈ Mixed Proxy Port:  ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(format!("[PORT: {}]", app.mixed_port), Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("  [P] Reconfigure Port", Style::default().fg(Theme::PEACH)),
        ]),
        Line::from(vec![
            Span::styled("◈ UAC Escalation:    ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled("[A] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
            Span::styled("Restart Mihomo with Administrator Rights (UAC Prompt)", Style::default().fg(Theme::RED)),
        ]),
    ];
    f.render_widget(Paragraph::new(net_lines).block(net_block), chunks[1]);

    // Card 3: Runtime Architecture Specs
    let help_block = Block::default()
        .title(Span::styled(" 󰋖 RUNTIME SPECS & ARCHITECTURE ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::inactive_border());
    let help_lines = vec![
        Line::from(vec![
            Span::styled("◈ Engine: ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled("Rust 2021 + Ratatui + Crossterm + Tokio Async Event-Loop", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("◈ Design: ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled("Ultra-lightweight native binary with 0ms web engine latency & zero webview overhead.", Style::default().fg(Theme::MUTED)),
        ]),
    ];
    f.render_widget(Paragraph::new(help_lines).block(help_block), chunks[2]);
}

// ----------------------------------------------------------------------------
// 8. Modals (Holographic Cyber Dialogs)
// ----------------------------------------------------------------------------
fn render_add_sub_modal(f: &mut Frame, modal: &crate::app::AddSubModal) {
    let area = centered_rect(64, 40, f.area());
    let editing = modal.editing_id.is_some();
    let title = if editing {
        " 󰑓 EDIT SUBSCRIPTION PROFILE "
    } else {
        " 󰑓 ADD SUBSCRIPTION PROFILE "
    };
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD),
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
        .title(" Profile Name ")
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
    let action = if editing { "Save" } else { "Save & Synchronize" };
    let hint_line = Line::from(vec![
        Span::styled("[Tab] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
        Span::raw("Next Field   "),
        Span::styled("[Enter] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
        Span::raw(format!("{action}   ")),
        Span::styled("[Esc] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
        Span::raw("Dismiss"),
    ]);
    f.render_widget(Paragraph::new(hint_line).alignment(Alignment::Center), chunks[2]);
}

fn render_add_rule_modal(f: &mut Frame, modal: &crate::app::AddRuleModal, mgr: &crate::rules::RuleManager) {
    let area = centered_rect(62, 38, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " 󰃢 ADD ROUTE RULE ",
            Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border());

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Hint
            Constraint::Length(1), // Target selector
            Constraint::Length(3), // Input box
            Constraint::Length(4), // Help
        ])
        .split(inner);

    let hint = Paragraph::new("Target domain / keyword / CIDR (injected at the top of rules):")
        .style(Style::default().fg(Theme::SUBTEXT0));
    f.render_widget(hint, chunks[0]);

    // 去向选择器：Tab / ← / → 循环 DIRECT → 默认代理组 → 各可选策略组
    let (target_label, target_color, target_desc) = match (&modal.target, &modal.group) {
        (RuleTarget::Direct, _) => (
            "DIRECT".to_string(),
            Theme::YELLOW,
            "直连，不经过代理".to_string(),
        ),
        (RuleTarget::Proxy, Some(group)) => (
            format!("PROXY: {}", group),
            Theme::TEAL,
            format!("走指定代理组 [{}]", group),
        ),
        (RuleTarget::Proxy, None) => (
            "PROXY (auto)".to_string(),
            Theme::TEAL,
            format!("走默认代理组 [{}]", mgr.target_hint),
        ),
    };
    let target_line = Line::from(vec![
        Span::styled(" TARGET ", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled("◀ ", Style::default().fg(Theme::MUTED)),
        Span::styled(
            format!(" {} ", target_label),
            Style::default().fg(target_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" ▶  ", Style::default().fg(Theme::MUTED)),
        Span::styled(target_desc, Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(
            format!("  [{} groups]", mgr.groups.len()),
            Style::default().fg(Theme::MUTED),
        ),
    ]);
    f.render_widget(Paragraph::new(target_line), chunks[1]);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::CYAN));
    let input_text = Line::from(vec![
        Span::raw(" "),
        Span::styled(modal.input.as_str(), Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD)),
        Span::styled("█", Style::default().fg(Theme::CYAN)),
    ]);
    f.render_widget(Paragraph::new(input_text).block(input_block), chunks[2]);

    let help_text = vec![
        Line::from(vec![
            Span::styled("example.com", Style::default().fg(Theme::GREEN)),
            Span::raw(" → 后缀 │ "),
            Span::styled("full:host", Style::default().fg(Theme::MAUVE)),
            Span::raw(" → 精确 │ "),
            Span::styled("keyword:xxx", Style::default().fg(Theme::PEACH)),
            Span::raw(" → 关键字"),
        ]),
        Line::from(vec![
            Span::styled("ip-cidr:10.0.0.0/8", Style::default().fg(Theme::TEAL)),
            Span::raw(" │ "),
            Span::styled("geosite:cn", Style::default().fg(Theme::RED)),
            Span::raw(" │ "),
            Span::styled("rule-set:proxy", Style::default().fg(Theme::YELLOW)),
            Span::raw(" │ "),
            Span::styled("process:chrome", Style::default().fg(Theme::BLUE)),
        ]),
        Line::from(vec![
            Span::styled("direct:xxx", Style::default().fg(Theme::YELLOW)),
            Span::raw(" 强制直连 │ "),
            Span::styled("proxy:xxx", Style::default().fg(Theme::TEAL)),
            Span::raw(" 强制走默认代理组（前缀优先于 TARGET）"),
        ]),
        Line::from(vec![
            Span::styled("[Tab]", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::raw(" 循环目标  "),
            Span::styled("[Enter]", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
            Span::raw(" 保存并热重载  "),
            Span::styled("[Esc]", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
            Span::raw(" 取消"),
        ]),
    ];
    f.render_widget(Paragraph::new(help_text).alignment(Alignment::Center), chunks[3]);
}

// ----------------------------------------------------------------------------
// Rule Match Probe modal
// ----------------------------------------------------------------------------
fn render_rule_test_modal(f: &mut Frame, modal: &crate::app::RuleTestModal) {
    let area = centered_rect(66, 30, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " 󰒍 RULE MATCH PROBE ",
            Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border());

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Hint
            Constraint::Length(3), // Input box
            Constraint::Length(1), // Spacer
            Constraint::Min(2),    // Result
            Constraint::Length(1), // Help
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new("Probe a domain against data/config.yaml (top-down priority):")
            .style(Style::default().fg(Theme::SUBTEXT0)),
        chunks[0],
    );

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::CYAN));
    let input_text = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            modal.input.as_str(),
            Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("█", Style::default().fg(Theme::CYAN)),
    ]);
    f.render_widget(Paragraph::new(input_text).block(input_block), chunks[1]);

    let (result_line, result_style) = match &modal.report {
        None => (
            "按 Enter 推演：本地只理解域名类规则与 MATCH".to_string(),
            Style::default().fg(Theme::MUTED),
        ),
        Some(report) => match &report.outcome {
            ProbeOutcome::Match { index, rule } => (
                format!("✔ 命中第 {} 条规则：{}", index + 1, rule),
                Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD),
            ),
            ProbeOutcome::KernelOnly { index, rule } => (
                format!("⚠ 第 {} 条规则需要内核解析，无法本地确定：{}", index + 1, rule),
                Style::default().fg(Theme::YELLOW),
            ),
            ProbeOutcome::NoMatch => (
                format!(
                    "○ 未命中任何规则（已扫描 {} 条），将使用 mihomo 的默认策略",
                    report.scanned
                ),
                Style::default().fg(Theme::SUBTEXT0),
            ),
        },
    };

    let result = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(result_line, result_style)),
        Line::from(Span::styled(
            "提示：IP-CIDR / RULE-SET / GEOSITE / PROCESS 由内核按真实网络/进程状态匹配，本地无法推演。",
            Style::default().fg(Theme::MUTED),
        )),
    ]))
    .wrap(Wrap { trim: true });
    f.render_widget(result, chunks[3]);

    let help = Line::from(vec![
        Span::styled("[Enter]", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
        Span::raw(" 推演  "),
        Span::styled("[Esc]", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
        Span::raw(" 关闭  "),
        Span::styled("输入直接覆盖", Style::default().fg(Theme::MUTED)),
    ])
    .alignment(Alignment::Center);
    f.render_widget(Paragraph::new(help), chunks[4]);
}

fn render_dns_modal(f: &mut Frame, modal: &crate::app::DnsModal) {
    let area = centered_rect(74, 74, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " 󰖟 DNS OVERRIDES & POLICY EDITOR ",
            Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border());

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Hint
            Constraint::Length(3), // DIRECT toggle
            Constraint::Length(3), // nameserver
            Constraint::Length(3), // fallback
            Constraint::Length(3), // proxy-server-nameserver
            Constraint::Length(3), // default-nameserver
            Constraint::Length(3), // domain policies
            Constraint::Length(2), // Help
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new("覆盖订阅 DNS 字段（留空 = 沿用订阅值）：")
            .style(Style::default().fg(Theme::SUBTEXT0)),
        chunks[0],
    );

    let toggle_style = if modal.active_field == 0 {
        Theme::active_border()
    } else {
        Theme::inactive_border()
    };
    let toggle_block = Block::default()
        .title(" DIRECT DNS POLICY ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(toggle_style);
    let toggle_line = if modal.direct_system {
        Line::from(vec![
            Span::styled(
                " [■ ENABLED ] ",
                Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "DIRECT 规则命中域名使用 system DNS（内网域名必需）  ",
                Style::default().fg(Theme::TEXT),
            ),
            Span::styled("[Space] Toggle", Style::default().fg(Theme::YELLOW)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" [  DISABLED] ", Style::default().fg(Theme::MUTED)),
            Span::styled(
                "沿用订阅 DNS（内网域名可能 NXDOMAIN）  ",
                Style::default().fg(Theme::SUBTEXT0),
            ),
            Span::styled("[Space] Toggle", Style::default().fg(Theme::YELLOW)),
        ])
    };
    f.render_widget(Paragraph::new(toggle_line).block(toggle_block), chunks[1]);

    render_dns_input(
        f,
        chunks[2],
        " Nameserver [逗号分隔] ",
        &modal.nameserver,
        modal.active_field == 1,
    );
    render_dns_input(
        f,
        chunks[3],
        " Fallback [逗号分隔] ",
        &modal.fallback,
        modal.active_field == 2,
    );
    render_dns_input(
        f,
        chunks[4],
        " Proxy Server Nameserver [逗号分隔] ",
        &modal.proxy_nameserver,
        modal.active_field == 3,
    );
    render_dns_input(
        f,
        chunks[5],
        " Default Nameserver [逗号分隔] ",
        &modal.default_nameserver,
        modal.active_field == 4,
    );
    render_dns_input(
        f,
        chunks[6],
        " Domain Policies（域名=服务器; 多条用分号） ",
        &modal.policies,
        modal.active_field == 5,
    );

    let help_text = vec![
        Line::from(vec![
            Span::styled("[Tab/↓] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::raw("下一字段  "),
            Span::styled("[Space] ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)),
            Span::raw("切换 DIRECT 策略  "),
            Span::styled("[Enter] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
            Span::raw("保存并热重载  "),
            Span::styled("[Esc] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
            Span::raw("取消"),
        ]),
        Line::from(vec![
            Span::styled("示例：", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled("+.corp.example=192.168.121.6", Style::default().fg(Theme::GREEN)),
            Span::raw("   ·   nameserver: "),
            Span::styled(
                "system, 223.6.6.6, https://doh.pub/dns-query",
                Style::default().fg(Theme::MAUVE),
            ),
        ]),
    ];
    f.render_widget(
        Paragraph::new(help_text).alignment(Alignment::Center),
        chunks[7],
    );
}

fn render_dns_input(f: &mut Frame, area: Rect, title: &str, value: &str, active: bool) {
    let border_style = if active {
        Theme::active_border()
    } else {
        Theme::inactive_border()
    };
    let block = Block::default()
        .title(title.to_string())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style);
    let text = if active {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(
                value,
                Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled("█", Style::default().fg(Theme::CYAN)),
        ])
    } else {
        Line::from(Span::styled(value.to_string(), Style::default().fg(Theme::TEXT)))
    };
    f.render_widget(Paragraph::new(text).block(block), area);
}

fn render_health_modal(f: &mut Frame, modal: &crate::app::HealthModal) {
    let screen = f.area();
    let width = 78.min(screen.width.saturating_sub(4)).max(40);
    let height = (modal.items.len() as u16 + 8)
        .min(screen.height.saturating_sub(2))
        .max(10);
    let area = Rect {
        x: screen.x + screen.width.saturating_sub(width) / 2,
        y: screen.y + screen.height.saturating_sub(height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, area);

    let title = if modal.generated.is_empty() {
        " ◈ KERNEL HEALTH CHECK · 检查中 ".to_string()
    } else {
        format!(" ◈ KERNEL HEALTH CHECK · {} ", modal.generated)
    };
    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(Theme::SAPPHIRE).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border())
        .style(Style::default().bg(Theme::MANTLE));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);

    let mut lines: Vec<Line> = Vec::new();
    if modal.loading {
        lines.push(Line::from(Span::styled(
            "  正在收集检查项（进程 / REST / 版本 / 端口）...",
            Style::default().fg(Theme::SAPPHIRE),
        )));
    }
    for item in &modal.items {
        let (icon, icon_style) = match item.ok {
            Some(true) => ("  ✔ ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
            Some(false) => ("  ✘ ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
            None => ("  · ", Style::default().fg(Theme::MUTED)),
        };
        lines.push(Line::from(vec![
            Span::styled(icon, icon_style),
            Span::styled(
                format!(" {:<14}", item.label),
                Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(item.detail.clone(), Style::default().fg(Theme::SUBTEXT0)),
        ]));
    }
    f.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: true }),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  Esc 关闭 · H / r 重新检查",
            Style::default().fg(Theme::MUTED),
        ))),
        chunks[1],
    );
}

fn render_logs_modal(f: &mut Frame, modal: &crate::app::LogModal) {
    let screen = f.area();
    let width = 110.min(screen.width.saturating_sub(4)).max(40);
    let height = screen.height.saturating_sub(4).max(12);
    let area = Rect {
        x: screen.x + screen.width.saturating_sub(width) / 2,
        y: screen.y + screen.height.saturating_sub(height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, area);

    let title = if modal.generated.is_empty() {
        format!(" ◈ KERNEL LOG TAIL · {} ", crate::diagnostics::LOG_PATH)
    } else {
        format!(
            " ◈ KERNEL LOG TAIL · {} · {} ",
            crate::diagnostics::LOG_PATH,
            modal.generated
        )
    };
    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border())
        .style(Style::default().bg(Theme::MANTLE));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(inner);

    let total = modal.lines.len();
    let filter_line = Line::from(vec![
        Span::styled(
            "  FILTER  ",
            Style::default().fg(Theme::SAPPHIRE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            modal.filter.clone(),
            Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("█", Style::default().fg(Theme::CYAN)),
        Span::styled(
            format!("   (共 {} 行，Enter 重新读取)", total),
            Style::default().fg(Theme::MUTED),
        ),
    ]);
    f.render_widget(Paragraph::new(filter_line), chunks[0]);

    let mut lines: Vec<Line> = Vec::new();
    if modal.loading {
        lines.push(Line::from(Span::styled(
            "  正在读取日志尾部...",
            Style::default().fg(Theme::SAPPHIRE),
        )));
    } else if let Some(err) = &modal.error {
        lines.push(Line::from(Span::styled(
            format!("  ✘ {}", err),
            Style::default().fg(Theme::RED),
        )));
    } else {
        let matched = crate::diagnostics::filter_lines(&modal.lines, &modal.filter, 300);
        if matched.is_empty() {
            lines.push(Line::from(Span::styled(
                "  (无匹配日志)",
                Style::default().fg(Theme::MUTED),
            )));
        }
        for line in matched {
            let style = match crate::diagnostics::classify_log_line(&line) {
                crate::diagnostics::LogLevel::Error => {
                    Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)
                }
                crate::diagnostics::LogLevel::Warn => Style::default().fg(Theme::YELLOW),
                crate::diagnostics::LogLevel::Info => Style::default().fg(Theme::SUBTEXT0),
                crate::diagnostics::LogLevel::Other => Style::default().fg(Theme::MUTED),
            };
            lines.push(Line::from(Span::styled(line, style)));
        }
    }
    f.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
        chunks[1],
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "  输入关键字过滤 · Backspace 删除 · Enter 重新读取 · Esc 关闭",
            Style::default().fg(Theme::MUTED),
        ))),
        chunks[2],
    );
}

// ----------------------------------------------------------------------------
// Runtime overrides modal (Settings → C)
// ----------------------------------------------------------------------------
fn overrides_option_label(value: Option<&str>) -> String {
    match value {
        None => "INHERIT".to_string(),
        Some(v) => v.to_ascii_uppercase(),
    }
}

fn overrides_bool_label(value: Option<bool>) -> String {
    match value {
        None => "INHERIT".to_string(),
        Some(true) => "ON".to_string(),
        Some(false) => "OFF".to_string(),
    }
}

fn render_overrides_field(
    f: &mut Frame,
    area: Rect,
    label: &str,
    value: &str,
    hint: &str,
    active: bool,
) {
    let border = if active {
        Style::default().fg(Theme::CYAN)
    } else {
        Theme::inactive_border()
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border);
    let value_style = if active {
        Style::default()
            .fg(Theme::CYAN)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::TEXT)
    };
    let line = Line::from(vec![
        Span::raw(" "),
        Span::styled(format!("{label:<14}"), Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(
            if active {
                format!("▶ {value} ◀")
            } else {
                format!("  {value}  ")
            },
            value_style,
        ),
        Span::styled(format!("  {hint}"), Style::default().fg(Theme::MUTED)),
    ]);
    f.render_widget(Paragraph::new(line).block(block), area);
}

fn render_overrides_modal(f: &mut Frame, modal: &crate::app::OverridesModal) {
    let screen = f.area();
    let width = 72.min(screen.width.saturating_sub(4)).max(40);
    let height = 21.min(screen.height.saturating_sub(4)).max(12);
    let area = Rect {
        x: screen.x + screen.width.saturating_sub(width) / 2,
        y: screen.y + screen.height.saturating_sub(height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " ⚙ RUNTIME OVERRIDES · data/config.yaml ",
            Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border())
        .style(Style::default().bg(Theme::MANTLE));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // hint
            Constraint::Length(3), // mode
            Constraint::Length(3), // log-level
            Constraint::Length(3), // allow-lan
            Constraint::Length(3), // ipv6
            Constraint::Length(3), // unified-delay
            Constraint::Length(2), // help
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(
            " Space / ← → 循环取值；INHERIT 表示沿用订阅配置（不写覆盖项）",
        )
        .style(Style::default().fg(Theme::SUBTEXT0)),
        chunks[0],
    );

    let rows = [
        (
            "mode",
            overrides_option_label(modal.mode.as_deref()),
            "rule / global / direct",
        ),
        (
            "log-level",
            overrides_option_label(modal.log_level.as_deref()),
            "silent / error / warning / info / debug",
        ),
        (
            "allow-lan",
            overrides_bool_label(modal.allow_lan),
            "允许局域网设备使用代理",
        ),
        (
            "ipv6",
            overrides_bool_label(modal.ipv6),
            "内核 IPv6 支持",
        ),
        (
            "unified-delay",
            overrides_bool_label(modal.unified_delay),
            "统一计算直连 / 代理延迟",
        ),
    ];
    for (idx, (label, value, hint)) in rows.iter().enumerate() {
        render_overrides_field(
            f,
            chunks[idx + 1],
            label,
            value,
            hint,
            idx == modal.active_field,
        );
    }

    let help = vec![
        Line::from(vec![
            Span::styled(
                "[Space] ",
                Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD),
            ),
            Span::raw("切换取值  "),
            Span::styled(
                "[Tab/↓] ",
                Style::default()
                    .fg(Theme::LAVENDER)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("下一项  "),
            Span::styled(
                "[Enter] ",
                Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD),
            ),
            Span::raw("保存并热重载  "),
            Span::styled(
                "[Esc] ",
                Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD),
            ),
            Span::raw("取消"),
        ]),
        Line::from(Span::styled(
            " 保存后写入 data/overrides.json，并在 apply_rules_to_config() 中覆盖订阅同名键",
            Style::default().fg(Theme::MUTED),
        )),
    ];
    f.render_widget(Paragraph::new(help), chunks[6]);
}

fn render_backup_modal(f: &mut Frame, modal: &crate::app::BackupModal) {
    let screen = f.area();
    let width = 82.min(screen.width.saturating_sub(4)).max(50);
    let height = (modal.entries.len() as u16 + 11)
        .min(screen.height.saturating_sub(4))
        .max(12);
    let area = Rect {
        x: screen.x + screen.width.saturating_sub(width) / 2,
        y: screen.y + screen.height.saturating_sub(height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, area);

    let title = format!(
        " ◈ LOCAL CONFIG BACKUPS · {} ",
        crate::backup::BACKUP_DIR
    );
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border())
        .style(Style::default().bg(Theme::MANTLE));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(2),
    ])
    .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " 备份含 config.yaml / dns.json / overrides.json / rules.json（不含订阅与 profiles）",
            Style::default().fg(Theme::MUTED),
        ))),
        chunks[0],
    );

    let mut lines: Vec<Line> = Vec::new();
    if modal.entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "  还没有备份，按 [n] 立即创建",
            Style::default().fg(Theme::MUTED),
        )));
    }
    for (i, entry) in modal.entries.iter().enumerate() {
        let selected = i == modal.selected;
        let marker = if selected { "❯" } else { " " };
        let row_style = if selected {
            Theme::selected_row_focused()
        } else {
            Theme::normal_row()
        };
        lines.push(
            Line::from(vec![
                Span::styled(
                    format!("{marker} {:>2}. ", i + 1),
                    Style::default().fg(Theme::MAUVE),
                ),
                Span::styled(
                    entry.name.clone(),
                    Style::default()
                        .fg(if selected { Theme::TEXT } else { Theme::SUBTEXT0 })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("   "),
                Span::styled(
                    format!("{} 个文件 · {}", entry.files, crate::backup::format_bytes(entry.bytes)),
                    Style::default().fg(Theme::MUTED),
                ),
            ])
            .style(row_style),
        );
    }
    f.render_widget(Paragraph::new(lines), chunks[1]);

    let bottom = if let Some(pending) = &modal.pending {
        vec![
            Line::from(Span::styled(
                format!(" ⚠ 再按 Enter 确认用 {pending} 覆盖当前配置 · Esc 取消"),
                Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                " [n] 新建备份   [Enter] 恢复选中（需二次确认）   [j/k] 选择   [Esc] 关闭",
                Style::default().fg(Theme::SUBTEXT0),
            )),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                " [n] 新建备份   [Enter] 恢复选中（需二次确认）   [j/k] 选择   [Esc] 关闭",
                Style::default().fg(Theme::SUBTEXT0),
            )),
            Line::from(Span::styled(
                " 恢复后会自动重新注入并热重载；如覆盖了配置，内核会立即应用",
                Style::default().fg(Theme::MUTED),
            )),
        ]
    };
    f.render_widget(Paragraph::new(bottom), chunks[2]);
}

/// 把采样序列渲染为块字符曲线（`scale` 为本次运行峰值，0 表示全空）
fn spark_bytes(values: &[u64], width: usize, scale: u64) -> String {
    const LEVELS: [char; 8] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇'];
    if values.is_empty() || width == 0 {
        return String::new();
    }
    let take = values.len().min(width);
    values[values.len() - take..]
        .iter()
        .map(|v| {
            if scale == 0 {
                return ' ';
            }
            let idx = ((*v as f64 / scale as f64) * 8.0).ceil().clamp(0.0, 8.0) as usize;
            if idx == 0 {
                ' '
            } else {
                LEVELS[idx.saturating_sub(1).min(7)]
            }
        })
        .collect()
}

fn render_traffic_modal(f: &mut Frame, app: &App) {

    let screen = f.area();
    let width = 96.min(screen.width.saturating_sub(4)).max(60);
    let height = 16.min(screen.height.saturating_sub(4)).max(10);
    let area = Rect {
        x: screen.x + screen.width.saturating_sub(width) / 2,
        y: screen.y + screen.height.saturating_sub(height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " ◈ TRAFFIC HISTORY ",
            Style::default()
                .fg(Theme::TEAL)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border())
        .style(Style::default().bg(Theme::MANTLE));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let samples = app.traffic_log.len();
    let span = samples.min(TRAFFIC_HISTORY_CAP);
    let scale = app
        .traffic_peak_up
        .max(app.traffic_peak_down)
        .max(1);
    let chart_width = inner.width.saturating_sub(2) as usize;

    let ups: Vec<u64> = app.traffic_log.iter().map(|p| p.up).collect();
    let downs: Vec<u64> = app.traffic_log.iter().map(|p| p.down).collect();
    let avg = |values: &[u64]| -> u64 {
        if values.is_empty() {
            0
        } else {
            values.iter().sum::<u64>() / values.len() as u64
        }
    };

    let heading = |icon: &str, color: Color, label: &str, now: u64, peak: u64, average: u64| {
        Line::from(vec![
            Span::styled(
                format!(" {icon} {label:<9}"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("now {:>10}   peak {:>10}   avg {:>10}", format_speed(now), format_speed(peak), format_speed(average)),
                Style::default().fg(Theme::TEXT),
            ),
        ])
    };

    let lines = vec![
        Line::from(Span::styled(
            format!(" 采样间隔 1s · 当前窗口 {span} 秒（最多 300 秒）"),
            Style::default().fg(Theme::MUTED),
        )),
        Line::from(""),
        heading("▲", Theme::PEACH, "UPLOAD", app.up_speed, app.traffic_peak_up, avg(&ups)),
        Line::from(Span::styled(
            format!("  {}", spark_bytes(&ups, chart_width, scale)),
            Style::default().fg(Theme::PEACH),
        )),
        Line::from(""),
        heading("▼", Theme::TEAL, "DOWNLOAD", app.down_speed, app.traffic_peak_down, avg(&downs)),
        Line::from(Span::styled(
            format!("  {}", spark_bytes(&downs, chart_width, scale)),
            Style::default().fg(Theme::TEAL),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" Σ 会话累计  ", Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("↑ {}   ↓ {}", format_bytes(app.total_upload), format_bytes(app.total_download)),
                Style::default().fg(Theme::TEXT),
            ),
        ]),
        Line::from(Span::styled(
            " 曲线按本次运行内的峰值自动缩放；清空后从下一个采样点重新积累",
            Style::default().fg(Theme::MUTED),
        )),
        Line::from(Span::styled(
            " [c] 清空历史   [Esc] 关闭",
            Style::default().fg(Theme::SUBTEXT0),
        )),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn render_providers_modal(f: &mut Frame, modal: &crate::app::ProvidersModal) {
    use crate::providers::ProviderKind;

    fn clip(text: &str, width: usize) -> String {
        let count = text.chars().count();
        if count <= width {
            text.to_string()
        } else {
            let mut s: String = text.chars().take(width.saturating_sub(1)).collect();
            s.push('…');
            s
        }
    }

    let screen = f.area();
    let kind = modal.kind;
    let items: &[crate::providers::ProviderInfo] = match kind {
        ProviderKind::Proxy => &modal.snapshot.proxies,
        ProviderKind::Rule => &modal.snapshot.rules,
    };
    let width = 104.min(screen.width.saturating_sub(4)).max(60);
    let height = (items.len() as u16 + 11)
        .min(screen.height.saturating_sub(4))
        .max(14);
    let area = Rect {
        x: screen.x + screen.width.saturating_sub(width) / 2,
        y: screen.y + screen.height.saturating_sub(height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " ◈ PROVIDERS · data/config.yaml ",
            Style::default()
                .fg(Theme::SAPPHIRE)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border())
        .style(Style::default().bg(Theme::MANTLE));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .split(inner);

    let tab = |label: &str, count: usize, active: bool| {
        let text = format!(" {label} {count} ");
        if active {
            Span::styled(
                text,
                Style::default()
                    .fg(Theme::BASE)
                    .bg(Theme::SAPPHIRE)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(text, Style::default().fg(Theme::SUBTEXT0))
        }
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::raw(" "),
                tab("PROXY PROVIDERS", modal.snapshot.proxies.len(), kind == ProviderKind::Proxy),
                Span::raw("  "),
                tab("RULE PROVIDERS", modal.snapshot.rules.len(), kind == ProviderKind::Rule),
                Span::raw("   [Tab] 切换"),
            ]),
            Line::from(Span::styled(
                " 只列出 data/config.yaml 中 proxy-providers / rule-providers 声明的条目；内核 API 返回的策略组已过滤",
                Style::default().fg(Theme::MUTED),
            )),
        ]),
        chunks[0],
    );

    let header = Line::from(vec![
        Span::styled(
            format!(" {:<34}", "NAME"),
            Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<7}", "VEHICLE"),
            Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:<10}", "BEHAVIOR"),
            Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{:>7}  ", "COUNT"),
            Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD),
        ),
        Span::styled("UPDATED", Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD)),
    ]);

    let mut lines = vec![header];
    if modal.loading {
        lines.push(Line::from(Span::styled(
            " ⟳ 正在载入 providers...",
            Style::default().fg(Theme::YELLOW),
        )));
    } else if items.is_empty() {
        lines.push(Line::from(Span::styled(
            " 当前作用域没有 provider（订阅配置里未声明）",
            Style::default().fg(Theme::MUTED),
        )));
    }
    for (i, p) in items.iter().enumerate() {
        let selected = i == modal.selected;
        let style = if selected {
            Theme::selected_row_focused()
        } else {
            Theme::normal_row()
        };
        let behavior = p.behavior.as_deref().unwrap_or("-");
        let unit = match p.kind {
            ProviderKind::Proxy => "nodes",
            ProviderKind::Rule => "rules",
        };
        lines.push(
            Line::from(vec![
                Span::styled(
                    format!(" {:<34}", clip(&p.name, 34)),
                    Style::default().fg(if selected { Theme::TEXT } else { Theme::SUBTEXT0 }),
                ),
                Span::styled(format!("{:<7}", p.vehicle), Style::default().fg(Theme::TEAL)),
                Span::styled(format!("{:<10}", behavior), Style::default().fg(Theme::SUBTEXT0)),
                Span::styled(
                    format!("{:>7}  ", format!("{} {unit}", p.count)),
                    Style::default().fg(Theme::GREEN),
                ),
                Span::styled(p.updated.clone(), Style::default().fg(Theme::MUTED)),
            ])
            .style(style),
        );
    }
    f.render_widget(Paragraph::new(lines), chunks[1]);

    let status_line = if modal.loading {
        Line::from(Span::styled(
            " 正在载入...",
            Style::default().fg(Theme::YELLOW),
        ))
    } else if let Some(message) = &modal.message {
        Line::from(Span::styled(
            format!(" {message}"),
            Style::default().fg(Theme::PEACH),
        ))
    } else {
        Line::from(Span::styled(
            format!(" {} 个 provider", items.len()),
            Style::default().fg(Theme::MUTED),
        ))
    };
    f.render_widget(
        Paragraph::new(vec![
            status_line,
            Line::from(Span::styled(
                " [Tab] Proxy/Rule   [j/k] 选择   [u] 更新选中   [r] 重新载入   [Esc] 关闭",
                Style::default().fg(Theme::SUBTEXT0),
            )),
        ]),
        chunks[2],
    );
}

fn render_edit_port_modal(f: &mut Frame, port_input: &str) {
    let area = centered_rect(52, 28, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " 󰛳 CONFIGURE MIXED PROXY PORT ",
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
        Span::raw("Dismiss"),
    ]);
    f.render_widget(Paragraph::new(action_hints).alignment(Alignment::Center), chunks[2]);
}

/// 把相对路径（相对于当前工作目录）显示为绝对路径，便于用户确认内核/配置到底在哪。
fn absolute_path_display(p: &std::path::Path) -> String {
    if p.is_absolute() {
        return p.to_string_lossy().to_string();
    }
    match std::env::current_dir() {
        Ok(cwd) => cwd.join(p).to_string_lossy().to_string(),
        Err(_) => p.to_string_lossy().to_string(),
    }
}

// ----------------------------------------------------------------------------
// 9.5 Help overlay: all key bindings (`?`)
// ----------------------------------------------------------------------------
fn help_section(title: &'static str, width: u16) -> Line<'static> {
    let accent = match title {
        "GLOBAL" => Theme::CYAN,
        "PROXIES" => Theme::GREEN,
        "CONNECTIONS" => Theme::RED,
        "SUBSCRIPTIONS" => Theme::YELLOW,
        "SETTINGS" => Theme::PEACH,
        "RULES" => Theme::MAUVE,
        "DIALOGS" => Theme::LAVENDER,
        _ => Theme::SUBTEXT0,
    };
    let fill = (width as usize).saturating_sub(title.len() + 3);
    Line::from(vec![
        Span::styled("◈ ", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
        Span::styled(
            title,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", "─".repeat(fill)),
            Style::default().fg(Theme::SURFACE2),
        ),
    ])
}

fn help_row(key: &'static str, desc: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {key:<16}"),
            Style::default()
                .fg(Theme::LAVENDER)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc, Style::default().fg(Theme::SUBTEXT0)),
    ])
}

fn help_column(sections: &[crate::keymap::Section], width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for section in sections {
        lines.push(help_section(section.title, width));
        for binding in section.bindings {
            lines.push(help_row(binding.keys, binding.desc));
        }
    }
    lines
}

fn render_help_modal(f: &mut Frame, area: Rect) {
    // 高度按键位表内容自适应（上/下各留一点背景），不再写死导致底部条目被裁。
    let body_lines = crate::keymap::line_count(crate::keymap::LEFT)
        .max(crate::keymap::line_count(crate::keymap::RIGHT));
    let width = 96.min(area.width.saturating_sub(2)).max(40);
    let height = (body_lines as u16 + 6)
        .min(area.height.saturating_sub(2))
        .max(11);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(Span::styled(
            " ◈ KEY BINDINGS — 全量快捷键 ",
            Style::default()
                .fg(Theme::LAVENDER)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::active_border())
        .style(Style::default().bg(Theme::MANTLE));

    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let inner = inner.inner(Margin {
        horizontal: 1,
        vertical: 0,
    });

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // 标题摘要 + 关闭提示
            Constraint::Length(1), // 分隔线
            Constraint::Min(0),    // 双栏按键表
            Constraint::Length(1), // 分隔线
            Constraint::Length(1), // 底部提示
        ])
        .split(inner);

    let head = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(24)])
        .split(rows[0]);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " FULL KEY REFERENCE",
                Style::default()
                    .fg(Theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "   · 按作用域分组 · 共 {} 项",
                    crate::keymap::binding_count()
                ),
                Style::default().fg(Theme::MUTED),
            ),
        ])),
        head[0],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            "? / Esc 关闭 ",
            Style::default().fg(Theme::MAUVE),
        ))
        .alignment(Alignment::Right),
        head[1],
    );

    let rule = |w: u16| {
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(w as usize),
            Style::default().fg(Theme::SURFACE2),
        )))
    };
    f.render_widget(rule(inner.width), rows[1]);

    let body = rows[2];
    let left_width = body.width.saturating_sub(1) / 2;
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(left_width),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(body);

    f.render_widget(
        Paragraph::new(help_column(crate::keymap::LEFT, columns[0].width)),
        columns[0],
    );
    let divider = vec![
        Line::from(Span::styled("│", Style::default().fg(Theme::SURFACE2)));
        body.height as usize
    ];
    f.render_widget(Paragraph::new(divider), columns[1]);
    f.render_widget(
        Paragraph::new(help_column(crate::keymap::RIGHT, columns[2].width)),
        columns[2],
    );

    f.render_widget(rule(inner.width), rows[3]);

    let key = |k: &'static str| {
        Span::styled(
            k,
            Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD),
        )
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("▪ ", Style::default().fg(Theme::MAUVE)),
            key("Tab"),
            Span::styled(" 切换页面    ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled("▪ ", Style::default().fg(Theme::MAUVE)),
            key("1 … 5"),
            Span::styled(" 直达    ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled("▪ ", Style::default().fg(Theme::MAUVE)),
            key("? / Esc / q"),
            Span::styled(" 关闭帮助", Style::default().fg(Theme::SUBTEXT0)),
        ]))
        .alignment(Alignment::Center),
        rows[4],
    );
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
// 9. Footer & Status Deck
// ----------------------------------------------------------------------------
/// 窄终端下按键提示行会溢出：按「按键 + 说明」成对从尾部丢弃，避免半截词。
fn fit_footer_line(spans: Vec<Span<'static>>, width: u16) -> Line<'static> {
    let mut spans = spans;
    while spans.len() > 2 && Line::from(spans.clone()).width() > width as usize {
        spans.pop();
        spans.pop();
    }
    Line::from(spans)
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // Toast bar or Active Tab Keymap
    if let Some(toast) = app.get_active_status() {
        let (prefix, toast_style) = if toast.starts_with('✔') {
            (" ◈ [SYSTEM NOTICE] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
        } else if toast.starts_with('✘') {
            (" ◈ [SYSTEM ALERT]  ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD))
        } else {
            (" ◈ [SYSTEM STATUS] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD))
        };

        let toast_p = Paragraph::new(Line::from(vec![
            Span::styled(prefix, toast_style),
            Span::styled(toast, toast_style),
        ]));
        f.render_widget(toast_p, chunks[0]);
    } else {
        let hint_line = match app.active_tab {
            ActiveTab::Proxies => Line::from(vec![
                Span::styled(" ◈ PROXIES: ", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
                Span::styled("[Enter] ", Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD)),
                Span::raw("Select  "),
                Span::styled("[t] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Ping  "),
                Span::styled("[T] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Ping All  "),
                Span::styled("[o] ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
                Span::raw("Sort  "),
                Span::styled("[←/→] ", Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD)),
                Span::raw("Switch Col  "),
                Span::styled("[PgUp/Dn] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("Scroll"),
            ]),
            ActiveTab::Connections => Line::from(vec![
                Span::styled(" ◈ CONNS: ", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
                Span::styled("[d] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::raw("Close Selected  "),
                Span::styled("[D] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::raw("Close All  "),
                Span::styled("[PgUp/Dn] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("Page Scroll"),
            ]),
            ActiveTab::Subscriptions => Line::from(vec![
                Span::styled(" ◈ SUBS: ", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
                Span::styled("[a] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
                Span::raw("Add  "),
                Span::styled("[e] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("Edit  "),
                Span::styled("[u] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Update  "),
                Span::styled("[U] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Update All  "),
                Span::styled("[i] ", Style::default().fg(Theme::SAPPHIRE).add_modifier(Modifier::BOLD)),
                Span::raw("Auto  "),
                Span::styled("[Enter] ", Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD)),
                Span::raw("Activate  "),
                Span::styled("[x] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::raw("Delete"),
            ]),
            ActiveTab::Settings => Line::from(vec![
                Span::styled(" ◈ SETTINGS: ", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
                Span::styled("[p] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
                Span::raw("SysProxy  "),
                Span::styled("[e] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("EnvProxy  "),
                Span::styled("[n] ", Style::default().fg(Theme::SAPPHIRE).add_modifier(Modifier::BOLD)),
                Span::raw("TUN  "),
                Span::styled("[d] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("DNS  "),
                Span::styled("[C] ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)),
                Span::raw("Config  "),
                Span::styled("[B] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Backup  "),
                Span::styled("[V] ", Style::default().fg(Theme::SAPPHIRE).add_modifier(Modifier::BOLD)),
                Span::raw("Providers  "),
                Span::styled("[Y] ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
                Span::raw("Traffic  "),
                Span::styled("[H] ", Style::default().fg(Theme::SAPPHIRE).add_modifier(Modifier::BOLD)),
                Span::raw("Health  "),
                Span::styled("[L] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("Logs  "),
                Span::styled("[K] ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
                Span::raw("Kernel  "),
                Span::styled("[U] ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)),
                Span::raw("Update  "),
                Span::styled("[P] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Port  "),
                Span::styled("[A] ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
                Span::raw("Admin  "),
                Span::styled("[s/S] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::raw("Core"),
            ]),
            ActiveTab::Rules => Line::from(vec![
                Span::styled(" ◈ RULES: ", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
                Span::styled("[a] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
                Span::raw("Add  "),
                Span::styled("[t] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Cycle Target  "),
                Span::styled("[K/J] ", Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD)),
                Span::raw("Move  "),
                Span::styled("[f] ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)),
                Span::raw("Probe  "),
                Span::styled("[x] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::raw("Delete  "),
                Span::styled("[PgUp/Dn] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("Scroll"),
            ]),
        };
        f.render_widget(
            Paragraph::new(fit_footer_line(hint_line.spans, chunks[0].width)),
            chunks[0],
        );
    }

    // Global Command Deck
    let global_keys = Line::from(vec![
        Span::styled(" [q] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
        Span::styled("Quit", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(" │ [Q] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
        Span::styled("Quit+Stop", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(" │ [Tab] ", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
        Span::styled("Next Tab", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(" │ [p] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
        Span::styled("SysProxy", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(" │ [e] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
        Span::styled("EnvProxy", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(" │ [n] ", Style::default().fg(Theme::SAPPHIRE).add_modifier(Modifier::BOLD)),
        Span::styled("TUN", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(" │ [P] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
        Span::styled("Port", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(" │ [m] ", Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD)),
        Span::styled("Mode", Style::default().fg(Theme::SUBTEXT0)),
        Span::styled(" │ [?] ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)),
        Span::styled("All Keys", Style::default().fg(Theme::SUBTEXT0)),
    ]);
    f.render_widget(
        Paragraph::new(fit_footer_line(global_keys.spans, chunks[1].width)),
        chunks[1],
    );
}

// ----------------------------------------------------------------------------
// 10. Helper: Format display name to prevent emoji & text font overlap
// ----------------------------------------------------------------------------
pub fn format_display_name(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len() + 8);
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        out.push(c);
        let u = c as u32;

        if is_emoji_or_symbol(u) {
            // Regional indicator pair (Flags like 🇭🇰, 🇺🇸)
            if is_regional_indicator(u) && i + 1 < chars.len() && is_regional_indicator(chars[i + 1] as u32) {
                i += 1;
                out.push(chars[i]);
            }

            // Include any variation selectors (FE0F / FE0E), skin tones, or ZWJ sequences
            while i + 1 < chars.len() {
                let next_u = chars[i + 1] as u32;
                if is_variation_selector(next_u) || is_skin_tone(next_u) || next_u == 0x200D {
                    i += 1;
                    out.push(chars[i]);
                    if next_u == 0x200D && i + 1 < chars.len() {
                        i += 1;
                        out.push(chars[i]);
                    }
                } else {
                    break;
                }
            }

            // If immediately followed by non-whitespace (e.g. ☀US -> ☀ US), insert space so terminal emoji font doesn't overlap text
            if i + 1 < chars.len() {
                let next_c = chars[i + 1];
                let next_u = next_c as u32;
                if !next_c.is_whitespace() && !is_variation_selector(next_u) && next_u != 0x200D {
                    out.push(' ');
                }
            }
        }
        i += 1;
    }

    out
}

fn is_emoji_or_symbol(u: u32) -> bool {
    (0x2300..=0x23FF).contains(&u)
        || (0x2600..=0x26FF).contains(&u)
        || (0x2700..=0x27BF).contains(&u)
        || (0x2B00..=0x2BFF).contains(&u)
        || (0x1F1E6..=0x1F1FF).contains(&u)
        || (0x1F300..=0x1F9FF).contains(&u)
        || (0x1FA00..=0x1FAFF).contains(&u)
}

fn is_regional_indicator(u: u32) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&u)
}

fn is_variation_selector(u: u32) -> bool {
    (0xFE00..=0xFE0F).contains(&u)
}

fn is_skin_tone(u: u32) -> bool {
    (0x1F3FB..=0x1F3FF).contains(&u)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Position;
    use ratatui::Terminal;

    fn dump_buffer(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let area = buffer.area;
        let mut out = String::new();
        for y in area.y..area.bottom() {
            let row: String = (area.x..area.right())
                .map(|x| {
                    buffer
                        .cell(Position::new(x, y))
                        .map(|c| c.symbol())
                        .unwrap_or(" ")
                        .to_string()
                })
                .collect();
            out.push_str(row.trim_end());
            out.push('\n');
        }
        out
    }

    #[test]
    fn help_overlay_is_compact_and_fits_columns() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render_help_modal(f, f.area()))
            .unwrap();

        let dump = dump_buffer(&terminal);
        // 弹窗高度按 keymap 行数自适应，不再固定 30/35 行（曾因此把 DIALOGS 整组裁掉）
        assert_eq!(dump.lines().next().unwrap(), "");
        assert!(dump.contains("KEY BINDINGS"));
        assert!(dump.contains("FULL KEY REFERENCE"));
        assert!(dump.contains("◈ GLOBAL"));
        assert!(dump.contains("◈ SUBSCRIPTIONS"));
        assert!(dump.contains("◈ DIALOGS"));
        assert!(dump.replace(' ', "").contains("切换开关/循环取值"), "DIALOGS 末行被裁剪: {dump}");
        assert!(dump.replace(' ', "").contains("?/Esc关闭"));
        // 双栏分隔线存在
        assert!(dump.contains("│"));
    }

    #[test]
    fn overrides_modal_shows_fields_and_values() {
        let backend = TestBackend::new(120, 44);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let modal = crate::app::OverridesModal {
                    mode: Some("rule".to_string()),
                    log_level: None,
                    allow_lan: Some(true),
                    ipv6: None,
                    unified_delay: None,
                    active_field: 0,
                };
                render_overrides_modal(f, &modal);
            })
            .unwrap();

        let dump = dump_buffer(&terminal);
        assert!(dump.contains("RUNTIME OVERRIDES"));
        assert!(dump.contains("allow-lan"));
        assert!(dump.contains("INHERIT"));
        assert!(dump.contains("RULE"));
    }

    #[test]
    fn backup_modal_lists_entries_and_pending_warning() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let modal = crate::app::BackupModal {
                    entries: vec![crate::backup::BackupEntry {
                        name: "backup-20261120-120000".to_string(),
                        files: 4,
                        bytes: 2048,
                    }],
                    selected: 0,
                    pending: Some("backup-20261120-120000".to_string()),
                    loading: false,
                };
                render_backup_modal(f, &modal);
            })
            .unwrap();

        let dump = dump_buffer(&terminal);
        assert!(dump.contains("LOCAL CONFIG BACKUPS"));
        assert!(dump.contains("backup-20261120-120000"));
        assert!(dump.replace(' ', "").contains("再按Enter确认"));
        assert!(dump.replace(' ', "").contains("Esc取消"));
    }

    #[test]
    fn providers_modal_lists_configured_entries() {
        use crate::providers::{ProviderInfo, ProviderKind, ProvidersSnapshot};

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let modal = crate::app::ProvidersModal {
                    kind: ProviderKind::Rule,
                    snapshot: ProvidersSnapshot {
                        proxies: vec![ProviderInfo {
                            name: "proxy-a".to_string(),
                            kind: ProviderKind::Proxy,
                            vehicle: "http".to_string(),
                            behavior: None,
                            count: 3,
                            updated: "—".to_string(),
                        }],
                        rules: vec![ProviderInfo {
                            name: "rules-a".to_string(),
                            kind: ProviderKind::Rule,
                            vehicle: "http".to_string(),
                            behavior: Some("classical".to_string()),
                            count: 12,
                            updated: "2026-11-20 08:33".to_string(),
                        }],
                    },
                    selected: 0,
                    loading: false,
                    message: None,
                };
                render_providers_modal(f, &modal);
            })
            .unwrap();

        let dump = dump_buffer(&terminal);
        assert!(dump.contains("PROVIDERS"));
        assert!(dump.contains("RULE PROVIDERS"));
        assert!(dump.contains("rules-a"));
        assert!(dump.contains("classical"));
        assert!(dump.contains("12 rules"));
        assert!(dump.contains("2026-11-20 08:33"));
        // 策略组被过滤：当前标签只显示 RULE 列，PROXY 条目不在正文
        assert!(!dump.contains("proxy-a"));
    }

    #[test]
    fn spark_bytes_takes_tail_and_normalizes_to_peak() {
        assert_eq!(spark_bytes(&[], 10, 100), "");
        assert_eq!(spark_bytes(&[0, 100], 2, 100), " ▇");
        // 宽度不够时保留最近的样本
        assert_eq!(spark_bytes(&[100, 0, 100], 2, 100), " ▇");
        // 峰值为 0 时全空白
        assert_eq!(spark_bytes(&[0, 0, 0], 3, 0), "   ");
    }

    #[test]
    fn fit_footer_line_drops_tail_pairs_when_narrow() {
        let spans = vec![
            Span::raw("TITLE"),
            Span::raw("[a] "),
            Span::raw("Add  "),
            Span::raw("[b] "),
            Span::raw("Remove"),
        ];
        let line = fit_footer_line(spans, 12);
        assert!(line.width() <= 12);
        assert_eq!(line.width(), "TITLE".len());
    }

    /// 五个标签页都要能在常见终端尺寸和极窄尺寸下完整渲染（布局不会越界 panic）。
    #[test]
    fn every_tab_renders_at_common_and_tiny_sizes() {
        use crate::app::{ActiveTab, App};

        let tabs = [
            (ActiveTab::Proxies, "PROXIES"),
            (ActiveTab::Connections, "CONNECTIONS"),
            (ActiveTab::Subscriptions, "SUBSCRIPTIONS"),
            (ActiveTab::Settings, "SETTINGS"),
            (ActiveTab::Rules, "RULES"),
        ];

        for (tab, marker) in tabs {
            for (width, height) in [(120u16, 36u16), (40, 12)] {
                let mut app = App::new("http://127.0.0.1:9090", None);
                app.active_tab = tab;
                app.set_status(format!("{marker} smoke"));
                let backend = TestBackend::new(width, height);
                let mut terminal = Terminal::new(backend).unwrap();
                terminal
                    .draw(|f| render(f, &mut app))
                    .unwrap_or_else(|e| panic!("{marker} @ {width}x{height} 渲染失败: {e}"));
                let dump = dump_buffer(&terminal);
                assert!(
                    !dump.trim().is_empty(),
                    "{marker} @ {width}x{height} 渲染结果为空"
                );
                if width >= 120 {
                    assert!(
                        dump.contains(marker),
                        "{marker} @ {width}x{height} 缺少标签标题: {dump}"
                    );
                }
            }
        }
    }
}

