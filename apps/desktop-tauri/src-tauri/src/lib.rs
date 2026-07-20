mod app_state;
mod config;
mod core;
mod discovery;
mod elevation;
mod input;
mod keyboard_hook;
mod mouse;
mod mouse_hook;
mod protocol;
mod routing;
mod sessions;
mod startup;
mod tray;
mod updates;

use app_state::{AppMode, AppStatus, KeyboardTarget, SharedState};
use config::apply_mouse_sensitivity;
use protocol::{BridgeEvent, TargetSide};
use serde::Serialize;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
use tauri::Manager;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NetworkDiagnostics {
    local_ips: Vec<String>,
    tcp_port: u16,
    discovery_port: u16,
    update_port: u16,
    configured_host: Option<String>,
    auto_discovery: bool,
    running_mode: String,
    connected: bool,
    keyboard_target: KeyboardTarget,
    target_host: Option<String>,
    tcp_reachable: Option<bool>,
    update_reachable: Option<bool>,
}

#[tauri::command]
fn app_status(state: tauri::State<SharedState>) -> AppStatus {
    state.snapshot()
}

#[tauri::command]
fn restart_as_admin(app: tauri::AppHandle) -> Result<(), String> {
    elevation::restart_as_admin().map_err(|err| err.to_string())?;
    app.exit(0);
    Ok(())
}

#[tauri::command]
fn start_mode(mode: String, state: tauri::State<SharedState>) -> Result<(), String> {
    let mode = AppMode::from_str(&mode).ok_or_else(|| format!("Unsupported mode: {mode}"))?;
    if mode == AppMode::Idle {
        state.stop_current();
        return Ok(());
    }
    let status = state.snapshot();
    let current_mode = AppMode::from_str(&status.mode).unwrap_or(AppMode::Idle);
    if status.running && current_mode == mode {
        state
            .runtime()
            .log("[应用] 当前模式已在运行，无需重复启动\n");
        return Ok(());
    }
    core::start_mode(mode, state.runtime()).map_err(|err| err.to_string())?;
    Ok(())
}

#[tauri::command]
fn stop_mode(state: tauri::State<SharedState>) {
    state.stop_current();
}

#[tauri::command]
fn clear_logs(state: tauri::State<SharedState>) {
    state.clear_logs();
}

#[tauri::command]
fn set_remote_host(host: Option<String>, state: tauri::State<SharedState>) {
    state.set_remote_host(host);
}

#[tauri::command]
fn set_keyboard_target(target: String, state: tauri::State<SharedState>) -> Result<(), String> {
    let target = match target.as_str() {
        "local" => KeyboardTarget::Local,
        "remote" => KeyboardTarget::Remote,
        other => return Err(format!("Unsupported keyboard target: {other}")),
    };
    let runtime = state.runtime();
    let mode = AppMode::from_str(&state.snapshot().mode).unwrap_or(AppMode::Idle);
    if mode == AppMode::Remote {
        let target_side = match target {
            KeyboardTarget::Local => TargetSide::Local,
            KeyboardTarget::Remote => TargetSide::Remote,
        };
        let label = keyboard_target_label(target);
        if runtime.send_remote_event(BridgeEvent::TargetRequest {
            target: target_side,
        }) {
            runtime.log(format!("[副电脑] 切换请求已发出：键盘到{label}\n"));
            runtime.log("[副电脑] 正在等待主电脑确认键盘目标\n");
        } else {
            runtime.log("[副电脑] 切换请求未发出：当前没有可用的主电脑连接\n");
        }
        return Ok(());
    }

    if target == KeyboardTarget::Local {
        core::force_local_release(&runtime, "[主电脑] 手动安全释放：键盘已回到主电脑\n");
    } else {
        runtime.set_target(target);
        keyboard_hook::set_key_suppression(true);
    }
    runtime.log(format!(
        "[主电脑] 键盘目标已手动切到：{}\n",
        keyboard_target_label(target)
    ));
    Ok(())
}

#[tauri::command]
fn release_control(state: tauri::State<SharedState>) {
    core::force_local_release(&state.runtime(), "[安全] 已立即释放控制：键盘回到主电脑\n");
}

fn keyboard_target_label(target: KeyboardTarget) -> &'static str {
    match target {
        KeyboardTarget::Local => "主电脑",
        KeyboardTarget::Remote => "副电脑",
    }
}

#[tauri::command]
fn set_theme(theme: String, state: tauri::State<SharedState>) -> Result<(), String> {
    if !matches!(theme.as_str(), "light" | "soft") {
        return Err(format!("Unsupported theme: {theme}"));
    }
    state.runtime().update_config(|config| config.theme = theme);
    Ok(())
}

#[tauri::command]
fn set_start_on_login(enabled: bool, state: tauri::State<SharedState>) -> Result<(), String> {
    let runtime = state.runtime();
    let config = runtime.config();
    let mode =
        resolve_startup_mode(&config.startup_mode, &config.last_mode).unwrap_or(AppMode::Host);
    startup::set_start_on_login(enabled, mode).map_err(|err| err.to_string())?;
    runtime.update_config(|config| config.start_on_login = enabled);
    runtime.log(if enabled {
        "[配置] 已开启开机自动启动\n"
    } else {
        "[配置] 已关闭开机自动启动\n"
    });
    Ok(())
}

#[tauri::command]
fn set_restore_last_mode(enabled: bool, state: tauri::State<SharedState>) {
    state.runtime().update_config(|config| {
        config.restore_last_mode = enabled;
        config.startup_mode = if enabled {
            "last".to_string()
        } else {
            "idle".to_string()
        };
    });
    state.runtime().log(if enabled {
        "[配置] 已开启启动时恢复上次模式\n"
    } else {
        "[配置] 已关闭启动时恢复上次模式\n"
    });
}

