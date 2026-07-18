use crate::config::AppConfig;
use crate::protocol::BridgeEvent;
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppMode {
    Idle,
    Host,
    Remote,
}

impl AppMode {
    pub fn as_str(self) -> &'static str {
        match self {
            AppMode::Idle => "idle",
            AppMode::Host => "host",
            AppMode::Remote => "remote",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AppMode::Idle => "空闲",
            AppMode::Host => "主电脑",
            AppMode::Remote => "副电脑",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "idle" => Some(AppMode::Idle),
            "host" => Some(AppMode::Host),
            "remote" => Some(AppMode::Remote),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyboardTarget {
    Local,
    Remote,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub version: String,
    pub mode: String,
    pub running: bool,
    pub connected: bool,
    pub target: KeyboardTarget,
    pub elevated: bool,
    pub logs: Vec<String>,
    pub config: AppConfig,
}

#[derive(Clone)]
pub struct SharedState {
    inner: Arc<AppRuntime>,
}

pub struct AppRuntime {
    state: Mutex<InnerState>,
    stop: AtomicBool,
}

#[derive(Debug)]
struct InnerState {
    version: String,
    mode: AppMode,
    connected: bool,
    target: KeyboardTarget,
    logs: VecDeque<String>,
    config: AppConfig,
    remote_sender: Option<mpsc::Sender<BridgeEvent>>,
    remote_sender_generation: u64,
    local_release_generation: u64,
    emergency_release_generation: u64,
}

impl SharedState {
    pub fn new(version: &str) -> Self {
        Self {
            inner: Arc::new(AppRuntime {
                state: Mutex::new(InnerState {
                    version: version.to_string(),
                    mode: AppMode::Idle,
                    connected: false,
                    target: KeyboardTarget::Local,
                    logs: VecDeque::new(),
                    config: AppConfig::load(),
                    remote_sender: None,
                    remote_sender_generation: 0,
                    local_release_generation: 0,
                    emergency_release_generation: 0,
                }),
                stop: AtomicBool::new(false),
            }),
        }
    }

    pub fn runtime(&self) -> Arc<AppRuntime> {
        Arc::clone(&self.inner)
    }

    pub fn stop_current(&self) {
        self.inner.request_stop();
        let mut inner = self.inner.state.lock().expect("state lock poisoned");
        inner.mode = AppMode::Idle;
        inner.connected = false;
        inner.target = KeyboardTarget::Local;
        inner.remote_sender = None;
        inner.remote_sender_generation = inner.remote_sender_generation.wrapping_add(1);
        inner.config.last_mode = AppMode::Idle.as_str().to_string();
        inner.config.save();
        push_log(&mut inner.logs, "[应用] 已停止\n".to_string());
    }

    pub fn snapshot(&self) -> AppStatus {
        let inner = self.inner.state.lock().expect("state lock poisoned");
        AppStatus {
            version: inner.version.clone(),
            mode: inner.mode.as_str().to_string(),
            running: inner.mode != AppMode::Idle,
            connected: inner.connected,
            target: inner.target,
            elevated: crate::elevation::is_elevated(),
            logs: inner.logs.iter().cloned().collect(),
            config: inner.config.clone(),
        }
    }

    pub fn set_remote_host(&self, host: Option<String>) {
        let mut inner = self.inner.state.lock().expect("state lock poisoned");
        inner.config.remote_host = host.and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        });
        inner.config.save();
        push_log(&mut inner.logs, "[配置] 主电脑地址已更新\n".to_string());
    }

    pub fn clear_logs(&self) {
        let mut inner = self.inner.state.lock().expect("state lock poisoned");
        inner.logs.clear();
        push_log(&mut inner.logs, "[日志] 已清空\n".to_string());
    }
}

