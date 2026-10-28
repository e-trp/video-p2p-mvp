# BUG-019: Observe Playback in a Visible QA Window

2026-10-28, base `76375d6`, untagged `0.1.0`, macOS 27.0 (26A428), arm64, Node 26.12.1, native WKWebView. Ownership: `BASE-VIEWER-PROBES`. This changes the opt-in GUI test driver, not production playback, capture or permissions.

## Launch and Permission Scope

All GUI runs directly launch `apps/desktop/src-tauri/target/debug/desktop-app` with separate test configuration directories. The executable is linker/ad-hoc signed, identifier `desktop_app-ba6391fa92860f95`, with no bound Info.plist or team identifier. No installed application is replaced or launched and no user permissions are changed. The local host generates synthetic H.264/Opus; the viewer test starts no screen/audio capture. Initial catalog discovery is recorded separately and does not establish installed-application permissions or native capture acceptance. BUG-016's bundled-launch comparison remains blocked.

## Controlled Comparison

The original failure in `/private/tmp/video-p2p-bug017-full-gui/viewer.json` records connected playback, 64×36 video, 465 decoded frames and hidden document visibility, but omits the element frame counter used by its readiness predicate. The new diagnostic observer records native focus/visibility, document visibility, connection state, dimensions, ready state, paused state, media time, the element/WebKit counters and WebRTC inbound decoded frames. A QA-only command hides/shows only the test window. It is gated by the existing `gui-qa` feature and isolated QA state.

