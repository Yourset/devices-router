use crate::protocol::{
    decode_discovery, decode_event, encode_discovery, encode_event, BridgeEvent, ClientRole,
};
use anyhow::{Context, Result};
use local_ip_address::list_afinet_netifas;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

pub const DISCOVERY_PORT: u16 = 8766;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredHost {
    pub host: IpAddr,
    pub port: u16,
}

pub fn broadcast_host(stop: impl Fn() -> bool + Send + 'static, tcp_port: u16) -> Result<()> {
    let socket =
        UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).context("bind discovery broadcaster")?;
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
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DISCOVERY_PORT))
        .context("bind discovery listener")?;
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
                    if is_ignored_ipv4(source.ip()) {
                        continue;
                    }
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
    anyhow::bail!(
        "No Devices Router host found in {} seconds",
        timeout.as_secs()
    )
}

pub fn scan_local_network(tcp_port: u16, timeout_per_host: Duration) -> Option<DiscoveredHost> {
    for local in local_ipv4_candidates() {
        if let Some(host) = scan_subnet(local, tcp_port, timeout_per_host) {
            return Some(host);
        }
    }
    None
}

fn scan_subnet(
    local: Ipv4Addr,
    tcp_port: u16,
    timeout_per_host: Duration,
) -> Option<DiscoveredHost> {
    let hosts: VecDeque<Ipv4Addr> = same_subnet_hosts(local)
        .filter(|host| *host != local && !is_ignored_ipv4(IpAddr::V4(*host)))
        .collect();
    if hosts.is_empty() {
        return None;
    }

    let worker_count = hosts.len().min(48);
    let queue = Arc::new(Mutex::new(hosts));
    let found = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let mut handles = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        let queue = Arc::clone(&queue);
        let found = Arc::clone(&found);
        let tx = tx.clone();
        handles.push(thread::spawn(move || loop {
            if found.load(Ordering::Relaxed) {
                break;
            }
            let Some(host) = queue.lock().ok().and_then(|mut hosts| hosts.pop_front()) else {
                break;
            };
            let address = SocketAddr::from((host, tcp_port));
            if verifies_devices_router_host(address, timeout_per_host) {
                found.store(true, Ordering::Relaxed);
                let _ = tx.send(host);
                break;
            }
        }));
    }

    drop(tx);
    for handle in handles {
        let _ = handle.join();
    }
    rx.try_recv().ok().map(|host| DiscoveredHost {
        host: IpAddr::V4(host),
        port: tcp_port,
    })
}

fn verifies_devices_router_host(address: SocketAddr, timeout: Duration) -> bool {
    let Ok(mut stream) = TcpStream::connect_timeout(&address, timeout) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let Ok(hello) = encode_event(&BridgeEvent::ClientHello {
        role: ClientRole::Remote,
        device_id: None,
        device_name: None,
        capabilities: vec!["discovery_probe".to_string()],
    }) else {
        return false;
    };
    if stream.write_all(&hello).is_err() {
        return false;
    }
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return false;
    }
    matches!(decode_event(line.as_bytes()), Ok(BridgeEvent::Ping { .. }))
}

fn is_ignored_ipv4(ip: IpAddr) -> bool {
    let IpAddr::V4(ip) = ip else {
        return true;
    };
    let [a, b, _, _] = ip.octets();
    ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || a == 0
        || (a == 198 && (b == 18 || b == 19))
}

fn local_ipv4_candidates() -> Vec<Ipv4Addr> {
    let mut candidates = Vec::new();
    if let Ok(interfaces) = list_afinet_netifas() {
        for (_, ip) in interfaces {
            if let IpAddr::V4(ip) = ip {
                push_candidate(&mut candidates, ip);
            }
        }
    }
    if let Some(primary) = primary_local_ipv4() {
        push_candidate(&mut candidates, primary);
    }
    candidates
}

pub fn local_ipv4_addresses() -> Vec<Ipv4Addr> {
    local_ipv4_candidates()
}

fn push_candidate(candidates: &mut Vec<Ipv4Addr>, ip: Ipv4Addr) {
    if !is_ignored_ipv4(IpAddr::V4(ip)) && !candidates.contains(&ip) {
        candidates.push(ip);
    }
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
        assert_eq!(
            hosts.last().copied(),
            Some(Ipv4Addr::new(192, 168, 31, 254))
        );
        assert!(hosts.contains(&Ipv4Addr::new(192, 168, 31, 18)));
    }

    #[test]
    fn ignores_clash_tun_range() {
        assert!(is_ignored_ipv4(IpAddr::from([198, 18, 0, 2])));
        assert!(is_ignored_ipv4(IpAddr::from([198, 19, 255, 254])));
        assert!(!is_ignored_ipv4(IpAddr::from([192, 168, 31, 18])));
    }

    #[test]
    fn ignores_non_lan_addresses() {
        assert!(is_ignored_ipv4(IpAddr::from([127, 0, 0, 1])));
        assert!(is_ignored_ipv4(IpAddr::from([169, 254, 1, 2])));
        assert!(is_ignored_ipv4(IpAddr::from([0, 0, 0, 0])));
        assert!(!is_ignored_ipv4(IpAddr::from([10, 0, 0, 8])));
    }

    #[test]
    fn candidate_list_deduplicates_and_filters() {
        let mut candidates = Vec::new();
        push_candidate(&mut candidates, Ipv4Addr::new(198, 18, 0, 2));
        push_candidate(&mut candidates, Ipv4Addr::new(192, 168, 31, 54));
        push_candidate(&mut candidates, Ipv4Addr::new(192, 168, 31, 54));

        assert_eq!(candidates, vec![Ipv4Addr::new(192, 168, 31, 54)]);
    }
}
