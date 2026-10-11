# Tauri GUI Validation — 0.1.0 Based on 74a8e37

**Result: FAIL.** This run exercises the actual Tauri application, including an instrumented release `.app`. It is not a passing release checkpoint. Debug controls/preferences pass after three fixes, but decoded Tauri playback fails under BUG-008 and the release controls run stalls under BUG-009. Required OS/device/network scenarios also remain blocked.

## Tested Tree and Environment

- Date: 2026-10-11; untagged development version `0.1.0`, based on `74a8e37`.
- Tested implementation: `d53e11e14f499bfd76b739c6bea5f7484c5129c2`, containing BUG-005 IPC/error fixes, BUG-006 input readiness, BUG-007 layout, and opt-in `gui-qa` tooling. Checks ran on that source before committing; subsequent commit/status recording is documentation only.
- macOS `27.0` (`26A428`), WebKit `22625.1.29.11.27`, Chrome `155.0.8059.40`, Node `26.11.1`, Rust/Cargo `1.99.0`.
- Actual native Tauri window: initial viewport 1120 × 728 logical pixels; decorated window configured as 1120 × 760. Native width checks: 1120, 800, 600.
- Signaling/ICE: temporary loopback TCP server, local WebRTC candidates, no deployed STUN/TURN or remote network. A placeholder loopback STUN entry is used only for preference persistence/validation checks.
- Capture: `blueprintfallback`, two example sources, Screen Recording `required` at startup; explicit start/poll verifies blocked permission diagnostics. No actual captured screen/audio reaches the viewer in this run.
- Accessibility: `osascript -e 'tell application "System Events" to get UI elements enabled'` returns `false`, including the later recheck. `screencapture -x /private/tmp/video-p2p-gui-qa/initial-desktop.png` fails with `could not create image from display`. Visual screenshots/system input remain blocked.
- Preferences: isolated directories under `/private/tmp/video-p2p-gui-qa`; existing user preferences and the pre-existing untracked `.python-version` are preserved.

## Method and Evidence

