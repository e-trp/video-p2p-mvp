# User Guide

## Scope

The desktop prototype has a macOS capture/encode/publish path and a WebView viewer. Synthetic media playback is verified in WKWebView and Chromium. [Actual window video](qa/actual-window-0.1.0-0185449.md) is verified between local debug bundles. Audio, other source kinds, packaged capture, and different networks still require [manual QA](QA_CHECKLIST.md). Linux capture/video encoding are not implemented; Linux playback is unvalidated.

The [current Tauri GUI QA](qa/gui-client-0.1.0-6bd1a31.md) verifies the controls and decoded synthetic playback in the desktop WebView, including the bundled app. Permission transitions, other source kinds, real audio, trusted user gestures, audible devices, and different networks still require manual validation.

For dependencies, running/building the app, and troubleshooting, see [installation](INSTALLATION.md). For completion status, see [TODO](../todo.md).

## Start A Real Desktop Session

1. Start signaling from the repository root:

   ```bash
   cargo run -p signaling-server -- 127.0.0.1:7000
   ```

2. Launch two desktop instances:

   ```bash
   cd apps/desktop/src-tauri
   cargo tauri dev
   ```

3. Give both instances the same **Room** and **Signaling** address. `127.0.0.1:7000` works only when both peers and the server are on the same machine; other machines must use the server's reachable address.
4. On the macOS host, grant Screen Recording access, refresh the catalog, select a real source, and set **Include Audio**. Audio requires macOS 13+ and a source with audio support.
5. Click **Start Host Prototype**. It prepares signaling and starts capture; encoded media is published when the viewer connects. **Prepare Host** only prepares the session; use **Start Native Capture** separately.
6. On the second instance, click **Join Viewer** or **Prepare Viewer**. The **Remote Window** panel starts video muted. Click **Enable Sound** for audio; click the native play control if autoplay is blocked.

The CLI `webrtc-viewer` is a short negotiation/RTP diagnostic command that exits after connection. Use a second desktop instance for sustained playback.

## Session Controls

| Control | Behavior |
| --- | --- |
| Room | Manually entered pairing string; no generated invite/link or authentication |
| Signaling | TCP `host:port`, not an HTTP/WSS URL |
| ICE Servers | Optional entries, one per line: `url` or `url|username|credential` |
| Save Config | Persist room, signaling, and ICE settings |
| Auto Refresh / Refresh Every | Persist diagnostic/host-signaling polling enablement and interval |
| Prepare Host | Create the Rust host peer and send its initial offer when signaling is available |
| Prepare Viewer / Join Viewer | Create the desktop WebView viewer session |
| Stop | Close the session/capture/player; refresh does not restart it |
| Reset | Return to idle and reload saved preferences; does not erase the preferences file |
| Reconnect | Rebuild the current role using active settings; save edited room/signaling/ICE fields first |

Accepted ICE schemes are `stun:`, `stuns:`, `turn:`, and `turns:`. Multiple URLs sharing credentials may be comma-separated within one entry. Example syntax (replace with your actual server):

```text
stun:stun.example.com:3478
turn:turn.example.com:3478?transport=udp|username|credential
```

Unsaved form drafts survive background refresh. **Reconnect** applies edited session fields before rebuilding the peer. On the host, start capture again after reconnect. If an established peer is replaced and the other side remains tied to its old SDP/DTLS session, rebuild both peers.

Host signaling needs refresh during negotiation. Viewer signaling runs independently of Auto Refresh. Once negotiation completes, capture, publication, and viewer playback continue with diagnostic refresh disabled.

## Capture Selection And Lifecycle

macOS sources come from SCShareableContent windows, applications, and displays. Matching uses native IDs, not labels. The first available source is selected if no explicit selection exists. Saved IDs may become invalid after an app/system restart; catalog refresh can rebind selection to the first available source and stops old capture.

Linux may list X11 window metadata through `wmctrl`. This does not enable X11 capture. Blueprint/example sources appear when enumeration is unavailable; they support demo/debug selection and cannot start real macOS capture. Linux runtime status/permission hints do not mean Portal/PipeWire is implemented.

