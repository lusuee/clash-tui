use crate::api::{ClashClient, ConnectionItem, ProxyItem};
use crate::appupdate;
use crate::core::CoreManager;
use crate::dnsconf::{format_policies, parse_list, parse_policies, DnsManager, DnsSettings};
use crate::envproxy::EnvProxy;
use crate::kernelupdate;
use crate::overrides::{OverridesManager, OverridesSettings};
use crate::providers::{ProviderInfo, ProviderKind, ProvidersSnapshot};
use crate::rules::{RuleManager, RuleTarget};
use crate::subscriptions::SubscriptionManager;
use crate::sysproxy::SysProxy;
use ratatui::widgets::TableState;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

/// 流量历史采样上限（1s 一点 = 最近 5 分钟）
pub const TRAFFIC_HISTORY_CAP: usize = 300;

/// 每秒采样点（`up` / `down` 为字节/秒）
#[derive(Debug, Clone, Copy, Default)]
pub struct TrafficPoint {
    pub up: u64,
    pub down: u64,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ActiveTab {
    Proxies = 0,
    Connections = 1,
    Subscriptions = 2,
    Settings = 3,
    Rules = 4,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ProxyFocus {
    Groups,
    Nodes,
}

/// 节点列表排序方式（Proxies 页按 `o` 循环切换）
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NodeSort {
    /// 订阅原始顺序
    Default,
    /// 按名称
    Name,
    /// 按延迟升序（未测速的排最后）
    Ping,
}

impl NodeSort {
    pub fn label(&self) -> &'static str {
        match self {
            NodeSort::Default => "Default",
            NodeSort::Name => "Name",
            NodeSort::Ping => "Ping",
        }
    }
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
    /// `Some(id)` 表示编辑已有订阅，`None` 表示新增
    pub editing_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AddRuleModal {
    pub input: String,
    /// 弹窗内选定的去向（Tab 循环），输入里的 `direct:` / `proxy:` 前缀优先
    pub target: RuleTarget,
    /// 去向为 PROXY 时指定的策略组；None 表示沿用解析出的默认代理组
    pub group: Option<String>,
}

/// 规则命中推演弹窗
#[derive(Debug, Clone)]
pub struct RuleTestModal {
    pub input: String,
    pub report: Option<crate::rules::ProbeReport>,
}

/// DNS 弹窗的字段数：0 为 DIRECT 系统 DNS 开关，1..=5 为文本字段
pub const DNS_MODAL_FIELDS: usize = 6;

#[derive(Debug, Clone)]
pub struct DnsModal {
    pub direct_system: bool,
    pub nameserver: String,
    pub fallback: String,
    pub proxy_nameserver: String,
    pub default_nameserver: String,
    pub policies: String,
    /// 当前焦点：0 = DIRECT 策略开关；1 = nameserver；2 = fallback；
    /// 3 = proxy-server-nameserver；4 = default-nameserver；5 = 域名策略
    pub active_field: usize,
}

/// 内核体检弹窗（Settings → `H`）
#[derive(Debug, Clone)]
pub struct HealthModal {
    pub items: Vec<crate::diagnostics::HealthItem>,
    pub generated: String,
    pub loading: bool,
}

/// 内核日志查看弹窗（Settings → `L`）
#[derive(Debug, Clone)]
pub struct LogModal {
    pub filter: String,
    pub lines: Vec<String>,
    pub error: Option<String>,
    pub generated: String,
    pub loading: bool,
}

/// 运行配置覆盖弹窗的字段数
pub const OVERRIDES_MODAL_FIELDS: usize = 5;

/// 本地配置备份弹窗（Settings → `B`）
#[derive(Debug, Clone)]
pub struct BackupModal {
    pub entries: Vec<crate::backup::BackupEntry>,
    pub selected: usize,
    /// 已请求恢复、等待二次确认的备份名
    pub pending: Option<String>,
    pub loading: bool,
}

/// Providers 弹窗（Settings → `V`）：proxy-providers / rule-providers 列表
#[derive(Debug, Clone)]
pub struct ProvidersModal {
    pub kind: ProviderKind,
    pub snapshot: ProvidersSnapshot,
    pub selected: usize,
    pub loading: bool,
    pub message: Option<String>,
}

/// 运行配置覆盖弹窗（Settings → `C`）
#[derive(Debug, Clone)]
pub struct OverridesModal {
    pub mode: Option<String>,
    pub log_level: Option<String>,
    pub allow_lan: Option<bool>,
    pub ipv6: Option<bool>,
    pub unified_delay: Option<bool>,
    /// 当前焦点：0 mode / 1 log-level / 2 allow-lan / 3 ipv6 / 4 unified-delay
    pub active_field: usize,
}

pub struct App {
    pub client: ClashClient,
    /// 内核 REST 地址与 secret（供后台任务判断内核是否真的在运行）
    pub api_url: String,
    pub core_secret: Option<String>,
    pub active_tab: ActiveTab,
    pub should_quit: bool,
    /// 通过 Q 退出时是否已成功停止内核（决定退出提示文案）
    pub core_stopped_on_exit: bool,

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

    // Kernel download/update (Settings → K)
    /// 磁盘上内核二进制的版本（`mihomo -v`），不是 REST 报告的运行版本。
    pub kernel_bin_version: String,
    /// 最近一次检查得到的 GitHub latest 版本（不含 `v`）。
    pub kernel_latest: Option<String>,
    pub kernel_busy: bool,
    pub kernel_progress: u8,

    // clash-tui self update (Settings → U)
    /// 当前 clash-tui 版本（编译期注入，可能带 git 描述后缀）。
    pub app_version: String,
    /// 最近一次检查到的最新 clash-tui 版本（不含 `v`）。
    pub app_latest: Option<String>,
    pub app_busy: bool,
    pub app_progress: u8,
    /// 自动更新安装完成后，退出主循环时是否用新二进制重启
    pub restart_after_exit: bool,

    // Traffic stats & History for Sparkline
    pub up_speed: u64,
    pub down_speed: u64,
    pub up_history: Vec<u64>,
    pub down_history: Vec<u64>,
    /// 每秒采样点（最多 300 个），供流量历史弹窗绘制 5 分钟曲线
    pub traffic_log: VecDeque<TrafficPoint>,
    /// 本次运行内观察到的峰值速率
    pub traffic_peak_up: u64,
    pub traffic_peak_down: u64,
    /// 流量历史弹窗（Settings → Y）
    pub show_traffic_modal: bool,

    // Proxies View
    pub proxy_focus: ProxyFocus,
    pub node_sort: NodeSort,
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
    pub updating_subs: HashSet<String>,
    /// 上次自动更新检查时间（Tick 中节流用）
    pub last_sub_auto_check: Instant,

    // Domain Rules View
    pub rule_mgr: RuleManager,
    pub selected_rule_idx: usize,

    // DNS Overrides (Settings → DNS modal)
    pub dns_mgr: DnsManager,

    // Runtime Overrides (Settings → C)
    pub overrides_mgr: OverridesManager,

    // Core Daemon
    pub core_mgr: CoreManager,

    // Modals
    pub show_add_sub_modal: Option<AddSubModal>,
    pub show_add_rule_modal: Option<AddRuleModal>,
    pub show_rule_test_modal: Option<RuleTestModal>,
    pub show_edit_port_modal: Option<String>,
    pub show_dns_modal: Option<DnsModal>,
    /// 内核体检（Settings → H）
    pub show_health_modal: Option<HealthModal>,
    /// 内核日志查看（Settings → L）
    pub show_logs_modal: Option<LogModal>,
    /// 日志弹窗自动刷新节流：打开期间每 2s 重读一次日志尾部
    pub last_logs_auto: Instant,
    /// 运行配置覆盖（Settings → C）
    pub show_overrides_modal: Option<OverridesModal>,
    /// 本地配置备份（Settings → B）
    pub show_backup_modal: Option<BackupModal>,
    /// Providers 列表（Settings → V）
    pub show_providers_modal: Option<ProvidersModal>,
    /// 快捷键帮助浮层（`?` 打开，Esc/`?`/`q` 关闭）
    pub show_help: bool,

    // Table States for scrolling
    pub groups_table_state: TableState,
    pub nodes_table_state: TableState,
    pub connections_table_state: TableState,
    pub subs_table_state: TableState,
    pub rules_table_state: TableState,

    // Test URL
    pub test_url: String,
}

impl App {
    pub fn new(base_url: &str, secret: Option<String>) -> Self {
        let (sys_enabled, _) = SysProxy::get_status().unwrap_or((false, String::new()));
        let env_enabled = EnvProxy::get_status();
        let sub_mgr = SubscriptionManager::new();
        let core_mgr = CoreManager::new();
        let kernel_bin_version = kernelupdate::installed_version(&core_mgr.bin_path);

        Self {
            client: ClashClient::new(base_url, secret.clone()),
            api_url: base_url.to_string(),
            core_secret: secret,
            active_tab: ActiveTab::Proxies,
            should_quit: false,
            core_stopped_on_exit: false,

            core_version: "Unknown".to_string(),
            is_connected: false,
            is_core_running: false,
            mode: "Rule".to_string(),
            mixed_port: 7897,
            sys_proxy_enabled: sys_enabled,
            env_proxy_enabled: env_enabled,
            tun_enabled: false,
            status_msg: Some(("Connecting to Mihomo Core...".to_string(), Instant::now())),

            kernel_bin_version: kernel_bin_version.unwrap_or_else(|| "Unknown".to_string()),
            kernel_latest: None,
            kernel_busy: false,
            kernel_progress: 0,

            app_version: appupdate::version_label(),
            app_latest: None,
            app_busy: false,
            app_progress: 0,
            restart_after_exit: false,

            up_speed: 0,
            down_speed: 0,
            up_history: vec![0; 36],
            down_history: vec![0; 36],
            traffic_log: VecDeque::with_capacity(TRAFFIC_HISTORY_CAP),
            traffic_peak_up: 0,
            traffic_peak_down: 0,
            show_traffic_modal: false,

            proxy_focus: ProxyFocus::Nodes,
            node_sort: NodeSort::Default,
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
            updating_subs: HashSet::new(),
            last_sub_auto_check: Instant::now(),

            rule_mgr: RuleManager::new(),
            selected_rule_idx: 0,

            dns_mgr: DnsManager::new(),
            overrides_mgr: OverridesManager::new(),

            core_mgr,
            show_add_sub_modal: None,
            show_add_rule_modal: None,
            show_rule_test_modal: None,
            show_edit_port_modal: None,
            show_dns_modal: None,
            show_overrides_modal: None,
            show_backup_modal: None,
            show_providers_modal: None,
            show_health_modal: None,
            show_logs_modal: None,
            last_logs_auto: Instant::now(),
            show_help: false,

            groups_table_state: TableState::default(),
            nodes_table_state: TableState::default(),
            connections_table_state: TableState::default(),
            subs_table_state: TableState::default(),
            rules_table_state: TableState::default(),

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
            ActiveTab::Settings => ActiveTab::Rules,
            ActiveTab::Rules => ActiveTab::Proxies,
        };
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            ActiveTab::Proxies => ActiveTab::Rules,
            ActiveTab::Connections => ActiveTab::Proxies,
            ActiveTab::Subscriptions => ActiveTab::Connections,
            ActiveTab::Settings => ActiveTab::Subscriptions,
            ActiveTab::Rules => ActiveTab::Settings,
        };
    }

    pub fn set_tab(&mut self, tab: ActiveTab) {
        self.active_tab = tab;
    }

    pub fn current_group_name(&self) -> Option<&str> {
        self.proxy_groups.get(self.selected_group_idx).map(|s| s.as_str())
    }

    pub fn current_group_nodes(&self) -> Vec<String> {
        let mut nodes: Vec<String> = if let Some(group_name) = self.current_group_name() {
            if let Some(group) = self.proxies.get(group_name) {
                group.all.clone().unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        match self.node_sort {
            NodeSort::Default => {}
            NodeSort::Name => nodes.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase())),
            NodeSort::Ping => nodes.sort_by_key(|n| self.delays.get(n).copied().unwrap_or(u64::MAX)),
        }

        nodes
    }

    /// 循环切换节点排序：Default -> Name -> Ping -> Default
    pub fn cycle_node_sort(&mut self) {
        self.node_sort = match self.node_sort {
            NodeSort::Default => NodeSort::Name,
            NodeSort::Name => NodeSort::Ping,
            NodeSort::Ping => NodeSort::Default,
        };
        // 重置光标避免越界或指向偏差
        self.selected_node_idx = 0;
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
            ActiveTab::Rules => {
                if self.selected_rule_idx > 0 {
                    self.selected_rule_idx -= 1;
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
            ActiveTab::Rules => {
                let count = self.rule_mgr.rules.len();
                if count > 0 && self.selected_rule_idx + 1 < count {
                    self.selected_rule_idx += 1;
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
            ActiveTab::Rules => {
                self.selected_rule_idx = self.selected_rule_idx.saturating_sub(step);
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
            ActiveTab::Rules => {
                let count = self.rule_mgr.rules.len();
                if count > 0 {
                    self.selected_rule_idx = (self.selected_rule_idx + step).min(count - 1);
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
            ActiveTab::Rules => {
                self.selected_rule_idx = 0;
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
            ActiveTab::Rules => {
                let count = self.rule_mgr.rules.len();
                if count > 0 {
                    self.selected_rule_idx = count - 1;
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

        self.traffic_log.push_back(TrafficPoint { up, down });
        if self.traffic_log.len() > TRAFFIC_HISTORY_CAP {
            self.traffic_log.pop_front();
        }
        self.traffic_peak_up = self.traffic_peak_up.max(up);
        self.traffic_peak_down = self.traffic_peak_down.max(down);
    }

    /// 打开流量历史弹窗（Settings → `Y`）
    pub fn open_traffic_modal(&mut self) {
        self.show_traffic_modal = true;
    }

    pub fn close_traffic_modal(&mut self) {
        self.show_traffic_modal = false;
    }

    /// 清空采样与峰值（会立即从下一个 Tick 重新积累）
    pub fn clear_traffic_history(&mut self) {
        self.traffic_log.clear();
        self.traffic_peak_up = 0;
        self.traffic_peak_down = 0;
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
            editing_id: None,
        });
    }

    /// 打开编辑订阅弹窗（预填当前选中订阅的名称/URL）
    pub fn open_edit_sub_modal(&mut self) {
        let Some(sub) = self.sub_mgr.subscriptions.get(self.selected_sub_idx).cloned() else {
            self.set_status("没有可编辑的订阅");
            return;
        };
        self.show_add_sub_modal = Some(AddSubModal {
            name_input: sub.name,
            url_input: sub.url,
            active_field: ModalField::Name,
            editing_id: Some(sub.id),
        });
    }

    pub fn open_add_rule_modal(&mut self) {
        // 配置可能已被订阅更新/内核重载改写，打开弹窗前刷新可选策略组
        self.rule_mgr.groups = crate::rules::config_proxy_groups();
        let target = if self.rule_mgr.target_hint.eq_ignore_ascii_case("DIRECT") {
            RuleTarget::Direct
        } else {
            RuleTarget::Proxy
        };
        self.show_add_rule_modal = Some(AddRuleModal {
            input: String::new(),
            target,
            group: None,
        });
    }

    /// 在添加规则弹窗内循环目的地：DIRECT → 默认代理组 → 各可选策略组
    pub fn rule_modal_cycle_destination(&mut self) {
        let states = self.rule_mgr.destinations();
        if let Some(modal) = &mut self.show_add_rule_modal {
            let pos = states
                .iter()
                .position(|(t, g)| *t == modal.target && g.as_deref() == modal.group.as_deref())
                .unwrap_or(0);
            let (target, group) = states[(pos + 1) % states.len()].clone();
            modal.target = target;
            modal.group = group;
        }
    }

    pub fn open_rule_test_modal(&mut self) {
        self.show_rule_test_modal = Some(RuleTestModal {
            input: String::new(),
            report: None,
        });
    }

    pub fn close_rule_test_modal(&mut self) {
        self.show_rule_test_modal = None;
    }

    /// 用当前生效配置对输入域名做命中推演
    pub fn rule_test_run(&mut self) {
        let Some(modal) = &self.show_rule_test_modal else {
            return;
        };
        let input = modal.input.trim().to_string();
        if input.is_empty() {
            self.set_status("✘ 请输入要推演的域名");
            return;
        }
        match crate::rules::probe_domain_from_config(&input) {
            Some(report) => {
                let summary = match &report.outcome {
                    crate::rules::ProbeOutcome::Match { index, rule } => {
                        format!("命中第 {} 条规则：{}", index + 1, rule)
                    }
                    crate::rules::ProbeOutcome::KernelOnly { index, rule } => {
                        format!("第 {} 条规则需内核解析：{}", index + 1, rule)
                    }
                    crate::rules::ProbeOutcome::NoMatch => {
                        "未命中任何规则（使用 mihomo 默认策略）".to_string()
                    }
                };
                self.set_status(format!("✔ 推演 {} → {}", report.domain, summary));
                if let Some(modal) = &mut self.show_rule_test_modal {
                    modal.report = Some(report);
                }
            }
            None => self.set_status("✘ 无法读取 data/config.yaml 或域名为空"),
        }
    }

    pub fn rule_test_handle_char(&mut self, c: char) {
        if let Some(modal) = &mut self.show_rule_test_modal {
            modal.input.push(c);
        }
    }

    pub fn rule_test_backspace(&mut self) {
        if let Some(modal) = &mut self.show_rule_test_modal {
            modal.input.pop();
            modal.report = None;
        }
    }

    pub fn close_modal(&mut self) {
        self.show_add_sub_modal = None;
        self.show_add_rule_modal = None;
        self.show_rule_test_modal = None;
    }

    // Health modal (Settings → H)
    pub fn open_health_modal(&mut self) {
        self.show_health_modal = Some(HealthModal {
            items: Vec::new(),
            generated: String::new(),
            loading: true,
        });
    }

    pub fn close_health_modal(&mut self) {
        self.show_health_modal = None;
    }

    pub fn set_health_items(&mut self, items: Vec<crate::diagnostics::HealthItem>) {
        if let Some(modal) = &mut self.show_health_modal {
            modal.items = items;
            modal.loading = false;
            modal.generated = chrono::Local::now().format("%H:%M:%S").to_string();
        }
    }

    // Kernel log modal (Settings → L)
    pub fn open_logs_modal(&mut self) {
        self.last_logs_auto = Instant::now();
        self.show_logs_modal = Some(LogModal {
            filter: String::new(),
            lines: Vec::new(),
            error: None,
            generated: String::new(),
            loading: true,
        });
    }

    pub fn close_logs_modal(&mut self) {
        self.show_logs_modal = None;
    }

    pub fn set_log_lines(&mut self, result: Result<Vec<String>, String>) {
        if let Some(modal) = &mut self.show_logs_modal {
            modal.loading = false;
            modal.generated = chrono::Local::now().format("%H:%M:%S").to_string();
            match result {
                Ok(lines) => {
                    modal.lines = lines;
                    modal.error = None;
                }
                Err(e) => {
                    modal.lines.clear();
                    modal.error = Some(e);
                }
            }
        }
    }

    pub fn log_modal_handle_char(&mut self, c: char) {
        if let Some(modal) = &mut self.show_logs_modal {
            if modal.filter.len() < 120 {
                modal.filter.push(c);
            }
        }
    }

    pub fn log_modal_backspace(&mut self) {
        if let Some(modal) = &mut self.show_logs_modal {
            modal.filter.pop();
        }
    }

    // Runtime overrides modal (Settings → C)
    pub fn open_overrides_modal(&mut self) {
        let settings = self.overrides_mgr.settings.clone();
        self.show_overrides_modal = Some(OverridesModal {
            mode: settings.mode,
            log_level: settings.log_level,
            allow_lan: settings.allow_lan,
            ipv6: settings.ipv6,
            unified_delay: settings.unified_delay,
            active_field: 0,
        });
    }

    pub fn close_overrides_modal(&mut self) {
        self.show_overrides_modal = None;
    }

    pub fn overrides_modal_next_field(&mut self) {
        if let Some(modal) = &mut self.show_overrides_modal {
            modal.active_field = (modal.active_field + 1) % OVERRIDES_MODAL_FIELDS;
        }
    }

    pub fn overrides_modal_prev_field(&mut self) {
        if let Some(modal) = &mut self.show_overrides_modal {
            modal.active_field = (modal.active_field + OVERRIDES_MODAL_FIELDS - 1)
                % OVERRIDES_MODAL_FIELDS;
        }
    }

    /// 空格 / 方向键：在「沿用订阅 → 各合法取值 → 关闭」之间循环当前字段
    pub fn overrides_modal_cycle_value(&mut self) {
        let Some(modal) = &mut self.show_overrides_modal else {
            return;
        };
        match modal.active_field {
            0 => {
                modal.mode = match modal.mode.as_deref() {
                    None => Some("rule".to_string()),
                    Some("rule") => Some("global".to_string()),
                    Some("global") => Some("direct".to_string()),
                    _ => None,
                }
            }
            1 => {
                modal.log_level = match modal.log_level.as_deref() {
                    None => Some("silent".to_string()),
                    Some("silent") => Some("error".to_string()),
                    Some("error") => Some("warning".to_string()),
                    Some("warning") => Some("info".to_string()),
                    Some("info") => Some("debug".to_string()),
                    _ => None,
                }
            }
            field => {
                let slot = match field {
                    2 => &mut modal.allow_lan,
                    3 => &mut modal.ipv6,
                    _ => &mut modal.unified_delay,
                };
                *slot = match *slot {
                    None => Some(true),
                    Some(true) => Some(false),
                    Some(false) => None,
                };
            }
        }
    }

    /// 当前弹窗内容转设置（值都来自循环选择，必然合法）
    pub fn overrides_modal_to_settings(&self) -> Result<OverridesSettings, String> {
        let modal = self
            .show_overrides_modal
            .as_ref()
            .ok_or("运行时配置弹窗未打开")?;
        Ok(OverridesSettings {
            mode: modal.mode.clone(),
            log_level: modal.log_level.clone(),
            allow_lan: modal.allow_lan,
            ipv6: modal.ipv6,
            unified_delay: modal.unified_delay,
        })
    }

    /// 保存弹窗内容：写 data/overrides.json + 重新注入 config.yaml（调用方负责热重载）
    pub fn save_overrides_from_modal(&mut self) -> Result<OverridesSettings, String> {
        let settings = self.overrides_modal_to_settings()?;
        self.overrides_mgr.settings = settings.clone();
        self.overrides_mgr.save();
        self.close_overrides_modal();
        Ok(settings)
    }

    // Backup modal (Settings → B)
    pub fn open_backup_modal(&mut self) {
        let entries = crate::backup::list_backups(std::path::Path::new(crate::backup::BACKUP_DIR));
        self.show_backup_modal = Some(BackupModal {
            entries,
            selected: 0,
            pending: None,
            loading: false,
        });
    }

    pub fn close_backup_modal(&mut self) {
        self.show_backup_modal = None;
    }

    pub fn set_backup_entries(&mut self, entries: Vec<crate::backup::BackupEntry>) {
        if let Some(modal) = &mut self.show_backup_modal {
            modal.entries = entries;
            modal.loading = false;
            if modal.selected >= modal.entries.len() {
                modal.selected = modal.entries.len().saturating_sub(1);
            }
        }
    }

    pub fn backup_modal_next(&mut self) {
        if let Some(modal) = &mut self.show_backup_modal {
            if !modal.entries.is_empty() {
                modal.selected = (modal.selected + 1) % modal.entries.len();
            }
        }
    }

    pub fn backup_modal_prev(&mut self) {
        if let Some(modal) = &mut self.show_backup_modal {
            if !modal.entries.is_empty() {
                modal.selected = (modal.selected + modal.entries.len() - 1) % modal.entries.len();
            }
        }
    }

    /// 返回当前选中备份名（用于恢复）；同时清掉待确认状态。
    pub fn backup_modal_selected_name(&mut self) -> Option<String> {
        let modal = self.show_backup_modal.as_mut()?;
        if modal.entries.is_empty() {
            return None;
        }
        let name = modal.entries[modal.selected].name.clone();
        Some(name)
    }

    pub fn backup_modal_set_pending(&mut self, name: String) {
        if let Some(modal) = &mut self.show_backup_modal {
            modal.pending = Some(name);
        }
    }

    pub fn backup_modal_clear_pending(&mut self) {
        if let Some(modal) = &mut self.show_backup_modal {
            modal.pending = None;
        }
    }

    pub fn backup_modal_pending(&self) -> Option<String> {
        self.show_backup_modal
            .as_ref()
            .and_then(|m| m.pending.clone())
    }

    // ---- Providers（Settings → V） ----

    pub fn open_providers_modal(&mut self) {
        self.show_providers_modal = Some(ProvidersModal {
            kind: ProviderKind::Rule,
            snapshot: ProvidersSnapshot::default(),
            selected: 0,
            loading: true,
            message: None,
        });
    }

    pub fn close_providers_modal(&mut self) {
        self.show_providers_modal = None;
    }

    /// 载入结果：保留当前标签页与选中位置（列表可能变短）
    pub fn set_providers_snapshot(&mut self, snapshot: ProvidersSnapshot) {
        if let Some(modal) = &mut self.show_providers_modal {
            modal.snapshot = snapshot;
            modal.loading = false;
            let len = match modal.kind {
                ProviderKind::Proxy => modal.snapshot.proxies.len(),
                ProviderKind::Rule => modal.snapshot.rules.len(),
            };
            if modal.selected >= len {
                modal.selected = len.saturating_sub(1);
            }
        }
    }

    pub fn providers_modal_set_message(&mut self, message: Option<String>) {
        if let Some(modal) = &mut self.show_providers_modal {
            modal.message = message;
        }
    }

    pub fn providers_modal_toggle_kind(&mut self) {
        if let Some(modal) = &mut self.show_providers_modal {
            modal.kind = match modal.kind {
                ProviderKind::Proxy => ProviderKind::Rule,
                ProviderKind::Rule => ProviderKind::Proxy,
            };
            modal.selected = 0;
        }
    }

    pub fn providers_modal_len(&self) -> usize {
        let Some(modal) = &self.show_providers_modal else {
            return 0;
        };
        match modal.kind {
            ProviderKind::Proxy => modal.snapshot.proxies.len(),
            ProviderKind::Rule => modal.snapshot.rules.len(),
        }
    }

    pub fn providers_modal_next(&mut self) {
        let len = self.providers_modal_len();
        if len == 0 {
            return;
        }
        if let Some(modal) = &mut self.show_providers_modal {
            modal.selected = (modal.selected + 1) % len;
        }
    }

    pub fn providers_modal_prev(&mut self) {
        let len = self.providers_modal_len();
        if len == 0 {
            return;
        }
        if let Some(modal) = &mut self.show_providers_modal {
            modal.selected = (modal.selected + len - 1) % len;
        }
    }

    /// 当前选中项（name + kind），用于触发更新
    pub fn providers_modal_selected(&self) -> Option<(String, ProviderKind)> {
        let modal = self.show_providers_modal.as_ref()?;
        let item: Option<&ProviderInfo> = match modal.kind {
            ProviderKind::Proxy => modal.snapshot.proxies.get(modal.selected),
            ProviderKind::Rule => modal.snapshot.rules.get(modal.selected),
        };
        item.map(|p| (p.name.clone(), p.kind))
    }

    /// 恢复后让内存里的管理器与磁盘一致（rules.json 等直接被覆盖）。
    pub fn reload_managers_from_disk(&mut self) {
        self.dns_mgr = crate::dnsconf::DnsManager::new();
        self.overrides_mgr = crate::overrides::OverridesManager::new();
        self.rule_mgr = crate::rules::RuleManager::new();
    }

    pub fn rule_modal_handle_char(&mut self, c: char) {
        if let Some(modal) = &mut self.show_add_rule_modal {
            modal.input.push(c);
        }
    }

    pub fn rule_modal_backspace(&mut self) {
        if let Some(modal) = &mut self.show_add_rule_modal {
            modal.input.pop();
        }
    }

    // DNS Modal
    pub fn open_dns_modal(&mut self) {
        let settings = self.dns_mgr.settings.clone();
        self.show_dns_modal = Some(DnsModal {
            direct_system: settings.direct_use_system_dns,
            nameserver: settings.nameserver.join(", "),
            fallback: settings.fallback.join(", "),
            proxy_nameserver: settings.proxy_server_nameserver.join(", "),
            default_nameserver: settings.default_nameserver.join(", "),
            policies: format_policies(&settings.policies),
            active_field: 0,
        });
    }

    pub fn close_dns_modal(&mut self) {
        self.show_dns_modal = None;
    }

    pub fn dns_modal_next_field(&mut self) {
        if let Some(modal) = &mut self.show_dns_modal {
            modal.active_field = (modal.active_field + 1) % DNS_MODAL_FIELDS;
        }
    }

    pub fn dns_modal_prev_field(&mut self) {
        if let Some(modal) = &mut self.show_dns_modal {
            modal.active_field = (modal.active_field + DNS_MODAL_FIELDS - 1) % DNS_MODAL_FIELDS;
        }
    }

    pub fn dns_modal_toggle(&mut self) {
        if let Some(modal) = &mut self.show_dns_modal {
            if modal.active_field == 0 {
                modal.direct_system = !modal.direct_system;
            }
        }
    }

    pub fn dns_modal_handle_char(&mut self, c: char) {
        if let Some(modal) = &mut self.show_dns_modal {
            let field = match modal.active_field {
                1 => &mut modal.nameserver,
                2 => &mut modal.fallback,
                3 => &mut modal.proxy_nameserver,
                4 => &mut modal.default_nameserver,
                5 => &mut modal.policies,
                _ => return, // 开关字段不接收字符
            };
            if field.chars().count() < 512 {
                field.push(c);
            }
        }
    }

    pub fn dns_modal_backspace(&mut self) {
        if let Some(modal) = &mut self.show_dns_modal {
            let field = match modal.active_field {
                1 => &mut modal.nameserver,
                2 => &mut modal.fallback,
                3 => &mut modal.proxy_nameserver,
                4 => &mut modal.default_nameserver,
                5 => &mut modal.policies,
                _ => return,
            };
            field.pop();
        }
    }

    /// 校验弹窗输入并转为可持久化的 DNS 设置（策略格式错误时返回提示，弹窗保持打开）
    pub fn dns_modal_to_settings(&self) -> Result<DnsSettings, String> {
        let modal = self.show_dns_modal.as_ref().ok_or("DNS 弹窗未打开")?;
        Ok(DnsSettings {
            direct_use_system_dns: modal.direct_system,
            nameserver: parse_list(&modal.nameserver),
            fallback: parse_list(&modal.fallback),
            proxy_server_nameserver: parse_list(&modal.proxy_nameserver),
            default_nameserver: parse_list(&modal.default_nameserver),
            policies: parse_policies(&modal.policies)?,
        })
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