#[tauri::command]
fn set_startup_mode(mode: String, state: tauri::State<SharedState>) -> Result<(), String> {
    if !matches!(mode.as_str(), "last" | "host" | "remote" | "idle") {
        return Err(format!("Unsupported startup mode: {mode}"));
    }
    state.runtime().update_config(|config| {
        config.startup_mode = mode.clone();
        config.restore_last_mode = mode == "last";
    });
    state.runtime().log(format!(
        "[配置] 启动默认模式已切换为：{}\n",
        startup_mode_label(&mode)
    ));
    Ok(())
}

#[tauri::command]
fn set_minimize_to_tray(enabled: bool, state: tauri::State<SharedState>) {
    state
        .runtime()
        .update_config(|config| config.minimize_to_tray = enabled);
    state.runtime().log(if enabled {
        "[配置] 已开启启动后最小化偏好\n"
    } else {
        "[配置] 已关闭启动后最小化偏好\n"
    });
}

#[tauri::command]
fn set_auto_discovery(enabled: bool, state: tauri::State<SharedState>) {
    state
        .runtime()
        .update_config(|config| config.auto_discovery = enabled);
    state.runtime().log(if enabled {
        "[配置] 已开启自动寻找主电脑\n"
    } else {
        "[配置] 已关闭自动寻找主电脑\n"
    });
}

#[tauri::command]
fn set_game_mode(enabled: bool, state: tauri::State<SharedState>) {
    state.runtime().update_config(|config| {
        config.game_mode = enabled;
    });
    state.runtime().log(if enabled {
        "[配置] 已开启游戏模式：自动鼠标切换暂时关闭\n"
    } else {
        "[配置] 已关闭游戏模式\n"
    });
}

#[tauri::command]
fn set_mouse_sensitivity(preset: String, state: tauri::State<SharedState>) -> Result<(), String> {
    if !matches!(preset.as_str(), "stable" | "balanced" | "sensitive") {
        return Err(format!("Unsupported mouse sensitivity: {preset}"));
    }
    state.runtime().update_config(|config| {
        config.mouse_sensitivity = preset.clone();
        apply_mouse_sensitivity(&mut config.mouse_follow, &preset);
    });
    state
        .runtime()
        .log(format!("[配置] 键盘自动跟随灵敏度已切换为：{preset}\n"));
    Ok(())
}

#[tauri::command]
fn network_diagnostics(state: tauri::State<SharedState>) -> NetworkDiagnostics {
    let status = state.snapshot();
    let target_host = status
        .config
        .remote_host
        .as_ref()
        .map(|host| host.trim().to_string())
        .filter(|host| !host.is_empty());
    let tcp_reachable = target_host
        .as_ref()
        .map(|host| probe_port(host, status.config.tcp_port));
    let update_reachable = target_host
        .as_ref()
        .map(|host| probe_port(host, status.config.update_port));
    NetworkDiagnostics {
        local_ips: discovery::local_ipv4_addresses()
            .into_iter()
            .map(|ip| ip.to_string())
            .collect(),
        tcp_port: status.config.tcp_port,
        discovery_port: status.config.discovery_port,
        update_port: status.config.update_port,
        configured_host: status.config.remote_host,
        auto_discovery: status.config.auto_discovery,
        running_mode: status.mode,
        connected: status.connected,
        keyboard_target: status.target,
        target_host,
        tcp_reachable,
        update_reachable,
    }
}

fn probe_port(host: &str, port: u16) -> bool {
    let Ok(mut addresses) = (host, port).to_socket_addrs() else {
        return false;
    };
    addresses
        .any(|address| TcpStream::connect_timeout(&address, Duration::from_millis(600)).is_ok())
}

fn resolve_startup_mode(startup_mode: &str, last_mode: &str) -> Option<AppMode> {
    match startup_mode {
        "host" => Some(AppMode::Host),
        "remote" => Some(AppMode::Remote),
        "idle" => None,
        _ => AppMode::from_str(last_mode).filter(|mode| *mode != AppMode::Idle),
    }
}

fn startup_mode_label(mode: &str) -> &'static str {
    match mode {
        "host" => "主电脑",
        "remote" => "副电脑",
        "idle" => "不自动启动模式",
        _ => "沿用上次模式",
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let state = SharedState::new(env!("CARGO_PKG_VERSION"));
            let arg_mode = std::env::args().find_map(|arg| match arg.as_str() {
                "--host" => Some(AppMode::Host),
                "--remote" => Some(AppMode::Remote),
                _ => None,
            });
            let config = state.runtime().config();
            let autostart =
                arg_mode.or_else(|| resolve_startup_mode(&config.startup_mode, &config.last_mode));
            app.manage(state.clone());
            tray::install_tray(app, state.clone())?;
            if config.minimize_to_tray {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.minimize();
                }
            }
            if let Some(mode) = autostart {
                let runtime = state.runtime();
                std::thread::spawn(move || {
                    if let Err(err) = crate::core::start_mode(mode, runtime.clone()) {
                        runtime.log(format!("[应用] 自动启动失败：{err:#}\n"));
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            restart_as_admin,
            start_mode,
            stop_mode,
            clear_logs,
            set_remote_host,
            set_keyboard_target,
            release_control,
            set_theme,
            set_start_on_login,
            set_restore_last_mode,
            set_startup_mode,
            set_minimize_to_tray,
            set_auto_discovery,
            set_game_mode,
            set_mouse_sensitivity,
            network_diagnostics
        ])
        .run(tauri::generate_context!())
        .expect("error while running Devices Router");
}
