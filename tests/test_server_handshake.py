import socket

from flow_keyboard_bridge.protocol import ClientHelloEvent, encode_message
from flow_keyboard_bridge.server import KeyboardBridgeServer


def test_invalid_connection_does_not_replace_existing_client():
    bridge = KeyboardBridgeServer("127.0.0.1", 8765)
    existing_server, existing_client = socket.socketpair()
    invalid_server, invalid_client = socket.socketpair()
    try:
        bridge.client = existing_server
        invalid_client.sendall(b"\n")
        invalid_client.shutdown(socket.SHUT_WR)

        accepted = bridge._accept_client_if_valid(invalid_server, ("127.0.0.1", 1234))

        assert accepted is False
        assert bridge.client is existing_server
    finally:
        existing_client.close()
        existing_server.close()
        invalid_client.close()
        invalid_server.close()


def test_valid_hello_replaces_existing_client():
    bridge = KeyboardBridgeServer("127.0.0.1", 8765)
    existing_server, existing_client = socket.socketpair()
    valid_server, valid_client = socket.socketpair()
    try:
        bridge.client = existing_server
        valid_client.sendall(encode_message(ClientHelloEvent()))

        accepted = bridge._accept_client_if_valid(valid_server, ("127.0.0.1", 1234))

        assert accepted is True
        assert bridge.client is valid_server
    finally:
        existing_client.close()
        valid_client.close()
        valid_server.close()


def test_legacy_lan_connection_can_update_from_new_host():
    bridge = KeyboardBridgeServer("127.0.0.1", 8765)
    legacy_server, legacy_client = socket.socketpair()
    try:
        accepted = bridge._accept_client_if_valid(legacy_server, ("192.168.31.54", 1234))

        assert accepted is True
        assert bridge.client is legacy_server
        assert legacy_client.recv(1024).startswith(b'{"type":"ping"')
    finally:
        legacy_client.close()
        legacy_server.close()
