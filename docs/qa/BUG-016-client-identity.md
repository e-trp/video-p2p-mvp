# BUG-016: Per-Client Executable Identity

2026-11-05, base `fbd933c`, untagged `0.1.0`, macOS 27.2 (26B123), arm64. This investigation and runner change leave BUG-016 unresolved. No production capture, timeout, ownership or permission changes are retained.

## Controlled Comparisons

The retained QA executable inside `~/Applications/VideoP2P Source Lifecycle QA.app` and its standalone copy have the same SHA-256, `39b91c1e5020d1cfd3618669cc208d67678057ff24276085063518c826991ded`. The bundle identifier is `dev.videop2p.qa.source-lifecycle`. Each comparison uses the existing real red/tone and blue/silent fixtures, local signaling, isolated preferences, and actual native capture/desktop decoding. All clients launch directly; these runs do not test BUG-023's LaunchServices negotiation.

Evidence directories start with `/private/tmp/video-p2p-bug016-nov05-`:

| Suffix | Host / viewer | Result |
| --- | --- | --- |
| `host-standalone` | Standalone / retained bundle | PASS, 17/17 |
| `both-standalone` | Same standalone executable path | FAIL at alternate-source startup after initial pixels/PCM pass; permission granted |
| `viewer-standalone` | Retained bundle / standalone | PASS, 17/17 |
| `separate-paths` | Byte-identical standalone executables at distinct paths | PASS, 17/17 |
| `embedded-id` | Both clients from one new debug binary with an experimental embedded application identifier | FAIL at alternate-source startup; experiment reverted |

Commands executed for these controls:

```bash
python3 /private/tmp/video-p2p-bug016-nov05.py host /private/tmp/video-p2p-bug016-nov05-host-standalone
python3 /private/tmp/video-p2p-bug016-nov05.py both /private/tmp/video-p2p-bug016-nov05-both-standalone
python3 /private/tmp/video-p2p-bug016-nov05.py viewer /private/tmp/video-p2p-bug016-nov05-viewer-standalone
python3 /private/tmp/video-p2p-bug016-nov05.py separate-path /private/tmp/video-p2p-bug016-nov05-separate-paths
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
python3 /private/tmp/video-p2p-bug016-nov05.py embedded /private/tmp/video-p2p-bug016-nov05-embedded-id
```

The temporary wrapper substitutes each role's executable and retained signaling/catalog probes. Its legacy `summary.binary` field names the standalone reference even in mixed runs; the role mapping above and the saved wrapper identify the actual commands. This evidence gap motivates the maintained runner change. The embedded-identifier experiment uses the current GUI implementation, unlike the unchanged retained executable comparisons; it is not a same-binary control.

`/private/tmp/video-p2p-bug016-nov05-process.log` contains the scoped `log show --last 8m --info --debug --style compact` output for `desktop-app` and `replayd`. At 13:26:38, stopping the first stream is followed by `uniqueClientIdentifierWithPID` selecting an executable-path identifier, cancellation of the host connection, then cancellation of the viewer connection as the host reconnects. Both clients repeatedly reconnect. No system service was terminated or restarted by the investigation. A narrower subsystem query yielded no events and supplies no evidence.

The same-path failure combined with both mixed-role and distinct-path controls supports a native client identity collision. Logs alone do not establish the internal OS implementation. Merely adding `CFBundleIdentifier` through Tauri's debug plist does not repair it. No experimental plist is retained. Distinct paths remain a diagnostic control, not a production fix or a reason to close BUG-016.

## Maintained Runner

`scripts/test-native-desktop-playback.py` now supports `--viewer-binary`, retaining `--binary` as the default for both roles. It validates both executables before starting fixtures, records each role's resolved path/hash/enclosing bundle in `summary.json`, and saves actual PIDs/paths in each case's `processes.json`, including partial startup failures. It neither copies nor signs clients, changes permissions, nor implicitly substitutes a passing launch context.

