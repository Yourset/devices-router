# Devices Router

A small Windows utility that adds keyboard following to Logitech Flow setups.

Logitech Flow keeps handling cross-computer mouse movement. Devices Router forwards keyboard input from the host PC to the remote PC. The intended experience is simple: start the app on both machines, move the mouse to a machine, and the keyboard follows.

Chinese documentation: [README.md](README.md)

## Current Status

- Platform: Windows -> Windows
- Main implementation: Tauri/Rust desktop app in `apps/desktop-tauri/`
- Current version: `v0.1.12`
- Ports:
  - TCP `8765`: keyboard events, control messages, heartbeat
  - UDP `8766`: host discovery
  - TCP `8767`: LAN update server

## Features

- Low-level keyboard hook on the host PC
- Windows `SendInput` injection on the remote PC
- Automatic discovery and reconnect
- Mouse-activity based keyboard target switching
- Bidirectional heartbeat for connection state
- LAN-based remote update from the host PC
- Copy, export, and clear logs
- Remembers the last mode and supports a startup option

## Quick Start

1. Install and open `Devices Router` on the host PC.
2. Click `Host mode` and keep the app running.
3. Install and open the same app on the remote PC.
4. Click `Remote mode` and wait for it to find the host.
5. Focus Notepad, chat, an IDE, or any target input field on the remote PC.
6. Move the mouse to the remote PC; the keyboard should follow. Move back to the host; the keyboard should return.

Manual switching is also available:

- `Ctrl+Alt+1` on the host: keyboard back to host
- `Ctrl+Alt+2` on the host: keyboard to remote
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

- [中文使用教程](docs/user-guide.zh.md)
- [English User Guide](docs/user-guide.en.md)
- [中文排障手册](docs/troubleshooting.zh.md)
- [English Troubleshooting](docs/troubleshooting.en.md)
- [中文视频脚本提纲](docs/video-outline.zh.md)
- [English Video Outline](docs/video-outline.en.md)

## Known Limits

- Currently focused on Windows-to-Windows usage.
- UAC, elevated windows, protected games, or security software may reject normal simulated input.
- Chinese IME composition, complex shortcuts, and media keys may need more polish.
- Mouse following is inferred from mouse activity on both machines. It does not read Logitech Flow's private protocol.

## Project Positioning

This is a practical personal utility for people who already use Logitech Flow for mouse movement but do not own a Logitech keyboard. It does not crack Flow or emulate Logitech devices; it works as an independent keyboard bridge.

The more honest origin story: I wanted to vibe code on another PC while keeping the main PC available for League of Legends or other windows. Flow moved the mouse, but the keyboard did not follow. See [Motivation](docs/motivation.en.md).
