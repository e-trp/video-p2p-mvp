# video-p2p-mvp

Workspace for a future Rust desktop application that will stream a chosen window and audio peer-to-peer on macOS and Linux.

The repository now has two layers:

- a working backend MVP with signaling and direct UDP mock media flow
- a Tauri desktop shell that can now drive live WebRTC negotiation state with attached host media tracks

## Workspace Layout

- `crates/app-core`: shared protocol and mock session logic
- `crates/capture-core`: shared capture types for sources, permissions, video frames, and audio buffers
- `crates/signaling-server`: standalone signaling binary
- `apps/cli`: sender/receiver CLI for backend testing
- `apps/desktop`: Tauri GUI scaffold
- `docs/SPECIFICATION.md`: saved product instruction
- `docs/USER_GUIDE.md`: how to run the Tauri GUI and what works today
- `docs/INSTALLATION.md`: setup, release build, and troubleshooting notes
- `docs/QA_CHECKLIST.md`: manual QA checklist for current MVP behavior
- `docs/NETWORKING.md`: signaling, STUN/TURN, deployment, and security assumptions
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

Run the host with explicit STUN/TURN entries:

```bash
cargo run -p p2p-cli -- webrtc-host --room demo --signal 127.0.0.1:7000 \
  --ice-server 'stun:stun.l.google.com:19302' \
  --ice-server 'turn:turn.example.com:3478?transport=udp|username|credential'
```

Run the host and publish one debug capture burst after the peer connection comes up:

