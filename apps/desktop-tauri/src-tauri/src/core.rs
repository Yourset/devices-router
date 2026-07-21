#![allow(dead_code)]

use crate::app_state::{AppMode, AppRuntime, KeyboardTarget};
use crate::config::AppConfig;
use crate::discovery::{broadcast_host, discover_host, scan_local_network};
use crate::input::{release_local_modifiers, send_key_event, send_mouse_input_event};
use crate::keyboard_hook::{
    run_keyboard_hook, set_key_suppression, take_panic_request, RawKeyEvent,
};
use crate::latency::LatencyProbeTracker;
use crate::mouse::{
    at_left_edge, at_right_edge, cursor_position, screen_center, set_cursor_position,
    virtual_screen_rect, MousePosition, ScreenRect,
};
use crate::mouse_hook::{run_mouse_hook, set_mouse_input_suppression};
use crate::protocol::{
    decode_event, encode_event, BridgeEvent, ClientRole, KeyAction, MouseButton, MouseButtonAction,
    MouseInputEvent, MouseSource, TargetSide,
};
use crate::updates::{check_remote_update, host_from_socket_addr, start_update_server};
use anyhow::{bail, Context, Result};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const TCP_PORT: u16 = 8765;
const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(300);
const LATENCY_PROBE_INTERVAL: Duration = Duration::from_millis(500);
const LOCAL_RELEASE_COOLDOWN: Duration = Duration::from_millis(120);
const EMERGENCY_RELEASE_COOLDOWN: Duration = Duration::from_secs(1);
const HOST_SESSION_POLL_INTERVAL: Duration = Duration::from_millis(5);
const CROSS_SCREEN_MOUSE_AVAILABLE: bool = false;
const MOUSE_ACTIVITY_FOLLOW_AVAILABLE: bool = true;

struct HostSessionSafetyGuard {
    runtime: Arc<AppRuntime>,
}

impl HostSessionSafetyGuard {
    fn new(runtime: Arc<AppRuntime>) -> Self {
        Self { runtime }
    }
}

impl Drop for HostSessionSafetyGuard {
    fn drop(&mut self) {
        force_local_release(
            &self.runtime,
            "[host] connection ended: local keyboard released\n",
        );
    }
}

pub fn force_local_release(runtime: &Arc<AppRuntime>, log_line: &str) -> bool {
    let changed = crate::multi_host::switch_target(runtime, KeyboardTarget::Local, log_line);
    runtime.mark_local_release();
    runtime.mark_emergency_release();
    set_key_suppression(false);
    set_mouse_input_suppression(false);
    let _ = runtime.send_remote_event(BridgeEvent::TargetRequest {
        target: TargetSide::Local,
    });
    changed
}

pub fn start_mode(mode: AppMode, runtime: Arc<AppRuntime>) -> Result<()> {
    runtime.request_stop();
    set_key_suppression(false);
    set_mouse_input_suppression(false);
    thread::sleep(Duration::from_millis(50));
    runtime.start(mode);
    match mode {
        AppMode::Host => start_host(runtime),
        AppMode::Remote => start_remote(runtime),
        AppMode::Idle => Ok(()),
    }
}

fn start_host(runtime: Arc<AppRuntime>) -> Result<()> {
    thread::Builder::new()
        .name("devices-router-host".to_string())
        .spawn(move || {
            if let Err(err) = crate::multi_host::run(runtime.clone()) {
                runtime.log(format!("[主电脑] 已停止：{err:#}\n"));
            }
        })
        .context("spawn host thread")?;
    Ok(())
}

fn run_host(runtime: Arc<AppRuntime>) -> Result<()> {
    let (key_tx, key_rx) = mpsc::channel::<RawKeyEvent>();
    let (mouse_tx, mouse_rx) = mpsc::channel::<MouseInputEvent>();
    let hook_runtime = Arc::clone(&runtime);
    thread::Builder::new()
        .name("devices-router-keyboard-hook".to_string())
        .spawn(move || {
            if let Err(err) = run_keyboard_hook(key_tx) {
                hook_runtime.log(format!("[主电脑] 键盘监听失败：{err:#}\n"));
            }
        })
        .context("spawn keyboard hook thread")?;
    if CROSS_SCREEN_MOUSE_AVAILABLE {
        let mouse_hook_runtime = Arc::clone(&runtime);
        thread::Builder::new()
            .name("devices-router-mouse-hook".to_string())
            .spawn(move || {
                if let Err(err) = run_mouse_hook(mouse_tx) {
                    mouse_hook_runtime.log(format!("[host] mouse hook failed: {err:#}\n"));
                }
            })
            .context("spawn mouse hook thread")?;
    }
    let discovery_runtime = Arc::clone(&runtime);
    thread::Builder::new()
        .name("devices-router-discovery-broadcast".to_string())
        .spawn(move || {
            let stop_runtime = Arc::clone(&discovery_runtime);
            if let Err(err) = broadcast_host(move || stop_runtime.should_stop(), TCP_PORT) {
                discovery_runtime.log(format!("[主电脑] 自动发现广播失败：{err:#}\n"));
            }
        })
        .context("spawn discovery broadcaster")?;
    let update_runtime = Arc::clone(&runtime);
    let update_port = runtime.config().update_port;
    thread::Builder::new()
        .name("devices-router-update-server".to_string())
        .spawn(move || {
            if let Err(err) = start_update_server(update_runtime.clone(), update_port) {
                update_runtime.log(format!("[更新] 更新服务已停止：{err:#}\n"));
            }
        })
        .context("spawn update server")?;
    if MOUSE_ACTIVITY_FOLLOW_AVAILABLE {
        let mouse_runtime = Arc::clone(&runtime);
        thread::Builder::new()
            .name("devices-router-host-mouse".to_string())
            .spawn(move || run_host_mouse_loop(mouse_runtime))
            .context("spawn host mouse loop")?;
    }

    let listener = TcpListener::bind(("0.0.0.0", TCP_PORT)).context("bind host TCP listener")?;
    listener
        .set_nonblocking(true)
        .context("set host listener nonblocking")?;
    runtime.log(format!("[主电脑] 正在监听 0.0.0.0:{TCP_PORT}\n"));
    while !runtime.should_stop() {
        match listener.accept() {
            Ok((stream, address)) => match accept_remote_client(&runtime, stream, address) {
                Ok(Some(stream)) => {
                    runtime.set_connected(true);
                    handle_host_client(&runtime, stream, &key_rx, &mouse_rx);
                    runtime.set_connected(false);
                }
                Ok(None) => {}
                Err(err) => runtime.log(format!(
                    "[主电脑] 已忽略无法握手的连接：{address}，{err:#}\n"
                )),
            },
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(err) => return Err(err).context("accept host client"),
        }
    }
    Ok(())
}

