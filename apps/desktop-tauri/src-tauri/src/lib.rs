mod app_state;
mod config;
mod core;
mod discovery;
mod input;
mod keyboard_hook;
mod mouse;
mod protocol;
mod updates;

use app_state::{AppMode, AppStatus, KeyboardTarget, SharedState};
use tauri::Manager;

#[tauri::command]
fn app_status(state: tauri::State<SharedState>) -> AppStatus {
    state.snapshot()
}

#[tauri::command]
fn start_mode(mode: String, state: tauri::State<SharedState>) -> Result<(), String> {
    let mode = match mode.as_str() {
        "host" => AppMode::Host,
        "remote" => AppMode::Remote,
        other => return Err(format!("Unsupported mode: {other}")),
    };
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
    runtime.set_target(target);
    keyboard_hook::set_key_suppression(target == KeyboardTarget::Remote);
    let label = match target {
        KeyboardTarget::Local => "主电脑",
        KeyboardTarget::Remote => "副电脑",
    };
    runtime.log(format!("[主电脑] 键盘目标已手动切到：{label}\n"));
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            app.manage(SharedState::new(env!("CARGO_PKG_VERSION")));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            start_mode,
            stop_mode,
            set_remote_host,
            set_keyboard_target
        ])
        .run(tauri::generate_context!())
        .expect("error while running Devices Router");
}
