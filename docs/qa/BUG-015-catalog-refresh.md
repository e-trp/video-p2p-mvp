# BUG-015: Preserve Source State During Catalog Refresh Failure

## Scope and Reproduction

Investigated 2026-10-23 through 2026-10-25, base `0d04b6f`, untagged `0.1.0`. This is a fix to existing `BASE-DESKTOP-CONTROLS`/`BASE-MACOS-CAPTURE`, not a new milestone feature (`0/5`). macOS 27.0 (26A428), arm64, native WKWebView, local loopback signaling, Screen Recording already granted. The test uses two temporary signed Cocoa applications launched through LaunchServices: a red animated window with a generated 440 Hz tone and a blue animated silent window. Reports contain measurements and diagnostics, not captured media.

The original native lifecycle run (`/private/tmp/video-p2p-source-lifecycle-first`) switches from source `5699` to `5705` successfully, then gets a temporary blueprint fallback catalog while permission remains granted. The backend replaces the selected ID with example `mac-window-vlc`, then unrelated Terminal `4283` when native discovery recovers. Capture stopped; no automatic capture of that replacement was observed. The exact underlying ScreenCaptureKit enumeration error is not retained in the original session snapshot, so its OS cause remains unspecified.

## Cause, Fix and Controlled Regressions

The backend treats non-authoritative example fallback as source removal. It now preserves the last runtime identities as `cachedruntime`, keeping current permission and error diagnostics. Repeated failure does not replace the source, stop a permitted live capture, or rewrite preferences. Permission loss/unknown permission during failed discovery stops capture while retaining the selection. A successful catalog confirming removal still stops and rebinds; native startup independently validates source availability.

The frontend also replaces confirmed controls with idle/preview state when a refresh command fails. It now retains the last confirmed session and source options, marks incomplete refresh/stale catalog diagnostics, and uses the latest confirmed selection rather than an older catalog selection. Successful refresh restores current diagnostics. Initial startup without confirmed state keeps the existing preview/error behavior.

Commands:

```bash
cargo test -p app-core catalog_failure --all-features
node scripts/test-tauri-gui.mjs --scenario source-picker --output /private/tmp/video-p2p-bug015-gui-final-before
node scripts/test-tauri-gui.mjs --scenario source-picker --output /private/tmp/video-p2p-bug015-gui-after
```

The controlled backend tests fail before the fix in two cases: selection becomes `example-source` instead of `native-source`. All three pass afterward. They cover repeated failures, unchanged preferences and live runtime, recovery/rename, confirmed replacement/empty catalog, permission denial/requirement/unknown state with one stop, and initial fallback without a cache. Logs: `/private/tmp/video-p2p-bug015-before.log` and `video-p2p-bug015-after.log`.

The GUI regression executes real IPC and injects delivery errors after real session/catalog responses; it does not change successful response data. On the original frontend it fails with `Failed refresh discarded the confirmed source options`; the prior source/menu regression checks pass. The first two attempted GUI fault-injection runs did not install the wrapper because the nested Tauri API object is not writable; they remain setup failures, not product reproduction evidence. The final runner replaces/restores the outer Tauri wrapper as the existing delayed-response test does.

The first backend-only native rerun (`/private/tmp/video-p2p-bug015-lifecycle-after`) preserves selection through repeated fallback/recovery but fails because Start becomes disabled after UI refresh failure. Explicit host focus in `video-p2p-bug015-lifecycle-focused` reproduces that same UI failure, ruling out focus as a sufficient repair. The driver now focuses each participating test window before its control actions/observations; foreground state is checked, not assumed. Failed results remain retained.

The controlled GUI after-run passes all 7 checks, including error delivery, unchanged source/controls, stale indication and recovery. The final native run (`/private/tmp/video-p2p-bug015-lifecycle-final`) preserves the chosen source and enables Start, but replacement capture fails with `Failed during stream due to application connection being interrupted` (BUG-016). The old stream stops with 97 encoded frames / 176 Opus packets; the replacement enters `starting` then `failed`. Initial decoded red pixels/PCM pass. Silent replacement decoding, return to the original and final source disappearance are not verified. The command exits 1 and retains its failure; it is not recorded as a passing lifecycle run.

BUG-015 is FIXED in `1f665d7` (untagged `0.1.0`) with verified state retention; its broader actual lifecycle closure criterion remains open under BUG-016. The subsequent status-recording commit changes documentation only.

## Baseline Regression and Mandatory Checks

`python3 scripts/test-native-desktop-playback.py --output /private/tmp/video-p2p-bug015-native-baseline` passes all 22 audio-on/off checks with the final implementation. Audio-on receives actual PCM (RMS 0.017968), 184,800 samples and 66 decoded frames; audio-off continues to 66 frames with zero audio. Both host/viewer pairs and source fixtures stop normally. This verifies the previously working separate sessions, not replacement within one session. Tested QA executable SHA-256: `1cd07c25e7e48dfefb0c1115b522a525a3cfc1f0fd3368ae333fb4d4ce4a684f`.

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
| `sh scripts/test-macos-capture.sh` | PASS, native extraction/timing/queue/stop |
| `cargo build -p signaling-server` | PASS |
| `cargo build -p app-core --example playback_probe` | PASS |
| `node scripts/test-viewer-playback.mjs` | PASS, 64×36, 20 frames / 35,040 audio samples; stop releases media |
| `node scripts/test-viewer-playback.mjs --keyframe-recovery` | PASS, 64×36, 20 frames / 39,360 audio samples; RTCP recovery and stop cleanup |
| `node scripts/test-viewer-playback.mjs --chrome` | PASS, 64×36, 23 frames / 38,400 audio samples; stop releases media |
| `node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery` | PASS, 64×36, 20 frames / 36,960 audio samples; RTCP recovery and stop cleanup |
| `cargo clean` | PASS, repository-local target directory removed; confirmation run removed 0 files |
| `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS, separate target directory removed; confirmation run removed 0 files |

No verification fixture/client/server/probe processes remain. Both target directories are absent; no release bundle was generated. Logs and JSON reports remain under the `/private/tmp/video-p2p-bug015-*` paths above. Syntax checks for the changed JavaScript/Python, Markdown relative links/anchors and shell blocks, the tracked-text Cyrillic scan (including hidden configuration), and `git diff --check` pass. README and the viewer guide were reviewed and remain accurate; architecture, capture, GUI, user guidance, QA checklist, TODO and the bug register were updated.

## Remaining Scope

The lifecycle scenario switches source, restores the original on the same viewer peer, and terminates its owning fixture application before refreshing the catalog. It does not certify closing only one window of a multi-window application. Physical output devices, trusted gestures, permission transitions on real OS settings, other applications/protected content, multiple monitors, packaged distribution, Linux and cross-network QA remain open. The existing BUG-010 hosted Ubuntu acceptance remains separate. No version/tag or release checkpoint is created.
