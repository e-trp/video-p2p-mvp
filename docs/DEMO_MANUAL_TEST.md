# Demo Manual Test

Use this checklist to verify the current desktop prototype from a clean user flow.

## 1. Start The Desktop App

Run:

```bash
cd apps/desktop/src-tauri
cargo tauri dev
```

Expected:

- the app window opens
- the runtime badge says `Tauri runtime connected`
- the first screen renders Host, Viewer, Prototype readiness, and Live overview blocks
- the UI should not stay forever on `loading...`

## 2. Run The No-Network Demo

Click `Run Local Demo`.

Expected:

- command result says `local desktop demo started without signaling server setup`
- Host panel shows a selected source
- Host panel shows peer as `local viewer demo`
- readiness shows room as `local-demo`
- readiness shows source as `selected`
- readiness shows peer link as `active`
- live overview shows `host / mock_streaming`
- session log contains `local GUI demo started without external signaling or peer setup`

This path does not require a signaling server, a second app instance, STUN/TURN, or real screen-capture permission.

## 3. Check Basic Controls

Click `Stop`.

Expected:

- command result says `session stopped`
- live overview shows stopped state
- recovery panel says reconnect is recommended

Click `Reset`.

Expected:

- command result says `session reset to idle state`
- live overview returns to idle/configure state
- reconnect becomes unavailable

Click `Run Local Demo` again.

Expected:

- the local demo starts again without restarting the app

## 4. Check Source Selection

Open `Capture Source`.

Expected:

- at least one source is listed
- if runtime source enumeration is unavailable, blueprint fallback sources are listed
- catalog notes explain whether runtime or fallback enumeration was used

Change `Available Source`.

Expected:

- command result says `capture source selected`
- Host panel source updates
- readiness source remains `selected`

## 5. Optional Real Signaling Smoke Test

Start signaling in a separate terminal:

```bash
cargo run -p signaling-server -- 127.0.0.1:7000
```

In the desktop app, leave room as `demo` and signaling as `127.0.0.1:7000`, then click `Start Host Prototype`.

Expected:

- the UI shows an in-progress command state and then returns control
- command result says host session was prepared
- live overview shows host mode
- signaling readiness shows `connected`
- session log says local SDP offer was created and sent automatically, or gives a concrete warning if native capture is still unavailable

To test a viewer, open a second app instance or use the CLI viewer:

```bash
cargo run -p p2p-cli -- webrtc-viewer --room demo --signal 127.0.0.1:7000
```

Expected:

- host/viewer signaling messages appear in the session log
- transport diagnostics move beyond initial/not-initialized state

## 6. Failure Notes To Report

If the app still appears stuck, capture these details:

- the exact text still showing `loading...`
- whether `Run Local Demo` is clickable
- the last terminal lines from `cargo tauri dev`
- the OS and desktop environment
- whether the app asked for Accessibility or Screen Recording permission
