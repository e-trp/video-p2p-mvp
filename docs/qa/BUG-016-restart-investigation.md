# BUG-016: Native Restart Investigation

2026-10-27, base `819a636`, untagged `0.1.0`, macOS 27.0 (26A428), arm64. This investigation leaves BUG-016 unresolved. It changes no production capture code, timeout, permission, or native ownership contract.

## Controlled Results

| Run | Result | Evidence |
| --- | --- | --- |
| Existing native lifecycle, default command sandbox | FAIL after initial decoded pixels/PCM; replacement's catalog lookup reports the source unavailable | `/private/tmp/video-p2p-bug016-oct27-before/` |
| Same binary and scenario, explicitly outside the sandbox | FAIL after initial decoded pixels/PCM; replacement reports `Failed during stream due to application connection being interrupted` | `/private/tmp/video-p2p-bug016-oct27-unrestricted/` |
| Standalone native bridge with a new pthread per capture | PASS for first capture, same-source restart, alternate source and restoration, with audio off and on; all interleaved catalog queries succeed | `/private/tmp/video-p2p-bug016-thread-unrestricted/` |
| Experimental native stop completion before output detachment | FAIL at the same replacement publication check; experiment reverted | `/private/tmp/video-p2p-bug016-stop-order/` |
| Same desktop binary copied into an ad-hoc signed temporary `.app`, launched using `open -n -W` | BLOCKED before capture: independent bundle reports Screen Recording `required` and blueprint fallback | `/private/tmp/video-p2p-bug016-bundle-before/` |

Commands:

```bash
cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --features gui-qa
cargo build -p signaling-server
cargo build -p capture-macos --example capture_probe
python3 scripts/test-native-desktop-playback.py --source-lifecycle --output /private/tmp/video-p2p-bug016-oct27-before
python3 scripts/test-native-desktop-playback.py --source-lifecycle --output /private/tmp/video-p2p-bug016-oct27-unrestricted
python3 /private/tmp/video-p2p-bug016-thread.py /private/tmp/video-p2p-bug016-thread-unrestricted 1
python3 scripts/test-native-desktop-playback.py --source-lifecycle --output /private/tmp/video-p2p-bug016-stop-order
python3 /private/tmp/video-p2p-bug016-bundle.py --source-lifecycle --output /private/tmp/video-p2p-bug016-bundle-before
```

The unrestricted direct-launch binary SHA-256 is `89ab7c16374343d14dab27227f1de267b01a3bb003158d91d2f296db4d857c1c`. Standalone diagnostic source and launch scripts remain under `/private/tmp/video-p2p-bug016-thread-probe.m`, `video-p2p-bug016-thread.py` and `video-p2p-bug016-bundle.py`. These are investigation artifacts, not maintained acceptance tests. The first sandboxed standalone fixture launch failed with LaunchServices `kLSNoExecutableErr`; its approved unrestricted run passed. No failed result is overwritten or counted as a PASS.

The native thread comparison uses the production C bridge, real fixture windows, and four captures per audio mode. Every phase receives video, audio-on receives PCM, audio-off receives none, and no native error event occurs. It has no Tauri, encoding or connected viewer, so it cannot establish end-to-end restart acceptance.

## Findings and Next Action

`/private/tmp/video-p2p-bug016-system.log` records SCStream output removal, stop and destruction at 11:04:20, immediately followed by repeated ReplayKit connection interruptions in both direct-launched clients. It also contains sandbox IPC denials. The unrestricted repetition still fails, ruling out removal of the command sandbox as a sufficient repair. The log does not establish whether application identity, framework state, or another OS condition causes the interruptions.

Moving output detachment into the stop completion handler does not repair this scenario. That temporary native edit is fully reverted; no unverified lifecycle change is retained. Thread creation alone also does not reproduce it in the standalone bridge. No timeout is increased and no system service is killed or restarted.

