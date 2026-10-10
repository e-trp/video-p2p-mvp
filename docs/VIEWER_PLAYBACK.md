# Desktop Viewer Playback

The desktop viewer receives H.264 video and Opus audio directly in a WebView `RTCPeerConnection`. Its WebRTC stack handles RTP depacketization, jitter buffering, decoding, and playback through a single `<video>` element. Rust retains native host capture/encoding and TCP signaling; media never crosses Tauri IPC. The CLI viewer is a short negotiation/RTP diagnostic command that exits after connection, without playback.

## Using The Player

1. Start `signaling-server`, then run two desktop instances with the same room/signaling address. Use a server address reachable by both; loopback only works on one machine.
2. On the macOS host, grant Screen Recording access, refresh/select a real source, and click Start Host Prototype, or Prepare Host followed by Start Native Capture. Audio requires macOS 13+ and Include Audio enabled.
3. On the viewer, click Join Viewer or Prepare Viewer. The Remote Window panel appears and starts video muted.
4. Click Enable Sound to hear the received audio. Video controls support play/pause, volume, and fullscreen. If autoplay is blocked, click the play control.
5. Check moving video, audible output, decoded video/received audio sample counts, and player errors. Received samples alone do not prove speaker output. Stop/reset/switch role closes the peer, stops media tracks, and clears the video element.

Host signaling uses the session refresh loop during setup. The viewer has a separate ~100 ms signaling pump and one-second stats reports, independent of diagnostic auto-refresh. Turning diagnostic refresh off does not pause established playback. A transient disconnection remains visible; recovery clears that message and preserves any play/pause or autoplay guidance. A failed connection stops the player and rejects delayed signals/reports until reconnect creates a new generation. Reconnect does not restart host capture automatically. For an established connection lost after replacing a peer, rebuild both peers so host SDP/DTLS state is refreshed.

## Signaling And Lifecycle

`SessionManager::start_playback_viewer` creates a signaling adapter instead of a second Rust PeerConnection. Its offer/ICE queue is bounded at 128 messages. The desktop receives host offers and ICE through `viewer_poll_signals`, sends browser answers/local ICE through `viewer_send_signal`, and reports connection/playback metrics through `viewer_report_playback`.

Every command is validated against the active generation and terminal playback state. Delayed responses/callbacks from stopped, failed, or replaced sessions cannot resume signaling or overwrite the session's diagnostics. The player buffers remote ICE until the offer is applied and sends its answer before local ICE. Stop/reconnect cancels pending work and releases tracks and the peer, including when an ICE operation is awaiting completion. Diagnostic refresh preserves the stopped state. Codec/WebView/SDP failures are shown in the player, distinct from a user-gesture/autoplay block.

Both peers use BUNDLE for shared media transport. The viewer requires `max-bundle`; host SDP advertises Constrained Baseline H.264 Level 3.1 (`42e01f`, packetization mode 1), 90 kHz video, and 48 kHz stereo Opus. VideoToolbox SPS tests verify the advertised profile constraints. Synthetic playback and initial-keyframe-loss recovery were verified in macOS WKWebView and Chromium, separately from the packaged Tauri app.

The WebView decoder sends RTCP keyframe feedback directly to the host. The host responds to video PLI/FIR through the capture worker, coalescing repeated requests and waiting at least 500 ms after the previous published keyframe. The host capture panel shows Keyframe Requests, Published Keyframes, and Keyframe Pending. Pending requests require another captured frame; keyframe recovery does not adjust bitrate or repair a sustained network outage.

## Verification

Unit/lifecycle checks:

```bash
node --test apps/desktop/tests/viewer-player.test.mjs
cargo test -p app-core viewer
```

Permission-free native playback test on macOS with Command Line Tools, Node 22+, a desktop session, local sockets, and system codec/WebKit services available:

```bash
cargo build -p signaling-server
cargo build -p app-core --example playback_probe
node scripts/test-viewer-playback.mjs
```

The probe starts a local signaling server, a synthetic VideoToolbox/Opus host, the real SessionManager viewer bridge, and a hidden WKWebView loading the actual player module. It checks decoded video dimensions/pixel color, nonzero decoded PCM through a Web Audio analyzer, backend counters, and stop followed by diagnostic refresh closing the peer and ending its media tracks. Audio output in this test has zero gain. No screen recording, camera/microphone capture, external signaling/STUN/TURN, or user preferences are used. The hidden probe allows autoplay; actual user-gesture behavior is separately covered by unit tests and needs manual desktop QA.

Optional Chromium interoperability check with installed Chrome:

```bash
node scripts/test-viewer-playback.mjs --chrome
```

To omit the initial SPS/PPS/IDR and require browser-originated RTCP feedback plus a successfully decoded refresh frame:

```bash
node scripts/test-viewer-playback.mjs --keyframe-recovery
node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery
```

## Remaining Validation

Real window/application audio capture, packaged Tauri playback, sound output devices, multiple monitors, macOS permission/relaunch behavior, and cross-network streaming require [manual QA](QA_CHECKLIST.md#desktop-viewer-playback-and-recovery--manual). Linux playback depends on the installed WebView's WebRTC/H.264/Opus support and has not been validated. Unsupported WebViews show a player error. Browser playout timing is used initially; end-to-end A/V skew, latency/CPU, adaptive bitrate, and sustained weak-network recovery remain open. Dropping the initial keyframe in a local probe does not validate these scenarios.

References: [WebKit WebRTC in WebViews](https://webkit.org/blog/7763/a-closer-look-into-webrtc/) and [the WebRTC specification](https://www.w3.org/TR/webrtc/).
