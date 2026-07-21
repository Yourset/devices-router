import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DESKTOP = ROOT / "apps" / "desktop-tauri"


def _function_body(source: str, name: str) -> str:
    match = re.search(
        rf"(?:async )?function {re.escape(name)}\([^)]*\) \{{(?P<body>.*?)\n\}}",
        source,
        re.DOTALL,
    )
    assert match is not None, f"missing async function {name}"
    return match.group("body")


def test_periodic_status_refresh_updates_existing_view_without_full_render():
    source = (DESKTOP / "src" / "main.ts").read_text(encoding="utf-8")

    body = _function_body(source, "refreshStatus")

    assert "render();" not in body
    assert "updateStatusView();" in body


def test_tab_refresh_is_paused_only_for_editable_fields():
    source = (DESKTOP / "src" / "main.ts").read_text(encoding="utf-8")

    body = _function_body(source, "updateTabView")

    assert "document.activeElement instanceof HTMLInputElement" in body
    assert "document.activeElement instanceof HTMLTextAreaElement" in body


def test_only_explicit_log_content_can_scroll():
    css = (DESKTOP / "src" / "styles.css").read_text(encoding="utf-8")
    scrolling_selectors = {
        selector.strip()
        for selector, declarations in re.findall(r"([^{}]+)\{([^{}]+)\}", css)
        if re.search(r"overflow\s*:\s*auto\s*;", declarations)
    }

    assert scrolling_selectors == {".log-panel textarea"}
    assert re.search(r"\.content\s*\{[^{}]*overflow\s*:\s*hidden\s*;", css, re.DOTALL)


def test_minimum_window_height_supports_the_non_scrolling_layout():
    config = json.loads((DESKTOP / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))

    assert config["app"]["windows"][0]["minHeight"] == 680


def test_desktop_release_version_is_consistent():
    package = json.loads((DESKTOP / "package.json").read_text(encoding="utf-8"))
    tauri = json.loads((DESKTOP / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
    cargo = (DESKTOP / "src-tauri" / "Cargo.toml").read_text(encoding="utf-8")
    cargo_lock = (DESKTOP / "src-tauri" / "Cargo.lock").read_text(encoding="utf-8")

    assert package["version"] == "0.2.2"
    assert tauri["version"] == "0.2.2"
    assert re.search(r'^version = "0\.2\.2"$', cargo, re.MULTILINE)
    assert re.search(
        r'name = "devices-router"\s+version = "0\.2\.2"',
        cargo_lock,
        re.MULTILINE,
    )


def test_overview_uses_independent_columns_to_avoid_grid_row_gaps():
    source = (DESKTOP / "src" / "main.ts").read_text(encoding="utf-8")
    css = (DESKTOP / "src" / "styles.css").read_text(encoding="utf-8")

    assert '<section class="workspace overview-workspace">' in source
    assert source.count('<div class="panel-stack">') >= 2
    assert re.search(r"\.panel-stack\s*\{[^{}]*display\s*:\s*grid", css, re.DOTALL)


def test_multi_device_ui_uses_device_ids_and_alias_command():
    source = (DESKTOP / "src" / "main.ts").read_text(encoding="utf-8")

    assert "type KeyboardTarget = string;" in source
    assert "localDeviceName: string;" in source
    assert "activeDeviceId: string | null;" in source
    assert "devices: DeviceStatus[];" in source
    assert 'invoke("set_device_alias"' in source
    assert 'data-device-id=' in source
    assert 'id="target-remote"' not in source


def test_multi_device_ui_keeps_two_remote_slots_in_fixed_layout():
    source = (DESKTOP / "src" / "main.ts").read_text(encoding="utf-8")
    css = (DESKTOP / "src" / "styles.css").read_text(encoding="utf-8")

    assert "MAX_REMOTE_DEVICES = 2" in source
    assert "renderRemoteDevices" in source
    assert re.search(r"\.device-grid\s*\{[^{}]*display\s*:\s*grid", css, re.DOTALL)
    assert "overflow: auto" not in re.sub(r"\.log-panel textarea\s*\{[^{}]*\}", "", css, flags=re.DOTALL)
