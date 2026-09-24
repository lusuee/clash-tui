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
        render_add_rule_modal(f, modal);
    }
    if let Some(port_input) = &app.show_edit_port_modal {
        render_edit_port_modal(f, port_input);
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

        let cells = vec![
            Cell::from(format!("{}{}", pointer, host_port)),
            Cell::from(Span::styled(format!(" {} ", network), Style::default().fg(net_color).add_modifier(Modifier::BOLD))),
            Cell::from(transfer_line),
            Cell::from(Span::styled(format!("[{}]", conn.rule), Style::default().fg(Theme::YELLOW))),
            Cell::from(chains),
        ];
        rows.push(Row::new(cells).style(row_style));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Length(8),
            Constraint::Length(24),
            Constraint::Percentage(16),
            Constraint::Percentage(22),
        ],
    )
    .header(
        Row::new(vec!["DESTINATION ENDPOINT", "NET", "DATA TRANSFER", "ROUTING RULE", "PROXY CHAIN"])
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
    ]);

    let block = Block::default()
        .title(sub_title)
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

        let pointer = if is_selected { "❯ " } else { "  " };
        let (active_badge, active_style) = if sub.active {
            (" ● ACTIVE ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))
        } else {
            (" ○ IDLE   ", Style::default().fg(Theme::MUTED))
        };

        let cells = vec![
            Cell::from(Span::styled(format!("{}{}", pointer, active_badge), active_style)),
            Cell::from(format_display_name(&sub.name)),
            Cell::from(Span::styled(
                format!("◈ {} nodes", sub.node_count),
                Style::default().fg(Theme::SAPPHIRE),
            )),
            Cell::from(Span::styled(format!("󰅐 {}", sub.last_updated), Style::default().fg(Theme::MUTED))),
            Cell::from(Span::styled(sub.url.clone(), Style::default().fg(Theme::SUBTEXT0))),
        ];
        rows.push(Row::new(cells).style(row_style));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Percentage(25),
            Constraint::Length(14),
            Constraint::Length(20),
            Constraint::Percentage(41),
        ],
    )
    .header(
        Row::new(vec!["STATE", "PROFILE IDENTIFIER", "NODES", "SYNCHRONIZED", "REMOTE URL"])
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
        Span::styled("│ TARGET: ", Style::default().fg(Theme::SURFACE2)),
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
            _ => (Theme::TEXT, "generic"),
        };

        let cells = vec![
            Cell::from(format!("{}{}", pointer, rule.rule_type))
                .style(Style::default().fg(rule_color).add_modifier(Modifier::BOLD)),
            Cell::from(rule.value.clone()),
            Cell::from(Span::styled(rule_desc, Style::default().fg(Theme::MUTED))),
            Cell::from(Span::styled("➜ PROXY", Style::default().fg(Theme::TEAL))),
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
        Row::new(vec!["RULE TYPE", "DOMAIN PATTERN / KEYWORD", "MATCH STRATEGY", "TARGET"])
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
        .constraints([Constraint::Length(8), Constraint::Length(9), Constraint::Min(4)])
        .split(area);

    // Card 1: Mihomo Core Daemon Telemetry
    let core_block = Block::default()
        .title(Span::styled(" ⚙ CORE ENGINE & DAEMON TELEMETRY ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::inactive_border());

    let core_lines = vec![
        Line::from(vec![
            Span::styled("◈ Binary Image:      ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(app.core_mgr.bin_path.to_string_lossy(), Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("◈ Engine Version:    ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(format!("Mihomo {}", app.core_version), Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
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

    let net_lines = vec![
        Line::from(vec![
            Span::styled("◈ Platform Driver:   ", Style::default().fg(Theme::SUBTEXT0)),
            Span::styled(platform_name, Style::default().fg(Theme::TEXT)),
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
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " 󰑓 ADD SUBSCRIPTION PROFILE ",
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
    let hint_line = Line::from(vec![
        Span::styled("[Tab] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
        Span::raw("Next Field   "),
        Span::styled("[Enter] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
        Span::raw("Save & Synchronize   "),
        Span::styled("[Esc] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
        Span::raw("Dismiss"),
    ]);
    f.render_widget(Paragraph::new(hint_line).alignment(Alignment::Center), chunks[2]);
}

fn render_add_rule_modal(f: &mut Frame, modal: &crate::app::AddRuleModal) {
    let area = centered_rect(58, 28, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(Span::styled(
            " 󰃢 ADD DOMAIN PROXY RULE ",
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
            Constraint::Length(2), // Help
        ])
        .split(inner);

    let hint = Paragraph::new("Target domain or keyword to route via proxy:")
        .style(Style::default().fg(Theme::SUBTEXT0));
    f.render_widget(hint, chunks[0]);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::CYAN));
    let input_text = Line::from(vec![
        Span::raw(" "),
        Span::styled(modal.input.as_str(), Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD)),
        Span::styled("█", Style::default().fg(Theme::CYAN)),
    ]);
    f.render_widget(Paragraph::new(input_text).block(input_block), chunks[1]);

    let help_text = vec![
        Line::from(vec![
            Span::styled("example.com", Style::default().fg(Theme::GREEN)),
            Span::raw(" ➜ DOMAIN-SUFFIX │ "),
            Span::styled("full:example.com", Style::default().fg(Theme::MAUVE)),
            Span::raw(" ➜ DOMAIN │ "),
            Span::styled("keyword:google", Style::default().fg(Theme::PEACH)),
            Span::raw(" ➜ KEYWORD"),
        ]),
        Line::from(vec![
            Span::styled("[Enter] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
            Span::raw("Save & Reload   "),
            Span::styled("[Esc] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
            Span::raw("Dismiss"),
        ]),
    ];
    f.render_widget(Paragraph::new(help_text).alignment(Alignment::Center), chunks[2]);
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
                Span::styled("[u] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Update  "),
                Span::styled("[U] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Update All  "),
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
                Span::styled("[P] ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
                Span::raw("Port  "),
                Span::styled("[A] ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
                Span::raw("Admin  "),
                Span::styled("[s/S] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::raw("Core Daemon"),
            ]),
            ActiveTab::Rules => Line::from(vec![
                Span::styled(" ◈ RULES: ", Style::default().fg(Theme::CYAN).add_modifier(Modifier::BOLD)),
                Span::styled("[a] ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
                Span::raw("Add Domain  "),
                Span::styled("[x] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
                Span::raw("Delete  "),
                Span::styled("[PgUp/Dn] ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
                Span::raw("Scroll"),
            ]),
        };
        f.render_widget(Paragraph::new(hint_line), chunks[0]);
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
    ]);
    f.render_widget(Paragraph::new(global_keys), chunks[1]);
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

