use chrono::{Local, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 默认自动更新间隔（分钟）：24 小时
pub const DEFAULT_AUTO_UPDATE_MINUTES: u32 = 1440;
/// 订阅页 `i` 键循环的自动更新间隔：关闭 / 1 小时 / 6 小时 / 24 小时
pub const AUTO_UPDATE_CHOICES: [u32; 4] = [0, 60, 360, 1440];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionItem {
    pub id: String,
    pub name: String,
    pub url: String,
    pub active: bool,
    pub node_count: usize,
    pub last_updated: String,
    /// 订阅流量（字节），来自响应头 `subscription-userinfo`
    #[serde(default)]
    pub upload: u64,
    #[serde(default)]
    pub download: u64,
    #[serde(default)]
    pub total: u64,
    /// 套餐到期 Unix 时间戳；0 表示未知
    #[serde(default)]
    pub expire: i64,
}

/// 订阅响应头 `subscription-userinfo` 解析结果
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SubUserinfo {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expire: i64,
}

#[derive(Debug, Clone)]
pub struct SubUpdateResult {
    pub sub_id: String,
    pub sub_name: String,
    pub node_count: usize,
    pub last_updated: String,
    pub is_active: bool,
    pub userinfo: SubUserinfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubscriptionStore {
    #[serde(default = "default_auto_update_minutes")]
    auto_update_minutes: u32,
    #[serde(default)]
    items: Vec<SubscriptionItem>,
}

fn default_auto_update_minutes() -> u32 {
    DEFAULT_AUTO_UPDATE_MINUTES
}

#[derive(Debug, Clone)]
pub struct SubscriptionManager {
    pub file_path: PathBuf,
    pub profiles_dir: PathBuf,
    pub subscriptions: Vec<SubscriptionItem>,
    /// 自动更新间隔（分钟，0 = 关闭）
    pub auto_update_minutes: u32,
}

impl SubscriptionManager {
    pub fn new() -> Self {
        let file_path = PathBuf::from("subscriptions.json");
        let profiles_dir = PathBuf::from("profiles");
        let _ = fs::create_dir_all(&profiles_dir);

        let mut mgr = Self {
            file_path,
            profiles_dir,
            subscriptions: Vec::new(),
            auto_update_minutes: DEFAULT_AUTO_UPDATE_MINUTES,
        };
        mgr.load();
        mgr
    }

    pub fn load(&mut self) {
        if self.file_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.file_path) {
                // 新格式：{ auto_update_minutes, items }
                if let Ok(store) = serde_json::from_str::<SubscriptionStore>(&content) {
                    self.subscriptions = store.items;
                    self.auto_update_minutes = store.auto_update_minutes;
                    return;
                }
                // 旧格式：裸数组，沿用默认自动更新间隔
                if let Ok(subs) = serde_json::from_str::<Vec<SubscriptionItem>>(&content) {
                    self.subscriptions = subs;
                    return;
                }
            }
            return;
        }
        self.subscriptions = Vec::new();
        self.save();
    }

    pub fn save(&self) {
        let store = SubscriptionStore {
            auto_update_minutes: self.auto_update_minutes,
            items: self.subscriptions.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&store) {
            let _ = fs::write(&self.file_path, json);
        }
    }

    pub fn apply_update(&mut self, res: &SubUpdateResult) {
        if let Some(sub) = self.subscriptions.iter_mut().find(|s| s.id == res.sub_id) {
            sub.node_count = res.node_count;
            sub.last_updated = res.last_updated.clone();
            sub.upload = res.userinfo.upload;
            sub.download = res.userinfo.download;
            sub.total = res.userinfo.total;
            sub.expire = res.userinfo.expire;
        }
        self.save();
    }

    pub fn add(&mut self, name: String, url: String) -> SubscriptionItem {
        let timestamp = chrono::Utc::now().timestamp();
        let sub_id = format!("sub_{}", timestamp);
        let is_first = self.subscriptions.is_empty();

        let item = SubscriptionItem {
            id: sub_id,
            name,
            url,
            active: is_first,
            node_count: 0,
            last_updated: "Never".to_string(),
            upload: 0,
            download: 0,
            total: 0,
            expire: 0,
        };

        self.subscriptions.push(item.clone());
        self.save();
        item
    }

    /// 编辑订阅名称/URL（不触发下载，由调用方按 URL 是否变化决定）
    pub fn update_meta(
        &mut self,
        id: &str,
        name: String,
        url: String,
    ) -> Option<SubscriptionItem> {
        let updated = if let Some(sub) = self.subscriptions.iter_mut().find(|s| s.id == id) {
            sub.name = name;
            sub.url = url;
            Some(sub.clone())
        } else {
            None
        };
        self.save();
        updated
    }

    pub fn delete(&mut self, idx: usize) -> Option<SubscriptionItem> {
        if idx < self.subscriptions.len() {
            let removed = self.subscriptions.remove(idx);
            if !self.subscriptions.is_empty() && !self.subscriptions.iter().any(|s| s.active) {
                self.subscriptions[0].active = true;
            }
            self.save();
            Some(removed)
        } else {
            None
        }
    }

    pub fn set_active(&mut self, idx: usize, mixed_port: u16) -> Result<SubscriptionItem, String> {
        if idx >= self.subscriptions.len() {
            return Err("Subscription index out of range".to_string());
        }

        let active_sub = self.subscriptions[idx].clone();
        let profile_path = self.profiles_dir.join(format!("{}.yaml", active_sub.id));
        if !profile_path.exists() {
            return Err(format!("Profile file '{}.yaml' not found. Please press 'u' to fetch it first.", active_sub.id));
        }

        let content = fs::read_to_string(&profile_path)
            .map_err(|e| format!("Failed to read profile: {}", e))?;

        deploy_config_yaml(&content, mixed_port)?;

        for (i, sub) in self.subscriptions.iter_mut().enumerate() {
            sub.active = i == idx;
        }
        self.save();

        Ok(active_sub)
    }

    /// 循环切换自动更新间隔（关闭 → 1h → 6h → 24h → 关闭）
    pub fn cycle_auto_update(&mut self) -> u32 {
        let pos = AUTO_UPDATE_CHOICES
            .iter()
            .position(|m| *m == self.auto_update_minutes)
            .unwrap_or(0);
        let next = AUTO_UPDATE_CHOICES[(pos + 1) % AUTO_UPDATE_CHOICES.len()];
        self.auto_update_minutes = next;
        self.save();
        next
    }

    /// 超过自动更新间隔、需要刷新的订阅
    pub fn due_subscriptions(&self) -> Vec<SubscriptionItem> {
        if self.auto_update_minutes == 0 {
            return Vec::new();
        }
        let now = Local::now().naive_local();
        self.subscriptions
            .iter()
            .filter(|s| is_due(&s.last_updated, self.auto_update_minutes, now))
            .cloned()
            .collect()
    }

    pub async fn download_and_update(
        sub_id: String,
        sub_name: String,
        url: String,
        is_active: bool,
        mixed_port: u16,
        profiles_dir: PathBuf,
    ) -> Result<SubUpdateResult, String> {
        if url.is_empty() {
            return Err("Subscription URL is empty".to_string());
        }

        // Mock test links
        if url.contains("example.com") || url.contains("demo") {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let node_count = 18;
            let last_updated = Local::now().format("%Y-%m-%d %H:%M").to_string();
            let out_path = profiles_dir.join(format!("{}.yaml", sub_id));
            if !out_path.exists() {
                let dummy_yaml = "proxies:\n- name: Demo-Node-1\n  type: socks5\n  server: 127.0.0.1\n  port: 1080\n";
                let _ = fs::write(&out_path, dummy_yaml);
                if is_active {
                    let _ = deploy_config_yaml(dummy_yaml, mixed_port);
                }
            }
            return Ok(SubUpdateResult {
                sub_id,
                sub_name,
                node_count,
                last_updated,
                is_active,
                userinfo: SubUserinfo::default(),
            });
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .get(&url)
            .header(
                "User-Agent",
                "ClashMeta/1.18.0 (Windows NT 10.0; Win64; x64) ClashTUI/1.0",
            )
            .send()
            .await
            .map_err(|e| format!("Download error: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("HTTP error {}", resp.status()));
        }

        // subscription-userinfo 在下载 body 前读取
        let userinfo = resp
            .headers()
            .get("subscription-userinfo")
            .and_then(|v| v.to_str().ok())
            .map(parse_userinfo)
            .unwrap_or_default();

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let node_count = parse_node_count(&body);

        // 先写临时文件再替换，下载失败/中断不会破坏已有 profile
        let out_path = profiles_dir.join(format!("{}.yaml", sub_id));
        let tmp_path = profiles_dir.join(format!("{}.yaml.tmp", sub_id));
        fs::write(&tmp_path, &body).map_err(|e| format!("Failed to write profile: {}", e))?;
        let _ = fs::remove_file(&out_path);
        fs::rename(&tmp_path, &out_path).map_err(|e| format!("Failed to store profile: {}", e))?;

        let last_updated = Local::now().format("%Y-%m-%d %H:%M").to_string();

        if is_active {
            deploy_config_yaml(&body, mixed_port)?;
        }

        Ok(SubUpdateResult {
            sub_id,
            sub_name,
            node_count,
            last_updated,
            is_active,
            userinfo,
        })
    }
}

/// 解析 `subscription-userinfo` 响应头，如 `upload=1; download=2; total=3; expire=4`
pub fn parse_userinfo(header: &str) -> SubUserinfo {
    let mut info = SubUserinfo::default();
    for part in header.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "upload" => info.upload = value.parse().unwrap_or(0),
            "download" => info.download = value.parse().unwrap_or(0),
            "total" => info.total = value.parse().unwrap_or(0),
            "expire" => info.expire = value.parse().unwrap_or(0),
            _ => {}
        }
    }
    info
}

