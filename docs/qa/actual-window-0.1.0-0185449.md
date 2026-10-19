# Actual Window Transmission — 0.1.0 / 0185449

Date: 2026-10-19. Tested code: `0185449` with no application-code changes; later edits only record QA. Version remains untagged `0.1.0`; no milestone feature is completed and the release gate remains open.

## Environment and Build

- macOS 27.0, build 26A428, local desktop session, native WKWebView.
- Two default-feature debug bundles built with `cargo tauri build --debug --bundles app` from `apps/desktop/src-tauri`; bundle ID `dev.videop2p.desktop`.
- Local signaling `127.0.0.1:17000`, room `actual-window-qa`, no configured STUN/TURN; viewer reports `streaming / direct`.
- Real source: the dedicated TextEdit document `VideoP2P Live Capture QA`, native ID `macos-window-1081`. Host raw frame diagnostics report BGRA `1119x720`. Screen Recording preflight is granted; no permission transition was tested.
- GUI actions use native macOS Accessibility against actual app processes, not injected DOM events or mocked IPC. Source text is updated through TextEdit AppleScript. Screenshots are visually inspected.
- Separate configuration directories preserve user preferences. The source produces actual ScreenCaptureKit frames, VideoToolbox H.264, and WebRTC media decoded by the second app.

## Results

| Scenario | Result | Evidence |
| --- | --- | --- |
| Old installed bundle startup/session | FAIL, obsolete artifact | `/Applications/VideoP2P.app`, binary dated October 11, reports `join_room did not finish: undefined`; BUG-005 follow-up records this |
| Current code builds and launches as two debug bundles | PASS | Build command exits 0; two distinct host/viewer processes and isolated preferences |
| Real source selection | PASS with workaround; BUG-011 open | Disable host Auto Refresh before menu selection; verify TextEdit native ID. An earlier refresh-enabled automation attempt selected Deluge instead |
| Actual window transmission | PASS | `connected.png` visibly shows the TextEdit content in the viewer, including `it works!`; player reports 1,375 decoded frames |
| Visible content update | PASS | Append `CHECK B - LIVE UPDATE RECEIVED` in TextEdit; `live-update.png` visibly shows the new text, 2,596 decoded frames, host Auto Refresh unchecked |
| Capture stop/restart | PASS within observed scope | `capture-stopped.txt` reports stopped, 2,817 published H.264 units; Start Native Capture resumes visible playback at 2,847 decoded frames in `capture-restarted.png` |
| Viewer Stop and repeated Refresh | PASS for visible GUI state | `viewer-stopped.txt` remains `viewer / stopped`, playback is stopped, and the Remote Window controls are absent after two refreshes |
| Host Stop | PASS for visible GUI state | `host-stopped.txt` reports `host / stopped`, no active capture, and stopped recovery |
| Audio / sound output | NOT RUN | Include Audio was enabled and counters increased, but the viewer stayed muted; no audible-output claim |
| Permission deny/grant/relaunch, application/display sources, multiple monitors | NOT RUN | Outside this focused window-video run |
| Separate devices, cross-network, Linux, signing/install/release acceptance | NOT RUN | Local debug bundles do not establish these scenarios |

Evidence is local under `/private/tmp/video-p2p-live-qa/`: native Accessibility snapshots, launch script, separate preferences, logs, and screenshots. The tested `.app` is copied there before Cargo cleanup. Screenshots include unrelated desktop content and are intentionally not committed. The installed old `/Applications` copy is unchanged.

The initial GUI exploration used AppleScript, whose references to duplicate process names were ambiguous. A native helper addressed each app by PID for the successful run. Source selection uncertainty is retained as BUG-011; the workaround is not a fix.

## Automated Checks and Cleanup

All 17 mandatory commands below exited 0. Full outputs and durations are retained in `checks-summary.json` and separate logs in the evidence directory. README, TODO, viewer guidance and the bug register are reviewed and updated; architecture and the manual checklist still describe the remaining acceptance scope accurately.

| Command | Result |
| --- | --- |
| `cargo fmt --all` | PASS |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --all --all-features` | PASS |
| `cargo test -p app-core --example playback_probe --all-features` | PASS |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features` | PASS |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS |
| `sh scripts/test-macos-capture.sh` | PASS |
| `cargo build -p signaling-server` | PASS |
| `cargo build -p app-core --example playback_probe` | PASS |
| `node scripts/test-viewer-playback.mjs` | PASS |
| `node scripts/test-viewer-playback.mjs --keyframe-recovery` | PASS |
| `node scripts/test-viewer-playback.mjs --chrome` | PASS |
| `node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery` | PASS |

All tracked text files, including hidden configuration, were scanned for Cyrillic prose: none found. Relative documentation links/anchors and `git diff --check` pass. No application code, dependency, version, or tag changed. The pre-existing untracked `.python-version` is preserved.

Test GUI sessions were stopped with their Stop controls before the launcher and its owned children were terminated. The tested application and signaling binary are saved outside Cargo targets in the evidence directory. The local `launch.mjs` can reopen the two apps with isolated preferences; its syntax check passes. The source window ID may change after TextEdit exits, so verify the selected source before a subsequent run.

Tested application executable SHA-256: `c510f76ee58cdba6dadd754ee74197028a7b4ee328ba91877c3cfdca7351e0c9`. Old installed executable SHA-256: `2b0b8e21b41888f81e416f57a65d034fb916646f88e34a2b91d68c0d98f96435`.

Cleanup: `cargo clean` passed (13,947 files, 3.3 GiB); `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` passed (18,716 files, 5.8 GiB). No test GUI, signaling, or playback-probe processes remain. Registry/download caches, sources, lockfiles, and user preferences are preserved.
