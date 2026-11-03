# BUG-022: Restore Hidden Windows Before Native QA Controls

2026-11-03, base `4cc9e26`, untagged `0.1.0`, macOS 27.2 (26B123), arm64. Only the opt-in GUI QA driver/runner changes; production capture, playback, window behavior and permissions are unchanged.

## Reproduction and Repair

The [native lifecycle comparison](native-lifecycle-2026-11-03.md) reaches source restoration and host closure through LaunchServices, then fails foreground checks. The host failure records `visible=false`, `focused=false` and a hidden document. The reason the uncontrolled windows became hidden remains unknown; the test's inability to restore a hidden window is reproduced independently.

The new `window-visibility` scenario hides the actual Tauri window, confirms native/document invisibility, then invokes the same helper used for native source changes, observations and Stop. Before the fix it fails with `Timed out: foreground QA controls`. Evidence and the test-only patch are retained in `/private/tmp/video-p2p-bug022-hidden-before/` and `/private/tmp/video-p2p-bug022-before-test.patch`.

`focusQaWindow` now uses the existing QA-only show/focus command and waits for both native visibility/focus and document visibility. The regression verifies that restoring the window preserves session mode and viewer generation. It also runs in the standard controls suite. No time limit, pixel/PCM threshold or source-lifecycle acceptance is relaxed.

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
node scripts/test-tauri-gui.mjs --scenario window-visibility --output /private/tmp/video-p2p-bug022-hidden-before
node scripts/test-tauri-gui.mjs --scenario window-visibility --output /private/tmp/video-p2p-bug022-hidden-after
```

The build is rerun after the helper fix. The after-run passes **3/3**, including the controlled hidden-window transition and no frontend errors. Both runners stop their child processes. Actual lifecycle, final mandatory verification and cleanup follow below.

## Final Verification

The after-build is copied into a separate signed QA bundle under `/private/tmp/video-p2p-bug022/VideoP2P Source Lifecycle QA.app`; the permission-granted retained bundle in `~/Applications` is not modified. `codesign --verify --deep --strict` passes. Executable SHA-256: `9dc1294c2d1a5dfebe54e50c58f421259f8040171d0aac0799fba40cf03c5fab`. Direct launch of this bundle observes permission `granted` and passes all **17 actual native lifecycle checks**, including source replacement/restoration, source closure, host Stop, viewer track/peer cleanup, pixels and PCM. This run does not claim to resolve intermittent initial LaunchServices negotiation (BUG-023).

```bash
python3 /private/tmp/video-p2p-bug022/run-direct.py /private/tmp/video-p2p-bug022/native-after
node scripts/test-tauri-gui.mjs --scenario controls --output /private/tmp/video-p2p-bug022-controls-after
python3 /private/tmp/video-p2p-bug022/checks.py
```

The controls/restart/source-removal suite records **29 PASS / 0 FAIL / 1 BLOCKED**, exit 1. Its new hidden-window regression, control actions, layout and exact saved-source restoration/removal pass. The required-permission case is blocked because this process has access and a native catalog. The unchanged permission-required branch already passed in the retained bundle's separate **22/22** run before the user granted access; broader denial/grant transitions remain open. Neither result is rewritten as a complete manual permission pass.

All **18 commands** in `/private/tmp/video-p2p-bug022/checks/results.json` pass. Numbered logs retain output. A [compact checked-in evidence extract](BUG-022-native-window-visibility-results.json) contains exact commands/exit codes, controlled before/after results and actual lifecycle outcomes.

| Check | Result |
| --- | --- |
| Both workspace format and strict Clippy commands | PASS |
| Root Rust tests and doctests | PASS, 127 tests |
| Playback example tests | PASS, 4 tests |
| Separate desktop Rust tests | PASS, 0 crate tests |
| Viewer JS lifecycle tests | PASS, 8 tests |
| Native extraction | PASS |
| Signaling/playback probe builds | PASS |
| WKWebView normal/recovery | PASS, decoded video/audio and cleanup |
| Chrome normal/recovery | PASS, decoded video/audio and cleanup |
| Probe cleanup tests | PASS, 6 tests |

After all runners finish, process inspection finds no test clients, fixtures, servers or probes left running. `CARGO_TARGET_DIR` is unset and both targets resolve inside the repository. `cargo clean` removes 8,327 files / 1.9 GiB; `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` removes 10,903 files / 3.2 GiB. Both commands exit 0 and both target directories are absent. The tested bundle and evidence remain outside targets; user preferences and the permission-granted retained application are preserved.

The changed JS passes syntax checks. Tracked-text language scanning includes hidden files and finds no Cyrillic prose. Markdown links/anchors, shell-block syntax and `git diff --check` are checked before committing. README, TODO, GUI/capture/viewer/architecture guides, checklist and affected bugs are reviewed; capture/codec/player contracts are unchanged. No feature counter, package version or release tag changes.
