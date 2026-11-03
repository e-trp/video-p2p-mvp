# Native Source Lifecycle and Permission Comparison

2026-11-03, base `4cc9e26`, untagged `0.1.0`, macOS 27.2 (26B123), arm64, native WKWebView. This completes the previously blocked actual-source acceptance for BUG-015. It does not repair the standalone capture restart failure (BUG-016). Programmatic GUI checks do not establish audible device output or trusted gestures.

## Identity and Permission

The retained application is `/Users/evgeniytoropchin/Applications/VideoP2P Source Lifecycle QA.app`, identifier `dev.videop2p.qa.source-lifecycle`. Its executable SHA-256 is `39b91c1e5020d1cfd3618669cc208d67678057ff24276085063518c826991ded`. The comparison uses this unchanged executable, including an identical standalone copy outside any bundle. The installed production app is untouched.

Before the user's grant, LaunchServices startup reports permission `required` and no native fixture in the fallback catalog. A separate run of the existing controls scenario passes all **22 checks**, including **Start Native Capture** and **Start Host Prototype** reporting `permission_required` without active capture. Evidence: `/private/tmp/video-p2p-permission-controls-nov03/controls.json`. This verifies the required-permission UI path, not explicit denial or grant/relaunch transitions.

After the user grants Screen Recording to this exact application, fresh host/viewer processes report `granted`, enumerate native sources and start capture. No permission database or toggle is changed by the agent.

## Controlled Comparisons

Every lifecycle attempt uses red/tone and blue/silent fixture applications, isolated preferences, a local signaling server, and the unchanged production capture/codec/transport/player implementation. Source applications are launched through LaunchServices. Evidence directories preserve fixture identities, cleanup, host/viewer reports and logs.

All paths below start with `/private/tmp/video-p2p-bug016-nov03-`.

| Suffix / client launch | Result |
| --- | --- |
| `launchservices` / both via LaunchServices, before grant | BLOCKED before capture: permission `required` |
| `after-grant` / both via LaunchServices | Initial native startup passes; viewer negotiation fails before any decoded media. BUG-023 remains open. |
| `direct` / both executables inside the retained `.app`, direct launch | **PASS, 17/17**, including replacement, restoration, disappearance and Stop |
| `unbundled/results` / identical standalone executable copy, direct launch | Initial pixels/PCM pass; replacement fails with `Failed during stream due to application connection being interrupted`, permission `granted`. Reproduces BUG-016. |
| `mixed` / retained host via LaunchServices, viewer direct inside `.app` | **PASS, 17/17** |
| `launchservices-repeat` / both via LaunchServices | Native replacement/restoration and host disappearance checks pass; viewer foreground guard and host Stop fail with hidden windows. BUG-022. This retry does not close BUG-023. |

Commands actually executed:

```bash
python3 /private/tmp/video-p2p-permission-controls-nov03.py
python3 /private/tmp/video-p2p-bugs-remaining/run-bundle.py /private/tmp/video-p2p-bug016-nov03-launchservices
python3 /private/tmp/video-p2p-bugs-remaining/run-bundle.py /private/tmp/video-p2p-bug016-nov03-after-grant
python3 /private/tmp/video-p2p-bug016-nov03-direct.py /private/tmp/video-p2p-bug016-nov03-direct
python3 /private/tmp/video-p2p-bug016-nov03-unbundled.py /private/tmp/video-p2p-bug016-nov03-unbundled/results
python3 /private/tmp/video-p2p-bug016-nov03-mixed.py /private/tmp/video-p2p-bug016-nov03-mixed
python3 /private/tmp/video-p2p-bugs-remaining/run-bundle.py /private/tmp/video-p2p-bug016-nov03-launchservices-repeat
```

These retained diagnostic wrappers use `scripts/test-native-desktop-playback.py` with explicit executable/catalog-probe paths and the saved signaling server. The LaunchServices wrapper uses `open -n -W`, QA-only environment variables and process ownership tracking. The direct wrapper changes only the saved signaling path. The standalone comparison copies the same signed executable without modifying it. These are investigation artifacts, not new supported launch options in the repository runner.

## Actual Acceptance

In the direct bundled run, the viewer decodes **1144×720** pixels. Initial/restored red is `[176, 73, 45, 255]`; alternate blue is `[39, 62, 184, 255]`. Alternate audio delivers samples with RMS **0** while the original source keeps playing; restored tone RMS is **0.03548**. The same viewer peer and generation survive both changes. Closing the original source stops host publication and new decoded frames, drains the tone, and does not automatically capture a rebound source. Stop ends tracks and closes the peer. Host/viewer Auto Refresh is disabled during media progression checks.

BUG-015's implementation remains `1f665d7`. Its earlier controlled failing-before/passing-after catalog/error-recovery tests are documented in [the original record](BUG-015-catalog-refresh.md). The successful bundled runs now supply its outstanding actual switch/restore/closure criterion. That source-retention fix can be verified independently of the still-reproducible standalone launch failure.

BUG-016 is no longer blocked on permission. The executable-location comparison narrows the failure to launch context in this environment; a passing bundle is not a production fix or proof of an OS-wide rule. Native source restart outside a bundle remains unresolved. Initial negotiation reliability is separately open under BUG-023.

The subsequent [BUG-022 record](BUG-022-native-window-visibility.md) records the QA-only visibility repair, final checks and cleanup. No new feature ID, version bump, release tag, physical-device/network acceptance or passing full-project checkpoint is claimed.
