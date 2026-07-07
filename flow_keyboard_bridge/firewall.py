from __future__ import annotations

import ctypes
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from .app_info import UPDATE_PORT
from .discovery import DISCOVERY_PORT


@dataclass(frozen=True)
class FirewallRule:
    name: str
    protocol: str
    port: int


HOST_RULES = (
    FirewallRule("Flow Keyboard Bridge TCP 8765", "TCP", 8765),
    FirewallRule("Flow Keyboard Bridge UDP 8766", "UDP", DISCOVERY_PORT),
    FirewallRule("Flow Keyboard Bridge TCP 8767", "TCP", UPDATE_PORT),
)


def ensure_host_firewall_rules() -> None:
    missing = [rule for rule in HOST_RULES if not _rule_exists(rule.name)]
    if not missing:
        return
    ports = ", ".join(f"{rule.protocol} {rule.port}" for rule in missing)
    print(f"[防火墙] 需要放行：{ports}，正在请求一次管理员确认...")
    script = _write_firewall_script(missing)
    result = ctypes.windll.shell32.ShellExecuteW(
        None,
        "runas",
        "cmd.exe",
        f'/c "{script}"',
        None,
        1,
    )
    if result <= 32:
        print(f"[防火墙] 管理员确认没有成功：ShellExecuteW={result}")
        return
    missing_after = [rule for rule in HOST_RULES if not _rule_exists(rule.name)]
    if not missing_after:
        print("[防火墙] 已完成放行")
    else:
        remaining = ", ".join(f"{rule.protocol} {rule.port}" for rule in missing_after)
        print(f"[防火墙] 仍未检测到规则：{remaining}")


def _rule_exists(name: str) -> bool:
    result = subprocess.run(
        ["netsh", "advfirewall", "firewall", "show", "rule", f"name={name}"],
        capture_output=True,
        text=True,
        encoding="mbcs",
        errors="ignore",
    )
    return result.returncode == 0


def _write_firewall_script(rules: list[FirewallRule]) -> Path:
    path = Path(tempfile.gettempdir()) / "flow-keyboard-bridge-firewall.cmd"
    lines = ["@echo off"]
    for rule in rules:
        lines.append(
            "netsh advfirewall firewall add rule "
            f'name="{rule.name}" dir=in action=allow protocol={rule.protocol} '
            f"localport={rule.port} profile=any"
        )
    lines.append("exit /b 0")
    path.write_text("\r\n".join(lines) + "\r\n", encoding="mbcs", errors="ignore")
    return path