fn accept_remote_client(
    runtime: &Arc<AppRuntime>,
    mut stream: TcpStream,
    address: SocketAddr,
) -> Result<Option<TcpStream>> {
    stream
        .set_read_timeout(Some(Duration::from_millis(900)))
        .context("设置握手超时失败")?;
    let mut reader = BufReader::new(stream.try_clone().context("复制握手连接失败")?);
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => {
            runtime.log(format!("[主电脑] 已忽略空连接：{address}\n"));
            Ok(None)
        }
        Ok(_) => match decode_event(line.as_bytes()) {
            Ok(BridgeEvent::ClientHello { .. }) => {
                stream.set_read_timeout(None).context("恢复连接超时失败")?;
                stream.write_all(&encode_event(&BridgeEvent::Ping {
                    message: "ok".to_string(),
                    probe_id: None,
                    reply_to: None,
                })?)?;
                runtime.log(format!("[主电脑] 副电脑已连接：{address}\n"));
                Ok(Some(stream))
            }
            Ok(other) => {
                runtime.log(format!("[主电脑] 已忽略未握手连接：{address}，{other:?}\n"));
                Ok(None)
            }
            Err(err) => {
                runtime.log(format!("[主电脑] 已忽略非协议连接：{address}，{err:#}\n"));
                Ok(None)
            }
        },
        Err(err)
            if err.kind() == std::io::ErrorKind::WouldBlock
                || err.kind() == std::io::ErrorKind::TimedOut =>
        {
            runtime.log(format!("[主电脑] 已忽略扫描/静默连接：{address}\n"));
            Ok(None)
        }
        Err(err) => Err(err).context("读取握手失败"),
    }
}

