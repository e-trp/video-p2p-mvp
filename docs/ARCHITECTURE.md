# Architecture

## Current MVP

The repository is now a workspace-based scaffold with a future desktop GUI target.

Components:

- `crates/app-core`: shared protocol, CLI parsing, and mock media session logic
- `crates/transport-webrtc`: WebRTC session/state skeleton with SDP/ICE lifecycle modeling
- `crates/capture-macos`: planned ScreenCaptureKit backend blueprint crate
- `crates/capture-linux`: planned Linux capture backend blueprint crate
- `crates/signaling-server`: TCP room coordination between exactly two peers
- `apps/cli`: sender/receiver CLI for testing the direct peer flow
- `apps/desktop`: Tauri desktop shell scaffold

## Current Data Flow

1. `signaling-server` starts and listens on a TCP port.
2. `apps/cli receiver` joins a room and exposes its UDP port.
3. `apps/cli sender` joins the same room and exposes its UDP port.
4. The signaling server exchanges peer endpoints.
5. The sender sends UDP packets directly to the receiver.

## Why This MVP Exists

The real product needs OS-specific capture and WebRTC integration. That is a much larger step than simple project scaffolding. This MVP isolates the parts that are stable today:

- process roles
- room lifecycle
- peer discovery shape
- direct peer delivery semantics
- documentation and repo structure

## Planned Production Modules

### `capture-core`

Cross-platform abstraction:

- list available sources
- select one source
- start and stop capture
- surface audio and video sample callbacks

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
- in-memory transport session state for offer/answer/candidate lifecycle
- Tauri commands that drive this state from the GUI
