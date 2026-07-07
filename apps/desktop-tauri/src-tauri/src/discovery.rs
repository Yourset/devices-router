use crate::protocol::{decode_discovery, encode_discovery};
use anyhow::{Context, Result};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

pub const DISCOVERY_PORT: u16 = 8766;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredHost {
    pub host: IpAddr,
    pub port: u16,
}

pub fn broadcast_host(stop: impl Fn() -> bool + Send + 'static, tcp_port: u16) -> Result<()> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).context("bind discovery broadcaster")?;
    socket
        .set_broadcast(true)
        .context("enable discovery broadcast")?;
    let payload = encode_discovery(tcp_port);
    let target = SocketAddr::from((Ipv4Addr::BROADCAST, DISCOVERY_PORT));
    while !stop() {
        let _ = socket.send_to(payload.as_bytes(), target);
        thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}

pub fn discover_host(timeout: Duration) -> Result<DiscoveredHost> {
    let socket =
        UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT)).context("bind discovery listener")?;
    socket
        .set_read_timeout(Some(Duration::from_millis(500)))
        .context("set discovery timeout")?;
    let deadline = Instant::now() + timeout;
    let mut buf = [0_u8; 256];
    while Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((len, source)) => {
                let payload = String::from_utf8_lossy(&buf[..len]);
                if let Some(port) = decode_discovery(&payload) {
                    return Ok(DiscoveredHost {
                        host: source.ip(),
                        port,
                    });
                }
            }
            Err(err)
                if err.kind() == std::io::ErrorKind::WouldBlock
                    || err.kind() == std::io::ErrorKind::TimedOut => {}
            Err(err) => return Err(err).context("receive discovery packet"),
        }
    }
    anyhow::bail!("No Devices Router host found in {} seconds", timeout.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovered_host_keeps_ip_and_port() {
        let host = DiscoveredHost {
            host: IpAddr::from([192, 168, 31, 18]),
            port: 8765,
        };

        assert_eq!(host.port, 8765);
        assert_eq!(host.host.to_string(), "192.168.31.18");
    }
}
