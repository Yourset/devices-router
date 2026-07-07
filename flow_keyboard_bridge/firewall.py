from __future__ import annotations

import ctypes
import subprocess

from .app_info import UPDATE_PORT


UPDATE_RULE_NAME = "Flow Keyboard Bridge Update TCP 8767"


def ensure_update_firewall_rule() -> None:
    if _rule_exists():
        return
    print(f"[firewall] update rule missing, requesting permission for TCP {UPDATE_PORT}")
    command = (
        "Start-Process netsh "
        f"-ArgumentList 'advfirewall firewall add rule name=\"{UPDATE_RULE_NAME}\" "
        f"dir=in action=allow protocol=TCP localport={UPDATE_PORT}' "
        "-Verb RunAs -WindowStyle Hidden -Wait"
    )
    result = ctypes.windll.shell32.ShellExecuteW(
        None,
        "runas",
        "powershell.exe",
        f"-NoProfile -ExecutionPolicy Bypass -Command \"{command}\"",
        None,
        0,
    )
    if result <= 32:
        print(f"[firewall] permission request failed: ShellExecuteW={result}")
    elif _rule_exists():
        print("[firewall] update rule enabled")
    else:
        print("[firewall] permission request finished, but rule was not found")


def _rule_exists() -> bool:
    result = subprocess.run(
        ["netsh", "advfirewall", "firewall", "show", "rule", f"name={UPDATE_RULE_NAME}"],
        capture_output=True,
        text=True,
        encoding="mbcs",
        errors="ignore",
    )
    return result.returncode == 0 and UPDATE_RULE_NAME in result.stdout
