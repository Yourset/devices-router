use crate::config::AppConfig;
use serde::Serialize;
use std::collections::VecDeque;
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
                    config: AppConfig::default(),
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
        push_log(&mut inner.logs, "[应用] 已停止\n".to_string());
    }

    pub fn snapshot(&self) -> AppStatus {
        let inner = self.inner.state.lock().expect("state lock poisoned");
        AppStatus {
            version: inner.version.clone(),
            mode: match inner.mode {
                AppMode::Idle => "idle",
                AppMode::Host => "host",
                AppMode::Remote => "remote",
            }
            .to_string(),
            running: inner.mode != AppMode::Idle,
            connected: inner.connected,
            target: inner.target,
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
        push_log(&mut inner.logs, "[配置] 主电脑地址已更新\n".to_string());
    }
}

impl AppRuntime {
    pub fn start(&self, mode: AppMode) {
        self.stop.store(false, Ordering::SeqCst);
        let mut inner = self.state.lock().expect("state lock poisoned");
        inner.mode = mode;
        inner.connected = false;
        inner.target = KeyboardTarget::Local;
        let label = match mode {
            AppMode::Idle => "空闲",
            AppMode::Host => "主电脑",
            AppMode::Remote => "副电脑",
        };
        push_log(&mut inner.logs, format!("[应用] 已启动{label}模式\n"));
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
}

fn push_log(logs: &mut VecDeque<String>, line: String) {
    logs.push_back(line);
    while logs.len() > 500 {
        logs.pop_front();
    }
}
