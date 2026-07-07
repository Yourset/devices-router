mod app_state;
mod config;
mod core;
mod discovery;
mod input;
mod keyboard_hook;
mod mouse;
mod protocol;

use app_state::{AppMode, AppStatus, SharedState};
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
            set_remote_host
        ])
        .run(tauri::generate_context!())
        .expect("error while running Devices Router");
}
