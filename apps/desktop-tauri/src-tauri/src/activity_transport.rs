use crate::app_state::AppRuntime;
use crate::discovery::DISCOVERY_PORT;
use crate::protocol::{
    decode_activity_datagram, decode_discovery, encode_activity_datagram, ActivityDatagram,
    BridgeEvent, MouseSource,
};
use crate::routing::KeyboardTarget;
use anyhow::{Context, Result};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

pub const UDP_ACTIVITY_CAPABILITY: &str = "udp_activity_v1";
pub const TCP_FALLBACK_DELAY: Duration = Duration::from_millis(60);
const HOST_ACTIVITY_BIND_RETRY: Duration = Duration::from_millis(500);
const HOST_ACTIVITY_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActivityDatagramOutcome {
    HelloAccepted {
        device_id: String,
        generation: u64,
    },
    ActivityAccepted {
        device_id: String,
        generation: u64,
        activity_id: u64,
    },
}

#[derive(Clone, Debug)]
enum RemoteActivityCommand {
    SetReady(bool),
    Activity {
        activity_id: u64,
        target_epoch: Option<u64>,
        target_was_remote: bool,
    },
}

#[derive(Clone)]
pub struct RemoteActivitySender {
    tx: mpsc::Sender<RemoteActivityCommand>,
}

impl RemoteActivitySender {
    pub fn set_ready(&self, ready: bool) -> bool {
        self.tx.send(RemoteActivityCommand::SetReady(ready)).is_ok()
    }

    pub fn send_activity(
        &self,
        activity_id: u64,
        target_epoch: Option<u64>,
        target_was_remote: bool,
    ) -> bool {
        self.tx
            .send(RemoteActivityCommand::Activity {
                activity_id,
                target_epoch,
                target_was_remote,
            })
            .is_ok()
    }
}

struct PendingActivity {
    started_at: Instant,
    retry_index: usize,
    payload: Vec<u8>,
    activity_id: u64,
    target_epoch: Option<u64>,
    target_was_remote: bool,
    tcp_fallback_sent: bool,
}

pub fn activity_retry_offsets() -> [Duration; 3] {
    [
        Duration::from_millis(0),
        Duration::from_millis(5),
        Duration::from_millis(15),
    ]
}

pub fn should_send_tcp_fallback(
    current_target: &KeyboardTarget,
    initial_epoch: Option<u64>,
    current_epoch: Option<u64>,
) -> bool {
    !current_target.is_remote() && initial_epoch == current_epoch
}

pub fn process_host_activity_datagram(
    runtime: &Arc<AppRuntime>,
    source: SocketAddr,
    payload: &[u8],
) -> Option<ActivityDatagramOutcome> {
    if std::str::from_utf8(payload)
        .ok()
        .and_then(decode_discovery)
        .is_some()
    {
        return None;
    }
    let source_ip = source.ip().to_string();
    match decode_activity_datagram(payload).ok()? {
        ActivityDatagram::Hello {
            device_id,
            activity_token,
        } => runtime
            .validate_session_activity_hello(&device_id, &activity_token, &source_ip)
            .map(|generation| ActivityDatagramOutcome::HelloAccepted {
                device_id,
                generation,
            }),
        ActivityDatagram::Activity {
            device_id,
            activity_token,
            activity_id,
        } => runtime
            .validate_session_activity(&device_id, &activity_token, &source_ip, activity_id)
            .map(|generation| ActivityDatagramOutcome::ActivityAccepted {
                device_id,
                generation,
                activity_id,
            }),
    }
}

pub fn spawn_host_activity_listener(
    runtime: Arc<AppRuntime>,
    on_outcome: impl Fn(ActivityDatagramOutcome) + Send + 'static,
) -> Result<()> {
    let run_generation = runtime.run_generation();
    let bind_deadline = Instant::now() + HOST_ACTIVITY_BIND_RETRY;
    let socket = loop {
        match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT)) {
            Ok(socket) => break socket,
            Err(err)
                if err.kind() == std::io::ErrorKind::AddrInUse
                    && runtime.run_generation_is_active(run_generation)
                    && Instant::now() < bind_deadline =>
            {
                thread::sleep(HOST_ACTIVITY_POLL_INTERVAL);
            }
            Err(err) => return Err(err).context("bind host activity listener"),
        }
    };
    socket
        .set_read_timeout(Some(HOST_ACTIVITY_POLL_INTERVAL))
        .context("set host activity listener timeout")?;
    thread::Builder::new()
        .name("devices-router-host-activity".to_string())
        .spawn(move || {
            let mut buf = [0_u8; 512];
            while runtime.run_generation_is_active(run_generation) {
                match socket.recv_from(&mut buf) {
                    Ok((len, source)) => {
                        if let Some(outcome) =
                            process_host_activity_datagram(&runtime, source, &buf[..len])
                        {
                            on_outcome(outcome);
                        }
                    }
                    Err(err)
                        if err.kind() == std::io::ErrorKind::WouldBlock
                            || err.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(_) => break,
                }
            }
        })
        .context("spawn host activity listener")?;
    Ok(())
}

