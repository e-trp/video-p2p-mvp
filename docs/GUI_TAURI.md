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
- baseline macOS Tauri bundle configuration for `.app` and `.dmg` packaging
- generated desktop icon assets plus helper scripts for icon refresh and release bundle builds
- initial commands for reading project status and the saved specification
- session manager hooks for `host`, `join`, `stop`, `status`, and `logs`
- UI controls for config save, host/viewer prep, transport smoke testing, reset, refresh, and log clearing
- manual QA coverage is now tracked in `docs/QA_CHECKLIST.md`

The backend around the GUI has moved one step forward:

- `signaling-server` can now relay validated SDP/ICE envelopes between peers
- stored signaling messages are replayed to a late joiner in the same room
- the GUI now refreshes through a live signaling path and a real `PeerConnection`
- host-side placeholder audio/video tracks are attached before offer creation and shown in the session snapshot
- host preparation now auto-creates and sends the first local SDP offer once signaling is connected
- a debug command can now push synthetic `capture-core` audio/video payloads into those attached tracks
- the session manager can now ingest backend `capture-core` stream events for native capture implementations without routing them through the GUI debug command
- the GUI now shows a platform capture catalog and allows selecting a source in-session
- the macOS source picker now attempts live application/window enumeration before falling back to blueprint data
- the Linux source picker now attempts `wmctrl`-based runtime window enumeration before falling back to blueprint data
- capture catalog diagnostics now show whether the GUI is using runtime data or blueprint fallback, plus backend notes
- capture permission state now follows the latest runtime probe instead of a fixed scaffold default
- host session logs and next-action hints now react to capture readiness, not only signaling progress
- host source selection now auto-rebinds when a refreshed runtime catalog drops the previously selected source
- host startup now defaults to the first available capture source when the user has not selected one explicitly
- capture picker and audio-toggle changes now apply immediately without a separate confirmation button
- desktop polling is now user-configurable from the GUI, and its enabled/disabled state plus interval persist through the shared session-preferences store
- room/signaling/ICE draft edits now survive background refresh polling until a successful session action re-syncs them
- the first viewport now prioritizes a live Tauri workflow overview for role, signaling, capture runtime, transport, and next action
- live overview values now use semantic state tokens, and backend-provided window labels, notes, and session values are escaped before rendering into HTML
- transport diagnostics now surface `PeerConnection` stage, data-channel readiness, and transport notes
- session config now includes editable ICE server entries that persist through the desktop preferences store
- transport diagnostics now include an ICE path summary derived from candidate-pair stats so the GUI can hint whether the current route looks direct or relay-backed
- the desktop shell now exposes a reconnect action that reuses the active backend session configuration instead of requiring a full reset/re-prepare cycle
- the desktop shell now also surfaces explicit recovery-state diagnostics derived from signaling reachability, stopped state, and peer-connection status
- transport diagnostics now also expose candidate-pair RTT, available bitrate, byte counters, and packet-loss estimates from the live stats report

## Intended GUI Responsibilities

1. Window/source selection
2. Room create/join flow
3. Start and stop streaming
4. Stream status and diagnostics
5. Settings for bitrate, fps, and audio routing

## Backend Split

- `app-core`: shared protocol, session state, signaling orchestration, source-validated capture-burst bridging, and live capture event ingestion
- `transport-webrtc`: real peer transport bootstrap with attached host tracks
- `capture-macos`: planned ScreenCaptureKit bridge
- `capture-linux`: planned Portal + PipeWire integration
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

- editing room, signaling, and ICE server fields
- scanning the live overview for current role, signaling reachability, capture runtime state, transport state, and next action
- reading command feedback as success, warning, or error state after each Tauri action
- tuning persisted auto-refresh polling behavior for the desktop shell
- selecting a platform source and audio preference
- applying capture-source changes immediately from the picker state
- saving session configuration
- preparing host or viewer session states with a live signaling connection
- auto-creating and sending the first local offer from the host path
- reconnecting the current host/viewer session after a stop or signaling failure
- surfacing recovery state, recovery reason, and reconnect availability directly in the GUI
- surfacing RTT, bitrate, byte counters, packet loss, and ICE-path diagnostics directly in the GUI
- publishing synthetic `capture-core` audio/video payloads into the attached host tracks
- polling signaling through repeated snapshot refresh
- auto-applying remote offer/answer/ICE during refresh
- stopping or resetting a session
- viewing current session status and next action
- using a dedicated capture runtime panel for native start/poll/stop and debug capture publishing controls
- viewing the configured ICE server count and summary in the session snapshot
- viewing transport connection state, transport stage, ICE path summary, bootstrap data-channel readiness, transport notes, local audio/video track attachment, sample counters, last payload summaries, local/remote description kind, and ICE counters
- viewing and clearing the rolling session log

## Important Note

The Tauri shell is scaffolded but not included in the default workspace build yet. This avoids making the entire repo dependent on downloading Tauri crates and installing platform prerequisites before the core architecture is in place.
It is now configured so `apps/desktop/src-tauri` can also be checked standalone without adding it to the root workspace default build.
