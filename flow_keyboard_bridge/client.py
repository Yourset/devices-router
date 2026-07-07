from __future__ import annotations

import argparse
import socket
import time

from .discovery import discover_server_auto
from .protocol import KeyEvent, PingEvent, decode_message
from .win_input import send_key_event


def run_client_once(host: str, port: int) -> None:
    print(f"[client] connecting to {host}:{port} ...")
    with socket.create_connection((host, port), timeout=5) as sock:
        sock.settimeout(None)
        print("[client] connected. Focus the target app here, then switch on server with Ctrl+Alt+2.")
        stream = sock.makefile("rb")
        for line in stream:
            try:
                event = decode_message(line)
                if isinstance(event, PingEvent):
                    print("[client] server handshake ok")
                elif isinstance(event, KeyEvent):
                    send_key_event(event.key, event.action == "down")
            except Exception as exc:
                print(f"[client] ignored message: {exc}")
    print("[client] server closed the connection")


def run_client(host: str | None, port: int, reconnect: bool, discovery_timeout: float) -> None:
    while True:
        target_host = host
        target_port = port
        try:
            if target_host is None:
                print(f"[client] searching for server for {discovery_timeout:g} seconds ...")
                info = discover_server_auto(port, discovery_timeout)
                target_host = info.host
                target_port = info.port
                print(f"[client] discovered server at {target_host}:{target_port}")
            run_client_once(target_host, target_port)
        except OSError as exc:
            print(f"[client] connection failed: {exc}")
        except TimeoutError as exc:
            print(f"[client] discovery failed: {exc}")
        if not reconnect:
            return
        print("[client] retrying in 3 seconds. Press Ctrl+C to stop.")
        time.sleep(3)


def main() -> None:
    parser = argparse.ArgumentParser(description="Flow Keyboard Bridge client")
    parser.add_argument("--host", help="Server computer IP address. Omit to auto-discover.")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--discovery-timeout", type=float, default=5)
    parser.add_argument("--no-reconnect", action="store_true", help="Exit after the first disconnect")
    args = parser.parse_args()
    try:
        run_client(args.host, args.port, not args.no_reconnect, args.discovery_timeout)
    except KeyboardInterrupt:
        print("\n[client] stopped")


if __name__ == "__main__":
    main()