pub fn spawn_remote_activity_sender(
    runtime: Arc<AppRuntime>,
    host_ip: IpAddr,
    device_id: String,
    activity_token: String,
    activity_port: u16,
    control_tx: mpsc::Sender<BridgeEvent>,
) -> Result<RemoteActivitySender> {
    let bind_addr = match host_ip {
        IpAddr::V4(_) => SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
        IpAddr::V6(_) => SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0)),
    };
    let socket = UdpSocket::bind(bind_addr).context("bind remote activity socket")?;
    socket
        .connect(SocketAddr::new(host_ip, activity_port))
        .context("connect remote activity socket")?;
    let hello = encode_activity_datagram(&ActivityDatagram::Hello {
        device_id: device_id.clone(),
        activity_token: activity_token.clone(),
    })?;
    socket.send(&hello).context("send remote activity hello")?;
    let (tx, rx) = mpsc::channel::<RemoteActivityCommand>();
    thread::Builder::new()
        .name("devices-router-remote-activity".to_string())
        .spawn(move || {
            run_remote_activity_worker(runtime, socket, device_id, activity_token, control_tx, rx)
        })
        .context("spawn remote activity sender")?;
    Ok(RemoteActivitySender { tx })
}

pub fn queue_remote_mouse_activity(
    sender: Option<&RemoteActivitySender>,
    control_tx: &mpsc::Sender<BridgeEvent>,
    activity_id: u64,
    target_epoch: Option<u64>,
    target_was_remote: bool,
) -> bool {
    if let Some(sender) = sender {
        return sender.send_activity(activity_id, target_epoch, target_was_remote);
    }
    send_tcp_mouse_activity(control_tx, activity_id, target_epoch)
}

fn run_remote_activity_worker(
    runtime: Arc<AppRuntime>,
    socket: UdpSocket,
    device_id: String,
    activity_token: String,
    control_tx: mpsc::Sender<BridgeEvent>,
    rx: mpsc::Receiver<RemoteActivityCommand>,
) {
    let mut ready = false;
    let mut pending = Vec::<PendingActivity>::new();
    loop {
        let timeout = next_pending_wait(&pending);
        match timeout {
            Some(wait) => match rx.recv_timeout(wait) {
                Ok(command) => handle_remote_command(
                    &mut ready,
                    &mut pending,
                    command,
                    &device_id,
                    &activity_token,
                    &control_tx,
                ),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            },
            None => match rx.recv() {
                Ok(command) => handle_remote_command(
                    &mut ready,
                    &mut pending,
                    command,
                    &device_id,
                    &activity_token,
                    &control_tx,
                ),
                Err(_) => break,
            },
        }
        flush_pending(&runtime, &socket, &control_tx, &mut pending, &mut ready);
    }
}

fn handle_remote_command(
    ready: &mut bool,
    pending: &mut Vec<PendingActivity>,
    command: RemoteActivityCommand,
    device_id: &str,
    activity_token: &str,
    control_tx: &mpsc::Sender<BridgeEvent>,
) {
    match command {
        RemoteActivityCommand::SetReady(next) => *ready = next,
        RemoteActivityCommand::Activity {
            activity_id,
            target_epoch,
            target_was_remote,
        } => {
            if !*ready {
                let _ = send_tcp_mouse_activity(control_tx, activity_id, target_epoch);
                return;
            }
            let Ok(payload) = encode_activity_datagram(&ActivityDatagram::Activity {
                device_id: device_id.to_string(),
                activity_token: activity_token.to_string(),
                activity_id,
            }) else {
                let _ = send_tcp_mouse_activity(control_tx, activity_id, target_epoch);
                return;
            };
            pending.push(PendingActivity {
                started_at: Instant::now(),
                retry_index: 0,
                payload,
                activity_id,
                target_epoch,
                target_was_remote,
                tcp_fallback_sent: false,
            });
        }
    }
}

