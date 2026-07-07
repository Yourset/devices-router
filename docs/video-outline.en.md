# Video Outline: Adding Normal Keyboard Following to Logitech Flow

## Title Ideas

- Make a normal keyboard follow Logitech Flow
- I built a keyboard bridge for Logitech Flow
- Cross-PC keyboard following without buying a Logitech keyboard

## One-Sentence Hook

Logitech Flow moves the mouse between computers, but a normal keyboard does not follow. Devices Router keeps Flow in charge of the mouse and adds a separate keyboard bridge.

The more honest motivation: I wanted to vibe code on another PC while keeping the main PC free for League of Legends or other windows. The mouse could move there, but the keyboard could not.

## Structure

### 1. Opening: The Problem

Visuals:

- Move the mouse from the host PC to the remote PC.
- Type on a normal keyboard and show that text still goes to the host.
- Keep the host PC available for a game or everyday windows while the remote PC shows the development workspace.

Voiceover:

```text
Logitech Flow is great, but keyboard following is tied to the Logitech ecosystem. I did not want to buy another keyboard just for that, so I built a sidecar utility: Flow keeps the mouse, and my tool forwards the keyboard.

The real-life version is: I wanted to vibe code on one machine while the main PC could still run League or whatever else was open. The mouse moved across, the keyboard did not, and that was annoying enough to build a tool.
```

### 2. Minimal Goal

Visuals:

- Simple diagram: Host PC, Remote PC, TCP connection.

Voiceover:

```text
The goal was not to crack Flow or rebuild a full KVM. Version one only had to do one thing: capture keyboard input on the host and inject it on the remote.
```

### 3. First Working Demo

Visuals:

- Host app in host mode.
- Remote app in remote mode and connected.
- `hello123` appears in Notepad on the remote PC.

Voiceover:

```text
When the first text appeared on the remote Notepad, the core path worked. But turning that into something usable every day was where the real work started.
```

### 4. Networking Problems

Visuals:

- Connection failure logs.
- Discovery and local scan logs.
- Firewall or port explanation.

Voiceover:

```text
The first set of problems was networking. IP addresses can be wrong, firewalls block ports, UDP broadcast is not always reliable, and proxy virtual adapters can make the app scan the wrong network.
```

Keywords:

- Discovery
- LAN scan
- Firewall
- Virtual adapter

### 5. Windows Input Problems

Visuals:

- Show `WinError 87`.
- Show `SendInput` and the `INPUT` structure.

Voiceover:

```text
The second problem was Windows SendInput. On 64-bit Windows the INPUT structure size must be right. If it is wrong, Windows only gives you a vague parameter error.
```

### 6. From CLI to Desktop App

Visuals:

- Old command-line version.
- Tauri desktop UI with status, logs, and update page.

Voiceover:

```text
A command-line prototype proves the idea, but it is not something most people want to use every day. So the app moved to Tauri and Rust, with a proper desktop control panel.
```

### 7. Input Isolation

Visuals:

- When target is remote, the host no longer receives typed text.
- Remote input field receives text.

Voiceover:

```text
Capturing keys was not enough. If the host still receives input while forwarding to the remote, both machines type at once. A low-level keyboard hook lets the host swallow local input in remote mode and only forward it to the remote.
```

### 8. Mouse Goes There, Keyboard Follows

Visuals:

- Move mouse on the remote PC; keyboard target becomes remote.
- Move mouse on the host PC; keyboard target returns to host.

Voiceover:

```text
The app does not read Logitech Flow's private protocol. It uses a practical heuristic: if the remote mouse moves, assume the user is on the remote; if the host mouse moves, switch back to the host.
```

### 9. Auto Update and Debugging

Visuals:

- Remote update logs.
- Clear logs, copy logs, heartbeat status.

Voiceover:

```text
To make it usable by other people, I added LAN updates, heartbeat-based connection status, and log buttons. When something breaks, you can copy logs instead of sending screenshots of terminal windows.
```

### 10. Closing

Voiceover:

```text
The interesting part was not designing a perfect architecture on day one. It was repeatedly running into real problems: connection failures, input injection bugs, update issues, stale status, and slowly turning them into a tool that is usable in daily work.
```

## Footage Checklist

- Logitech Flow mouse movement demo
- Normal keyboard failing to follow
- Host and remote modes starting
- Auto discovery and successful connection
- Firewall and port explanation
- Remote Notepad input success
- Host input isolation in remote target mode
- Mouse-activity auto switching
- Auto update logs
- Clear/copy/export logs

## Short-Video Three-Part Structure

1. Problem: mouse moves between PCs, normal keyboard does not.
2. Build: keyboard bridge, networking, input injection, isolation, update.
3. Result: start the app on two PCs; the keyboard follows the mouse.
