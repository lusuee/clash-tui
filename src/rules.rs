use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

/// 规则的命中目标：DIRECT（直连，不经过代理）或 PROXY（解析出的代理组）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleTarget {
    Direct,
    #[default]
    Proxy,
}

impl RuleTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            RuleTarget::Direct => "DIRECT",
            RuleTarget::Proxy => "PROXY",
        }
    }
}

impl Serialize for RuleTarget {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RuleTarget {
    /// 兼容旧版 rules.json（缺 target 字段由 `#[serde(default)]` 补 PROXY），
    /// 同时容忍手工编辑的大小写差异与未知值（回落到 PROXY）。
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Option::<String>::deserialize(deserializer)?;
        Ok(match raw.map(|s| s.trim().to_ascii_uppercase()).as_deref() {
            Some("DIRECT") => RuleTarget::Direct,
            _ => RuleTarget::Proxy,
        })
    }
}

/// 用户自定义的路由规则（持久化于 rules.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleItem {
    /// 规则类型: DOMAIN | DOMAIN-SUFFIX | DOMAIN-KEYWORD | IP-CIDR | GEOSITE | RULE-SET | PROCESS
    pub rule_type: String,
    /// 匹配值：域名 / 关键字 / CIDR / geosite 代码 / ruleset 名 / 进程名
    pub value: String,
    /// 命中后的去向：DIRECT 直连 或 PROXY 走代理组（旧存档缺省为 PROXY）
    #[serde(default)]
    pub target: RuleTarget,
    /// 走代理时指定的策略组；None 表示沿用从配置解析出的默认代理组
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

pub struct RuleManager {
    pub file_path: PathBuf,
    pub rules: Vec<RuleItem>,
    /// 当前解析到的目标代理组（用于 UI 展示）
    pub target_hint: String,
    /// 从当前配置解析出的可选策略组（用于 PROXY 目的地循环）
    pub groups: Vec<String>,
}

impl RuleManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            file_path: PathBuf::from("rules.json"),
            rules: Vec::new(),
            target_hint: "PROXY".to_string(),
            groups: Vec::new(),
        };
        mgr.load();
        mgr.groups = config_proxy_groups();
        mgr.target_hint = resolve_current_target_hint();
        mgr
    }

    pub fn load(&mut self) {
        if self.file_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.file_path) {
                if let Ok(rules) = serde_json::from_str::<Vec<RuleItem>>(&content) {
                    self.rules = rules;
                    return;
                }
            }
        }
        self.rules = Vec::new();
        self.save();
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.rules) {
            let _ = crate::fsutil::atomic_write(&self.file_path, json);
        }
    }

    /// 新增规则（自动去重），返回新增的规则。
    /// `default_target` 用于输入未显式写出 `direct:` / `proxy:` 前缀时的去向；
    /// `default_group` 仅在最终去向为 PROXY 时生效（None = 沿用解析出的默认代理组）。
    pub fn add(
        &mut self,
        raw_input: &str,
        default_target: RuleTarget,
        default_group: Option<&str>,
    ) -> Option<RuleItem> {
        let parsed = parse_domain_input(raw_input)?;
        let target = parsed.target.unwrap_or(default_target);
        let group = if target == RuleTarget::Proxy {
            default_group
                .map(str::trim)
                .filter(|g| !g.is_empty())
                .map(|g| g.to_string())
        } else {
            None
        };

        if self
            .rules
            .iter()
            .any(|r| r.value.eq_ignore_ascii_case(&parsed.value) && r.rule_type == parsed.rule_type)
        {
            return None; // 已存在
        }

        let item = RuleItem {
            rule_type: parsed.rule_type,
            value: parsed.value,
            target,
            group,
        };
        self.rules.push(item.clone());
        self.save();
        Some(item)
    }

    /// 删除指定索引的规则
    pub fn remove(&mut self, idx: usize) -> Option<RuleItem> {
        if idx < self.rules.len() {
            let removed = self.rules.remove(idx);
            self.save();
            Some(removed)
        } else {
            None
        }
    }

    /// 弹窗与列表共用的目的地序列：DIRECT → PROXY（默认组）→ 各可选策略组
    pub fn destinations(&self) -> Vec<(RuleTarget, Option<String>)> {
        let mut list = vec![(RuleTarget::Direct, None), (RuleTarget::Proxy, None)];
        for group in &self.groups {
            list.push((RuleTarget::Proxy, Some(group.clone())));
        }
        list
    }

    /// 在目的地序列中把第 idx 条规则循环切到下一位（DIRECT ↔ 默认组 ↔ 指定组）
    pub fn cycle_destination(&mut self, idx: usize) -> Option<RuleItem> {
        let states = self.destinations();
        let current = self.rules.get(idx)?.clone();
        let pos = states
            .iter()
            .position(|(t, g)| *t == current.target && g.as_deref() == current.group.as_deref())
            .unwrap_or(0);
        let (target, group) = states[(pos + 1) % states.len()].clone();

        let rule = self.rules.get_mut(idx)?;
        rule.target = target;
        rule.group = group;
        let updated = rule.clone();
        self.save();
        Some(updated)
    }

    /// 上移一条规则（提高优先级），返回新索引
    pub fn move_up(&mut self, idx: usize) -> Option<usize> {
        if idx == 0 || idx >= self.rules.len() {
            return None;
        }
        self.rules.swap(idx, idx - 1);
        self.save();
        Some(idx - 1)
    }

    /// 下移一条规则（降低优先级），返回新索引
    pub fn move_down(&mut self, idx: usize) -> Option<usize> {
        if idx + 1 >= self.rules.len() {
            return None;
        }
        self.rules.swap(idx, idx + 1);
        self.save();
        Some(idx + 1)
    }
}

