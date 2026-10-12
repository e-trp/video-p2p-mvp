# Desktop GUI Follow-up — 0.1.0 Based on 6bd1a31

**GUI automation: PASS. Full release gate: BLOCKED.** All 32 real Tauri checks pass in debug and in two complete QA-instrumented `.app` runs. Native H.264/Opus playback now decodes in the desktop WebView. Actual Screen Recording, audible hardware, trusted gestures, remote networks, and installation/distribution acceptance remain unverified.

## Tested Tree and Environment

- Date: 2026-10-12; untagged development version `0.1.0`, base `6bd1a31`.
- Tested implementation: `f588cb81e86552c10820e0d57b6e8f42ab33d7d5` fixes BUG-008 and `480a5819665211a4dea923134e46bb61bafe57e9` repairs BUG-009. Checks ran on that source before committing; subsequent evidence/status edits are documentation only. BUG-005, BUG-008 and BUG-009 meet their focused closure criteria; required manual QA stays open.
- macOS `27.0` (`26A428`), WebKit `22625.1.29.11.27`, Chrome `155.0.8059.40`, Node `26.11.1`, Rust/Cargo `1.99.0`.
- Real Tauri windows: initial viewport 1120 × 728; native widths 1120, 800, and 600. Every final initial-render report confirms native focus and visibility.
- Temporary loopback signaling, ordinary local ICE interfaces, no deployed STUN/TURN or second machine. A placeholder STUN entry tests preference validation/persistence only.
- Capture catalog remains `blueprintfallback`, with two example sources and Screen Recording `required`. Explicit capture attempts verify permission-blocked diagnostics. Synthetic native encoding/transport supplies the playback fixture; no actual screen/audio source is claimed.
- Accessibility/system screenshot testing remains unavailable from the previous run. Screen Recording and Accessibility prerequisites were requested during this follow-up and no grant was reported. Programmatic DOM actions do not certify trusted gestures or device output.
- Preferences are isolated under `/private/tmp/video-p2p-gui-fix`; existing user preferences and the pre-existing untracked `.python-version` are preserved.

## Defects, Cause, and Regression Evidence

**BUG-008:** a traced real Tauri failure leaves native ICE checking while browser ICE reports connected and DTLS receives no response. The host first learns the browser's LAN peer-reflexive address; later same-machine mDNS resolution introduces loopback. Nomination replies arrive from loopback instead of the earlier pair's remote address and fail the required source/destination check. This also explains intermittent standalone WKWebView failures; it is not confined to Tauri.

The final native configuration disables remote mDNS queries. Browser answerers check the native numeric candidates, and the native host learns their authenticated peer-reflexive address. Numeric interface candidates, ICE credentials, STUN response validation, and DTLS remain intact. No SDP rewriting, candidate filtering, vendored library, dependency, or media-over-IPC change is introduced. This follows the current native-offerer/browser-answerer architecture; remote NAT/STUN/TURN acceptance is still open.

The portable `browser_mdns_answer_connects_through_authenticated_peer_reflexive_checks` regression replaces every signaled viewer candidate address with a `.local` name, exchanges real offer/answer and numeric host candidates, requires an exercised browser-style candidate and a connected native host, then closes both peers. All final browser and desktop checks pass after this configuration change.

A provisional loopback-only gathering change passed one desktop viewer run but still failed a standalone WKWebView repeat with zero dimensions/frames/samples. That implementation and its temporary test are discarded. Temporary ICE logging and its development dependency are also removed. Successful retries do not erase the original failures.

**BUG-009:** a directly launched inactive QA bundle again stalls, this time after three checks. A main-thread sample shows an idle event loop rather than a blocked native/backend operation. Activating VideoP2P with `osascript -e 'tell application id "dev.videop2p.desktop" to activate'` permits all 22 controls checks to complete. The QA plugin now focuses its own window before injecting the driver; the driver waits for native focus/visibility. Debug and two full bundled runs subsequently pass without external activation. This is a fixture foreground lifecycle repair; a production preference deadlock is not established. The commands/plugin remain opt-in under `gui-qa` and are absent from the ordinary bundle.

**BUG-005:** the earlier fixing commit `d53e11e14f499bfd76b739c6bea5f7484c5129c2` already supplies the explicit snake_case IPC contract and useful rejection text. The previously blocked decoded desktop/Stop acceptance now passes alongside configuration, invalid-input, source, interval, restart, and role checks.

## Real GUI Runs

