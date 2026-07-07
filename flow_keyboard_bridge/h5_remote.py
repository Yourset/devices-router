from __future__ import annotations

from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
import sys
import threading
from urllib.parse import parse_qs, urlparse
import webbrowser

from .app_info import APP_VERSION
from .client import run_client
from .updates import check_local_self_update


class LogBuffer:
    def __init__(self, limit: int = 1000) -> None:
        self.limit = limit
        self._entries: list[dict[str, int | str]] = []
        self._next_id = 1
        self._lock = threading.Lock()

    def write(self, text: str) -> int:
        if not text:
            return 0
        with self._lock:
            entry = {"id": self._next_id, "text": text}
            self._next_id += 1
            self._entries.append(entry)
            if len(self._entries) > self.limit:
                self._entries = self._entries[-self.limit :]
        return len(text)

    def flush(self) -> None:
        return None

    def entries_after(self, last_id: int) -> list[dict[str, int | str]]:
        with self._lock:
            return [entry for entry in self._entries if int(entry["id"]) > last_id]


class TeeWriter:
    def __init__(self, *writers) -> None:
        self.writers = writers

    def write(self, text: str) -> int:
        for writer in self.writers:
            writer.write(text)
        return len(text)

    def flush(self) -> None:
        for writer in self.writers:
            writer.flush()


def run_remote_h5_app(port: int = 0) -> None:
    check_local_self_update("remote")
    logs = LogBuffer()
    sys.stdout = TeeWriter(sys.__stdout__, logs)
    sys.stderr = TeeWriter(sys.__stderr__, logs)

    stop_event = threading.Event()
    client_thread = threading.Thread(target=lambda: run_client(None, 8765, True, 8), daemon=True)
    client_thread.start()

    server = _create_server(logs, stop_event, port)
    host, actual_port = server.server_address
    url = f"http://127.0.0.1:{actual_port}/"
    print(f"[H5] 副电脑控制台已启动：{url}")
    webbrowser.open(url)

    while not stop_event.is_set():
        server.handle_request()
    server.server_close()
    os._exit(0)


def _create_server(logs: LogBuffer, stop_event: threading.Event, port: int) -> ThreadingHTTPServer:
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self) -> None:
            parsed = urlparse(self.path)
            if parsed.path == "/":
                self._send_text(render_remote_page(APP_VERSION), "text/html; charset=utf-8")
                return
            if parsed.path == "/api/logs":
                query = parse_qs(parsed.query)
                after = int(query.get("after", ["0"])[0])
                payload = {"entries": logs.entries_after(after), "version": APP_VERSION}
                self._send_json(payload)
                return
            if parsed.path == "/api/stop":
                stop_event.set()
                self._send_json({"ok": True})
                return
            self.send_error(404)

        def log_message(self, format, *args) -> None:
            return None

        def _send_json(self, payload: dict) -> None:
            body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def _send_text(self, text: str, content_type: str) -> None:
            body = text.encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

    return ThreadingHTTPServer(("127.0.0.1", port), Handler)


def render_remote_page(version: str) -> str:
    return f"""<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>键盘跟随工具 - 副电脑</title>
  <style>
    :root {{
      color-scheme: light;
      font-family: "Microsoft YaHei UI", "Segoe UI", sans-serif;
      background: #f5f7fb;
      color: #18202f;
    }}
    body {{
      margin: 0;
      min-height: 100vh;
      display: grid;
      grid-template-rows: auto 1fr;
    }}
    header {{
      padding: 18px 24px 14px;
      background: #ffffff;
      border-bottom: 1px solid #d9deea;
    }}
    h1 {{
      margin: 0 0 6px;
      font-size: 20px;
      font-weight: 700;
      letter-spacing: 0;
    }}
    .sub {{
      display: flex;
      flex-wrap: wrap;
      gap: 10px 16px;
      align-items: center;
      color: #4f5b6f;
      font-size: 14px;
    }}
    .pill {{
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 5px 9px;
      border: 1px solid #b8c4d8;
      border-radius: 6px;
      background: #f8fafc;
      color: #223047;
    }}
    .dot {{
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: #1f9d55;
    }}
    main {{
      padding: 18px 24px 24px;
      display: grid;
      gap: 14px;
      grid-template-rows: auto 1fr;
      min-height: 0;
    }}
    .toolbar {{
      display: flex;
      gap: 10px;
      align-items: center;
      flex-wrap: wrap;
    }}
    button {{
      border: 1px solid #9aa8bd;
      background: #ffffff;
      color: #18202f;
      border-radius: 6px;
      padding: 8px 12px;
      font-size: 14px;
      cursor: pointer;
    }}
    button:hover {{
      background: #edf2f8;
    }}
    pre {{
      margin: 0;
      padding: 14px;
      background: #10141c;
      color: #e7edf7;
      border-radius: 6px;
      overflow: auto;
      min-height: 280px;
      font: 13px/1.5 Consolas, "Microsoft YaHei UI", monospace;
      white-space: pre-wrap;
    }}
  </style>
</head>
<body>
  <header>
    <h1>键盘跟随工具 - 副电脑</h1>
    <div class="sub">
      <span class="pill"><span class="dot"></span>本地 H5 控制台运行中</span>
      <span>版本 v{version}</span>
      <span>保持此页面或后台程序打开即可接收键盘</span>
    </div>
  </header>
  <main>
    <div class="toolbar">
      <button id="clear">清空日志</button>
      <button id="stop">停止客户端</button>
    </div>
    <pre id="log">正在启动...</pre>
  </main>
  <script>
    const log = document.getElementById("log");
    let lastId = 0;
    let empty = true;

    document.getElementById("clear").onclick = () => {{
      log.textContent = "";
      empty = true;
    }};
    document.getElementById("stop").onclick = async () => {{
      await fetch("/api/stop");
      log.textContent += "\\n[H5] 已请求停止客户端。";
    }};

    async function poll() {{
      try {{
        const res = await fetch(`/api/logs?after=${{lastId}}`);
        const data = await res.json();
        for (const entry of data.entries) {{
          if (empty) {{
            log.textContent = "";
            empty = false;
          }}
          lastId = entry.id;
          log.textContent += entry.text;
          log.scrollTop = log.scrollHeight;
        }}
      }} catch (error) {{
        if (empty) {{
          log.textContent = "";
          empty = false;
        }}
        log.textContent += "\\n[H5] 日志连接暂时不可用。";
      }}
      setTimeout(poll, 700);
    }}
    poll();
  </script>
</body>
</html>"""
