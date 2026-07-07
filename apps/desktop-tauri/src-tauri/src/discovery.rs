use crate::protocol::{decode_discovery, encode_discovery};
use anyhow::{Context, Result};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
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

pub fn scan_local_network(tcp_port: u16, timeout_per_host: Duration) -> Option<DiscoveredHost> {
    let local = primary_local_ipv4()?;
    for host in same_subnet_hosts(local) {
        if host == local {
            continue;
        }
        let address = SocketAddr::from((host, tcp_port));
        if TcpStream::connect_timeout(&address, timeout_per_host).is_ok() {
            return Some(DiscoveredHost {
                host: IpAddr::V4(host),
                port: tcp_port,
            });
        }
    }
    None
}

fn primary_local_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(8, 8, 8, 8), 80)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Some(ip),
        _ => None,
    }
}

fn same_subnet_hosts(local: Ipv4Addr) -> impl Iterator<Item = Ipv4Addr> {
    let [a, b, c, _] = local.octets();
    (1_u8..=254).map(move |last| Ipv4Addr::new(a, b, c, last))
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

    #[test]
    fn same_subnet_hosts_contains_expected_range() {
        let hosts: Vec<Ipv4Addr> = same_subnet_hosts(Ipv4Addr::new(192, 168, 31, 54)).collect();

        assert_eq!(hosts.first().copied(), Some(Ipv4Addr::new(192, 168, 31, 1)));
        assert_eq!(hosts.last().copied(), Some(Ipv4Addr::new(192, 168, 31, 254)));
        assert!(hosts.contains(&Ipv4Addr::new(192, 168, 31, 18)));
    }
}