Seven portable tests exercise default/explicit role dispatch, invalid executable paths, same-byte/different-context identity evidence, and cleanup with retained host evidence after startup failure. Existing actual source-lifecycle assertions are unchanged.

## Verification

The portable runner suite passes:

```bash
python3 -B -m unittest discover -s scripts -p test_native_desktop_runner.py
```

All 20 commands in `/private/tmp/video-p2p-bug016-nov05-checks/results.json` exit 0; numbered logs retain the output. Toolchain: Rust 1.99.1, Node 26.13.0, Python 3.15.2.

| Check | Result |
| --- | --- |
| Formatting and format checks, both workspaces | PASS |
| Strict all-target/all-feature Clippy, both workspaces | PASS |
| All-feature Rust tests/doctests, both workspaces; playback-probe example tests | PASS |
| Viewer JavaScript lifecycle tests and probe-cleanup tests | PASS |
| Seven portable runner regressions | PASS |
| Permission-free native extraction/lifecycle tests | PASS |
| Signaling and playback-probe builds | PASS |
| WKWebView playback and keyframe recovery | PASS |
| Chrome playback and keyframe recovery | PASS |
| Final `gui-qa` desktop build | PASS |

The portable regression suite rejects the baseline runner with its missing per-role/evidence API; `runner-before.log` preserves that controlled result. Final-binary Mach-O plist inspection confirms the experimental `CFBundleIdentifier` is absent again. Documentation relative links/anchors and shell-block syntax pass; the tracked-text Cyrillic scan has zero matches. README, architecture and capture guides were reviewed and remain accurate within their stated scope.

Required device/network/distribution QA and the full-project release gate remain open; no feature count, version or tag is advanced. The following record-only additions do not change the tested implementation.


## Maintained-Runner Acceptance and Cleanup

The final production code is unchanged from `fbd933c`; only the QA runner/tests and documentation change. Both clients in this comparison have SHA-256 `69ae41547da81e8b36203291b70c7a781549034517cf6efd13e25da4e9e4579c`. The viewer copy is retained outside Cargo targets. The existing catalog probe is selected explicitly; the signaling server is rebuilt from the current source.

```bash
python3 scripts/test-native-desktop-playback.py --source-lifecycle --probe /private/tmp/video-p2p-bugs-remaining/capture_probe --output /private/tmp/video-p2p-bug016-nov05-maintained-shared
python3 scripts/test-native-desktop-playback.py --source-lifecycle --viewer-binary /private/tmp/video-p2p-bug016-nov05-current-viewer/desktop-app --probe /private/tmp/video-p2p-bugs-remaining/capture_probe --output /private/tmp/video-p2p-bug016-nov05-maintained-separated
```

The first command exits 1 and preserves the original BUG-016 alternate-source startup failure after initial decoded pixels/PCM pass. Its summary records equal client paths/hashes and its process record identifies both PIDs. This is expected negative evidence for the unresolved bug, not a passing capture test.

The second command exits 0 with **17/17 checks passing**. Its summary records distinct paths with equal hashes, no enclosing bundles, and direct launches. It verifies 1144×720 red pixels `[176, 73, 45, 255]`, blue replacement pixels `[39, 62, 184, 255]` with delivered silent PCM (RMS 0), restored red/tone (RMS 0.03508), source disappearance, and Stop cleanup on the same viewer peer/generation. Both clients exit 0. Both fixtures exit 0 during cleanup in each run. These paired outcomes validate the new runner option and evidence while leaving the shared-path product/environment failure open.

After every build/probe completed, process inspection found no remaining project QA/build processes. `CARGO_TARGET_DIR` is unset and both metadata-reported targets are repository-local. `cargo clean` exits 0 and removes 8,327 files / 1.9 GiB; `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` exits 0 and removes 10,626 files / 3.1 GiB. Logs and retained comparison binaries remain outside target directories; installed applications and user preferences are untouched.
