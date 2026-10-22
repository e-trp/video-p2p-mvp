# BUG-013: Standalone Window Capture Initialization

## Scope and Environment

- Date: 2026-10-22; development version `0.1.0`, base `ba1e9e6` plus the capture initialization/probe changes in this task. No version bump or release tag.
- macOS 27.0 (26A428), arm64, Apple clang 21.0.0, Rust 1.99.1, Node 26.12.1.
- Screen Recording permission granted to the command-line capture process; main display ID `1`. The opt-in Cocoa fixture creates an animated 480×270 content window and plays a generated stereo 48 kHz, 440 Hz WAV with amplitude 0.05. This is real ScreenCaptureKit window/application/display sampling, not an injected media buffer.
- No captured media is saved or sent over a network. Logs include source catalog metadata, sample dimensions/counts and PCM measurements. Synthetic browser checks below use local WebRTC independently of these native-source checks.

## Cause and Controlled Regression

The original bridge aborts while constructing `SCContentFilter initWithDesktopIndependentWindow:`. The native crash report `capture_probe-2026-10-22-121113.ips` shows `SLSGetDisplaysWithRect` / `SLGetDisplaysWithRect` / `setContentsAndStreamTypeMacOS` followed by the window initializer on `dev.videop2p.capture`. Its assertion is `CGS_REQUIRE_INIT: did_initialize`.

The fix initializes the display connection with public `CGMainDisplayID()` immediately before constructing a window filter. A null display produces a contextual error. Application/display filters keep their prior behavior; no AppKit application or private API is introduced into the capture bridge.

An isolated two-crate workspace at `/private/tmp/video-p2p-bug013-before` copies `capture-core`, `capture-macos`, and the updated probe, replacing only `native/screen_capture.m` with `git show HEAD:crates/capture-macos/native/screen_capture.m` from `ba1e9e6`. Thus the same runner, measurements and arguments exercise the original bridge and the fixed bridge without overwriting the working tree.

Commands executed:

```bash
cargo build --manifest-path /private/tmp/video-p2p-bug013-before/Cargo.toml -p capture-macos --example capture_probe
cargo build -p capture-macos --example capture_probe
/usr/bin/python3 scripts/test-native-capture.py --probe /private/tmp/video-p2p-bug013-before/target/debug/examples/capture_probe --output /private/tmp/video-p2p-bug013-before-results
/usr/bin/python3 scripts/test-native-capture.py --output /private/tmp/video-p2p-bug013-after-results
```

The original runner exits 1 as expected: all five window cases abort with subprocess exit `-6`; application/display controls pass. The fixed runner exits 0 with all seven cases passing:

| Case | Original bridge | Fixed bridge | Fixed evidence |
| --- | --- | --- | --- |
| Window, required audio | SIGABRT | PASS | 254 video frames, 471 audio buffers, RMS 0.035354 |
| Window, included audio | SIGABRT | PASS | 254 video frames, 464 audio buffers, RMS 0.035354 |
| Window, audio opt-out | SIGABRT | PASS | 143 video frames, zero audio buffers/samples |
| Application, required audio | PASS | PASS | 268 video frames, 473 audio buffers, RMS 0.035354 |
| Display, audio opt-out | PASS | PASS | 44 video frames, zero audio buffers/samples |
| Silent window, required audio | SIGABRT | PASS (expected exit 1) | 44 video frames, 486 audio buffers, RMS 0; contextual missing-signal error |
| Silent window, diagnostic audio | SIGABRT | PASS | 46 video frames, 487 audio buffers, RMS 0 |

Every case uses a fresh fixture and capture process. The runner terminates/waits for its children, deletes its temporary binary/WAV and keeps diagnostics. The probe explicitly stops capture after success or a polling/validation failure. Existing native tests cover stopped-buffer delivery and handle cleanup; Rust tests cover startup failure/status and runtime lifecycle.

