# Devices Router User Guide

## Who It Is For

You already have Logitech Flow working, so your mouse can move between two PCs, but a normal keyboard does not follow. Devices Router fills that gap.

Recommended setup:

- A physical keyboard is attached to the host PC.
- The remote PC occasionally needs text, shortcuts, or code input.
- Logitech Flow still handles mouse movement.
- You do not want to buy a Logitech keyboard just for keyboard following.

## Install and Start

Normal users only need the installer. Node.js, Rust, Python, and command line usage are not required.

1. Download `Devices Router_version_x64-setup.exe` from the Release page.
2. Install it on the host PC.
3. Install it on the remote PC.
4. Open the same `Devices Router` app on both PCs.

Host PC:

1. Open the app.
2. Click `Host mode`.
3. Keep the app running.

Remote PC:

1. Open the app.
2. Click `Remote mode`.
3. Wait until the status becomes `Connected`.
4. Focus a target input field, such as Notepad, chat, browser input, or an IDE.

## Daily Usage

Normally you do not need manual switching:

- Move the mouse on the remote PC, and the keyboard switches to the remote PC.
- Move the mouse on the host PC, and the keyboard returns to the host PC.

Manual switching is available:

- Click `Keyboard to host`
- Click `Keyboard to remote`
- Press `Ctrl+Alt+1` on the host to return to host
- Press `Ctrl+Alt+2` on the host to switch to remote

## Understanding Status

The most important fields on the overview page are:

- `Mode`: whether this app instance is host or remote.
- `Connection`: whether the two PCs are connected.
- `Keyboard target`: where host keyboard input should go.

If you move the mouse on the remote PC but the target does not become remote, mouse activity reporting or the control channel is not working.

## Log Buttons

The log panel has three buttons:

- `Clear logs`: clears current logs in the app.
- `Copy logs`: copies logs to the clipboard.
- `Export logs`: saves logs as a `.txt` file.

Useful keywords:

- `connected to host`
- `switch request sent`
- `remote requested keyboard to remote`
- `forwarded key`
- `input key`
- `heartbeat failed`

## Auto Update

The host exposes a LAN update service. After the remote connects, it checks the host for an update package.

Usually you only need to update the host PC; the remote follows after it reconnects. If update fails, logs show whether it failed during download, verification, or installation.

## Start on Login

The `Update` page has a `Start on login` option. The app remembers the last mode:

- If the previous mode was host, it starts as host.
- If the previous mode was remote, it starts as remote.

## Short Test Path

1. Start `Host mode` on the host PC.
2. Start `Remote mode` on the remote PC.
3. Confirm the remote shows `Connected`.
4. Open Notepad on the remote PC.
5. Move the mouse on the remote PC.
6. Type `hello123` on the host keyboard.
7. If the text appears in the remote Notepad, the core path works.

## Boundaries

This is not a full KVM and not a Logitech Flow plugin. It does not read Flow's private state; it infers the current target from mouse activity on both machines.

UAC prompts, elevated windows, some games, or protected windows may reject simulated input because of Windows security boundaries.
