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

| Bug | Owning feature | Priority | Status | Kind | Impact |
| --- | --- | --- | --- | --- | --- |
| [BUG-001](#bug-001-intermittent-wkwebview-playback-probe-failure) | [BASE-VIEWER-PROBES](../todo.md#desktop-viewer-and-recovery) | P2 | OPEN | Test/integration; product cause unconfirmed | One WKWebView run reached its video-width assertion without a decoded frame |
| [BUG-002](#bug-002-tauri-artifact-cleanup-is-blocked-by-root-ownership) | [BASE-DEV-WORKFLOW](../todo.md#ui-preferences-and-tooling) | P2 | BLOCKED | Local environment | Tauri `cargo clean` cannot remove pre-existing root-owned artifacts |

### BUG-001: Intermittent WKWebView Playback Probe Failure

- **Affected version/components:** `0.1.0`; [playback probe](../scripts/test-viewer-playback.mjs), WKWebView, and the synthetic host/viewer integration. Related milestones: M1/M2 verification; ownership remains with the baseline probe.
- **First observed:** pre-commit validation recorded in `c86e437`, on macOS with Node `26.11.0`. The ordinary WKWebView probe ran while the separate Tauri workspace was compiling. A single failed attempt is recorded; reproduction frequency is unknown.
- **Reproduction attempt:** build `signaling-server` and the `app-core` `playback_probe` example, then run `node scripts/test-viewer-playback.mjs`. Compare repeated isolated runs with runs while the Tauri workspace is compiling; preserve host/viewer/browser diagnostics. These conditions describe the observed run, not a deterministic reproducer.
- **Expected:** decoded `64x36` video, nonzero decoded audio, and successful media cleanup within the probe deadline.
- **Observed:** exit code `1` at the width assertion, `actual: 0`, `expected: 64`. An isolated retry passed with `64x36`, 24 decoded frames, and 41,760 audio samples. Recovery and both Chrome modes subsequently passed.
- **Latest verification (2026-10-10):** sequential checks for this documentation-only change, based on `c86e437`, passed after both Rust workspace builds finished: ordinary/recovery WKWebView and ordinary/recovery Chrome, including media cleanup and RTCP recovery where applicable. No fix was implemented; the cause and concurrent-build reproduction remain unresolved, so the entry stays `OPEN`.
- **Cause:** unknown. Concurrent compilation was present; resource contention, a probe issue, and a media/runtime issue have not been distinguished. This evidence does not establish a defect in actual screen capture.
- **Next action:** collect diagnostics on failed attempts, determine a reproducer/cause, and implement the appropriate fix with regression evidence. Keep successful retries visible without treating them as a resolution.
- **Closure criteria:** identify and address the cause; retain a regression check; verify ordinary/recovery modes in WKWebView and Chrome, including the previously observed concurrent-build conditions. Record the fixing commit/version and verification evidence.
- **Deferral:** none; no planned feature currently establishes that it replaces the affected probe/media behavior.

### BUG-002: Tauri Artifact Cleanup Is Blocked by Root Ownership

- **Affected version/components:** local `0.1.0` development checkout; artifact cleanup for the separate `apps/desktop/src-tauri` workspace. Related milestone: M7 build/distribution tooling; this is an environment problem, not evidence of an application defect.
- **Evidence:** repeated `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` attempts, including execution outside the sandbox, fail with `Permission denied (os error 13)` at `target/debug/.fingerprint/desktop-app-9b0e9fca0a28ae60/bin-desktop-app` beneath that workspace. The fingerprint directory/file are owned by `root`.
- **Reproduction:** run `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` in the affected checkout. The failure is persistent with the current ownership; a fresh normally owned checkout is not known to have this problem.
- **Expected:** Cargo removes local build artifacts from both workspaces after verification.
- **Observed:** root-workspace cleanup succeeds; Tauri cleanup fails and leaves artifacts outstanding.
- **Latest verification (2026-10-10):** root `cargo clean` removed 7,937 files / 1.9 GiB after the automated checks. Tauri cleanup again failed at the same root-owned fingerprint with `Permission denied (os error 13)`; the entry remains `BLOCKED`.
- **Cause/blocker:** pre-existing root ownership prevents the development user from deleting the affected files. Noninteractive `sudo -n chown` returned `sudo: a password is required`; local administrator authentication is unavailable to unattended execution.
- **Next action:** an authenticated local administrator restores ownership of this project's Tauri target directory to the development user, then reruns Cargo cleanup. Scope the action to that target directory and preserve source files, preferences, and shared caches.
- **Closure criteria:** ownership is corrected and `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` completes successfully; record the command/result. No Rust application change or version bump is required for this local environment repair.
- **Deferral:** none; future release/bundle features do not justify leaving incorrect local ownership unresolved.

## Closed Issues

None recorded yet. Keep resolved entries with their feature links, fixing commit/version where applicable, and closure evidence.
