# TODO

## Iteration 1: Real WebRTC Transport

1. Create a real `transport-webrtc` implementation crate.
2. Define session-facing APIs for:
   `create_offer`, `accept_offer`, `add_ice_candidate`, `connection_state`, `stats`.
3. Replace mock UDP session transitions with real transport state transitions.
4. Extend signaling protocol from simple room pairing to SDP/ICE exchange.
5. Add integration tests for host/viewer negotiation state.

## Iteration 2: macOS Capture Backend

1. Add a native bridge for `ScreenCaptureKit`.
2. Enumerate shareable windows and applications.
3. Request and validate Screen Recording permission.
4. Capture video frames from the selected window.
5. Capture available audio buffers and normalize them for transport.
6. Feed captured samples into the WebRTC publishing pipeline.

## Iteration 3: Linux Capture Backend

1. Add Wayland path via `XDG Desktop Portal ScreenCast`.
2. Integrate PipeWire stream consumption for frames and audio.
3. Model permission and source selection flow in the session manager.
4. Add X11 fallback for unsupported environments.
5. Test on at least GNOME Wayland and KDE Wayland.

## Iteration 4: Tauri Production Flow

1. Replace mock session actions in the GUI with real backend session orchestration.
2. Add source picker UI backed by macOS/Linux capture enumeration.
3. Add room creation and invite code UX.
4. Add connection diagnostics:
   transport state, RTT, bitrate, packet loss, ICE path.
5. Add start, stop, reconnect and recovery states in the GUI.

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