impl AppRuntime {
    pub fn start(&self, mode: AppMode) {
        self.stop.store(false, Ordering::SeqCst);
        let mut inner = self.state.lock().expect("state lock poisoned");
        inner.mode = mode;
        inner.connected = false;
        inner.target = KeyboardTarget::Local;
        inner.remote_sender = None;
        inner.remote_sender_generation = inner.remote_sender_generation.wrapping_add(1);
        inner.config.last_mode = mode.as_str().to_string();
        inner.config.save();
        push_log(
            &mut inner.logs,
            format!("[应用] 已启动{}模式\n", mode.label()),
        );
    }

    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn should_stop(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    pub fn set_connected(&self, connected: bool) {
        let mut inner = self.state.lock().expect("state lock poisoned");
        inner.connected = connected;
    }

    pub fn set_target(&self, target: KeyboardTarget) {
        let mut inner = self.state.lock().expect("state lock poisoned");
        inner.target = target;
    }

    pub fn target(&self) -> KeyboardTarget {
        let inner = self.state.lock().expect("state lock poisoned");
        inner.target
    }

    pub fn log(&self, line: impl Into<String>) {
        let mut inner = self.state.lock().expect("state lock poisoned");
        push_log(&mut inner.logs, line.into());
    }

    pub fn config(&self) -> AppConfig {
        let inner = self.state.lock().expect("state lock poisoned");
        inner.config.clone()
    }

    pub fn update_config(&self, updater: impl FnOnce(&mut AppConfig)) -> AppConfig {
        let mut inner = self.state.lock().expect("state lock poisoned");
        updater(&mut inner.config);
        inner.config.save();
        inner.config.clone()
    }

    pub fn set_remote_sender(&self, sender: mpsc::Sender<BridgeEvent>) -> u64 {
        let mut inner = self.state.lock().expect("state lock poisoned");
        inner.remote_sender_generation = inner.remote_sender_generation.wrapping_add(1);
        inner.remote_sender = Some(sender);
        inner.remote_sender_generation
    }

    pub fn clear_remote_sender(&self, generation: u64) {
        let mut inner = self.state.lock().expect("state lock poisoned");
        if inner.remote_sender_generation == generation {
            inner.remote_sender = None;
        }
    }

    pub fn send_remote_event(&self, event: BridgeEvent) -> bool {
        let sender = {
            let inner = self.state.lock().expect("state lock poisoned");
            inner.remote_sender.clone()
        };
        sender.is_some_and(|sender| sender.send(event).is_ok())
    }

    pub fn mark_local_release(&self) {
        let mut inner = self.state.lock().expect("state lock poisoned");
        inner.local_release_generation = inner.local_release_generation.wrapping_add(1);
    }

    pub fn local_release_generation(&self) -> u64 {
        let inner = self.state.lock().expect("state lock poisoned");
        inner.local_release_generation
    }

    pub fn mark_emergency_release(&self) {
        let mut inner = self.state.lock().expect("state lock poisoned");
        inner.emergency_release_generation = inner.emergency_release_generation.wrapping_add(1);
    }

    pub fn emergency_release_generation(&self) -> u64 {
        let inner = self.state.lock().expect("state lock poisoned");
        inner.emergency_release_generation
    }
}

fn push_log(logs: &mut VecDeque<String>, line: String) {
    logs.push_back(line);
    while logs.len() > 500 {
        logs.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_mode_round_trips() {
        assert_eq!(AppMode::from_str("host"), Some(AppMode::Host));
        assert_eq!(AppMode::Remote.as_str(), "remote");
    }

    #[test]
    fn local_release_generation_increments() {
        let state = SharedState::new("test");
        let runtime = state.runtime();
        let before = runtime.local_release_generation();

        runtime.mark_local_release();

        assert_eq!(runtime.local_release_generation(), before.wrapping_add(1));
    }

    #[test]
    fn emergency_release_generation_increments_independently() {
        let state = SharedState::new("test");
        let runtime = state.runtime();
        let local_before = runtime.local_release_generation();
        let emergency_before = runtime.emergency_release_generation();

        runtime.mark_emergency_release();

        assert_eq!(runtime.local_release_generation(), local_before);
        assert_eq!(
            runtime.emergency_release_generation(),
            emergency_before.wrapping_add(1)
        );
    }
}
