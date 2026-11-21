//! 运行配置覆盖：把用户显式设置的 mode / log-level / allow-lan / ipv6 /
//! unified-delay 写回 `data/config.yaml`，在 `apply_rules_to_config()` 的流水线
//! 里与规则、DNS 一起重新注入，避免被订阅更新覆盖。
//!
//! 持久化文件为 `data/overrides.json`；字段为 `null` 表示沿用订阅配置。

use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub const OVERRIDES_FILE: &str = "data/overrides.json";

/// 与 mihomo `mode` 字段一致（小写）
pub const MODE_CHOICES: [&str; 3] = ["rule", "global", "direct"];
/// 与 mihomo `log-level` 字段一致（小写）
pub const LOG_LEVEL_CHOICES: [&str; 5] = ["silent", "error", "warning", "info", "debug"];

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct OverridesSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_lan: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unified_delay: Option<bool>,
}

impl OverridesSettings {
    pub fn has_overrides(&self) -> bool {
        self.mode.is_some()
            || self.log_level.is_some()
            || self.allow_lan.is_some()
            || self.ipv6.is_some()
            || self.unified_delay.is_some()
    }

    /// Settings 页与弹窗使用的紧凑摘要，例如 `mode rule · log info · LAN on`
    pub fn summary(&self) -> String {
        if !self.has_overrides() {
            return "INHERIT".to_string();
        }
        let mut parts: Vec<String> = Vec::new();
        if let Some(mode) = &self.mode {
            parts.push(format!("mode {mode}"));
        }
        if let Some(level) = &self.log_level {
            parts.push(format!("log {level}"));
        }
        if let Some(on) = self.allow_lan {
            parts.push(format!("LAN {}", if on { "on" } else { "off" }));
        }
        if let Some(on) = self.ipv6 {
            parts.push(format!("IPv6 {}", if on { "on" } else { "off" }));
        }
        if let Some(on) = self.unified_delay {
            parts.push(format!("UDP-delay {}", if on { "on" } else { "off" }));
        }
        parts.join(" · ")
    }

    /// 丢弃 JSON 中手写进来的非法值，避免把坏配置写进内核
    fn sanitize(&mut self) {
        if let Some(mode) = self.mode.take() {
            self.mode = normalize_mode(&mode).unwrap_or(None);
        }
        if let Some(level) = self.log_level.take() {
            self.log_level = normalize_log_level(&level).unwrap_or(None);
        }
    }
}

/// 校验并归一化用户输入的 mode（空串表示沿用订阅配置）
pub fn normalize_mode(input: &str) -> Result<Option<String>, String> {
    let value = input.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Ok(None);
    }
    if MODE_CHOICES.contains(&value.as_str()) {
        Ok(Some(value))
    } else {
        Err("mode 只能是 rule / global / direct".to_string())
    }
}

pub fn normalize_log_level(input: &str) -> Result<Option<String>, String> {
    let value = input.trim().to_ascii_lowercase();
    if value.is_empty() {
        return Ok(None);
    }
    if LOG_LEVEL_CHOICES.contains(&value.as_str()) {
        Ok(Some(value))
    } else {
        Err("log-level 只能是 silent / error / warning / info / debug".to_string())
    }
}

pub struct OverridesManager {
    pub file_path: PathBuf,
    pub settings: OverridesSettings,
}

impl OverridesManager {
    pub fn new() -> Self {
        Self::new_at(PathBuf::from(OVERRIDES_FILE))
    }

    pub fn new_at(path: PathBuf) -> Self {
        let mut settings = load_settings(&path);
        settings.sanitize();
        Self {
            file_path: path,
            settings,
        }
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.settings) {
            let _ = crate::fsutil::atomic_write(&self.file_path, json);
        }
    }
}

fn load_settings(path: &Path) -> OverridesSettings {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default()
}

