from __future__ import annotations

from dataclasses import dataclass
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import urllib.request

from .app_info import APP_VERSION, HOST_EXE_NAME, REMOTE_EXE_NAME, UPDATE_PORT


@dataclass(frozen=True)
class UpdateFile:
    version: str
    path: str

    def needs_update(self, current_version: str) -> bool:
        return self.version != current_version


@dataclass(frozen=True)
class UpdateManifest:
    files: dict[str, UpdateFile]

    def file_for(self, role: str) -> UpdateFile:
        return self.files[role]


def parse_manifest(payload: bytes) -> UpdateManifest:
    data = json.loads(payload.decode("utf-8-sig"))
    files = {
        role: UpdateFile(version=str(info["version"]), path=str(info["path"]))
        for role, info in data.get("files", {}).items()
    }
    return UpdateManifest(files=files)


def app_dir() -> Path:
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parents[1]


def updates_dir() -> Path:
    return app_dir() / "updates"


def local_manifest_path() -> Path:
    return updates_dir() / "manifest.json"


def start_update_server(port: int = UPDATE_PORT) -> threading.Thread | None:
    root = updates_dir()
    manifest = root / "manifest.json"
    if not manifest.exists():
        print(f"[update] no local update manifest: {manifest}")
        return None

    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(root), **kwargs)

        def log_message(self, format, *args):
            print("[update] " + format % args)

    def serve() -> None:
        try:
            with ThreadingHTTPServer(("0.0.0.0", port), Handler) as server:
                print(f"[update] serving updates on 0.0.0.0:{port}")
                server.serve_forever()
        except OSError as exc:
            print(f"[update] server unavailable: {exc}")

    thread = threading.Thread(target=serve, daemon=True)
    thread.start()
    return thread


def check_local_self_update(role: str) -> None:
    manifest_path = local_manifest_path()
    if not manifest_path.exists():
        return
    manifest = parse_manifest(manifest_path.read_bytes())
    update_file = manifest.file_for(role)
    if not update_file.needs_update(APP_VERSION):
        return
    source = updates_dir() / update_file.path
    if not source.exists():
        print(f"[update] local update file missing: {source}")
        return
    print(f"[update] local {role} update found: {APP_VERSION} -> {update_file.version}")
    apply_update_and_restart(source, Path(sys.executable).resolve())


def check_remote_update(host: str, role: str = "remote", port: int = UPDATE_PORT) -> None:
    base_url = f"http://{host}:{port}"
    try:
        with urllib.request.urlopen(f"{base_url}/manifest.json", timeout=3) as response:
            manifest = parse_manifest(response.read())
    except Exception as exc:
        print(f"[update] remote update check skipped: {exc}")
        return

    update_file = manifest.file_for(role)
    if not update_file.needs_update(APP_VERSION):
        print(f"[update] already current: {APP_VERSION}")
        return

    target = Path(sys.executable).resolve()
    download = target.with_suffix(target.suffix + ".download")
    url = f"{base_url}/{update_file.path}"
    print(f"[update] downloading {role} update: {APP_VERSION} -> {update_file.version}")
    try:
        urllib.request.urlretrieve(url, download)
    except Exception as exc:
        print(f"[update] download failed: {exc}")
        return
    apply_update_and_restart(download, target)


def apply_update_and_restart(source: Path, target: Path) -> None:
    script = target.with_suffix(".update.ps1")
    script.write_text(
        "\n".join(
            [
                "$ErrorActionPreference = 'Stop'",
                f"$pidToWait = {os.getpid()}",
                f"$source = '{source}'",
                f"$target = '{target}'",
                "Wait-Process -Id $pidToWait -ErrorAction SilentlyContinue",
                "Start-Sleep -Milliseconds 300",
                "Move-Item -Force -LiteralPath $source -Destination $target",
                "Start-Process -FilePath $target",
                "Remove-Item -LiteralPath $MyInvocation.MyCommand.Path -Force",
            ]
        ),
        encoding="utf-8",
    )
    print("[update] applying update and restarting...")
    subprocess.Popen(
        [
            "powershell",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            str(script),
        ],
        close_fds=True,
    )
    os._exit(0)


def default_manifest() -> dict:
    return {
        "version": APP_VERSION,
        "files": {
            "host": {"version": APP_VERSION, "path": HOST_EXE_NAME},
            "remote": {"version": APP_VERSION, "path": REMOTE_EXE_NAME},
        },
    }