fn handle_host_client(
    runtime: &Arc<AppRuntime>,
    stream: TcpStream,
    key_rx: &mpsc::Receiver<RawKeyEvent>,
    mouse_rx: &mpsc::Receiver<MouseInputEvent>,
) {
    let mut writer = match stream.try_clone() {
        Ok(writer) => writer,
        Err(err) => {
            runtime.log(format!("[主电脑] 连接初始化失败：{err}\n"));
            return;
        }
    };
    let mut reader = BufReader::new(stream);
    let _safety_guard = HostSessionSafetyGuard::new(Arc::clone(runtime));
    let mut line = String::new();
    let mut ctrl_down = false;
    let mut alt_down = false;
    let mut forwarded_key_logs = 0_u8;
    let mut last_switch = Instant::now()
        .checked_sub(Duration::from_millis(500))
        .unwrap_or_else(Instant::now);
    let mut last_mouse_pos = cursor_position().ok();
    let mut last_mouse_crossing = Instant::now()
        .checked_sub(Duration::from_secs(2))
        .unwrap_or_else(Instant::now);
    let mut mouse_was_suppressed = false;
    let mut last_target = runtime.target();
    let mut observed_release_generation = runtime.local_release_generation();
    let mut observed_emergency_generation = runtime.emergency_release_generation();
    let mut last_local_release = Instant::now()
        .checked_sub(LOCAL_RELEASE_COOLDOWN)
        .unwrap_or_else(Instant::now);
    let mut last_emergency_release = Instant::now()
        .checked_sub(EMERGENCY_RELEASE_COOLDOWN)
        .unwrap_or_else(Instant::now);
    let mut last_heartbeat = Instant::now();
    'session: while !runtime.should_stop() {
        if take_panic_request() {
            force_local_release(
                runtime,
                "[host] Ctrl+Alt+Esc emergency release: control returned to this computer\n",
            );
        }
        let release_generation = runtime.local_release_generation();
        if release_generation != observed_release_generation {
            observed_release_generation = release_generation;
            last_local_release = Instant::now();
            last_switch = Instant::now();
        }
        let emergency_generation = runtime.emergency_release_generation();
        if emergency_generation != observed_emergency_generation {
            observed_emergency_generation = emergency_generation;
            last_emergency_release = Instant::now();
        }
        let config = runtime.config();
        let current_target = runtime.target();
        let suppress_mouse = should_suppress_local_mouse_input(&config, current_target.clone());
        if should_release_remote_inputs(
            last_target.clone(),
            current_target.clone(),
            mouse_was_suppressed,
            suppress_mouse,
        ) {
            release_remote_inputs(runtime, &mut writer);
        }
        if last_target == KeyboardTarget::Remote && current_target == KeyboardTarget::Local {
            send_target_state(runtime, &mut writer);
        }
        set_mouse_input_suppression(suppress_mouse);
        mouse_was_suppressed = suppress_mouse;
        last_target = current_target.clone();
        while let Ok(event) = mouse_rx.try_recv() {
            if suppress_mouse
                && !send_bridge_event(
                    runtime,
                    &mut writer,
                    BridgeEvent::MouseInput { event },
                    "[host] failed to forward remote mouse input",
                )
            {
                break 'session;
            }
        }
        if last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            let heartbeat = BridgeEvent::Ping {
                message: "host-heartbeat".to_string(),
                probe_id: None,
                reply_to: None,
            };
            if let Err(err) = encode_event(&heartbeat).and_then(|bytes| {
                writer.write_all(&bytes)?;
                Ok(())
            }) {
                runtime.log(format!("[主电脑] 副电脑心跳检测失败，已断开：{err:#}\n"));
                break 'session;
            }
            last_heartbeat = Instant::now();
        }
        while let Ok(event) = key_rx.try_recv() {
            update_modifier_state(&event, &mut ctrl_down, &mut alt_down);
            if event.is_down && ctrl_down && alt_down && event.vk_code == 0x31 {
                apply_host_target(
                    runtime,
                    KeyboardTarget::Local,
                    "[主电脑] 快捷键切换：键盘回主电脑\n",
                    false,
                );
                last_switch = Instant::now();
                continue;
            }
            if event.is_down && ctrl_down && alt_down && event.vk_code == 0x32 {
                apply_host_target(
                    runtime,
                    KeyboardTarget::Remote,
                    "[主电脑] 快捷键切换：键盘到副电脑\n",
                    false,
                );
                last_switch = Instant::now();
                continue;
            }
            if runtime.target() != KeyboardTarget::Remote {
                continue;
            }
            let Some(key) = remote_key_payload(&event) else {
                continue;
            };
            let payload = BridgeEvent::Key {
                action: if event.is_down {
                    KeyAction::Down
                } else {
                    KeyAction::Up
                },
                key,
            };
            match encode_event(&payload).and_then(|bytes| {
                writer.write_all(&bytes)?;
                Ok(())
            }) {
                Ok(()) => {
                    if forwarded_key_logs < 5 {
                        forwarded_key_logs += 1;
                        let action = if event.is_down { "按下" } else { "松开" };
                        let BridgeEvent::Key { key, .. } = &payload else {
                            unreachable!();
                        };
                        runtime.log(format!("[主电脑] 已转发按键：{key} {action}\n"));
                    }
                }
                Err(err) => {
                    runtime.log(format!("[主电脑] 按键发送失败：{err:#}\n"));
                    break 'session;
                }
            }
        }

        if let Ok(current_pos) = cursor_position() {
            let config = runtime.config();
            if should_accept_mouse_input(&config) {
                let screen = virtual_screen_rect();
                let center = screen_center(screen);
                let moving_right = last_mouse_pos.is_some_and(|last| current_pos.x > last.x);
                if runtime.target() == KeyboardTarget::Local
                    && moving_right
                    && at_right_edge(current_pos, screen, 2)
                    && last_mouse_crossing.elapsed() >= Duration::from_millis(600)
                {
                    let remote_y = screen.y_permille(current_pos.y);
                    apply_host_target_and_notify(
                        runtime,
                        &mut writer,
                        KeyboardTarget::Remote,
                        "[host] mouse crossed right edge: keyboard and mouse to remote\n",
                        true,
                    );
                    let event = BridgeEvent::MouseInput {
                        event: MouseInputEvent::MoveToLeftEdge {
                            y_permille: remote_y,
                        },
                    };
                    if let Err(err) = encode_event(&event).and_then(|bytes| {
                        writer.write_all(&bytes)?;
                        Ok(())
                    }) {
                        runtime.log(format!(
                            "[host] failed to send remote mouse entry: {err:#}\n"
                        ));
                        break 'session;
                    }
                    last_switch = Instant::now();
                    last_mouse_crossing = Instant::now();
                    let _ = set_cursor_position(center);
                    last_mouse_pos = Some(center);
                    continue;
                }
                if runtime.target() == KeyboardTarget::Remote {
                    let dx = current_pos.x - center.x;
                    let dy = current_pos.y - center.y;
                    if dx != 0 || dy != 0 {
                        if !send_bridge_event(
                            runtime,
                            &mut writer,
                            BridgeEvent::MouseInput {
                                event: MouseInputEvent::MoveRelative { dx, dy },
                            },
                            "[host] failed to forward remote mouse movement",
                        ) {
                            break 'session;
                        }
                        let _ = set_cursor_position(center);
                        last_mouse_pos = Some(center);
                    }
                    continue;
                }
            }
            last_mouse_pos = Some(current_pos);
        }

        line.clear();
        match reader.get_mut().set_nonblocking(true) {
            Ok(()) => {}
            Err(err) => {
                runtime.log(format!("[主电脑] 连接状态设置失败：{err}\n"));
                break;
            }
        }
        match reader.read_line(&mut line) {
            Ok(bytes_read) if stream_read_reached_eof(bytes_read) => break,
            Ok(_) => match decode_event(line.as_bytes()) {
                Ok(BridgeEvent::TargetRequest { target }) => match target {
                    TargetSide::Local => {
                        apply_host_target_and_notify(
                            runtime,
                            &mut writer,
                            KeyboardTarget::Local,
                            "[主电脑] 副电脑请求：键盘回主电脑\n",
                            false,
                        );
                        last_switch = Instant::now();
                    }
                    TargetSide::Remote => {
                        apply_host_target_and_notify(
                            runtime,
                            &mut writer,
                            KeyboardTarget::Remote,
                            "[主电脑] 副电脑请求：键盘到副电脑\n",
                            false,
                        );
                        last_switch = Instant::now();
                    }
                },
                Ok(BridgeEvent::MouseActivity {
                    source: MouseSource::Remote,
                }) => {
                    if should_follow_mouse_activity(&runtime.config())
                        && should_accept_remote_mouse_activity(
                            last_switch.elapsed(),
                            last_local_release.elapsed(),
                            last_emergency_release.elapsed(),
                            Duration::from_millis(runtime.config().mouse_follow.switch_debounce_ms),
                        )
                        && apply_host_target(
                            runtime,
                            KeyboardTarget::Remote,
                            "[主电脑] 副电脑鼠标活动：键盘到副电脑\n",
                            true,
                        )
                    {
                        send_target_state(runtime, &mut writer);
                        last_switch = Instant::now();
                    }
                }
                Ok(BridgeEvent::Ping { .. }) => {}
                Ok(other) => runtime.log(format!("[主电脑] 收到消息：{other:?}\n")),
                Err(err) => runtime.log(format!("[主电脑] 已忽略异常消息：{err:#}\n")),
            },
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(err) => {
                runtime.log(format!("[主电脑] 读取副电脑消息失败：{err}\n"));
                break;
            }
        }
        thread::sleep(HOST_SESSION_POLL_INTERVAL);
    }
    if should_release_inputs_on_session_end(runtime.target(), mouse_was_suppressed) {
        release_remote_inputs(runtime, &mut writer);
    }
    set_mouse_input_suppression(false);
}

