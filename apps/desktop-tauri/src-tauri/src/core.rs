use crate::app_state::{AppMode, AppRuntime, KeyboardTarget};
use crate::discovery::{broadcast_host, discover_host, scan_local_network};
use crate::input::send_key_event;
use crate::keyboard_hook::{run_keyboard_hook, RawKeyEvent};
use crate::mouse::cursor_position;
use crate::protocol::{
    decode_event, encode_event, BridgeEvent, ClientRole, KeyAction, MouseSource,
};
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const TCP_PORT: u16 = 8765;

pub fn start_mode(mode: AppMode, runtime: Arc<AppRuntime>) -> Result<()> {
    runtime.request_stop();
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
                runtime.log(format!("[host] stopped: {err:#}\n"));
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
                hook_runtime.log(format!("[host] keyboard hook failed: {err:#}\n"));
            }
        })
        .context("spawn keyboard hook thread")?;
    let discovery_runtime = Arc::clone(&runtime);
    thread::Builder::new()
        .name("devices-router-discovery-broadcast".to_string())
        .spawn(move || {
            let stop_runtime = Arc::clone(&discovery_runtime);
            if let Err(err) = broadcast_host(move || stop_runtime.should_stop(), TCP_PORT) {
                discovery_runtime.log(format!("[host] discovery broadcast failed: {err:#}\n"));
            }
        })
        .context("spawn discovery broadcaster")?;
    let mouse_runtime = Arc::clone(&runtime);
    thread::Builder::new()
        .name("devices-router-host-mouse".to_string())
        .spawn(move || run_host_mouse_loop(mouse_runtime))
        .context("spawn host mouse loop")?;

    let listener = TcpListener::bind(("0.0.0.0", TCP_PORT)).context("bind host TCP listener")?;
    listener
        .set_nonblocking(true)
        .context("set host listener nonblocking")?;
    runtime.log(format!("[host] listening on 0.0.0.0:{TCP_PORT}\n"));
    while !runtime.should_stop() {
        match listener.accept() {
            Ok((mut stream, address)) => {
                runtime.log(format!("[host] client connected: {address}\n"));
                runtime.set_connected(true);
                let _ = stream.write_all(&encode_event(&BridgeEvent::Ping {
                    message: "ok".to_string(),
                })?);
                handle_host_client(&runtime, stream, &key_rx);
                runtime.set_connected(false);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(100));
            }
            Err(err) => return Err(err).context("accept host client"),
        }
    }
    Ok(())
}

fn handle_host_client(
    runtime: &Arc<AppRuntime>,
    stream: TcpStream,
    key_rx: &mpsc::Receiver<RawKeyEvent>,
) {
    let mut writer = match stream.try_clone() {
        Ok(writer) => writer,
        Err(err) => {
            runtime.log(format!("[host] client clone failed: {err}\n"));
            return;
        }
    };
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    while !runtime.should_stop() {
        while let Ok(event) = key_rx.try_recv() {
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
                Ok(()) => {}
                Err(err) => {
                    runtime.log(format!("[host] key send failed: {err:#}\n"));
                    return;
                }
            }
        }

        line.clear();
        match reader.get_mut().set_nonblocking(true) {
            Ok(()) => {}
            Err(err) => {
                runtime.log(format!("[host] nonblocking failed: {err}\n"));
                break;
            }
        }
        match reader.read_line(&mut line) {
            Ok(0) => {}
            Ok(_) => match decode_event(line.as_bytes()) {
                Ok(BridgeEvent::MouseActivity {
                    source: MouseSource::Remote,
                }) => {
                    runtime.set_target(KeyboardTarget::Remote);
                    runtime.log("[host] keyboard target: remote\n");
                }
                Ok(other) => runtime.log(format!("[host] received: {other:?}\n")),
                Err(err) => runtime.log(format!("[host] invalid message: {err:#}\n")),
            },
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(err) => {
                runtime.log(format!("[host] client read failed: {err}\n"));
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn run_host_mouse_loop(runtime: Arc<AppRuntime>) {
    let Ok(mut last) = cursor_position() else {
        runtime.log("[host] cursor tracking unavailable\n");
        return;
    };
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
        if current != last {
            last = current;
            runtime.set_target(KeyboardTarget::Local);
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
        let target = resolve_remote_target(&runtime);
        match TcpStream::connect(target.as_str()) {
            Ok(mut stream) => {
                runtime.log(format!("[remote] connected to {target}\n"));
                runtime.set_connected(true);
                stream.write_all(&encode_event(&BridgeEvent::ClientHello {
                    role: ClientRole::Remote,
                })?)?;
                let mouse_stream = stream.try_clone().context("clone remote stream for mouse")?;
                let mouse_runtime = Arc::clone(&runtime);
                thread::Builder::new()
                    .name("devices-router-remote-mouse".to_string())
                    .spawn(move || run_remote_mouse_loop(mouse_runtime, mouse_stream))
                    .context("spawn remote mouse loop")?;
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                while !runtime.should_stop() {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => match decode_event(line.as_bytes()) {
                            Ok(BridgeEvent::Key { action, key }) => {
                                let is_down = matches!(action, KeyAction::Down);
                                if let Err(err) = send_key_event(&key, is_down) {
                                    runtime.log(format!("[remote] key ignored: {err:#}\n"));
                                }
                            }
                            Ok(other) => runtime.log(format!("[remote] received: {other:?}\n")),
                            Err(err) => runtime.log(format!("[remote] invalid message: {err:#}\n")),
                        },
                        Err(err) => {
                            runtime.log(format!("[remote] read failed: {err}\n"));
                            break;
                        }
                    }
                }
                runtime.set_connected(false);
            }
            Err(err) => {
                runtime.log(format!("[remote] connection failed: {target}, {err}\n"));
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
    Ok(())
}

fn resolve_remote_target(runtime: &Arc<AppRuntime>) -> String {
    let config = runtime.config();
    if let Some(host) = config.remote_host.as_ref().filter(|host| !host.trim().is_empty()) {
        return format!("{}:{}", host.trim(), config.tcp_port);
    }
    runtime.log("[remote] searching for host...\n");
    match discover_host(Duration::from_secs(8)) {
        Ok(found) => {
            let target = format!("{}:{}", found.host, found.port);
            runtime.log(format!("[remote] discovered host: {target}\n"));
            target
        }
        Err(err) => {
            runtime.log(format!("[remote] discovery failed: {err:#}\n"));
            runtime.log("[remote] scanning local network...\n");
            if let Some(found) = scan_local_network(config.tcp_port, Duration::from_millis(120)) {
                let target = format!("{}:{}", found.host, found.port);
                runtime.log(format!("[remote] found host by scan: {target}\n"));
                return target;
            }
            runtime.log("[remote] local scan failed, falling back to 127.0.0.1\n");
            format!("127.0.0.1:{}", config.tcp_port)
        }
    }
}

fn run_remote_mouse_loop(runtime: Arc<AppRuntime>, mut stream: TcpStream) {
    let Ok(mut last) = cursor_position() else {
        runtime.log("[remote] cursor tracking unavailable\n");
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
        match encode_event(&event).and_then(|bytes| {
            stream.write_all(&bytes)?;
            Ok(())
        }) {
            Ok(()) => {}
            Err(err) => {
                runtime.log(format!("[remote] mouse report failed: {err:#}\n"));
                return;
            }
        }
    }
}
