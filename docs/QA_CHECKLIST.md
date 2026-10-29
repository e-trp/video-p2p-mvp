# QA Checklist

This is a set of checks to perform, not a record that manual QA has passed. Real macOS capture/permissions, packaged playback, Linux playback, different networks, and performance measurements remain open in [TODO](../todo.md).

For each manual run, record commit/build, OS/WebView version, source type, monitor/audio setup, network/ICE servers, steps, result, and logs for any failure. Mark a scenario complete only after recording its result.

Before classifying a macOS capture failure as a product defect, identify the exact executable/bundle, bundle identifier/signature, launch method (direct executable or LaunchServices), and observed permission state for that launch context. An installed application's Screen Recording grant does not establish the permission state of a separately built or copied QA executable. Keep direct-build checks and installed-application acceptance separate. Classify confirmed permission failures as environment/permission blockers; when attribution is uncertain, record the observed failure and unknown cause rather than claiming a code defect. Do not create a product bug solely from a failure under a different, unverified permission context.

Record observed defects and intermittent failures in the [bug register](BUGS.md), with their owning feature IDs. Link QA evidence to permanent bug IDs; a successful retry alone does not close a bug. Missing implementation and never-run QA remain in TODO/checklist.

## Full-Project QA and Version Gate

Follow the [version/tag policy](../AGENTS.md#commits-versions-and-git-tags) and [ordered milestones](../todo.md#ordered-feature-milestones). Ordinary tasks receive separate commits. Run a full-project checkpoint at each completed functional block, before every annotated version tag, and no later than five newly completed feature IDs. Documentation, refactoring, and fixes do not advance the feature counter; a fix-only version still requires this gate.

1. Identify the candidate commit/build, version, milestone, feature IDs, and supported scope. Synchronize package/bundle versions before final validation.
2. Run every automated check and available native/browser probe, then review every manual section below. Exercise all implemented paths and regression scenarios on supported systems, including packaged apps where supported.
3. Record each scenario as `PASS`, `FAIL`, `BLOCKED`, `NOT RUN`, or `N/A`, with evidence or a reason. Future capabilities outside the declared release scope may be `N/A`; missing hardware, OS, permissions, or network access for a required scenario is `BLOCKED`.
4. Fix failures and rerun affected checks. Required `FAIL`, `BLOCKED`, or `NOT RUN` results and unresolved bugs affecting required release scenarios keep the release gate open and prevent a version tag. A deferred replacement must meet the bug's regression/closure criteria. Passing synthetic probes does not close actual capture/device/network QA.
5. Save an English QA record under `docs/qa/` named for the candidate version and tested commit, such as `v0.2.0-<commit>.md`. Include the fields and scenario table below. Update TODO's checkpoint with the record, covered IDs, and gate result; reset the feature counter only after a passing full run.
6. Commit the version metadata, release notes, and QA record. The record must identify the tested tree; any later code/config/version changes require revalidation. Create the annotated `vMAJOR.MINOR.PATCH` tag on the resulting release commit after the gate passes. Save evidence/bundles, stop test processes, and clean both workspaces.

QA record fields: version/milestone, tested commit and any uncommitted version diff, build/bundle identifier, completed feature IDs, related bug IDs and closure evidence, OS/WebView/browser versions, source/monitor/audio setup, network/ICE servers, exact commands and results, regression notes, known limitations, and release-gate decision. Record-only changes after testing must be distinguishable from changes to the tested code/configuration.

| Checklist section/scenario | Environment/build | Result | Evidence or reason |
| --- | --- | --- | --- |
| Each automated command | Exact command and platform | PASS / FAIL / BLOCKED / NOT RUN | Output summary or failure |
| Each manual scenario below | OS, source/audio/network setup | PASS / FAIL / BLOCKED / NOT RUN / N/A | Steps, observations, and logs or scope reason |

There are currently no passing full-project release QA records. The `0.1.0` baseline remains untagged and required manual checks remain open.

The [2026-10-11 Tauri GUI run](qa/tauri-gui-0.1.0-74a8e37.md) exercises real desktop windows/IPC and records each checklist section. It found and verified fixes for three GUI defects, but desktop playback fails under BUG-008 and OS/device/network checks remain blocked. This is a failed GUI validation run, not a passing release checkpoint. Programmatic DOM actions are recorded separately from manual/system interactions.

The [2026-10-12 follow-up](qa/gui-client-0.1.0-6bd1a31.md) verifies the ICE and foreground-fixture fixes with all 32 checks passing in debug and in two full bundled runs. Synthetic desktop video/audio now decodes. Actual Screen Recording/device/network/distribution scenarios remain open, so this is still not a passing full-project release checkpoint.

## Automated Checks

Run from the repository root with local sockets and macOS codec services available where required:

```bash
cargo fmt --all -- --check
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --all --all-features
cargo test -p app-core --example playback_probe --all-features
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features
node --test apps/desktop/tests/viewer-player.test.mjs
node --test scripts/probe-cleanup.test.mjs
```

Root CI runs the root Rust/Node checks on Linux/macOS, plus `sh scripts/test-macos-capture.sh` on macOS. It does not check/package Tauri or run native browser playback probes. Native extraction and codec tests use synthetic sample buffers/frames, not actual screen recording.

The [BUG-010 record](qa/BUG-010-late-viewer-ordering.md) investigates Ubuntu CI's late-join assertion race and records controlled failing-before/passing-after local verification. A local fix does not establish a successful hosted Ubuntu run; that acceptance remains open until the fixing commit runs in Actions.

For synthetic decoded playback/recovery in WKWebView and Chromium, follow [viewer verification](VIEWER_PLAYBACK.md#verification). These probes verify pixels, Opus PCM, RTCP recovery, and cleanup; they do not establish actual capture, audible devices, user gestures, or cross-network behavior.

For production DOM handlers and IPC inside real Tauri windows, follow the [GUI runner](GUI_TAURI.md#real-tauri-gui-regression-runner). Preserve the runner's failed evidence and leave dependent media scenarios blocked when decoding never starts.

[AGENTS.md](../AGENTS.md) requires formatting/lint/tests for each completed change, all available macOS native/browser probes, documentation updates, and cleanup after the checks. Save results and any needed release bundles, stop test processes, then clean both workspaces:

```bash
cargo clean
cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml
```

Clean removes built binaries, so run it after playback probes. Rebuild before another test/probe run; never clean while Cargo or test processes are active.

The [native desktop playback record](qa/native-desktop-playback-0.1.0-c0d19af.md) verifies actual ScreenCaptureKit fixture pixels/PCM through two local Tauri clients, audio opt-out, programmatic mute controls and Stop cleanup. This focused run leaves physical output, trusted gestures, permission transitions and broader source/device scenarios open.

The [BUG-015 record](qa/BUG-015-catalog-refresh.md) verifies failed-refresh state retention. Its actual switch/restore/closure scenario remains blocked by BUG-016; do not mark source lifecycle complete from initial playback or passing unit tests.

The [BUG-017 record](qa/BUG-017-command-dispatch.md) separates controlled native-window responsiveness during session-lock contention from actual source replacement. The contention regression passes after blocking-pool dispatch; the follow-up native lifecycle run still fails under BUG-016.

## Backend Negotiation Smoke Test

Start signaling, then the host/viewer commands in separate terminals:

```bash
cargo run -p signaling-server -- 127.0.0.1:7000
cargo run -p p2p-cli -- webrtc-host --room qa-demo --signal 127.0.0.1:7000 --push-debug-capture
cargo run -p p2p-cli -- webrtc-viewer --room qa-demo --signal 127.0.0.1:7000
```

Verify offer/answer/ICE reaches a connected peer and the optional host burst increments publication counters. Both CLI peer commands exit after connection. A debug burst is non-playable and does not increment encoded counters. Use desktop instances for continuous real playback.

## Desktop Session And Source Controls — Manual

- [ ] Start Tauri from `apps/desktop/src-tauri`; verify **Tauri runtime connected**.
- [ ] Prepare a host and desktop viewer in the same room; verify offer/answer/ICE and connected state.
- [ ] Verify **Start Host Prototype** starts capture, while **Prepare Host** requires **Start Native Capture** separately.
- [ ] Confirm native catalog origin or explanatory fallback notes; select a source and audio setting.
- [ ] Change source/audio while capturing; confirm the old runtime stops and the new selection starts only after **Start Native Capture**.
- [ ] Close the selected window; refresh catalog and verify selection rebinding/old capture cleanup.
- [ ] Keep unsaved room/signaling/ICE edits during polling; confirm they are preserved and successful save/start applies them.
- [ ] Stop, then refresh repeatedly; the session stays stopped. Reset returns idle and restores saved preferences.

## macOS Actual Capture And Publication — Manual

See [capture guide](MACOS_CAPTURE.md) for permission/source details.

- [ ] Test denied access, grant, relaunch if required, catalog refresh, and retry on app/CLI.
- [ ] Select real window, application-per-display, and display sources on single/multiple monitors.
- [ ] Change source content; confirm capture is running and raw video summaries arrive.
- [ ] Play audio with Include Audio on (macOS 13+); confirm buffers and selected-source audio behavior.
- [ ] On a fresh host without a connected peer, confirm encoded publication counts remain zero. With a connected viewer, encoded/received/decoded counters increase.
- [ ] After negotiation, disable host Auto Refresh; media continues. Raw diagnostic counts depend on refresh cadence.
- [ ] Stop capture; encoded counters stop. Restart and verify new media/keyframe delivery.
- [ ] Disable audio and restart; encoded audio counters stop increasing while video continues. Check the viewer's actual audio behavior separately.
- [ ] Stop/reset/replace/close the host during startup and running capture; macOS recording indicators disappear and old publication stops.

`Native Video`/`Native Audio` are summaries. Raw diagnostic counts, encoded access units, received RTP packets, and decoded frames are different measurements.

## Desktop Viewer Playback And Recovery — Manual

- [ ] Join an actual macOS host from a second desktop; verify moving video and selected-source audio, not only counters.
- [ ] Verify muted startup, Enable Sound/Mute Sound, audible output devices, play/pause/fullscreen, and gesture recovery from autoplay block.
- [ ] Disable viewer Auto Refresh before joining; signaling and playback still proceed.
- [ ] Interrupt/restore connectivity briefly; transient messages clear without discarding playback/autoplay guidance.
- [ ] Stop viewer and repeatedly refresh; peer/tracks close, video clears, sound stops, and stale callbacks do not restore playback.
- [ ] Reconnect/reset/switch role during negotiation and buffered ICE; old generations cannot overwrite new state.
- [ ] Force terminal peer failure; show error and keep player closed until reconnect.
- [ ] Reconnect host; explicitly restart capture. Rebuild both peers if an established remote peer still has old SDP/DTLS state.
- [ ] Exercise loss/decoder refresh with changing source content; Keyframe Requests increase, a published keyframe clears pending work, and picture recovers. Feedback cooldown is 500 ms; initial/restart/resize frames may be immediate.
- [ ] Exercise sustained congestion/loss separately. Current keyframe recovery is not bitrate adaptation or proven weak-network handling.

Do not inject debug bursts during real playback checks. Use [demo checks](DEMO_MANUAL_TEST.md) only for the simulated workflow.

## Debug Burst — Manual

With a connected diagnostic host/peer, publish a debug burst with Include Audio on, then off. Verify source/audio validation, video publication, and no new audio publication after opt-out. Verify encoded counters do not count these payloads. This does not test decoding.

## Recovery And Preferences — Manual

- [ ] Stop a prepared role; recovery says stopped and reconnect is available/recommended.
- [ ] Reconnect with active or edited room/signaling/ICE settings; verify the role is rebuilt.
- [ ] Stop signaling and refresh; show signaling unavailability and appropriate guidance.
- [ ] Save non-default room/signaling/ICE/source/audio/Auto Refresh settings; restart and verify restoration from `session.conf`.
- [ ] Verify Reset reloads preferences rather than deleting them. Test fresh defaults using an isolated config directory.

## Linux — Manual / Blocked By Missing Implementation

Current checks cover only X11 `wmctrl` metadata/fallback, source selection, and structured scaffold status. Do not interpret granted catalog permission as functioning capture.

- [ ] After implementation, validate GNOME Wayland and KDE Wayland portal permission/source selection, PipeWire video/audio, and cleanup.
- [ ] After implementation, validate X11 media fallback and Linux video encoding.
- [ ] Validate WebView H.264/Opus support, playback, and unsupported-codec error handling on Linux.

## Cross-Network And Performance — Manual

- [ ] Use a reachable signaling address on different machines; loopback is not a remote server address.
- [ ] Check a direct path with configured STUN and a relay path with deployed TURN/credentials.
- [ ] Confirm selected candidate-pair path hints and available stats; reject invalid ICE schemes before save/start.
- [ ] Test restrictive NAT, loss/outage/reconnect, and sustained congestion.
- [ ] Measure capture-to-render latency, A/V skew, CPU, and packet loss, with environment/load recorded.

No managed STUN/TURN service or credentials flow is bundled. See [networking](NETWORKING.md).

## Packaged Release — Manual

```bash
./scripts/build-desktop-release.sh
```

- [ ] Verify generated icons and successful macOS `.app`/`.dmg` build.
- [ ] Install/launch the bundle and rerun permission, capture/playback, audio, preferences, and stop/close checks.
- [ ] Complete signing/notarization and validate the distribution/install path.
- [ ] Configure and validate Linux bundle targets after platform implementation.

Existing configuration/scripts are completed artifacts. Release build/install/playback/signing checks remain outstanding.
