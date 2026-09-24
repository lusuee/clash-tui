use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct VersionInfo {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub premium: bool,
    #[serde(default)]
    pub meta: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TunConfig {
    #[serde(default)]
    pub enable: bool,
    #[serde(default)]
    pub stack: Option<String>,
    #[serde(default)]
    pub device: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Configs {
    pub port: Option<u16>,
    #[serde(rename = "socks-port")]
    pub socks_port: Option<u16>,
    #[serde(rename = "redir-port")]
    pub redir_port: Option<u16>,
    #[serde(rename = "mixed-port")]
    pub mixed_port: Option<u16>,
    #[serde(default)]
    pub mode: String,
    #[serde(rename = "log-level", default)]
    pub log_level: String,
    #[serde(default)]
    pub tun: Option<TunConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DelayHistory {
    pub time: Option<String>,
    pub delay: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProxyItem {
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub udp: Option<bool>,
    pub now: Option<String>,
    pub all: Option<Vec<String>>,
    pub history: Option<Vec<DelayHistory>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ProxiesResponse {
    pub proxies: HashMap<String, ProxyItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DelayResponse {
    pub delay: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[allow(dead_code)]
pub struct TrafficInfo {
    pub up: u64,
    pub down: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ConnectionMetadata {
    #[serde(rename = "network")]
    pub network: Option<String>,
    #[serde(rename = "type")]
    pub conn_type: Option<String>,
    #[serde(rename = "sourceIP")]
    pub source_ip: Option<String>,
    #[serde(rename = "destinationIP")]
    pub destination_ip: Option<String>,
    #[serde(rename = "sourcePort")]
    pub source_port: Option<String>,
    #[serde(rename = "destinationPort")]
    pub destination_port: Option<String>,
    pub host: Option<String>,
    #[serde(rename = "processPath")]
    pub process_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ConnectionItem {
    pub id: String,
    pub metadata: ConnectionMetadata,
    pub upload: u64,
    pub download: u64,
    pub start: String,
    pub chains: Vec<String>,
    pub rule: String,
    #[serde(rename = "rulePayload")]
    pub rule_payload: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ConnectionsResponse {
    #[serde(rename = "downloadTotal")]
    pub download_total: u64,
    #[serde(rename = "uploadTotal")]
    pub upload_total: u64,
    pub connections: Vec<ConnectionItem>,
}

#[derive(Clone)]
pub struct ClashClient {
    base_url: String,
    secret: Option<String>,
    client: reqwest::Client,
}

impl ClashClient {
    pub fn new(base_url: &str, secret: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            secret,
            client,
        }
    }

    fn request_builder(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(secret) = &self.secret {
            if !secret.is_empty() {
                return builder.header("Authorization", format!("Bearer {}", secret));
            }
        }
        builder
    }

    pub async fn get_version(&self) -> Result<VersionInfo, String> {
        let url = format!("{}/version", self.base_url);
        let resp = self
            .request_builder(self.client.get(&url))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        resp.json::<VersionInfo>().await.map_err(|e| e.to_string())
    }

    pub async fn get_configs(&self) -> Result<Configs, String> {
        let url = format!("{}/configs", self.base_url);
        let resp = self
            .request_builder(self.client.get(&url))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        resp.json::<Configs>().await.map_err(|e| e.to_string())
    }

    pub async fn set_mode(&self, mode: &str) -> Result<(), String> {
        let url = format!("{}/configs", self.base_url);
        let body = serde_json::json!({ "mode": mode });
        let resp = self
            .request_builder(self.client.patch(&url).json(&body))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("Set mode failed: HTTP {}", resp.status()))
        }
    }

    pub async fn get_proxies(&self) -> Result<ProxiesResponse, String> {
        let url = format!("{}/proxies", self.base_url);
        let resp = self
            .request_builder(self.client.get(&url))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        resp.json::<ProxiesResponse>().await.map_err(|e| e.to_string())
    }

    pub async fn select_proxy(&self, group: &str, node: &str) -> Result<(), String> {
        let encoded_group = urlencoding_simple(group);
        let url = format!("{}/proxies/{}", self.base_url, encoded_group);
        let body = serde_json::json!({ "name": node });
        let resp = self
            .request_builder(self.client.put(&url).json(&body))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("Select proxy failed: HTTP {}", resp.status()))
        }
    }

    pub async fn test_delay(&self, node: &str, test_url: &str, timeout_ms: u64) -> Result<u64, String> {
        let encoded_node = urlencoding_simple(node);
        let encoded_url = urlencoding_simple(test_url);
        let url = format!(
            "{}/proxies/{}/delay?timeout={}&url={}",
            self.base_url, encoded_node, timeout_ms, encoded_url
        );
        let resp = self
            .request_builder(self.client.get(&url).timeout(Duration::from_millis(timeout_ms + 1000)))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Timeout or error: HTTP {}", resp.status()));
        }

        let delay_resp = resp.json::<DelayResponse>().await.map_err(|e| e.to_string())?;
        Ok(delay_resp.delay)
    }

    pub async fn get_connections(&self) -> Result<ConnectionsResponse, String> {
        let url = format!("{}/connections", self.base_url);
        let resp = self
            .request_builder(self.client.get(&url))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        resp.json::<ConnectionsResponse>().await.map_err(|e| e.to_string())
    }

    pub async fn close_connection(&self, id: &str) -> Result<(), String> {
        let url = format!("{}/connections/{}", self.base_url, id);
        let resp = self
            .request_builder(self.client.delete(&url))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("Close connection failed: HTTP {}", resp.status()))
        }
    }

    pub async fn close_all_connections(&self) -> Result<(), String> {
        let url = format!("{}/connections", self.base_url);
        let resp = self
            .request_builder(self.client.delete(&url))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("Close all failed: HTTP {}", resp.status()))
        }
    }

    pub async fn reload_config(&self, path: &str) -> Result<(), String> {
        let url = format!("{}/configs?force=true", self.base_url);
        let body = serde_json::json!({ "path": path });
        let resp = self
            .request_builder(self.client.put(&url).json(&body))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("Reload config failed: HTTP {}", resp.status()))
        }
    }

    pub async fn set_mixed_port(&self, port: u16) -> Result<(), String> {
        let url = format!("{}/configs", self.base_url);
        let body = serde_json::json!({ "mixed-port": port });
        let resp = self
            .request_builder(self.client.patch(&url).json(&body))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("Set mixed-port failed: HTTP {}", resp.status()))
        }
    }

    pub async fn set_tun(&self, enable: bool) -> Result<(), String> {
        let url = format!("{}/configs", self.base_url);
        let body = serde_json::json!({ "tun": { "enable": enable } });
        let resp = self
            .request_builder(self.client.patch(&url).json(&body))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("Set TUN failed: HTTP {}", resp.status()))
        }
    }
}

fn urlencoding_simple(s: &str) -> String {
    let mut result = String::new();
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(b as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", b));
            }
        }
    }
    result
}
