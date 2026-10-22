# Source Selection During Auto Refresh — BUG-011 / BUG-012

Dates: 2026-10-20 investigation; 2026-10-22 completion checks. Base: `e45512d`, untagged `0.1.0`. Ownership: `BASE-DESKTOP-CONTROLS`. Fixing/tested implementation commit: `f450187aef31a9c5dcd5e9927d7f4f59d8315a4c`. Record-only follow-up changes do not alter the tested code. No version bump, release tag, publication or new milestone feature is part of this task.

## Controlled Regressions

The opt-in Tauri `source-picker` scenario uses the production frontend and real Rust IPC. It gates delivery of real backend responses without changing their data. The source catalog is native ScreenCaptureKit on this machine, with Screen Recording already granted. No capture stream is started for source-selection checks.

- On the original code, a focused picker loses its option nodes during one-second Auto Refresh.
- With an older refresh response delivered between a source change and the handler's catalog response, the original code selects `macos-window-1840` but submits `macos-window-1865`. The backend consequently retains the wrong source. This is a confirmed product race independent of the external Accessibility driver.
- Both regressions pass after retaining unchanged options, deferring changes while focused, reading source/audio synchronously, and draining/discarding refresh responses that predate user actions.
- Audio opt-out survives focused refresh and blur.
- A deliberately stale DOM option exercises the real backend's missing-source rejection. The initial boundary run preserves the previous backend source but incorrectly reports success (`capture source selected: qa-source-no-longer-present`). BUG-012 tracks that related IPC feedback defect. The wrapper now checks the returned source ID before reporting success.

The original 2026-10-19 Accessibility incident is preserved in the [actual-window record](actual-window-0.1.0-0185449.md). It is not possible to establish which of these races, or a driver/focus race, caused that single historical mismatch solely from its logs.

## Commands and Evidence

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
cargo build -p signaling-server
node scripts/test-tauri-gui.mjs --scenario source-picker
```

Machine-local evidence is under `/private/tmp/video-p2p-bug011/`. `gui-before` preserves the first fixture attempt: focused-option replacement fails, but its response gate cannot intercept the read-only Tauri module export. `gui-before-gated` replaces only the frontend reference to the invoke wrapper and reproduces both failures. `gui-after` passes both regressions. `gui-boundaries` preserves the first missing-source feedback failure. The 2026-10-20 `gui-final` run passes all six source checks. The resumed verification below covers the additional native-open guard. Native screenshots contain unrelated desktop content and are not committed.

## Native Menu Follow-up (2026-10-22)

Environment: macOS 27.0 (26A428), Chrome 156.0.8078.48, Node v26.12.1, rustc 1.99.1 (58cf8f9c3 2026-10-13) (Homebrew), local debug Tauri build, WKWebView reporting AppleWebKit/605.1.15, native ScreenCaptureKit catalog, Screen Recording granted, one-second Auto Refresh. No capture stream is started. Preferences are isolated under `/private/tmp/video-p2p-bug011/resumed/`; existing user settings are preserved.

The focus-only implementation still permits an Accessibility mismatch: in `resumed/native`, the helper presses `TextEdit - URL Handling Utility (window, audio)` but the UI and preferences select Terminal, `macos-window-1969`. Accessibility can open a popup without DOM focus. The final guard also checks the browser's native `:open` state, guarded by `CSS.supports` for older WebViews. `gui-open-state/source-picker.json` confirms support in the tested WebView.

After rebuilding, `native-final` opens the menu using Accessibility, waits three seconds with Auto Refresh enabled, presses the TextEdit menu entry, waits another three seconds, and verifies both the native UI label and preferences: TextEdit, `macos-window-2678`, audio enabled. An earlier selection in `native-activated` also passes. This verifies selection/persistence, not capture or playback from that source.

Preserved unsuccessful fixture attempts: `native-open-state` and `native-verified` cannot find the popup entries after waiting; `native-activated` passes its first selection but loses popup entries during the second. They do not count as complete repeated-selection runs. The initial attempt overlaps browser automation; foreground/menu lifetime is not established for the others. The final `native-focus-checked` run passes all three selections (TextEdit → System Settings → TextEdit), each after a three-second open menu. Its foreground process log confirms that other applications can become active during these runs; all three selected labels/IDs still persist correctly. Earlier interrupted attempts remain preserved, rather than being counted as passes. The temporary helper's first `select.py` filename shadows Python's standard module; renaming it to `native-selection.py` repairs that fixture startup failure without changing application code.

The native helper uses `AXPress` on **Available Source** and on the exact menu label, with `NSRunningApplication.activate` before opening. Dumps and isolated `session.conf` files preserve the selected ID. Desktop titles/screenshots remain machine-local because they contain unrelated user content.

## Final Verification

The initial unrestricted GUI launch fails its foreground precondition before any scenarios run (`resumed-source-picker`). A temporary launcher activates the exact spawned app through Accessibility before letting the existing GUI driver proceed; it does not change IPC, response data, or assertions. The launcher and scripts are saved beside the local evidence.

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
node scripts/test-tauri-gui.mjs --binary /private/tmp/video-p2p-bug011/resumed/focused-app --output /private/tmp/video-p2p-bug011/resumed/gui-all
node scripts/test-tauri-gui.mjs --binary /private/tmp/video-p2p-bug011/resumed/focused-app --scenario source-picker --output /private/tmp/video-p2p-bug011/resumed/gui-open-state
python3 /private/tmp/video-p2p-bug011/resumed/native-regression.py
```