Four portable probe unit tests cover argument modes/rejection, sample-weighted RMS across buffers, silence and signal-threshold boundaries, opt-out rejection, and non-finite/empty PCM without corrupting prior measurements. CI now runs the capture example tests explicitly.

## Failed Attempts and Separate Audio Investigation

Preserved historical evidence in `/private/tmp/video-p2p-native-audio-qa/` includes four original window crashes and passing application/display cases. After the initialization change, `after/` and `distinct-apps/` still fail the silent-source expectation while another copy of the fixture plays a tone. Copies wrapped with distinct bundle IDs also produce a signal. Later cases outlive fixture windows and report unavailable sources. These are not passing audio-isolation results; [BUG-014](../BUGS.md#bug-014-concurrent-audio-fixture-does-not-establish-source-isolation) tracks the uncertain fixture/OS/filter attribution separately.

The first resumed run, `/private/tmp/video-p2p-bug013-resumed`, also exposes a fixture readiness race: a Cocoa window number is printed before the window appears in the native catalog. `window-include` and `silent-required` report unavailable sources. The final runner waits for catalog visibility before capture and creates a fresh fixture for each scenario; it does not retry failed captures. The controlled before/after runs above use this final runner. Silent cases run after the tone process exits, so they establish silence handling but not concurrent-application filtering.

## Automated Results

| Command | Result |
| --- | --- |
| `cargo fmt --all` | PASS |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --all --all-features` | PASS outside sandbox: 124 tests and doctests |
| `cargo test -p app-core --example playback_probe --all-features` | PASS, 4 tests |
| `cargo test -p capture-macos --example capture_probe --all-features` | PASS, 4 tests |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features` | PASS (0 crate tests) |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS, 8 tests |
| `sh scripts/test-macos-capture.sh` | PASS, native extraction/timing/queue/stop |
| `cargo build -p signaling-server` | PASS |
| `cargo build -p app-core --example playback_probe` | PASS |
| `node scripts/test-viewer-playback.mjs` | PASS: 64×36, 6 decoded frames, 9,600 audio samples; stop releases media |
| `node scripts/test-viewer-playback.mjs --keyframe-recovery` | PASS: 64×36, 4 frames, 8,640 audio samples; browser RTCP recovery and stop |
| `node scripts/test-viewer-playback.mjs --chrome` | PASS: 64×36, 22 frames, 36,960 audio samples; stop releases media |
| `node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery` | PASS: 64×36, 18 frames, 34,560 audio samples; browser RTCP recovery and stop |

The first root Rust test attempt inside the sandbox fails because local TCP/UDP sockets are denied (`Operation not permitted`); its media test consequently times out with zero ICE candidates. The authorized unrestricted rerun passes. Logs are `/private/tmp/video-p2p-bug013-root-tests.log` and `video-p2p-bug013-root-tests-unrestricted.log` in the same directory. This is an execution restriction, not a new product defect. Other test logs use the `/private/tmp/video-p2p-bug013-` prefix.

## Review, Cleanup and Remaining QA

README, architecture and media pipeline responsibilities remain accurate; the capture guide, TODO and bug register are updated. Relative links/anchors, shell block syntax, Python syntax, the tracked Cyrillic scan (including hidden configuration), and `git diff --check` pass. The complete code/documentation diff was reviewed. Test processes are stopped. `cargo clean` removed 8,594 files / 1.9 GiB; `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` removed 9,581 files / 2.6 GiB. The isolated before-fix workspace cleanup removed 382 files / 12.1 MiB. All three target directories are absent; no shared target/cache was used. Logs remain outside target directories. Later fixing-commit and closure entries change only this verification record and the bug register.

This is a focused bug regression, not a full-project release checkpoint. Audio isolation (BUG-014), actual captured audio through WebRTC to audible output, permission deny/grant/relaunch, multiple monitors, packaged lifecycle, Linux, cross-network and distribution QA remain open. BUG-010 still awaits hosted Ubuntu acceptance. No new feature ID is counted (`0/5`). The pre-existing untracked `.python-version` is unrelated and is preserved outside the commit.
