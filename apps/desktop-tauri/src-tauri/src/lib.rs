mod app_state;
mod config;
mod core;
mod discovery;
mod input;
mod keyboard_hook;
mod mouse;
mod protocol;
mod startup;
mod updates;

use app_state::{AppMode, AppStatus, KeyboardTarget, SharedState};
use protocol::{BridgeEvent, TargetSide};
use tauri::Manager;

#[tauri::command]
fn app_status(state: tauri::State<SharedState>) -> AppStatus {
    state.snapshot()
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
            runtime.log(format!("[副电脑] 已请求主电脑切换键盘目标：{label}\n"));
        } else {
            runtime.log("[副电脑] 尚未连接主电脑，无法请求切换键盘目标\n");
        }
        return Ok(());
    }

    runtime.set_target(target);
    keyboard_hook::set_key_suppression(target == KeyboardTarget::Remote);
    runtime.log(format!(
        "[主电脑] 键盘目标已手动切到：{}\n",
        keyboard_target_label(target)
    ));
    Ok(())
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
    let mode = AppMode::from_str(&runtime.config().last_mode).unwrap_or(AppMode::Host);
    startup::set_start_on_login(enabled, mode).map_err(|err| err.to_string())?;
    runtime.update_config(|config| config.start_on_login = enabled);
    runtime.log(if enabled {
        "[配置] 已开启开机自动启动\n"
    } else {
        "[配置] 已关闭开机自动启动\n"
    });
    Ok(())
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
            let remembered_mode = AppMode::from_str(&state.runtime().config().last_mode)
                .filter(|mode| *mode != AppMode::Idle);
            let autostart = arg_mode.or(remembered_mode);
            app.manage(state.clone());
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
            start_mode,
            stop_mode,
            set_remote_host,
            set_keyboard_target,
            set_theme,
            set_start_on_login
        ])
        .run(tauri::generate_context!())
        .expect("error while running Devices Router");
}
