use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fs;
use std::path::{Path, PathBuf};

/// 规则的命中目标：DIRECT（直连，不经过代理）或 PROXY（解析出的代理组）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleTarget {
    Direct,
    Proxy,
}

impl RuleTarget {
    pub fn as_str(self) -> &'static str {
        match self {
            RuleTarget::Direct => "DIRECT",
            RuleTarget::Proxy => "PROXY",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            RuleTarget::Direct => RuleTarget::Proxy,
            RuleTarget::Proxy => RuleTarget::Direct,
        }
    }
}

impl Default for RuleTarget {
    fn default() -> Self {
        RuleTarget::Proxy
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

/// 用户自定义的域名路由规则（持久化于 rules.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleItem {
    /// 规则类型: DOMAIN | DOMAIN-SUFFIX | DOMAIN-KEYWORD
    pub rule_type: String,
    /// 域名或关键字
    pub value: String,
    /// 命中后的去向：DIRECT 直连 或 PROXY 走代理组（旧存档缺省为 PROXY）
    #[serde(default)]
    pub target: RuleTarget,
}

pub struct RuleManager {
    pub file_path: PathBuf,
    pub rules: Vec<RuleItem>,
    /// 当前解析到的目标代理组（用于 UI 展示）
    pub target_hint: String,
}

impl RuleManager {
    pub fn new() -> Self {
        let mut mgr = Self {
            file_path: PathBuf::from("rules.json"),
            rules: Vec::new(),
            target_hint: "PROXY".to_string(),
        };
        mgr.load();
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
            let _ = fs::write(&self.file_path, json);
        }
    }

    /// 新增规则（自动去重），返回新增的规则。
    /// `default_target` 用于输入未显式写出 `direct:` / `proxy:` 前缀时的去向。
    pub fn add(&mut self, raw_input: &str, default_target: RuleTarget) -> Option<RuleItem> {
        let parsed = parse_domain_input(raw_input)?;
        let target = parsed.target.unwrap_or(default_target);

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

    /// 切换指定规则的目标（DIRECT ↔ PROXY），返回切换后的规则
    pub fn toggle_target(&mut self, idx: usize) -> Option<RuleItem> {
        let rule = self.rules.get_mut(idx)?;
        rule.target = rule.target.toggle();
        let updated = rule.clone();
        self.save();
        Some(updated)
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
                "suffix" => "DOMAIN-SUFFIX",
                "full" | "domain" => "DOMAIN",
                "keyword" => "DOMAIN-KEYWORD",
                _ => "",
            };
            if kind.is_empty() {
                // 未知前缀（例如带端口的 example.com:8080）整体按域名处理
                ("DOMAIN-SUFFIX", rest)
            } else {
                let value = tail.trim();
                if value.is_empty() {
                    return None;
                }
                (kind, value)
            }
        }
        None => ("DOMAIN-SUFFIX", rest),
    };

    Some(ParsedRuleInput {
        target,
        rule_type: rule_type.to_string(),
        value: value.to_string(),
    })
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

/// 将自定义规则注入到 YAML 配置的 rules 列表顶部（规则优先级最高）。
/// 每条规则按自身 target 决定去向：DIRECT 直连，或解析出的代理组。
/// 注入前先移除旧配置里同域名的自定义规则，避免反复 apply 造成重复堆积。
/// 返回 PROXY 类规则实际使用的目标代理组
pub fn inject_domain_rules(yaml_val: &mut serde_yaml::Value, rules: &[RuleItem]) -> String {
    let proxy_target = resolve_proxy_target(yaml_val);
    if rules.is_empty() {
        return proxy_target;
    }

    let custom = rules
        .iter()
        .map(|r| {
            let target = match r.target {
                RuleTarget::Direct => "DIRECT",
                RuleTarget::Proxy => proxy_target.as_str(),
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
    if mgr.rules.is_empty() && !dns_mgr.settings.is_active(&mgr.rules) {
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
        if let Ok(out) = serde_yaml::to_string(&yaml_val) {
            let _ = fs::write(&config_path, out);
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
}
