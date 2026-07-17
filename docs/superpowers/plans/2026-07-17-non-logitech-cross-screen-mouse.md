# Non-Logitech Cross-Screen Mouse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let this Windows host use any ordinary mouse to cross the right screen edge and control the remote Windows computer without Logitech Flow.

**Architecture:** Keep the existing TCP `MouseInput` protocol and cursor-centering movement path. Add a dedicated Windows low-level mouse hook for button and wheel events so they can be forwarded while being suppressed locally. Keep the feature opt-in and disable suppression immediately on disconnect, local target, game mode, or feature shutdown.

**Tech Stack:** Rust, Windows `WH_MOUSE_LL`, Tauri 2, TypeScript, existing LAN updater.

---

### Task 1: Capture and suppress remote mouse buttons and wheel

**Files:**
- Create: `apps/desktop-tauri/src-tauri/src/mouse_hook.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/lib.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/core.rs`

- [ ] **Step 1: Write failing message-decoding tests**

Add tests for left/right/middle down/up and signed vertical/horizontal wheel deltas against a wished-for `decode_mouse_message(message, mouse_data)` API.

- [ ] **Step 2: Run the focused tests and verify RED**

Run: `cargo test mouse_hook`

Expected: compilation fails because `decode_mouse_message` and the mouse hook module implementation do not exist yet.

- [ ] **Step 3: Implement the minimal low-level hook**

Map `WM_LBUTTON*`, `WM_RBUTTON*`, `WM_MBUTTON*`, `WM_MOUSEWHEEL`, and `WM_MOUSEHWHEEL` to the existing `MouseInputEvent`. Ignore injected events, publish physical events through an `mpsc::Sender`, and return `LRESULT(1)` only while remote suppression is enabled.

- [ ] **Step 4: Wire hook lifecycle and safety**

Start the hook with host mode. Drain events in `handle_host_client`, forward them only when the target is remote, and always clear suppression when returning local, disconnecting, stopping, entering game mode, or disabling experimental mouse input.

- [ ] **Step 5: Verify GREEN**

Run: `cargo test mouse_hook && cargo test`

Expected: all mouse hook tests and the full Rust suite pass.

### Task 2: Prepare a real install/update build

**Files:**
- Modify: `apps/desktop-tauri/package.json`
- Modify: `apps/desktop-tauri/src-tauri/Cargo.toml`
- Modify: `apps/desktop-tauri/src-tauri/tauri.conf.json`
- Modify: `apps/desktop-tauri/src-tauri/Cargo.lock`
- Modify: `apps/desktop-tauri/src/main.ts`
- Create: `docs/releases/v0.1.23.md`

- [ ] **Step 1: Bump the application version for LAN delivery**

Update only project-owned version fields so the LAN updater can distinguish the mouse build from `0.1.22`; the final verified delivery is `0.1.26`.

- [ ] **Step 2: Make the mouse control explicit in the UI**

Keep the opt-in switch visible, explain that any Windows mouse works, and state that game mode disables remote mouse injection.

- [ ] **Step 3: Build and package**

Run: `cargo test`, `npm.cmd run build`, and `npm.cmd run build:lan-update`.

Expected: tests pass and the update directory contains the `0.1.26` installer plus matching manifest and hash.

### Task 3: Install, launch, and prove the local/LAN path

**Files:**
- Runtime output: `%LOCALAPPDATA%\Devices Router\updates\`
- Runtime config: `%LOCALAPPDATA%\Devices Router\config.json`

- [ ] **Step 1: Stop the old elevated process safely**

Use the tray exit path or a same-elevation process stop, then launch the freshly built `target/release/devices-router.exe --host`.

- [ ] **Step 2: Enable the experimental mouse setting on this host**

Persist `experimentalMouseInput: true` and keep `gameMode: false` without changing unrelated user settings.

- [ ] **Step 3: Verify LAN services and remote connection**

Confirm TCP `8765` and update port `8767` are listening, inspect the current connection state, and verify that the `0.1.26` update manifest is being served.

- [ ] **Step 4: Commit the usable build**

Run final tests/build/diff checks, inspect staged scope, and commit only the mouse hook, version, UI, and release documentation changes.
