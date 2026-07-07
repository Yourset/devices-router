use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
            host_poll_interval_ms: 20,
            remote_report_interval_ms: 40,
            host_priority_cooldown_ms: 60,
            switch_debounce_ms: 80,
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
    pub last_mode: String,
    pub start_on_login: bool,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            tcp_port: 8765,
            discovery_port: 8766,
            update_port: 8767,
            remote_host: None,
            mouse_follow: MouseFollowConfig::default(),
            last_mode: "idle".to_string(),
            start_on_login: false,
            theme: "light".to_string(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        let Ok(payload) = fs::read(&path) else {
            return Self::default();
        };
        let mut config: Self = serde_json::from_slice(strip_utf8_bom(&payload)).unwrap_or_default();
        config.normalize();
        config
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(payload) = serde_json::to_vec_pretty(self) {
            let _ = fs::write(path, payload);
        }
    }

    fn normalize(&mut self) {
        if self.mouse_follow.host_poll_interval_ms == 50
            || self.mouse_follow.host_poll_interval_ms == 30
        {
            self.mouse_follow.host_poll_interval_ms = 20;
        }
        if self.mouse_follow.remote_report_interval_ms == 500
            || self.mouse_follow.remote_report_interval_ms == 80
        {
            self.mouse_follow.remote_report_interval_ms = 40;
        }
        if self.mouse_follow.host_priority_cooldown_ms == 800
            || self.mouse_follow.host_priority_cooldown_ms == 120
        {
            self.mouse_follow.host_priority_cooldown_ms = 60;
        }
        if self.mouse_follow.switch_debounce_ms == 300
            || self.mouse_follow.switch_debounce_ms == 150
        {
            self.mouse_follow.switch_debounce_ms = 80;
        }
    }
}

pub fn app_data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("Devices Router")
}

pub fn config_path() -> PathBuf {
    app_data_dir().join("config.json")
}

fn strip_utf8_bom(payload: &[u8]) -> &[u8] {
    payload.strip_prefix(b"\xef\xbb\xbf").unwrap_or(payload)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_remembers_idle_mode() {
        assert_eq!(AppConfig::default().last_mode, "idle");
        assert_eq!(AppConfig::default().theme, "light");
    }

    #[test]
    fn normalize_upgrades_old_mouse_follow_defaults() {
        let mut config = AppConfig::default();
        config.mouse_follow.host_poll_interval_ms = 50;
        config.mouse_follow.remote_report_interval_ms = 500;
        config.mouse_follow.host_priority_cooldown_ms = 800;
        config.mouse_follow.switch_debounce_ms = 300;

        config.normalize();

        assert_eq!(config.mouse_follow.host_poll_interval_ms, 20);
        assert_eq!(config.mouse_follow.remote_report_interval_ms, 40);
        assert_eq!(config.mouse_follow.host_priority_cooldown_ms, 60);
        assert_eq!(config.mouse_follow.switch_debounce_ms, 80);
    }
}
