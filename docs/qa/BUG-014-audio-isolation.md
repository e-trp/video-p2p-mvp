# BUG-014: Concurrent Audio Fixture Isolation

## Scope and Environment

- Date: 2026-10-22; base `beaf3f0`, untagged development version `0.1.0`.
- macOS 27.0 (26A428), arm64; Rust 1.99.1, Apple clang 21.0.0, Node 26.12.1.
- Screen Recording permission granted to the command-line probe, one display. Two animated Cocoa fixture applications use distinct bundle IDs and ad-hoc signatures. One plays a generated 440 Hz, stereo 48 kHz tone with amplitude 0.05; the other stays silent throughout.
- This task changes the QA launch/lifecycle setup, not `capture-macos`, media codecs, transport or desktop behavior. No dependency, public API, version bump, tag or new feature ID (`0/5`).
- The user-authorized deletion of the unrelated untracked `.python-version` is complete; it contained only `test` and was not part of a Git commit.

## Investigation and Cause

The earlier `after/` and `distinct-apps/` results under `/private/tmp/video-p2p-native-audio-qa/` show unexpected nonzero PCM when capturing a silent fixture while another fixture plays audio. Those fixtures were directly launched as subprocesses. Distinct bundle metadata alone had not established independent audio attribution.

The first investigation, `/private/tmp/video-p2p-bug014/signed-direct`, uses separately compiled application bundles with different bundle IDs and ad-hoc signing identifiers. Both the silent window and silent application still capture the tone (RMS approximately 0.03535). Positive tone and opt-out controls pass. Thus adding signatures alone does not correct the test setup.

