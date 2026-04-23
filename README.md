# video-p2p-mvp

Workspace for a future Rust desktop application that will stream a chosen window and audio peer-to-peer on macOS and Linux.

The repository now has two layers:

- a working backend MVP with signaling and direct UDP mock media flow
- a Tauri desktop shell that can now drive live WebRTC negotiation state with attached host media tracks

## Workspace Layout

- `crates/app-core`: shared protocol and mock session logic
- `crates/signaling-server`: standalone signaling binary
- `apps/cli`: sender/receiver CLI for backend testing
- `apps/desktop`: Tauri GUI scaffold
- `docs/SPECIFICATION.md`: saved product instruction
- `docs/ARCHITECTURE.md`: architecture notes
- `docs/GUI_TAURI.md`: GUI plan
- `todo.md`: remaining project work by iteration

## Running The Working MVP

Start signaling:

```bash
cargo run -p signaling-server -- 127.0.0.1:7000
```

Start receiver in another terminal:

```bash
cargo run -p p2p-cli -- receiver --room demo --signal 127.0.0.1:7000 --expected-frames 10
```

Start sender in a third terminal:

```bash
cargo run -p p2p-cli -- sender --room demo --signal 127.0.0.1:7000 --frames 10 --fps 5
```

Run the live WebRTC negotiation host:

```bash
cargo run -p p2p-cli -- webrtc-host --room demo --signal 127.0.0.1:7000
```

Run the live WebRTC negotiation viewer:

```bash
cargo run -p p2p-cli -- webrtc-viewer --room demo --signal 127.0.0.1:7000
```

Print the saved project specification:

```bash
cargo run -p p2p-cli -- spec
```

## Tauri GUI Scaffold

The desktop scaffold lives in `apps/desktop`.

- frontend shell: `apps/desktop/public`
- Tauri Rust shell: `apps/desktop/src-tauri`

It now exposes commands for:

- reading project and session status
- preparing host and viewer sessions against the live signaling server
- creating and sending a local SDP offer
- publishing placeholder audio/video samples into the attached host tracks for smoke testing
- polling signaling and auto-applying remote offer/answer/ICE state
- accepting manual remote SDP answer state for debugging
- adding manual remote ICE candidate state for debugging
- stopping a session
- reading session logs
- showing the saved specification

The signaling server now keeps a room alive after pairing and relays validated SDP/ICE envelopes between peers, including replaying stored signaling history to a late joiner.
`transport-webrtc` is no longer an in-memory lifecycle stub: it now creates a real `RTCPeerConnection`, attaches placeholder host-side audio/video tracks, keeps a bootstrap data channel for control traffic, gathers ICE candidates, and exposes real connection state.

The Tauri shell is still not included in the default workspace build yet.

## What Is Still Missing

This is not yet a real screen-sharing application. It does not currently include:

- ScreenCaptureKit integration
- Portal/PipeWire integration
- real captured samples flowing through the attached tracks
- real audio/video codecs
- STUN/TURN
- production GUI workflow

What changed relative to the earlier scaffold:

- shared signaling envelope encoding/decoding now exists in `app-core`
- `signaling-server` relays `SIG|SDP|...` and `SIG|ICE|...` lines between peers
- signaling history is replayed when the second peer joins late
- `app-core` now has a live signaling client used by CLI and Tauri session flow
- CLI now has `webrtc-host` and `webrtc-viewer` commands for real negotiation
- `transport-webrtc` now wraps a real `PeerConnection` with attached placeholder audio/video tracks, ICE gathering, and connection-state snapshots
- session snapshots and the Tauri UI now expose local media-track attachment state
- session manager and transport now expose placeholder media-sample publishing state

## Recommended Next Build Steps

1. Replace placeholder media samples with real captured audio/video input.
2. Add `capture-macos` using ScreenCaptureKit.
3. Replace the remaining manual/debug signaling fields in the Tauri GUI with production UX.
4. Add Linux Wayland capture via Portal + PipeWire.
