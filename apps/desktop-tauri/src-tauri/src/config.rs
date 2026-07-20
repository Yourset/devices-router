use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
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
            host_poll_interval_ms: 10,
            remote_report_interval_ms: 15,
            host_priority_cooldown_ms: 25,
            switch_debounce_ms: 30,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub tcp_port: u16,
    pub discovery_port: u16,
    pub update_port: u16,
    pub remote_host: Option<String>,
    pub device_id: String,
    pub device_aliases: BTreeMap<String, String>,
    pub mouse_follow: MouseFollowConfig,
    pub mouse_sensitivity: String,
    pub startup_mode: String,
    pub last_mode: String,
    pub restore_last_mode: bool,
    pub start_on_login: bool,
    pub minimize_to_tray: bool,
    pub auto_discovery: bool,
    pub game_mode: bool,
    pub experimental_mouse_input: bool,
    #[serde(default)]
    pub mouse_input_initialized: bool,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            tcp_port: 8765,
            discovery_port: 8766,
            update_port: 8767,
            remote_host: None,
            device_id: generate_device_id(),
            device_aliases: BTreeMap::new(),
            mouse_follow: MouseFollowConfig::default(),
            mouse_sensitivity: "balanced".to_string(),
            startup_mode: "last".to_string(),
            last_mode: "idle".to_string(),
            restore_last_mode: true,
            start_on_login: false,
            minimize_to_tray: false,
            auto_discovery: true,
            game_mode: false,
            experimental_mouse_input: false,
            mouse_input_initialized: true,
            theme: "light".to_string(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        let Ok(payload) = fs::read(&path) else {
            let config = Self::default();
            if !cfg!(test) {
                config.save();
            }
            return config;
        };
        let had_device_id = serde_json::from_slice::<serde_json::Value>(strip_utf8_bom(&payload))
            .ok()
            .and_then(|value| value.get("deviceId").cloned())
            .is_some();
        let mut config: Self = serde_json::from_slice(strip_utf8_bom(&payload)).unwrap_or_default();
        config.normalize();
        if !had_device_id && !cfg!(test) {
            config.save();
        }
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
        if self.device_id.trim().is_empty() {
            self.device_id = generate_device_id();
        }
        self.experimental_mouse_input = false;
        self.mouse_input_initialized = true;
        self.mouse_follow.enabled = true;
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
        if !matches!(
            self.mouse_sensitivity.as_str(),
            "stable" | "balanced" | "sensitive"
        ) {
            self.mouse_sensitivity = "balanced".to_string();
        }
        if !matches!(
            self.startup_mode.as_str(),
            "last" | "host" | "remote" | "idle"
        ) {
            self.startup_mode = if self.restore_last_mode {
                "last".to_string()
            } else {
                "idle".to_string()
            };
        }
        self.restore_last_mode = self.startup_mode == "last";
        apply_mouse_sensitivity(&mut self.mouse_follow, &self.mouse_sensitivity);
    }
}

pub fn apply_mouse_sensitivity(mouse: &mut MouseFollowConfig, preset: &str) {
    match preset {
        "stable" => {
            mouse.host_poll_interval_ms = 20;
            mouse.remote_report_interval_ms = 60;
            mouse.host_priority_cooldown_ms = 100;
            mouse.switch_debounce_ms = 120;
        }
        "sensitive" => {
            mouse.host_poll_interval_ms = 5;
            mouse.remote_report_interval_ms = 10;
            mouse.host_priority_cooldown_ms = 15;
            mouse.switch_debounce_ms = 20;
        }
        _ => {
            mouse.host_poll_interval_ms = 10;
            mouse.remote_report_interval_ms = 15;
            mouse.host_priority_cooldown_ms = 25;
            mouse.switch_debounce_ms = 30;
        }
    }
}

pub fn computer_name() -> String {
    std::env::var("COMPUTERNAME")
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Windows-PC".to_string())
}

