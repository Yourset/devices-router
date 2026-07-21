# Devices Router

A small Windows utility that routes one ordinary keyboard between two Windows PCs.

Devices Router forwards keyboard input but never forwards the mouse. `v0.2.0` keeps the desktop console fixed in place: periodic status refreshes no longer rebuild the whole page, and only the log viewer scrolls.

Language / 璇█: [绠€浣撲腑鏂嘳(README.md) | **English**

## Current Status

- Platform: Windows -> Windows
- Main implementation: Tauri/Rust desktop app in `apps/desktop-tauri/`
- Current version: `v0.2.4`
- For normal users: install the `.exe` setup package. Node.js, Rust, Python, and other development dependencies are not required.
- Ports:
  - TCP `8765`: keyboard events, reliable control, heartbeat, and RTT probes with `TCP_NODELAY`
  - UDP `8766`: host discovery and the mouse-activity fast path
  - TCP `8767`: LAN update server

## Features

- Low-level keyboard hook on the host PC
- Windows `SendInput` injection on the remote PC
- Automatic discovery and reconnect
- Mouse-activity based keyboard target switching
- Automatic TCP fallback when the UDP activity fast path is unavailable
- Host-authoritative live RTT, stable RTT, jitter, and loss over the latest 20 probes
- Bidirectional heartbeat for connection state
- LAN-based remote update from the host PC
- Copy, export, and clear logs
- Remembers the last mode and supports a startup option

## Quick Start

1. Download `Devices Router_version_x64-setup.exe` from the Release page.
2. Install the same package on both the host PC and remote PC.
3. Open `Devices Router` on the host PC and click `Host mode`.
4. Open `Devices Router` on the remote PC and click `Remote mode`.
5. Focus Notepad, chat, an IDE, or any target input field on the remote PC.
6. Move the mouse on the remote PC; the keyboard should follow. Move the mouse on the host PC; the keyboard should return.

The ready-to-use installer does not require command line usage or a development environment. The source and build commands below are only for developers.

Manual switching is also available:

- `Ctrl+Alt+1` on the host: keyboard back to host
- `Ctrl+Alt+2` on the host: keyboard to remote
- `Ctrl+Alt+Esc` on the host: emergency local release, independent of the network
- App buttons: `Keyboard to host` / `Keyboard to remote`

## Auto Update

When the host starts, it exposes a LAN update service. The remote checks the host after connecting:

- If versions match, it stays as-is.
- If the host has a newer package, the remote downloads, verifies, and installs it.
- The host manifest is stored at:

```text
%LOCALAPPDATA%\Devices Router\updates\manifest.json
```

For development LAN publishing:

```powershell
cd apps\desktop-tauri
powershell.exe -ExecutionPolicy Bypass -File .\scripts\prepare-lan-update.ps1
```

## Run From Source

This section is for developers only. If you just want to use the app, download the installer from Releases.

Requires Node.js, Rust, and the Windows dependencies needed by Tauri.

```powershell
cd apps\desktop-tauri
npm install
npm run tauri -- dev
```

Build installers:

```powershell
cd apps\desktop-tauri
npm run tauri -- build
```

Installer output:

```text
apps/desktop-tauri/src-tauri/target/release/bundle/nsis/
apps/desktop-tauri/src-tauri/target/release/bundle/msi/
```

## Tests

```powershell
cargo test --manifest-path apps\desktop-tauri\src-tauri\Cargo.toml
cd apps\desktop-tauri
npm run build
```

## Common Issues

### Remote says disconnected

Check:

- Both PCs are on the same LAN.
- The host app is running in `Host mode`.
- Windows Firewall allows TCP `8765`, TCP `8767`, and UDP `8766`.
- VPN/TUN/proxy virtual adapters are not interfering with LAN discovery.

### Connected but keyboard does not arrive

Check logs for:

- Remote: `switch request sent`
- Host: `remote requested keyboard to remote`
- Host: `forwarded key`
- Remote: `input key`

If the remote logs a sent request but the host logs nothing, the control message did not reach the host. If the host forwards keys but the remote does not input them, the remote injection failed or the target input field is not focused.

### Why not pure H5

A browser page cannot inject system-level keyboard input into other Windows applications. The UI is a Tauri desktop shell; keyboard capture and input injection are implemented in the local Rust backend.

## Docs

- [涓枃浣跨敤鏁欑▼](docs/user-guide.zh.md)
- [English User Guide](docs/user-guide.en.md)
- [涓枃鎺掗殰鎵嬪唽](docs/troubleshooting.zh.md)
- [English Troubleshooting](docs/troubleshooting.en.md)
- [涓枃瑙嗛鑴氭湰鎻愮翰](docs/video-outline.zh.md)
- [English Video Outline](docs/video-outline.en.md)

## Known Limits

- Currently focused on Windows-to-Windows usage.
- UAC, elevated windows, protected games, or security software may reject normal simulated input.
- Chinese IME composition, complex shortcuts, and media keys may need more polish.
- Cross-computer mouse movement, click, and wheel forwarding remain disabled; only mouse activity is observed to select the keyboard target.

## Project Positioning

This is a practical personal utility for people who already use Logitech Flow for mouse movement but do not own a Logitech keyboard. It does not crack Flow or emulate Logitech devices; it works as an independent keyboard bridge.

The more honest origin story: I wanted to vibe code on another PC while keeping the main PC available for League of Legends or other windows. Flow moved the mouse, but the keyboard did not follow. See [Motivation](docs/motivation.en.md).


## v0.2.0: Two Remote PCs

- One host can keep two remote PCs connected at the same time; a third distinct remote is rejected without displacing either active session.
- Logitech Flow still owns mouse movement. Devices Router only observes mouse activity on all three PCs and routes the keyboard to the last active PC.
- The host UI shows the local PC and two remote slots, supports per-device aliases, and allows explicit device selection.
- `Ctrl+Alt+Esc` remains a host-local emergency release. Disconnects and send failures immediately return the keyboard to the host.
- Upgrade the host first, then both remotes. During mixed-version operation, only one legacy remote without a device ID can connect.
