from __future__ import annotations

import argparse
import socket
import threading

from pynput import keyboard

from .discovery import broadcast_server
from .keynames import normalize_key_name
from .protocol import KeyEvent, PingEvent, encode_message
from .target_state import TargetState


class KeyboardBridgeServer:
    def __init__(self, bind_host: str, port: int) -> None:
        self.bind_host = bind_host
        self.port = port
        self.state = TargetState()
        self.client: socket.socket | None = None
        self.lock = threading.Lock()

    def serve(self) -> None:
        discovery_stop = threading.Event()
        discovery_thread = threading.Thread(
            target=broadcast_server,
            args=(self.port, discovery_stop),
            daemon=True,
        )
        discovery_thread.start()
        accept_thread = threading.Thread(target=self._accept_loop, daemon=True)
        accept_thread.start()

        print("[server] hotkeys:")
        print("  Ctrl+Alt+1 -> keyboard local")
        print("  Ctrl+Alt+2 -> keyboard remote")
        print("  Ctrl+Alt+Esc -> exit")
        print("[server] waiting for client while keyboard listener runs...")

        try:
            with keyboard.Listener(on_press=self._on_press, on_release=self._on_release) as listener:
                listener.join()
        finally:
            discovery_stop.set()

    def _accept_loop(self) -> None:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
            server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            server.bind((self.bind_host, self.port))
            server.listen(1)
            print(f"[server] listening on {self.bind_host}:{self.port}")
            while True:
                client, address = server.accept()
                client.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
                with self.lock:
                    if self.client is not None:
                        self.client.close()
                    self.client = client
                    self.client.sendall(encode_message(PingEvent()))
                print(f"[server] client connected: {address[0]}:{address[1]}")

    def _on_press(self, key: keyboard.Key | keyboard.KeyCode) -> bool | None:
        if self._is_hotkey(key):
            return True
        self._send_if_remote("down", key)
        return True

    def _on_release(self, key: keyboard.Key | keyboard.KeyCode) -> bool | None:
        self._send_if_remote("up", key)
        return True

    def _is_hotkey(self, key: keyboard.Key | keyboard.KeyCode) -> bool:
        # GlobalHotKeys handles target switching; listener still sees those keys.
        return False

    def _send_if_remote(self, action: str, key: keyboard.Key | keyboard.KeyCode) -> None:
        if not self.state.remote_enabled:
            return
        key_name = normalize_key_name(str(key))
        payload = encode_message(KeyEvent(action=action, key=key_name))
        with self.lock:
            if self.client is None:
                return
            try:
                self.client.sendall(payload)
            except OSError as exc:
                print(f"[server] client disconnected: {exc}")
                self.client.close()
                self.client = None

    def enable_local(self) -> None:
        self.state.enable_local()
        print("[server] target: local")

    def enable_remote(self) -> None:
        self.state.enable_remote()
        print("[server] target: remote")

    def stop(self) -> None:
        print("[server] exiting")
        raise SystemExit(0)


def run_server(bind_host: str, port: int) -> None:
    bridge = KeyboardBridgeServer(bind_host, port)
    hotkeys = keyboard.GlobalHotKeys(
        {
            "<ctrl>+<alt>+1": bridge.enable_local,
            "<ctrl>+<alt>+2": bridge.enable_remote,
            "<ctrl>+<alt>+<esc>": bridge.stop,
        }
    )
    hotkeys.start()
    bridge.serve()


def main() -> None:
    parser = argparse.ArgumentParser(description="Flow Keyboard Bridge server")
    parser.add_argument("--bind", default="0.0.0.0", help="Address to listen on")
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    run_server(args.bind, args.port)


if __name__ == "__main__":
    main()
