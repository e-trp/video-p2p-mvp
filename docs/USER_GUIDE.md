# User Guide

## What This Project Can Do Today

This repository is still an MVP. Today it can already provide:

- a TCP signaling server for room pairing and WebRTC signaling relay
- a Tauri desktop GUI for host/viewer session control
- a live WebRTC negotiation path between host and viewer
- attached placeholder host audio/video tracks
- placeholder media sample publishing for transport smoke testing
- a source picker fed by the current `capture-core` platform blueprint data

It does **not** yet provide real OS screen capture, real system audio capture, STUN/TURN, or production invite/auth flows.

## Requirements

- Rust toolchain with `cargo`
- Tauri CLI available as `cargo tauri`
- desktop platform prerequisites required by Tauri on your OS

You can verify the Tauri CLI is available:

```bash
cargo tauri --help
```

## Run The Signaling Server

Open one terminal in the repository root:

```bash
cargo run -p signaling-server -- 127.0.0.1:7000
```

Keep it running while you test the GUI or CLI peers.

## Run The Tauri GUI

Open another terminal in the repository root:

```bash
cargo tauri dev --manifest-path apps/desktop/src-tauri/Cargo.toml
```

This launches the desktop shell from `apps/desktop`.

## Current GUI Workflow

### Session Control

- `Room`: room name for host/viewer pairing
- `Signaling`: TCP address of the signaling server
- `Save Config`: save the current room and signaling address into the in-memory session state
- `Prepare Host`: create a host-side WebRTC session, connect signaling, and automatically send the first local offer when signaling is available
- `Prepare Viewer`: create a viewer-side WebRTC session and connect signaling
- `Stop`: close the current session
- `Reset`: reset session state back to idle

### Capture Source

- `Available Source`: current platform blueprint source list
- `Include Audio`: stores whether the chosen source should include audio when capture is wired for real
- changing the picker or audio toggle applies the host-side capture selection immediately

If you never touch the picker before starting a host session, the first available source is selected automatically.

Today this picker is backed by:

- macOS: runtime application/window enumeration when available, with a fallback to `ScreenCaptureKit`-shaped blueprint sources
- Linux: runtime X11 window enumeration through `wmctrl` when available, with a fallback to `Portal + PipeWire`-shaped blueprint sources

This is still a contract layer overall. Both platform catalogs are best-effort metadata enumeration, not real capture sessions, and they fall back to blueprint data when the host environment blocks runtime discovery.

### Transport Diagnostics

The GUI now surfaces transport-side diagnostics from the Rust `PeerConnection` wrapper:

- current transport stage
- bootstrap data channel readiness
- current stats report count
- transport notes describing track attachment and sample counters

### Transport Smoke Test

- `Push Placeholder Media`: write placeholder audio/video samples into the attached host tracks

Host-side offer creation, answer delivery, and ICE now flow automatically through the signaling server during refresh. The remaining manual control is only there to smoke-test media publication before real capture is wired.

### Status And Snapshot

The GUI currently shows:

- signaling connectivity
- selected source id and audio flag
- capture backend and permission state
- transport stage and bootstrap data-channel state
- media track attachment state
- transport notes and stats report count
- published placeholder sample counters
- local/remote SDP state
- ICE counters
- session logs

## Browser Preview vs Tauri Runtime

If you open the static frontend without Tauri, the page renders in preview mode:

- no Rust commands are executed
- no session state changes happen
- no signaling actions are sent

The real workflow requires running the Tauri app.

## Optional CLI Smoke Tests

You can also use the CLI binaries directly.

Host:

```bash
cargo run -p p2p-cli -- webrtc-host --room demo --signal 127.0.0.1:7000
```

Viewer:

```bash
cargo run -p p2p-cli -- webrtc-viewer --room demo --signal 127.0.0.1:7000
```

## Current Limits

- Linux runtime enumeration currently depends on X11-style `wmctrl` metadata and does not cover the real Wayland portal flow yet
- macOS source enumeration is metadata-only and still does not capture real media
- placeholder media is not real encoded screen/audio content
- the GUI still falls back to blueprint/example capture sources when runtime enumeration is unavailable
- the signaling service is still a minimal two-peer MVP
