from __future__ import annotations

from dataclasses import dataclass
from concurrent.futures import ThreadPoolExecutor, as_completed
import ipaddress
import json
import socket
import subprocess
import time


DISCOVERY_PORT = 8766
DISCOVERY_MAGIC = "flow-keyboard-bridge"


@dataclass(frozen=True)
class DiscoveryInfo:
    host: str
    port: int


def encode_discovery(info: DiscoveryInfo) -> bytes:
    payload = {
        "type": DISCOVERY_MAGIC,
        "host": info.host,
        "port": info.port,
    }
    return json.dumps(payload, separators=(",", ":")).encode("utf-8")


def decode_discovery(payload: bytes) -> DiscoveryInfo:
    data = json.loads(payload.decode("utf-8"))
    if data.get("type") != DISCOVERY_MAGIC:
        raise ValueError(f"Unsupported discovery message: {data.get('type')}")
    host = data.get("host")
    port = data.get("port")
    if not isinstance(host, str) or not host:
        raise ValueError("Discovery host must be a non-empty string")
    if not isinstance(port, int):
        raise ValueError("Discovery port must be an integer")
    return DiscoveryInfo(host=host, port=port)


def guess_lan_ip() -> str:
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        try:
            sock.connect(("8.8.8.8", 80))
            return sock.getsockname()[0]
        except OSError:
            return socket.gethostbyname(socket.gethostname())


def guess_lan_network() -> ipaddress.IPv4Network:
    networks = guess_lan_networks()
    if not networks:
        raise OSError("No usable IPv4 LAN network found")
    return networks[0]


def guess_lan_networks() -> list[ipaddress.IPv4Network]:
    networks: list[ipaddress.IPv4Network] = []
    for ip_text in guess_lan_ips():
        ip = ipaddress.ip_address(ip_text)
        if isinstance(ip, ipaddress.IPv4Address) and is_usable_lan_ip(ip):
            network = ipaddress.ip_network(f"{ip}/24", strict=False)
            if network not in networks:
                networks.append(network)
    return networks


def guess_lan_ips() -> list[str]:
    ips: list[str] = []
    for candidate in _ips_from_ipconfig() + _ips_from_hostname() + [guess_lan_ip()]:
        try:
            ip = ipaddress.ip_address(candidate)
        except ValueError:
            continue
        if isinstance(ip, ipaddress.IPv4Address) and str(ip) not in ips:
            ips.append(str(ip))
    return ips


def _ips_from_hostname() -> list[str]:
    try:
        return socket.gethostbyname_ex(socket.gethostname())[2]
    except OSError:
        return []


def _ips_from_ipconfig() -> list[str]:
    try:
        result = subprocess.run(
            ["ipconfig"],
            capture_output=True,
            text=True,
            encoding="mbcs",
            errors="ignore",
            timeout=5,
        )
    except (OSError, subprocess.SubprocessError):
        return []
    return _extract_ipv4_addresses(result.stdout)


def _extract_ipv4_addresses(text: str) -> list[str]:
    ips: list[str] = []
    for line in text.splitlines():
        if "IPv4" not in line:
            continue
        _, _, value = line.partition(":")
        ip_text = value.strip()
        if ip_text and ip_text not in ips:
            ips.append(ip_text)
    return ips


def is_usable_lan_ip(ip: ipaddress.IPv4Address) -> bool:
    if ip.is_loopback or ip.is_link_local or ip.is_multicast or ip.is_unspecified:
        return False
    # 198.18.0.0/15 is commonly used by proxy/TUN software such as Mihomo.
    if ip in ipaddress.ip_network("198.18.0.0/15"):
        return False
    return ip.is_private


def broadcast_server(port: int, stop_event) -> None:
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
        while not stop_event.is_set():
            info = DiscoveryInfo(host=guess_lan_ip(), port=port)
            sock.sendto(encode_discovery(info), ("255.255.255.255", DISCOVERY_PORT))
            stop_event.wait(1)


def discover_server(timeout_seconds: float) -> DiscoveryInfo:
    deadline = time.monotonic() + timeout_seconds
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        sock.bind(("", DISCOVERY_PORT))
        sock.settimeout(0.5)
        while time.monotonic() < deadline:
            try:
                payload, address = sock.recvfrom(4096)
            except TimeoutError:
                continue
            info = decode_discovery(payload)
            if info.host in {"0.0.0.0", "127.0.0.1"}:
                return DiscoveryInfo(host=address[0], port=info.port)
            return info
    raise TimeoutError(f"No Flow Keyboard Bridge server found in {timeout_seconds:g} seconds")


def discover_server_by_scan(port: int, timeout_seconds: float = 0.25) -> DiscoveryInfo:
    networks = guess_lan_networks()
    if not networks:
        raise TimeoutError("No usable local network to scan")
    own_ips = set(guess_lan_ips())
    hosts = [str(host) for network in networks for host in network.hosts() if str(host) not in own_ips]

    def can_connect(host: str) -> DiscoveryInfo | None:
        try:
            with socket.create_connection((host, port), timeout=timeout_seconds) as sock:
                sock.settimeout(1)
                line = sock.makefile("rb").readline()
                if line:
                    return DiscoveryInfo(host=host, port=port)
        except OSError:
            return None
        return None

    with ThreadPoolExecutor(max_workers=64) as executor:
        futures = [executor.submit(can_connect, host) for host in hosts]
        for future in as_completed(futures):
            info = future.result()
            if info is not None:
                return info
    scanned = ", ".join(str(network) for network in networks)
    raise TimeoutError(f"No Flow Keyboard Bridge server found by scanning {scanned}")


def discover_server_auto(port: int, broadcast_timeout: float) -> DiscoveryInfo:
    try:
        return discover_server(broadcast_timeout)
    except TimeoutError as broadcast_error:
        print(f"[client] broadcast discovery failed: {broadcast_error}")
        print("[client] scanning local network ...")
        return discover_server_by_scan(port)