The [runner](../GUI_TAURI.md#real-tauri-gui-regression-runner) invokes production DOM handlers, Rust IPC, signaling, and browser playback inside actual Tauri windows. VideoToolbox H.264 and Opus traverse real native WebRTC; neither backend nor decoded playback is mocked. Peer observation does not change the browser's methods/configuration.

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
cargo build -p signaling-server
cargo build -p app-core --example playback_probe
node scripts/test-tauri-gui.mjs --output /private/tmp/video-p2p-gui-fix/debug-final
```

Build the QA bundle from `apps/desktop/src-tauri`, then run it from the repository root:

```bash
cargo tauri build --features gui-qa --bundles app
```

```bash
node scripts/test-tauri-gui.mjs --binary apps/desktop/src-tauri/target/release/bundle/macos/VideoP2P.app/Contents/MacOS/desktop-app --output /private/tmp/video-p2p-gui-fix/release-final
node scripts/test-tauri-gui.mjs --binary apps/desktop/src-tauri/target/release/bundle/macos/VideoP2P.app/Contents/MacOS/desktop-app --output /private/tmp/video-p2p-gui-fix/release-repeat
```

| Run | Controls | Application restart | Viewer | Result |
| --- | --- | --- | --- | --- |
| `debug-final` | 22 PASS | 3 PASS | 7 PASS | 32/32 PASS |
| `release-final` | 22 PASS | 3 PASS | 7 PASS | 32/32 PASS |
| `release-repeat` | 22 PASS | 3 PASS | 7 PASS | 32/32 PASS |

Coverage includes initial render/guards, demo/busy state, persistent Stop/Reset, escaped configuration, invalid ICE without overwrite, unsaved drafts during refresh, all intervals 1/3/5/10/15, immediate prototype guards, source/audio preferences, host preparation/debug separation, blocked capture diagnostics, host reconnect/unavailable signaling, actual process restart, logs, and expanded diagnostics at three native widths. Viewer checks include correct decoded pixels, video/audio progress with Auto Refresh disabled, sound button state and pause/resume, track/peer cleanup, no revival after refresh, and fresh generations on reconnect/Reset. No unhandled frontend errors are recorded.

| Viewer run | Pixel RGBA | Advancing video frames | Advancing audio samples |
| --- | --- | --- | --- |
| Debug | Within expected encoded-color tolerance | 0 → 52 | 0 → 87,840 |
| First bundle | 191, 63, 30, 255 | 0 → 51 | 0 → 88,800 |
| Repeated bundle | Within expected encoded-color tolerance | 0 → 50 | 0 → 90,240 |

Progress counts are collected after an explicit diagnostic refresh; the initial zero snapshot does not mean no frames were decoded for the pixel assertion. Reconnect checks fresh generation/lifecycle in a new room. Successful resumed media after rebuilding both established peers is a separate manual check and is not claimed here. Sound state is tested programmatically; audible device output remains blocked.

## Automated and Browser Checks

All commands below run on the final code/configuration. Record-only edits do not change that tested code.

| Command | Result |
| --- | --- |
| `cargo fmt --all`; `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS |
| Both formatting commands with `-- --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --all --all-features` | PASS: 123 tests, all doctest targets |
| `cargo test -p app-core --example playback_probe --all-features` | PASS: 4 lifetime tests |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features` | PASS: compiles; zero crate-owned tests; GUI evidence is separate |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS: 8 lifecycle tests |
| `node --check apps/desktop/tests/gui-scenarios.js` | PASS |
| `sh scripts/test-macos-capture.sh` | PASS: extraction/timing/backpressure/stop; nonfatal hypervisor sysctl diagnostic |
| Signaling-server, playback_probe, debug QA desktop, QA release bundle builds | PASS |
| `node scripts/test-viewer-playback.mjs` | PASS: 22 frames/38,400 samples; final repeat PASS: 17/28,800; 64 × 36 and cleanup |
| Same command with `--keyframe-recovery` | PASS: 17 frames/34,080 samples; browser RTCP recovery and cleanup |
| Same command with `--chrome` | PASS: 21 frames/35,520 samples and cleanup |
| Same command with `--chrome --keyframe-recovery` | PASS: 22 frames/40,320 samples; browser RTCP recovery and cleanup |
| Same command with `--signaling-disconnect` | PASS: 43 frames/75,360 samples; signaling EOF and continued media on the same peer |
| Same command with `--chrome --signaling-disconnect` | PASS: 53 frames/94,080 samples; signaling EOF and continued media on the same peer |
| `cargo tauri build --bundles app,dmg` in `apps/desktop/src-tauri` | PASS: default production `.app` and `.dmg` |
| `hdiutil verify /private/tmp/video-p2p-gui-fix/artifacts/VideoP2P_0.1.0_aarch64.dmg` | PASS: checksum VALID |

## Every QA Checklist Section Reviewed

| Section | Result/scope |
| --- | --- |
| Automated checks | PASS for both Rust workspaces, JS lifecycle, native extraction, all six browser modes, and actual GUI runs |
| Backend negotiation smoke | Real native offer/answer/ICE/connection regression and GUI synthetic host negotiation PASS; standalone CLI smoke NOT RUN again (previous result retained in the original record) |
| Desktop session/source controls | Programmatic production DOM/IPC/fallback, role, drafts, Stop/Reset, preferences and layout PASS; real source selection/rebinding during capture BLOCKED |
| macOS actual capture/publication | Required/blocked permission path PASS; grant/relaunch, real window/application/display, changing/closing sources, audio filtering, multiple monitors, worker recording-indicator cleanup BLOCKED by Screen Recording and source/device prerequisites |
| Desktop viewer playback/recovery | Synthetic decoding, pixel/progress, muted/sound state, pause/resume, Stop/Reset/generation, RTCP recovery and signaling EOF PASS; actual moving source/audio, trusted sound/fullscreen gestures, hardware audibility, actual-source decoder recovery, outages/congestion and rebuilt-peer media reconnect BLOCKED |
| Debug burst | Disconnected GUI validation and separation from encoded media PASS; connected GUI audio-on/audio-off burst NOT RUN; previous CLI evidence is historical |
| Recovery/preferences | Stop/edited reconnect/unavailable signaling/persistence/restart/Reset PASS in debug and bundled runs; whole-server interruption during actual capture/playback BLOCKED |
| Linux | BLOCKED: no Linux execution environment; media backend/encoder implementation remains missing |
| Cross-network/performance | BLOCKED: no second machine/network, deployed STUN/TURN/restrictive NAT, audio devices or latency/A/V instrumentation; local prflx checks do not certify remote NAT traversal |
| Packaged release | Default/QA `.app` builds, `.dmg` checksum and two QA bundle runs PASS; ordinary bundle clean installation/manual capture/visual/gesture/device QA BLOCKED; signing/notarization/distribution NOT RUN; Linux bundles remain unimplemented |

Future milestone capabilities are not counted as completed features. Current baseline QA is still incomplete; no passing full-project checkpoint, version bump, or release tag is created. The counter remains `0/5`.

## Saved Evidence and Bundles

The [normalized JSON](gui-client-0.1.0-6bd1a31-results.json) retains all 96 final GUI outcomes and selected observed details, final browser results, and failing-before evidence. Full logs/snapshots remain machine-local:

| Directory under `/private/tmp/video-p2p-gui-fix` | Evidence |
| --- | --- |
| `before-trace` | Original native ICE/DTLS failure; temporary trace output |
| `local-ice` | Provisional candidate configuration desktop PASS; not the final implementation |
| `release-ice` | Inactive-bundle stall, dependent failed restoration, main-thread sample, provisional viewer PASS |
| `release-foreground` | Manually activated controls-only 22/22 PASS, establishing fixture precondition |
| `debug-final`, `release-final`, `release-repeat` | Final complete reports, summaries, isolated preferences and child logs |
| `artifacts` | Saved `VideoP2P-GuiQA.app`, default `VideoP2P.app`, and default `VideoP2P_0.1.0_aarch64.dmg` outside Cargo targets |

Default executable SHA-256: `96735a1cc2d029cbc9b3a1a8a8866844b694c79d52673af2c32455c4e7df5698`. DMG SHA-256: `1994ea2a75e8d13fd369179db48b187193457b306e0299c3645a62e2d8461161`. The QA window-state command marker is present in the saved QA executable and absent from the default executable. This bundle is a development artifact; successful distribution acceptance is not claimed.

## Documentation and Cleanup

README, TODO, bug register, GUI/user/viewer/networking guides and QA checklist reflect the passing desktop automation and separate open manual scope. Architecture, installation, capture, media and demo guides were reviewed; their boundaries, commands and outstanding device/distribution scope remain accurate.

Documentation verification passes: 164 relative links/anchors, 42 shell blocks, no Cyrillic authored prose across tracked/new text (including hidden configuration), and normalized JSON totals of 32 PASS per final run. `git diff --check` passes; the complete source/documentation diff and new evidence are reviewed.

All verification processes have finished; no owned desktop/server/probe process remains. After saving evidence/bundles and completing every compile/test/probe step, `cargo clean` removes 12,835 files / 2.9 GiB and `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` removes 17,659 files / 4.9 GiB. Both commands exit 0, both target directories are absent, and saved bundles remain available. User preferences, source/lockfiles and download caches are preserved. The original [failed GUI record](tauri-gui-0.1.0-74a8e37.md) remains unchanged.
