# Native macOS Capture

`capture-macos` now builds an Objective-C ScreenCaptureKit bridge on macOS and exposes native `CaptureStreamEvent` samples to Rust. Linux builds do not compile or link Apple frameworks.

## Requirements

- macOS 12.3+ for video; macOS 13+ for system/application audio
- Xcode Command Line Tools (`xcrun`, `clang`, macOS SDK)
- Screen Recording access for the desktop app or the terminal running the CLI probe

Passive catalog refresh checks `CGPreflightScreenCaptureAccess` and does not request permission. An explicit capture start can call `CGRequestScreenCaptureAccess`. If macOS requires relaunch after granting permission, restart the app/terminal, refresh the catalog, select a native source, and retry capture. Accessibility/Apple Events permission is no longer needed for macOS enumeration.

## Native Sources

The source catalog comes from `SCShareableContent`:

- windows: `macos-window-WINDOW_ID`, captured through a desktop-independent window filter
- applications: `macos-application-PROCESS_ID-DISPLAY_ID`, all windows of that application on the indicated display
- displays: `macos-display-DISPLAY_ID`, full display capture

Window titles are labels; matching uses native IDs. These IDs can change after an application/system restart. Missing saved sources are rebound by the session manager. Blueprint fallback sources support demo/debug behavior and cannot start an SCStream.

## Samples And Lifecycle

Video is packed BGRA8; the bridge removes pixel-buffer row padding and ignores non-complete frames. Dimensions preserve source aspect ratio within the configured bounds. Runtime defaults are 30 fps and 1280×720, with FPS clamped to 1–120 and dimensions clamped to 1–4096.

Audio is configured for 48 kHz stereo on macOS 13+. Both planar and interleaved float32 CoreMedia buffers are normalized to interleaved `AudioBuffer.samples`. The stream excludes the capturing process's audio. Actual audio availability and application filtering need manual testing; protected content may not be capturable. No microphone capture is performed.

Both sample types preserve CoreMedia presentation timestamps in microseconds. The native queue retains the latest video frame and up to 32 audio buffers. This bounds memory while the desktop polls; it is an initial capture diagnostic policy, not a continuous encoded streaming pipeline.

Start returns `starting` while native content lookup and SCStream startup run asynchronously, then `running` only after the native completion succeeds. Startup has an eight-second timeout. Native failures become error events. Stop/drop detaches outputs, cancels further sample delivery, and stops the SCStream. Changing source/audio selection stops the old runtime; explicitly start capture again for the new selection.

## Capture Diagnostics And WebRTC

The desktop capture panel exposes `Captured Video`, `Captured Audio`, `Native Video`, and `Native Audio`. Counts reflect events drained into the session manager, so they depend on polling and queue backpressure. Samples update source-validated diagnostics without retaining raw media in the snapshot.

Native raw BGRA/PCM is **not automatically sent** to the VP8/Opus WebRTC tracks. Video and audio encoders and viewer decoding/playback are still required. `Push Debug Capture Burst` remains a separate synthetic packetization smoke test and does not verify playable media. Host prototype startup now attempts native capture without automatically publishing debug samples.

## Verification

Automated checks do not record the screen or prompt for permission:

```bash
cargo test --all --all-features
sh scripts/test-macos-capture.sh
```

The native test uses real CoreVideo/CoreMedia sample buffers to verify BGRA row packing, planar/interleaved PCM, timestamps, queue bounds, and stopped-stream behavior. Rust tests cover target validation, malformed buffers, runtime status transitions, sample diagnostics, source/audio validation, and capture shutdown on selection changes. CI runs Rust checks on Linux and macOS and native extraction tests on macOS.

For an explicit manual capture without signaling or WebRTC:

```bash
cargo run -p capture-macos --example capture_probe -- --list
cargo run -p capture-macos --example capture_probe -- macos-window-WINDOW_ID --include-audio
```

Replace `WINDOW_ID` with an ID from the native catalog. The probe captures for ten seconds, reports first-sample metadata and counts, and stops. It saves no frames and opens no network connection. On an unchanged static window, frames may be sparse; change the window content during the check.

Manual QA still required:

1. Test permission denied, permission grant, relaunch, and retry on the packaged app and CLI.
2. Verify window, application-per-display, and display selection on single/multiple monitors.
3. Play changing video and audio in the selected app; confirm native counters and timestamps.
4. Disable audio and restart capture; confirm no audio samples arrive.
5. Change/close the selected window; verify the previous capture stops and source availability updates.
6. Stop/reset/close the desktop while starting and while running; verify macOS recording indicators disappear.

API reference: [Apple ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit) and [SCStreamOutput](https://developer.apple.com/documentation/screencapturekit/scstreamoutput).
