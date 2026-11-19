//! DNS 配置：Settings 页 DNS 弹窗的覆盖项。
//!
//! - 持久化到 `data/dns.json`（相对工作目录，与 `rules.json` 一样属于运行数据）；
//! - 每次生成/更新生效配置后，由 `inject_dns_settings` 把用户配置合并进 `dns` 段；
//! - 默认行为：DIRECT 规则命中的域名自动使用系统 DNS（订阅里的公共 DNS
//!   无法解析内网域名，会返回 NXDOMAIN）。

use crate::rules::{RuleItem, RuleTarget};
use serde::{Deserialize, Serialize};
use serde_yaml::Mapping;
use std::fs;
use std::path::PathBuf;

pub const DNS_CONFIG_FILE: &str = "data/dns.json";

fn default_true() -> bool {
    true
}

/// 一条域名 DNS 策略，对应 mihomo `dns.nameserver-policy` 的键值对
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsPolicy {
    pub domain: String,
    pub server: String,
}

/// 用户可编辑的 DNS 覆盖项；空列表/空策略表示沿用订阅里的值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsSettings {
    /// DIRECT 规则命中的域名自动改用系统 DNS（内网域名必需）
    #[serde(default = "default_true")]
    pub direct_use_system_dns: bool,
    #[serde(default)]
    pub nameserver: Vec<String>,
    #[serde(default)]
    pub fallback: Vec<String>,
    #[serde(default)]
    pub proxy_server_nameserver: Vec<String>,
    #[serde(default)]
    pub default_nameserver: Vec<String>,
    #[serde(default)]
    pub policies: Vec<DnsPolicy>,
}

impl Default for DnsSettings {
    fn default() -> Self {
        Self {
            direct_use_system_dns: true,
            nameserver: Vec::new(),
            fallback: Vec::new(),
            proxy_server_nameserver: Vec::new(),
            default_nameserver: Vec::new(),
            policies: Vec::new(),
        }
    }
}

impl DnsSettings {
    /// 用户是否显式覆盖了任何 DNS 字段
    pub fn has_overrides(&self) -> bool {
        !self.nameserver.is_empty()
            || !self.fallback.is_empty()
            || !self.proxy_server_nameserver.is_empty()
            || !self.default_nameserver.is_empty()
            || !self.policies.is_empty()
    }

    /// 当前设置是否需要对配置做任何 DNS 改动
    pub fn is_active(&self, rules: &[RuleItem]) -> bool {
        self.has_overrides()
            || (self.direct_use_system_dns && !auto_policy_domains(rules).is_empty())
    }
}

pub struct DnsManager {
    pub file_path: PathBuf,
    pub settings: DnsSettings,
}

impl DnsManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            file_path: PathBuf::from(DNS_CONFIG_FILE),
            settings: DnsSettings::default(),
        };
        mgr.load();
        mgr
    }

    pub fn load(&mut self) {
        if self.file_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.file_path) {
                if let Ok(settings) = serde_json::from_str::<DnsSettings>(&content) {
                    self.settings = settings;
                    return;
                }
            }
        }
        self.settings = DnsSettings::default();
        self.save();
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.settings) {
            let _ = fs::write(&self.file_path, json);
        }
    }
}

/// 逗号分隔的列表输入（空项自动丢弃）
pub fn parse_list(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// 解析策略输入：`域名=服务器; 域名=服务器`（分号分隔，同域名后者覆盖前者）
pub fn parse_policies(input: &str) -> Result<Vec<DnsPolicy>, String> {
    let mut policies: Vec<DnsPolicy> = Vec::new();
    for raw in input.split(';') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let Some((domain, server)) = raw.split_once('=') else {
            return Err(format!("DNS 策略 '{}' 缺少 '='（格式：域名=服务器）", raw));
        };
        let domain = domain.trim();
        let server = server.trim();
        if domain.is_empty() || server.is_empty() {
            return Err(format!("DNS 策略 '{}' 的域名或服务器为空", raw));
        }
        match policies.iter_mut().find(|p| p.domain == domain) {
            Some(existing) => existing.server = server.to_string(),
            None => policies.push(DnsPolicy {
                domain: domain.to_string(),
                server: server.to_string(),
            }),
        }
    }
    Ok(policies)
}

