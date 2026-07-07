from flow_keyboard_bridge.discovery import (
    DiscoveryInfo,
    decode_discovery,
    encode_discovery,
    guess_lan_network,
    is_usable_lan_ip,
)
import ipaddress


def test_discovery_message_round_trips():
    info = DiscoveryInfo(host="192.168.31.18", port=8765)

    payload = encode_discovery(info)

    assert decode_discovery(payload) == info


def test_decode_rejects_wrong_discovery_type():
    try:
        decode_discovery(b'{"type":"other"}')
    except ValueError as exc:
        assert "Unsupported discovery message" in str(exc)
    else:
        raise AssertionError("decode_discovery should reject wrong message type")


def test_guess_lan_network_returns_ipv4_network():
    network = guess_lan_network()

    assert network.version == 4


def test_proxy_tun_network_is_not_treated_as_lan():
    assert is_usable_lan_ip(ipaddress.ip_address("198.18.0.1")) is False


def test_private_home_network_is_treated_as_lan():
    assert is_usable_lan_ip(ipaddress.ip_address("192.168.31.54")) is True
