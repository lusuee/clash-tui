//! Provider 列表（proxy-providers / rule-providers）。
//!
//! Mihomo 的 `/providers/proxies` 会混入策略组（`vehicleType: Compatible`、
//! 无 `updatedAt`），直接展示会误导用户。这里以 `data/config.yaml` 中的
//! `proxy-providers` / `rule-providers` 为准，再用 REST 返回的数据补全
//! 条目数、规则行为与更新时间；API 里但配置里没有的条目一律忽略。

use chrono::NaiveDateTime;
use serde_json::Value;
use serde_yaml::Value as YamlValue;

pub const CONFIG_PATH: &str = "data/config.yaml";

/// `api updatedAt` 的三种形态
const API_TIME_FORMAT: &str = "%Y/%m/%d %H:%M:%S";
const DISPLAY_TIME_FORMAT: &str = "%Y-%m-%d %H:%M";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    Proxy,
    Rule,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            ProviderKind::Proxy => "PROXY",
            ProviderKind::Rule => "RULE",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProviderInfo {
    pub name: String,
    pub kind: ProviderKind,
    /// 配置里声明的 `type`（http / file / inline）
    pub vehicle: String,
    /// rule-providers 的 behavior（classical / domain / ipcidr），proxy 为 None
    pub behavior: Option<String>,
    /// rule 规则条数 / proxy 节点数；API 缺失时为 0
    pub count: u64,
    /// 展示用更新时间（`YYYY-MM-DD HH:MM`），未知时 `—`
    pub updated: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProvidersSnapshot {
    pub proxies: Vec<ProviderInfo>,
    pub rules: Vec<ProviderInfo>,
}

impl ProvidersSnapshot {
    pub fn is_empty(&self) -> bool {
        self.proxies.is_empty() && self.rules.is_empty()
    }

    pub fn total(&self) -> usize {
        self.proxies.len() + self.rules.len()
    }
}

/// 从配置文本 + 两个 REST 返回构造快照（纯函数，便于测试）。
pub fn parse_snapshot(config_yaml: &str, api_proxies: &Value, api_rules: &Value) -> ProvidersSnapshot {
    let yaml: YamlValue = serde_yaml::from_str(config_yaml).unwrap_or(YamlValue::Null);
    let proxies = build(
        ProviderKind::Proxy,
        section_entries(&yaml, "proxy-providers"),
        api_proxies,
    );
    let rules = build(
        ProviderKind::Rule,
        section_entries(&yaml, "rule-providers"),
        api_rules,
    );
    ProvidersSnapshot { proxies, rules }
}

fn section_entries(yaml: &YamlValue, key: &str) -> Vec<(String, String)> {
    let Some(section) = yaml.get(key).and_then(YamlValue::as_mapping) else {
        return Vec::new();
    };
    section
        .iter()
        .filter_map(|(name, def)| {
            let name = name.as_str()?.to_string();
            let vehicle = def
                .get("type")
                .and_then(YamlValue::as_str)
                .unwrap_or("unknown")
                .to_ascii_lowercase();
            Some((name, vehicle))
        })
        .collect()
}

fn build(kind: ProviderKind, defs: Vec<(String, String)>, api: &Value) -> Vec<ProviderInfo> {
    let entries = api.get("providers").and_then(Value::as_object);
    defs.into_iter()
        .map(|(name, vehicle)| {
            let entry = entries.and_then(|m| m.get(&name));
            let behavior = entry
                .and_then(|e| e.get("behavior"))
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            let (count, updated) = match kind {
                ProviderKind::Rule => (
                    entry
                        .and_then(|e| e.get("ruleCount"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                    format_updated(
                        entry
                            .and_then(|e| e.get("updatedAt"))
                            .and_then(Value::as_str)
                            .unwrap_or(""),
                    ),
                ),
                ProviderKind::Proxy => (
                    entry
                        .and_then(|e| e.get("proxies"))
                        .and_then(Value::as_array)
                        .map(|a| a.len() as u64)
                        .unwrap_or(0),
                    format_updated(
                        entry
                            .and_then(|e| e.get("updatedAt"))
                            .and_then(Value::as_str)
                            .unwrap_or(""),
                    ),
                ),
            };
            ProviderInfo {
                name,
                kind,
                vehicle,
                behavior,
                count,
                updated,
            }
        })
        .collect()
}

/// 把 mihomo 的 `2026/11/20 8:33:14` 统一成 `2026-11-20 08:33`；
/// 零值（`0001/1/1 0:00:00`）或解析失败返回 `—`。
fn format_updated(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return "—".to_string();
    }
    match NaiveDateTime::parse_from_str(raw, API_TIME_FORMAT) {
        Ok(dt) if dt.format(DISPLAY_TIME_FORMAT).to_string() != "0001-01-01 00:00" => {
            dt.format(DISPLAY_TIME_FORMAT).to_string()
        }
        Ok(_) => "—".to_string(),
        Err(_) => raw.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"
proxy-providers:
  provider-a:
    type: http
    url: https://example.com/a.yaml
  provider-b:
    type: file
    path: ./p.yaml
rule-providers:
  rules-a:
    type: http
    behavior: classical
    url: https://example.com/r.yaml
"#;

    fn proxies_api() -> Value {
        serde_json::json!({
            "providers": {
                "provider-a": {
                    "name": "provider-a",
                    "type": "Proxy",
                    "vehicleType": "HTTP",
                    "proxies": [{"name": "n1"}, {"name": "n2"}, {"name": "n3"}],
                    "updatedAt": "2026/11/20 8:33:14"
                },
                "provider-b": {
                    "name": "provider-b",
                    "type": "Proxy",
                    "vehicleType": "File",
                    "proxies": [{"name": "n1"}, {"name": "n2"}],
                    "updatedAt": "0001/1/1 0:00:00"
                },
                "some-group": {
                    "name": "some-group",
                    "type": "Proxy",
                    "vehicleType": "Compatible",
                    "proxies": [{"name": "DIRECT"}]
                }
            }
        })
    }

    fn rules_api() -> Value {
        serde_json::json!({
            "providers": {
                "rules-a": {
                    "name": "rules-a",
                    "type": "Rule",
                    "vehicleType": "HTTP",
                    "behavior": "classical",
                    "ruleCount": 12,
                    "updatedAt": "2026/11/20 8:33:14"
                }
            }
        })
    }

    #[test]
    fn merges_config_defs_with_api_counts() {
        let snap = parse_snapshot(CONFIG, &proxies_api(), &rules_api());
        assert_eq!(snap.proxies.len(), 2, "策略组不应出现在 provider 列表里");
        let a = &snap.proxies[0];
        assert_eq!(a.name, "provider-a");
        assert_eq!(a.vehicle, "http");
        assert_eq!(a.count, 3);
        assert_eq!(a.updated, "2026-11-20 08:33");
        assert!(a.behavior.is_none());

        let b = &snap.proxies[1];
        assert_eq!(b.name, "provider-b");
        assert_eq!(b.vehicle, "file");
        assert_eq!(b.count, 2);
        assert_eq!(b.updated, "—", "零值更新时间应显示为 —");

        let r = &snap.rules[0];
        assert_eq!(r.name, "rules-a");
        assert_eq!(r.count, 12);
        assert_eq!(r.behavior.as_deref(), Some("classical"));
        assert_eq!(r.updated, "2026-11-20 08:33");
    }

    #[test]
    fn missing_api_entry_falls_back_to_empty_stats() {
        let snap = parse_snapshot(CONFIG, &serde_json::json!({}), &serde_json::json!({}));
        assert_eq!(snap.proxies[0].count, 0);
        assert_eq!(snap.proxies[0].updated, "—");
        assert_eq!(snap.rules[0].count, 0);
        assert_eq!(snap.rules[0].updated, "—");
    }

    #[test]
    fn config_without_provider_sections_is_empty() {
        let snap = parse_snapshot(
            "mixed-port: 7897\n",
            &proxies_api(),
            &rules_api(),
        );
        assert!(snap.is_empty());
        assert_eq!(snap.total(), 0);
    }

    #[test]
    fn unknown_api_timestamp_is_kept_verbatim() {
        assert_eq!(format_updated("2026年11月20日"), "2026年11月20日");
        assert_eq!(format_updated(""), "—");
        assert_eq!(format_updated("0001/1/1 0:00:00"), "—");
    }
}