fn stream_read_reached_eof(bytes_read: usize) -> bool {
    bytes_read == 0
}

fn update_modifier_state(event: &RawKeyEvent, ctrl_down: &mut bool, alt_down: &mut bool) {
    match event.vk_code {
        0x11 | 0xA2 | 0xA3 => *ctrl_down = event.is_down,
        0x12 | 0xA4 | 0xA5 => *alt_down = event.is_down,
        _ => {}
    }
}

fn remote_key_payload(event: &RawKeyEvent) -> Option<String> {
    const VK_NUMLOCK: u32 = 0x90;
    if event.vk_code == VK_NUMLOCK {
        return None;
    }
    Some(format!("<{}>", event.vk_code))
}

fn apply_host_target(
    runtime: &Arc<AppRuntime>,
    target: KeyboardTarget,
    log_line: &str,
    log_only_on_change: bool,
) -> bool {
    let changed = runtime.target() != target;
    if !changed && log_only_on_change {
        return false;
    }
    if target == KeyboardTarget::Remote {
        if let Err(err) = release_local_modifiers() {
            runtime.log(format!("[主电脑] 释放本地修饰键失败：{err:#}\n"));
        }
    }
    runtime.set_target(target.clone());
    if target == KeyboardTarget::Local {
        runtime.mark_local_release();
    }
    set_key_suppression(target == KeyboardTarget::Remote);
    if changed || !log_only_on_change {
        runtime.log(log_line);
    }
    changed
}

fn apply_host_target_and_notify(
    runtime: &Arc<AppRuntime>,
    writer: &mut TcpStream,
    target: KeyboardTarget,
    log_line: &str,
    log_only_on_change: bool,
) -> bool {
    let changed = apply_host_target(runtime, target, log_line, log_only_on_change);
    if changed || !log_only_on_change {
        send_target_state(runtime, writer);
    }
    changed
}

fn target_state_for_device(target: &crate::routing::KeyboardTarget, device_id: &str) -> TargetSide {
    if target.device_id() == Some(device_id) {
        TargetSide::Remote
    } else {
        TargetSide::Local
    }
}

fn send_target_state(runtime: &Arc<AppRuntime>, writer: &mut TcpStream) {
    let target = match runtime.target() {
        KeyboardTarget::Local => TargetSide::Local,
        KeyboardTarget::Remote | KeyboardTarget::Device(_) => TargetSide::Remote,
    };
    let event = BridgeEvent::TargetState { target };
    if let Err(err) = encode_event(&event).and_then(|bytes| {
        writer.write_all(&bytes)?;
        Ok(())
    }) {
        runtime.log(format!("[主电脑] 键盘目标状态同步失败：{err:#}\n"));
    }
}

fn send_bridge_event(
    runtime: &Arc<AppRuntime>,
    writer: &mut TcpStream,
    event: BridgeEvent,
    error_prefix: &str,
) -> bool {
    if let Err(err) = encode_event(&event).and_then(|bytes| {
        writer.write_all(&bytes)?;
        Ok(())
    }) {
        runtime.log(format!("{error_prefix}: {err:#}\n"));
        return false;
    }
    true
}

fn release_remote_mouse_buttons(runtime: &Arc<AppRuntime>, writer: &mut TcpStream) {
    for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
        send_bridge_event(
            runtime,
            writer,
            BridgeEvent::MouseInput {
                event: MouseInputEvent::Button {
                    button,
                    action: MouseButtonAction::Up,
                },
            },
            "[host] failed to release remote mouse button",
        );
    }
}

fn release_remote_inputs(runtime: &Arc<AppRuntime>, writer: &mut TcpStream) {
    release_remote_mouse_buttons(runtime, writer);
    for vk_code in [0x10_u32, 0x11, 0x12, 0x5B, 0x5C] {
        let _ = send_bridge_event(
            runtime,
            writer,
            BridgeEvent::Key {
                action: KeyAction::Up,
                key: format!("<{vk_code}>"),
            },
            "[host] failed to release remote modifier key",
        );
    }
}