```bash
cargo run -p p2p-cli -- webrtc-host --room demo --signal 127.0.0.1:7000 --push-debug-capture
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
- listing current capture sources and selecting one in-session
- applying capture source changes immediately from the picker
- defaulting the host session to the first available capture source when none was selected explicitly
- persisting last-used room, signaling address, and capture-source selection across session-manager restarts
- persisting desktop auto-refresh enablement and polling interval across restarts
- preparing host and viewer sessions against the live signaling server
- configuring custom ICE server entries for STUN/TURN-assisted traversal
- auto-creating and sending the first local SDP offer from the host once signaling is connected
- publishing debug `capture-core` audio/video payloads into the attached host tracks for smoke testing
- starting, polling, and stopping the current platform capture stream runtime from the session manager and desktop GUI
- polling signaling with a persisted auto-refresh cadence and auto-applying remote offer/answer/ICE state
- reconnecting the current host/viewer session with the active room, signaling, and ICE settings after a stop or signaling failure
- surfacing explicit recovery-state diagnostics in the desktop UI so reconnect guidance is visible without digging through logs
- surfacing live transport diagnostics from the `PeerConnection` snapshot, including ICE path, RTT, bitrate, byte counters, and packet-loss metrics when available
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
- real source enumeration from the operating system
- real captured samples flowing through the attached tracks
- real audio/video codecs
- bundled STUN/TURN deployment defaults
- production GUI workflow
- production signing/notarization flow

What changed relative to the earlier scaffold:

- shared signaling envelope encoding/decoding now exists in `app-core`
- `signaling-server` relays `SIG|SDP|...` and `SIG|ICE|...` lines between peers
- signaling history is replayed when the second peer joins late
- `app-core` now has a live signaling client used by CLI and Tauri session flow
- CLI now has `webrtc-host` and `webrtc-viewer` commands for real negotiation
- `transport-webrtc` now wraps a real `PeerConnection` with attached placeholder audio/video tracks, ICE gathering, and connection-state snapshots
- the CLI host path now relies on the same automatic first-offer behavior as the GUI instead of forcing a second manual offer attempt
- host session refresh now auto-creates its local offer when signaling is available
- session snapshots and the Tauri UI now expose local media-track attachment state
- session manager and transport now expose capture-payload publishing state and payload summaries
- session manager now exposes source-validated capture video/audio publishing APIs above `transport-webrtc`, and the debug burst uses that same path
- `capture-core` now defines live capture stream config/status/event contracts, and `app-core` can ingest those events into the same source-validated WebRTC publishing path
- `capture-core` now also exposes a shared capture stream runtime trait, with planned macOS/Linux runtime scaffolds ready for native bridge implementations
- `app-core` now owns the active platform capture runtime lifecycle and can start/poll/stop it through the same event ingestion path used by future native media samples
- session snapshots and the Tauri UI now expose transport stage, bootstrap data-channel state, and transport notes
- session snapshots and the Tauri UI now expose native capture runtime status alongside capture permission state
- native capture runtime start planning now preserves denied permission as `permission_denied` instead of folding it into `permission_required`
- the CLI host path can now push one debug `capture-core` burst after the peer connection connects
- `capture-core` now provides shared Rust-side contracts for capture sources, permission state, and raw media payloads
- the Tauri shell now exposes capture backend, permission state, and source selection from `app-core`
- macOS capture-source listing now attempts runtime application/window enumeration and falls back to blueprint data when the environment blocks it
- Linux capture-source listing now attempts `wmctrl`-based runtime window enumeration and falls back to blueprint data when the environment blocks it
- capture catalog diagnostics now surface runtime-vs-fallback origin and backend notes in the GUI
- capture permission state is now derived from runtime probing and refreshed with the catalog instead of staying a static scaffold value
- host session guidance now reacts to capture permission readiness instead of only signaling/transport state
- host source selection now automatically rebinds to the first available source if a refreshed runtime catalog drops the previously selected source
- session-manager preferences now persist room/signaling/source metadata across resets and Tauri restarts
- session-manager preferences now also persist custom ICE server entries for later host/viewer reconnects
- session-manager preferences now also persist desktop auto-refresh enablement and refresh interval
- desktop session-form drafts now survive background refresh polling until a successful save/start/reset syncs them explicitly
- the desktop GUI now opens with a compact live overview for role, signaling, capture runtime, transport, and next action before the detailed diagnostics
- the desktop GUI now renders backend-provided labels, notes, and session fields through escaped detail rows with semantic state tokens in the live overview
- the desktop GUI now starts with prototype-first host/viewer workflow panels, including a host action that prepares signaling, attempts native capture, and sends a test capture frame
- the desktop GUI now serializes button actions behind a busy state so manual commands do not compete with background refresh polling
- the desktop GUI now shows a first-screen prototype readiness strip for room, source, native capture, signaling, and peer-link state
- the desktop shell now exposes a reconnect action that rebuilds the current host/viewer session using the active backend configuration
- the desktop shell now exposes native capture runtime start/poll/stop controls above the platform runtime scaffolds
- session snapshots and the desktop GUI now also expose recovery state, recovery reason, and whether reconnect is currently available or recommended
- transport snapshots and the desktop GUI now also expose selected-candidate RTT, available incoming/outgoing bitrate, candidate-pair byte counters, and packet-loss estimates from remote inbound RTP stats
- transport snapshots and the Tauri UI now surface an ICE path summary derived from candidate-pair stats, including direct-vs-relay hints when a pair is selected
- the desktop Tauri config now enables macOS `.app` and `.dmg` bundle targets with a real bundle identifier and baseline metadata
- desktop release support now includes generated Tauri icon assets, package metadata, and helper scripts for icon refresh plus `cargo tauri build`
- installation, saved-preferences, and troubleshooting guidance now live in a dedicated `docs/INSTALLATION.md`
- networking and deployment assumptions now live in `docs/NETWORKING.md`

## Recommended Next Build Steps

1. Replace debug `capture-core` media bursts with real captured audio/video input.
2. Implement the `capture-macos` ScreenCaptureKit bridge on top of `capture-core`.
3. Wire native capture stream events into the existing `SessionManager::ingest_capture_stream_event` path with real permission-aware capture/session lifecycle control.
4. Add Linux Wayland capture via Portal + PipeWire on top of `capture-core`.
