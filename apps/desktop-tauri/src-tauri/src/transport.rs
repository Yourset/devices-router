use std::net::TcpStream;
use std::time::{Duration, Instant};

pub(crate) const CONTROL_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(300);
pub(crate) const CONTROL_PROBE_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackgroundSendDue {
    Heartbeat,
    Probe,
}

pub(crate) fn configure_control_stream(stream: &TcpStream) -> std::io::Result<()> {
    stream.set_nodelay(true)
}

pub(crate) fn background_send_due(
    last_outbound: Instant,
    last_probe: Instant,
    now: Instant,
) -> Option<BackgroundSendDue> {
    if now.saturating_duration_since(last_probe) >= CONTROL_PROBE_INTERVAL {
        Some(BackgroundSendDue::Probe)
    } else if now.saturating_duration_since(last_outbound) >= CONTROL_HEARTBEAT_INTERVAL {
        Some(BackgroundSendDue::Heartbeat)
    } else {
        None
    }
}

pub(crate) fn background_send_due_for_mode(
    last_outbound: Instant,
    last_probe: Instant,
    now: Instant,
    probe_enabled: bool,
) -> Option<BackgroundSendDue> {
    if probe_enabled {
        background_send_due(last_outbound, last_probe, now)
    } else if now.saturating_duration_since(last_outbound) >= CONTROL_HEARTBEAT_INTERVAL {
        Some(BackgroundSendDue::Heartbeat)
    } else {
        None
    }
}

pub(crate) fn background_send_after_outbound(
    last_outbound: Instant,
    last_probe: Instant,
    now: Instant,
) -> Option<BackgroundSendDue> {
    match background_send_due(last_outbound, last_probe, now) {
        Some(BackgroundSendDue::Probe) => Some(BackgroundSendDue::Probe),
        _ => None,
    }
}

pub(crate) fn background_send_after_outbound_for_mode(
    last_outbound: Instant,
    last_probe: Instant,
    now: Instant,
    probe_enabled: bool,
) -> Option<BackgroundSendDue> {
    if probe_enabled {
        background_send_after_outbound(last_outbound, last_probe, now)
    } else {
        None
    }
}

pub(crate) fn background_send_wait(
    last_outbound: Instant,
    last_probe: Instant,
    now: Instant,
) -> Duration {
    let heartbeat_wait =
        CONTROL_HEARTBEAT_INTERVAL.saturating_sub(now.saturating_duration_since(last_outbound));
    let probe_wait =
        CONTROL_PROBE_INTERVAL.saturating_sub(now.saturating_duration_since(last_probe));
    heartbeat_wait.min(probe_wait)
}

pub(crate) fn background_send_wait_for_mode(
    last_outbound: Instant,
    last_probe: Instant,
    now: Instant,
    probe_enabled: bool,
) -> Duration {
    if probe_enabled {
        background_send_wait(last_outbound, last_probe, now)
    } else {
        CONTROL_HEARTBEAT_INTERVAL.saturating_sub(now.saturating_duration_since(last_outbound))
    }
}

#[cfg(test)]
mod tests {
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::{Duration, Instant};

    use super::*;

    #[test]
    fn configure_control_stream_enables_tcp_nodelay() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let accept_thread = thread::spawn(move || listener.accept().unwrap().0);

        let stream = TcpStream::connect(address).unwrap();
        configure_control_stream(&stream).unwrap();

        assert!(stream.nodelay().unwrap());

        drop(stream);
        drop(accept_thread.join().unwrap());
    }

    #[test]
    fn background_send_due_prioritizes_probe_then_heartbeat() {
        let now = Instant::now();

        assert_eq!(
            background_send_due(
                now - Duration::from_millis(500),
                now - Duration::from_millis(500),
                now,
            ),
            Some(BackgroundSendDue::Probe)
        );
        assert_eq!(
            background_send_due(
                now - Duration::from_millis(300),
                now - Duration::from_millis(499),
                now,
            ),
            Some(BackgroundSendDue::Heartbeat)
        );
        assert_eq!(
            background_send_due(
                now - Duration::from_millis(299),
                now - Duration::from_millis(499),
                now,
            ),
            None
        );
    }

    #[test]
    fn event_traffic_still_requires_probe_once_probe_budget_expires() {
        let start = Instant::now();
        let last_probe = start;

        for step in 1..=10 {
            let now = start + Duration::from_millis(step * 50);
            let due = background_send_after_outbound(now, last_probe, now);

            if step < 10 {
                assert_eq!(due, None, "probe should not fire early at step {step}");
            } else {
                assert_eq!(due, Some(BackgroundSendDue::Probe));
            }
        }
    }

    #[test]
    fn host_authoritative_mode_never_schedules_a_remote_probe() {
        let now = Instant::now();
        let old = now - Duration::from_secs(2);

        assert_eq!(
            background_send_due_for_mode(old, old, now, false),
            Some(BackgroundSendDue::Heartbeat)
        );
        assert_eq!(
            background_send_after_outbound_for_mode(now, old, now, false),
            None
        );
        assert_eq!(
            background_send_wait_for_mode(now, old, now, false),
            CONTROL_HEARTBEAT_INTERVAL
        );
    }
}