fn next_pending_wait(pending: &[PendingActivity]) -> Option<Duration> {
    let now = Instant::now();
    pending
        .iter()
        .filter_map(|activity| {
            let retry_due = activity
                .next_retry_due()
                .map(|due| due.saturating_duration_since(now));
            let fallback_due = activity
                .fallback_due()
                .map(|due| due.saturating_duration_since(now));
            match (retry_due, fallback_due) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (Some(left), None) => Some(left),
                (None, Some(right)) => Some(right),
                (None, None) => None,
            }
        })
        .min()
}

fn flush_pending(
    runtime: &Arc<AppRuntime>,
    socket: &UdpSocket,
    control_tx: &mpsc::Sender<BridgeEvent>,
    pending: &mut Vec<PendingActivity>,
    ready: &mut bool,
) {
    let now = Instant::now();
    for activity in pending.iter_mut() {
        while let Some(due) = activity.next_retry_due() {
            if due > now {
                break;
            }
            if socket.send(&activity.payload).is_err() {
                let _ = send_tcp_mouse_activity(
                    control_tx,
                    activity.activity_id,
                    activity.target_epoch,
                );
                activity.tcp_fallback_sent = true;
                activity.retry_index = activity_retry_offsets().len();
                *ready = false;
                runtime.set_activity_transport(false);
                break;
            }
            activity.retry_index += 1;
        }
        if activity.should_attempt_fallback(now)
            && should_send_tcp_fallback(
                &runtime.target(),
                activity.target_epoch,
                runtime.observed_host_target_epoch(),
            )
        {
            let _ =
                send_tcp_mouse_activity(control_tx, activity.activity_id, activity.target_epoch);
            activity.tcp_fallback_sent = true;
            *ready = false;
            runtime.set_activity_transport(false);
        }
    }
    pending.retain(|activity| !activity.is_finished(now));
}

fn send_tcp_mouse_activity(
    control_tx: &mpsc::Sender<BridgeEvent>,
    activity_id: u64,
    target_epoch: Option<u64>,
) -> bool {
    control_tx
        .send(BridgeEvent::MouseActivity {
            source: MouseSource::Remote,
            activity_id: Some(activity_id),
            target_epoch,
        })
        .is_ok()
}

impl PendingActivity {
    fn next_retry_due(&self) -> Option<Instant> {
        activity_retry_offsets()
            .get(self.retry_index)
            .copied()
            .map(|offset| self.started_at + offset)
    }

    fn fallback_due(&self) -> Option<Instant> {
        (!self.target_was_remote && !self.tcp_fallback_sent)
            .then_some(self.started_at + TCP_FALLBACK_DELAY)
    }

    fn should_attempt_fallback(&self, now: Instant) -> bool {
        self.retry_index >= activity_retry_offsets().len()
            && self
                .fallback_due()
                .is_some_and(|fallback_due| fallback_due <= now)
    }