fn run_host_mouse_loop(runtime: Arc<AppRuntime>) {
    let Ok(mut last) = cursor_position() else {
        runtime.log("[主电脑] 鼠标监听不可用\n");
        return;
    };
    let mut last_switch = Instant::now()
        .checked_sub(Duration::from_millis(500))
        .unwrap_or_else(Instant::now);
    while !runtime.should_stop() {
        let config = runtime.config();
        thread::sleep(Duration::from_millis(
            config.mouse_follow.host_poll_interval_ms,
        ));
        if !should_follow_mouse_activity(&config) || !config.mouse_follow.host_mouse_returns_local {
            continue;
        }
        let Ok(current) = cursor_position() else {
            continue;
        };
        if current != last {
            last = current;
            if runtime.target() == KeyboardTarget::Remote {
                let debounce_ms = config.mouse_follow.switch_debounce_ms;
                let cooldown_ms = config.mouse_follow.host_priority_cooldown_ms;
                let wait_ms = debounce_ms.max(cooldown_ms);
                if last_switch.elapsed() < Duration::from_millis(wait_ms) {
                    continue;
                }
                if apply_host_target(
                    &runtime,
                    KeyboardTarget::Local,
                    "[主电脑] 主电脑鼠标活动：键盘回主电脑\n",
                    true,
                ) {
                    last_switch = Instant::now();
                }
            }
        }
    }
}

fn start_remote(runtime: Arc<AppRuntime>) -> Result<()> {
    thread::Builder::new()
        .name("devices-router-remote".to_string())
        .spawn(move || {
            if let Err(err) = run_remote(runtime.clone()) {
                runtime.log(format!("[remote] stopped: {err:#}\n"));
            }
        })
        .context("spawn remote thread")?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RemoteHandshake {
    Accepted,
    LegacyHost,
}

fn classify_remote_handshake(event: BridgeEvent) -> Result<RemoteHandshake> {
    match event {
        BridgeEvent::ServerHello { accepted: true, .. } => Ok(RemoteHandshake::Accepted),
        BridgeEvent::ServerHello {
            accepted: false,
            reason,
            ..
        } => bail!(
            "{}",
            reason.unwrap_or_else(|| "host rejected the connection".to_string())
        ),
        BridgeEvent::Ping { .. } => Ok(RemoteHandshake::LegacyHost),
        other => bail!("unexpected host handshake response: {other:?}"),
    }
}

fn read_remote_handshake(stream: &mut TcpStream) -> Result<RemoteHandshake> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .context("set host handshake timeout")?;
    let mut bytes = Vec::with_capacity(256);
    let mut byte = [0_u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => bail!("host closed during handshake"),
            Ok(_) => {
                bytes.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
                if bytes.len() > 8 * 1024 {
                    bail!("host handshake response is too large");
                }
            }
            Err(err) => return Err(err).context("read host handshake response"),
        }
    }
    stream
        .set_read_timeout(None)
        .context("clear host handshake timeout")?;
    classify_remote_handshake(decode_event(&bytes)?)
}

