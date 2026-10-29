# BUG-010 — Late Viewer Response Ordering

**Local regression verification: PASS. Hosted Ubuntu acceptance: PASS (confirmed 2026-10-29).** The supplied Ubuntu Actions assertion is a test ordering race, reproduced deterministically on macOS before the fix. The integration test now waits for both local offer readiness and the asynchronous server waiting acknowledgement before joining the viewer. BUG-010 is CLOSED after confirming a successful hosted run containing the unchanged fix.

## Scope and Evidence

- Date: 2026-10-12; untagged `0.1.0`, local base `70276b6`.
- Fixing implementation: `72a9927c066c72d9727b8f670208dafa22746b7c`; checks ran on that code before committing. Subsequent status/commit metadata edits are documentation only.
- Affected file: `crates/app-core/tests/host_viewer_flow.rs`; owning feature `BASE-SIGNALING`, related `BASE-DEV-WORKFLOW`.
- The user supplies the Ubuntu `late_viewer_join_replays_offer_over_real_tcp_signaling` failure at line 192: missing `signaling server is waiting for the second peer`; one of two integration tests fails, Cargo exits 101.
- Public GitHub API inspection finds 10 runs. The latest three fail Ubuntu `Run tests`, while their macOS matrix jobs are cancelled. Latest remote commit is `615cb2a84235cb81ebf472539386e8e9a3e573f4`, not the current local GUI implementation. The supplied error is the detailed assertion evidence; API step metadata alone does not establish identical assertions in every failed run.
- Full job log download returns HTTP 403 (`actions/jobs/114166325534/logs`); the public job page requires sign-in to view logs. No authentication token is supplied or accessed.
- Local environment: macOS `27.0` (`26A428`), Rust/Cargo `1.99.0`, Node `26.11.1`. Docker client exists but its configured `desktop-linux` daemon socket does not; `docker version` exits 1. There is no available Ubuntu runner. This is an execution prerequisite, not another project bug.

## Actions History

