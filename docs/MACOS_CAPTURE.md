# Native macOS Capture

`capture-macos` builds an Objective-C ScreenCaptureKit bridge on macOS and exposes native `CaptureStreamEvent` samples to Rust. Linux builds do not compile or link Apple frameworks. Native extraction/lifecycle checks use synthetic buffers. [Actual window video](qa/actual-window-0.1.0-0185449.md) is verified between local debug bundles; standalone window/application/display capture and raw audio checks are recorded in [BUG-013 verification](qa/BUG-013-command-line-capture.md). Concurrent fixture audio isolation is covered by the [BUG-014 regression](qa/BUG-014-audio-isolation.md); permission transitions, broader application/device audio behavior, audible playback, and packaged capture still require validation.

## Requirements

- macOS 12.3+ for video; macOS 13+ for system/application audio
- Xcode Command Line Tools (`xcrun`, `clang`, macOS SDK)
- Screen Recording access for the desktop app or the terminal running the CLI probe

Passive catalog refresh checks `CGPreflightScreenCaptureAccess` and does not request permission. An explicit capture start can call `CGRequestScreenCaptureAccess`. If macOS requires relaunch after granting permission, restart the app/terminal, refresh the catalog, select a native source, and retry capture. Enumeration uses ScreenCaptureKit, not Accessibility/Apple Events.

## Native Sources

The source catalog comes from `SCShareableContent`:

- windows: `macos-window-WINDOW_ID`, captured through a desktop-independent window filter
- applications: `macos-application-PROCESS_ID-DISPLAY_ID`, all windows of that application on the indicated display
- displays: `macos-display-DISPLAY_ID`, full display capture

Window titles are labels; matching uses native IDs. These IDs can change after an application/system restart. Standalone callers initialize the CoreGraphics display connection before constructing a window filter; no AppKit application is required. Missing saved sources are rebound by the session manager. Blueprint fallback sources support demo/debug behavior and cannot start an SCStream.

## Samples And Lifecycle

Video is packed BGRA8; the bridge removes pixel-buffer row padding and ignores non-complete frames. Dimensions preserve source aspect ratio within the configured bounds. The bridge can normalize settings to 1–120 fps and dimensions 1–4096, but the desktop worker uses 30 fps and 1280×720 bounds to fit the current H.264 encoder's 720p/30fps limits. The GUI has no quality/FPS controls yet.

Audio is configured for 48 kHz stereo on macOS 13+. Both planar and interleaved float32 CoreMedia buffers are normalized to interleaved `AudioBuffer.samples`. The stream excludes the capturing process's audio. Audio filtering is application-level: selecting a window can include sound from other windows of its owning application ([Apple WWDC22](https://developer.apple.com/videos/play/wwdc2022/10155/)). The [concurrent fixture regression](qa/BUG-014-audio-isolation.md) checks window/application isolation on the recorded macOS build; other applications, routing and protected content still need manual testing. No microphone capture is performed.

Both sample types preserve CoreMedia presentation timestamps in microseconds. The native queue retains the latest video frame and up to 32 audio buffers. A dedicated worker drains this queue roughly every 5 ms, encodes media, and publishes to WebRTC independently of GUI refresh. GUI diagnostics retain only the newest raw video/audio samples.

Start returns `starting` while native content lookup and SCStream startup run asynchronously, then `running` only after the native completion succeeds. Startup has an eight-second timeout. Native failures become error events. Stop/drop detaches outputs, cancels further sample delivery, and stops the SCStream. Changing source/audio selection stops the old runtime; explicitly start capture again for the new selection.

## Capture Diagnostics And WebRTC

The desktop capture panel exposes `Captured Video`/`Captured Audio` counts and `Native Video`/`Native Audio` summaries of the latest samples. Counts reflect events drained into the session manager, so they depend on polling and latest-only diagnostic buffering. Samples update source-validated diagnostics without retaining raw media in the snapshot.

Native BGRA/PCM is encoded by VideoToolbox H.264 and vendored Opus before publication to codec-aware WebRTC tracks. Publication starts only with a connected peer. `Encoded H.264`/`Encoded Opus` count published access units/packets; viewer RTP counts may differ due to fragmentation. The desktop viewer decodes/displays media and starts muted. `Push Debug Capture Burst` is a separate non-playable packetization test. See [media pipeline](MEDIA_PIPELINE.md) and [viewer playback](VIEWER_PLAYBACK.md).