The [GUI runner](../GUI_TAURI.md#real-tauri-gui-regression-runner) starts real Tauri windows and dispatches DOM input/change/click events to production handlers. All Rust IPC/session/signaling/player behavior is real. The playback fixture produces native VideoToolbox H.264 and Opus through Rust WebRTC; it does not inject decoded frames or replace the player. A constructor observer records the actual browser peers without altering their configuration/methods. No new dependencies are added.

Programmatic DOM actions do not certify trusted user gestures, OS Accessibility interactions, visual appearance, Screen Recording grant/relaunch, or audible hardware. Native window geometry is tested, while screenshot review remains blocked. Debug and instrumented release results are separate; a default `.app`/`.dmg` is saved separately from QA instrumentation.

Saved machine-local evidence:

| Directory under `/private/tmp/video-p2p-gui-qa` | Evidence |
| --- | --- |
| `debug-first`, `debug-second` | Initial driver/ACL setup attempts; first times out without report, second is cancelled after a build error; no application PASS claimed |
| `debug-third`, `debug-before` | Before-fix IPC/preference/readiness/layout failures; expanded document widths 2527/1120, 2521/800, and 2521/600 |
| `debug-after`, `viewer-diagnostics`, `peer-diagnostics` | Corrected IPC and actual peer/media investigation; first added permission/Stop assertions needed adjustment for asynchronous status and inactive retained generations |
| `debug-final` | Final-source run with one overstrict permission-render assertion; corrected to explicit Poll Capture and accepted blocked states |
| `debug-reviewed` | Final 32 checks: 29 PASS, 1 FAIL, 2 BLOCKED; 22 controls and 3 restart checks all pass |
| `packaged-viewer` | First QA `.app` viewer failure; failure eventually closes tracks/peer |
| `packaged-reviewed` | Eight controls PASS then 90-second timeout; dependent restore fails because setup did not reach final save; viewer has 4 PASS, 1 FAIL, 2 BLOCKED |
| `packaged-controls-isolated` | Controls-only retry: seven PASS then timeout during draft-preservation check; BUG-009 is reproduced at a nearby step |
| `cli` | Real CLI negotiation and debug burst; host/viewer both exit 0 |
| `final-production` | Default `.app` and verified `.dmg`, saved outside Cargo targets |

Each completed GUI scenario saves `*.json`; partial reports identify the running check. `summary.json` records process outcome, and `process.log` preserves child output. Browser failure output is summarized below. No successful retry erases a failed attempt.

The [normalized, repository-owned evidence](tauri-gui-0.1.0-74a8e37-results.json) preserves relevant failing-before checks, all final debug outcomes, release partial/failed results, and the controls-only retry. Machine-local files retain full snapshots/logs. The default saved executable SHA-256 is `baa31e530499905faa757dc5a83c22592c506e398a313d53080f1df2a96e92ef`; the DMG SHA-256 is `bc4d281d46d4f613565c30082460c36c7f814e470b1da9df43a3e371b2081543`.

## GUI Scenario Results

Commands from the repository root:

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
cargo build -p signaling-server
cargo build -p app-core --example playback_probe
node scripts/test-tauri-gui.mjs --output /private/tmp/video-p2p-gui-qa/debug-reviewed
node scripts/test-tauri-gui.mjs --binary apps/desktop/src-tauri/target/release/bundle/macos/VideoP2P.app/Contents/MacOS/desktop-app --output /private/tmp/video-p2p-gui-qa/packaged-reviewed
node scripts/test-tauri-gui.mjs --scenario controls --binary apps/desktop/src-tauri/target/release/bundle/macos/VideoP2P.app/Contents/MacOS/desktop-app --output /private/tmp/video-p2p-gui-qa/packaged-controls-isolated
```

All three GUI commands exit 1 with failed evidence; this is the expected reporting behavior for the observed defects.

| Scenario group | Debug result | Instrumented release result |
| --- | --- | --- |
| Runtime badge, loaded diagnostics/specification, idle player/guards | PASS | PASS |
| Local state-only demo, busy-state action exclusion | PASS | PASS |
| Stop remains stopped across three refreshes; Reset/idle | PASS | PASS |
| Escaped room rendering, signaling/ICE save, invalid ICE rejection without overwrite | PASS | PASS |
| Draft room/signaling/ICE preservation during Auto Refresh | PASS | PASS in full run; retry stalls during this check |
| All allowed refresh intervals (1/3/5/10/15), enable/disable persistence | PASS | Full run stalls during this check, BUG-009 |
| Immediate empty/valid/empty prototype readiness without polling | PASS | NOT RUN after stall |
| Source/audio opt-out and persisted selection | PASS | NOT RUN after stall |
| Prepare Host signaling/offer without capture; debug action separate from encoded counts | PASS | NOT RUN after stall |
| Start Native Capture, Poll Capture, Start Host Prototype blocked-permission diagnostics, Stop Capture | PASS for blocked/fallback path | NOT RUN after stall |
| Host Stop and edited reconnect; unavailable signaling/recovery | PASS | NOT RUN after stall |
| Saved preferences, Reset restoration, actual application restart | PASS | BLOCKED by incomplete setup; subsequent failed restore is dependent evidence |
| Diagnostics expand/Clear Logs and three native window widths | PASS | NOT RUN after stall |
| Join Viewer with Auto Refresh off and actual decoded pixels | FAIL, BUG-008 | FAIL, BUG-008 |
| Advancing video/audio without diagnostic polling | BLOCKED by BUG-008 | BLOCKED by BUG-008 |
| Sound state and pause/resume on decoded media | BLOCKED by BUG-008 | BLOCKED by BUG-008 |
| Created viewer tracks/peer close on Stop; refresh cannot revive them | PASS | PASS |
| Edited-room reconnect creates a new viewer generation; Reset clears player | PASS | PASS |
| Captured unhandled frontend errors | None | None before timeout/in completed viewer scenario |

The final viewer negotiation wait inspects native peer/video state without repeatedly invoking diagnostic refresh. Reconnect uses a new room and checks generation/lifecycle only; successful resumed media after rebuilding both established peers is not claimed.

## Findings and Fixes

- **BUG-005:** Tauri defaults to camelCase command arguments, while the production frontend sends snake_case. Required arguments fail; optional settings disappear silently. Commands now explicitly use `rename_all = "snake_case"`, and string rejections retain their reason. Real save/invalid-input/source/refresh/restart/host/viewer IPC regressions pass. Full decoded desktop acceptance remains blocked by BUG-008, so BUG-005 is not fully closed.
- **BUG-006:** form input marked drafts but left prototype buttons based on the previous render. Input now updates guards immediately; valid-to-empty and empty-to-valid transitions pass without Auto Refresh.
- **BUG-007:** diagnostic grid minimum widths and long detail values expand the document beyond its window. Explicit zero-minimum grid tracks, contained controls/cards, and wrapped details remove overflow at all three tested native widths with expanded diagnostics.
- **BUG-008:** Tauri reaches SDP/ICE and receives two tracks but does not decode media. In `peer-diagnostics`, ICE is `connected`, signaling `stable`, and a nominated pair succeeds; DTLS is still `connecting` as client, with 1,944 bytes/9 packets sent and zero received. Video dimensions/time remain zero. The `.app` can progress to terminal peer failure. Cause is unknown. Later standalone WKWebView checks also fail before media establishment, so a framework-only cause is not established. Rust/Chrome control results do not resolve this failure.
- **BUG-009:** the release controls driver stops producing results near one-second Auto Refresh/draft/preference operations. A controls-only retry also times out. No Rust panic or captured frontend exception establishes a cause. Keep product, WebKit scheduling/IPC, and fixture/environment causes under investigation.

See the [bug register](../BUGS.md) for fixing commits/status and next actions. No release/version tag or new milestone feature ID is created.

## Automated, Browser, and Packaging Checks

| Command/check | Result and evidence |
| --- | --- |
| `cargo fmt --all`; desktop equivalent; both `-- --check` commands | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| Desktop strict Clippy with `--manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS with QA feature compiled |
| `cargo test --all --all-features` | PASS: 122 unit/integration tests; all doctest targets pass |
| `cargo test -p app-core --example playback_probe --all-features` | PASS: 4 lifetime tests |
| Desktop `cargo test --all-features` with its manifest path | PASS: compiles; zero crate-owned Rust tests, GUI checks are separate |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS: 8 lifecycle tests |
| `node --check` for both new GUI scripts | PASS |
| `sh scripts/test-macos-capture.sh` | PASS: native extraction/timing/backpressure/stop; nonfatal hypervisor sysctl diagnostic |
| Builds: signaling-server, playback_probe, p2p-cli, debug QA desktop | PASS |
| `node scripts/test-viewer-playback.mjs` | Initial PASS: 24 frames/42,720 samples; later FAIL: 0 dimensions/frames/samples, transport connecting, cleanup passes |
| Same command with `--keyframe-recovery` | Initial PASS: 23 frames/43,200 samples; later FAIL before decoding; retained under BUG-008 investigation |
| Same command with `--chrome` | PASS: final 21 frames/34,560 samples |
| Same command with `--chrome --keyframe-recovery` | PASS: final 19 frames/34,080 samples, RTCP recovery verified |
| Same command with `--signaling-disconnect` | One FAIL before cutoff (`Signaling disconnect requires established playback`), then PASS; final 55 frames/97,440 samples, EOF and continued media verified |
| Same command with `--chrome --signaling-disconnect` | PASS: final 50 frames/87,840 samples, EOF and continued media verified |
| `./scripts/build-desktop-release.sh` | First sandbox DMG stage FAIL; escalated retry builds `.app` and `.dmg` |
| `cargo tauri build --bundles app,dmg` in desktop crate | PASS on final product code with default features |
| `cargo tauri build --features gui-qa --bundles app` in desktop crate | PASS; actual bundled GUI results above remain FAIL |
| `hdiutil verify /private/tmp/video-p2p-gui-qa/final-production/VideoP2P_0.1.0_aarch64.dmg` | PASS: image checksum valid |
| CLI `webrtc-host --room qa-gui-cli --signal <temporary loopback address> --push-debug-capture` and matching `webrtc-viewer` | PASS: both connected/exit 0; host publishes one debug video/audio sample. Non-playable diagnostic payloads, no decoded-playback claim |
| `git diff --check`, tracked-text Cyrillic scan, documentation links/anchors/shell blocks | PASS: 155 relative links/anchors, 41 shell blocks, no Cyrillic; normalized JSON totals match the run |
| `cargo clean`; desktop workspace clean | PASS: root removes 8,113 files/1.9 GiB; desktop removes 18,622 files/5.5 GiB |

Mandatory checks have been executed, but browser/GUI failures prevent a passing full-project checkpoint. Root tests do not cover the desktop workspace; desktop Rust compilation does not replace its GUI tests.

## Every QA Checklist Section Reviewed

| Checklist section | Result/scope |
| --- | --- |
| Automated checks | Rust/Node/native checks PASS; WKWebView media failures retained as FAIL |
| Backend negotiation smoke | PASS via actual CLI host/viewer and local signaling; diagnostic burst is not playable media |
| Desktop session/source controls | PASS for debug production DOM/IPC/fallback controls; packaged controls FAIL/stall; actual capture-dependent paths BLOCKED |
| macOS actual capture/publication | Blocked permission diagnostics PASS; grant/relaunch, real window/application/display, source close/rebinding, audio filtering, monitor variants and running-worker cleanup BLOCKED by missing granted Screen Recording and suitable devices/sources |
| Desktop viewer playback/recovery | Negotiation/decoded media FAIL (BUG-008); created-peer Stop/Reset/generation checks PASS; gesture/fullscreen, actual moving-source audio, decoder recovery on that source, outages/congestion and successful media reconnect BLOCKED |
| Debug burst | CLI connected publication and debug/core validation PASS; full connected GUI audio-on/audio-off publication NOT RUN, Tauri media connection fails |
| Recovery/preferences | Debug PASS; packaged full preference/restart coverage FAIL/BLOCKED (BUG-009); whole-server interruption during actual capture/playback BLOCKED |
| Linux | BLOCKED: no Linux execution environment; media backend/encoder implementation also missing. macOS GUI results do not certify Linux |
| Cross-network/performance | BLOCKED: no second machine/network, deployed STUN/TURN, restrictive NAT setup, audio devices or latency/A/V instrumentation. Local tests are not network/performance acceptance |
| Packaged release | `.app`/`.dmg` builds and DMG checksum PASS; instrumented `.app` GUI FAIL; clean installation, ordinary bundle visual/gesture/device QA, signing/notarization and distribution NOT RUN/BLOCKED; Linux bundles unimplemented |

Screen Recording and Accessibility were requested as missing OS prerequisites during the run and were not available by final verification. Audio counters, example catalogs, and successful standalone browser probes are not substituted for these checks.

## Documentation and Cleanup

README, TODO, GUI guide, user guide, viewer guide, QA checklist, and bug register now reflect verified controls and the failed desktop gate. Architecture, installation, capture, media, networking, and demo guides were reviewed; their component boundaries, commands, and open actual-device/release scope remain accurate. Icons regenerated by the build helper were restored to the pre-existing source bytes; bundles are saved separately. No user changes are included.

All verification processes have finished and owned application/server/probe processes are stopped. Both workspaces are cleaned after all compile/test/probe steps; target directories are absent and default bundles/evidence remain outside them. Registry/download caches and user preferences are preserved. The feature counter stays `0/5`; this is baseline validation and defect work. BUG-005 is FIXED pending decoded acceptance; BUG-006/BUG-007 are CLOSED against their focused regressions; BUG-008/BUG-009 remain INVESTIGATING. The release gate remains failed/open.
