# Architecture

## Current MVP

The repository is now a workspace-based scaffold with a future desktop GUI target.

Components:

- `crates/app-core`: shared protocol, CLI parsing, live signaling client, and session orchestration
- `crates/capture-core`: shared capture-domain types for source selection, permissions, audio buffers, and video frames
- `crates/transport-webrtc`: real `RTCPeerConnection` bootstrap with attached host media tracks and control data channel
- `crates/capture-macos`: planned ScreenCaptureKit backend blueprint crate
- `crates/capture-linux`: planned Linux capture backend blueprint crate
- `crates/signaling-server`: TCP room coordination between exactly two peers
- `apps/cli`: sender/receiver CLI for testing the direct peer flow
- `apps/desktop`: Tauri desktop shell scaffold

## Current Data Flow

1. `signaling-server` starts and listens on a TCP port.
2. `app-core` exposes the current platform capture catalog and selected source metadata to the GUI.
   On macOS, that catalog now attempts runtime application/window enumeration before falling back to blueprint data.
   On Linux, it now attempts `wmctrl`-based runtime window enumeration before falling back to blueprint data.
3. A host/viewer session joins a room through `app-core::SignalingConnection`.
4. `transport-webrtc` creates a real `PeerConnection`, attaches placeholder host audio/video tracks, and keeps a bootstrap data channel for control flow.
5. Local SDP/ICE is encoded through shared protocol messages and sent through `signaling-server`.
6. The peer receives signaling messages, and the GUI session refresh path auto-applies remote answer/ICE through the same signaling channel.
7. Mock UDP sender/receiver flow still exists separately for the old media scaffold.

The current signaling server also accepts future WebRTC signaling envelopes after pairing:

- `SIG|SDP|...` session descriptions
- `SIG|ICE|...` ICE candidates
- replay of stored signaling messages to a late-joining room participant

## Why This MVP Exists

The real product needs OS-specific capture and WebRTC integration. That is a much larger step than simple project scaffolding. This MVP isolates the parts that are stable today:

- process roles
- room lifecycle
- peer discovery shape
- direct peer delivery semantics
- documentation and repo structure

## Planned Production Modules

### `capture-core`

Cross-platform capture contracts that now exist in code:

- source and permission metadata
- capture selection payloads
- raw video frame and audio buffer shapes
- a shared Rust model that macOS/Linux backends can target

### `capture-macos`

- native ScreenCaptureKit bridge
- video frame extraction
- audio sample extraction
- permission handling

### `capture-linux-wayland`

- portal session lifecycle
- PipeWire stream setup
- frame and audio sample handling

### `transport-webrtc`

- peer connection lifecycle
- audio/video track publishing
- encoded sample injection into attached local tracks
- ICE candidate exchange
- statistics and reconnect handling

### `desktop-app`

- Tauri shell
- source picker
- room code / invite UI
- stream status
- diagnostics

## Non-Goals Of The Current MVP

- no attempt at production NAT traversal
- no media relay
- no persistence layer
- no encryption beyond what the transport would later provide

## Current Signaling Progress

The codebase now contains a minimal signaling model for future WebRTC:

- session description messages for `offer` and `answer`
- ICE candidate messages
- signaling server relay for validated SDP/ICE envelopes between room participants
- signaling history replay for a late-joining peer in the same room
- a live signaling client in `app-core` used by CLI and Tauri session flow
- a capture catalog in `app-core` that surfaces permission state and platform source metadata, including runtime macOS and Linux enumeration with blueprint fallback
- capture catalog origin and backend notes now flow through `app-core` into the Tauri GUI for operator diagnostics
- session refresh now re-synchronizes the capture catalog so permission/origin changes are not stuck at process startup
- host session guidance now branches on capture readiness before falling through to signaling-only advice
- host capture selection now self-heals to the first available source when runtime catalog refresh invalidates the previous selection
- a real `PeerConnection` bootstrap path with attached placeholder audio/video tracks, connection-state snapshots, and ICE gathering
- Tauri commands and session snapshots that surface local media-track state and placeholder sample publish counters in the GUI
- the main Tauri GUI path now relies on automatic signaling refresh instead of exposing manual answer/ICE controls
- `capture-core` as the shared Rust-side model for future native capture backends
