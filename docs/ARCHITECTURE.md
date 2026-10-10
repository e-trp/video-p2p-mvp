# Architecture

## Components

The root Rust workspace contains the core, capture, codec, transport, signaling, and CLI crates. `apps/desktop/src-tauri` is a separate workspace using the same core through a path dependency.

| Component | Current responsibility |
| --- | --- |
| `app-core` | Signaling protocol/client, session lifecycle, source selection, preferences, capture worker, browser viewer bridge |
| `capture-core` | Shared source/permission types, raw BGRA/PCM, runtime config/status/events |
| `capture-macos` | Objective-C/C ABI ScreenCaptureKit catalog and raw sample delivery |
| `capture-linux` | Best-effort `wmctrl` catalog and planned runtime; no Portal/PipeWire or X11 media capture |
| `media-codec` | macOS VideoToolbox H.264 and vendored Opus encoders |
| `transport-webrtc` | Rust host and CLI diagnostic peers, encoded publication, RTCP feedback, stats |
| `signaling-server` | TCP pairing, validated SDP/ICE relay, late-join history replay |
| `apps/desktop` | Tauri controls, diagnostics, and WebView-owned receive-only player |
| `apps/cli` | Short WebRTC negotiation checks and separate legacy mock UDP commands |

## Media And Signaling Flow

```text
macOS ScreenCaptureKit
        | BGRA / PCM
        v
app-core capture worker -> VideoToolbox H.264 / Opus -> Rust host PeerConnection
                                                               |
                                                 WebRTC RTP / DTLS-SRTP
                                                               |
                                                               v
                                             WebView viewer PeerConnection
                                                               |
                                                  decode / video + audio

Rust host <---- TCP SDP/ICE ----> signaling-server <---- TCP SDP/ICE ----> Rust viewer bridge
                                                                             |
                                                               generation-checked Tauri IPC
                                                                             |
                                                                      WebView signaling
```

The host attaches H.264/Opus tracks and a bootstrap data channel before creating its offer. Session refresh processes host signaling and remote answer/ICE. The desktop viewer owns its PeerConnection in JavaScript; Rust relays SDP/ICE and receives playback diagnostics. Encoded/decoded media never crosses Tauri IPC.

The viewer signaling pump runs independently of diagnostic Auto Refresh. Host negotiation requires refresh while offer/answer/ICE is in progress. Once connected, capture and publication run independently of GUI polling.

## Capture And Worker Lifecycle

The macOS catalog uses SCShareableContent native IDs, with example-source fallback when enumeration is unavailable. Linux discovers X11 metadata through `wmctrl` when possible. A fallback catalog or Linux permission/status scaffold does not imply working capture.

The session manager owns a background capture worker. The worker validates source/audio selection, drains raw samples, encodes only with a connected peer, and publishes with a bounded write timeout. Native queues retain the newest video and at most 32 audio buffers. GUI polling receives only the latest raw samples for diagnostics.

Stop, reset, source/audio changes, session replacement, and manager drop stop/join the worker before closing its transport runtime. Source changes and host reconnect require starting capture again. Reconnect rebuilds a peer; it is not an automatic ICE-restart or seamless media recovery implementation.

Video RTCP PLI/FIR sets bounded pending keyframe state. The worker requests a refresh on the next eligible captured frame, subject to a 500 ms cooldown. Successful keyframe publication acknowledges pending feedback; disconnection clears pending work. See [media pipeline](MEDIA_PIPELINE.md).

Desktop viewer generations reject stale signals/reports after stop, failure, reconnect, reset, or role changes. Cleanup closes the peer and tracks and clears the video element. See [viewer playback](VIEWER_PLAYBACK.md).

## State And Diagnostics

Session snapshots combine role/stage, capture catalog/runtime, transport, playback, and recovery guidance. Raw diagnostic counts depend on GUI polling; encoded publication, received RTP, and decoded frame/sample counters describe different pipeline stages.

Room/signaling/ICE configuration, selected source metadata, and polling preferences persist in `session.conf`. Runtime logs are in memory. Media, SDP sessions, and active capture are not restored from preferences.

## Signaling And Limits

The server accepts one sender/host and one receiver/viewer per room, relays `SIG|SDP|...` and `SIG|ICE|...`, and replays stored signals from the opposite role to a late joiner. Disconnect removes that role's history; an empty room is removed. There is no authentication, TLS, room TTL, rate limiting, or bounded server replay history.

WebRTC media uses DTLS-SRTP. TCP signaling and the separate mock UDP path are plaintext. Custom STUN/TURN entries are supported; bundled services, managed credentials, and cross-network QA remain open. See [networking](NETWORKING.md) and [TODO](../todo.md).