The second investigation, `launch-services/` in the same directory, starts the fixtures through `/usr/bin/open -n -W`. Silent window/application capture now delivers actual PCM with zero peak/RMS while the other app continues playing. Positive tone and opt-out controls pass. Apple documents that window audio is filtered at the owning application level, including other windows of that application ([WWDC22](https://developer.apple.com/videos/play/wwdc2022/10155/)).

The final controlled runner builds and signs the bundles once, records their binary SHA-256 hashes, and reuses those exact binaries in both launch modes. Only the launch path changes. Direct launch reproduces both isolation failures; LaunchServices passes all seven checks. This establishes a fixture launch-context defect in this environment. The internal OS grouping mechanism is not established, and the result does not claim that every terminal-launched program behaves identically on every macOS release.

## Regression and Lifecycle

Commands executed:

```bash
cargo build -p capture-macos --example capture_probe
/usr/bin/python3 /private/tmp/video-p2p-bug014/investigate.py
/usr/bin/python3 /private/tmp/video-p2p-bug014/launch.py
/usr/bin/python3 scripts/test-native-audio-isolation.py --compare-direct-launch --output /private/tmp/video-p2p-bug014-controlled
/usr/bin/python3 scripts/test-native-audio-isolation.py --compare-direct-launch --output /private/tmp/video-p2p-bug014-final
```

The first repository-runner comparison (`controlled`) passes its expected-before/after assertions. The final run adds explicit fixture exit-code and stop-wait evidence. `direct/results.json` intentionally retains two `FAIL` results; the top-level command succeeds only when they are the expected isolation failures (exit 0, actual video and RMS above 0.001) and all controls and LaunchServices cases pass. The default runner omits this reproduction phase and requires correct isolation only.

| Case | Direct launch | LaunchServices | Final PCM evidence |
| --- | --- | --- | --- |
| tone-window | PASS | PASS | 282 video frames, 488 audio buffers, RMS 0.035346 |
| silent-window | FAIL (expected), RMS 0.035353 | PASS | 284 video frames, 487 audio buffers, RMS 0.0 |
| silent-application | FAIL (expected), RMS 0.035353 | PASS | 281 video frames, 486 audio buffers, RMS 0.0 |
| tone-application | PASS | PASS | 283 video frames, 487 audio buffers, RMS 0.035353 |
| window-opt-out | PASS | PASS | 284 video frames, 0 audio buffers, RMS 0.0 |
| application-opt-out | PASS | PASS | 284 video frames, 0 audio buffers, RMS 0.0 |
| tone-still-playing | PASS | PASS | 284 video frames, 486 audio buffers, RMS 0.035354 |

Silent checks require delivered PCM, peak/RMS at or below `0.000001`, and actual video frames. No buffers, a missing source, or an aborted capture cannot masquerade as silence. The final tone check proves the audio producer is still playing after the isolation and opt-out cases. Each case starts/stops its own capture runtime, so the sequence also covers capture restarts against the same live sources.

Fixtures print their runtime bundle IDs and native window/process IDs. The runner waits for native catalog visibility. Its private stop files request termination in the fixture's event loop; `open -W` waits for actual app exit. Cleanup results are recorded per fixture. In the final comparison all four fixture/launcher exits are 0; direct stop waits are below 0.14 seconds and LaunchServices waits below 0.08 seconds. A 120-second fixture watchdog bounds an abandoned run. No existing user application is controlled or terminated. Temporary binaries/WAVs are removed after process cleanup; saved files contain diagnostics/identities, not captured media.

## Mandatory Automated Results

| Command | Result |
| --- | --- |
| `cargo fmt --all` | PASS |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --all --all-features` | PASS, 124 tests plus doctests; local sockets/codec services allowed |
| `cargo test -p app-core --example playback_probe --all-features` | PASS, 4 tests |
| `cargo test -p capture-macos --example capture_probe --all-features` | PASS, 4 tests |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features` | PASS, 0 crate tests |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS, 8 tests |
| `sh scripts/test-macos-capture.sh` | PASS, native extraction/timing/queue/stop |
| `cargo build -p signaling-server` | PASS |
| `cargo build -p app-core --example playback_probe` | PASS |
| `node scripts/test-viewer-playback.mjs` | PASS, 64×36, 22 frames / 38,400 audio samples; stop releases media |
| `node scripts/test-viewer-playback.mjs --keyframe-recovery` | PASS, 64×36, 24 frames / 44,160 audio samples; RTCP recovery; stop releases media |
| `node scripts/test-viewer-playback.mjs --chrome` | PASS, 64×36, 23 frames / 37,440 audio samples; stop releases media |
| `node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery` | PASS, 64×36, 23 frames / 42,720 audio samples; RTCP recovery; stop releases media |
| `/usr/bin/python3 scripts/test-native-capture.py --output /private/tmp/video-p2p-bug014-startup` | PASS, all 7 original fixture interface scenarios |

Logs use `/private/tmp/video-p2p-bug014-*.log`; native comparison results are in the directories above. Browser checks use synthetic encoded media and remain separate from the actual ScreenCaptureKit source/isolation evidence.

## Documentation, Cleanup and Remaining Work

Capture guide, bug register and TODO are updated. README, architecture, media pipeline and QA checklist were reviewed and remain accurate: the broader real-device/audio-playback scenarios are still incomplete. Relative links/anchors, shell command syntax, Python syntax and `--help`, the tracked Cyrillic scan (including hidden configuration), and `git diff --check` pass. All native/browser/startup probes pass. Test processes are stopped. `cargo clean` removed 8,227 files / 1.9 GiB; `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` removed 9,581 files / 2.6 GiB. Both target directories are absent and neither target was shared with another project. The preliminary investigation bundles/generated WAVs were also removed; their diagnostic logs and scripts remain. Later fixing-commit/closure entries change documentation only.

This is a focused fixture repair and native audio verification, not a full-project release checkpoint. Still open: actual captured audio through the desktop viewer to audible output, mute/autoplay and output devices, permission transitions, multiple monitors, other application types/protected content, packaged capture lifecycle, Linux and cross-network QA. BUG-010 still awaits hosted Ubuntu acceptance after authorized publication.