fn run_remote(runtime: Arc<AppRuntime>) -> Result<()> {
    while !runtime.should_stop() {
        let Some(target) = resolve_remote_target(&runtime) else {
            thread::sleep(Duration::from_secs(2));
            continue;
        };
        match TcpStream::connect(target.as_str()) {
            Ok(mut stream) => {
                let config = runtime.config();
                stream.write_all(&encode_event(&BridgeEvent::ClientHello {
                    role: ClientRole::Remote,
                    device_id: Some(config.device_id),
                    device_name: Some(crate::config::computer_name()),
                    capabilities: vec![
                        "multi_remote_v1".to_string(),
                        "server_hello_v1".to_string(),
                    ],
                })?)?;
                match read_remote_handshake(&mut stream) {
                    Ok(RemoteHandshake::Accepted) => {
                        runtime.log(format!("[remote] connected to host: {target}\n"))
                    }
                    Ok(RemoteHandshake::LegacyHost) => {
                        runtime.log(format!("[remote] connected to legacy host: {target}\n"))
                    }
                    Err(err) => {
                        runtime.set_connected(false);
                        runtime.log(format!(
                            "[remote] host rejected or failed handshake: {err:#}\n"
                        ));
                        thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                }
                runtime.set_connected(true);
                if let Ok(address) = stream.peer_addr() {
                    if let Some(host) = host_from_socket_addr(&address) {
                        check_remote_update(
                            Arc::clone(&runtime),
                            &host,
                            runtime.config().update_port,
                        );
                    }
                }
                let mut writer = stream
                    .try_clone()
                    .context("clone remote stream for writer")?;
                let (event_tx, event_rx) = mpsc::channel::<BridgeEvent>();
                let sender_generation = runtime.set_remote_sender(event_tx.clone());
                let writer_runtime = Arc::clone(&runtime);
                let latency_tracker = Arc::new(Mutex::new(LatencyProbeTracker::default()));
                let writer_latency_tracker = Arc::clone(&latency_tracker);
                thread::Builder::new()
                    .name("devices-router-remote-writer".to_string())
                    .spawn(move || {
                        let mut last_probe = Instant::now();
                        loop {
                            let event = match event_rx.recv_timeout(HEARTBEAT_INTERVAL) {
                                Ok(event) => event,
                                Err(RecvTimeoutError::Timeout) => BridgeEvent::Ping {
                                    message: "remote-heartbeat".to_string(),
                                    probe_id: None,
                                    reply_to: None,
                                },
                                Err(RecvTimeoutError::Disconnected) => break,
                            };
                            let result = encode_event(&event).and_then(|bytes| {
                                writer.write_all(&bytes)?;
                                if last_probe.elapsed() >= LATENCY_PROBE_INTERVAL {
                                    let now = Instant::now();
                                    let probe_id = writer_latency_tracker
                                        .lock()
                                        .expect("latency tracker lock poisoned")
                                        .start_probe(now);
                                    let probe = BridgeEvent::Ping {
                                        message: "latency-probe".to_string(),
                                        probe_id: Some(probe_id),
                                        reply_to: None,
                                    };
                                    writer.write_all(&encode_event(&probe)?)?;
                                    last_probe = now;
                                }
                                Ok(())
                            });
                            if let Err(err) = result {
                                writer_runtime.log(format!("[副电脑] 发送控制消息失败：{err:#}\n"));
                                break;
                            }
                        }
                    })
                    .context("spawn remote writer loop")?;
                if MOUSE_ACTIVITY_FOLLOW_AVAILABLE {
                    let mouse_runtime = Arc::clone(&runtime);
                    thread::Builder::new()
                        .name("devices-router-remote-mouse".to_string())
                        .spawn(move || run_remote_mouse_loop(mouse_runtime, event_tx))
                        .context("spawn remote mouse loop")?;
                }
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                let mut received_key_logs = 0_u8;
                while !runtime.should_stop() {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => match decode_event(line.as_bytes()) {
                            Ok(BridgeEvent::Key { action, key }) => {
                                let is_down = matches!(action, KeyAction::Down);
                                if let Err(err) = send_key_event(&key, is_down) {
                                    runtime
                                        .log(format!("[副电脑] 已忽略无法输入的按键：{err:#}\n"));
                                } else if received_key_logs < 5 {
                                    received_key_logs += 1;
                                    let action_label = if is_down { "按下" } else { "松开" };
                                    runtime.log(format!(
                                        "[副电脑] 已输入按键：{key} {action_label}\n"
                                    ));
                                }
                            }
                            Ok(BridgeEvent::TargetState { target }) => {
                                let target = match target {
                                    TargetSide::Local => KeyboardTarget::Local,
                                    TargetSide::Remote => {
                                        KeyboardTarget::Device(runtime.config().device_id)
                                    }
                                };
                                runtime.set_target(target.clone());
                                let label = match target {
                                    KeyboardTarget::Local => "\u{4e3b}\u{7535}\u{8111}",
                                    KeyboardTarget::Remote | KeyboardTarget::Device(_) => {
                                        "\u{526f}\u{7535}\u{8111}"
                                    }
                                };
                                runtime.log(format!("[副电脑] 主电脑确认键盘目标：{label}\n"));
                            }
                            Ok(BridgeEvent::MouseInput { event }) => {
                                let config = runtime.config();
                                if !should_accept_mouse_input(&config) {
                                    runtime.log("[remote] ignored mouse input: experimental mouse input is disabled or game mode is active\n");
                                    continue;
                                }
                                if let Err(err) = send_mouse_input_event(&event) {
                                    runtime.log(format!(
                                        "[remote] ignored mouse input injection failure: {err:#}\n"
                                    ));
                                } else if cursor_position().is_ok_and(|cursor| {
                                    should_request_local_return(
                                        runtime.target(),
                                        &event,
                                        cursor,
                                        virtual_screen_rect(),
                                    )
                                }) {
                                    runtime.log("[remote] left edge reached: requesting control return to host\n");
                                    let _ = runtime.send_remote_event(BridgeEvent::TargetRequest {
                                        target: TargetSide::Local,
                                    });
                                }
                            }
                            Ok(BridgeEvent::Ping {
                                probe_id, reply_to, ..
                            }) => {
                                if let Some(probe_id) = probe_id {
                                    let _ = runtime.send_remote_event(BridgeEvent::Ping {
                                        message: "latency-reply".to_string(),
                                        probe_id: None,
                                        reply_to: Some(probe_id),
                                    });
                                }
                                if let Some(reply_to) = reply_to {
                                    let sample_ms = latency_tracker
                                        .lock()
                                        .expect("latency tracker lock poisoned")
                                        .complete_probe(reply_to, Instant::now());
                                    if let Some(sample_ms) = sample_ms {
                                        runtime.record_host_latency(sample_ms);
                                    }
                                }
                            }
                            Ok(other) => runtime.log(format!("[副电脑] 收到消息：{other:?}\n")),
                            Err(err) => runtime.log(format!("[副电脑] 已忽略异常消息：{err:#}\n")),
                        },
                        Err(err) => {
                            runtime.log(format!("[副电脑] 连接读取失败：{err}\n"));
                            break;
                        }
                    }
                }
                runtime.set_connected(false);
                runtime.clear_remote_sender(sender_generation);
            }
            Err(err) => {
                runtime.log(format!("[副电脑] 连接失败：{target}，{err}\n"));
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
    Ok(())
}

fn should_accept_mouse_input(config: &AppConfig) -> bool {
    CROSS_SCREEN_MOUSE_AVAILABLE && config.experimental_mouse_input && !config.game_mode
}

fn should_follow_mouse_activity(config: &AppConfig) -> bool {
    MOUSE_ACTIVITY_FOLLOW_AVAILABLE && config.mouse_follow.enabled && !config.game_mode
}

fn should_suppress_local_mouse_input(config: &AppConfig, target: KeyboardTarget) -> bool {
    should_accept_mouse_input(config) && target == KeyboardTarget::Remote
}

fn should_release_remote_inputs(
    previous_target: KeyboardTarget,
    current_target: KeyboardTarget,
    mouse_was_suppressed: bool,
    suppress_mouse: bool,
) -> bool {
    (previous_target == KeyboardTarget::Remote && current_target == KeyboardTarget::Local)
        || (mouse_was_suppressed && !suppress_mouse)
}

fn should_accept_remote_mouse_activity(
    time_since_switch: Duration,
    time_since_local_release: Duration,
    time_since_emergency_release: Duration,
    switch_debounce: Duration,
) -> bool {
    time_since_switch >= switch_debounce
        && time_since_local_release >= LOCAL_RELEASE_COOLDOWN
        && time_since_emergency_release >= EMERGENCY_RELEASE_COOLDOWN
}

fn should_release_inputs_on_session_end(
    target: KeyboardTarget,
    mouse_was_suppressed: bool,
) -> bool {
    target == KeyboardTarget::Remote || mouse_was_suppressed
}

fn should_request_local_return(
    target: KeyboardTarget,
    event: &MouseInputEvent,
    cursor: MousePosition,
    screen: ScreenRect,
) -> bool {
    target == KeyboardTarget::Remote
        && matches!(event, MouseInputEvent::MoveRelative { dx, .. } if *dx < 0)
        && at_left_edge(cursor, screen, 2)
}

fn resolve_remote_target(runtime: &Arc<AppRuntime>) -> Option<String> {
    let config = runtime.config();
    if let Some(host) = config
        .remote_host
        .as_ref()
        .filter(|host| !host.trim().is_empty())
    {
        return Some(format!("{}:{}", host.trim(), config.tcp_port));
    }
    if !config.auto_discovery {
        runtime.log("[副电脑] 自动发现已关闭，请在网络诊断里填写主电脑 IP。\n");
        return None;
    }
    runtime.log("[副电脑] 正在自动寻找主电脑...\n");
    match discover_host(Duration::from_secs(8)) {
        Ok(found) => {
            let target = format!("{}:{}", found.host, found.port);
            runtime.log(format!("[副电脑] 发现主电脑：{target}\n"));
            Some(target)
        }
        Err(err) => {
            if err.chain().any(|cause| {
                cause
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.raw_os_error() == Some(10048))
            }) {
                runtime.log("[副电脑] 广播发现端口被占用，已跳过广播发现。\n");
            } else {
                runtime.log(format!("[副电脑] 广播发现失败：{err:#}\n"));
            }
            runtime.log("[副电脑] 正在扫描本地网络...\n");
            if let Some(found) = scan_local_network(config.tcp_port, Duration::from_millis(120)) {
                let target = format!("{}:{}", found.host, found.port);
                runtime.log(format!("[副电脑] 扫描找到主电脑：{target}\n"));
                return Some(target);
            }
            runtime.log("[副电脑] 本地扫描失败，将继续重试。也可以手动填写主电脑 IP。\n");
            None
        }
    }
}

fn run_remote_mouse_loop(runtime: Arc<AppRuntime>, event_tx: mpsc::Sender<BridgeEvent>) {
    let Ok(mut last) = cursor_position() else {
        runtime.log("[副电脑] 鼠标监听不可用\n");
        return;
    };
    let mut activity_logs = 0_u8;
    while !runtime.should_stop() {
        let config = runtime.config();
        thread::sleep(Duration::from_millis(
            config.mouse_follow.remote_report_interval_ms,
        ));
        if !should_follow_mouse_activity(&config)
            || !config.mouse_follow.remote_mouse_switches_remote
        {
            continue;
        }
        let Ok(current) = cursor_position() else {
            continue;
        };
        if current == last {
            continue;
        }
        last = current;
        let event = BridgeEvent::MouseActivity {
            source: MouseSource::Remote,
        };
        if event_tx.send(event).is_err() {
            runtime.log("[副电脑] 鼠标活动上报已停止，等待重新连接\n");
            return;
        }
        if activity_logs < 3 {
            activity_logs += 1;
            runtime.log("[副电脑] 已上报鼠标活动给主电脑\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::AppConfig;
    use crate::mouse::{MousePosition, ScreenRect};

    use super::*;

    #[test]
    fn zero_byte_read_marks_tcp_session_as_finished() {
        assert!(stream_read_reached_eof(0));
        assert!(!stream_read_reached_eof(1));
    }

    #[test]
    fn remote_payload_ignores_num_lock() {
        let event = RawKeyEvent {
            vk_code: 0x90,
            is_down: true,
            text: None,
        };

        assert_eq!(remote_key_payload(&event), None);
    }

    #[test]
    fn remote_payload_keeps_vk_even_when_text_is_available() {
        let down = RawKeyEvent {
            vk_code: 0x41,
            is_down: true,
            text: Some("a".to_string()),
        };
        let up = RawKeyEvent {
            vk_code: 0x41,
            is_down: false,
            text: Some("a".to_string()),
        };

        assert_eq!(remote_key_payload(&down), Some("<65>".to_string()));
        assert_eq!(remote_key_payload(&up), Some("<65>".to_string()));
    }

    #[test]
    fn remote_payload_keeps_non_text_keys() {
        let event = RawKeyEvent {
            vk_code: 0x09,
            is_down: true,
            text: None,
        };

        assert_eq!(remote_key_payload(&event), Some("<9>".to_string()));
    }

    #[test]
    fn mouse_input_is_disabled_even_if_legacy_config_requests_it() {
        let mut config = AppConfig {
            experimental_mouse_input: true,
            ..AppConfig::default()
        };
        assert!(!should_accept_mouse_input(&config));

        config.game_mode = true;
        assert!(!should_accept_mouse_input(&config));
    }

    #[test]
    fn mouse_activity_can_follow_keyboard_without_cross_screen_mouse_input() {
        let config = AppConfig::default();

        assert!(should_follow_mouse_activity(&config));
        assert!(!should_accept_mouse_input(&config));
        assert!(!should_suppress_local_mouse_input(
            &config,
            KeyboardTarget::Remote
        ));
    }

    #[test]
    fn automatic_follow_latency_budget_stays_below_200ms() {
        assert!(LOCAL_RELEASE_COOLDOWN <= Duration::from_millis(150));
        assert!(HOST_SESSION_POLL_INTERVAL <= Duration::from_millis(5));
        assert!(EMERGENCY_RELEASE_COOLDOWN >= Duration::from_secs(1));
    }

    #[test]
    fn local_mouse_events_are_never_suppressed_in_keyboard_only_release() {
        let mut config = AppConfig::default();

        assert!(!should_suppress_local_mouse_input(
            &config,
            KeyboardTarget::Local
        ));
        config.experimental_mouse_input = true;
        assert!(!should_suppress_local_mouse_input(
            &config,
            KeyboardTarget::Remote
        ));

        config.game_mode = true;
        assert!(!should_suppress_local_mouse_input(
            &config,
            KeyboardTarget::Remote
        ));
    }

    #[test]
    fn remote_leftward_move_at_clamped_left_edge_requests_local() {
        let screen = ScreenRect {
            left: -1920,
            top: 0,
            width: 1920,
            height: 1080,
        };

        assert!(should_request_local_return(
            KeyboardTarget::Remote,
            &MouseInputEvent::MoveRelative { dx: -5, dy: 0 },
            MousePosition { x: -1920, y: 500 },
            screen,
        ));
    }

    #[test]
    fn return_request_rejects_non_leftward_or_local_movement() {
        let screen = ScreenRect {
            left: 0,
            top: 0,
            width: 1920,
            height: 1080,
        };
        let edge = MousePosition { x: 0, y: 500 };

        assert!(!should_request_local_return(
            KeyboardTarget::Remote,
            &MouseInputEvent::MoveRelative { dx: 5, dy: 0 },
            edge,
            screen,
        ));
        assert!(!should_request_local_return(
            KeyboardTarget::Local,
            &MouseInputEvent::MoveRelative { dx: -5, dy: 0 },
            edge,
            screen,
        ));
    }

    #[test]
    fn force_local_release_sets_target_local() {
        let state = crate::app_state::SharedState::new("test");
        let runtime = state.runtime();
        runtime.start(AppMode::Host);
        runtime.set_target(KeyboardTarget::Remote);

        force_local_release(&runtime, "test release");

        assert_eq!(runtime.target(), KeyboardTarget::Local);
    }

    #[test]
    fn host_session_guard_fails_open_when_connection_ends() {
        let state = crate::app_state::SharedState::new("test");
        let runtime = state.runtime();
        runtime.start(AppMode::Host);
        runtime.set_target(KeyboardTarget::Remote);

        drop(HostSessionSafetyGuard::new(Arc::clone(&runtime)));

        assert_eq!(runtime.target(), KeyboardTarget::Local);
    }

    #[test]
    fn remote_to_local_releases_inputs_even_when_mouse_was_not_suppressed() {
        assert!(should_release_remote_inputs(
            KeyboardTarget::Remote,
            KeyboardTarget::Local,
            false,
            false,
        ));
    }

    #[test]
    fn disabling_mouse_suppression_releases_buttons_without_target_change() {
        assert!(should_release_remote_inputs(
            KeyboardTarget::Remote,
            KeyboardTarget::Remote,
            true,
            false,
        ));
    }

    #[test]
    fn queued_remote_activity_is_rejected_during_emergency_release_cooldown() {
        assert!(!should_accept_remote_mouse_activity(
            Duration::from_secs(2),
            Duration::from_secs(2),
            Duration::from_millis(20),
            Duration::from_millis(80),
        ));
        assert!(should_accept_remote_mouse_activity(
            Duration::from_secs(2),
            Duration::from_secs(2),
            Duration::from_secs(2),
            Duration::from_millis(80),
        ));
    }

    #[test]
    fn remote_keyboard_only_session_still_releases_inputs_on_exit() {
        assert!(should_release_inputs_on_session_end(
            KeyboardTarget::Remote,
            false,
        ));
        assert!(!should_release_inputs_on_session_end(
            KeyboardTarget::Local,
            false,
        ));
    }

    #[test]
    fn remote_handshake_accepts_new_and_legacy_hosts() {
        assert_eq!(
            classify_remote_handshake(BridgeEvent::ServerHello {
                accepted: true,
                reason: None,
                max_devices: 2
            })
            .unwrap(),
            RemoteHandshake::Accepted
        );
        assert_eq!(
            classify_remote_handshake(BridgeEvent::Ping {
                message: "ok".to_string(),
                probe_id: None,
                reply_to: None,
            })
            .unwrap(),
            RemoteHandshake::LegacyHost
        );
    }

    #[test]
    fn remote_handshake_surfaces_host_rejection_reason() {
        let error = classify_remote_handshake(BridgeEvent::ServerHello {
            accepted: false,
            reason: Some("two device limit".to_string()),
            max_devices: 2,
        })
        .unwrap_err();
        assert!(error.to_string().contains("two device limit"));
    }

    #[test]
    fn target_state_is_remote_only_for_selected_device() {
        let target = crate::routing::KeyboardTarget::Device("a".to_string());

        assert_eq!(target_state_for_device(&target, "a"), TargetSide::Remote);
        assert_eq!(target_state_for_device(&target, "b"), TargetSide::Local);
    }
}
