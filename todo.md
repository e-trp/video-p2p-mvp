# TODO

## Iteration 1: Real WebRTC Transport

1. Replace the current in-memory WebRTC state skeleton with a real `PeerConnection`.
   Current state: done for bootstrap negotiation; a real `PeerConnection` and data channel are created in `transport-webrtc`.
2. Map session-facing APIs onto actual transport actions:
   `create_offer`, `accept_answer`, `add_ice_candidate`, `connection_state`, `stats`.
   Current state: done for offer/answer/ICE, connection-state snapshots, and debug `capture-core` sample publishing into attached host audio/video tracks.
3. Replace mock UDP and placeholder WebRTC transitions with real transport state transitions.
   Current state: session manager, CLI, and Tauri now use live signaling and transport negotiation; mock UDP still exists as a separate legacy media scaffold.
4. Wire the signaling server to exchange SDP/ICE instead of only room pairing.
   Current state: room relay and signaling replay are implemented, and end-to-end use now exists for CLI and Tauri session flow.
5. Add integration tests for host/viewer negotiation state and signaling replay.

### Iteration 1 Progress

- Done in part: protocol envelopes for SDP/ICE in `app-core`
- Done in part: signaling relay and replay in `signaling-server`
- Done in part: real WebRTC peer transport instead of lifecycle stubs
- Done in part: CLI and Tauri integration with the live signaling path
- Done in part: integration-style coverage for late-join signaling replay and host/viewer WebRTC negotiation in `app-core`
- Done in part: host-side placeholder audio/video tracks are attached to the `PeerConnection`
- Done in part: session-facing debug `capture-core` sample publishing now exists above the transport
- Done in part: CLI host startup now relies on the same automatic first-offer path as the GUI and can optionally publish a debug capture burst after connect
- Still open: feeding real captured samples into the attached tracks and retiring the bootstrap-only path

## Iteration 2: macOS Capture Backend

1. Add a native bridge for `ScreenCaptureKit`.
2. Enumerate shareable windows and applications.
3. Request and validate Screen Recording permission.
4. Capture video frames from the selected window.
5. Capture available audio buffers and normalize them for transport.
6. Feed captured samples into the WebRTC publishing pipeline.

### Iteration 2 Progress

- Done in part: `capture-core` now exists for shared source, permission, video-frame, and audio-buffer types
- Done in part: `capture-core` now also defines live capture stream config/status/event contracts for native backends to emit video/audio samples without using the debug burst path
- Done in part: `capture-core` now exposes a common capture stream runtime trait, and macOS/Linux crates provide planned runtime scaffolds that emit structured status/stop events until native bridges are implemented
- Done in part: `capture-macos` now exposes a best-effort runtime application/window catalog with blueprint fallback
- Done in part: runtime probing on macOS now maps catalog results into live permission-state diagnostics
- Done in part: Tauri/app-core now surface the current platform capture catalog and selected source metadata
- Done in part: session manager and `transport-webrtc` now accept debug `capture-core` video/audio payloads, including source-audio opt-out during smoke tests
- Done in part: `app-core::SessionManager` now exposes source-validated capture video/audio publish APIs that future native backends can target directly
- Done in part: `app-core::SessionManager` now ingests live `capture-core` stream events and maps media events into the same source-validated WebRTC publishing pipeline
- Done in part: `app-core::SessionManager` now owns a platform capture runtime lifecycle, with start/poll/stop APIs that drain runtime events into the same capture ingestion path
- Done in part: `capture-macos` now preflights macOS Screen Recording permission through CoreGraphics, validates the selected runtime source before stream start, and reports permission-vs-bridge-unavailable states separately
- Done in part: `capture-macos` now requests Screen Recording access during native stream start when preflight reports permission is still required
- Done in part: `capture-macos` now has a native ScreenCaptureKit bridge lifecycle boundary that owns start/poll/stop events separately from catalog and permission validation
- Done in part: `capture-macos` now normalizes selected source, audio opt-in, target FPS, and max frame dimensions before handing stream startup to the native bridge boundary
- Done in part: `capture-macos` now passes catalog-backed source kind, display name, app name, and audio capability into native bridge settings so future ScreenCaptureKit target matching does not depend on UI labels
- Done in part: `capture-macos` now has injected bridge-boundary tests for normalized settings, poll-event status updates, and native bridge startup failures
- Done in part: macOS and Linux stream start planning now preserve denied capture permission as a distinct runtime status for GUI diagnostics
- Still open: the actual ScreenCaptureKit bridge, permission flow, and live sample delivery

