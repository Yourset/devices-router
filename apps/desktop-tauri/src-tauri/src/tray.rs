use crate::app_state::{AppStatus, KeyboardTarget, SharedState};
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
    if status.connected && status.target == KeyboardTarget::Remote {
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

pub fn install_tray(app: &App, state: SharedState) -> tauri::Result<()> {
    let status_item = MenuItem::with_id(
        app,
        "devices-router-status",
        tray_label(tray_state(&state.snapshot())),
        false,
        None::<&str>,
    )?;
    let release_item = MenuItem::with_id(
        app,
        RELEASE_CONTROL_ID,
        "立即释放控制（Ctrl+Alt+Esc）",
        true,
        None::<&str>,
    )?;
    let show_item = MenuItem::with_id(app, SHOW_WINDOW_ID, "显示窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, QUIT_ID, "退出", true, None::<&str>)?;
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
    let initial_state = tray_state(&state.snapshot());
    let menu_state = state.clone();
    let tray = TrayIconBuilder::with_id("devices-router-status")
        .menu(&menu)
        .icon(tray_icon(initial_state))
        .tooltip(tray_label(initial_state))
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            RELEASE_CONTROL_ID => {
                crate::core::force_local_release(
                    &menu_state.runtime(),
                    "[安全] 托盘已立即释放控制：键盘和鼠标回到主电脑\n",
                );
            }
            SHOW_WINDOW_ID => show_main_window(app),
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .build(app)?;

    thread::spawn(move || {
        let mut last_state = initial_state;
        loop {
            thread::sleep(Duration::from_millis(500));
            let next_state = tray_state(&state.snapshot());
            if next_state == last_state {
                continue;
            }
            last_state = next_state;
            let label = tray_label(next_state);
            let _ = tray.set_icon(Some(tray_icon(next_state)));
            let _ = tray.set_tooltip(Some(label));
            let _ = status_item.set_text(label);
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

    fn status(connected: bool, target: KeyboardTarget) -> AppStatus {
        AppStatus {
            version: "test".to_string(),
            mode: "host".to_string(),
            running: true,
            connected,
            target,
            elevated: false,
            logs: Vec::new(),
            config: AppConfig::default(),
        }
    }

    #[test]
    fn connected_remote_target_is_controlling() {
        assert_eq!(
            tray_state(&status(true, KeyboardTarget::Remote)),
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
            tray_state(&status(false, KeyboardTarget::Remote)),
            TrayState::Disconnected
        );
    }

    #[test]
    fn tray_labels_are_plain_status_lines() {
        assert!(tray_label(TrayState::Controlling).contains("绿色"));
        assert!(tray_label(TrayState::ConnectedIdle).contains("蓝色"));
        assert!(tray_label(TrayState::Disconnected).contains("红色"));
    }
}
