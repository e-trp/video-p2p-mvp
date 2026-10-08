# User Guide

## What This Project Can Do Today

This repository is still an MVP. Today it can already provide:

- a TCP signaling server for room pairing and WebRTC signaling relay
- a Tauri desktop GUI for host/viewer session control
- a live WebRTC negotiation path between host and viewer
- configurable ICE server entries for custom STUN/TURN traversal
- codec-aware H.264/Opus host audio/video tracks
- debug `capture-core` media payload publishing for transport smoke testing
- native macOS ScreenCaptureKit capture and continuous H.264/Opus WebRTC publication with separate raw/encoded/received counters
- a source picker fed by current platform capture metadata, with runtime enumeration when available and blueprint fallback otherwise

macOS capture, encoding, and publication are implemented; Linux capture/video encoding, viewer playback, bundled STUN/TURN deployment defaults, and production invite/auth flows remain pending. See [macOS capture](MACOS_CAPTURE.md) for setup and validation.

For setup, release build notes, and troubleshooting, see `docs/INSTALLATION.md`.
For manual validation before a release candidate, see `docs/QA_CHECKLIST.md`.

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
cd apps/desktop/src-tauri
cargo tauri dev
```

This launches the desktop shell from `apps/desktop`.

## Build A Desktop Release Bundle

Refresh the generated icon assets:

```bash
./scripts/generate-desktop-icons.sh
```

Build the Tauri release bundle:

```bash
./scripts/build-desktop-release.sh
```

This regenerates the desktop icons, changes into `apps/desktop/src-tauri`, and runs `cargo tauri build`.

## Current GUI Workflow

The first screen now starts with a live overview for the current role, signaling reachability, capture runtime state, transport state, and next action. The detailed snapshot and logs remain lower on the page for diagnostics.

### Session Control

- `Room`: room name for host/viewer pairing
- `Signaling`: TCP address of the signaling server
- `ICE Servers`: optional newline-separated STUN/TURN entries using `url` or `url|username|credential`
  Accepted URL schemes are `stun:`, `stuns:`, `turn:`, and `turns:`.
- `Auto Refresh`: enable or disable the background polling loop that refreshes session, signaling, and capture state
- `Refresh Every`: choose the persisted polling interval used by that background refresh loop
- `Save Config`: save the current room, signaling address, and ICE server list into the local session-preferences file used by the desktop shell
- `Prepare Host`: create a host-side WebRTC session, connect signaling, and automatically send the first local offer when signaling is available
- `Prepare Viewer`: create a viewer-side WebRTC session and connect signaling
- `Reconnect`: rebuild the current host or viewer session with the active room, signaling address, and ICE server configuration
- `Stop`: close the current session
- `Reset`: reset session state back to idle

While auto-refresh is running, the desktop shell now keeps in-progress `Room`, `Signaling`, and `ICE Servers` edits intact instead of overwriting them with the latest snapshot on every poll. Those fields are synchronized explicitly after successful save/start/reset actions.

If those fields have unsaved edits when you click `Reconnect`, the GUI first saves the edited room/signaling/ICE values into the session manager and then starts the reconnect attempt with that updated configuration.

### Capture Source

- `Available Source`: current platform blueprint source list
- `Include Audio`: selects whether native capture includes audio when the source supports it
- changing the picker or audio toggle applies the host-side capture selection immediately
- the most recent room, signaling address, source selection, and auto-refresh preferences are restored automatically on the next desktop launch or session-manager reset

If you never touch the picker before starting a host session, the first available source is selected automatically.

Today this picker is backed by:

- macOS: SCShareableContent windows, applications, and displays with native IDs; blueprint fallback when permission or enumeration is unavailable
- Linux: runtime X11 window enumeration through `wmctrl` when available, with a fallback to `Portal + PipeWire`-shaped blueprint sources

On macOS, starting capture validates Screen Recording permission and creates a native SCStream for the selected source. Video arrives as packed BGRA, and audio on macOS 13+ arrives as 48 kHz interleaved float PCM. A worker encodes video as H.264 and audio as Opus and publishes after WebRTC connects. The capture panel shows raw diagnostics, encoded packet counts, and received RTP counts. Viewer decoding/playback is still pending. Disabling GUI refresh does not pause capture or publication after negotiation.
Changing the source or audio toggle stops the current stream; click `Start Capture` to capture the new selection. Blueprint sources support the local demo/debug flow but cannot start native capture. Linux still uses metadata discovery and a planned runtime.

### Transport Diagnostics

The GUI now surfaces transport-side diagnostics from the Rust `PeerConnection` wrapper:

- current transport stage
- current ICE path summary, including direct-vs-relay hints when a candidate pair is selected
- current candidate-pair RTT
- available outgoing and incoming bitrate reported by the selected candidate pair
- candidate-pair payload byte counters for sent and received traffic
- packet-loss percentage and lost-packet count when remote inbound RTP stats are available
- bootstrap data channel readiness
- current stats report count
- transport notes describing track attachment and sample counters

The capture panel also now shows:

- backend label
- catalog origin (`runtime`, `blueprint_fallback`, or `unavailable`)
- permission state derived from the current runtime probe
- native capture runtime status (`not_started`, `starting`, `running`, `permission_required`, `permission_denied`, `failed`, or `stopped`)
- backend notes explaining runtime enumeration success or fallback reason

This data is refreshed through the session manager rather than staying fixed from initial app startup.

When you prepare a host session, the session log and next-action hint now also react to capture readiness:

- `granted`: the host catalog is ready for real source selection
- `required`: OS permission is still needed or the app is still running on fallback metadata
- `denied`: host capture needs explicit OS approval
- `unknown`: the desktop session or capture tooling could not be verified cleanly

When capture stream startup sees denied platform permission, the native runtime status stays distinct as `permission_denied` instead of being reported as another `permission_required` state.

If the selected host source disappears after a later catalog refresh, the session manager now rebinds to the first available source automatically and records that change in the session log.

### Recovery

The GUI now surfaces recovery diagnostics directly instead of leaving recovery guidance only in logs or `Next Action` text:

- `healthy`: the peer connection is live
- `negotiating`: signaling and transport setup are still in progress
- `signaling_unavailable`: signaling is down or unreachable; reconnect is recommended after fixing reachability
- `stopped`: the session was stopped intentionally; reconnect can rebuild it
- `transport_disconnected`, `transport_failed`, `transport_closed`: the peer connection degraded or shut down; reconnect is recommended

The `Reconnect` button is disabled only when no host/viewer session has been prepared yet. When recovery is recommended, the reconnect button is also highlighted in the session controls.

### Transport Smoke Test

- the capture runtime panel controls native capture start, poll, and stop, and reports `Captured Video`, `Captured Audio`, `Native Video`, `Native Audio`, `Encoded H.264`, `Encoded Opus`, `Received Video RTP`, and `Received Audio RTP`
- `Push Debug Capture Burst`: synthesize `capture-core` video/audio payloads and write them into the attached host tracks

The debug burst respects the selected source audio toggle:

- if `Include Audio` is enabled, both video and audio payload summaries are updated
- if `Include Audio` is disabled, only the video payload is published during the smoke test

Both debug publication and native sample diagnostics validate source selection and audio opt-in:

- the selected source id must match the payload source being published
- audio payloads are rejected when the current selection has `Include Audio` turned off

The backend also now accepts live `capture-core` stream events through `SessionManager::ingest_capture_stream_event`. The continuous worker validates source identity/audio opt-in and encodes before transport publication. It retains only the latest raw video/audio event for this diagnostic API, so native diagnostic counts depend on refresh cadence while encoded counters reflect all published packets.

Host-side offer creation, answer delivery, and ICE flow automatically through the signaling server during refresh. Native capture and debug packetization have separate controls and counters.

If signaling dies or you stop a session intentionally, the `Reconnect` action reinitializes the current host/viewer role instead of forcing a full `Reset` followed by another manual prepare step.

### Status And Snapshot

The GUI currently shows:

- signaling connectivity
- selected source id and audio flag
- capture backend and permission state
- recovery state, recovery reason, and reconnect readiness
- transport stage and bootstrap data-channel state
- transport RTT, bitrate, byte counters, packet-loss metrics, and ICE-path summary
- media track attachment state
- transport notes and stats report count
- published debug sample counters and last payload summaries
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

Host with one debug capture burst after connect:

```bash
cargo run -p p2p-cli -- webrtc-host --room demo --signal 127.0.0.1:7000 --push-debug-capture
```

Host with explicit STUN/TURN:

```bash
cargo run -p p2p-cli -- webrtc-host --room demo --signal 127.0.0.1:7000 \
  --ice-server 'stun:stun.l.google.com:19302' \
  --ice-server 'turn:turn.example.com:3478?transport=udp|username|credential'
```

Viewer:

```bash
cargo run -p p2p-cli -- webrtc-viewer --room demo --signal 127.0.0.1:7000
```

## Current Limits

- Linux runtime enumeration currently depends on X11-style `wmctrl` metadata and does not cover the real Wayland portal flow yet
- macOS native capture needs Screen Recording access; audio needs macOS 13+; real permission/source/audio behavior still needs manual QA
- debug `capture-core` payload bursts are synthetic transport smoke tests; native sample counters are separate
- encoded media reaches the viewer, but remote picture/sound still require viewer decoding and rendering
- the GUI still falls back to blueprint/example capture sources when runtime enumeration is unavailable
- ICE server configuration is manual; there is still no bundled TURN service or auth flow
- the signaling service is still a minimal two-peer MVP
