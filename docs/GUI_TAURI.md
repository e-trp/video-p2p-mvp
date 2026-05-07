# Tauri GUI Plan

## Why Tauri

Tauri is a reasonable fit for this project because:

- Rust remains the primary application language
- the desktop shell can stay lightweight
- the UI can evolve independently from the media backend
- platform-specific capture code can still live in Rust/native layers

## Current State

The repository now includes a desktop scaffold in `apps/desktop`:

- static frontend in `apps/desktop/public`
- Tauri Rust shell in `apps/desktop/src-tauri`
- initial commands for reading project status and the saved specification
- session manager hooks for `host`, `join`, `stop`, `status`, and `logs`
- UI controls for config save, host/viewer prep, transport smoke testing, reset, refresh, and log clearing

The backend around the GUI has moved one step forward:

- `signaling-server` can now relay validated SDP/ICE envelopes between peers
- stored signaling messages are replayed to a late joiner in the same room
- the GUI now refreshes through a live signaling path and a real `PeerConnection`
- host-side placeholder audio/video tracks are attached before offer creation and shown in the session snapshot
- host preparation now auto-creates and sends the first local SDP offer once signaling is connected
- a debug command can now push placeholder audio/video samples into those attached tracks
- the GUI now shows a platform capture catalog and allows selecting a source in-session
- the macOS source picker now attempts live application/window enumeration before falling back to blueprint data
- the Linux source picker now attempts `wmctrl`-based runtime window enumeration before falling back to blueprint data
- capture catalog diagnostics now show whether the GUI is using runtime data or blueprint fallback, plus backend notes
- host startup now defaults to the first available capture source when the user has not selected one explicitly
- capture picker and audio-toggle changes now apply immediately without a separate confirmation button
- transport diagnostics now surface `PeerConnection` stage, data-channel readiness, and transport notes

## Intended GUI Responsibilities

1. Window/source selection
2. Room create/join flow
3. Start and stop streaming
4. Stream status and diagnostics
5. Settings for bitrate, fps, and audio routing

## Backend Split

- `app-core`: shared protocol, session state, mock transport today
- `transport-webrtc`: future real peer transport
- `capture-macos`: future ScreenCaptureKit bridge
- `capture-linux`: future Portal + PipeWire integration
- `desktop-app`: Tauri shell and user-facing commands

## Session Management Layer

The current GUI iteration is backed by an in-memory session manager in `app-core`.

It already models:

- idle / host / viewer roles
- configured / awaiting peer / negotiating WebRTC / live WebRTC / mock streaming / stopped stages
- room and signaling metadata
- a rolling in-memory event log

This gives the Tauri shell a stable API before the real transport and capture layers are connected.

## Current User Interface

The current Tauri UI is wired to the session manager and supports:

- editing room and signaling fields
- selecting a platform source and audio preference
- applying capture-source changes immediately from the picker state
- saving session configuration
- preparing host or viewer session states with a live signaling connection
- auto-creating and sending the first local offer from the host path
- publishing placeholder audio/video samples into the attached host tracks
- polling signaling through repeated snapshot refresh
- auto-applying remote offer/answer/ICE during refresh
- stopping or resetting a session
- viewing current session status and next action
- viewing transport connection state, transport stage, bootstrap data-channel readiness, transport notes, local audio/video track attachment, sample counters, local/remote description kind, and ICE counters
- viewing and clearing the rolling session log

## Important Note

The Tauri shell is scaffolded but not included in the default workspace build yet. This avoids making the entire repo dependent on downloading Tauri crates and installing platform prerequisites before the core architecture is in place.
It is now configured so `apps/desktop/src-tauri` can also be checked standalone without adding it to the root workspace default build.
