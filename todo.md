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
- Done in part: `capture-macos` now exposes a best-effort runtime application/window catalog with blueprint fallback
- Done in part: runtime probing on macOS now maps catalog results into live permission-state diagnostics
- Done in part: Tauri/app-core now surface the current platform capture catalog and selected source metadata
- Done in part: session manager and `transport-webrtc` now accept debug `capture-core` video/audio payloads, including source-audio opt-out during smoke tests
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
- Done in part: source picker UI now exists, backed by the current platform capture catalog blueprint data
- Done in part: GUI now surfaces transport-stage, data-channel, and transport-note diagnostics from the live WebRTC layer
- Done in part: GUI now surfaces capture-catalog origin and backend notes for runtime-vs-fallback diagnostics
- Done in part: session refresh now re-synchronizes capture-catalog permission/origin state instead of freezing it at startup
- Done in part: host session guidance now reacts to capture permission readiness before falling back to signaling-only hints
- Done in part: host source selection now auto-rebinds when runtime catalog refresh invalidates the previous source
- Done in part: host session flow now auto-creates and sends its first local offer once signaling is connected
- Done in part: host session flow now defaults to the first available capture source when none was selected explicitly
- Done in part: capture-source picker changes now apply immediately without a separate confirmation button
- Done in part: the main GUI path no longer exposes manual answer/ICE or legacy mock/WebRTC staging controls
- Done in part: macOS GUI source selection now uses runtime enumeration when available
- Done in part: Linux GUI source selection now uses `wmctrl`-based runtime enumeration when available
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

## Iteration 7: Persistence And App Packaging

1. Persist last-used signaling settings and UI preferences.
2. Persist selected source metadata where safe.
3. Add Tauri bundling configuration for macOS.
4. Add desktop icons, metadata and release build scripts.
5. Write installation and troubleshooting documentation.

## Iteration 8: Testing And Release Readiness

1. Add unit tests for signaling protocol parsing and session transitions.
2. Add integration tests for end-to-end host/viewer flow.
3. Add manual QA checklist for macOS and Linux.
4. Test real streaming across different networks.
5. Measure latency, CPU usage and packet loss behavior.
