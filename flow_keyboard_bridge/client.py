from __future__ import annotations

import argparse
import socket
import time
import threading

from pynput import mouse

from .discovery import discover_server_auto
from .protocol import KeyEvent, MouseActivityEvent, PingEvent, decode_message, encode_message
from .updates import check_remote_update
from .win_input import send_key_event


def run_client_once(host: str, port: int) -> None:
    print(f"[client] connecting to {host}:{port} ...")
    with socket.create_connection((host, port), timeout=5) as sock:
        sock.settimeout(None)
        print("[client] connected. Focus the target app here, then switch on server with Ctrl+Alt+2.")
        check_remote_update(host, "remote")
        mouse_stop = threading.Event()
        mouse_thread = threading.Thread(target=_send_mouse_activity, args=(sock, mouse_stop), daemon=True)
        mouse_thread.start()
        stream = sock.makefile("rb")
        try:
            for line in stream:
                try:
                    event = decode_message(line)
                    if isinstance(event, PingEvent):
                        print("[client] server handshake ok")
                    elif isinstance(event, KeyEvent):
                        send_key_event(event.key, event.action == "down")
                except Exception as exc:
                    print(f"[client] ignored message: {exc}")
        finally:
            mouse_stop.set()
    print("[client] server closed the connection")


def _send_mouse_activity(sock: socket.socket, stop_event: threading.Event) -> None:
    last_sent_at = 0.0
    lock = threading.Lock()

    def on_move(x, y) -> None:
        nonlocal last_sent_at
        now = time.monotonic()
        if now - last_sent_at < 0.5:
            return
        last_sent_at = now
        with lock:
            try:
                sock.sendall(encode_message(MouseActivityEvent(source="remote")))
            except OSError:
                stop_event.set()
                return False

    listener = mouse.Listener(on_move=on_move)
    listener.start()
    while not stop_event.wait(0.2):
        pass
    listener.stop()


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
