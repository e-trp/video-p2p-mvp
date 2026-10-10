# Bug Register

Track observed failures here, linked to their owning feature. [AGENTS.md](../AGENTS.md#general-rules) defines bug-first work and the narrow module-replacement exception; [TODO](../todo.md#current-bug-priority) links to this register. Missing planned capabilities and manual checks that have never run remain in TODO/QA rather than becoming invented defects.

## Tracking Rules

- Assign permanent `BUG-NNN` IDs. Link new behavior to its milestone feature ID (`M1-F1`, etc.); link existing behavior/tooling to a `BASE-*` ID in TODO. Related future milestones may be listed separately from the owning feature.
- Record priority, status, kind, affected version/components, first-observed evidence, reproduction steps and reliability, expected/actual behavior, cause or uncertainty, next action, and closure criteria. Distinguish confirmed symptoms from suspected causes.
- Priorities: `P0` blocks all meaningful operation; `P1` breaks a core supported workflow; `P2` affects a limited workflow, test reliability, or development tooling; `P3` is a minor usability/documentation defect. Within priority, address actionable issues by impact and then oldest ID.
- Statuses: `OPEN` awaits investigation/fix; `INVESTIGATING` has active investigation; `BLOCKED` needs a recorded external prerequisite; `DEFERRED` has the documented replacement exception; `FIXED` has a fix and awaits remaining verification; `CLOSED` has met its closure criteria. Code fixes need a fixing commit/regression check; environment repairs need recorded commands/results. Only `CLOSED` is resolved.
- Defer only for a specific replacement/substantial redesign of the affected implementation. Record the feature ID, module/path, supersession reason, target milestone, and replacement regression/acceptance check. Keep the bug unresolved and blocking required release scenarios until verified; a generic future enhancement is insufficient.
- Update entries after relevant checks, including failed attempts and successful retries. Close with fixing commit/version and verification evidence; keep closed records for traceability. A retry or workaround alone is insufficient.

## Current Issues

No unresolved issues are currently recorded. New observed failures must be registered before extending the affected feature.

## Closed Issues

| Bug | Owning feature | Priority | Status | Kind | Resolution |
| --- | --- | --- | --- | --- | --- |
| [BUG-001](#bug-001-intermittent-wkwebview-playback-probe-failure) | [BASE-VIEWER-PROBES](../todo.md#desktop-viewer-and-recovery) | P2 | CLOSED | Test fixture lifecycle | Connection-relative host lifetime; slow-start and all browser modes verified |
| [BUG-002](#bug-002-tauri-artifact-cleanup-is-blocked-by-root-ownership) | [BASE-DEV-WORKFLOW](../todo.md#ui-preferences-and-tooling) | P2 | CLOSED | Local environment | Both workspace cleanups succeeded; both target directories are absent |

### BUG-001: Intermittent WKWebView Playback Probe Failure

- **Affected version/components:** `0.1.0`; [playback probe](../scripts/test-viewer-playback.mjs), WKWebView, and the synthetic host/viewer integration. Related milestones: M1/M2 verification; ownership remains with the baseline probe.
- **First observed:** pre-commit validation recorded in `c86e437`, on macOS with Node `26.11.0`. The ordinary WKWebView probe ran while the separate Tauri workspace was compiling. A single failed attempt is recorded; reproduction frequency is unknown.
- **Confirmed reproducer (2026-10-10):** the unchanged probe/host from `615cb2a`, with a temporary diagnostic copy delaying browser startup by 42 seconds, failed after 59,537 ms. The host had exited successfully (`hostExit: 0`), and the viewer remained `new`/`waiting` with `0x0` video, zero video frames/audio samples, and no tracks. This reproduces the originally reported width assertion. The fixed runner exposes the same condition through `VIEWER_TEST_BROWSER_STARTUP_DELAY_MS=42000`; see [verification](VIEWER_PLAYBACK.md#verification).
- **Expected:** decoded `64x36` video, nonzero decoded audio, and successful media cleanup within the probe deadline.
- **Observed:** exit code `1` at the width assertion, `actual: 0`, `expected: 64`. An isolated retry passed with `64x36`, 24 decoded frames, and 41,760 audio samples. Recovery and both Chrome modes subsequently passed.
- **Latest verification (2026-10-10):** the fixed delayed-start WKWebView probe passed while both Rust workspaces were compiling: `64x36`, eight decoded frames, and 12,000 audio samples. All four ordinary/recovery WKWebView/Chrome modes subsequently passed, including media cleanup and browser RTCP recovery. Both workspaces passed formatting, strict Clippy, and tests; four lifetime tests and eight JS lifecycle tests passed. See the [focused verification record](qa/BUG-001-startup-lifetime.md).
- **Cause:** the synthetic host's old 40-second deadline began before the WKWebView runner was compiled/browser started. A long startup consumed the entire lifetime before viewer negotiation. The initial historical run lacked host-exit diagnostics, but the controlled delayed-start reproduction confirms this fixture defect and the same symptom. Actual screen capture is outside this finding.
- **Implementation:** separate a bounded 90-second connection wait from 40 seconds of streaming beginning at the first connected state; reconnect does not extend the window. Four portable unit tests cover delayed startup, the startup failure boundary, late connection, and reconnect. Probe failures now print host exit state and returned diagnostics. CI explicitly runs the example tests.
- **Fixing commit/version:** `4f733d0de7e8761ce2c4070b0699fd5a730de670` on the untagged `0.1.0` development baseline. No release tag or version bump was created.
- **Resolution:** `CLOSED`; controlled before/after reproduction, portable regression tests, delayed startup under compilation load, and all four browser playback/recovery modes satisfy the closure criteria. Subsequent closure/evidence edits are documentation only.
- **Closure criteria:** identify and address the cause; retain a regression check; verify ordinary/recovery modes in WKWebView and Chrome, including the previously observed concurrent-build conditions. Record the fixing commit/version and verification evidence.
- **Deferral:** none; no planned feature currently establishes that it replaces the affected probe/media behavior.

### BUG-002: Tauri Artifact Cleanup Is Blocked by Root Ownership

- **Affected version/components:** local `0.1.0` development checkout; artifact cleanup for the separate `apps/desktop/src-tauri` workspace. Related milestone: M7 build/distribution tooling; this is an environment problem, not evidence of an application defect.
- **Historical evidence:** repeated `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` attempts, including execution outside the sandbox, failed with `Permission denied (os error 13)` at `target/debug/.fingerprint/desktop-app-9b0e9fca0a28ae60/bin-desktop-app` beneath that workspace. The fingerprint directory/file were owned by `root`.
- **Historical reproduction:** run `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` in the affected checkout. The failure persisted with that ownership; a fresh normally owned checkout was not known to have this problem.
- **Expected:** Cargo removes local build artifacts from both workspaces after verification.
- **Previously observed:** root-workspace cleanup succeeded; Tauri cleanup failed and left artifacts outstanding.
- **Closure verification (2026-10-10):** after BUG-001 verification, `cargo clean` exited `0` and removed 8,161 files / 1.9 GiB. `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` also exited `0` and removed 9,581 files / 2.6 GiB. A filesystem check confirmed both `target` and `apps/desktop/src-tauri/target` are absent. The development user is UID `501`. See the [verification record](qa/BUG-001-startup-lifetime.md#automated-results).
- **Historical cause:** pre-existing root ownership prevented deletion. A previous noninteractive `sudo -n chown` attempt required a password. No ownership-repair command was performed in this task, and the change that removed the earlier ownership obstacle is not established; the successful cleanup and absent target directories verify that no affected artifacts remain.
- **Resolution:** required cleanup completed with no application-code fix or version bump; no outstanding action remains for this checkout.
- **Closure criteria:** ownership is corrected and `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` completes successfully; record the command/result. No Rust application change or version bump is required for this local environment repair.
- **Deferral:** none; future release/bundle features do not justify leaving incorrect local ownership unresolved.
