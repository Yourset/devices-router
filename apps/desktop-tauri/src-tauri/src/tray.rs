use crate::app_state::{AppStatus, SharedState};
use std::thread;
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{App, Manager};

const SHOW_WINDOW_ID: &str = "show-main-window";
const RELEASE_CONTROL_ID: &str = "release-control";
const QUIT_ID: &str = "quit-devices-router";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrayState {
    Controlling,
    ConnectedIdle,
    Disconnected,
}

pub fn tray_state(status: &AppStatus) -> TrayState {
    if status.connected && status.active_device_id.is_some() {
        TrayState::Controlling
    } else if status.connected {
        TrayState::ConnectedIdle
    } else {
        TrayState::Disconnected
    }
}

impl TrayState {
    fn color(self) -> [u8; 4] {
        match self {
            TrayState::Controlling => [34, 197, 94, 255],
            TrayState::ConnectedIdle => [59, 130, 246, 255],
            TrayState::Disconnected => [239, 68, 68, 255],
        }
    }
}

pub fn tray_label(state: TrayState) -> &'static str {
    match state {
        TrayState::Controlling => "绿色：已连接，正在操控副电脑",
        TrayState::ConnectedIdle => "蓝色：已连接，当前未操控副电脑",
        TrayState::Disconnected => "红色：未连接或未启动",
    }
}

fn tray_status_label(status: &AppStatus) -> String {
    match tray_state(status) {
        TrayState::Controlling => {
            let name = status
                .active_device_id
                .as_deref()
                .and_then(|id| status.devices.iter().find(|device| device.device_id == id))
                .map(|device| device.name.as_str())
                .unwrap_or("remote computer");
            format!("green: keyboard is controlling {name}")
        }
        other => tray_label(other).to_string(),
    }
}

pub fn install_tray(app: &App, state: SharedState) -> tauri::Result<()> {
    let initial_status = state.snapshot();
    let initial_state = tray_state(&initial_status);
    let initial_label = tray_status_label(&initial_status);
    let status_item = MenuItem::with_id(
        app,
        "devices-router-status",
        &initial_label,
        false,
        None::<&str>,
    )?;
    let release_item = MenuItem::with_id(
        app,
        RELEASE_CONTROL_ID,
        "Emergency release (Ctrl+Alt+Esc)",
        true,
        None::<&str>,
    )?;
    let show_item = MenuItem::with_id(app, SHOW_WINDOW_ID, "Show window", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &status_item,
            &release_item,
            &separator,
            &show_item,
            &quit_item,
        ],
    )?;
    let menu_state = state.clone();
    let tray = TrayIconBuilder::with_id("devices-router-status")
        .menu(&menu)
        .icon(tray_icon(initial_state))
        .tooltip(&initial_label)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            RELEASE_CONTROL_ID => {
                crate::core::force_local_release(
                    &menu_state.runtime(),
                    "[safety] tray emergency release: keyboard returned to host\n",
                );
            }
            SHOW_WINDOW_ID => show_main_window(app),
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .build(app)?;

    thread::spawn(move || {
        let mut last_state = initial_state;
        let mut last_label = initial_label;
        loop {
            thread::sleep(Duration::from_millis(500));
            let status = state.snapshot();
            let next_state = tray_state(&status);
            let next_label = tray_status_label(&status);
            if next_state == last_state && next_label == last_label {
                continue;
            }
            last_state = next_state;
            last_label = next_label.clone();
            let _ = tray.set_icon(Some(tray_icon(next_state)));
            let _ = tray.set_tooltip(Some(&next_label));
            let _ = status_item.set_text(&next_label);
        }
    });

    Ok(())
}

fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn tray_icon(state: TrayState) -> Image<'static> {
    const SIZE: u32 = 32;
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    let color = state.color();
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as i32 - 15;
            let dy = y as i32 - 15;
            let distance_squared = dx * dx + dy * dy;
            if distance_squared <= 13 * 13 {
                rgba.extend_from_slice(&color);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Image::new_owned(rgba, SIZE, SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::AppStatus;
    use crate::config::AppConfig;
    use crate::routing::KeyboardTarget;

    fn status(connected: bool, target: KeyboardTarget) -> AppStatus {
        AppStatus {
            version: "test".to_string(),
            mode: "host".to_string(),
            running: true,
            connected,
            target: target.as_status_value(),
            local_device_name: "Host-PC".to_string(),
            active_device_id: target.is_remote().then(|| target.as_status_value()),
            devices: Vec::new(),
            host_latency_ms: None,
            link_stats: None,
            activity_transport: "tcp".to_string(),
            elevated: false,
            logs: Vec::new(),
            config: AppConfig::default(),
        }
    }

    #[test]
    fn connected_remote_target_is_controlling() {
        assert_eq!(
            tray_state(&status(true, KeyboardTarget::Device("a".to_string()))),
            TrayState::Controlling
        );
    }

    #[test]
    fn connected_local_target_is_connected_idle() {
        assert_eq!(
            tray_state(&status(true, KeyboardTarget::Local)),
            TrayState::ConnectedIdle
        );
    }

    #[test]
    fn disconnected_is_red_state() {
        assert_eq!(
            tray_state(&status(false, KeyboardTarget::Device("a".to_string()))),
            TrayState::Disconnected
        );
    }

    #[test]
    fn tray_status_label_names_the_active_device() {
        let mut status = status(true, KeyboardTarget::Device("device-a".to_string()));
        status.devices.push(crate::sessions::DeviceStatus {
            device_id: "device-a".to_string(),
            name: "Studio PC".to_string(),
            address: "10.0.0.2:8765".to_string(),
            connected: true,
            legacy: false,
            last_activity_ago_ms: Some(0),
            latency_ms: Some(4),
            link_stats: None,
            activity_transport: "tcp".to_string(),
        });
        assert!(tray_status_label(&status).contains("Studio PC"));
    }

    #[test]
    fn tray_labels_are_plain_status_lines() {
        assert!(tray_label(TrayState::Controlling).contains("绿色"));
        assert!(tray_label(TrayState::ConnectedIdle).contains("蓝色"));
        assert!(tray_label(TrayState::Disconnected).contains("红色"));
    }
}