/// 把非空覆盖项写进 config.yaml 顶层；空设置不会改动 YAML。
pub fn inject_overrides(yaml: &mut Value, settings: &OverridesSettings) {
    if !settings.has_overrides() {
        return;
    }
    let Some(map) = yaml.as_mapping_mut() else {
        return;
    };
    let mut insert = |key: &str, value: Value| {
        map.insert(Value::String(key.to_string()), value);
    };
    if let Some(mode) = &settings.mode {
        insert("mode", Value::String(mode.clone()));
    }
    if let Some(level) = &settings.log_level {
        insert("log-level", Value::String(level.clone()));
    }
    if let Some(on) = settings.allow_lan {
        insert("allow-lan", Value::Bool(on));
    }
    if let Some(on) = settings.ipv6 {
        insert("ipv6", Value::Bool(on));
    }
    if let Some(on) = settings.unified_delay {
        insert("unified-delay", Value::Bool(on));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml_with(mode: &str) -> Value {
        serde_yaml::from_str(&format!(
            "mode: {mode}\nproxies: []\n"
        ))
        .expect("yaml")
    }

    #[test]
    fn empty_settings_have_no_overrides() {
        assert!(!OverridesSettings::default().has_overrides());
        assert_eq!(OverridesSettings::default().summary(), "INHERIT");
    }

    #[test]
    fn normalizes_modes_and_rejects_unknown_values() {
        assert_eq!(normalize_mode(" Rule ").unwrap(), Some("rule".to_string()));
        assert_eq!(normalize_mode("GLOBAL").unwrap(), Some("global".to_string()));
        assert_eq!(normalize_mode("").unwrap(), None);
        assert!(normalize_mode("script").is_err());

        assert_eq!(normalize_log_level("Info").unwrap(), Some("info".to_string()));
        assert_eq!(normalize_log_level("").unwrap(), None);
        assert!(normalize_log_level("verbose").is_err());
    }

    #[test]
    fn injects_top_level_keys_and_preserves_others() {
        let mut yaml = yaml_with("rule");
        let settings = OverridesSettings {
            mode: Some("global".to_string()),
            log_level: Some("debug".to_string()),
            allow_lan: Some(true),
            ipv6: Some(false),
            unified_delay: Some(true),
        };
        inject_overrides(&mut yaml, &settings);

        assert_eq!(yaml["mode"].as_str(), Some("global"));
        assert_eq!(yaml["log-level"].as_str(), Some("debug"));
        assert_eq!(yaml["allow-lan"].as_bool(), Some(true));
        assert_eq!(yaml["ipv6"].as_bool(), Some(false));
        assert_eq!(yaml["unified-delay"].as_bool(), Some(true));
        assert!(yaml.get("proxies").is_some());
    }

    #[test]
    fn empty_settings_leave_yaml_untouched() {
        let original = "mode: rule\nallow-lan: false\n";
        let mut yaml: Value = serde_yaml::from_str(original).expect("yaml");
        inject_overrides(&mut yaml, &OverridesSettings::default());
        assert_eq!(yaml["allow-lan"].as_bool(), Some(false));
        assert!(yaml.get("log-level").is_none());
    }

    #[test]
    fn manager_roundtrip_and_sanitizes_bad_values() {
        let path = std::env::temp_dir().join(format!(
            "clash-tui-overrides-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);

        let manager = OverridesManager::new_at(path.clone());
        assert!(!manager.settings.has_overrides());

        fs::write(
            &path,
            r#"{"mode":"bogus","log_level":"info","allow_lan":true}"#,
        )
        .expect("write");
        let loaded = OverridesManager::new_at(path.clone());
        assert_eq!(loaded.settings.mode, None);
        assert_eq!(loaded.settings.log_level, Some("info".to_string()));
        assert_eq!(loaded.settings.allow_lan, Some(true));

        let mut saved = loaded.settings.clone();
        saved.ipv6 = Some(true);
        let to_save = OverridesManager {
            file_path: path.clone(),
            settings: saved,
        };
        to_save.save();
        let reloaded = OverridesManager::new_at(path.clone());
        assert_eq!(reloaded.settings.ipv6, Some(true));
        assert!(reloaded.settings.summary().contains("log info"));

        let _ = fs::remove_file(&path);
    }
}