## Iteration 3: Linux Capture Backend

1. Add Wayland path via `XDG Desktop Portal ScreenCast`.
2. Integrate PipeWire stream consumption for frames and audio.
3. Model permission and source selection flow in the session manager.
4. Add X11 fallback for unsupported environments.
5. Test on at least GNOME Wayland and KDE Wayland.

### Iteration 3 Progress

- Done in part: `capture-linux` now exposes a best-effort `wmctrl`-based runtime window catalog with blueprint fallback
- Done in part: runtime probing on Linux now maps catalog results into live permission-state diagnostics
- Done in part: `capture-linux` now has a planned Portal/PipeWire runtime scaffold implementing the shared capture stream runtime contract
- Done in part: `capture-linux` now validates selected sources against the current catalog and routes startup through a planned Portal/PipeWire bridge boundary with normalized stream settings
- Done in part: Linux stream start planning now preserves denied portal/capture permission separately from required permission
- Still open: real portal session lifecycle, PipeWire media consumption, and robust Wayland coverage

## Iteration 4: Tauri Production Flow

1. Replace mock session actions in the GUI with real backend session orchestration.
2. Add source picker UI backed by macOS/Linux capture enumeration.
3. Add room creation and invite code UX.
4. Add connection diagnostics:
   transport state, RTT, bitrate, packet loss, ICE path.
5. Add start, stop, reconnect and recovery states in the GUI.

### Iteration 4 Progress

- Done in part: GUI session actions already drive real signaling, negotiation, and debug capture media publishing
- Done in part: GUI transport smoke test now drives a debug `capture-core` media bridge instead of hardcoded anonymous sample bytes
- Done in part: the debug GUI publish path now uses the same source-validated session ingest API planned for future native capture backends
- Done in part: session orchestration now has a production-facing live capture event ingestion API, so native backend events can enter the pipeline without going through GUI debug publishing
- Done in part: GUI session actions can now start, poll, and stop the platform capture runtime scaffold instead of only pushing synthetic debug samples
- Done in part: session snapshots and the desktop GUI now expose native capture runtime status directly instead of requiring log inspection
- Done in part: source picker UI now exists, backed by the current platform capture catalog blueprint data
- Done in part: GUI now surfaces transport-stage, data-channel, and transport-note diagnostics from the live WebRTC layer
- Done in part: GUI now surfaces capture-catalog origin and backend notes for runtime-vs-fallback diagnostics
- Done in part: desktop GUI now prioritizes a live workflow overview and a dedicated capture runtime panel before raw diagnostic snapshots
- Done in part: session refresh now re-synchronizes capture-catalog permission/origin state instead of freezing it at startup
- Done in part: host session guidance now reacts to capture permission readiness before falling back to signaling-only hints
- Done in part: host source selection now auto-rebinds when runtime catalog refresh invalidates the previous source
- Done in part: host session flow now auto-creates and sends its first local offer once signaling is connected
- Done in part: host session flow now defaults to the first available capture source when none was selected explicitly
- Done in part: capture-source picker changes now apply immediately without a separate confirmation button
- Done in part: the main GUI path no longer exposes manual answer/ICE or legacy mock/WebRTC staging controls
- Done in part: macOS GUI source selection now uses runtime enumeration when available
- Done in part: Linux GUI source selection now uses `wmctrl`-based runtime enumeration when available
- Done in part: background GUI polling no longer overwrites in-progress room/signaling/ICE draft edits, and the desktop shell now exposes persisted auto-refresh controls for tuning that polling loop
- Done in part: the desktop shell now supports an explicit reconnect action that rebuilds the current host/viewer session with the active room, signaling, and ICE configuration
- Done in part: the desktop GUI now surfaces explicit recovery-state diagnostics so stopped, disconnected, failed, and healthy session states are visible alongside reconnect guidance
- Done in part: the desktop GUI now surfaces candidate-pair RTT, available bitrate, byte counters, and packet-loss diagnostics alongside the existing ICE path summary
- Still open: replace metadata-only runtime catalogs with real capture-session enumeration and Wayland portal flow

