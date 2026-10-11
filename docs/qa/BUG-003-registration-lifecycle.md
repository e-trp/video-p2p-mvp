# BUG-003: Signaling Registration Lifecycle Verification

This focused fix does not complete a new feature or a release checkpoint. The project stays at the untagged `0.1.0` baseline with `0/5` newly completed features. Owning feature: [BASE-SIGNALING](../../todo.md#webrtc-and-signaling); defect: [BUG-003](../BUGS.md#bug-003-rejected-signaling-clients-can-relay-and-remove-the-registered-peer).

## Tested Tree and Environment

- Date: 2026-10-11.
- Tested implementation/configuration: `12b96ae0380cec1dc6054cd3019200f4ffb191cd`, based on `6841888`. Later closure/evidence edits are documentation only; tested code/configuration is unchanged.
- macOS `27.0` (`26A428`), WebKit `22625.1.29.11.27`, Chrome `155.0.8059.40`.
- Node `26.11.1`, Rust/Cargo `1.99.0`.
- Registration tests use loopback TCP and deterministic write failures; browser probes use synthetic `64x36` video and stereo 48 kHz audio with native codecs and local signaling/WebRTC.

## Before and After

Before the implementation change, `cargo test -p signaling-server` ran five tests: the two existing normal relay/late-replay tests passed, and all three new regressions failed. The rejected client's ICE reached the opposite participant; failed `WAITING` left the room allocated; failed pairing left the new role occupied.

An occupied-role rejection now propagates an error to terminate the handler before entering the relay loop or unregistering a role. If initial announcement/replay writes fail after insertion, registration releases only the joining role before returning the write error.

After the fix, all six focused tests pass:

- Duplicate-role rejection cannot send signaling, change history, or unregister the accepted client; the exact wire error is preserved.
- Failed `WAITING` removes the empty room and permits role reuse.
- Failures writing the joining peer announcement, opposite peer announcement, or replay release the joining slot while preserving the opposite participant/history; retries replay the retained history when the opposite writer is healthy.
- An accepted TCP client relays successfully, disconnects normally, and clears only its own slot/history. The last participant's departure removes the room.
- Existing paired relay and late-join replay tests remain passing.

Every new scenario runs for both sender and receiver. TCP reads have three-second deadlines. Write failures are deterministic and require no unreliable socket-reset timing.

## Automated Results

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo test -p signaling-server` before fix | Expected FAIL | Three regressions failed; two existing tests passed |
| `cargo test -p signaling-server` after fix | PASS | Six tests passed; both roles and each failure phase covered |
| `cargo fmt --all` | PASS | Root workspace formatted |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS | Separate workspace formatted |
| `cargo fmt --all -- --check` | PASS | No formatting diff |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | PASS | No formatting diff |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS | No warnings |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS | No warnings |
| `cargo test --all --all-features` | PASS | 113 unit/integration tests; all doctest targets passed |
| `cargo test -p app-core --example playback_probe --all-features` | PASS | Four probe lifetime tests |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features` | PASS | Separate workspace compiled; zero own tests |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS | Eight lifecycle tests |
| `sh scripts/test-macos-capture.sh` | PASS | Permission-free extraction/timing/backpressure/stop checks |
| `cargo build -p signaling-server` | PASS | Updated server used by all browser probes |
| `cargo build -p app-core --example playback_probe` | PASS | Synthetic host/viewer bridge built |
| `node scripts/test-viewer-playback.mjs` | PASS | WKWebView: `64x36`, 23 decoded frames, 40,800 audio samples; media released |
| `node scripts/test-viewer-playback.mjs --keyframe-recovery` | PASS | WKWebView: `64x36`, 24 decoded frames, 43,680 audio samples; RTCP recovery and media release |
| `node scripts/test-viewer-playback.mjs --chrome` | PASS | Chrome: `64x36`, 26 decoded frames, 44,640 audio samples; media released |
| `node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery` | PASS | Chrome: `64x36`, 20 decoded frames, 36,000 audio samples; RTCP recovery and media release |
| Documentation checks | PASS | Tracked text/new record contain no Cyrillic; 116 relative links/anchors and 37 shell blocks validated |
| `git diff --check` and final diff review | PASS | No whitespace errors or unrelated changes selected |
| `cargo clean` | PASS | Removed 8,454 files / 1.9 GiB |
| `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS | Removed 9,581 files / 2.6 GiB |

All mandatory automated checks passed. Browser runners stopped their child processes and removed temporary probe directories. Both workspaces were cleaned after all checks; a filesystem check confirmed that both target directories are absent.

## Scope and Remaining QA

BUG-003 is `CLOSED`: the fixing commit and all stated closure checks are recorded. No new feature ID is completed.

Actual capture/permission prompts, audible output devices, packaged-app operation, Linux execution, and cross-network behavior were **NOT RUN** in this focused run and remain open in the [QA checklist](../QA_CHECKLIST.md). Synthetic playback does not verify those scenarios. No release tag or version bump was created for this development fix.

README, architecture, installation, user guide, and QA checklist were reviewed: their existing normal signaling commands, role limits, and outstanding production/manual QA statements remain accurate. The networking guide and TODO describe the corrected registration behavior. TLS, authorization, TTL, resource bounds, and rate limits remain M5 work.