- Focused source run: **PASS**, 6/6, including delayed real IPC delivery, focused refresh, audio opt-out, missing-source rejection/recovery, and no unhandled frontend errors.
- Full GUI run: **37 PASS, 1 BLOCKED**, overall nonzero exit as required. Source picker 6/6, controls 21 PASS plus 1 BLOCKED, restart 3/3, decoded viewer 7/7. Denied-permission startup requires a fallback catalog; the machine has granted Screen Recording. Permissions were not revoked. This is not a full release QA pass.
- Initial sandbox Rust run fails local listener binds with `EPERM` and has no usable ICE candidates. All four sandbox browser probes fail local listener creation with `EPERM`. The same commands pass when rerun with approved local socket/codec access. These environment-limited attempts are preserved in `resumed/check-*.log`; they do not establish a product regression.

All 17 mandatory commands pass on the final implementation. Logs are in `resumed/final-checks/`; the [committed result summary](BUG-011-source-selection-results.json) records command exit codes, GUI results, and native selections. Source/native evidence is separate from synthetic playback and from device/permission acceptance.

| Commands | Result |
| --- | --- |
| `cargo fmt --all`; `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml`; both commands with `-- --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --all --all-features` | PASS, including doctests |
| `cargo test -p app-core --example playback_probe --all-features` | PASS, 4 tests |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features` | PASS |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS, 8 tests |
| `sh scripts/test-macos-capture.sh` | PASS |
| `cargo build -p signaling-server`; `cargo build -p app-core --example playback_probe` | PASS |
| `node scripts/test-viewer-playback.mjs` | PASS, 19 decoded frames / 33,600 audio samples |
| `node scripts/test-viewer-playback.mjs --keyframe-recovery` | PASS, 21 frames / 39,840 samples; RTCP recovery verified |
| `node scripts/test-viewer-playback.mjs --chrome` | PASS, 20 frames / 33,600 samples |
| `node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery` | PASS, 20 frames / 36,960 samples; RTCP recovery verified |

Each browser probe verifies media release after Stop. Both target directories resolve inside this repository; no shared target override is present. Verification child processes are stopped. `cargo clean` passes (8,299 files / 1.9 GiB); `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` passes (12,182 files / 3.6 GiB). Both target directories are absent after cleanup; registry/download caches and user preferences are preserved. Markdown relative links/anchors, shell-block syntax, tracked text including hidden configuration (no Cyrillic prose), and `git diff --check` pass.

## Scope

README, architecture, capture, playback and QA guides are reviewed for affected behavior. GUI/user guides describe the fixed selection behavior; capture/user summaries now link the existing actual-window evidence instead of calling all real capture unverified. Component boundaries and media flow remain unchanged. Source-selection checks do not establish actual audio, permission deny/grant transitions, other devices/networks, Linux capture/playback, packaged installation or a passing release gate. BUG-010 still awaits hosted Ubuntu acceptance; no push is authorized by this task. The pre-existing untracked `.python-version` is preserved.