/// 自动更新间隔的展示文本（Off / 1h / 6h / 24h）
pub fn auto_update_label(minutes: u32) -> String {
    match minutes {
        0 => "Off".to_string(),
        m if m % 60 == 0 => format!("{}h", m / 60),
        m => format!("{}m", m),
    }
}

fn is_due(last_updated: &str, minutes: u32, now: NaiveDateTime) -> bool {
    if minutes == 0 {
        return false;
    }
    let Ok(last) = NaiveDateTime::parse_from_str(last_updated, "%Y-%m-%d %H:%M") else {
        return false;
    };
    (now - last).num_minutes() >= i64::from(minutes)
}

/// 将 profile 内容注入 external-controller、mixed-port 及自定义规则后部署到 data/config.yaml
pub fn deploy_config_yaml(profile_content: &str, mixed_port: u16) -> Result<(), String> {
    let config_dest = Path::new("data").join("config.yaml");
    if let Ok(mut yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(profile_content) {
        if let Some(map) = yaml_val.as_mapping_mut() {
            map.insert(
                serde_yaml::Value::String("external-controller".to_string()),
                serde_yaml::Value::String("127.0.0.1:9090".to_string()),
            );
            map.insert(
                serde_yaml::Value::String("mixed-port".to_string()),
                serde_yaml::Value::Number(mixed_port.into()),
            );

            // 确保开机自启与部署订阅配置时默认不启用 TUN 模式
            let tun_key = serde_yaml::Value::String("tun".to_string());
            if let Some(tun_val) = map.get_mut(&tun_key) {
                if let Some(tun_map) = tun_val.as_mapping_mut() {
                    tun_map.insert(
                        serde_yaml::Value::String("enable".to_string()),
                        serde_yaml::Value::Bool(false),
                    );
                }
            } else {
                let mut tun_map = serde_yaml::Mapping::new();
                tun_map.insert(
                    serde_yaml::Value::String("enable".to_string()),
                    serde_yaml::Value::Bool(false),
                );
                map.insert(tun_key, serde_yaml::Value::Mapping(tun_map));
            }
        }
        if let Ok(modified_yaml) = serde_yaml::to_string(&yaml_val) {
            fs::write(&config_dest, modified_yaml)
                .map_err(|e| format!("Failed to write config.yaml: {}", e))?;
        } else {
            fs::write(&config_dest, profile_content)
                .map_err(|e| format!("Failed to write config.yaml: {}", e))?;
        }
    } else {
        fs::write(&config_dest, profile_content)
            .map_err(|e| format!("Failed to write config.yaml: {}", e))?;
    }

    // Inject user-defined proxy domain rules (rules.json) on top of profile rules
    crate::rules::apply_rules_to_config();
    Ok(())
}

fn parse_node_count(content: &str) -> usize {
    if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(content) {
        if let Some(proxies) = yaml.get("proxies").and_then(|v| v.as_sequence()) {
            return proxies.len();
        }
    }

    use base64::Engine;
    let clean = content.replace(['\r', '\n', ' '], "");
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&clean) {
        if let Ok(text) = String::from_utf8(decoded) {
            let count = text.lines().filter(|l| l.contains("://")).count();
            if count > 0 {
                return count;
            }
        }
    }
    if let Ok(decoded) = base64::engine::general_purpose::URL_SAFE.decode(&clean) {
        if let Ok(text) = String::from_utf8(decoded) {
            let count = text.lines().filter(|l| l.contains("://")).count();
            if count > 0 {
                return count;
            }
        }
    }

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn parses_subscription_userinfo() {
        let info = parse_userinfo(
            "upload=1073741824; download=2147483648; total=10737418240; expire=1767225600",
        );
        assert_eq!(info.upload, 1_073_741_824);
        assert_eq!(info.download, 2_147_483_648);
        assert_eq!(info.total, 10_737_418_240);
        assert_eq!(info.expire, 1_767_225_600);
    }

    #[test]
    fn tolerates_missing_and_malformed_userinfo_fields() {
        let info = parse_userinfo("download=42; total=not-a-number; expire=");
        assert_eq!(info.download, 42);
        assert_eq!(info.total, 0);
        assert_eq!(info.expire, 0);
        assert_eq!(parse_userinfo(""), SubUserinfo::default());
    }

    #[test]
    fn old_subscription_format_still_loads() {
        let dir = std::env::temp_dir().join(format!("clash-tui-subs-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("subscriptions.json");
        fs::write(
            &path,
            "[{\"id\":\"sub_1\",\"name\":\"Legacy\",\"url\":\"https://example.net/api\",\
              \"active\":true,\"node_count\":3,\"last_updated\":\"2026-01-02 03:04\"}]",
        )
        .unwrap();

        let mut mgr = SubscriptionManager {
            file_path: path.clone(),
            profiles_dir: dir.join("profiles"),
            subscriptions: Vec::new(),
            auto_update_minutes: DEFAULT_AUTO_UPDATE_MINUTES,
        };
        mgr.load();
        assert_eq!(mgr.subscriptions.len(), 1);
        assert_eq!(mgr.subscriptions[0].name, "Legacy");
        assert_eq!(mgr.subscriptions[0].total, 0);
        assert_eq!(mgr.auto_update_minutes, DEFAULT_AUTO_UPDATE_MINUTES);

        // 保存后升级为新格式，仍可读回
        mgr.save();
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.contains("auto_update_minutes"));
        let mut second = SubscriptionManager {
            file_path: path,
            profiles_dir: dir.join("profiles"),
            subscriptions: Vec::new(),
            auto_update_minutes: 0,
        };
        second.load();
        assert_eq!(second.subscriptions.len(), 1);
        assert_eq!(second.auto_update_minutes, DEFAULT_AUTO_UPDATE_MINUTES);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn auto_update_cycle_walks_through_choices() {
        let dir = std::env::temp_dir().join(format!("clash-tui-cycle-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let mut mgr = SubscriptionManager {
            file_path: dir.join("subscriptions.json"),
            profiles_dir: dir.join("profiles"),
            subscriptions: Vec::new(),
            auto_update_minutes: 0,
        };
        assert_eq!(mgr.cycle_auto_update(), 60);
        assert_eq!(mgr.cycle_auto_update(), 360);
        assert_eq!(mgr.cycle_auto_update(), 1440);
        assert_eq!(mgr.cycle_auto_update(), 0);
        assert_eq!(auto_update_label(360), "6h");
        assert_eq!(auto_update_label(90), "90m");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn due_depends_on_interval_and_last_update() {
        let now = Local.with_ymd_and_hms(2026, 11, 20, 12, 0, 0).unwrap().naive_local();
        assert!(is_due("2026-11-20 10:00", 60, now));
        assert!(!is_due("2026-11-20 11:30", 60, now));
        assert!(!is_due("Never", 60, now));
        assert!(!is_due("2026-11-20 10:00", 0, now));
        assert!(is_due("2026-11-19 11:00", 1440, now));
    }
}