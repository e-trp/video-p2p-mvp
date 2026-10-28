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
