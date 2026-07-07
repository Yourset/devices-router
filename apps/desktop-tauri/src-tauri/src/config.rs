use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MouseFollowConfig {
    pub enabled: bool,
    pub host_mouse_returns_local: bool,
    pub remote_mouse_switches_remote: bool,
    pub host_poll_interval_ms: u64,
    pub remote_report_interval_ms: u64,
    pub host_priority_cooldown_ms: u64,
    pub switch_debounce_ms: u64,
}

impl Default for MouseFollowConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            host_mouse_returns_local: true,
            remote_mouse_switches_remote: true,
            host_poll_interval_ms: 50,
            remote_report_interval_ms: 500,
            host_priority_cooldown_ms: 800,
            switch_debounce_ms: 300,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub tcp_port: u16,
    pub discovery_port: u16,
    pub update_port: u16,
    pub remote_host: Option<String>,
    pub mouse_follow: MouseFollowConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            tcp_port: 8765,
            discovery_port: 8766,
            update_port: 8767,
            remote_host: None,
            mouse_follow: MouseFollowConfig::default(),
        }
    }
}
