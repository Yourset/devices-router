use serde::{Deserialize, Serialize};
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
            host_poll_interval_ms: 20,
            remote_report_interval_ms: 40,
            host_priority_cooldown_ms: 60,
            switch_debounce_ms: 80,
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
            mouse_follow: MouseFollowConfig::default(),
            mouse_sensitivity: "balanced".to_string(),
            startup_mode: "last".to_string(),
            last_mode: "idle".to_string(),
            restore_last_mode: true,
            start_on_login: false,
            minimize_to_tray: false,
            auto_discovery: true,
            game_mode: false,
            experimental_mouse_input: true,
            mouse_input_initialized: true,
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
        if !self.mouse_input_initialized {
            self.experimental_mouse_input = true;
            self.mouse_input_initialized = true;
        }
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
            mouse.host_poll_interval_ms = 30;
            mouse.remote_report_interval_ms = 80;
            mouse.host_priority_cooldown_ms = 140;
            mouse.switch_debounce_ms = 160;
        }
        "sensitive" => {
            mouse.host_poll_interval_ms = 15;
            mouse.remote_report_interval_ms = 25;
            mouse.host_priority_cooldown_ms = 40;
            mouse.switch_debounce_ms = 50;
        }
        _ => {
            mouse.host_poll_interval_ms = 20;
            mouse.remote_report_interval_ms = 40;
            mouse.host_priority_cooldown_ms = 60;
            mouse.switch_debounce_ms = 80;
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
        assert!(AppConfig::default().restore_last_mode);
        assert!(AppConfig::default().auto_discovery);
        assert!(AppConfig::default().experimental_mouse_input);
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

        assert_eq!(config.mouse_follow.host_poll_interval_ms, 20);
        assert_eq!(config.mouse_follow.remote_report_interval_ms, 40);
        assert_eq!(config.mouse_follow.host_priority_cooldown_ms, 60);
        assert_eq!(config.mouse_follow.switch_debounce_ms, 80);
    }

    #[test]
    fn mouse_sensitivity_presets_are_applied() {
        let mut mouse = MouseFollowConfig::default();

        apply_mouse_sensitivity(&mut mouse, "stable");
        assert_eq!(mouse.remote_report_interval_ms, 80);
        assert_eq!(mouse.switch_debounce_ms, 160);

        apply_mouse_sensitivity(&mut mouse, "sensitive");
        assert_eq!(mouse.remote_report_interval_ms, 25);
        assert_eq!(mouse.switch_debounce_ms, 50);
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
    fn normalize_enables_mouse_input_once_for_pre_0124_configs() {
        let payload = r#"{
            "experimentalMouseInput": false,
            "gameMode": false,
            "lastMode": "remote"
        }"#;
        let mut config: AppConfig = serde_json::from_str(payload).unwrap();

        assert!(!config.mouse_input_initialized);
        config.normalize();

        assert!(config.experimental_mouse_input);
        assert!(config.mouse_input_initialized);

        config.experimental_mouse_input = false;
        config.normalize();
        assert!(!config.experimental_mouse_input);
    }
}