/// `parse_domain_input` 的解析结果；`target` 为 None 表示输入未显式指定去向
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRuleInput {
    pub target: Option<RuleTarget>,
    pub rule_type: String,
    pub value: String,
}

/// 解析用户输入（可选的 `direct:` / `proxy:` 前缀用于指定去向）：
///   example.com            -> DOMAIN-SUFFIX, PROXY（沿用调用方默认值）
///   direct:example.com     -> DOMAIN-SUFFIX, DIRECT 直连
///   proxy:full:example.com -> DOMAIN, PROXY
///   suffix:example.com     -> DOMAIN-SUFFIX
///   full:example.com       -> DOMAIN（精确匹配）
///   domain:example.com     -> DOMAIN
///   keyword:google         -> DOMAIN-KEYWORD
pub fn parse_domain_input(raw: &str) -> Option<ParsedRuleInput> {
    let raw = raw.trim();
    if raw.is_empty() || raw.contains(',') || raw.contains(' ') {
        return None;
    }

    // 1. 可选的目标前缀（必须写在整个输入最前面）
    let (target, rest) = match raw.split_once(':') {
        Some((head, tail)) if matches!(head.to_ascii_lowercase().as_str(), "direct" | "proxy") => {
            let tail = tail.trim();
            if tail.is_empty() {
                return None;
            }
            let target = if head.eq_ignore_ascii_case("direct") {
                RuleTarget::Direct
            } else {
                RuleTarget::Proxy
            };
            (Some(target), tail)
        }
        _ => (None, raw),
    };

    // 2. 可选的规则类型前缀
    let (rule_type, value) = match rest.split_once(':') {
        Some((head, tail)) => {
            let kind = match head.to_ascii_lowercase().as_str() {
                "suffix" => Some("DOMAIN-SUFFIX"),
                "full" | "domain" => Some("DOMAIN"),
                "keyword" => Some("DOMAIN-KEYWORD"),
                "ip-cidr" | "cidr" => Some("IP-CIDR"),
                "geosite" | "site" => Some("GEOSITE"),
                "rule-set" | "ruleset" | "set" => Some("RULE-SET"),
                "process" | "proc" => Some("PROCESS"),
                _ => None,
            };
            match kind {
                Some(kind) => {
                    let value = tail.trim();
                    if value.is_empty() || !valid_rule_value(kind, value) {
                        return None;
                    }
                    (kind, value)
                }
                None => {
                    // 未知前缀（例如带端口的 example.com:8080）整体按域名处理
                    let value = rest;
                    if !valid_rule_value("DOMAIN-SUFFIX", value) {
                        return None;
                    }
                    ("DOMAIN-SUFFIX", value)
                }
            }
        }
        None => {
            if !valid_rule_value("DOMAIN-SUFFIX", rest) {
                return None;
            }
            ("DOMAIN-SUFFIX", rest)
        }
    };

    Some(ParsedRuleInput {
        target,
        rule_type: rule_type.to_string(),
        value: value.to_string(),
    })
}

