# BUG-017: Keep Native Windows Responsive During Session Commands

## Scope and Controlled Evidence

2026-10-26, base `9d67fb8` plus the previously uncommitted contention fixture, untagged `0.1.0`. macOS 27.0 (26A428), arm64, native WKWebView, Node 26.12.1, local loopback signaling and isolated test preferences. This fixes existing `BASE-DESKTOP-CONTROLS`; the feature counter stays `0/5` and no release tag is created.

The inherited changes add an opt-in QA worker that holds the real session mutex for two seconds and a WebView scenario that submits `session_snapshot` before querying the native window. The inherited assertion incorrectly checked `mode == stopped`; the stopped value belongs to `stage`, which the completed test now checks.

Before the fix, the production synchronous Tauri handler blocks the native event loop while waiting for the mutex. The native window query only returns after the holder releases it. After dispatch through `tauri::async_runtime::spawn_blocking`, the window query returns while the lock is still held and the queued snapshot later returns the real stopped session. Both unknown-command and missing-required-argument requests reject. Tauri's generated argument/result handling remains unchanged; unknown-command rejection is explicitly preserved because dispatch now returns before the generated handler runs. Session access stays serialized, with no new dependencies, public APIs or media over IPC.

Commands and evidence:

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
node scripts/test-tauri-gui.mjs --scenario source-picker --output /private/tmp/video-p2p-bug016-resumed-before
node scripts/test-tauri-gui.mjs --scenario source-picker --output /private/tmp/video-p2p-bug016-resumed-after
node scripts/test-tauri-gui.mjs --output /private/tmp/video-p2p-bug017-full-gui
python3 scripts/test-native-desktop-playback.py --source-lifecycle --output /private/tmp/video-p2p-bug016-resumed-lifecycle
python3 scripts/test-native-desktop-playback.py --output /private/tmp/video-p2p-bug017-native-baseline
```

The before run fails only the contention check (7 PASS / 1 FAIL). The after run passes all 8 checks. Native GUI commands remain on their existing QA path, which is absent from default builds. The delay intentionally models a blocking operation without requiring an OS enumeration failure.

## Broader GUI and Native Results

The full GUI run is not a PASS: 35 PASS / 2 FAIL / 3 BLOCKED. Source-picker checks and session/control/error/layout checks pass, but the denied-permission scenario is BLOCKED because Screen Recording is already granted. Restore fails because controls selected its own window `7554`, which disappears on process exit; clearing this absent source is expected idle-session behavior (BUG-018). Its room/ICE/refresh assertions pass. Viewer readiness times out despite connected ICE/DTLS, 64×36 live media and a status reporting 465 decoded frames; the diagnostic snapshot has hidden visibility. Individual predicate values were not captured, so the cause remains open (BUG-019). Dependent progression/sound checks are blocked; viewer Stop/reconnect pass. The failed reports remain retained and are not overwritten by retries.

The native replacement run still fails under BUG-016: initial window `7496` produces decoded pixels/PCM, but replacement `7502` times out at native startup. Catalog queries intermittently time out and recover without discarding the chosen ID. Moving IPC off the event loop is insufficient to fix native replacement. Restore/source closure acceptance remains unverified. Previously retained standalone bridge runs passed same-source restart, alternate source and restoration for audio off/on; the next investigation should compare Rust worker shutdown/concurrent discovery against that standalone lifecycle.

The separate native audio-on/off baseline passes all 22 checks, with both host/viewer pairs exiting 0. Audio-on reaches 54 decoded frames and 86,400 audio samples, PCM peak 0.050171 / RMS 0.017881. Audio-off reaches 55 frames with zero received audio samples and measured zero PCM. Both cases verify programmatic mute controls, disabled diagnostic polling and Stop/track cleanup. Tested binary SHA-256: `1d61e734a91a79700314f758849c87ed3f4d50450241375c8f975a2cdf0a4413`. The only subsequent Rust edit adds an explanatory comment to the QA holder. Physical audio devices, trusted gestures, permission transitions, packaged builds, multiple monitors, Linux and cross-network QA remain open.

## Mandatory Verification and Cleanup

Logs and command exit statuses are saved in `/private/tmp/video-p2p-bug017-checks/`.

The first complete gate passes all 17 commands (`first-pass/`). Revalidation after documentation edits again passes formatting, lint, Rust/JS/native tests and both WKWebView modes. Its ordinary Chrome probe passes media assertions but exits 1 while removing the temporary profile (`ENOTEMPTY`, `16.log`), so that attempt is a failure, not an overall PASS. The remaining file is `chrome/Default/Network Persistent State`; no associated processes remain. Removing the exact leftover scratch directory succeeds. The subsequent ordinary Chrome run (`16-retry.log`, exit 0) passes with 23 decoded frames / 39,840 audio samples; recovery (`17-final.log`, exit 0) passes with 19 frames / 36,000 samples. Both clean their profiles. This intermittent runner issue is tracked as BUG-020; these successful runs do not close it. Later record-only edits add outcomes, cleanup evidence and commit identity without changing the tested implementation.

| Command | Result |
| --- | --- |
| `cargo fmt --all` | PASS |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --all --all-features` | PASS, 127 tests plus doctests |
| `cargo test -p app-core --example playback_probe --all-features` | PASS, 4 tests |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features` | PASS, 0 crate tests |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS, 8 tests |
| `sh scripts/test-macos-capture.sh` | PASS |
| `cargo build -p signaling-server` | PASS |
| `cargo build -p app-core --example playback_probe` | PASS |
| `node scripts/test-viewer-playback.mjs` | PASS |
| `node scripts/test-viewer-playback.mjs --keyframe-recovery` | PASS |
| `node scripts/test-viewer-playback.mjs --chrome` | First gate PASS; revalidation media PASS but profile cleanup FAIL (BUG-020); retry PASS including cleanup |
| `node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery` | PASS |
| `cargo clean` | PASS, removed 8,327 files / 1.9 GiB |
| `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS, removed 10,903 files / 3.2 GiB |

Cargo metadata confirmed repository-local target directories; both are absent after cleanup. No matching verification client/server/probe/build processes remain. No release bundle was produced. JavaScript syntax, relative documentation links/anchors and shell-block syntax, the tracked-text Cyrillic scan (including hidden files), and `git diff --check` pass. Logs and failed evidence remain at the paths above. BUG-017 has a verified implementation; broader GUI acceptance remains open under BUG-018/BUG-019, so its status is FIXED rather than CLOSED. BUG-016, BUG-020 and the existing hosted Ubuntu requirement BUG-010 remain unresolved. No passing full-project release checkpoint is claimed.

README, viewer playback and user guidance link the follow-up and its limits; user guidance now distinguishes host rebinding from idle missing-source cleanup, matching existing code. The macOS capture guide was reviewed and its native/permission limitations remain accurate. Architecture, GUI guidance, the QA checklist, TODO and bug register record the command dispatch behavior and outstanding native/test failures.
