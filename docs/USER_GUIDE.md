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
- `Source Label`: manual label stored in the session snapshot
- `Save Config`: save the current room/signaling/source label into the in-memory session state
- `Prepare Host`: create a host-side WebRTC session and connect signaling
- `Prepare Viewer`: create a viewer-side WebRTC session and connect signaling
- `Mock Streaming`: switch the session snapshot into the legacy mock UDP stage
- `Arm WebRTC`: move the session snapshot into the live WebRTC negotiation stage
- `Stop`: close the current session
- `Reset`: reset session state back to idle

### Capture Source

- `Available Source`: current platform blueprint source list
- `Include Audio`: stores whether the chosen source should include audio when capture is wired for real
- `Use Source`: selects the source and updates the session source label

Today this picker is backed by example sources from the platform blueprint crates:

- macOS: `ScreenCaptureKit`-shaped sources
- Linux: `Portal + PipeWire`-shaped sources

This is a contract layer only. It is not yet reading the real OS window list.

### WebRTC Signaling

- `Create Offer`: create and send a local SDP offer from the host side
- `Push Placeholder Media`: write placeholder audio/video samples into the attached host tracks
- `Accept Answer`: manually apply a remote SDP answer for debugging
- `Add ICE`: manually apply a remote ICE candidate for debugging

Offer/answer and ICE can already flow automatically through the signaling server during refresh, but the manual fields are still available for debugging.

### Status And Snapshot

The GUI currently shows:

- signaling connectivity
- selected source id and audio flag
- capture backend and permission state
- media track attachment state
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

- capture sources are blueprint/example data, not OS-enumerated windows
- placeholder media is not real encoded screen/audio content
- the GUI still exposes several debug controls
- the signaling service is still a minimal two-peer MVP