/// 策略列表 → 弹窗文本（`dns-mgr` 与 UI 共用）
pub fn format_policies(policies: &[DnsPolicy]) -> String {
    policies
        .iter()
        .map(|p| format!("{}={}", p.domain, p.server))
        .collect::<Vec<_>>()
        .join("; ")
}

fn k(name: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(name.to_string())
}

fn yaml_list(items: &[String]) -> serde_yaml::Value {
    serde_yaml::Value::Sequence(
        items
            .iter()
            .map(|s| serde_yaml::Value::String(s.clone()))
            .collect(),
    )
}

/// DIRECT 规则中可自动加系统 DNS 策略的域名（DOMAIN 精确 / DOMAIN-SUFFIX 子域）
fn auto_policy_domains(rules: &[RuleItem]) -> Vec<String> {
    rules
        .iter()
        .filter(|r| r.target == RuleTarget::Direct)
        .filter(|r| r.rule_type == "DOMAIN" || r.rule_type == "DOMAIN-SUFFIX")
        .map(|r| match r.rule_type.as_str() {
            "DOMAIN" => r.value.clone(),
            _ => format!("+.{}.", r.value).trim_end_matches('.').to_string(),
        })
        .collect()
}

/// 把用户 DNS 配置合并进 YAML 的 `dns` 段：
/// - 非空列表覆盖同名键；空列表保持订阅原值；
/// - 用户策略优先写入（覆盖订阅同名策略），再补 DIRECT 规则的系统 DNS 策略（仅缺失）；
/// - 订阅没有 `dns` 段时，只有用户显式提供 nameserver 才创建；
/// - `dns.enable` 为 false 时，仅在用户显式覆盖且有 nameserver 可用的情况下启用。
pub fn inject_dns_settings(yaml: &mut serde_yaml::Value, settings: &DnsSettings, rules: &[RuleItem]) {
    if !settings.is_active(rules) {
        return;
    }
    let Some(map) = yaml.as_mapping_mut() else {
        return;
    };

    if !map.contains_key(&k("dns")) {
        if settings.nameserver.is_empty() {
            return; // 订阅交给系统解析，无需创建 dns 段
        }
        let mut dns = Mapping::new();
        dns.insert(k("enable"), serde_yaml::Value::Bool(true));
        dns.insert(k("nameserver"), yaml_list(&settings.nameserver));
        let defaults = if settings.default_nameserver.is_empty() {
            vec!["223.6.6.6".to_string(), "system".to_string()]
        } else {
            settings.default_nameserver.clone()
        };
        dns.insert(k("default-nameserver"), yaml_list(&defaults));
        map.insert(k("dns"), serde_yaml::Value::Mapping(dns));
    }

    let Some(dns_map) = map.get_mut(&k("dns")).and_then(|v| v.as_mapping_mut()) else {
        return;
    };

    let enabled = dns_map
        .get(&k("enable"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !enabled {
        if !settings.has_overrides() {
            return; // 仅自动策略不足以在 DNS 关闭时生效
        }
        let existing_ns = dns_map
            .get(&k("nameserver"))
            .and_then(|v| v.as_sequence())
            .map_or(false, |s| !s.is_empty());
        if settings.nameserver.is_empty() && !existing_ns {
            return;
        }
        dns_map.insert(k("enable"), serde_yaml::Value::Bool(true));
    }

    if !settings.nameserver.is_empty() {
        dns_map.insert(k("nameserver"), yaml_list(&settings.nameserver));
    }
    if !settings.fallback.is_empty() {
        dns_map.insert(k("fallback"), yaml_list(&settings.fallback));
    }
    if !settings.proxy_server_nameserver.is_empty() {
        dns_map.insert(
            k("proxy-server-nameserver"),
            yaml_list(&settings.proxy_server_nameserver),
        );
    }
    if !settings.default_nameserver.is_empty() {
        dns_map.insert(
            k("default-nameserver"),
            yaml_list(&settings.default_nameserver),
        );
    }

    let policy_entry = dns_map
        .entry(k("nameserver-policy"))
        .or_insert_with(|| serde_yaml::Value::Mapping(Mapping::new()));
    if !policy_entry.is_mapping() {
        *policy_entry = serde_yaml::Value::Mapping(Mapping::new());
    }
    let Some(policy_map) = policy_entry.as_mapping_mut() else {
        return;
    };

    // 用户显式策略优先（覆盖订阅同名项）
    for policy in &settings.policies {
        policy_map.insert(k(&policy.domain), serde_yaml::Value::String(policy.server.clone()));
    }

    // DIRECT 规则自动系统 DNS 策略：只补缺失项，不覆盖用户/订阅已有策略
    if settings.direct_use_system_dns {
        for domain in auto_policy_domains(rules) {
            let key = k(&domain);
            if !policy_map.contains_key(&key) {
                policy_map.insert(key, serde_yaml::Value::String("system".into()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(rule_type: &str, value: &str, target: RuleTarget) -> RuleItem {
        RuleItem {
            rule_type: rule_type.to_string(),
            value: value.to_string(),
            target,
        }
    }

    fn settings(nameserver: &[&str]) -> DnsSettings {
        DnsSettings {
            direct_use_system_dns: true,
            nameserver: nameserver.iter().map(|s| s.to_string()).collect(),
            ..DnsSettings::default()
        }
    }

    fn yaml(raw: &str) -> serde_yaml::Value {
        serde_yaml::from_str(raw).unwrap()
    }

    fn policy_of<'a>(val: &'a serde_yaml::Value, key: &str) -> Option<&'a str> {
        val.get("dns")
            .and_then(|d| d.get("nameserver-policy"))
            .and_then(|p| p.get(key))
            .and_then(|v| v.as_str())
    }

    #[test]
    fn parses_lists_and_policies() {
        assert_eq!(parse_list(" 223.6.6.6, , https://doh.pub/dns-query "),
            vec!["223.6.6.6", "https://doh.pub/dns-query"]);

        let policies = parse_policies("+.corp.com=192.168.121.6; bi.corp.com=system;").unwrap();
        assert_eq!(policies.len(), 2);
        assert_eq!(policies[0].domain, "+.corp.com");
        assert_eq!(policies[0].server, "192.168.121.6");
        assert_eq!(format_policies(&policies), "+.corp.com=192.168.121.6; bi.corp.com=system");

        assert!(parse_policies("no-equals").is_err());
        assert!(parse_policies("=system").is_err());
        assert!(parse_policies("a.com=").is_err());
    }

    #[test]
    fn direct_rules_inject_system_policy_and_ignore_others() {
        let mut val = yaml(
            r#"
dns:
  enable: true
  nameserver:
    - 223.6.6.6
rules:
  - MATCH,DIRECT
"#,
        );
        let rules = vec![
            rule("DOMAIN", "bi.example.com", RuleTarget::Direct),
            rule("DOMAIN-SUFFIX", "corp.example", RuleTarget::Direct),
            rule("DOMAIN-SUFFIX", "public.example", RuleTarget::Proxy),
            rule("DOMAIN-KEYWORD", "keyword", RuleTarget::Direct),
        ];

        inject_dns_settings(&mut val, &settings(&[]), &rules);

        assert_eq!(policy_of(&val, "bi.example.com"), Some("system"));
        assert_eq!(policy_of(&val, "+.corp.example"), Some("system"));
        assert_eq!(policy_of(&val, "+.public.example"), None);
        assert_eq!(policy_of(&val, "+.keyword"), None);
    }

    #[test]
    fn existing_policy_and_user_policy_both_win_over_auto() {
        let mut val = yaml(
            r#"
dns:
  enable: true
  nameserver-policy:
    '+.corp.example': '192.168.121.6'
"#,
        );
        let mut cfg = settings(&[]);
        cfg.policies = vec![DnsPolicy {
            domain: "+.internal.example".to_string(),
            server: "10.0.0.1".to_string(),
        }];
        let rules = vec![rule("DOMAIN-SUFFIX", "corp.example", RuleTarget::Direct)];

        inject_dns_settings(&mut val, &cfg, &rules);

        assert_eq!(policy_of(&val, "+.corp.example"), Some("192.168.121.6"));
        assert_eq!(policy_of(&val, "+.internal.example"), Some("10.0.0.1"));
    }

    #[test]
    fn overrides_nameserver_and_related_sections() {
        let mut val = yaml(
            r#"
dns:
  enable: true
  nameserver:
    - 223.6.6.6
  fallback:
    - 119.29.29.29
"#,
        );
        let mut cfg = settings(&["192.168.121.6", "https://doh.pub/dns-query"]);
        cfg.fallback = vec!["8.8.8.8".to_string()];
        cfg.proxy_server_nameserver = vec!["223.6.6.6".to_string()];
        cfg.default_nameserver = vec!["223.6.6.6".to_string()];

        inject_dns_settings(&mut val, &cfg, &[]);

        let dns = val.get("dns").unwrap();
        let ns: Vec<&str> = dns
            .get("nameserver")
            .unwrap()
            .as_sequence()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(ns, vec!["192.168.121.6", "https://doh.pub/dns-query"]);
        assert_eq!(
            dns.get("fallback").unwrap().as_sequence().unwrap()[0].as_str(),
            Some("8.8.8.8")
        );
        assert_eq!(
            dns.get("proxy-server-nameserver").unwrap().as_sequence().unwrap()[0].as_str(),
            Some("223.6.6.6")
        );
        assert_eq!(
            dns.get("default-nameserver").unwrap().as_sequence().unwrap()[0].as_str(),
            Some("223.6.6.6")
        );
    }

    #[test]
    fn empty_overrides_leave_subscription_dns_untouched() {
        let mut val = yaml(
            r#"
dns:
  enable: true
  nameserver:
    - 223.6.6.6
  fallback:
    - 119.29.29.29
"#,
        );
        let before = serde_yaml::to_string(&val).unwrap();
        inject_dns_settings(&mut val, &settings(&[]), &[]);
        assert_eq!(before, serde_yaml::to_string(&val).unwrap());
    }

    #[test]
    fn missing_dns_section_is_created_only_with_nameserver() {
        let mut val = yaml("rules:\n  - MATCH,DIRECT\n");
        let direct = vec![rule("DOMAIN", "a.example", RuleTarget::Direct)];

        // 仅 DIRECT 自动策略：没有 dns 段时不创建（系统解析本就可用）
        inject_dns_settings(&mut val, &settings(&[]), &direct);
        assert!(val.get("dns").is_none());

        // 显式 nameserver：创建并启用 dns
        inject_dns_settings(&mut val, &settings(&["192.168.121.6"]), &direct);
        let dns = val.get("dns").unwrap();
        assert_eq!(dns.get("enable").unwrap().as_bool(), Some(true));
        assert_eq!(dns.get("nameserver").unwrap().as_sequence().unwrap().len(), 1);
        assert!(dns.get("default-nameserver").unwrap().as_sequence().unwrap().len() >= 1);
    }

    #[test]
    fn overrides_enable_disabled_dns() {
        let mut val = yaml(
            r#"
dns:
  enable: false
  nameserver:
    - 223.6.6.6
"#,
        );
        inject_dns_settings(&mut val, &settings(&["192.168.121.6"]), &[]);
        assert_eq!(
            val.get("dns").unwrap().get("enable").unwrap().as_bool(),
            Some(true)
        );
    }
}