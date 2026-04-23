# video-p2p-mvp

Workspace for a future Rust desktop application that will stream a chosen window and audio peer-to-peer on macOS and Linux.

The repository now has two layers:

- a working backend MVP with signaling and direct UDP mock media flow
- a Tauri desktop scaffold for the future GUI application

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
- preparing host and viewer sessions
- creating local SDP offer state
- accepting remote SDP answer state
- adding remote ICE candidate state
- stopping a session
- reading session logs
- showing the saved specification

The signaling server now keeps a room alive after pairing and can relay validated SDP/ICE envelopes between peers, including replaying stored signaling history to a late joiner.

The Tauri shell is still not included in the default workspace build yet.

## What Is Still Missing

This is not yet a real screen-sharing application. It does not currently include:

- ScreenCaptureKit integration
- Portal/PipeWire integration
- real WebRTC PeerConnection integration
- real audio/video codecs
- STUN/TURN
- production GUI workflow

What changed relative to the earlier scaffold:

- shared signaling envelope encoding/decoding now exists in `app-core`
- `signaling-server` relays `SIG|SDP|...` and `SIG|ICE|...` lines between peers
- signaling history is replayed when the second peer joins late
- `transport-webrtc` is still a lifecycle model, not a real `PeerConnection`

## Recommended Next Build Steps

1. Add a real `transport-webrtc` crate.
2. Add `capture-macos` using ScreenCaptureKit.
3. Connect the Tauri GUI to backend session management.
4. Add Linux Wayland capture via Portal + PipeWire.
