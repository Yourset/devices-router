from __future__ import annotations

import argparse
import socket
import threading
import time

from pynput import mouse

from .discovery import discover_server_auto
from .protocol import ClientHelloEvent, KeyEvent, MouseActivityEvent, PingEvent, decode_message, encode_message
from .updates import check_remote_update
from .win_input import send_key_event


def run_client_once(host: str, port: int) -> None:
    print(f"[客户端] 正在连接主电脑 {host}:{port} ...")
    with socket.create_connection((host, port), timeout=5) as sock:
        sock.settimeout(None)
        sock.sendall(encode_message(ClientHelloEvent()))
        print("[客户端] 已连接。请在副电脑打开目标输入框，键盘会跟随鼠标切换。")
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
                        print("[客户端] 主电脑握手成功")
                    elif isinstance(event, KeyEvent):
                        send_key_event(event.key, event.action == "down")
                except Exception as exc:
                    print(f"[客户端] 已忽略无法处理的消息：{exc}")
        finally:
            mouse_stop.set()
    print("[客户端] 主电脑连接已关闭")


def _send_mouse_activity(sock: socket.socket, stop_event: threading.Event) -> None:
    last_sent_at = 0.0
    lock = threading.Lock()

    def on_move(x, y) -> bool | None:
        nonlocal last_sent_at
        now = time.monotonic()
        if now - last_sent_at < 0.5:
            return None
        last_sent_at = now
        with lock:
            try:
                sock.sendall(encode_message(MouseActivityEvent(source="remote")))
            except OSError:
                stop_event.set()
                return False
        return None

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
                print(f"[客户端] 正在自动寻找主电脑，最多等待 {discovery_timeout:g} 秒...")
                info = discover_server_auto(port, discovery_timeout)
                target_host = info.host
                target_port = info.port
                print(f"[客户端] 找到主电脑：{target_host}:{target_port}")
            run_client_once(target_host, target_port)
        except OSError as exc:
            print(f"[客户端] 连接失败：{exc}")
        except TimeoutError as exc:
            print(f"[客户端] 自动寻找失败：{exc}")
        if not reconnect:
            return
        print("[客户端] 3 秒后自动重试。关闭窗口即可停止。")
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
        print("\n[客户端] 已停止")


if __name__ == "__main__":
    main()
