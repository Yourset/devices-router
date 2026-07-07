use crate::app_state::{AppMode, AppRuntime, KeyboardTarget};
use crate::discovery::{broadcast_host, discover_host, scan_local_network};
use crate::input::send_key_event;
use crate::keyboard_hook::{run_keyboard_hook, set_key_suppression, RawKeyEvent};
use crate::mouse::cursor_position;
use crate::protocol::{
    decode_event, encode_event, BridgeEvent, ClientRole, KeyAction, MouseSource, TargetSide,
};
use crate::updates::{check_remote_update, host_from_socket_addr, start_update_server};
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const TCP_PORT: u16 = 8765;

pub fn start_mode(mode: AppMode, runtime: Arc<AppRuntime>) -> Result<()> {
    runtime.request_stop();
    set_key_suppression(false);
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
            if let Err(err) = run_host(runtime.clone()) {
                runtime.log(format!("[主电脑] 已停止：{err:#}\n"));
            }
        })
        .context("spawn host thread")?;
    Ok(())
}

fn run_host(runtime: Arc<AppRuntime>) -> Result<()> {
    let (key_tx, key_rx) = mpsc::channel::<RawKeyEvent>();
    let hook_runtime = Arc::clone(&runtime);
    thread::Builder::new()
        .name("devices-router-keyboard-hook".to_string())
        .spawn(move || {
            if let Err(err) = run_keyboard_hook(key_tx) {
                hook_runtime.log(format!("[主电脑] 键盘监听失败：{err:#}\n"));
            }
        })
        .context("spawn keyboard hook thread")?;
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
    let mouse_runtime = Arc::clone(&runtime);
    thread::Builder::new()
        .name("devices-router-host-mouse".to_string())
        .spawn(move || run_host_mouse_loop(mouse_runtime))
        .context("spawn host mouse loop")?;

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
                    handle_host_client(&runtime, stream, &key_rx);
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
) {
    let mut writer = match stream.try_clone() {
        Ok(writer) => writer,
        Err(err) => {
            runtime.log(format!("[主电脑] 连接初始化失败：{err}\n"));
            return;
        }
    };
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut ctrl_down = false;
    let mut alt_down = false;
    let mut forwarded_key_logs = 0_u8;
    while !runtime.should_stop() {
        while let Ok(event) = key_rx.try_recv() {
            update_modifier_state(event, &mut ctrl_down, &mut alt_down);
            if event.is_down && ctrl_down && alt_down && event.vk_code == 0x31 {
                runtime.set_target(KeyboardTarget::Local);
                set_key_suppression(false);
                runtime.log("[主电脑] 快捷键切换：键盘回主电脑\n");
                continue;
            }
            if event.is_down && ctrl_down && alt_down && event.vk_code == 0x32 {
                runtime.set_target(KeyboardTarget::Remote);
                set_key_suppression(true);
                runtime.log("[主电脑] 快捷键切换：键盘到副电脑\n");
                continue;
            }
            if runtime.target() != KeyboardTarget::Remote {
                continue;
            }
            let payload = BridgeEvent::Key {
                action: if event.is_down {
                    KeyAction::Down
                } else {
                    KeyAction::Up
                },
                key: format!("<{}>", event.vk_code),
            };
            match encode_event(&payload).and_then(|bytes| {
                writer.write_all(&bytes)?;
                Ok(())
            }) {
                Ok(()) => {
                    if forwarded_key_logs < 5 {
                        forwarded_key_logs += 1;
                        let action = if event.is_down { "按下" } else { "松开" };
                        runtime.log(format!(
                            "[主电脑] 已转发按键：<{vk}> {action}\n",
                            vk = event.vk_code
                        ));
                    }
                }
                Err(err) => {
                    runtime.log(format!("[主电脑] 按键发送失败：{err:#}\n"));
                    return;
                }
            }
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
            Ok(0) => {}
            Ok(_) => match decode_event(line.as_bytes()) {
                Ok(BridgeEvent::TargetRequest { target }) => match target {
                    TargetSide::Local => {
                        runtime.set_target(KeyboardTarget::Local);
                        set_key_suppression(false);
                        runtime.log("[主电脑] 副电脑请求：键盘回主电脑\n");
                    }
                    TargetSide::Remote => {
                        runtime.set_target(KeyboardTarget::Remote);
                        set_key_suppression(true);
                        runtime.log("[主电脑] 副电脑请求：键盘到副电脑\n");
                    }
                },
                Ok(BridgeEvent::MouseActivity {
                    source: MouseSource::Remote,
                }) => {
                    runtime.set_target(KeyboardTarget::Remote);
                    set_key_suppression(true);
                    runtime.log("[主电脑] 键盘目标：副电脑\n");
                }
                Ok(other) => runtime.log(format!("[主电脑] 收到消息：{other:?}\n")),
                Err(err) => runtime.log(format!("[主电脑] 已忽略异常消息：{err:#}\n")),
            },
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(err) => {
                runtime.log(format!("[主电脑] 读取副电脑消息失败：{err}\n"));
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn update_modifier_state(event: RawKeyEvent, ctrl_down: &mut bool, alt_down: &mut bool) {
    match event.vk_code {
        0x11 | 0xA2 | 0xA3 => *ctrl_down = event.is_down,
        0x12 | 0xA4 | 0xA5 => *alt_down = event.is_down,
        _ => {}
    }
}

fn run_host_mouse_loop(runtime: Arc<AppRuntime>) {
    let Ok(mut last) = cursor_position() else {
        runtime.log("[主电脑] 鼠标监听不可用\n");
        return;
    };
    let mut remote_since: Option<Instant> = None;
    while !runtime.should_stop() {
        let config = runtime.config();
        thread::sleep(Duration::from_millis(
            config.mouse_follow.host_poll_interval_ms,
        ));
        if !config.mouse_follow.enabled || !config.mouse_follow.host_mouse_returns_local {
            continue;
        }
        let Ok(current) = cursor_position() else {
            continue;
        };
        let target_is_remote = runtime.target() == KeyboardTarget::Remote;
        match (target_is_remote, remote_since) {
            (true, None) => remote_since = Some(Instant::now()),
            (false, Some(_)) => remote_since = None,
            _ => {}
        }
        if current != last {
            last = current;
            if target_is_remote {
                let cooldown_ms = config.mouse_follow.host_priority_cooldown_ms;
                let in_cooldown = remote_since
                    .map(|since| since.elapsed() < Duration::from_millis(cooldown_ms))
                    .unwrap_or(false);
                if in_cooldown {
                    continue;
                }
                runtime.set_target(KeyboardTarget::Local);
                set_key_suppression(false);
                runtime.log("[主电脑] 鼠标移动：键盘回主电脑\n");
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

fn run_remote(runtime: Arc<AppRuntime>) -> Result<()> {
    while !runtime.should_stop() {
        let Some(target) = resolve_remote_target(&runtime) else {
            thread::sleep(Duration::from_secs(2));
            continue;
        };
        match TcpStream::connect(target.as_str()) {
            Ok(mut stream) => {
                runtime.log(format!("[副电脑] 已连接主电脑：{target}\n"));
                runtime.set_connected(true);
                stream.write_all(&encode_event(&BridgeEvent::ClientHello {
                    role: ClientRole::Remote,
                })?)?;
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
                runtime.set_remote_sender(Some(event_tx.clone()));
                let writer_runtime = Arc::clone(&runtime);
                thread::Builder::new()
                    .name("devices-router-remote-writer".to_string())
                    .spawn(move || {
                        while let Ok(event) = event_rx.recv() {
                            let result = encode_event(&event).and_then(|bytes| {
                                writer.write_all(&bytes)?;
                                Ok(())
                            });
                            if let Err(err) = result {
                                writer_runtime.log(format!("[副电脑] 发送控制消息失败：{err:#}\n"));
                                break;
                            }
                        }
                    })
                    .context("spawn remote writer loop")?;
                let mouse_runtime = Arc::clone(&runtime);
                thread::Builder::new()
                    .name("devices-router-remote-mouse".to_string())
                    .spawn(move || run_remote_mouse_loop(mouse_runtime, event_tx))
                    .context("spawn remote mouse loop")?;
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
                runtime.set_remote_sender(None);
            }
            Err(err) => {
                runtime.log(format!("[副电脑] 连接失败：{target}，{err}\n"));
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
    Ok(())
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
    while !runtime.should_stop() {
        let config = runtime.config();
        thread::sleep(Duration::from_millis(
            config.mouse_follow.remote_report_interval_ms,
        ));
        if !config.mouse_follow.enabled || !config.mouse_follow.remote_mouse_switches_remote {
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
    }
}