The next comparison requires a retained QA application bundle with Screen Recording permission granted through macOS, then the existing source-lifecycle test through LaunchServices. The temporary bundle used here was removed after its process exited; it never obtained capture permission. Do not change TCC databases or treat direct-launch success on another path as bundled acceptance. BUG-016 remains BLOCKED on this comparison; BUG-015's original switch/restore/closure acceptance remains open. Independent GUI fixture work can proceed.

The per-change gate is shared with the subsequent preference-fixture task and saved under `/private/tmp/video-p2p-bug018-checks/`. It runs with those QA-only changes present and no production capture changes. This investigation is committed separately. It adds no feature ID, version bump, release tag, or full-project QA claim.

## Resumed Verification and Cleanup — 2026-10-28

All 17 mandatory commands pass on the final QA implementation, including both workspace format/lint/test gates, native extraction and four browser playback/recovery modes. Exact commands, exit codes and logs are retained in `/private/tmp/video-p2p-bug018-resumed-checks/results.json` and `1.log` through `17.log`. The focused preference lifecycle passes 10 checks; the full GUI run records 43 PASS / 0 FAIL / 1 BLOCKED because the permission-denied scenario requires an environment without the existing Screen Recording grant. These results do not verify native source replacement.

After the runners exited, process inspection found no remaining test/build processes. `cargo clean` removed 9,167 files / 2.0 GiB and `cargo clean --manifest-path apps/desktop/src-tauri/Cargo.toml` removed 12,745 files / 3.7 GiB; both exit 0. Both target directories were repository-local, with no `CARGO_TARGET_DIR` override. Logs remain outside target directories. Final documentation recording changes no tested code/configuration.

## Retained Permission Comparison — 2026-10-29

macOS 27.2 (26B123), base `045257d`, unchanged native implementation. The approved unrestricted command `python3 scripts/test-native-desktop-playback.py --source-lifecycle --output /private/tmp/video-p2p-bugs-remaining/direct-before` again fails after initial actual pixels/PCM pass. Replacing window `macos-window-411` with `macos-window-417` preserves the selected ID and stops old publication, then reports `Failed during stream due to application connection being interrupted`; final permission is `granted`, capture is `failed`, and encoded counts remain 93 video / 162 audio. Reports, fixture identities/cleanup and a normalized `direct-before-summary.json` are retained in that evidence directory. This reproduces the prior failure on the newer OS; it does not identify its cause.

A copy of the current `gui-qa` executable is retained in `/private/tmp/video-p2p-bugs-remaining/VideoP2P Source Lifecycle QA.app`, identifier `dev.videop2p.qa.source-lifecycle`. The app is ad-hoc signed and `codesign --verify --deep --strict` passes. `qa-bundle-identity.json` records its binary SHA-256 and identity. It has an isolated configuration directory and does not replace the installed application. The catalog probe and signaling binary are also saved outside Cargo targets.

LaunchServices preflight uses `open -n -W`, per-process `--env` QA configuration, the existing `source-picker` scenario and `preflight.json`/`preflight.log`. All eight source-picker/control checks pass, but the initial catalog permission is `required`. This passive run neither requests nor grants Screen Recording. The user was asked to add this exact retained application in macOS Privacy & Security settings and enable Screen Recording. Permission to run shell commands does not supply the application's TCC grant. No permission database is edited or reset.

`/private/tmp/video-p2p-bugs-remaining/run-bundle.py` prepares the existing source-lifecycle scenario using this app through LaunchServices, forwarding only QA environment values and tracking/cleaning up the newly launched QA processes. It has passed Python syntax validation but has **not** run the capture comparison while permission is pending. Once access is confirmed, run it with a fresh evidence output path, then compare direct launch of the same signed binary. Keep BUG-016 and dependent BUG-015 acceptance open until the responsible behavior is established and switch/restore/closure playback passes.

