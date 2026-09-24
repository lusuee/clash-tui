use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// 用户自定义的域名代理规则（持久化于 rules.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleItem {
    /// 规则类型: DOMAIN | DOMAIN-SUFFIX | DOMAIN-KEYWORD
    pub rule_type: String,
    /// 域名或关键字
    pub value: String,
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

    /// 新增规则（自动去重），返回新增的规则
    pub fn add(&mut self, raw_input: &str) -> Option<RuleItem> {
        let (rule_type, value) = parse_domain_input(raw_input)?;

        if self
            .rules
            .iter()
            .any(|r| r.value.eq_ignore_ascii_case(&value) && r.rule_type == rule_type)
        {
            return None; // 已存在
        }

        let item = RuleItem { rule_type, value };
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
}

/// 解析用户输入：
///   example.com            -> DOMAIN-SUFFIX（含所有子域名）
///   suffix:example.com     -> DOMAIN-SUFFIX
///   full:example.com       -> DOMAIN（精确匹配）
///   domain:example.com     -> DOMAIN
///   keyword:google         -> DOMAIN-KEYWORD
pub fn parse_domain_input(raw: &str) -> Option<(String, String)> {
    let raw = raw.trim();
    if raw.is_empty() || raw.contains(',') || raw.contains(' ') {
        return None;
    }

    if let Some((prefix, rest)) = raw.split_once(':') {
        let rule_type = match prefix.to_lowercase().as_str() {
            "suffix" => "DOMAIN-SUFFIX",
            "full" | "domain" => "DOMAIN",
            "keyword" => "DOMAIN-KEYWORD",
            _ => "",
        };
        if !rule_type.is_empty() {
            let value = rest.trim();
            if !value.is_empty() {
                return Some((rule_type.to_string(), value.to_string()));
            }
            return None;
        }
    }

    Some(("DOMAIN-SUFFIX".to_string(), raw.to_string()))
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

/// 将自定义规则注入到 YAML 配置的 rules 列表顶部（规则优先级最高）
/// 返回实际使用的目标代理组
pub fn inject_domain_rules(yaml_val: &mut serde_yaml::Value, rules: &[RuleItem]) -> String {
    if rules.is_empty() {
        return resolve_proxy_target(yaml_val);
    }

    let target = resolve_proxy_target(yaml_val);
    let Some(map) = yaml_val.as_mapping_mut() else {
        return target;
    };

    let custom = rules
        .iter()
        .map(|r| format!("{},{},{}", r.rule_type, r.value, target))
        .collect::<Vec<_>>();

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
        new_rules.push(r);
    }

    map.insert(
        serde_yaml::Value::String("rules".into()),
        serde_yaml::Value::Sequence(new_rules),
    );

    target
}

/// 读取 rules.json 并注入当前生效配置 data/config.yaml（无状态，任何调用路径行为一致）
/// 返回 (注入的规则数量, 目标代理组)
pub fn apply_rules_to_config() -> (usize, String) {
    let mgr = RuleManager::new();
    if mgr.rules.is_empty() {
        return (0, mgr.target_hint);
    }

    let config_path = Path::new("data").join("config.yaml");
    let Ok(content) = fs::read_to_string(&config_path) else {
        return (0, mgr.target_hint);
    };

    if let Ok(mut yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
        let count = mgr.rules.len();
        let target = inject_domain_rules(&mut yaml_val, &mgr.rules);
        if let Ok(out) = serde_yaml::to_string(&yaml_val) {
            let _ = fs::write(&config_path, out);
        }
        return (count, target);
    }

    (0, mgr.target_hint)
}