## Verification

Automated checks do not record the screen or prompt for permission:

```bash
cargo test --all --all-features
cargo test -p capture-macos --example capture_probe --all-features
sh scripts/test-macos-capture.sh
```

The native test uses real CoreVideo/CoreMedia sample buffers to verify BGRA row packing, planar/interleaved PCM, timestamps, queue bounds, and stopped-stream behavior. Rust tests cover target validation, malformed buffers, runtime status transitions, sample diagnostics, source/audio validation, and capture shutdown on selection changes. CI runs Rust checks on Linux and macOS and native extraction tests on macOS.

For an explicit manual capture without signaling or WebRTC:

```bash
cargo run -p capture-macos --example capture_probe -- --list
cargo run -p capture-macos --example capture_probe -- macos-window-WINDOW_ID --include-audio
cargo run -p capture-macos --example capture_probe -- macos-window-WINDOW_ID --require-audio
```

Replace `WINDOW_ID` with an ID from the native catalog. The probe captures for ten seconds, reports first-sample metadata, counts, audio peak and sample-weighted RMS, and explicitly stops even after a polling error. No audio flag disables audio and rejects unexpected audio buffers. `--include-audio` permits silence; `--require-audio` fails unless RMS exceeds `0.001`. Both modes require video frames. Non-finite/empty audio buffers and invalid arguments are errors; an audio-signal check does not establish audible output or source isolation. It saves no frames and opens no network connection. On an unchanged static window, frames may be sparse; change the window content during the check.

For an opt-in regression using a visible animated Cocoa window and a generated low-amplitude 440 Hz tone:

```bash
cargo build -p capture-macos --example capture_probe
python3 scripts/test-native-capture.py --output /tmp/video-p2p-native-capture-results
```

Use a new output directory for each run. This requires Python 3, Xcode Command Line Tools, a macOS desktop session, and Screen Recording permission. It opens test windows, plays the generated tone, and briefly captures a display from the native catalog; it saves only diagnostic logs, source catalog metadata and `results.json`, never captured video/audio. Temporary fixture binaries and the generated tone are removed and fixture processes stopped. The runner waits for native catalog visibility and gives every scenario a fresh fixture. It covers window audio modes, application audio, display opt-out, and silent-source success/failure. Silent cases run without the tone process; they do not test filtering between concurrent applications.

For concurrent application audio isolation:

```bash
cargo build -p capture-macos --example capture_probe
python3 scripts/test-native-audio-isolation.py --output /tmp/video-p2p-audio-isolation-results
```

This runner creates two temporary application bundles with distinct identifiers and ad-hoc signatures, launches them through macOS LaunchServices, and keeps a tone playing in one while capturing the silent window/application in the other. Seven checks cover positive audio controls, delivered silent PCM, window/application opt-out, and a final tone control. The silent checks require actual audio buffers with peak/RMS at or below `0.000001`; missing streams do not count as silence. It saves only identities and diagnostic logs, stops its applications through private stop files and waits for `open -W`, then removes temporary binaries/WAV data. Screen Recording access and a desktop session are required; `open` and `codesign` ship with macOS.

`--compare-direct-launch` first runs the same binaries directly, preserving the two expected failing isolation results before the LaunchServices checks. This diagnostic option reproduces BUG-014 on the recorded OS; it fails if the old condition no longer reproduces. The default mode tests correct isolation only. Direct execution from a common launcher is not a reliable way to model independent macOS audio applications, even with distinct bundle IDs/signatures. See the [controlled comparison](qa/BUG-014-audio-isolation.md).

Manual permission/relaunch, multiple-monitor, broader application/device audio filtering, capture-to-playback audio, and packaged stop/close checks remain outstanding. Their steps are maintained in the [QA checklist](QA_CHECKLIST.md#macos-actual-capture-and-publication--manual).

API reference: [Apple ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit) and [SCStreamOutput](https://developer.apple.com/documentation/screencapturekit/scstreamoutput).