## Iteration 5: Audio/Video Pipeline Hardening

1. Choose initial codec set:
   video `H.264` or `VP8`, audio `Opus`.
2. Add bitrate presets and adaptive quality controls.
3. Add frame pacing and backpressure handling.
4. Add A/V sync monitoring.
5. Add graceful degradation for weak networks.

## Iteration 6: NAT Traversal And Deployment

1. Add STUN configuration to transport settings.
2. Add TURN fallback support for restrictive NAT environments.
3. Expose relay/direct path state in the GUI.
4. Separate signaling deployment config from desktop runtime config.
5. Document production deployment topology and security assumptions.

### Iteration 6 Progress

- Done in part: `transport-webrtc` already accepted ICE server entries, and `app-core`, CLI, Tauri, and persisted desktop session config now expose custom STUN/TURN server configuration end to end
- Done in part: transport snapshots and the desktop GUI now surface a best-effort ICE candidate-pair summary with direct-vs-relay hints
- Done in part: ICE server configuration is validated before it reaches CLI/Tauri session startup, including scheme checks for STUN/TURN URLs
- Done in part: production networking topology and security assumptions now live in `docs/NETWORKING.md`
- Still open: bundled TURN deployment/auth flow and deeper relay/direct metrics

## Iteration 7: Persistence And App Packaging

1. Persist last-used signaling settings and UI preferences.
2. Persist selected source metadata where safe.
3. Add Tauri bundling configuration for macOS.
4. Add desktop icons, metadata and release build scripts.
5. Write installation and troubleshooting documentation.

### Iteration 7 Progress

- Done in part: `app-core::SessionManager` now persists last-used room, signaling address, source label, and selected capture-source metadata to a lightweight local preferences file
- Done in part: the desktop preferences store now also persists auto-refresh enablement and polling interval for the Tauri GUI
- Done in part: session-manager reset now restores persisted preferences after clearing the active live session
- Done in part: `apps/desktop/src-tauri/tauri.conf.json` now enables macOS `.app` and `.dmg` bundle targets with a project-specific bundle identifier and baseline bundle metadata
- Done in part: desktop release packaging now has generated icon assets, crate/bundle metadata, and helper scripts for icon generation plus `cargo tauri build`
- Done in part: installation, release-build, saved-preferences, and troubleshooting guidance now live in `docs/INSTALLATION.md`
- Still open: additional desktop UI preference persistence beyond polling controls, plus notarization/signing details

## Iteration 8: Testing And Release Readiness

1. Add unit tests for signaling protocol parsing and session transitions.
2. Add integration tests for end-to-end host/viewer flow.
3. Add manual QA checklist for macOS and Linux.
4. Test real streaming across different networks.
5. Measure latency, CPU usage and packet loss behavior.

### Iteration 8 Progress

- Done in part: `app-core` now has broader unit coverage for signaling parser edge cases plus viewer/debug-capture session transition behavior
- Done in part: `app-core/tests/host_viewer_flow.rs` now exercises host/viewer negotiation and late-join offer replay over a real local TCP signaling path, isolated from any user-local desktop preferences
- Done in part: `app-core` now has unit coverage for reconnecting stopped host/viewer sessions and rejecting reconnect attempts from idle state
- Done in part: `app-core` now has unit coverage for recovery diagnostics when signaling is unavailable, plus more stable isolated capture-selection tests
- Done in part: manual QA coverage for current MVP behavior now lives in `docs/QA_CHECKLIST.md`
- Still open: broader QA checklists and real-network validation