/// 按规则类型校验值：域名类非空；IP-CIDR 需要合法地址与前缀长度；其余非空且不含逗号
fn valid_rule_value(rule_type: &str, value: &str) -> bool {
    if value.is_empty() || value.contains(',') {
        return false;
    }
    match rule_type {
        "IP-CIDR" => value
            .split_once('/')
            .map(|(addr, prefix)| {
                addr.parse::<IpAddr>().is_ok()
                    && prefix
                        .parse::<u8>()
                        .map(|p| p <= 128)
                        .unwrap_or(false)
            })
            .unwrap_or(false),
        _ => true,
    }
}

/// 从当前生效配置 (data/config.yaml) 解析用于展示的目标代理组
pub fn resolve_current_target_hint() -> String {
    let config_path = Path::new("data").join("config.yaml");
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
            return resolve_proxy_target(&yaml_val);
        }
    }
    "PROXY".to_string()
}

/// 从 YAML 配置中选择目标代理组：
/// 1. proxy-groups 中名字含 "proxy" 的组
/// 2. 第一个 proxy-group
/// 3. 第一个节点
/// 4. DIRECT（兜底）
fn resolve_proxy_target(yaml_val: &serde_yaml::Value) -> String {
    if let Some(groups) = yaml_val.get("proxy-groups").and_then(|v| v.as_sequence()) {
        let mut names: Vec<String> = groups
            .iter()
            .filter_map(|g| g.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
            .collect();

        if !names.is_empty() {
            // 优先通用代理组
            if let Some(name) = names
                .iter()
                .find(|n| n.to_lowercase().contains("proxy"))
                .cloned()
            {
                return name;
            }
            // 退化为非直连/拒绝组
            if let Some(name) = names
                .iter()
                .find(|n| {
                    let l = n.to_lowercase();
                    !l.contains("direct") && !l.contains("reject")
                })
                .cloned()
            {
                return name;
            }
            return names.remove(0);
        }
    }

    if let Some(proxies) = yaml_val.get("proxies").and_then(|v| v.as_sequence()) {
        if let Some(name) = proxies
            .first()
            .and_then(|p| p.get("name").and_then(|n| n.as_str()))
        {
            return name.to_string();
        }
    }

    "DIRECT".to_string()
}

/// 从当前生效配置读取所有可选策略组名（供规则目的地循环使用）
pub fn config_proxy_groups() -> Vec<String> {
    let config_path = Path::new("data").join("config.yaml");
    let Ok(content) = fs::read_to_string(&config_path) else {
        return Vec::new();
    };
    let Ok(yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(&content) else {
        return Vec::new();
    };
    proxy_groups_from(&yaml_val)
}

fn proxy_groups_from(yaml_val: &serde_yaml::Value) -> Vec<String> {
    yaml_val
        .get("proxy-groups")
        .and_then(|v| v.as_sequence())
        .map(|groups| {
            groups
                .iter()
                .filter_map(|g| g.get("name").and_then(|n| n.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// 本地规则命中推演结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// 第 index 条规则命中（0 起）
    Match { index: usize, rule: String },
    /// 第 index 条规则需要内核解析（IP-CIDR/RULE-SET/GEOSITE/PROCESS/未知类型）
    KernelOnly { index: usize, rule: String },
    /// 扫描完全部规则仍未命中（会落到 mihomo 的默认策略）
    NoMatch,
}

#[derive(Debug, Clone)]
pub struct ProbeReport {
    pub domain: String,
    pub outcome: ProbeOutcome,
    pub scanned: usize,
}

/// 读取 data/config.yaml 并对域名做一次本地命中推演
pub fn probe_domain_from_config(domain: &str) -> Option<ProbeReport> {
    let domain = domain.trim();
    if domain.is_empty() {
        return None;
    }
    let config_path = Path::new("data").join("config.yaml");
    let content = fs::read_to_string(&config_path).ok()?;
    let yaml_val: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    Some(probe_domain(&yaml_val, domain))
}

/// 只理解域名类规则与 MATCH；遇到需要内核语义（IP-CIDR/RULE-SET/GEOSITE/PROCESS 等）
/// 的规则时停止并标记 KernelOnly，因为无法确定它是否命中。
pub fn probe_domain(yaml_val: &serde_yaml::Value, domain: &str) -> ProbeReport {
    let domain_l = domain.trim().trim_end_matches('.').to_ascii_lowercase();
    let empty = Vec::new();
    let rules = yaml_val
        .get("rules")
        .and_then(|v| v.as_sequence())
        .unwrap_or(&empty);

    let mut scanned = 0;
    let mut outcome = ProbeOutcome::NoMatch;
    for (index, raw) in rules.iter().enumerate() {
        let Some(line) = raw.as_str() else { continue };
        let mut parts = line.split(',');
        let kind = parts.next().unwrap_or("").trim().to_ascii_uppercase();
        let value = parts.next().unwrap_or("").trim().to_ascii_lowercase();
        scanned = index + 1;

        let hit = match kind.as_str() {
            "DOMAIN" => domain_l == value,
            "DOMAIN-SUFFIX" => {
                !value.is_empty() && (domain_l == value || domain_l.ends_with(&format!(".{value}")))
            }
            "DOMAIN-KEYWORD" => !value.is_empty() && domain_l.contains(&value),
            "MATCH" => true,
            _ => {
                outcome = ProbeOutcome::KernelOnly {
                    index,
                    rule: line.to_string(),
                };
                break;
            }
        };
        if hit {
            outcome = ProbeOutcome::Match {
                index,
                rule: line.to_string(),
            };
            break;
        }
    }

    ProbeReport {
        domain: domain.trim().to_string(),
        outcome,
        scanned,
    }
}

/// 将自定义规则注入到 YAML 配置的 rules 列表顶部（规则优先级最高）。
/// 每条规则按自身 target 决定去向：DIRECT 直连，或指定/解析出的代理组。
/// 注入前先移除旧配置里同域名的自定义规则，避免反复 apply 造成重复堆积。
/// 返回 PROXY 类规则实际使用的默认目标代理组
pub fn inject_domain_rules(yaml_val: &mut serde_yaml::Value, rules: &[RuleItem]) -> String {
    let proxy_target = resolve_proxy_target(yaml_val);
    if rules.is_empty() {
        return proxy_target;
    }

    let custom = rules
        .iter()
        .map(|r| {
            let target = match r.target {
                RuleTarget::Direct => "DIRECT".to_string(),
                RuleTarget::Proxy => r
                    .group
                    .as_deref()
                    .map(str::trim)
                    .filter(|g| !g.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| proxy_target.clone()),
            };
            format!("{},{},{}", r.rule_type, r.value, target)
        })
        .collect::<Vec<_>>();

    let Some(map) = yaml_val.as_mapping_mut() else {
        return proxy_target;
    };

    let values: std::collections::HashSet<&str> =
        rules.iter().map(|r| r.value.as_str()).collect();

    let existing: serde_yaml::Sequence = map
        .get(serde_yaml::Value::String("rules".into()))
        .and_then(|v| v.as_sequence())
        .cloned()
        .unwrap_or_default();

    let mut new_rules = serde_yaml::Sequence::new();
    for line in custom {
        new_rules.push(serde_yaml::Value::String(line));
    }
    for r in existing {
        // 旧的同名规则（可能是上次注入、目标已变或类型已变）直接丢弃，避免重复堆积
        let stale = r.as_str().map(|s| {
            s.split(',').nth(1).map(str::trim).map(|p| values.contains(p)).unwrap_or(false)
        });
        if stale == Some(true) {
            continue;
        }
        new_rules.push(r);
    }

    map.insert(
        serde_yaml::Value::String("rules".into()),
        serde_yaml::Value::Sequence(new_rules),
    );

    proxy_target
}

/// 读取 rules.json / data/dns.json 并注入当前生效配置 data/config.yaml
/// （无状态，任何调用路径行为一致）。规则为空但配置了 DNS 时同样执行。
/// 返回 (注入的规则数量, 目标代理组)
pub fn apply_rules_to_config() -> (usize, String) {
    let mgr = RuleManager::new();
    let dns_mgr = crate::dnsconf::DnsManager::new();
    let overrides_mgr = crate::overrides::OverridesManager::new();
    if mgr.rules.is_empty()
        && !dns_mgr.settings.is_active(&mgr.rules)
        && !overrides_mgr.settings.has_overrides()
    {
        return (0, mgr.target_hint);
    }

    let config_path = Path::new("data").join("config.yaml");
    let Ok(content) = fs::read_to_string(&config_path) else {
        return (0, mgr.target_hint);
    };

    if let Ok(mut yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
        let count = mgr.rules.len();
        let target = inject_domain_rules(&mut yaml_val, &mgr.rules);
        crate::dnsconf::inject_dns_settings(&mut yaml_val, &dns_mgr.settings, &mgr.rules);
        crate::overrides::inject_overrides(&mut yaml_val, &overrides_mgr.settings);
        if let Ok(out) = serde_yaml::to_string(&yaml_val) {
            let _ = crate::fsutil::atomic_write(&config_path, out);
        }
        return (count, target);
    }

    (0, mgr.target_hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> ParsedRuleInput {
        parse_domain_input(raw).expect("input should be valid")
    }

    fn rule(rule_type: &str, value: &str, target: RuleTarget) -> RuleItem {
        RuleItem {
            rule_type: rule_type.to_string(),
            value: value.to_string(),
            target,
            group: None,
        }
    }

    #[test]
    fn plain_domain_defaults_to_suffix_without_explicit_target() {
        let p = parse("example.com");
        assert_eq!(p.target, None);
        assert_eq!(p.rule_type, "DOMAIN-SUFFIX");
        assert_eq!(p.value, "example.com");
    }

    #[test]
    fn parses_direct_and_proxy_prefixes() {
        let p = parse("direct:example.com");
        assert_eq!(p.target, Some(RuleTarget::Direct));
        assert_eq!(p.rule_type, "DOMAIN-SUFFIX");
        assert_eq!(p.value, "example.com");

        let p = parse("proxy:example.com");
        assert_eq!(p.target, Some(RuleTarget::Proxy));
        assert_eq!(p.rule_type, "DOMAIN-SUFFIX");
    }

    #[test]
    fn parses_target_combined_with_type_prefix() {
        let p = parse("direct:full:api.example.com");
        assert_eq!(p.target, Some(RuleTarget::Direct));
        assert_eq!(p.rule_type, "DOMAIN");
        assert_eq!(p.value, "api.example.com");

        let p = parse("PROXY:keyword:google");
        assert_eq!(p.target, Some(RuleTarget::Proxy));
        assert_eq!(p.rule_type, "DOMAIN-KEYWORD");
        assert_eq!(p.value, "google");
    }

    #[test]
    fn unknown_prefix_keeps_whole_domain() {
        let p = parse("example.com:8080");
        assert_eq!(p.target, None);
        assert_eq!(p.rule_type, "DOMAIN-SUFFIX");
        assert_eq!(p.value, "example.com:8080");
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(parse_domain_input("").is_none());
        assert!(parse_domain_input("   ").is_none());
        assert!(parse_domain_input("direct:").is_none());
        assert!(parse_domain_input("full:").is_none());
        assert!(parse_domain_input("direct:example com").is_none());
        assert!(parse_domain_input("a,b").is_none());
    }

    #[test]
    fn legacy_rule_item_without_target_defaults_to_proxy() {
        let item: RuleItem =
            serde_json::from_str(r#"{"rule_type":"DOMAIN","value":"a.com"}"#).unwrap();
        assert_eq!(item.target, RuleTarget::Proxy);
    }

    #[test]
    fn target_serde_roundtrip_and_case_insensitive_read() {
        let item = rule("DOMAIN", "a.com", RuleTarget::Direct);
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains(r#""target":"DIRECT""#));

        let from_lower: RuleItem =
            serde_json::from_str(r#"{"rule_type":"DOMAIN","value":"a.com","target":"direct"}"#)
                .unwrap();
        assert_eq!(from_lower.target, RuleTarget::Direct);
    }

    #[test]
    fn injection_uses_per_rule_target() {
        let yaml = r#"
proxy-groups:
  - name: MyProxy
    type: select
"#;
        let mut val: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let rules = vec![
            rule("DOMAIN-SUFFIX", "direct.example", RuleTarget::Direct),
            rule("DOMAIN-KEYWORD", "proxied", RuleTarget::Proxy),
        ];

        let hint = inject_domain_rules(&mut val, &rules);
        assert_eq!(hint, "MyProxy");

        let injected: Vec<String> = val
            .get("rules")
            .and_then(|v| v.as_sequence())
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        assert_eq!(injected[0], "DOMAIN-SUFFIX,direct.example,DIRECT");
        assert_eq!(injected[1], "DOMAIN-KEYWORD,proxied,MyProxy");
    }

    #[test]
    fn reinjection_does_not_duplicate_same_domain_rules() {
        let yaml = r#"
rules:
  - DOMAIN,bi.example.com,OldProxy
  - DOMAIN-SUFFIX,bi.example.com,DIRECT
  - DOMAIN-SUFFIX,other.example,OldProxy
  - MATCH,DIRECT
"#;
        let mut val: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let rules = vec![rule("DOMAIN", "bi.example.com", RuleTarget::Direct)];

        inject_domain_rules(&mut val, &rules);
        inject_domain_rules(&mut val, &rules);

        let rules_out: Vec<String> = val
            .get("rules")
            .and_then(|v| v.as_sequence())
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        let bi: Vec<&String> = rules_out
            .iter()
            .filter(|r| r.split(',').nth(1) == Some("bi.example.com"))
            .collect();
        assert_eq!(bi.len(), 1);
        assert_eq!(bi[0], "DOMAIN,bi.example.com,DIRECT");
        // 不属于自定义规则的订阅规则必须原样保留
        assert!(rules_out.contains(&"DOMAIN-SUFFIX,other.example,OldProxy".to_string()));
        assert_eq!(rules_out.last().unwrap(), "MATCH,DIRECT");
    }

    #[test]
    fn parses_extended_rule_types() {
        assert_eq!(parse("ip-cidr:192.168.0.0/16").rule_type, "IP-CIDR");
        assert_eq!(parse("cidr:10.0.0.0/8").value, "10.0.0.0/8");
        assert_eq!(parse("geosite:cn").rule_type, "GEOSITE");
        assert_eq!(parse("site:netflix").value, "netflix");
        assert_eq!(parse("rule-set:proxy").rule_type, "RULE-SET");
        assert_eq!(parse("ruleset:ads").value, "ads");
        assert_eq!(parse("process:clash-tui.exe").rule_type, "PROCESS");
        assert_eq!(parse("proc:chrome").value, "chrome");

        // CIDR 必须合法
        assert!(parse_domain_input("ip-cidr:192.168.0.0").is_none());
        assert!(parse_domain_input("ip-cidr:999.1.1.1/8").is_none());
        assert!(parse_domain_input("ip-cidr:10.0.0.0/200").is_none());
    }

    #[test]
    fn injection_honours_explicit_group() {
        let yaml = r#"
proxy-groups:
  - name: MyProxy
    type: select
  - name: Streaming
    type: select
"#;
        let mut val: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        let rules = vec![
            RuleItem {
                rule_type: "GEOSITE".into(),
                value: "netflix".into(),
                target: RuleTarget::Proxy,
                group: Some("Streaming".into()),
            },
            rule("RULE-SET", "proxy", RuleTarget::Proxy),
        ];
        inject_domain_rules(&mut val, &rules);
        let injected: Vec<String> = val
            .get("rules")
            .and_then(|v| v.as_sequence())
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        assert_eq!(injected[0], "GEOSITE,netflix,Streaming");
        assert_eq!(injected[1], "RULE-SET,proxy,MyProxy");
    }

    #[test]
    fn cycle_destination_walks_direct_proxy_and_groups() {
        let mut mgr = RuleManager {
            file_path: std::env::temp_dir().join("clash-tui-rules-cycle.json"),
            rules: vec![rule("DOMAIN", "a.com", RuleTarget::Direct)],
            target_hint: "MyProxy".into(),
            groups: vec!["MyProxy".into(), "Streaming".into()],
        };

        let first = mgr.cycle_destination(0).unwrap();
        assert_eq!(first.target, RuleTarget::Proxy);
        assert_eq!(first.group, None);

        let second = mgr.cycle_destination(0).unwrap();
        assert_eq!(second.target, RuleTarget::Proxy);
        assert_eq!(second.group.as_deref(), Some("MyProxy"));

        let third = mgr.cycle_destination(0).unwrap();
        assert_eq!(third.group.as_deref(), Some("Streaming"));

        let wrapped = mgr.cycle_destination(0).unwrap();
        assert_eq!(wrapped.target, RuleTarget::Direct);
        assert_eq!(wrapped.group, None);
        let _ = std::fs::remove_file(&mgr.file_path);
    }

    #[test]
    fn move_up_and_down_swaps_and_clamps() {
        let mut mgr = RuleManager {
            file_path: std::env::temp_dir().join("clash-tui-rules-move.json"),
            rules: vec![
                rule("DOMAIN", "a.com", RuleTarget::Direct),
                rule("DOMAIN", "b.com", RuleTarget::Direct),
                rule("DOMAIN", "c.com", RuleTarget::Direct),
            ],
            target_hint: "PROXY".into(),
            groups: Vec::new(),
        };

        assert_eq!(mgr.move_up(0), None);
        assert_eq!(mgr.move_down(2), None);

        assert_eq!(mgr.move_down(0), Some(1));
        assert_eq!(mgr.rules[0].value, "b.com");
        assert_eq!(mgr.rules[1].value, "a.com");

        assert_eq!(mgr.move_up(2), Some(1));
        assert_eq!(mgr.rules[1].value, "c.com");
        assert_eq!(mgr.rules[2].value, "a.com");
        let _ = std::fs::remove_file(&mgr.file_path);
    }

    fn probe(yaml: &str, domain: &str) -> ProbeOutcome {
        let val: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
        probe_domain(&val, domain).outcome
    }

    #[test]
    fn probe_prefers_the_first_matching_rule() {
        let yaml = r#"
rules:
  - DOMAIN-SUFFIX,example.com,DIRECT
  - DOMAIN,api.example.com,MyProxy
  - DOMAIN-KEYWORD,ads,DIRECT
  - MATCH,MyProxy
"#;
        assert_eq!(
            probe(yaml, "api.example.com"),
            ProbeOutcome::Match {
                index: 0,
                rule: "DOMAIN-SUFFIX,example.com,DIRECT".into()
            }
        );
        assert_eq!(
            probe(yaml, "tracker.ads"),
            ProbeOutcome::Match {
                index: 2,
                rule: "DOMAIN-KEYWORD,ads,DIRECT".into()
            }
        );
        assert_eq!(
            probe(yaml, "unrelated.dev"),
            ProbeOutcome::Match {
                index: 3,
                rule: "MATCH,MyProxy".into()
            }
        );
    }

    #[test]
    fn probe_stops_at_rules_that_need_the_kernel() {
        let yaml = r#"
rules:
  - DOMAIN-SUFFIX,other.dev,DIRECT
  - IP-CIDR,10.0.0.0/8,DIRECT
  - MATCH,MyProxy
"#;
        assert_eq!(
            probe(yaml, "sub.other.dev"),
            ProbeOutcome::Match {
                index: 0,
                rule: "DOMAIN-SUFFIX,other.dev,DIRECT".into()
            }
        );
        assert_eq!(
            probe(yaml, "example.org"),
            ProbeOutcome::KernelOnly {
                index: 1,
                rule: "IP-CIDR,10.0.0.0/8,DIRECT".into()
            }
        );
    }
}
