# BUG-020: Bounded Browser Probe Cleanup

2026-10-29, base `f5e7c612c4c666fd612d4d61c264abdd51a3eaa2`, untagged `0.1.0`, macOS 27.2 (26B123), Node 26.13.0, Chrome 156.0.8078.74 and system WKWebView. Ownership: `BASE-VIEWER-PROBES`. Changes affect verification tooling only; no capture, playback or permission behavior changes.

## Controlled Failure and Repair

The historical failure is recorded in [BUG-020](../BUGS.md#bug-020-chrome-probe-profile-cleanup-fails-with-enotempty): decoded Chrome playback passes, then recursive profile removal throws `ENOTEMPTY`, leaving `Network Persistent State`. The historical `/private/tmp/video-p2p-bug017-checks/16.log` is absent in this environment; its recorded evidence is preserved in the register. The original writer and timing cannot be retrospectively established.

`node --test scripts/probe-cleanup.test.mjs` controls the enumeration/removal boundary: enumerate an empty `chrome/Default`, write `Network Persistent State`, then call real filesystem `rmdir`. The original one-attempt behavior raises `ENOTEMPTY`; the same injected race succeeds through the new removal helper and leaves no scratch directory. This is a deterministic filesystem race regression, not a claim to have reproduced Chrome's internal writer.

The runner requests CDP `Browser.close` on success and on failure after CDP setup. It gives Chrome three seconds to flush/exit, then signals remaining owned process groups with TERM and, after three seconds, KILL. Another three-second bound reports failure if a group survives. Detached children have private POSIX groups; no user browser instance is targeted. Group existence is checked even after the direct child exits. Failed spawn owns no process. Scratch removal starts only after every owned group stops, retries only transient errors (five retries, 100–500 ms delays), and preserves the path/cause on failure.

Six portable tests cover the controlled late write, persistent ENOTEMPTY with retained evidence, immediate non-transient failure/missing directory, a helper flushing its last write on TERM, an unresponsive child requiring KILL, and graceful exit/failed spawn. The helper test verifies both its PID and the process group are absent before removing the profile. CI runs these alongside viewer lifecycle tests on Linux/macOS; hosted results are not claimed here.

## Verification

The initial sandboxed gate passes format/Clippy but `cargo test --all --all-features` fails: two signaling tests cannot bind local listeners (`Operation not permitted`) and the media-worker test times out with zero ICE candidates. Logs are retained in `/private/tmp/video-p2p-bug020/checks/`. The approved run outside the sandbox passes all 17 mandatory commands, plus the new cleanup tests, on unchanged code. Exact commands, durations and exits are saved in `checks-unrestricted/results.json` under the same evidence directory.

| Check | Result / log |
| --- | --- |
| Both workspace formatting and strict Clippy | PASS; logs 1–6 |
| Root Rust tests and doctests | PASS, 127 tests; log 7 |
| Playback-probe example tests | PASS, 4 tests; log 8 |
| Separate Tauri workspace tests | PASS (0 tests); log 9 |
| Node viewer lifecycle | PASS, 8 tests; log 10 |
| Permission-free native extraction | PASS; log 11 |
| Signaling/probe builds | PASS; logs 12–13 |
| WKWebView ordinary / keyframe recovery | PASS, 20/23 decoded frames, 33,600/43,680 audio samples; logs 14–15 |
| Chrome ordinary / keyframe recovery | PASS, 20/19 decoded frames, 34,080/35,520 audio samples; logs 16–17 |
| `node --test scripts/probe-cleanup.test.mjs` | PASS, 6/6; log 18 |

All four browser runs verify decoded pixels/nonzero PCM, media Stop cleanup, process-group exit and successful scratch removal. Recovery modes receive browser RTCP feedback and publish a replacement keyframe. No screen capture permission or physical sound-output acceptance is inferred.

Additional checks are retained in `/private/tmp/video-p2p-bug020/extra/` with exact command/environment/exit records in `results.json`:

| Command | Result |
| --- | --- |
| `node scripts/test-viewer-playback.mjs --signaling-disconnect` | PASS; 49 decoded frames / 85,440 audio samples, same peer after EOF, cleanup passes |
| `node scripts/test-viewer-playback.mjs --chrome --signaling-disconnect` | PASS; 54 decoded frames / 92,160 audio samples, same peer after EOF, cleanup passes |
| `VIEWER_TEST_CHROME=/missing-bug020-chrome node scripts/test-viewer-playback.mjs --chrome` | Expected exit 1 after the existing endpoint timeout, with ENOENT diagnostics; owned groups stop and scratch removal passes |
| `node --test apps/desktop/tests/viewer-player.test.mjs scripts/probe-cleanup.test.mjs` | PASS, 14/14; same invocation as CI |

The three extra browser scenarios each leave no new scratch directory. Post-run process inspection finds no remaining browser profile/probe/server/build processes. Both Cargo target directories are repository-local and `CARGO_TARGET_DIR` is unset. After all checks, `cargo clean` removes 8,327 files / 1.9 GiB and `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` removes 9,593 files / 2.6 GiB, both exit 0. Both target directories are absent; cleanup logs remain in the evidence directory. No release bundle was produced.

All three changed/new JavaScript files pass `node --check`. Tracked-text Cyrillic scanning, relative Markdown links/anchors, shell-block syntax and `git diff --check` pass. README, TODO, viewer guide, QA checklist and bug register are updated; architecture remains accurate. Final documentation edits record verification only and change no tested code/configuration.

No release tag, version bump or new feature ID is claimed. Native replacement acceptance (BUG-016), dependent source-lifecycle acceptance (BUG-015), hosted Ubuntu acceptance (BUG-010), physical device/network and other release checks remain open.

Fixing commit: `ff8bf66`, untagged `0.1.0`. BUG-020 is CLOSED. This final identity/status update changes documentation only; the tested code/configuration and verification results are unchanged.
