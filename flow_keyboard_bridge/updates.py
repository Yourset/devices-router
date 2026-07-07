from __future__ import annotations

from dataclasses import dataclass
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import hashlib
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
    kind: str = "file"
    size: int | None = None
    sha256: str | None = None

    def needs_update(self, current_version: str) -> bool:
        return self.version != current_version

    def verify(self, file_path: Path) -> tuple[bool, str]:
        if self.size is not None and file_path.stat().st_size != self.size:
            return False, f"size mismatch: expected {self.size}, got {file_path.stat().st_size}"
        if self.sha256 is not None and _sha256(file_path) != self.sha256:
            return False, "sha256 mismatch"
        return True, "ok"


@dataclass(frozen=True)
class UpdateManifest:
    files: dict[str, UpdateFile]

    def file_for(self, role: str) -> UpdateFile:
        return self.files[role]


def parse_manifest(payload: bytes) -> UpdateManifest:
    data = json.loads(payload.decode("utf-8-sig"))
    files = {
        role: UpdateFile(
            version=str(info["version"]),
            path=str(info["path"]),
            kind=str(info.get("kind", "file")),
            size=int(info["size"]) if "size" in info else None,
            sha256=str(info["sha256"]) if "sha256" in info else None,
        )
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
        print(f"[update] local manifest missing: {manifest}")
        return None

    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(root), **kwargs)

        def log_message(self, format, *args):
            print("[update] " + format % args)

    def serve() -> None:
        try:
            with ThreadingHTTPServer(("0.0.0.0", port), Handler) as server:
                print(f"[update] serving on 0.0.0.0:{port}")
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
        print(f"[update] local file missing: {source}")
        return
    ok, reason = update_file.verify(source)
    if not ok:
        print(f"[update] local file verification failed: {reason}")
        return
    print(f"[update] local {role} update found: {APP_VERSION} -> {update_file.version}")
    apply_update_and_restart(source, Path(sys.executable).resolve())


def check_remote_update(host: str, role: str = "remote", port: int = UPDATE_PORT) -> None:
    base_url = f"http://{host}:{port}"
    try:
        with urllib.request.urlopen(f"{base_url}/manifest.json", timeout=3) as response:
            manifest = parse_manifest(response.read())
    except Exception as exc:
        print(f"[update] remote check skipped: {exc}")
        return

    update_file = manifest.file_for(role)
    if not update_file.needs_update(APP_VERSION):
        print(f"[update] already current: {APP_VERSION}")
        return

    target = update_target_path()
    download = target.with_suffix(target.suffix + ".download")
    url = f"{base_url}/{update_file.path}"
    print(f"[update] downloading {role}: {APP_VERSION} -> {update_file.version}")
    try:
        urllib.request.urlretrieve(url, download)
    except Exception as exc:
        print(f"[update] download failed: {exc}")
        return
    ok, reason = update_file.verify(download)
    if not ok:
        print(f"[update] downloaded file verification failed: {reason}")
        try:
            download.unlink()
        except OSError:
            pass
        return
    apply_update_and_restart(download, target)


def update_target_path() -> Path:
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve()
    return Path(sys.argv[0]).resolve()


def apply_update_and_restart(source: Path, target: Path) -> None:
    script = target.with_suffix(".update.ps1")
    script.write_text(build_update_script(os.getpid(), source, target), encoding="utf-8")
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


def build_update_script(pid_to_wait: int, source: Path, target: Path) -> str:
    source_text = _ps_quote(str(source))
    target_text = _ps_quote(str(target))
    return "\n".join(
        [
            "$ErrorActionPreference = 'Stop'",
            f"$pidToWait = {pid_to_wait}",
            f"$source = {source_text}",
            f"$target = {target_text}",
            "Wait-Process -Id $pidToWait -ErrorAction SilentlyContinue",
            "Start-Sleep -Seconds 3",
            "Move-Item -Force -LiteralPath $source -Destination $target",
            "Remove-Item -LiteralPath $MyInvocation.MyCommand.Path -Force",
        ]
    )


def default_manifest() -> dict:
    return {
        "version": APP_VERSION,
        "files": {
            "host": _manifest_file(HOST_EXE_NAME),
            "remote": _manifest_file(REMOTE_EXE_NAME),
        },
    }


def _manifest_file(exe_name: str) -> dict:
    info = {"version": APP_VERSION, "path": exe_name}
    exe_path = _find_built_exe(exe_name)
    if exe_path.exists():
        info["size"] = exe_path.stat().st_size
        info["sha256"] = _sha256(exe_path)
    return info


def _find_built_exe(exe_name: str) -> Path:
    candidates = [
        app_dir() / exe_name,
        Path.cwd() / "dist" / exe_name,
        Path.cwd() / exe_name,
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate
    return candidates[0]


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _ps_quote(value: str) -> str:
    return "'" + value.replace("'", "''") + "'"
