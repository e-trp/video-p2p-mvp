# BUG-018: Keep the Preference Source Alive Across Restart

2026-10-27, base `819a636`, untagged `0.1.0`, macOS 27.0 (26A428), arm64, native WKWebView, Node 26.12.1. This fixes the `BASE-DESKTOP-CONTROLS` test fixture. Production preference, catalog and capture behavior is unchanged; the feature counter remains `0/5`.

## Implementation and Regression

The old controls scenario selected the second source option. With native discovery this could be the same desktop window about to exit. Correct idle-session removal was then misreported as persistence loss.

The runner now owns a separate silent Cocoa window for save and restore. It passes the native ID to the saving client, retains that process until restoration completes, and requires the exact saved ID with audio disabled. It then closes the fixture and starts a third client using the same isolated settings. That client must preserve room/ICE/UI preferences while dropping the absent source without silently selecting another one. Native removal needs permission; example fallback restoration cannot certify it. Fixture startup, child lifetimes and temporary binary cleanup are bounded. No source media is captured by this preference test.

`--scenario preferences` runs the focused three-process sequence. Default and `--scenario controls` runs use the same fixture across the full controls and restore/removal phases. The standalone `--scenario restore` choice is replaced by the complete lifecycle to avoid stale fixtures. The QA-only Rust configuration adds the expected source ID; default production builds contain none of this driver.

Controlled commands:

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
node scripts/test-tauri-gui.mjs --scenario preferences --output /private/tmp/video-p2p-bug018-preferences
node scripts/test-tauri-gui.mjs --scenario preferences --output /private/tmp/video-p2p-bug018-old-selector
node scripts/test-tauri-gui.mjs --output /private/tmp/video-p2p-bug018-full-gui
```

The first focused run passes all 10 checks. Fixture `macos-window-8835` survives the saving process, is restored exactly, and is removed only after the runner closes it. The negative comparison temporarily reinstates the old `options[Math.min(1, options.length - 1)]` selector in the same test harness. It saves the client's own window `macos-window-8892` while the controlled fixture `macos-window-8889` remains alive. The next process drops `8892`; exact-source restoration fails (exit 1). Room/ICE/UI preferences still pass. That negative version is not retained.

The negative run's removal check was BLOCKED because its saved source was not the controlled fixture. Its generic permission reason is misleading on this granted system; the final test distinguishes an incorrect native source (FAIL) from example fallback (BLOCKED). Reports are preserved unchanged. The final implementation restores the controlled selector and requires removal evidence from the real fixture.

## Verification Record

The full GUI run records **43 PASS / 0 FAIL / 1 BLOCKED**. All 21 applicable controls checks, exact restoration (3 checks), missing-source cleanup (4), source-picker/contention (8) and viewer (7) checks pass. The denied-permission check remains BLOCKED because this machine already grants Screen Recording; the full runner correctly exits 1. Fixture `macos-window-8927` is saved/restored exactly with audio off, a 15-second disabled Auto Refresh interval, room and ICE settings preserved. `saved-session.conf` retains the settings before removal. This is not a passing release gate.

The viewer passes on this run, but BUG-019 remains OPEN: this retry neither isolates its hidden-window predicate failure nor repairs its cause. BUG-020 also remains OPEN despite both Chrome modes cleaning up successfully in this task. BUG-017 retains its remaining investigation-dependent acceptance.

The initial complete mandatory gate passes all 17 commands, with 127 root Rust tests plus doctests, 4 playback-probe tests, the separate desktop test target, 8 Node lifecycle tests, native extraction, and all four browser modes. Logs are in `/private/tmp/video-p2p-bug018-checks/1.log` through `17.log`. WKWebView ordinary/recovery reports 21/19 decoded frames and 35,520/37,440 audio samples; Chrome ordinary/recovery reports 10/4 frames and 15,840/9,600 samples. Each mode verifies media Stop cleanup; both recovery modes verify browser RTCP feedback.

Final review additionally makes cached native catalogs use the controlled source ID instead of falling into example-source selection. Final GUI/gate revalidation and cleanup are recorded below; the initial evidence remains intact. Native source replacement remains unresolved under [BUG-016](BUG-016-restart-investigation.md). This task does not close BUG-019 or certify manual permission transitions, audible devices, packaged applications, other OSes or networks.

## Final Revalidation — 2026-10-28

The final code is unchanged from the resumed working tree based on `819a636`; subsequent edits record documentation and commit identities only. Exact mandatory commands, exit codes and durations are saved in `/private/tmp/video-p2p-bug018-resumed-checks/results.json`, with individual output in `1.log` through `20.log`.

The GUI runner directly launches `apps/desktop/src-tauri/target/debug/desktop-app` with isolated configuration directories. It does not reinstall, replace or launch the user's installed application, or modify its permissions. Successful native catalog discovery in this launch context does not establish permission or capture behavior for an installed `.app`. The preference scenario does not start capture, and browser probes use synthetic media. Installed/bundled application acceptance remains separate, including the LaunchServices comparison required by BUG-016.

| Check | Result / evidence |
| --- | --- |
| Both workspace format and strict Clippy commands | PASS, logs 1–6 |
| Root Rust tests and doctests | PASS, 127 tests; log 7 |
| Playback-probe example tests | PASS, 4 tests; log 8 |
| Separate Tauri workspace tests | PASS; log 9 |
| Node viewer lifecycle tests | PASS, 8 tests; log 10 |
| Permission-free native extraction | PASS; log 11 |
| Signaling and playback-probe builds | PASS; logs 12–13 |
| WKWebView ordinary / recovery | PASS, 23/22 frames and 39,360/40,800 audio samples; logs 14–15 |
| Chrome ordinary / recovery | PASS, 26/7 frames and 43,200/14,400 audio samples; logs 16–17 |
| GUI QA build | PASS; log 18 |
| Focused preference lifecycle | PASS, 10/10, source `macos-window-10796`; `/private/tmp/video-p2p-bug018-resumed-preferences/`, log 19 |
| Full GUI regression | 43 PASS / 0 FAIL / 1 BLOCKED, source `macos-window-10834`; `/private/tmp/video-p2p-bug018-resumed-full-gui/`, log 20 |

All four browser probes verify Stop cleanup and both recovery modes verify browser-originated RTCP feedback. Both GUI runs restore the exact controlled source with audio disabled, then observe removal only after the runner terminates its fixture. The full GUI command exits 1 because the permission-denied check needs a fallback catalog while this environment has Screen Recording access. The seven viewer checks pass, but the earlier BUG-019 failure remains unresolved because no controlled repair was made.

`node --check` passes for both changed JavaScript files. Tracked text (including hidden configuration) contains no Cyrillic prose; local Markdown links/anchors and shell-block syntax pass. README, TODO, architecture, GUI guide, QA checklist and relevant bug entries were reviewed; architecture and checklist behavior remains accurate. `git diff --check` passes.

Process inspection after the runners exited finds no remaining test/build processes. Both target paths are repository-local and `CARGO_TARGET_DIR` is unset. `cargo clean` exits 0, removing 9,167 files / 2.0 GiB; the separate desktop clean exits 0, removing 12,745 files / 3.7 GiB. Logs and failure evidence are retained outside the targets. No release bundle was produced or needed by this task. No version bump, release tag or full-project QA acceptance is claimed.
