from __future__ import annotations

import argparse
import ipaddress
import socket
import threading
import time

from .discovery import broadcast_server
from .keyboard_router import KeyboardRouter, RawKeyEvent
from .protocol import ClientHelloEvent, KeyEvent, MouseActivityEvent, PingEvent, decode_message, encode_message
from .target_state import TargetState
from .win_keyboard_hook import run_keyboard_hook
from .win_mouse import get_cursor_pos


class KeyboardBridgeServer:
    def __init__(self, bind_host: str, port: int) -> None:
        self.bind_host = bind_host
        self.port = port
        self.state = TargetState()
        self.router = KeyboardRouter()
        self.client: socket.socket | None = None
        self.lock = threading.Lock()
        self.last_host_mouse_at = 0.0
        self.last_remote_mouse_at = 0.0

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
        mouse_thread = threading.Thread(target=self._host_mouse_poll_loop, daemon=True)
        mouse_thread.start()

        print("[主电脑] 快捷键：")
        print("  Ctrl+Alt+1 -> 键盘回到主电脑")
        print("  Ctrl+Alt+2 -> 键盘转到副电脑")
        print("  Ctrl+Alt+Esc -> 退出")
        print("  主电脑鼠标移动 -> 键盘回到主电脑")
        print("  副电脑鼠标移动 -> 键盘转到副电脑")
        print("[主电脑] 正在等待副电脑连接，键盘监听已启动...")

        try:
            run_keyboard_hook(self._handle_raw_keyboard_event)
        finally:
            discovery_stop.set()

    def _accept_loop(self) -> None:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as server:
            server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
            server.bind((self.bind_host, self.port))
            server.listen(5)
            print(f"[主电脑] 正在监听 {self.bind_host}:{self.port}")
            while True:
                client, address = server.accept()
                client.setsockopt(socket.SOL_SOCKET, socket.SO_KEEPALIVE, 1)
                if self._accept_client_if_valid(client, address):
                    reader = threading.Thread(target=self._read_client_events, args=(client,), daemon=True)
                    reader.start()

    def _accept_client_if_valid(self, client: socket.socket, address) -> bool:
        old_timeout = client.gettimeout()
        client.settimeout(2)
        try:
            line = client.makefile("rb").readline()
            event = decode_message(line)
        except TimeoutError:
            if self._is_legacy_lan_client(address[0]):
                client.settimeout(old_timeout)
                return self._accept_verified_client(client, address)
            print(f"[主电脑] 已忽略本机静默连接：{address[0]}:{address[1]}")
            try:
                client.settimeout(old_timeout)
            finally:
                client.close()
            return False
        except Exception as exc:
            print(f"[主电脑] 已忽略非副电脑连接：{address[0]}:{address[1]}，{exc}")
            try:
                client.settimeout(old_timeout)
            finally:
                client.close()
            return False
        client.settimeout(old_timeout)

        if not isinstance(event, ClientHelloEvent):
            print(f"[主电脑] 已忽略未握手的连接：{address[0]}:{address[1]}")
            client.close()
            return False

        return self._accept_verified_client(client, address)

    def _accept_verified_client(self, client: socket.socket, address) -> bool:
        with self.lock:
            if self.client is not None:
                self.client.close()
            self.client = client
            self.client.sendall(encode_message(PingEvent()))
        print(f"[主电脑] 副电脑已连接：{address[0]}:{address[1]}")
        return True

    def _is_legacy_lan_client(self, host: str) -> bool:
        try:
            return not ipaddress.ip_address(host).is_loopback
        except ValueError:
            return False

    def _read_client_events(self, client: socket.socket) -> None:
        stream = client.makefile("rb")
        for line in stream:
            try:
                event = decode_message(line)
            except Exception as exc:
                print(f"[主电脑] 已忽略无法处理的副电脑消息：{exc}")
                continue
            if isinstance(event, MouseActivityEvent) and event.source == "remote":
                self._enable_remote_from_mouse()

    def _handle_raw_keyboard_event(self, action: str, vk_code: int, scan_code: int) -> bool | str:
        suppress = self.router.handle(RawKeyEvent(action, vk_code))
        self.state.remote_enabled = self.router.remote_enabled

        if self.router.stop_requested:
            print("[主电脑] 正在退出")
            return "stop"

        if self.router.remote_enabled:
            self._send_if_remote(action, f"<{vk_code}>")
        return suppress

    def _send_if_remote(self, action: str, key_name: str) -> None:
        if not self.state.remote_enabled:
            return
        payload = encode_message(KeyEvent(action=action, key=key_name))
        with self.lock:
            if self.client is None:
                return
            try:
                self.client.sendall(payload)
            except OSError as exc:
                print(f"[主电脑] 副电脑已断开：{exc}")
                self.client.close()
                self.client = None

    def enable_local(self) -> None:
        self.state.enable_local()
        self.router.remote_enabled = False
        print("[主电脑] 键盘当前在：主电脑")

    def enable_remote(self) -> None:
        self.state.enable_remote()
        self.router.remote_enabled = True
        print("[主电脑] 键盘当前在：副电脑")

    def _enable_remote_from_mouse(self) -> None:
        now = time.monotonic()
        self.last_remote_mouse_at = now
        # Host mouse activity wins because it is the user's escape path back to local.
        if now - self.last_host_mouse_at < 0.8:
            return
        if not self.router.remote_enabled:
            self.enable_remote()

    def _host_mouse_poll_loop(self) -> None:
        try:
            last_pos = get_cursor_pos()
        except OSError:
            return
        while True:
            time.sleep(0.05)
            try:
                current_pos = get_cursor_pos()
            except OSError:
                continue
            if current_pos == last_pos:
                continue
            last_pos = current_pos
            now = time.monotonic()
            self.last_host_mouse_at = now
            if self.router.remote_enabled:
                self.enable_local()

    def stop(self) -> None:
        print("[主电脑] 正在退出")
        raise SystemExit(0)


def run_server(bind_host: str, port: int) -> None:
    bridge = KeyboardBridgeServer(bind_host, port)
    bridge.serve()


def main() -> None:
    parser = argparse.ArgumentParser(description="Flow Keyboard Bridge server")
    parser.add_argument("--bind", default="0.0.0.0", help="Address to listen on")
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()
    run_server(args.bind, args.port)


if __name__ == "__main__":
    main()