    fn is_finished(&self, now: Instant) -> bool {
        self.retry_index >= activity_retry_offsets().len()
            && (self.target_was_remote
                || self.tcp_fallback_sent
                || self
                    .fallback_due()
                    .is_some_and(|fallback_due| fallback_due <= now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_state::{AppMode, SharedState};
    use crate::sessions::SessionIdentity;

    #[test]
    fn retry_schedule_is_0_5_15_ms() {
        assert_eq!(
            activity_retry_offsets(),
            [
                Duration::from_millis(0),
                Duration::from_millis(5),
                Duration::from_millis(15),
            ]
        );
    }

    #[test]
    fn fallback_requires_same_epoch_and_no_remote_target() {
        assert!(should_send_tcp_fallback(
            &KeyboardTarget::Local,
            Some(3),
            Some(3)
        ));
        assert!(!should_send_tcp_fallback(
            &KeyboardTarget::Device("device-a".to_string()),
            Some(3),
            Some(3)
        ));
        assert!(!should_send_tcp_fallback(
            &KeyboardTarget::Local,
            Some(3),
            Some(4)
        ));
    }

    #[test]
    fn host_udp_loopback_accepts_hello_and_activity() {
        let state = SharedState::new("test");
        let runtime = state.runtime();
        let (session_tx, _session_rx) = mpsc::channel();
        let accepted = runtime
            .register_session_with_activity_support(
                SessionIdentity {
                    device_id: Some("device-a".to_string()),
                    device_name: Some("Windows-A".to_string()),
                    address: "127.0.0.1:8765".to_string(),
                },
                session_tx,
                true,
            )
            .accepted()
            .unwrap();
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let hello = encode_activity_datagram(&ActivityDatagram::Hello {
            device_id: accepted.device_id.clone(),
            activity_token: accepted.activity_token.clone().unwrap(),
        })
        .unwrap();
        sender
            .send_to(&hello, socket.local_addr().unwrap())
            .unwrap();
        let mut buf = [0_u8; 512];
        let (len, source) = socket.recv_from(&mut buf).unwrap();
        let hello_outcome = process_host_activity_datagram(&runtime, source, &buf[..len]).unwrap();
        assert_eq!(
            hello_outcome,
            ActivityDatagramOutcome::HelloAccepted {
                device_id: accepted.device_id.clone(),
                generation: accepted.generation,
            }
        );

        let activity = encode_activity_datagram(&ActivityDatagram::Activity {
            device_id: accepted.device_id.clone(),
            activity_token: accepted.activity_token.unwrap(),
            activity_id: 1,
        })
        .unwrap();
        sender
            .send_to(&activity, socket.local_addr().unwrap())
            .unwrap();
        let (len, source) = socket.recv_from(&mut buf).unwrap();
        let activity_outcome =
            process_host_activity_datagram(&runtime, source, &buf[..len]).unwrap();
        assert_eq!(
            activity_outcome,
            ActivityDatagramOutcome::ActivityAccepted {
                device_id: accepted.device_id,
                generation: accepted.generation,
                activity_id: 1,
            }
        );
    }

    #[test]
    fn remote_sender_retries_udp_and_falls_back_after_60ms() {
        let state = SharedState::new("test");
        let runtime = state.runtime();
        runtime.start(AppMode::Remote);
        runtime.apply_remote_target_state(KeyboardTarget::Local, Some(5));
        let listener = UdpSocket::bind("127.0.0.1:0").unwrap();
        listener
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let (control_tx, control_rx) = mpsc::channel();
        let sender = spawn_remote_activity_sender(
            runtime.clone(),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            "device-a".to_string(),
            "token-123".to_string(),
            listener.local_addr().unwrap().port(),
            control_tx,
        )
        .unwrap();

        let mut buf = [0_u8; 512];
        let (hello_len, _) = listener.recv_from(&mut buf).unwrap();
        assert!(matches!(
            decode_activity_datagram(&buf[..hello_len]).unwrap(),
            ActivityDatagram::Hello { .. }
        ));

        assert!(sender.set_ready(true));
        runtime.set_activity_transport(true);
        let started = Instant::now();
        assert!(sender.send_activity(7, Some(5), false));

        let mut seen_at = Vec::new();
        for _ in 0..3 {
            let (len, _) = listener.recv_from(&mut buf).unwrap();
            assert!(matches!(
                decode_activity_datagram(&buf[..len]).unwrap(),
                ActivityDatagram::Activity { activity_id: 7, .. }
            ));
            seen_at.push(started.elapsed());
        }
        assert!(seen_at[0] < Duration::from_millis(10));
        assert!(seen_at[1] >= Duration::from_millis(3));
        assert!(seen_at[2] >= Duration::from_millis(10));

        let fallback = control_rx.recv_timeout(Duration::from_millis(120)).unwrap();
        assert_eq!(
            fallback,
            BridgeEvent::MouseActivity {
                source: MouseSource::Remote,
                activity_id: Some(7),
                target_epoch: Some(5),
            }
        );
        assert_eq!(state.snapshot().activity_transport, "tcp");
    }

    #[test]
    fn queue_without_udp_sender_falls_back_to_tcp_control_channel() {
        let (_unused_tx, _unused_rx) = mpsc::channel::<RemoteActivityCommand>();
        let (control_tx, control_rx) = mpsc::channel();

        assert!(queue_remote_mouse_activity(
            None,
            &control_tx,
            9,
            Some(4),
            false,
        ));
        assert_eq!(
            control_rx.recv_timeout(Duration::from_millis(20)).unwrap(),
            BridgeEvent::MouseActivity {
                source: MouseSource::Remote,
                activity_id: Some(9),
                target_epoch: Some(4),
            }
        );
    }
}