| Run | Commit | Failed step/platform |
| --- | --- | --- |
| [38035966630](https://github.com/e-trp/video-p2p-mvp/actions/runs/38035966630) | `615cb2a8` | Ubuntu Run tests; macOS cancelled |
| [38034422745](https://github.com/e-trp/video-p2p-mvp/actions/runs/38034422745) | `f1cb06bb` | Ubuntu Run tests; macOS cancelled |
| [37781533975](https://github.com/e-trp/video-p2p-mvp/actions/runs/37781533975) | `ad8a0bba` | Ubuntu Run tests; macOS cancelled |
| [33505124030](https://github.com/e-trp/video-p2p-mvp/actions/runs/33505124030) | `e7fd56d1` | Run Clippy |
| [33504819263](https://github.com/e-trp/video-p2p-mvp/actions/runs/33504819263) | `d8263e57` | Run Clippy |
| [30799505610](https://github.com/e-trp/video-p2p-mvp/actions/runs/30799505610) | `32163a7b` | Check formatting |
| [28510205897](https://github.com/e-trp/video-p2p-mvp/actions/runs/28510205897) | `e2175ea2` | Check formatting |
| [28504562836](https://github.com/e-trp/video-p2p-mvp/actions/runs/28504562836) | `1efb308e` | Run Clippy |
| [28504556642](https://github.com/e-trp/video-p2p-mvp/actions/runs/28504556642) | `f0c333a1` | Run Clippy |
| [28504343890](https://github.com/e-trp/video-p2p-mvp/actions/runs/28504343890) | `909058ab` | Run Clippy |

All 10 runs have conclusion `failure`; the older seven are separate formatting/lint failures whose full diagnostics were not retrieved. Checkout Node-runtime deprecation and upcoming Ubuntu-image migration annotations are warnings; the latest job actually reaches and fails Cargo tests. Cancellation of the other OS is consistent with the workflow's default matrix fail-fast, which [GitHub documents as enabled by default](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idstrategyfail-fast); that configuration is preserved. No workflow, toolchain, dependency, production signaling or GUI behavior is changed.

Machine-local responses are saved as `/private/tmp/video-p2p-actions-runs.json`, `/private/tmp/video-p2p-actions-<run>-jobs.json`, and `/private/tmp/video-p2p-actions-<check>-annotations.json`. The [normalized repository evidence](BUG-010-late-viewer-ordering-results.json) retains run/job conclusions, failed steps and controlled test outcomes.

## Cause and Controlled Regression

Host startup connects, creates/sends its local SDP offer, and returns without waiting for server registration. Signaling reads are deliberately nonblocking. The old `wait_until` predicate checks only `local_offer_ready`, then immediately asserts a server log that may not have arrived. CPU/thread/TCP scheduling can expose this independent ordering on any platform.

A test-only channel gate withholds the first registration response. With the original readiness predicate, the added regression fails at the same missing-waiting-log assertion: exit 101, zero passed/one failed, 0.26 seconds. Dropping the fixture's release sender unblocks registration during panic cleanup.

After refining the gate to release inside the first polling predicate, restoring only the original offer-only condition again fails deterministically at the same assertion: exit 101, zero passed/one failed, 0.11 seconds. The final fixture ordering is identical for this reverted-condition failure and the passing fixed-condition run.

The final regression first asserts that a host offer exists while the gated server acknowledgement is absent. Registration is released only after the polling snapshot is taken, inside the wait predicate. That first snapshot therefore lacks the response regardless of subsequent server scheduling: restoring the original offer-only predicate deterministically fails the waiting assertion. The fixed predicate waits for both offer and waiting log within the existing five-second bound, then executes the same late-viewer replay checks as the ordinary path. Real TCP offer/answer/replay, active-peer checks, and subsequent negotiation remain asserted. No fixed startup delay or increased timeout hides the race.

```bash
cargo test -p app-core --test host_viewer_flow late_viewer_join_waits_for_delayed_server_registration -- --exact
cargo test -p app-core --test host_viewer_flow
```

After the fix, all three integration scenarios pass. Ten additional sequential runs also pass all three each (30/30), including ordinary negotiation, normal late replay, and deliberately withheld registration.

## Mandatory Local Checks

| Command | Result |
| --- | --- |
| `cargo fmt --all`; `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml` | PASS |
| Both formatting commands with `-- --check` | PASS |
| `cargo clippy --all-targets --all-features -- -D warnings` | PASS |
| `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | PASS |
| `cargo test --all --all-features` | PASS: 124 tests and all doctest targets |
| `cargo test -p app-core --example playback_probe --all-features` | PASS: 4 lifetime tests |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features` | PASS: compiles; zero crate-owned Rust tests |
| `node --test apps/desktop/tests/viewer-player.test.mjs` | PASS: 8 lifecycle tests |
| `sh scripts/test-macos-capture.sh` | PASS; native extraction/timing/backpressure/stop; nonfatal hypervisor sysctl diagnostic |
| `cargo build -p signaling-server`; `cargo build -p app-core --example playback_probe` | PASS |
| `node scripts/test-viewer-playback.mjs` | PASS: 64 × 36, final 20 frames/34,080 audio samples; Stop releases media |
| Same command with `--keyframe-recovery` | PASS: final 24 frames/44,160 samples; browser RTCP recovery and cleanup |
| Same command with `--chrome` | PASS: final 19 frames/32,640 samples and cleanup |
| Same command with `--chrome --keyframe-recovery` | PASS: final 20 frames/36,960 samples; browser RTCP recovery and cleanup |

All checks are repeated after the final registration-gate refinement; both browser batches pass and the normalized evidence preserves their separate results. Actual capture/device/network/manual GUI acceptance is not reclassified by this fixture-only fix. The earlier desktop GUI record remains separate.

## Status and Follow-up

As of the original 2026-10-12 investigation, BUG-010 was FIXED in `72a9927c066c72d9727b8f670208dafa22746b7c`, pending hosted Ubuntu verification. The later acceptance below closes that requirement. No push, release tag, version bump, or feature-counter change is part of this task.

README, architecture and installation guides were reviewed; their CI scope, component boundaries and commands remain accurate. Networking, TODO, QA checklist and bug register describe the corrected test ordering and separate remote acceptance. Cleanup is performed after all builds/tests/probes, preserving the previously saved GUI bundles, preferences, lockfiles and pre-existing `.python-version`.

Documentation consistency passes: 171 relative links/anchors, 43 shell blocks, no Cyrillic authored prose across tracked/new text including hidden configuration, and normalized run/regression totals. `git diff --check` passes; the complete source/documentation diff and generated evidence are reviewed.

All owned verification processes have finished. Final `cargo clean` exits 0 and removes 8,291 files / 1.9 GiB; `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` exits 0 and removes 9,581 files / 2.6 GiB. Both target directories are absent; previously saved GUI bundles remain outside them. Download caches, source files and user preferences are preserved.

## Hosted Acceptance — Confirmed 2026-10-29

The public GitHub Actions API reports [run 40301710908](https://github.com/e-trp/video-p2p-mvp/actions/runs/40301710908), created 2026-10-26, successful at commit `819a636922159662b33421924fb79ee9f5e80fcc`. Both [Ubuntu job 121500925069](https://github.com/e-trp/video-p2p-mvp/actions/runs/40301710908/job/121500925069) and [macOS job 121500925224](https://github.com/e-trp/video-p2p-mvp/actions/runs/40301710908/job/121500925224) completed successfully, including format, Clippy, Rust tests and Node lifecycle tests. Native extraction passed on macOS and was platform-skipped on Ubuntu. The retained [normalized API evidence](BUG-010-hosted-acceptance.json) records job/step conclusions and URLs; full job logs were not downloaded.

`git merge-base --is-ancestor 72a9927c066c72d9727b8f670208dafa22746b7c 819a636922159662b33421924fb79ee9f5e80fcc` exits 0. `git diff` between those commits for `crates/app-core/tests/host_viewer_flow.rs` is empty; both blobs are `098e5d8712ca2d148fc9b84e42ae20f6fb2c8cb5`. The published run therefore includes the original fix and unchanged controlled delayed-registration regression. The existing workflow's root test command runs this integration target. This is acceptance on a descendant containing the fix, not an assertion that Actions ran at the fixing commit itself.

No publication or workflow dispatch was needed: the user had already published the descendant. This check only reads remote refs and the public API. The prior local failing-before/passing-after evidence plus hosted Ubuntu success meet BUG-010's closure criteria. The project's remaining native-source and release QA stay open.

The 2026-10-29 local per-change gate passes all 17 mandatory commands plus six cleanup regressions on macOS 27.2 / Node 26.13.0, base `045257d` with this documentation-only diff. Root tests pass 127 cases, probe lifetime tests 4, Node viewer/cleanup tests 8/6; both Clippy/format workspaces, Tauri tests, native extraction and all four WKWebView/Chrome playback/recovery modes pass. Exact commands, durations and logs are saved under `/private/tmp/video-p2p-bugs-remaining/bug010-checks/`. After process checks, `cargo clean` removes 8,129 files / 1.9 GiB and desktop `cargo clean` removes 10,109 files / 2.9 GiB, both exit 0; both repository-local targets are absent. The separately prepared BUG-016 QA bundle/probes remain outside target directories for its pending permission comparison.

README and architecture were reviewed and remain accurate. Networking, TODO, QA checklist and bug register now distinguish completed hosted acceptance from outstanding native/release QA. Links/anchors, shell syntax, tracked-text language scanning, normalized JSON and `git diff --check` pass. Final result-recording edits change no tested code/configuration.