fn generate_device_id() -> String {
    let seed = format!(
        "devices-router-v1|{}|{}|{}",
        computer_name(),
        std::env::var("USERNAME").unwrap_or_default(),
        std::env::var("USERPROFILE").unwrap_or_default()
    );
    let digest = Sha256::digest(seed.as_bytes());
    digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
    use std::collections::BTreeMap;

    #[test]
    fn default_config_remembers_idle_mode() {
        assert_eq!(AppConfig::default().last_mode, "idle");
        assert_eq!(AppConfig::default().theme, "light");
        assert!(AppConfig::default().restore_last_mode);
        assert!(AppConfig::default().auto_discovery);
        assert!(!AppConfig::default().experimental_mouse_input);
        assert!(AppConfig::default().mouse_follow.enabled);
        assert_eq!(AppConfig::default().startup_mode, "last");
        assert_eq!(AppConfig::default().mouse_sensitivity, "balanced");
    }

    #[test]
    fn normalize_upgrades_old_mouse_follow_defaults() {
        let mut config = AppConfig::default();
        config.mouse_follow.host_poll_interval_ms = 50;
        config.mouse_follow.remote_report_interval_ms = 500;
        config.mouse_follow.host_priority_cooldown_ms = 800;
        config.mouse_follow.switch_debounce_ms = 300;

        config.normalize();

        assert_eq!(config.mouse_follow.host_poll_interval_ms, 10);
        assert_eq!(config.mouse_follow.remote_report_interval_ms, 15);
        assert_eq!(config.mouse_follow.host_priority_cooldown_ms, 25);
        assert_eq!(config.mouse_follow.switch_debounce_ms, 30);
    }

    #[test]
    fn mouse_sensitivity_presets_are_applied() {
        let mut mouse = MouseFollowConfig::default();

        apply_mouse_sensitivity(&mut mouse, "stable");
        assert_eq!(mouse.remote_report_interval_ms, 60);
        assert_eq!(mouse.switch_debounce_ms, 120);

        apply_mouse_sensitivity(&mut mouse, "sensitive");
        assert_eq!(mouse.remote_report_interval_ms, 10);
        assert_eq!(mouse.switch_debounce_ms, 20);
    }

    #[test]
    fn normalize_syncs_legacy_restore_last_mode() {
        let mut config = AppConfig {
            startup_mode: "bad-value".to_string(),
            restore_last_mode: false,
            ..AppConfig::default()
        };

        config.normalize();

        assert_eq!(config.startup_mode, "idle");
        assert!(!config.restore_last_mode);
    }

    #[test]
    fn normalize_keeps_activity_follow_but_disables_cross_screen_mouse() {
        let payload = r#"{
            "experimentalMouseInput": false,
            "gameMode": false,
            "lastMode": "remote"
        }"#;
        let mut config: AppConfig = serde_json::from_str(payload).unwrap();

        assert!(!config.mouse_input_initialized);
        config.normalize();

        assert!(!config.experimental_mouse_input);
        assert!(config.mouse_follow.enabled);
        assert!(config.mouse_input_initialized);
    }

    #[test]
    fn old_config_gains_stable_device_identity_without_losing_fields() {
        let payload = r#"{
            "remoteHost": "192.168.1.20",
            "startupMode": "remote",
            "mouseSensitivity": "sensitive"
        }"#;
        let mut config: AppConfig = serde_json::from_str(payload).unwrap();

        config.normalize();
        let first_id = config.device_id.clone();
        config.normalize();

        assert!(!first_id.is_empty());
        assert_eq!(config.device_id, first_id);
        assert_eq!(config.remote_host.as_deref(), Some("192.168.1.20"));
        assert_eq!(config.startup_mode, "remote");
        assert_eq!(config.mouse_sensitivity, "sensitive");
    }

    #[test]
    fn device_aliases_default_to_empty_and_round_trip() {
        let mut config = AppConfig::default();
        assert_eq!(config.device_aliases, BTreeMap::new());

        config
            .device_aliases
            .insert("device-a".to_string(), "????".to_string());
        let payload = serde_json::to_string(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&payload).unwrap();

        assert_eq!(restored.device_aliases["device-a"], "????");
    }
}
