# BUG-001: Playback Probe Startup Lifetime Verification

This is a focused bug-fix record, not a full-project release QA checkpoint. The project remains `0.1.0`, untagged, with `0/5` newly completed feature IDs. Owning feature: [BASE-VIEWER-PROBES](../../todo.md#desktop-viewer-and-recovery). Bug and fixing-commit tracking: [BUG-001](../BUGS.md#bug-001-intermittent-wkwebview-playback-probe-failure).

## Tested Tree and Environment

- Date: 2026-10-10.
- Tested code/configuration: the implementation working tree based on `615cb2a`; the fixing commit will be recorded after committing this verified tree. Later closure/evidence edits are documentation only.
- macOS `27.0` (`26A428`), system WebKit `22625`, Chrome `155.0.8059.40`.
- Node `26.11.0`, Rust/Cargo `1.99.0`.
- Synthetic `64x36` BGRA frames and stereo 48 kHz PCM; native VideoToolbox/Opus encoding, loopback TCP signaling and WebRTC, real viewer bridge/player module. No external STUN/TURN, screen-recording permissions, physical source, or packaged Tauri app. Audio analysis uses zero-gain output.

## Before and After

The former host lifetime started before browser startup/compilation and ended after 40 seconds regardless of viewer connection. The unchanged `615cb2a` runner/host, instrumented in a temporary diagnostic copy with a 42-second browser-start delay, failed after 59,537 ms:

```text
hostExit: 0
viewer connection/state: new / waiting
video dimensions: 0x0
decoded video frames / audio samples: 0 / 0
width assertion: actual 0, expected 64
```

The historical failure had no host-exit diagnostics, so this controlled reproduction establishes the fixture defect and matching symptom without proving every historical failure had the same cause.

The fixed host waits at most 90 seconds for its first connected state, then permits 40 seconds of streaming. A disconnect/reconnect cannot reset that window. Missing connection produces an explicit error. Four portable tests cover delayed startup, startup timeout, late connection, and reconnect; CI now explicitly runs them.

The equivalent fixed regression was executed while root and separate Tauri workspace compilation was active:

```bash
VIEWER_TEST_BROWSER_STARTUP_DELAY_MS=42000 node scripts/test-viewer-playback.mjs
```

Result: **PASS** — `64x36`, eight decoded frames, 12,000 audio samples, and media cleanup. The test preserves the existing browser/media deadlines and uses no retry. Failure output now includes mode, startup delay, host exit state, playback diagnostics, and child stderr.

## Automated Results

All commands ran locally. Rust checks completed before the four ordinary browser modes below; the delayed-start regression ran under concurrent compilation as described above.

| Command | Result | Evidence |
| --- | --- | --- |
| `cargo fmt --all` | PASS | Formatted root workspace |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS | Formatted separate workspace |
| `cargo fmt --all -- --check` | PASS | No formatting diff |
| `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` | PASS | No formatting diff |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS | No warnings |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS | No warnings |
| `cargo test --all --all-features` | PASS | 109 unit/integration tests; doctests passed |
| `cargo test -p app-core --example playback_probe --all-features` | PASS | Four lifetime regression tests |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features` | PASS | Separate workspace compiled; zero own tests |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS | Eight lifecycle tests |
| `sh scripts/test-macos-capture.sh` | PASS | Permission-free native extraction/lifecycle tests |
| `cargo build -p signaling-server` | PASS | Local signaling binary used by the probes |
| `cargo build -p app-core --example playback_probe` | PASS | Fixed synthetic host/viewer bridge |
| `node --check scripts/test-viewer-playback.mjs` | PASS | Valid JavaScript syntax |
| Documentation consistency checks | PASS | English-only tracked text, Markdown links/anchors, shell examples, stable TODO IDs, and completed-entry placement |
| `node scripts/test-viewer-playback.mjs` | PASS | WKWebView: `64x36`, five decoded frames, 7,680 audio samples; media released |
| `node scripts/test-viewer-playback.mjs --keyframe-recovery` | PASS | WKWebView: `64x36`, 23 decoded frames, 43,680 audio samples; RTCP recovery and media release |
| `node scripts/test-viewer-playback.mjs --chrome` | PASS | Chrome: `64x36`, 24 decoded frames, 40,320 audio samples; media released |
| `node scripts/test-viewer-playback.mjs --chrome --keyframe-recovery` | PASS | Chrome: `64x36`, 22 decoded frames, 40,800 audio samples; RTCP recovery and media release |
| `cargo clean` | PASS | Removed 8,161 files / 1.9 GiB after probes |
| `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS | Removed 9,581 files / 2.6 GiB; the previously recorded [BUG-002](../BUGS.md#bug-002-tauri-artifact-cleanup-is-blocked-by-root-ownership) is resolved |

## Scope and Remaining QA

BUG-001's playback, recovery, slow-start-under-load, and regression criteria are satisfied. The fixing commit must be recorded before changing its registry status to `CLOSED`.

Actual capture/permissions, physical audio output, packaged-app playback/install, Linux execution, cross-network behavior, and performance QA were **NOT RUN** in this focused check. The [full-project checklist](../QA_CHECKLIST.md) remains open; no release tag or version bump is justified by this record. The lifetime tests are portable and configured for Linux/macOS CI; this local run only verifies macOS.

Verification processes were stopped by the probe runner. Both workspaces were cleaned; a filesystem check confirmed that `target` and `apps/desktop/src-tauri/target` are absent. No new feature ID was completed.
