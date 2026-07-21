# Devices Router User Guide

## Who It Is For

You want to use one ordinary keyboard on two Windows PCs. Devices Router observes mouse activity to select the keyboard target, but never forwards or suppresses the mouse.

Recommended setup:

- A physical keyboard is attached to the host PC.
- The remote PC occasionally needs text, shortcuts, or code input.
- Moving the mouse on a PC automatically switches the keyboard target to that PC.
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

Normally no manual switching is needed:

- Move the mouse on the remote PC to switch the keyboard to remote.
- Move the mouse on the host PC to return the keyboard to host.

Manual switching is also available:

- Click `Keyboard to host`
- Click `Keyboard to remote`
- Press `Ctrl+Alt+1` on the host to return to host
- Press `Ctrl+Alt+2` on the host to switch to remote

## Understanding Status

The most important fields on the overview page are:

- `Mode`: whether this app instance is host or remote.
- `Connection`: whether the two PCs are connected.
- `Keyboard target`: where host keyboard input should go.

If the target does not change after mouse activity, mouse activity reporting or the control channel is not working.

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

This is not a full KVM and not a Logitech Flow plugin. The current stable release routes keyboard input only.

UAC prompts, elevated windows, some games, or protected windows may reject simulated input because of Windows security boundaries.


## Three-PC Setup

1. Install v0.2.2 on the host and start Host mode.
2. Install the same package on both remote PCs and start Remote mode on each.
3. When both remotes appear on the host overview, optionally assign aliases.
4. Mouse activity on any PC selects that PC after the 30 ms debounce window.
5. Manual shortcuts: Ctrl+Alt+1 selects host, Ctrl+Alt+2 selects the first remote, and Ctrl+Alt+3 selects the second remote.
6. Ctrl+Alt+Esc always releases the keyboard back to the host.

Devices Router never transports, locks, or suppresses mouse input.
