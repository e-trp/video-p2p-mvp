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

The IPC command argument contract is explicitly `snake_case`, matching the frontend (for example `signaling_addr`, `ice_servers`, `source_id`, and `include_audio`). Required and optional arguments use the same convention. String IPC rejections are displayed with their original reason. Prototype readiness updates on form input, including when Auto Refresh is disabled. Expanded diagnostics and long detail values stay within the window width.

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

## Real Tauri GUI Regression Runner

Build the opt-in QA executable and local fixtures from the repository root:

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
cargo build -p signaling-server
cargo build -p app-core --example playback_probe
node scripts/test-tauri-gui.mjs
```

The runner starts actual Tauri windows, executes the production DOM event handlers and Rust IPC, restarts the application to check preferences, and attempts real native-codec/WebRTC decoding in the desktop viewer. It does not replace IPC, sessions, or media with mocks. A diagnostic constructor observer only records the actual browser peers. Window-width tests resize the native window to 1120, 800, and 600 logical pixels.

The `gui-qa` feature has no additional dependencies and is absent from default builds. Its driver activates only with the runner's environment variables and requires a separate `VIDEO_P2P_MVP_CONFIG_DIR`. QA commands require that test state. The plugin focuses its own window before automation, and the driver waits for actual native focus/visibility. Keep that window active during control checks: directly launching an inactive `.app` left timer-dependent steps pending under BUG-009. A foreground guard failure remains failed evidence rather than a product PASS. JSON evidence, process logs, and isolated test preferences are saved in a temporary directory printed by the runner. `--output PATH` selects that directory; use a new directory for a fresh run. Existing user preferences are preserved. All child processes are stopped on completion/failure; partial evidence survives timeouts. Exit status is nonzero if any scenario fails or remains blocked.

To run the same checks against a QA-instrumented release bundle:

```bash
cd apps/desktop/src-tauri
cargo tauri build --features gui-qa --bundles app
cd ../../..
node scripts/test-tauri-gui.mjs --binary apps/desktop/src-tauri/target/release/bundle/macos/VideoP2P.app/Contents/MacOS/desktop-app
```

Keep a default `.app`/`.dmg` separate from the instrumented QA build. DOM `.click()`/input events are programmatic actions: they do not certify Accessibility clicks, trusted user gestures, audible devices, or Screen Recording grant/relaunch. The playback fixture is synthetic, while decoding and the Tauri/Rust path are real. A failing Tauri playback check remains a failure even when standalone WKWebView/Chrome probes pass. See the [current GUI QA record](qa/gui-client-0.1.0-6bd1a31.md) and the [original failed run](qa/tauri-gui-0.1.0-74a8e37.md).
