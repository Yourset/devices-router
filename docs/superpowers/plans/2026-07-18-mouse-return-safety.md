# Mouse Return Safety Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make cross-screen mouse control reliably return to the host and always provide a local, network-independent emergency release path.

**Architecture:** Keep the existing host-owned target and TCP `TargetState` acknowledgement. Detect return from the actual forwarded leftward movement plus the remote cursor boundary, so a clamped cursor can still request local control. Add a low-level `Ctrl+Alt+Esc` panic chord that immediately disables local suppression, plus a session cleanup guard and tray action that force the target local on every disconnect or manual release.

**Tech Stack:** Rust, Windows low-level keyboard/mouse hooks, Tauri 2, TypeScript, existing TCP protocol and LAN updater.

---

### Task 1: Lock the return and emergency behavior with failing tests

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/core.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/keyboard_hook.rs`

- [ ] **Step 1: Add a failing return-intent test**

Add `remote_leftward_move_at_clamped_left_edge_requests_local` against a wished-for `should_request_local_return` helper. Cover the exact regression: target is remote, incoming `MoveRelative { dx: -5, dy: 0 }`, cursor is already at the virtual left edge, and the helper returns true.

- [ ] **Step 2: Add failing panic-chord tests**

Add tests against a wished-for `PanicChordState::observe(vk, is_down)` API. Verify `Ctrl+Alt+Esc` activates, ordinary Escape does not, and releasing modifiers resets the chord.

- [ ] **Step 3: Run focused RED tests**

Run: `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml remote_leftward_move_at_clamped_left_edge_requests_local panic_chord`

Expected: compilation fails because the helpers do not exist.

### Task 2: Implement reliable return acknowledgement

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/core.rs`

- [ ] **Step 1: Implement the pure boundary decision**

Implement `should_request_local_return(target, event, cursor, screen)` so only a remote target, a negative relative X delta, and the remote left edge can request local control. Do not require the cursor coordinate to change between polls.

- [ ] **Step 2: Request local control from the remote input receiver**

After successful mouse injection, inspect the resulting remote cursor position. When the pure helper matches, enqueue `TargetRequest { target: Local }`. Continue allowing repeated requests while the remote target remains unacknowledged; `TargetState::Local` is the acknowledgement.

- [ ] **Step 3: Remove the fragile polling return condition**

Keep remote physical mouse activity reporting, but remove the old `current.x < last.x` two-pixel return gate from `run_remote_mouse_loop`.

- [ ] **Step 4: Run focused GREEN tests**

Run: `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml remote_leftward_move`

Expected: all return decision tests pass.

### Task 3: Add local panic release and fail-open cleanup

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/keyboard_hook.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/mouse_hook.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/core.rs`
- Modify: `apps/desktop-tauri/src-tauri/src/lib.rs`

- [ ] **Step 1: Implement the low-level panic chord**

Track Ctrl and Alt inside the keyboard hook. On Escape down while both are held, atomically disable keyboard and mouse suppression before forwarding, store a panic request, and consume the Escape event locally.

- [ ] **Step 2: Consume panic requests in the host loop**

At the top of every host-client iteration, consume the panic request, force the runtime target to local, notify the remote, and release remote mouse buttons and modifier keys.

- [ ] **Step 3: Add fail-open session cleanup**

Make every host-client exit path set the target local and disable both suppressors. Make mouse forwarding write failures terminate the session immediately rather than waiting for a later heartbeat.

- [ ] **Step 4: Add a reusable manual release command**

Expose `force_local_release` through a Tauri command. It must synchronously set the runtime target local and disable both suppressors; the host loop performs best-effort remote releases and target notification.

- [ ] **Step 5: Run safety tests**

Run: `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml panic_chord` and `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml force_local`

Expected: panic and cleanup tests pass.

### Task 4: Add visible safety controls

**Files:**
- Modify: `apps/desktop-tauri/src-tauri/src/tray.rs`
- Modify: `apps/desktop-tauri/src/main.ts`
- Modify: `README.zh.md`
- Modify: `docs/user-guide.zh.md`

- [ ] **Step 1: Add the tray release action**

Add an always-enabled `立即释放控制（Ctrl+Alt+Esc）` menu item that calls the same local release path as the panic chord.

- [ ] **Step 2: Add the main-window release button and warning**

Show a prominent `立即回到主电脑` action in the overview and mouse pages. State that `Ctrl+Alt+Esc` is processed locally and works even if the remote connection fails.

- [ ] **Step 3: Document the escape path**

Update the Chinese README and user guide with the right-edge entry, left-edge return, emergency shortcut, tray action, and fail-open behavior.

- [ ] **Step 4: Build the web UI**

Run: `npm.cmd run build` in `apps/desktop-tauri`.

Expected: TypeScript and Vite build pass.

### Task 5: Version, package, verify, and deliver

**Files:**
- Modify: `apps/desktop-tauri/package.json`
- Modify: `apps/desktop-tauri/src-tauri/Cargo.toml`
- Modify: `apps/desktop-tauri/src-tauri/Cargo.lock`
- Modify: `apps/desktop-tauri/src-tauri/tauri.conf.json`
- Create: `docs/releases/v0.1.27.md`

- [ ] **Step 1: Bump to v0.1.27 and add release notes**

Update all project-owned version fields and describe the return fix, panic shortcut, tray release, and disconnect fail-open behavior.

- [ ] **Step 2: Run the complete verification suite**

Run: `cargo test --manifest-path apps/desktop-tauri/src-tauri/Cargo.toml`, `npm.cmd run build`, and the repository Python tests.

Expected: all suites pass with zero failures.

- [ ] **Step 3: Build the LAN update package**

Run: `npm.cmd run build:lan-update` in `apps/desktop-tauri`.

Expected: the update directory contains a v0.1.27 installer, manifest, and matching hash.

- [ ] **Step 4: Inspect and commit the exact scope**

Review `git diff --check`, `git status --short`, and the staged diff. Commit only the mouse return, safety controls, documentation, version, and generated update metadata required by the existing release workflow.
