use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionItem {
    pub id: String,
    pub name: String,
    pub url: String,
    pub active: bool,
    pub node_count: usize,
    pub last_updated: String,
}

#[derive(Debug, Clone)]
pub struct SubscriptionManager {
    pub file_path: PathBuf,
    pub profiles_dir: PathBuf,
    pub subscriptions: Vec<SubscriptionItem>,
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
        };
        mgr.load();
        mgr
    }

    pub fn load(&mut self) {
        if self.file_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.file_path) {
                if let Ok(subs) = serde_json::from_str::<Vec<SubscriptionItem>>(&content) {
                    self.subscriptions = subs;
                    return;
                }
            }
        }
        self.subscriptions = Vec::new();
        self.save();
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.subscriptions) {
            let _ = fs::write(&self.file_path, json);
        }
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
        };

        self.subscriptions.push(item.clone());
        self.save();
        item
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

        // Read profile content
        let content = fs::read_to_string(&profile_path)
            .map_err(|e| format!("Failed to read profile: {}", e))?;

        // Ensure external-controller and mixed-port are injected
        if let Ok(mut yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
            if let Some(map) = yaml_val.as_mapping_mut() {
                map.insert(
                    serde_yaml::Value::String("external-controller".to_string()),
                    serde_yaml::Value::String("127.0.0.1:9090".to_string()),
                );
                map.insert(
                    serde_yaml::Value::String("mixed-port".to_string()),
                    serde_yaml::Value::Number(mixed_port.into()),
                );
            }
            let config_dest = Path::new("data").join("config.yaml");
            if let Ok(modified_yaml) = serde_yaml::to_string(&yaml_val) {
                let _ = fs::write(&config_dest, modified_yaml);
            } else {
                let _ = fs::copy(&profile_path, &config_dest);
            }
        } else {
            let config_dest = Path::new("data").join("config.yaml");
            let _ = fs::copy(&profile_path, &config_dest);
        }

        for (i, sub) in self.subscriptions.iter_mut().enumerate() {
            sub.active = i == idx;
        }
        self.save();

        Ok(active_sub)
    }

    pub async fn update_subscription(&mut self, idx: usize) -> Result<(String, usize), String> {
        if idx >= self.subscriptions.len() {
            return Err("Subscription index out of range".to_string());
        }

        let sub_id = self.subscriptions[idx].id.clone();
        let sub_name = self.subscriptions[idx].name.clone();
        let url = self.subscriptions[idx].url.clone();

        if url.is_empty() {
            return Err("Subscription URL is empty".to_string());
        }

        // Mock test links
        if url.contains("example.com") || url.contains("demo") {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let node_count = 18;
            self.subscriptions[idx].node_count = node_count;
            self.subscriptions[idx].last_updated = Local::now().format("%Y-%m-%d %H:%M").to_string();
            self.save();
            return Ok((format!("Updated demo sub '{}'", sub_name), node_count));
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

        let body = resp.text().await.map_err(|e| e.to_string())?;
        let node_count = parse_node_count(&body);

        let out_path = self.profiles_dir.join(format!("{}.yaml", sub_id));
        let _ = fs::write(&out_path, &body);

        self.subscriptions[idx].node_count = node_count;
        self.subscriptions[idx].last_updated = Local::now().format("%Y-%m-%d %H:%M").to_string();
        let is_active = self.subscriptions[idx].active;
        self.save();

        if is_active {
            let config_dest = Path::new("data").join("config.yaml");
            if let Ok(mut yaml_val) = serde_yaml::from_str::<serde_yaml::Value>(&body) {
                if let Some(map) = yaml_val.as_mapping_mut() {
                    map.insert(
                        serde_yaml::Value::String("external-controller".to_string()),
                        serde_yaml::Value::String("127.0.0.1:9090".to_string()),
                    );
                }
                if let Ok(modified_yaml) = serde_yaml::to_string(&yaml_val) {
                    let _ = fs::write(&config_dest, modified_yaml);
                } else {
                    let _ = fs::copy(&out_path, &config_dest);
                }
            } else {
                let _ = fs::copy(&out_path, &config_dest);
            }
        }

        Ok((format!("Updated '{}' ({} nodes)", sub_name, node_count), node_count))
    }
}

fn parse_node_count(content: &str) -> usize {
    if let Ok(yaml) = serde_yaml::from_str::<serde_yaml::Value>(content) {
        if let Some(proxies) = yaml.get("proxies").and_then(|v| v.as_sequence()) {
            return proxies.len();
        }
    }

    use base64::Engine;
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(content.trim()) {
        if let Ok(text) = String::from_utf8(decoded) {
            let count = text.lines().filter(|l| l.contains("://")).count();
            if count > 0 {
                return count;
            }
        }
    }

    0
}