Before changing the readiness behavior, the instrumented original predicate is run with the window hidden after the boot focus check and before joining. The instrumentation patch is retained in `/private/tmp/video-p2p-bug019/instrumented-before.patch`. The binary SHA-256 is `48cf2c1d99ac0ae73c41fdde72dd1adb2b65abb686c576655504b3b9dfa97401`.

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
cargo build -p signaling-server -p app-core --example playback_probe
cargo build -p signaling-server
node scripts/test-tauri-gui.mjs --scenario viewer-visibility --output /private/tmp/video-p2p-bug019/hidden-before
node scripts/test-tauri-gui.mjs --scenario viewer --output /private/tmp/video-p2p-bug019/visible-before
```

| Same-binary comparison | Result |
| --- | --- |
| Controlled hidden window | FAIL, exit 1: the unchanged 20-second readiness wait times out. Last observation: connected, 64×36, readyState 4, playing at 18.533 s, **476 WebRTC decoded frames / 0 element frames**, native visible/focused false and document hidden. 48 observations retained. Stop/reconnect checks pass; dependent media checks remain BLOCKED. |
| Visible control | PASS, 7/7: native visible/focused true, document visible, **15 WebRTC decoded frames / 10 element frames** at readiness; pixels, progression, sound controls and cleanup pass. |

The controlled difference establishes a GUI fixture visibility defect: its startup focus check does not guarantee a visible window at the later element-frame observation. It does not establish a decoder or Screen Recording permission failure. The exact reason the original uncontrolled run lost visibility is unknown; that event need not be attributed to the product to repair the missing test precondition.

## Implementation and Regression

Before synthetic viewer readiness, progression and sound-control observations, the driver checks current native/document visibility and focus, showing/focusing its own QA window when needed. The unchanged 20-second bound still requires connected playback, 64×36 dimensions and more than three element frames, with explicit playing/ready-state checks. Canvas pixels must match the synthetic source. WebRTC counters do not substitute for element frames or pixels. A bounded observation history and failure snapshot preserve individual readiness values when a check fails.

`--scenario viewer-visibility` negotiates while deliberately hidden, records actual hidden decoding, then verifies visible playback on the same peer/generation. It hides the already playing window again and verifies restoration and advancing element frames without rebuilding the peer. The ordinary/default viewer path uses the same visibility precondition. Progression now also samples actual nonzero decoded PCM through a zero-gain Web Audio analyzer, closes that context in `finally`, and retains counters, programmatic sound/pause/resume, Stop and reconnect checks. These checks do not certify audible devices or trusted user gestures. Blocked follow-up messages reference the failed readiness prerequisite instead of incorrectly blaming the already closed BUG-008.

The first corrected run, `/private/tmp/video-p2p-bug019/hidden-after/`, passes 8/8. Hidden playback reports 5 WebRTC frames / 0 element frames; after showing, the same peer reports 20/10 frames and pixel `[191, 63, 30, 255]`. A second hide/restore passes. Visible element frames advance from 60 to 123, audio sample counters from 112,800 to 163,200, and measured PCM peaks are 0.10068 / 0.10056. A subsequent cleanup-only edit ensures the analyzer/context are released even if setup fails; final validation follows below.

## Final Verification

The first gate stops at root tests because the pre-existing source-label test compares two different live window titles. Its original output remains in `/private/tmp/video-p2p-bug019/checks/7.log`. This independent test expectation is repaired in `e4a1ad6`; see [BUG-021](BUG-021-source-label-test.md). The final gate tests both corrections together, with no production capture/player changes.

Exact commands, durations and exit codes are saved in `/private/tmp/video-p2p-bug019/checks-final/results.json` with numbered logs. All 17 mandatory commands pass on the final code.

| Check | Result / evidence |
| --- | --- |
| Both workspace format and strict Clippy commands | PASS; logs 1–6 |
| Root Rust tests and doctests | PASS, 127 tests; log 7 |
| Playback-probe example tests | PASS, 4 tests; log 8 |
| Separate Tauri workspace tests | PASS; log 9 |
| Node viewer lifecycle tests | PASS, 8 tests; log 10 |
| Permission-free native extraction | PASS; log 11 |
| Supporting signaling/probe builds | PASS; logs 12–13 |
| WKWebView ordinary / recovery | PASS, 23/23 decoded frames and 39,360/43,200 audio samples; logs 14–15 |
| Chrome ordinary / recovery | PASS, 18/23 decoded frames and 31,200/40,800 audio samples; logs 16–17 |
| GUI QA build | PASS; log 18 |
| Controlled visibility regression | PASS, 8/8; `/private/tmp/video-p2p-bug019/visibility-final/`, log 19 |
| Full GUI regression | 43 PASS / 0 FAIL / 1 BLOCKED; `/private/tmp/video-p2p-bug019/full-final/`, log 20 |

Final hidden-start observations show connected decoding with 4 WebRTC frames / 0 element frames. Restoring the window and a second hide/restore pass without replacing the peer/generation. The progression check records element frames 59 → 115, audio samples 79,680 → 174,240 and decoded PCM peaks 0.09961 / 0.10068. Stop ends the tracks/closes the peer; reconnect creates a new generation. Both browser recovery modes receive RTCP feedback and all four browser modes release media on Stop. Passing Chrome cleanup does not by itself close the independent BUG-020.

The full GUI run passes all applicable controls, source-picker/contention, exact preference restoration, missing-source removal and viewer checks. Exit 1 reflects the single permission-denied scenario, which requires a fallback catalog while this direct-launch context has granted discovery. No permission is revoked to manufacture that condition. This remains a required blocked release scenario, not a failure of the repaired viewer test. The prior BUG-017 command/error/contention regression and its pending broader controls/preferences/playback acceptance now have passing evidence; native source replacement remains separate under BUG-016.

After all runners exited, process inspection found no remaining test/build processes. Both target directories were repository-local and `CARGO_TARGET_DIR` was unset. `cargo clean` removed 9,603 files / 2.0 GiB and the separate desktop clean removed 11,143 files / 3.3 GiB, both exit 0. Both target directories are absent; logs and final executable identity (`final-binary-identity.log`) remain under `/private/tmp/video-p2p-bug019/`. No release bundle was produced.

Both changed JavaScript files pass `node --check`. Tracked-text language scanning (including hidden files), local Markdown links/anchors, shell-block syntax and `git diff --check` pass. README, TODO, architecture, GUI/viewer guides, the QA checklist and affected bug entries were reviewed. Architecture and permission-context guidance remain accurate; final record-only edits change no tested code. No release gate, version bump or new feature ID is claimed.
