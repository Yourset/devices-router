use crate::app_state::AppRuntime;
use crate::discovery::broadcast_host;
use crate::input::release_local_modifiers;
use crate::keyboard_hook::{
    run_keyboard_hook, set_key_suppression, take_panic_request, RawKeyEvent,
};
use crate::mouse::cursor_position;
use crate::mouse_hook::set_mouse_input_suppression;
use crate::protocol::{
    decode_event, encode_event, BridgeEvent, KeyAction, MouseButton, MouseButtonAction, TargetSide,
};
use crate::routing::{ActivityArbiter, KeyboardTarget};
use crate::sessions::{RegisterResult, SessionIdentity, MAX_REMOTE_DEVICES};
use crate::updates::start_update_server;
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const TCP_PORT: u16 = 8765;
const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(300);
const LOOP_INTERVAL: Duration = Duration::from_millis(5);
const EMERGENCY_ACTIVITY_BLOCK: Duration = Duration::from_secs(1);

enum HostEvent {
    RemoteActivity {
        device_id: String,
        generation: u64,
    },
    TargetRequest {
        device_id: String,
        generation: u64,
        target: TargetSide,
    },
    Disconnected {
        device_id: String,
        generation: u64,
        reason: String,
    },
}

pub(crate) fn run(runtime: Arc<AppRuntime>) -> Result<()> {
    let (key_tx, key_rx) = mpsc::channel::<RawKeyEvent>();
    let keyboard_runtime = Arc::clone(&runtime);
    thread::Builder::new()
        .name("devices-router-keyboard-hook".to_string())
        .spawn(move || {
            if let Err(err) = run_keyboard_hook(key_tx) {
                keyboard_runtime.log(format!("[host] keyboard hook failed: {err:#}\n"));
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

    let update_runtime = Arc::clone(&runtime);
    let update_port = runtime.config().update_port;
    thread::Builder::new()
        .name("devices-router-update-server".to_string())
        .spawn(move || {
            if let Err(err) = start_update_server(update_runtime.clone(), update_port) {
                update_runtime.log(format!("[host] update server failed: {err:#}\n"));
            }
        })
        .context("spawn update server")?;

    let (host_event_tx, host_event_rx) = mpsc::channel::<HostEvent>();
    spawn_host_mouse_activity(runtime.clone(), host_event_tx.clone())?;

    let listener = TcpListener::bind(("0.0.0.0", TCP_PORT)).context("bind host TCP listener")?;
    listener
        .set_nonblocking(true)
        .context("set host listener nonblocking")?;
    runtime.log(format!(
        "[host] listening on 0.0.0.0:{TCP_PORT}; waiting for remote computers\n"
    ));

    let now = Instant::now();
    let mut arbiter = ActivityArbiter::ready(
        KeyboardTarget::Local,
        Duration::from_millis(runtime.config().mouse_follow.switch_debounce_ms),
        now,
    );
    let mut ctrl_down = false;
    let mut alt_down = false;
    let mut forwarded_key_logs = 0_u8;
    let mut emergency_block_until = now;

    while !runtime.should_stop() {
        loop {
            match listener.accept() {
                Ok((stream, address)) => {
                    spawn_host_connection(runtime.clone(), stream, address, host_event_tx.clone());
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(err) => return Err(err).context("accept host client"),
            }
        }

        if take_panic_request() {
            crate::core::force_local_release(
                &runtime,
                "[safety] Ctrl+Alt+Esc returned keyboard to host\n",
            );
            arbiter.force(KeyboardTarget::Local, Instant::now());
            emergency_block_until = Instant::now() + EMERGENCY_ACTIVITY_BLOCK;
        }

        while let Ok(event) = key_rx.try_recv() {
            handle_key_event(
                &runtime,
                &mut arbiter,
                event,
                &mut ctrl_down,
                &mut alt_down,
                &mut forwarded_key_logs,
            );
        }

        while let Ok(event) = host_event_rx.try_recv() {
            handle_host_event(&runtime, &mut arbiter, event, emergency_block_until);
        }

        if let Some(target) = arbiter.poll(Instant::now()) {
            switch_target(
                &runtime,
                target,
                "[host] keyboard followed latest mouse activity\n",
            );
        }
        thread::sleep(LOOP_INTERVAL);
    }

    switch_target(
        &runtime,
        KeyboardTarget::Local,
        "[host] stopped and returned keyboard to host\n",
    );
    Ok(())
}

fn spawn_host_mouse_activity(
    runtime: Arc<AppRuntime>,
    event_tx: mpsc::Sender<HostEvent>,
) -> Result<()> {
    thread::Builder::new()
        .name("devices-router-host-mouse".to_string())
        .spawn(move || {
            let Ok(mut last) = cursor_position() else {
                runtime.log("[host] keyboard target changed\n");
                return;
            };
            while !runtime.should_stop() {
                let config = runtime.config();
                thread::sleep(Duration::from_millis(
                    config.mouse_follow.host_poll_interval_ms,
                ));
                if !config.mouse_follow.enabled
                    || !config.mouse_follow.host_mouse_returns_local
                    || config.game_mode
                {
                    continue;
                }
                let Ok(current) = cursor_position() else {
                    continue;
                };
                if current != last {
                    last = current;
                    if event_tx
                        .send(HostEvent::TargetRequest {
                            device_id: String::new(),
                            generation: 0,
                            target: TargetSide::Local,
                        })
                        .is_err()
                    {
                        return;
                    }
                }
            }
        })
        .context("spawn host mouse activity thread")?;
    Ok(())
}

fn spawn_host_connection(
    runtime: Arc<AppRuntime>,
    stream: TcpStream,
    address: SocketAddr,
    event_tx: mpsc::Sender<HostEvent>,
) {
    let _ = thread::Builder::new()
        .name(format!("devices-router-host-client-{}", address.ip()))
        .spawn(move || {
            if let Err(err) = handle_host_connection(runtime.clone(), stream, address, event_tx) {
                runtime.log(format!("[host] client {address} failed: {err:#}\n"));
            }
        });
}

fn handle_host_connection(
    runtime: Arc<AppRuntime>,
    mut stream: TcpStream,
    address: SocketAddr,
    event_tx: mpsc::Sender<HostEvent>,
) -> Result<()> {
    stream
        .set_nonblocking(false)
        .context("set client stream blocking")?;
    stream
        .set_read_timeout(Some(Duration::from_millis(900)))
        .context("client handshake stream")?;
    let mut handshake_reader =
        BufReader::new(stream.try_clone().context("client handshake stream")?);
    let mut line = String::new();
    if handshake_reader.read_line(&mut line)? == 0 {
        return Ok(());
    }
    let BridgeEvent::ClientHello {
        device_id,
        device_name,
        capabilities,
        ..
    } = decode_event(line.as_bytes())?
    else {
        return Ok(());
    };

    if capabilities.iter().any(|value| value == "discovery_probe") {
        stream.write_all(&encode_event(&BridgeEvent::Ping {
            message: "ok".to_string(),
        })?)?;
        return Ok(());
    }

    let supports_server_hello = capabilities.iter().any(|value| value == "server_hello_v1");

    stream.set_read_timeout(None)?;
    let (outbound_tx, outbound_rx) = mpsc::channel::<BridgeEvent>();
    let registration = runtime.register_session(
        SessionIdentity {
            device_id,
            device_name,
            address: address.to_string(),
        },
        outbound_tx,
    );
    let acceptance = match registration {
        RegisterResult::Accepted(acceptance) => acceptance,
        RegisterResult::Rejected(reason) => {
            if supports_server_hello {
                stream.write_all(&encode_event(&BridgeEvent::ServerHello {
                    accepted: false,
                    reason: Some(reason.clone()),
                    max_devices: MAX_REMOTE_DEVICES as u8,
                })?)?;
            }
            runtime.log(format!("[host] rejected client {address}: {reason}\n"));
            return Ok(());
        }
    };
    let handshake_response = if supports_server_hello {
        BridgeEvent::ServerHello {
            accepted: true,
            reason: None,
            max_devices: MAX_REMOTE_DEVICES as u8,
        }
    } else {
        BridgeEvent::Ping {
            message: "ok".to_string(),
        }
    };
    stream.write_all(&encode_event(&handshake_response)?)?;

    let device_id = acceptance.device_id;
    let generation = acceptance.generation;
    runtime.log(format!(
        "[host] remote connected: {address}, device={device_id} {}\n",
        if acceptance.replaced {
            "replaced stale connection"
        } else {
            ""
        }
    ));

    let mut writer = stream.try_clone().context("clone host writer stream")?;
    let writer_events = event_tx.clone();
    let writer_device_id = device_id.clone();
    thread::Builder::new()
        .name(format!("devices-router-host-writer-{device_id}"))
        .spawn(move || loop {
            let event = match outbound_rx.recv_timeout(HEARTBEAT_INTERVAL) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => BridgeEvent::Ping {
                    message: "host-heartbeat".to_string(),
                },
                Err(RecvTimeoutError::Disconnected) => break,
            };
            let result = encode_event(&event).and_then(|bytes| {
                writer.write_all(&bytes)?;
                Ok(())
            });
            if let Err(err) = result {
                let _ = writer_events.send(HostEvent::Disconnected {
                    device_id: writer_device_id.clone(),
                    generation,
                    reason: format!("write failed: {err:#}"),
                });
                break;
            }
        })
        .context("spawn host writer thread")?;

    let mut reader = BufReader::new(stream);
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => match decode_event(line.as_bytes()) {
                Ok(BridgeEvent::MouseActivity { .. }) => {
                    if event_tx
                        .send(HostEvent::RemoteActivity {
                            device_id: device_id.clone(),
                            generation,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(BridgeEvent::TargetRequest { target }) => {
                    if event_tx
                        .send(HostEvent::TargetRequest {
                            device_id: device_id.clone(),
                            generation,
                            target,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(BridgeEvent::Ping { .. }) => {}
                Ok(other) => runtime.log(format!(
                    "[host] ignored message from {device_id}: {other:?}\n"
                )),
                Err(err) => runtime.log(format!(
                    "[host] invalid message from {device_id}: {err:#}\n"
                )),
            },
            Err(err) => {
                let _ = event_tx.send(HostEvent::Disconnected {
                    device_id: device_id.clone(),
                    generation,
                    reason: format!("read failed: {err}"),
                });
                return Ok(());
            }
        }
    }
    let _ = event_tx.send(HostEvent::Disconnected {
        device_id,
        generation,
        reason: "connection closed".to_string(),
    });
    Ok(())
}

fn handle_host_event(
    runtime: &Arc<AppRuntime>,
    arbiter: &mut ActivityArbiter,
    event: HostEvent,
    emergency_block_until: Instant,
) {
    match event {
        HostEvent::RemoteActivity {
            device_id,
            generation,
        } => {
            let now = Instant::now();
            if now < emergency_block_until
                || !runtime.session_generation_matches(&device_id, generation)
            {
                return;
            }
            let config = runtime.config();
            if !config.mouse_follow.enabled
                || !config.mouse_follow.remote_mouse_switches_remote
                || config.game_mode
            {
                return;
            }
            runtime.mark_session_activity(&device_id, now);
            if let Some(target) = arbiter.observe(KeyboardTarget::Device(device_id.clone()), now) {
                switch_target(
                    runtime,
                    target,
                    &format!("[host] mouse activity selected {device_id}\n"),
                );
            }
        }
        HostEvent::TargetRequest {
            device_id,
            generation,
            target,
        } => {
            if generation != 0 && !runtime.session_generation_matches(&device_id, generation) {
                return;
            }
            let now = Instant::now();
            let requested = requested_target(target, &device_id);
            arbiter.force(requested.clone(), now);
            switch_target(runtime, requested, "[host] keyboard target changed\n");
        }
        HostEvent::Disconnected {
            device_id,
            generation,
            reason,
        } => {
            if !runtime.remove_session(&device_id, generation) {
                return;
            }
            runtime.log(format!(
                "[host] remote disconnected: {device_id}; {reason}\n"
            ));
            let target = target_after_disconnect(&runtime.target(), &device_id);
            if target == KeyboardTarget::Local {
                arbiter.force(KeyboardTarget::Local, Instant::now());
                switch_target(
                    runtime,
                    KeyboardTarget::Local,
                    "[safety] active remote disconnected; keyboard returned to host\n",
                );
            } else {
                broadcast_target_states(runtime);
            }
        }
    }
}

fn handle_key_event(
    runtime: &Arc<AppRuntime>,
    arbiter: &mut ActivityArbiter,
    event: RawKeyEvent,
    ctrl_down: &mut bool,
    alt_down: &mut bool,
    forwarded_key_logs: &mut u8,
) {
    update_modifier_state(&event, ctrl_down, alt_down);
    if event.is_down && *ctrl_down && *alt_down {
        let target = match event.vk_code {
            0x31 => Some(KeyboardTarget::Local),
            0x32 => runtime.first_session_id().map(KeyboardTarget::Device),
            0x33 => {
                let mut ids = runtime
                    .session_senders()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>();
                ids.sort();
                ids.into_iter().nth(1).map(KeyboardTarget::Device)
            }
            _ => None,
        };
        if let Some(target) = target {
            arbiter.force(target.clone(), Instant::now());
            switch_target(runtime, target, "[host] keyboard target changed\n");
            return;
        }
    }

    let KeyboardTarget::Device(device_id) = runtime.target() else {
        return;
    };
    let payload = BridgeEvent::Key {
        action: if event.is_down {
            KeyAction::Down
        } else {
            KeyAction::Up
        },
        key: format!("<{}>", event.vk_code),
    };
    let sent = runtime
        .session_sender(&device_id)
        .is_some_and(|sender| sender.send(payload).is_ok());
    if !sent {
        arbiter.force(KeyboardTarget::Local, Instant::now());
        switch_target(
            runtime,
            KeyboardTarget::Local,
            "[safety] send failed; keyboard returned to host\n",
        );
    } else if *forwarded_key_logs < 5 {
        *forwarded_key_logs += 1;
        runtime.log(format!("[host] forwarded key to {device_id}\n"));
    }
}

pub(crate) fn switch_target(
    runtime: &Arc<AppRuntime>,
    requested: KeyboardTarget,
    log_line: &str,
) -> bool {
    let target = match requested {
        KeyboardTarget::Remote => runtime
            .first_session_id()
            .map(KeyboardTarget::Device)
            .unwrap_or(KeyboardTarget::Local),
        KeyboardTarget::Device(device_id) if runtime.session_sender(&device_id).is_none() => {
            KeyboardTarget::Local
        }
        other => other,
    };
    let previous = runtime.target();
    if previous == target {
        return false;
    }
    if let KeyboardTarget::Device(device_id) = &previous {
        release_remote_inputs(runtime, device_id);
    }
    if target.is_remote() {
        if let Err(err) = release_local_modifiers() {
            runtime.log(format!(
                "[host] failed to release local modifiers: {err:#}\n"
            ));
        }
    }
    runtime.set_target(target.clone());
    if target == KeyboardTarget::Local {
        runtime.mark_local_release();
    }
    set_key_suppression(target.is_remote());
    set_mouse_input_suppression(false);
    broadcast_target_states(runtime);
    runtime.log(log_line);
    true
}

fn broadcast_target_states(runtime: &Arc<AppRuntime>) {
    let target = runtime.target();
    for (device_id, sender) in runtime.session_senders() {
        let _ = sender.send(BridgeEvent::TargetState {
            target: target_state_for_device(&target, &device_id),
        });
    }
}

fn target_state_for_device(target: &KeyboardTarget, device_id: &str) -> TargetSide {
    if target.device_id() == Some(device_id) {
        TargetSide::Remote
    } else {
        TargetSide::Local
    }
}

fn release_remote_inputs(runtime: &Arc<AppRuntime>, device_id: &str) {
    let Some(sender) = runtime.session_sender(device_id) else {
        return;
    };
    for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
        let _ = sender.send(BridgeEvent::MouseInput {
            event: crate::protocol::MouseInputEvent::Button {
                button,
                action: MouseButtonAction::Up,
            },
        });
    }
    for vk_code in [0x10_u32, 0x11, 0x12, 0x5B, 0x5C] {
        let _ = sender.send(BridgeEvent::Key {
            action: KeyAction::Up,
            key: format!("<{vk_code}>"),
        });
    }
}

fn update_modifier_state(event: &RawKeyEvent, ctrl_down: &mut bool, alt_down: &mut bool) {
    match event.vk_code {
        0x11 | 0xA2 | 0xA3 => *ctrl_down = event.is_down,
        0x12 | 0xA4 | 0xA5 => *alt_down = event.is_down,
        _ => {}
    }
}

fn requested_target(target: TargetSide, device_id: &str) -> KeyboardTarget {
    match target {
        TargetSide::Local => KeyboardTarget::Local,
        TargetSide::Remote => KeyboardTarget::Device(device_id.to_string()),
    }
}

fn target_after_disconnect(current: &KeyboardTarget, device_id: &str) -> KeyboardTarget {
    if current.device_id() == Some(device_id) {
        KeyboardTarget::Local
    } else {
        current.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::TargetSide;
    use crate::routing::KeyboardTarget;

    fn connect_test_client(address: SocketAddr, device_id: &str) -> (TcpStream, BridgeEvent) {
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .write_all(
                &encode_event(&BridgeEvent::ClientHello {
                    role: crate::protocol::ClientRole::Remote,
                    device_id: Some(device_id.to_string()),
                    device_name: Some(device_id.to_string()),
                    capabilities: vec![
                        "multi_remote_v1".to_string(),
                        "server_hello_v1".to_string(),
                    ],
                })
                .unwrap(),
            )
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        (stream, decode_event(line.as_bytes()).unwrap())
    }

    #[test]
    fn loopback_host_accepts_two_clients_and_rejects_third() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let state = crate::app_state::SharedState::new("test");
        let runtime = state.runtime();
        let (event_tx, _event_rx) = mpsc::channel();
        let accept_runtime = runtime.clone();
        let accept_thread = thread::spawn(move || {
            for _ in 0..3 {
                let (stream, peer) = listener.accept().unwrap();
                let handler_runtime = accept_runtime.clone();
                let handler_tx = event_tx.clone();
                thread::spawn(move || {
                    handle_host_connection(handler_runtime, stream, peer, handler_tx).unwrap();
                });
            }
        });

        let (client_a, hello_a) = connect_test_client(address, "device-a");
        let (client_b, hello_b) = connect_test_client(address, "device-b");
        let (client_c, hello_c) = connect_test_client(address, "device-c");

        assert!(matches!(
            hello_a,
            BridgeEvent::ServerHello {
                accepted: true,
                max_devices: 2,
                ..
            }
        ));
        assert!(matches!(
            hello_b,
            BridgeEvent::ServerHello {
                accepted: true,
                max_devices: 2,
                ..
            }
        ));
        assert!(matches!(
            hello_c,
            BridgeEvent::ServerHello {
                accepted: false,
                max_devices: 2,
                ..
            }
        ));
        assert_eq!(state.snapshot().devices.len(), 2);

        drop((client_a, client_b, client_c));
        accept_thread.join().unwrap();
    }

    #[test]
    fn loopback_host_keeps_connection_alive_after_handshake() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let state = crate::app_state::SharedState::new("test");
        let runtime = state.runtime();
        runtime.start(crate::app_state::AppMode::Host);
        let (event_tx, _event_rx) = mpsc::channel();
        let handler = thread::spawn(move || {
            let (stream, peer) = loop {
                match listener.accept() {
                    Ok(connection) => break connection,
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(err) => panic!("accept failed: {err}"),
                }
            };
            thread::sleep(Duration::from_millis(20));
            stream.set_nonblocking(true).unwrap();
            handle_host_connection(runtime, stream, peer, event_tx).unwrap();
        });

        let (stream, hello) = connect_test_client(address, "persistent-device");
        assert!(matches!(
            hello,
            BridgeEvent::ServerHello { accepted: true, .. }
        ));
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        let bytes = reader.read_line(&mut line).unwrap();

        assert!(
            bytes > 0,
            "host closed the connection immediately after handshake"
        );
        assert!(matches!(
            decode_event(line.as_bytes()).unwrap(),
            BridgeEvent::Ping { .. }
        ));

        drop(reader);
        drop(stream);
        handler.join().unwrap();
    }

    #[test]
    fn loopback_host_replies_with_legacy_ping_before_old_client_updates() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let state = crate::app_state::SharedState::new("test");
        let runtime = state.runtime();
        let (event_tx, _event_rx) = mpsc::channel();
        let handler = thread::spawn(move || {
            let (stream, peer) = listener.accept().unwrap();
            handle_host_connection(runtime, stream, peer, event_tx).unwrap();
        });

        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .write_all(
                &encode_event(&BridgeEvent::ClientHello {
                    role: crate::protocol::ClientRole::Remote,
                    device_id: None,
                    device_name: None,
                    capabilities: Vec::new(),
                })
                .unwrap(),
            )
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();

        assert!(matches!(
            decode_event(line.as_bytes()).unwrap(),
            BridgeEvent::Ping { .. }
        ));

        drop(reader);
        drop(stream);
        handler.join().unwrap();
    }

    #[test]
    fn remote_request_targets_the_requesting_device() {
        assert_eq!(
            requested_target(TargetSide::Remote, "device-a"),
            KeyboardTarget::Device("device-a".to_string())
        );
        assert_eq!(
            requested_target(TargetSide::Local, "device-a"),
            KeyboardTarget::Local
        );
    }

    #[test]
    fn disconnecting_active_device_falls_back_to_local() {
        assert_eq!(
            target_after_disconnect(&KeyboardTarget::Device("device-a".to_string()), "device-a"),
            KeyboardTarget::Local
        );
    }

    #[test]
    fn disconnecting_inactive_device_keeps_current_target() {
        let current = KeyboardTarget::Device("device-b".to_string());

        assert_eq!(target_after_disconnect(&current, "device-a"), current);
    }
}
