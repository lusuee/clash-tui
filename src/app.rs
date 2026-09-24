use crate::api::{ClashClient, ConnectionItem, ProxyItem};
use crate::core::CoreManager;
use crate::envproxy::EnvProxy;
use crate::subscriptions::SubscriptionManager;
use crate::sysproxy::SysProxy;
use ratatui::widgets::TableState;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ActiveTab {
    Proxies = 0,
    Connections = 1,
    Subscriptions = 2,
    Settings = 3,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ProxyFocus {
    Groups,
    Nodes,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ModalField {
    Name,
    Url,
}

#[derive(Debug, Clone)]
pub struct AddSubModal {
    pub name_input: String,
    pub url_input: String,
    pub active_field: ModalField,
}

pub struct App {
    pub client: ClashClient,
    pub active_tab: ActiveTab,
    pub should_quit: bool,

    // Core & Status
    pub core_version: String,
    pub is_connected: bool,
    pub is_core_running: bool,
    pub mode: String,
    pub mixed_port: u16,
    pub sys_proxy_enabled: bool,
    pub env_proxy_enabled: bool,
    pub tun_enabled: bool,
    pub status_msg: Option<(String, Instant)>,

    // Traffic stats & History for Sparkline
    pub up_speed: u64,
    pub down_speed: u64,
    pub up_history: Vec<u64>,
    pub down_history: Vec<u64>,

    // Proxies View
    pub proxy_focus: ProxyFocus,
    pub proxy_groups: Vec<String>,
    pub selected_group_idx: usize,
    pub proxies: HashMap<String, ProxyItem>,
    pub delays: HashMap<String, u64>,
    pub testing_nodes: HashSet<String>,
    pub selected_node_idx: usize,

    // Connections View
    pub connections: Vec<ConnectionItem>,
    pub selected_conn_idx: usize,
    pub total_upload: u64,
    pub total_download: u64,

    // Subscriptions View
    pub sub_mgr: SubscriptionManager,
    pub selected_sub_idx: usize,

    // Core Daemon
    pub core_mgr: CoreManager,

    // Modals
    pub show_add_sub_modal: Option<AddSubModal>,
    pub show_edit_port_modal: Option<String>,

    // Table States for scrolling
    pub groups_table_state: TableState,
    pub nodes_table_state: TableState,
    pub connections_table_state: TableState,
    pub subs_table_state: TableState,

    // Test URL
    pub test_url: String,
}

impl App {
    pub fn new(base_url: &str, secret: Option<String>) -> Self {
        let (sys_enabled, _) = SysProxy::get_status().unwrap_or((false, String::new()));
        let env_enabled = EnvProxy::get_status();
        let sub_mgr = SubscriptionManager::new();
        let core_mgr = CoreManager::new();

        Self {
            client: ClashClient::new(base_url, secret),
            active_tab: ActiveTab::Proxies,
            should_quit: false,

            core_version: "Unknown".to_string(),
            is_connected: false,
            is_core_running: false,
            mode: "Rule".to_string(),
            mixed_port: 7897,
            sys_proxy_enabled: sys_enabled,
            env_proxy_enabled: env_enabled,
            tun_enabled: false,
            status_msg: Some(("Connecting to Mihomo Core...".to_string(), Instant::now())),

            up_speed: 0,
            down_speed: 0,
            up_history: vec![0; 36],
            down_history: vec![0; 36],

            proxy_focus: ProxyFocus::Nodes,
            proxy_groups: Vec::new(),
            selected_group_idx: 0,
            proxies: HashMap::new(),
            delays: HashMap::new(),
            testing_nodes: HashSet::new(),
            selected_node_idx: 0,

            connections: Vec::new(),
            selected_conn_idx: 0,
            total_upload: 0,
            total_download: 0,

            sub_mgr,
            selected_sub_idx: 0,

            core_mgr,
            show_add_sub_modal: None,
            show_edit_port_modal: None,

            groups_table_state: TableState::default(),
            nodes_table_state: TableState::default(),
            connections_table_state: TableState::default(),
            subs_table_state: TableState::default(),

            test_url: "http://www.gstatic.com/generate_204".to_string(),
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = Some((msg.into(), Instant::now()));
    }

    pub fn get_active_status(&self) -> Option<&str> {
        if let Some((msg, time)) = &self.status_msg {
            if time.elapsed() < Duration::from_secs(4) {
                return Some(msg.as_str());
            }
        }
        None
    }

    pub fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Proxies => ActiveTab::Connections,
            ActiveTab::Connections => ActiveTab::Subscriptions,
            ActiveTab::Subscriptions => ActiveTab::Settings,
            ActiveTab::Settings => ActiveTab::Proxies,
        };
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Proxies => ActiveTab::Settings,
            ActiveTab::Connections => ActiveTab::Proxies,
            ActiveTab::Subscriptions => ActiveTab::Connections,
            ActiveTab::Settings => ActiveTab::Subscriptions,
        };
    }

    pub fn set_tab(&mut self, tab: ActiveTab) {
        self.active_tab = tab;
    }

    pub fn current_group_name(&self) -> Option<&str> {
        self.proxy_groups.get(self.selected_group_idx).map(|s| s.as_str())
    }

    pub fn current_group_nodes(&self) -> Vec<String> {
        if let Some(group_name) = self.current_group_name() {
            if let Some(group) = self.proxies.get(group_name) {
                return group.all.clone().unwrap_or_default();
            }
        }
        Vec::new()
    }

    pub fn current_selected_node_name(&self) -> Option<String> {
        let nodes = self.current_group_nodes();
        nodes.get(self.selected_node_idx).cloned()
    }

    pub fn on_up(&mut self) {
        match self.active_tab {
            ActiveTab::Proxies => match self.proxy_focus {
                ProxyFocus::Groups => {
                    if self.selected_group_idx > 0 {
                        self.selected_group_idx -= 1;
                        self.selected_node_idx = 0;
                        self.nodes_table_state = TableState::default();
                    }
                }
                ProxyFocus::Nodes => {
                    if self.selected_node_idx > 0 {
                        self.selected_node_idx -= 1;
                    }
                }
            },
            ActiveTab::Connections => {
                if self.selected_conn_idx > 0 {
                    self.selected_conn_idx -= 1;
                }
            }
            ActiveTab::Subscriptions => {
                if self.selected_sub_idx > 0 {
                    self.selected_sub_idx -= 1;
                }
            }
            ActiveTab::Settings => {}
        }
    }

    pub fn on_down(&mut self) {
        match self.active_tab {
            ActiveTab::Proxies => match self.proxy_focus {
                ProxyFocus::Groups => {
                    if !self.proxy_groups.is_empty() && self.selected_group_idx + 1 < self.proxy_groups.len() {
                        self.selected_group_idx += 1;
                        self.selected_node_idx = 0;
                        self.nodes_table_state = TableState::default();
                    }
                }
                ProxyFocus::Nodes => {
                    let count = self.current_group_nodes().len();
                    if count > 0 && self.selected_node_idx + 1 < count {
                        self.selected_node_idx += 1;
                    }
                }
            },
            ActiveTab::Connections => {
                if !self.connections.is_empty() && self.selected_conn_idx + 1 < self.connections.len() {
                    self.selected_conn_idx += 1;
                }
            }
            ActiveTab::Subscriptions => {
                let count = self.sub_mgr.subscriptions.len();
                if count > 0 && self.selected_sub_idx + 1 < count {
                    self.selected_sub_idx += 1;
                }
            }
            ActiveTab::Settings => {}
        }
    }

    pub fn on_page_up(&mut self) {
        let step = 10;
        match self.active_tab {
            ActiveTab::Proxies => match self.proxy_focus {
                ProxyFocus::Groups => {
                    if self.selected_group_idx > 0 {
                        self.selected_group_idx = self.selected_group_idx.saturating_sub(step);
                        self.selected_node_idx = 0;
                        self.nodes_table_state = TableState::default();
                    }
                }
                ProxyFocus::Nodes => {
                    self.selected_node_idx = self.selected_node_idx.saturating_sub(step);
                }
            },
            ActiveTab::Connections => {
                self.selected_conn_idx = self.selected_conn_idx.saturating_sub(step);
            }
            ActiveTab::Subscriptions => {
                self.selected_sub_idx = self.selected_sub_idx.saturating_sub(step);
            }
            ActiveTab::Settings => {}
        }
    }

    pub fn on_page_down(&mut self) {
        let step = 10;
        match self.active_tab {
            ActiveTab::Proxies => match self.proxy_focus {
                ProxyFocus::Groups => {
                    if !self.proxy_groups.is_empty() {
                        let max_idx = self.proxy_groups.len() - 1;
                        self.selected_group_idx = (self.selected_group_idx + step).min(max_idx);
                        self.selected_node_idx = 0;
                        self.nodes_table_state = TableState::default();
                    }
                }
                ProxyFocus::Nodes => {
                    let count = self.current_group_nodes().len();
                    if count > 0 {
                        self.selected_node_idx = (self.selected_node_idx + step).min(count - 1);
                    }
                }
            },
            ActiveTab::Connections => {
                if !self.connections.is_empty() {
                    let max_idx = self.connections.len() - 1;
                    self.selected_conn_idx = (self.selected_conn_idx + step).min(max_idx);
                }
            }
            ActiveTab::Subscriptions => {
                let count = self.sub_mgr.subscriptions.len();
                if count > 0 {
                    self.selected_sub_idx = (self.selected_sub_idx + step).min(count - 1);
                }
            }
            ActiveTab::Settings => {}
        }
    }

    pub fn on_home(&mut self) {
        match self.active_tab {
            ActiveTab::Proxies => match self.proxy_focus {
                ProxyFocus::Groups => {
                    self.selected_group_idx = 0;
                    self.selected_node_idx = 0;
                    self.nodes_table_state = TableState::default();
                }
                ProxyFocus::Nodes => {
                    self.selected_node_idx = 0;
                }
            },
            ActiveTab::Connections => {
                self.selected_conn_idx = 0;
            }
            ActiveTab::Subscriptions => {
                self.selected_sub_idx = 0;
            }
            ActiveTab::Settings => {}
        }
    }

    pub fn on_end(&mut self) {
        match self.active_tab {
            ActiveTab::Proxies => match self.proxy_focus {
                ProxyFocus::Groups => {
                    if !self.proxy_groups.is_empty() {
                        self.selected_group_idx = self.proxy_groups.len() - 1;
                        self.selected_node_idx = 0;
                        self.nodes_table_state = TableState::default();
                    }
                }
                ProxyFocus::Nodes => {
                    let count = self.current_group_nodes().len();
                    if count > 0 {
                        self.selected_node_idx = count - 1;
                    }
                }
            },
            ActiveTab::Connections => {
                if !self.connections.is_empty() {
                    self.selected_conn_idx = self.connections.len() - 1;
                }
            }
            ActiveTab::Subscriptions => {
                let count = self.sub_mgr.subscriptions.len();
                if count > 0 {
                    self.selected_sub_idx = count - 1;
                }
            }
            ActiveTab::Settings => {}
        }
    }

    pub fn on_left(&mut self) {
        if self.active_tab == ActiveTab::Proxies {
            self.proxy_focus = ProxyFocus::Groups;
        }
    }

    pub fn on_right(&mut self) {
        if self.active_tab == ActiveTab::Proxies {
            self.proxy_focus = ProxyFocus::Nodes;
        }
    }

    pub fn toggle_sys_proxy(&mut self) {
        let next_state = !self.sys_proxy_enabled;
        let server = format!("127.0.0.1:{}", self.mixed_port);
        match SysProxy::set_proxy(next_state, &server) {
            Ok(_) => {
                self.sys_proxy_enabled = next_state;
                if next_state {
                    self.set_status(format!("✔ SysProxy ON ({})", server));
                } else {
                    self.set_status("✔ SysProxy OFF");
                }
            }
            Err(e) => {
                self.set_status(format!("✘ SysProxy error: {}", e));
            }
        }
    }

    pub fn toggle_env_proxy(&mut self) {
        let next_state = !self.env_proxy_enabled;
        match EnvProxy::set_env_proxy(next_state, self.mixed_port) {
            Ok(_) => {
                self.env_proxy_enabled = next_state;
                if next_state {
                    self.set_status(format!("✔ 环境变量已设置 (127.0.0.1:{}) | 终端输入 source env.nu 立即生效", self.mixed_port));
                } else {
                    self.set_status("✔ 环境变量代理已关闭 (已恢复系统默认)");
                }
            }
            Err(e) => {
                self.set_status(format!("✘ 环境变量设置失败: {}", e));
            }
        }
    }

    pub fn open_edit_port_modal(&mut self) {
        self.show_edit_port_modal = Some(self.mixed_port.to_string());
    }

    pub fn close_edit_port_modal(&mut self) {
        self.show_edit_port_modal = None;
    }

    pub fn edit_port_handle_char(&mut self, c: char) {
        if let Some(buf) = &mut self.show_edit_port_modal {
            if c.is_ascii_digit() && buf.len() < 5 {
                buf.push(c);
            }
        }
    }

    pub fn edit_port_backspace(&mut self) {
        if let Some(buf) = &mut self.show_edit_port_modal {
            buf.pop();
        }
    }

    pub fn cycle_mode(&mut self) -> Option<String> {
        let next_mode = match self.mode.to_lowercase().as_str() {
            "rule" => "Global",
            "global" => "Direct",
            _ => "Rule",
        };
        Some(next_mode.to_string())
    }

    pub fn update_traffic(&mut self, up: u64, down: u64) {
        self.up_speed = up;
        self.down_speed = down;

        self.up_history.push(up);
        if self.up_history.len() > 36 {
            self.up_history.remove(0);
        }

        self.down_history.push(down);
        if self.down_history.len() > 36 {
            self.down_history.remove(0);
        }
    }

    pub fn update_proxies(&mut self, resp: HashMap<String, ProxyItem>) {
        let mut groups: Vec<String> = resp
            .iter()
            .filter(|(_, item)| item.all.is_some() && !item.all.as_ref().unwrap().is_empty())
            .map(|(name, _)| name.clone())
            .collect();

        groups.sort_by(|a, b| {
            if a == "GLOBAL" {
                std::cmp::Ordering::Less
            } else if b == "GLOBAL" {
                std::cmp::Ordering::Greater
            } else if a == "Proxy" || a == "PROXY" {
                std::cmp::Ordering::Less
            } else if b == "Proxy" || b == "PROXY" {
                std::cmp::Ordering::Greater
            } else {
                a.cmp(b)
            }
        });

        for (name, item) in &resp {
            if let Some(history) = &item.history {
                if let Some(last) = history.last() {
                    if last.delay > 0 {
                        self.delays.insert(name.clone(), last.delay);
                    }
                }
            }
        }

        self.proxy_groups = groups;
        self.proxies = resp;

        if self.selected_group_idx >= self.proxy_groups.len() {
            self.selected_group_idx = self.proxy_groups.len().saturating_sub(1);
        }
        let node_count = self.current_group_nodes().len();
        if self.selected_node_idx >= node_count {
            self.selected_node_idx = node_count.saturating_sub(1);
        }
    }

    // Modal Management
    pub fn open_add_sub_modal(&mut self) {
        self.show_add_sub_modal = Some(AddSubModal {
            name_input: String::new(),
            url_input: String::new(),
            active_field: ModalField::Name,
        });
    }

    pub fn close_modal(&mut self) {
        self.show_add_sub_modal = None;
    }

    pub fn modal_handle_char(&mut self, c: char) {
        if let Some(modal) = &mut self.show_add_sub_modal {
            match modal.active_field {
                ModalField::Name => modal.name_input.push(c),
                ModalField::Url => modal.url_input.push(c),
            }
        }
    }

    pub fn modal_backspace(&mut self) {
        if let Some(modal) = &mut self.show_add_sub_modal {
            match modal.active_field {
                ModalField::Name => {
                    modal.name_input.pop();
                }
                ModalField::Url => {
                    modal.url_input.pop();
                }
            }
        }
    }

    pub fn modal_toggle_field(&mut self) {
        if let Some(modal) = &mut self.show_add_sub_modal {
            modal.active_field = match modal.active_field {
                ModalField::Name => ModalField::Url,
                ModalField::Url => ModalField::Name,
            };
        }
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;

    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.1} MB", b / MB)
    } else if b >= KB {
        format!("{:.0} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_speed(bps: u64) -> String {
    format!("{}/s", format_bytes(bps))
}