The 17 mandatory commands and six probe-cleanup tests pass on this exact application code during the adjacent BUG-010 verification; see `/private/tmp/video-p2p-bugs-remaining/bug010-checks/results.json`. Both workspaces are then cleaned (8,129 files / 1.9 GiB and 10,109 files / 2.9 GiB). The retained QA bundle/probes and diagnostic logs remain outside targets, and no test process remains. These final investigation/status edits change documentation only; the application code/configuration is unchanged. This is pending investigation evidence, not a code fix or a passing native lifecycle run.

## Bundle Registration and Permission Follow-up — 2026-11-02

The user reports adding the application permission and notes it was already present. The restarted QA process still reports `required`, independently of that report. Read-only Accessibility inspection of the Screen & System Audio Recording panel confirms an enabled entry named `VideoP2P`; its label alone does not identify the QA executable. The production identifier resolves to `/Applications/VideoP2P.app`, which remains untouched.

The external comparison wrapper's first attempt (`bundle-after-grant`) fails before launching a client because replacing the signaling executable path accidentally discards its port argument. `server.log` shows port 7000 instead of the reserved port. The wrapper is corrected to preserve the remaining arguments; this is a diagnostic setup failure, not a capture regression. `bundle-after-grant-2` then launches the actual QA host through LaunchServices, but initial catalog permission is `required` and the fixture is absent. No media starts. Both attempts retain logs and cleanup evidence.

The scoped TCC log (`tcc-after-grant.log`, timestamps 2026-10-29) attributes the ScreenCapture preflight to `dev.videop2p.qa.source-lifecycle` and reports `kLSApplicationNotFoundErr` when looking up that identifier. Explicit `lsregister -f` and a direct public `LSRegisterURL` call return success for the temporary app, yet `NSWorkspace` identifier lookup remains null. A further passive source-picker run (`registered-preflight.json`) passes all eight controls while permission remains `required`. No TCC database is read, edited or reset, and no permission toggle is changed by the agent.

On 2026-11-02, the same bundle is copied without modification to `/Users/evgeniytoropchin/Applications/VideoP2P Source Lifecycle QA.app`. The destination did not exist; no installed app is overwritten. Both binaries have SHA-256 `39b91c1e5020d1cfd3618669cc208d67678057ff24276085063518c826991ded`, and both pass `codesign --verify --deep --strict`. At this persistent path, `LSRegisterURL` returns 0 and identifier lookup returns that exact app URL. The controlled path difference establishes the registration prerequisite in this environment; it does not establish an OS-wide rule about all temporary applications.

The external runner is pointed at the persistent copy. `retained-bundle-first` still stops at initial permission `required` before capture, so neither restart nor media behavior is tested. The user is asked to confirm Screen Recording for this exact QA application in the already open settings panel. `retained-bundle-identity.json` preserves the matching identities and registration results. The next step is the same signed binary through LaunchServices and direct launch after the QA process observes access; BUG-016 and BUG-015 remain unresolved. This is a repaired test setup and a specific external prerequisite, not a production fix.

## Permission Resolved and Same-Binary Comparison — 2026-11-03

The user grants Screen Recording to the registered QA application; fresh processes observe `granted`. The [controlled comparison](native-lifecycle-2026-11-03.md) passes all 17 actual lifecycle/Stop checks for executables inside the unchanged signed `.app`, both direct/direct and LaunchServices-host/direct-viewer. An identical standalone executable copy reproduces replacement failure after initial pixels/PCM pass. This establishes a launch-context difference on the tested OS, not a production repair. BUG-016 is INVESTIGATING rather than permission-blocked or closed.

All-LaunchServices runs separately expose initial negotiation failure (BUG-023) and a hidden-window QA precondition defect (BUG-022). Their failed evidence remains preserved; passing controls do not erase them. BUG-015's independent catalog-retention implementation now has its outstanding bundled lifecycle acceptance. The linked comparison and [visibility repair record](BUG-022-native-window-visibility.md) contain final checks and cleanup.
