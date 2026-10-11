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

| Bug | Owning feature | Priority | Status | Kind | Next action |
| --- | --- | --- | --- | --- | --- |
| [BUG-008](#bug-008-tauri-viewer-stays-connecting-without-decoded-media) | [BASE-DESKTOP-CONTROLS](../todo.md#ui-preferences-and-tooling) | P1 | INVESTIGATING | Tauri WebRTC/media | Inspect host DTLS/candidates and compare retained Tauri/WKWebView failures with passing controls |
| [BUG-009](#bug-009-packaged-gui-run-stalls-during-refresh-preference-checks) | [BASE-DESKTOP-CONTROLS](../todo.md#ui-preferences-and-tooling) | P2 | INVESTIGATING | Unexplained packaged test stall | Repeat without concurrent browser work and identify the pending operation |
| [BUG-005](#bug-005-desktop-command-arguments-do-not-match-tauri-ipc) | [BASE-DESKTOP-CONTROLS](../todo.md#ui-preferences-and-tooling) | P1 | INVESTIGATING | Desktop IPC contract | Record verified IPC fix; decoded acceptance remains blocked by BUG-008 |
| [BUG-006](#bug-006-prototype-buttons-remain-disabled-after-valid-form-edits) | [BASE-DESKTOP-CONTROLS](../todo.md#ui-preferences-and-tooling) | P2 | INVESTIGATING | Input readiness | Real Tauri regression passes; record fixing commit |
| [BUG-007](#bug-007-desktop-diagnostics-overflow-the-window-horizontally) | [BASE-DESKTOP-CONTROLS](../todo.md#ui-preferences-and-tooling) | P2 | INVESTIGATING | GUI layout | All three native widths pass after fix; record fixing commit |

### BUG-005: Desktop Command Arguments Do Not Match Tauri IPC

- **Affected version/components:** untagged `0.1.0`, tested base `74a8e37`; desktop `main.js` and Rust Tauri command wrappers; ownership `BASE-DESKTOP-CONTROLS`.
- **Priority/status/kind:** P1, INVESTIGATING, product IPC contract defect.
- **Evidence/reproduction (2026-10-11):** real Tauri GUI run `/private/tmp/video-p2p-gui-qa/debug-third`, opt-in DOM driver with production IPC, no backend mocks. Save a signaling/ICE configuration; change refresh interval/audio/source; prepare host/viewer. Required snake_case arguments are rejected; optional ones are silently ignored. `session.conf` contains only room and default refresh settings. The UI catches string IPC rejections as `undefined`.
- **Expected/actual/reliability:** all supplied settings must reach the backend, invalid ICE must fail, and valid roles must start. Instead Save reports success without saving signaling/ICE, refresh preferences stay default, and source/role commands fail. Deterministic across controls and restart scenarios.
- **Cause:** Tauri commands default to camelCase arguments; the production frontend sends snake_case. Installed `tauri-macros` command parsing confirms the default.
- **Implementation/verification:** all production wrappers explicitly accept snake_case; string rejection reasons are displayed. Final real Tauri debug controls/restart/role IPC checks pass. The instrumented release's first save/ICE validation checks also pass before a separate stall. Decoded acceptance remains blocked by BUG-008; keep this entry unresolved. See the [GUI QA record](qa/tauri-gui-0.1.0-74a8e37.md).
- **Closure criteria/next action:** align the explicit IPC contract and preserve useful rejection text; fail before/pass after real Tauri save/invalid-input/source/refresh/restart/host/viewer checks, including decoded playback and Stop. Record fixing commit and mandatory checks.
- **Deferral:** none.

### BUG-006: Prototype Buttons Remain Disabled After Valid Form Edits

- **Affected version/components:** untagged `0.1.0`, desktop form input/readiness; ownership `BASE-DESKTOP-CONTROLS`.
- **Priority/status/kind:** P2, INVESTIGATING, product input-state defect.
- **Evidence/reproduction (2026-10-11):** start with empty room/signaling fields, refresh (prototype buttons disabled), then fill both fields. With Auto Refresh disabled, Join Viewer and Start Host Prototype remain disabled until manual Refresh. The first viewer GUI run hit the disabled Join Viewer button before invoking IPC.
- **Expected/actual/reliability:** button readiness should follow current input immediately. Input only marks drafts dirty, leaving controls based on the previous render. Deterministic; independent of BUG-005.
- **Cause:** `markSessionFieldDirty` does not refresh prototype controls.
- **Implementation/verification:** input now recomputes guards from the current form and session. The same empty-to-valid regression fails before and passes after; valid-to-empty also passes with polling disabled. Fixing commit recording is pending. See the [GUI QA record](qa/tauri-gui-0.1.0-74a8e37.md).
- **Closure criteria/next action:** real Tauri regression checks empty-to-valid and valid-to-empty edits with polling disabled; no command/refresh required to update readiness. Record fixing commit and mandatory checks.
- **Deferral:** none.

### BUG-007: Desktop Diagnostics Overflow the Window Horizontally

- **Affected version/components:** untagged `0.1.0`, desktop CSS/detail rows/diagnostics grid; ownership `BASE-DESKTOP-CONTROLS`.
- **Priority/status/kind:** P2, INVESTIGATING, product layout defect.
- **Evidence/reproduction (2026-10-11):** expand Diagnostics in the real Tauri window and resize to 1120, 800, and 600 logical pixels. All three checks in `debug-third/controls.json` detect document width exceeding the viewport. The driver uses native window resizing and DOM geometry, not a simulated viewport.
- **Expected/actual/reliability:** controls/detail panels should fit horizontally with internal scrolling for logs/specification. Instead the whole document gains horizontal overflow; deterministic in the recorded run.
- **Cause/implementation:** diagnostic grid auto minimums expand to the long specification/log content; narrow layouts and detail values also need zero-minimum tracks and wrapping. Explicit bounded tracks, minimum widths, and wrapped detail values fix the overflow. Final real native window checks pass at 1120, 800, and 600 pixels with Diagnostics expanded; fixing commit recording is pending. Screenshot review remains blocked separately. See the [GUI QA record](qa/tauri-gui-0.1.0-74a8e37.md).
- **Closure criteria/next action:** contain diagnostic grid minimum widths and long detail values; repeat native-window geometry checks at all three sizes with expanded diagnostics and realistic long values. Record fixing commit and mandatory checks. Visual screenshot QA remains separate.
- **Deferral:** none.

### BUG-008: Tauri Viewer Stays Connecting Without Decoded Media

- **Affected version/components:** untagged `0.1.0`; actual Tauri WKWebView viewer, after BUG-005's argument fix; ownership `BASE-DESKTOP-CONTROLS`, related `BASE-VIEWER-PROBES`.
- **Priority/status/kind:** P1, INVESTIGATING, observed product/runtime failure; environment contribution is not yet established.
- **Evidence/reproduction (2026-10-11):** `/private/tmp/video-p2p-gui-qa/viewer-diagnostics/viewer.json`. A real Tauri viewer joins the synthetic native H.264/Opus host, with Auto Refresh disabled. After 20 seconds SDP/ICE are ready, two tracks are live, signaling is connected, and a direct candidate pair is selected; transport stays `connecting`, video dimensions/time/received bytes/decoded counts stay zero. The element is unpaused, muted, visible, in a secure context. Two Tauri runs reproduce it; the normal and recovery standalone WKWebView control probes pass against the same host/player code.
- **Expected/actual:** the desktop WebView must establish transport and decode video/audio. A passing standalone browser probe does not establish Tauri playback. The sound label also waits for a pending play operation while no media arrives; that symptom is not yet established as an independent defect.
- **Cause/next action:** unknown; inspect host DTLS/candidate diagnostics and compare retained Tauri/WKWebView failures against passing browser controls. Release and peer diagnostics below establish the symptom but not its cause. Do not bypass media/signaling checks or substitute a mocked player.
- **Further investigation:** release viewer also fails, sometimes reaching terminal peer failure. Actual peer stats show ICE connected/stable signaling with a nominated pair, but DTLS connecting (client), 1,944 bytes/9 packets sent and zero received. Later normal/recovery standalone WKWebView runs also fail with no media, while Chrome and WKWebView EOF runs can pass. A Tauri-only cause is not established. All failures/retries remain in the [GUI record](qa/tauri-gui-0.1.0-74a8e37.md); no fix or closure is claimed.
- **Closure criteria:** failing-before/passing-after real Tauri decoding with correct pixels and increasing audio/video counts, independent viewer signaling, sound-state/Stop/Reset/reconnect checks, and mandatory verification. Actual capture, audible device output, and user gesture QA remain separate.
- **Deferral:** none.

### BUG-009: Packaged GUI Run Stalls During Refresh Preference Checks

- **Affected version/components:** untagged `0.1.0`, QA-instrumented release `.app`, GUI regression driver; ownership `BASE-DESKTOP-CONTROLS`.
- **Priority/status/kind:** P2, INVESTIGATING, observed test/runtime stall. A product cause versus fixture/environment contribution is not established.
- **Evidence/reproduction (2026-10-11):** full bundled run `/private/tmp/video-p2p-gui-qa/packaged-reviewed` completes eight controls checks, then the report stays at `Auto Refresh toggle and interval boundaries persist` until the external 90-second timeout terminates the app. `session.conf` ends at interval 3 and Auto Refresh enabled; there are no captured frontend errors or Rust panic logs. The same final controls run passes all 22 checks in debug. The following restart check fails because the timed-out setup never saved its final settings; this is dependent evidence, not a second preference defect.
- **Expected/actual/reliability:** each preference update must finish and the GUI remain responsive. The bundled run stops producing evidence mid-scenario. A controls-only repeat completes seven checks then stalls during draft preservation; observed twice at nearby one-second-refresh steps. The repeat overlapped standalone browser verification, so the entire OS was not idle. No cause or closure is claimed; see the [GUI record](qa/tauri-gui-0.1.0-74a8e37.md).
- **Cause/next action:** unknown; repeat controls without concurrent browser verification, add operation-level timing if needed, and distinguish UI timers, pending IPC, native enumeration, and test-driver behavior.
- **Closure criteria:** establish cause, implement a fix if required, and verify the original bundled scenario plus application restart with recorded regression evidence. A passing retry alone does not close this entry.
- **Deferral:** none.

## Closed Issues

| Bug | Owning feature | Priority | Status | Kind | Resolution |
| --- | --- | --- | --- | --- | --- |
| [BUG-001](#bug-001-intermittent-wkwebview-playback-probe-failure) | [BASE-VIEWER-PROBES](../todo.md#desktop-viewer-and-recovery) | P2 | CLOSED | Test fixture lifecycle | Connection-relative host lifetime; slow-start and all browser modes verified |
| [BUG-002](#bug-002-tauri-artifact-cleanup-is-blocked-by-root-ownership) | [BASE-DEV-WORKFLOW](../todo.md#ui-preferences-and-tooling) | P2 | CLOSED | Local environment | Both workspace cleanups succeeded; both target directories are absent |
| [BUG-003](#bug-003-rejected-signaling-clients-can-relay-and-remove-the-registered-peer) | [BASE-SIGNALING](../todo.md#webrtc-and-signaling) | P1 | CLOSED | Registration lifecycle | Rejected-client isolation and failed-registration rollback verified for both roles |
| [BUG-004](#bug-004-graceful-signaling-disconnect-is-reported-as-connected) | [BASE-SIGNALING](../todo.md#webrtc-and-signaling) | P1 | CLOSED | Client connection lifecycle | EOF/recovery/reconnect verified; both browsers keep decoding after viewer signaling loss |

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

### BUG-003: Rejected Signaling Clients Can Relay and Remove the Registered Peer

- **Affected version/components:** untagged `0.1.0`; `crates/signaling-server/src/main.rs`, registration and client cleanup. Owning feature: `BASE-SIGNALING`; related future milestone: M5.
- **First observed (2026-10-11):** inspection of `6841888` found that an occupied-role rejection writes `ERROR` but returns `Ok(())`. `handle_client` consequently enters the relay loop and later unregisters the already occupied role. Registration announcement/replay write errors also return after inserting the new participant, without unregistering it.
- **Reproduction:** join a room as sender/receiver, then connect a second client with the same role, send a valid signaling envelope, and disconnect. Separately, fail a writer during initial registration/peer announcement/replay. Before the fix, `cargo test -p signaling-server` failed all three new regressions: rejected ICE was delivered to the opposite peer, failed `WAITING` left the room allocated, and failed pairing left the joining role occupied; the two existing tests passed.
- **Expected/actual:** a rejected client must not relay, alter replay history, or remove the accepted peer; a failed registration must release its own slot. The former control flow permitted all of these lifecycle violations.
- **Cause:** rejection was reported as successful registration, and cleanup started only after registration returned successfully.
- **Implementation/verification (2026-10-11):** rejection now returns an error before the relay loop; announcement/replay write failures unregister the inserted participant. All six focused tests pass, including both roles, all initial write phases, retry/replay, and normal accepted-client relay/disconnect. Both workspaces passed formatting, strict Clippy, tests, and cleanup; native extraction, JS lifecycle tests, probe lifetime tests, and all four WKWebView/Chrome playback/recovery modes passed. See the [focused record](qa/BUG-003-registration-lifecycle.md).
- **Fixing commit/version:** `12b96ae0380cec1dc6054cd3019200f4ffb191cd`, on the untagged `0.1.0` development baseline; no release tag or version bump.
- **Resolution:** `CLOSED`; before/after regressions, normal lifecycle/retry/replay checks, and full automated verification satisfy the closure criteria. Subsequent closure/evidence edits are documentation only.
- **Closure criteria:** reproduce the defects with failing regression tests, terminate rejected clients before relay, roll back inserted participants on announcement/replay failure, preserve the opposite participant/history, and verify normal pairing, late replay, retry, and disconnect. Record the fixing commit and both workspace/native/browser checks.
- **Deferral:** none; the existing supported signaling path needed an immediate fix.

### BUG-004: Graceful Signaling Disconnect Is Reported as Connected

- **Affected version/components:** untagged `0.1.0`; `app-core::signaling`, `SessionManager`, desktop/CLI recovery diagnostics. Owning feature: `BASE-SIGNALING`; related milestone: M5 room lifecycle.
- **First observed (2026-10-11):** inspection of `1810c03` found that `SignalingConnection::poll` handles `read == 0` like a temporarily empty socket. `SessionManager` therefore keeps `signaling_connected = true` after a server's graceful disconnect. Its existing recovery test sets the disconnected flag directly and does not exercise EOF.
- **Reproduction/reliability:** establish a signaling connection, receive `WAITING`, then have the server shut down its write side or exit gracefully. Poll/refresh the client repeatedly. Before the fix, `cargo test -p app-core signaling::tests` failed five EOF regressions (two normal parsing tests passed), and `cargo test -p app-core graceful_signaling_close` failed both session recovery checks. Unix socket shutdown makes the reproducer deterministic; a separate loopback TCP check covers the production socket type.
- **Expected/actual:** EOF must expose disconnected signaling and reconnect guidance; complete final messages (especially server rejection reasons) must be delivered before closure. The former client silently returned an empty successful poll forever after EOF.
- **Cause:** the nonblocking read loop did not distinguish EOF from `WouldBlock`; session poll errors also left the failed connection available for repeated polling/logging.
- **Implementation/verification (2026-10-11):** EOF now records a terminal read state, complete final events are delivered once, and the next poll returns a contextual closure/truncation error. Sending after observed EOF fails. Session poll errors release the signaling connection once and preserve transport/player ownership. Seven read/parser tests and two session tests pass. Both workspaces passed formatting, strict Clippy, tests, and cleanup; native extraction, eight JS lifecycle tests, four probe lifetime tests, and all six WKWebView/Chrome normal/recovery/EOF modes passed. EOF modes verified continued decoded video/audio on the same peer with unavailable signaling and reconnect guidance. See the [focused record](qa/BUG-004-signaling-eof.md).
- **Fixing commit/version:** `34585a32c335e155412f847e78d353cd32f5f876` on the untagged `0.1.0` development baseline; no release tag or version bump.
- **Resolution:** `CLOSED`; failing before/passing after regressions, fresh TCP reconnect, and actual synthetic video/audio decoding after viewer signaling EOF satisfy the closure criteria. Later closure/evidence edits are documentation only. Actual capture and packaged/cross-network interruption QA remain open.
- **Closure criteria:** demonstrate a failing EOF regression; distinguish idle/fragmented reads from closure, preserve final messages and server errors, diagnose incomplete final lines, reject sends after observed EOF, release failed session signaling once, and verify host/viewer recovery and manual reconnect. Preserve established WebRTC playback and generation/stop behavior. Record fixing commit and all mandatory workspace/native/browser results.
- **Deferral:** none; correct disconnect detection was required by the existing recovery workflow.
