# Tauri Desktop UI

## Runtime And Build

The desktop app uses static HTML/CSS/JavaScript in `apps/desktop/public` and a Rust Tauri shell in `apps/desktop/src-tauri`. It is a separate Cargo workspace and is not included in root workspace builds or CI checks.

```bash
cd apps/desktop/src-tauri
cargo tauri dev
```

Opening the HTML outside Tauri shows a static preview without Rust session commands. See [installation](INSTALLATION.md) for prerequisites and release scripts.

## Implemented UI

| Area | Behavior |
| --- | --- |
| Host | Start Host Prototype prepares signaling and starts selected native capture; Prepare Host prepares signaling only |
| Viewer | Join Viewer / Prepare Viewer create the WebView playback session |
| Remote Window | Received video, muted startup, Enable Sound/Mute Sound, native playback controls, errors |
| Session | Room and TCP address, custom ICE entries, save, stop, reset, manual reconnect |
| Capture | Runtime/fallback catalog, immediate source/audio selection, native start/poll/stop, raw/encoded/received diagnostics |
| Diagnostics | Capture status, SDP/ICE state, selected path/RTT/bytes/loss when available, keyframe counters, recovery guidance, logs |
| Preferences | Persisted room/signaling/ICE/source and Auto Refresh interval; draft form edits survive background refresh |
| Local demo | Simulated session/readiness state without capture, WebRTC, or media delivery |
| Debug | Send Test Frame / Push Debug Capture Burst inject non-playable transport smoke payloads |

Actions use a shared busy state. Backend-provided labels are escaped when rendered as detail rows. Stop/reset/role changes close the viewer player; generation checks reject callbacks from replaced sessions.

Source/audio changes stop active capture; Start Native Capture starts the new selection. Reconnect rebuilds the current role but does not restart host capture automatically. Host signaling uses the refresh path during negotiation; viewer signaling and established media run independently of diagnostic refresh.

## Backend Responsibilities

- Tauri commands wrap `SessionManager`; snapshots are serialized as UI diagnostics.
- `app-core` owns session/capture lifecycle and the background encode/publish worker.
- `transport-webrtc` owns the Rust host and CLI diagnostic peers.
- `viewer-player.mjs` owns the desktop viewer peer and decoding; Rust only bridges signaling/diagnostics.
- `capture-macos` supplies native media. `capture-linux` supplies metadata and a scaffold only.

## Not Yet Completed

- Invite-code/link generation and authenticated room access; current pairing uses a manually entered room string.
- Bitrate, FPS, resolution, and audio-routing settings in the UI; current capture/codec defaults are fixed by the backend.
- Actual macOS permission/source/audio and packaged playback QA.
- Linux portal source-selection/capture flow and Linux WebView playback QA.
- Signed/notarized release validation and Linux bundle targets.

The existing macOS `.app`/`.dmg` configuration, icons, and build helper do not establish release readiness. See [TODO](../todo.md) and [manual QA](QA_CHECKLIST.md).
