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
}