Auto Refresh preserves the source menu during selection. If a selected source has disappeared, the command reports that it is unavailable; refresh and choose an available source again.

Changing **Available Source** or **Include Audio** applies selection immediately and stops active capture. Click **Start Native Capture** for the new selection. **Stop Capture** leaves the peer session available. **Poll Capture** updates diagnostics; it does not drive the media worker.

Capture defaults are 30 fps and at most 1280x720. H.264 defaults to 2 Mbit/s and stereo Opus to 64 kbit/s. The UI has no quality/FPS/audio-routing settings yet. Audio availability/application filtering need real testing; microphone capture is not implemented. See [macOS capture](MACOS_CAPTURE.md).

## Reading Diagnostics

| Field | Meaning |
| --- | --- |
| Captured Video / Captured Audio | Raw events delivered to session diagnostics; latest-only buffering makes counts depend on refresh cadence |
| Native Video / Native Audio | Latest raw sample summaries, not additional frame/buffer counters |
| Encoded H.264 / Encoded Opus | Successfully published encoded access units/packets on the host |
| Received Video RTP / Received Audio RTP | Received RTP packets; fragmentation means counts differ from encoded video frames |
| Decoded Video / Received Audio Samples | WebView stats, when exposed; decoded frames and received audio samples do not by themselves prove audible speaker output |
| Keyframe Requests / Published Keyframes / Keyframe Pending | Host RTCP refresh requests, all published keyframes, and pending refresh state |
| ICE Path / RTT / Bitrate / Bytes / Packet Loss | Peer stats when available; Rust host and WebView viewer expose different fields |
| Status / Permission / Origin | Capture runtime, permission probe, and runtime/fallback catalog status |

RTP reception is not proof of decoded playback. Raw counters are not publication counters. PLI/FIR recovery waits for a captured frame and respects a 500 ms cooldown after the last published keyframe. It does not adapt bitrate. See [media pipeline](MEDIA_PIPELINE.md) and [viewer playback](VIEWER_PLAYBACK.md).

## Recovery

The Recovery panel distinguishes healthy/negotiating/stopped, signaling unavailability, and disconnected/failed/closed transport. **Reconnect** is available after preparing a role and rebuilds that role. It is manual session recreation, not an automatic ICE restart. Transient viewer disconnections keep the player available; terminal failure closes it until reconnect.

When the signaling server closes the connection, host **Refresh** or the viewer's independent signaling pump detects it and recommends reconnecting. Complete final server messages are processed before closure is reported. Signaling loss alone keeps the current peer/player in place; verify whether media is still arriving. Restore the server and use **Reconnect** when needed; this rebuilds the session, and host capture must be started again.

Screen Recording permission can require relaunch. Restart the app, refresh/select a native source, and retry. Passive refresh does not request permission; explicit capture start can request it.

## Demo And Debug Tools

**Run Local Demo** simulates host/readiness state with a placeholder peer. It does not capture, encode, connect WebRTC, or send media. See [demo checks](DEMO_MANUAL_TEST.md).

**Send Test Frame** and **Push Debug Capture Burst** inject synthetic non-playable payloads into host tracks for packetization diagnostics. They respect source/audio selection but are not H.264/Opus encoder output. Do not mix them with a real playback check.

Legacy CLI mock UDP commands remain separate test tools:

```bash
cargo run -p p2p-cli -- receiver --room demo --signal 127.0.0.1:7000 --expected-frames 10
cargo run -p p2p-cli -- sender --room demo --signal 127.0.0.1:7000 --frames 10 --fps 5
```

Run them in separate terminals with signaling available. They do not capture or display a window.

## Limits

Linux capture/encoding, invite/auth flows, TLS signaling, managed STUN/TURN, quality adaptation, A/V skew/latency measurements, and release validation remain open. macOS bundle configuration and build scripts exist; signing/notarization and packaged playback have not been validated. [TODO](../todo.md) separates implemented work from outstanding implementation and QA.
