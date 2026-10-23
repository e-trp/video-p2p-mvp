# Native Desktop Playback — 0.1.0 / c0d19af

Date: 2026-10-23. Base `c0d19af`, with the native desktop QA runner and opt-in `gui-qa` driver additions in this task. Production capture, encoding, transport and player logic are unchanged. Version remains untagged `0.1.0`; no new milestone feature ID and no release checkpoint (`0/5`).

## Scope and Environment

- macOS 27.0 (26A428), arm64 desktop session, local loopback TCP signaling on a dynamically selected port, room `native-playback-qa`, no configured STUN/TURN.
- Two real Tauri processes built with `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa`, isolated host/viewer configuration directories, native WKWebView.
- A temporary signed Cocoa source application launched through LaunchServices displays an animated window and plays a generated 440 Hz stereo tone. The application is external to both clients; ScreenCaptureKit supplies the actual frames/PCM to the production host worker and H.264/Opus/WebRTC path.
- The viewer checks decoded fixture pixels and actual PCM with a zero-gain Web Audio analyzer. Only measurements and diagnostics are saved; captured media is never transported over Tauri IPC or saved in evidence.
- Screen Recording permission is already granted. Programmatic DOM events drive the production controls. No permission transition, trusted user gesture or physical speaker/headphone acceptance is claimed.

## Commands and Focused Results

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
cargo build -p signaling-server -p capture-macos --examples --bins
python3 scripts/test-native-desktop-playback.py --output /private/tmp/video-p2p-native-desktop-first
python3 scripts/test-native-desktop-playback.py --binary /usr/bin/false --output /private/tmp/video-p2p-native-desktop-startup-failure
```

The first run passes all 22 host/viewer checks across separate audio-on/off sessions. Both views decode 1144×720 video; the sampled fixture pixel is `[176, 73, 45, 255]`. Audio-on records peak 0.050646 and RMS 0.011798, with 97,920 received audio samples and 57 decoded frames after the measurement interval. Audio-off continues to 55 decoded frames with zero received audio packets/samples and zero host encoded audio packets. Host publication and viewer playback advance with Auto Refresh disabled after explicit host negotiation refreshes. Both viewers start muted; programmatic Enable Sound/Mute Sound updates state. Stop ends viewer tracks/closes peers, repeated Refresh cannot revive the sessions, and host publication stops.

Each source fixture receives a private stop request and exits 0; first-run stop waits are below 0.13 seconds. The runner terminates/reaps only its own clients/server, and removes temporary source bundles/WAV files after cleanup. JSON reports and process logs remain under the evidence paths above. Fresh directories are required; previous evidence cannot be overwritten.

The final run uses the final QA driver and runner:

```bash
python3 scripts/test-native-desktop-playback.py --output /private/tmp/video-p2p-native-desktop-final
node scripts/test-tauri-gui.mjs --scenario viewer --output /private/tmp/video-p2p-native-desktop-synthetic-regression
```

All 22 native checks pass again. Source IDs are `macos-window-5587` (audio-on) and `macos-window-5611` (audio-off). Audio-on records decoded PCM peak 0.050654, RMS 0.018593, 150,240 received audio samples and 84 decoded frames; audio-off records 68 decoded frames and zero audio packets/samples. Host snapshots show 65 encoded video frames / 124 Opus packets with audio, and 62 encoded video frames / zero Opus packets without audio. Both host/viewer pairs exit 0 and both fixtures exit 0 (stop waits below 0.23 seconds). These counts are interval observations, not an A/V synchronization or latency measurement.

Final tested executable SHA-256: `921ba5b330506ce5062fadf560f63c9037293f325641a411d37a395cf79d55c2`. Full source identities, OS version, browser user agent, per-check measurements and cleanup records are preserved in the final evidence directory. The existing synthetic Tauri viewer regression also passes all 7 checks, including sound state, pause/resume, Stop and reconnect.

The deliberate startup-failure command substitutes `/usr/bin/false` for the GUI executable. It exits 1 as expected, records `success: false` and `native host exited before capture` in `summary.json`, and stops the already-started source/server. Its fixture exits 0 within 0.13 seconds. This verifies the runner's error and cleanup path; it is not a product failure or a passing media scenario.

## Mandatory Verification

| Command | Result |
| --- | --- |
| `cargo fmt --all` | PASS |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --all --all-features` | PASS, 124 tests plus doctests |
| `cargo test -p app-core --example playback_probe --all-features` | PASS, 4 tests |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features` | PASS, 0 crate tests |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS, 8 tests |
| `sh scripts/test-macos-capture.sh` | PASS, extraction/timing/queue/stop |
| `cargo build -p signaling-server` | PASS |
| `cargo build -p app-core --example playback_probe` | PASS |
| `node scripts/test-viewer-playback.mjs` | PASS, 64×36, 23 frames / 39,840 audio samples, stop cleanup |
| `node scripts/test-viewer-playback.mjs --keyframe-recovery` | PASS, 64×36, 23 frames / 43,200 audio samples, RTCP recovery and stop cleanup |
| `node scripts/test-viewer-playback.mjs --chrome` | PASS, 64×36, 26 frames / 42,240 audio samples, stop cleanup |
| `node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery` | PASS, 64×36, 21 frames / 38,880 audio samples, RTCP recovery and stop cleanup |

Lint, JS and extraction logs use `/private/tmp/video-p2p-native-desktop-*.log`; GUI results live in the directories above. Rust tests used local sockets and macOS codec services outside the sandbox. Synthetic browser probes remain separate from the actual-source desktop checks.

## Documentation and Cleanup

README, TODO and the GUI/capture/viewer/user/QA guides now distinguish verified native fixture decoding from outstanding physical-output and broader source/device acceptance. Architecture, media-pipeline guidance and the bug register were reviewed and remain accurate; no production behavior or new defect requires a change there. Python/JavaScript syntax, runner help, tracked/new text including hidden configuration for Cyrillic prose, changed-document relative links/anchors, shell-block syntax, final diff review and `git diff --check` pass.

Process inspection confirms no current test client, source fixture, signaling server, playback probe or Cargo build/test remains. Cargo metadata confirms both target directories belong to this repository. `cargo clean` removed 8,123 files / 1.9 GiB; `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` removed 10,621 files / 3.1 GiB. Both target directories are absent. Temporary fixture bundles/WAVs are removed, and logs/results remain outside Cargo targets. No release bundle was created. Final record-only edits do not change the tested code/configuration.

## Remaining Acceptance

Actual captured fixture media reaching desktop pixels/PCM is verified within this local QA-build scope. Physical output devices, trusted autoplay/unmute gestures, permission deny/grant/relaunch, other application/protected-content behavior, multiple monitors, source changes/closure, packaged installation/signing and cross-network/Linux acceptance remain open. The QA checklist's broader scenarios remain unchecked. BUG-010 still requires hosted Ubuntu acceptance; no new product defect is established by this run.
