# BUG-004: Graceful Signaling EOF Verification

This is a focused bug-fix record, not a full-project release checkpoint. The project stays on the untagged `0.1.0` baseline with `0/5` newly completed features. Ownership: [BASE-SIGNALING](../../todo.md#webrtc-and-signaling); defect: [BUG-004](../BUGS.md#bug-004-graceful-signaling-disconnect-is-reported-as-connected).

## Tested Tree and Environment

- Date: 2026-10-11.
- Tested implementation/configuration: `34585a32c335e155412f847e78d353cd32f5f876`, based on `1810c03`. Later closure/evidence edits are documentation only; tested code/configuration is unchanged.
- macOS `27.0` (`26A428`), WebKit `22625.1.29.11.27`, Chrome `155.0.8059.40`.
- Node `26.11.1`, Rust/Cargo `1.99.0`.
- Deterministic Unix socket shutdown and loopback TCP; isolated session preferences. No Screen Recording permission is requested by these regressions.

## Before and After

Before the fix, `cargo test -p app-core signaling::tests` ran seven tests: five EOF cases failed and the server-rejection/fragmented-open-message cases passed. `cargo test -p app-core graceful_signaling_close` failed both new session checks because `signaling_connected` remained true after EOF.

The client now records observed EOF, delivers complete final events once, and reports closure on the following poll. With no complete events, EOF is reported immediately. An incomplete final line produces a contextual error; a final `ERROR` keeps the server's rejection reason. Sends fail after observed EOF. The session drops failed signaling once, enabling existing reconnect guidance without replacing transport/player ownership.

After the fix, all nine focused tests pass:

- Idle reads and fragmented lines remain valid while the connection is open.
- EOF is terminal for reads and sends; a loopback TCP test verifies the production socket path with a two-second propagation deadline.
- Complete final `WAITING`/`PEER`/ICE messages are delivered in order once, including CRLF lines.
- A final server rejection keeps its reason, and truncated final input is diagnosed after any complete preceding line.
- Both host Refresh and viewer signaling polling expose unavailable signaling, release it once, and avoid repeating the same poll error; Stop remains stopped after refresh.
- A viewer's reported live state, counters, and generation survive signaling loss; sending fails. Manual reconnect to a fresh TCP listener creates a new connection/generation, resets counters, and rejects the old generation.

The viewer-state unit test uses synthetic playback reports and verifies backend ownership/recovery only. An additional `--signaling-disconnect` browser probe forwards viewer signaling through a local TCP proxy, then gracefully closes that channel after video/audio playback starts. The synthetic host keeps its own signaling connection. The probe checks detected EOF/reconnect guidance, the same peer/generation, more than three additional decoded frames and 48,000 received audio samples, fresh nonzero decoded PCM after at least 1.5 seconds, and Stop/cleanup.

## Automated Results

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo test -p app-core signaling::tests` before fix | Expected FAIL | Five failed EOF cases; two normal parsing checks passed |
| `cargo test -p app-core graceful_signaling_close` before fix | Expected FAIL | Both session checks failed at the connected flag |
| `cargo test -p app-core signaling::tests` after fix | PASS | Seven tests passed |
| `cargo test -p app-core graceful_signaling_close` after fix | PASS | Two tests passed, including both roles and fresh TCP reconnect |
| `node scripts/test-viewer-playback.mjs --signaling-disconnect` | PASS | WKWebView: `64x36`, 52 decoded frames, 90,720 audio samples; viewer EOF detected with continued decoding on the same peer; media released |
| `node scripts/test-viewer-playback.mjs --chrome --signaling-disconnect` | PASS | Chrome: `64x36`, 53 decoded frames, 92,640 audio samples; viewer EOF detected with continued decoding on the same peer; media released |
| `cargo fmt --all` | PASS | Root workspace formatted |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS | Separate workspace formatted |
| `cargo fmt --all -- --check` | PASS | No formatting diff |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | PASS | No formatting diff |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS | No warnings |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS | No warnings |
| `cargo test --all --all-features` | PASS | 122 unit/integration tests; all doctest targets passed |
| `cargo test -p app-core --example playback_probe --all-features` | PASS | Four lifetime tests |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features` | PASS | Separate workspace compiled; zero own tests |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS | Eight lifecycle tests |
| `sh scripts/test-macos-capture.sh` | PASS | Permission-free extraction/timing/backpressure/stop checks |
| `cargo build -p signaling-server` | PASS | Server used by all six browser modes |
| `cargo build -p app-core --example playback_probe` | PASS | Synthetic host/viewer bridge built |
| `node --check scripts/test-viewer-playback.mjs` | PASS | Valid JavaScript syntax |
| `node scripts/test-viewer-playback.mjs` | PASS | WKWebView: `64x36`, 26 decoded frames, 44,160 audio samples; media released |
| `node scripts/test-viewer-playback.mjs --keyframe-recovery` | PASS | WKWebView: `64x36`, 26 decoded frames, 47,040 audio samples; RTCP recovery and media release |
| `node scripts/test-viewer-playback.mjs --chrome` | PASS | Chrome: `64x36`, 25 decoded frames, 42,720 audio samples; media released |
| `node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery` | PASS | Chrome: `64x36`, 22 decoded frames, 40,800 audio samples; RTCP recovery and media release |
| Documentation checks | PASS | English-only tracked text/new record, 126 relative links/anchors, 38 shell blocks |
| `git diff --check` and final diff review | PASS | No whitespace errors; unrelated user file excluded |
| `cargo clean` | PASS | Removed 9,350 files / 2.0 GiB |
| `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS | Removed 9,603 files / 2.6 GiB |

All mandatory automated checks passed. Workspace checks were rerun after the final probe/bridge changes. Browser runners stopped their child processes, closed proxy sockets/listeners, and removed temporary probe directories. Both workspaces were cleaned after all checks, and both target directories are absent. Two empty preference-test directories left by the intentional failing reproduction were also removed.

## Scope and Remaining QA

BUG-004 is `CLOSED`: the fixing commit and all closure checks are recorded, including decoded synthetic video/audio after viewer signaling EOF in both browsers. No new feature ID was completed, and no release tag/version bump was created.

Actual screen/audio capture, audible devices, packaged operation, whole-server interruption during actual capture/playback, Linux execution, and cross-network scenarios have **NOT RUN** in this focused verification. These remain open in the [QA checklist](../QA_CHECKLIST.md). Browser EOF probes use synthetic media and close only the viewer's signaling channel; the ordinary/recovery modes verify normal decoding and cleanup.

README, desktop UI, installation, and QA checklist were reviewed. Their commands, UI labels, and outstanding manual QA statements remain accurate. Networking, architecture, viewer playback, user guide, TODO, and the bug register document EOF/recovery behavior and the new probe commands. TLS/authentication/TTL/resource limits and media milestones remain open; this fix completes no new feature ID or release gate.
