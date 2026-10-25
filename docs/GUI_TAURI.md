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

If discovery fails after a runtime catalog was obtained, the GUI shows `cachedruntime` and the latest error/permission notes, retaining the last source list and selection. A cached list does not prove current availability; native startup validates again. Lost/unknown permission stops active capture. A failed UI refresh retains the last confirmed session controls/source options and marks the runtime badge or catalog notes as incomplete/stale; a later successful refresh clears that state. Only a successful catalog can establish source disappearance and trigger rebinding.

Source/audio changes stop active capture; Start Native Capture starts the new selection. Reconnect rebuilds the current role but does not restart host capture automatically. Host signaling uses the refresh path during negotiation; viewer signaling and established media run independently of diagnostic refresh.

Auto Refresh preserves source options while the picker has focus or its native popup is open (`:open`, when supported), including Accessibility opening without DOM focus. The open menu keeps the same source IDs and ordering. The latest catalog is applied after interaction ends; unchanged catalogs retain their option nodes. Selection uses the displayed source/audio values immediately. An older in-flight refresh cannot overwrite a user action, and source/audio controls are disabled until that action finishes. A source that disappears before selection is reported as unavailable, with guidance to refresh and choose again. See [source-selection verification](qa/BUG-011-source-selection.md).

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

Run `node scripts/test-tauri-gui.mjs --scenario source-picker` for the focused source regression (also included in the default run). It checks focused Auto Refresh, delays delivery of real catalog IPC responses to reproduce the selection race, checks audio opt-out, and submits a deliberately stale option to the real backend. Response data is unchanged. These DOM checks do not certify native menu gestures; the linked verification record records those separately.

The `gui-qa` feature has no additional dependencies and is absent from default builds. Its driver activates only with the runner's environment variables and requires a separate `VIDEO_P2P_MVP_CONFIG_DIR`. QA commands require that test state. The plugin focuses its own window before automation, and the driver waits for actual native focus/visibility. Keep that window active during control checks: directly launching an inactive `.app` left timer-dependent steps pending under BUG-009. A foreground guard failure remains failed evidence rather than a product PASS. JSON evidence, process logs, and isolated test preferences are saved in a temporary directory printed by the runner. `--output PATH` selects that directory; use a new directory for a fresh run. Existing user preferences are preserved. All child processes are stopped on completion/failure; partial evidence survives timeouts. Exit status is nonzero if any scenario fails or remains blocked.

To run the same checks against a QA-instrumented release bundle:

```bash
cd apps/desktop/src-tauri
cargo tauri build --features gui-qa --bundles app
cd ../../..
node scripts/test-tauri-gui.mjs --binary apps/desktop/src-tauri/target/release/bundle/macos/VideoP2P.app/Contents/MacOS/desktop-app
```

Keep a default `.app`/`.dmg` separate from the instrumented QA build. DOM `.click()`/input events are programmatic actions: they do not certify Accessibility clicks, trusted user gestures, audible devices, or Screen Recording grant/relaunch. The playback fixture is synthetic, while decoding and the Tauri/Rust path are real. A failing Tauri playback check remains a failure even when standalone WKWebView/Chrome probes pass. See the [current GUI QA record](qa/gui-client-0.1.0-6bd1a31.md) and the [original failed run](qa/tauri-gui-0.1.0-74a8e37.md).

## Native Desktop Playback QA

Run the opt-in actual-source test on macOS with a desktop session and Screen Recording access:

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
cargo build -p signaling-server
cargo build -p capture-macos --example capture_probe
python3 scripts/test-native-desktop-playback.py --output /tmp/video-p2p-native-desktop-results
```

Use a fresh output directory. The runner creates a signed, temporary Cocoa source application that animates a real window and plays a generated 440 Hz tone. It launches that source through LaunchServices, then two actual Tauri processes with isolated preferences. ScreenCaptureKit, the production host worker, H.264/Opus and the desktop WebView carry the media. No raw/encoded media crosses Tauri IPC.

Separate audio-on/off sessions check decoded fixture pixels, advancing video, actual nonzero decoded PCM through a zero-gain Web Audio analyzer, muted startup, programmatic sound controls, and Stop/track/peer cleanup. The host negotiates with explicit refreshes, then publication and viewer decoding are checked with Auto Refresh disabled. Audio opt-out requires zero host encoded audio and zero viewer audio packets/samples while video continues. The QA-only private stop marker coordinates host shutdown after viewer completion. The runner saves JSON diagnostics and process logs, stops only its own processes and deletes temporary source binaries/WAV data.

`--binary PATH` selects a `gui-qa` executable and `--probe PATH` selects the capture catalog probe. The default build has no QA driver. This checks actual captured media from a controlled source, not a microphone or unrelated user content. Physical speaker/headphone output, trusted user gestures, permission transitions and packaged distribution remain manual checks.

To exercise source replacement and disappearance in one established peer session:

```bash
python3 scripts/test-native-desktop-playback.py --source-lifecycle --output /tmp/video-p2p-source-lifecycle-results
```

This mode launches a red tone source and a blue silent source. It verifies that selecting another source stops publication until **Start Native Capture**, then checks replacement pixels and delivered silent audio while the original tone continues playing. Returning to the original source must restore pixels/nonzero PCM without recreating the viewer peer or generation. The runner then terminates its original source application, waits for catalog refresh to remove its window, and checks that rebinding does not automatically capture another source. The viewer must stop decoding new frames and the tone must drain away. The driver focuses its own host/viewer window before control actions and phase observations. Private checkpoints coordinate observations before each change; saved reports identify the source IDs and transitions. The test closes the owning fixture application, not just one window of a multi-window application.

Current acceptance: catalog/session state-retention regressions pass; the complete native source-lifecycle mode remains blocked by [BUG-016](BUGS.md#bug-016-native-capture-fails-when-starting-a-replacement-source). See [BUG-015 verification and failed evidence](qa/BUG-015-catalog-refresh.md).
