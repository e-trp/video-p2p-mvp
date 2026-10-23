# TODO — Current Status and Milestones

The baseline implementation inventory was verified against code and tests at `0c6273f`; the project guidelines were added at `f1cb06b`. Later completed fixes link to their verification records below. The roadmap does not claim unimplemented features or manual QA results.

Development rules, English-only project content, mandatory checks, and version/tag policy: [AGENTS.md](AGENTS.md).

Current defects and their feature ownership: [bug register](docs/BUGS.md). Actionable bugs take priority over new features; defer only for a documented replacement/substantial redesign of their affected implementation.

- **FULLY COMPLETED**: the stated result exists within its documented scope and is supported by code, completed artifacts, or automated checks. Outstanding manual QA is listed separately; the product is not ready for release.
- **PARTIALLY COMPLETED — INCOMPLETE**: part of the result exists, but implementation or required verification remains.
- **NOT DONE**: the implementation is absent or the stated check has not run. An unverified scenario is incomplete.

Use `[x]` only for fully completed results. `[ ]` marks outstanding work. Runtime scaffolding, RTP counters, and synthetic playback do not replace actual capture/application verification.

## Version Checkpoint

| Field | Current state |
| --- | --- |
| Project version | `0.1.0` in all first-party manifests and Tauri bundle metadata |
| Latest annotated release tag | None; the baseline is untagged |
| Newly completed feature IDs since adopting this policy | None — `0/5`; the existing inventory is the baseline |
| Last passing full-project QA checkpoint | None; automated checks alone do not satisfy manual QA |
| Next feature milestone | M1, after actionable bugs; baseline macOS capture/playback QA is the first feature prerequisite |
| Release gate | Open; [current Tauri GUI automation passes](docs/qa/gui-client-0.1.0-6bd1a31.md), but [the CI regression fix awaits Ubuntu verification](docs/BUGS.md#bug-010-late-viewer-test-races-the-server-waiting-response), and actual capture, OS/device/network and distribution QA remain outstanding |

Update this checkpoint as feature IDs are completed. Keep ordinary task commits separate. Run full QA at each milestone, before any version tag, and at five newly completed features at the latest. Record the covered IDs and reset the counter only after a passing full QA run. Documentation, refactoring, and fixes do not count as features. Tag only a verified release commit; a docs-only update keeps `0.1.0`.

## FULLY COMPLETED

### WebRTC and Signaling

- [x] Real Rust `PeerConnection`: offer/answer, ICE, connection state, stats, and a bootstrap data channel.
- [x] H.264/Opus host tracks and encoded publication; the diagnostic Rust viewer receives/counts RTP.
- [x] **BASE-SIGNALING:** TCP signaling client/server, SDP/ICE envelopes, relay, and replay for the second participant joining later.
- [x] Local host/viewer SDP/ICE integration tests and late-join replay checks.
- [x] Rejected signaling clients cannot relay or remove accepted peers; failed registration writes release the joining slot. Both roles and retry/disconnect/replay behavior have regression coverage; see [BUG-003 verification](docs/qa/BUG-003-registration-lifecycle.md).
- [x] Graceful signaling closure exposes reconnect guidance and releases the failed connection once, preserving final messages and viewer ownership; EOF/fragmentation/rejection and host/viewer/reconnect regressions pass. See [BUG-004 verification](docs/qa/BUG-004-signaling-eof.md).
- [x] WebRTC session flow in CLI and Tauri; legacy mock UDP is separate from the real media path.

### macOS Capture and Codec Implementation

- [x] Shared `capture-core` contracts for sources/permissions, BGRA/PCM, and runtime configuration/status/events.
- [x] **BASE-MACOS-CAPTURE:** Native Objective-C/C ABI ScreenCaptureKit bridge: window/application/display catalog using native IDs, asynchronous start, timeout, and stop/drop.
- [x] Passive Screen Recording preflight, explicit requests at startup, and distinct permission-required/denied states.
- [x] Packed BGRA with timestamps, incomplete-frame filtering, and aspect-ratio preservation; macOS 13+ PCM conversion to interleaved float32, 48 kHz/stereo.
- [x] Standalone capture probe reports validated PCM peak/RMS, distinguishes required signal from permitted silence, and has portable tests plus an opt-in real-source regression runner; see [BUG-013 verification](docs/qa/BUG-013-command-line-capture.md).
- [x] Concurrent native audio isolation runner uses independently launched application bundles, checks window/application silence while another source plays a tone, and verifies audio opt-out; see [BUG-014 verification](docs/qa/BUG-014-audio-isolation.md).
- [x] Native bounded queues: latest video frame and up to 32 audio buffers; permission-free extraction/lifecycle tests.
- [x] VideoToolbox H.264 Constrained Baseline, SPS/PPS/IDR, Annex B, resize/odd-dimension handling, and PTS/buffer validation; actual encoder tests with synthetic frames.
- [x] Vendored Opus, 20 ms packet aggregation, timestamp/discontinuity handling, and actual encode/decode tests.
- [x] Continuous capture/encode/publish worker independent of GUI refresh; source/audio validation, encoding only with a connected peer, and stop/join before replacing sources/sessions/runtimes.
- [x] Synthetic capture → native H.264/Opus → local WebRTC/RTP tests, including stop/restart and audio opt-out.

### Desktop Viewer and Recovery

- [x] Receive-only WebView `PeerConnection`: video-element decoding/playback without media crossing Tauri IPC.
- [x] Muted startup, Enable Sound/Mute Sound, playback/error states, and a separate viewer signaling pump.
- [x] Generation guards and peer/track cleanup on stop/reset/reconnect/role change/failure; refresh or delayed callbacks cannot revive a stopped session.
- [x] Rust bridge and Node lifecycle tests, including ICE ordering, autoplay, terminal failure, and transient disconnect/recovery.
- [x] **BASE-VIEWER-PROBES:** Synthetic WKWebView/Chromium playback probes: decoded dimensions/pixels, nonzero Opus PCM, and media cleanup after stop/refresh.
- [x] Bounded probe startup and connection-relative streaming lifetime, with four portable regression tests, slow-start verification under compilation load, and failure diagnostics; see [BUG-001 verification](docs/qa/BUG-001-startup-lifetime.md).
- [x] Video-SSRC RTCP PLI/FIR, FIR duplicate detection, coalescing, and a 500 ms cooldown; successful keyframe publication or disconnect/close clears pending state.
- [x] Host keyframe request/publication/pending diagnostics, SRTCP/publication-race tests, and capture stop/restart recovery.
- [x] WKWebView/Chromium recovery probes that drop the first keyframe and verify browser-originated RTCP refresh.

### UI, Preferences, and Tooling

- [x] **BASE-DESKTOP-CONTROLS:** Host/viewer preparation, source picker, capture start/poll/stop, session stop/reset, and manual reconnect with recovery guidance. Real Tauri GUI defects are tracked in the bug register.
- [x] Source selection survives Auto Refresh and native Accessibility menu interaction; unavailable-source errors preserve the prior selection. [BUG-011/BUG-012 verification](docs/qa/BUG-011-source-selection.md) records six focused checks, three native selections, and mandatory verification. Fixes do not advance the `0/5` feature counter.
- [x] Transport/capture/playback diagnostics, selected ICE path, and available RTT/bitrate/bytes/loss metrics.
- [x] Persisted room/signaling/ICE/source/audio selection and Auto Refresh preferences; polling preserves unsaved form edits.
- [x] Custom STUN/TURN validation and configuration propagation to Rust/WebView peers through CLI/Tauri settings.
- [x] State-only local GUI demo and separate non-playable debug bursts.
- [x] Linux `wmctrl` metadata catalog, example fallback, and runtime contract scaffold within that scope; media capture is absent.
- [x] macOS `.app`/`.dmg` bundle configuration, icons/metadata, and generation/build helper scripts.
- [x] Root CI: Rust format/Clippy/tests and Node viewer tests on Linux/macOS; native extraction tests on macOS. The [late-join response-order regression](docs/qa/BUG-010-late-viewer-ordering.md) records local verification and outstanding hosted Ubuntu acceptance.
- [x] Installation/user/networking/media/capture/playback guides and a manual QA checklist.
- [x] **BASE-DEV-WORKFLOW:** Project `AGENTS.md`: Rust style, safe FFI, implementation order, tests, documentation maintenance, and build-artifact cleanup rules. This records the instructions artifact; local cleanup results are tracked separately.
- [x] English project guidelines/documentation, stable milestone feature IDs, and a version/checkpoint policy with annotated-tag gates.
- [x] Dedicated bug register with stable bug/feature IDs, evidence, priorities, lifecycle/closure rules, and a documented module-replacement exception.
- [x] Opt-in real Tauri GUI regression runner for production DOM/IPC, preferences across application restarts, native window layout, and decoded synthetic playback. Foreground guards prevent inactive-bundle test suspension. Debug and bundled runs pass all 32 checks; see the [current GUI QA record](docs/qa/gui-client-0.1.0-6bd1a31.md).

## PARTIALLY COMPLETED — INCOMPLETE

| Outcome | Existing implementation | Required completion |
| --- | --- | --- |
| macOS end-to-end streaming | Native capture, codecs, worker, viewer, and synthetic playback | Actual audio, other source types, and release-bundle verification; local debug-bundle window video is verified |
| Cross-platform capture/playback | Shared contracts, macOS backend, Linux metadata/scaffold | Linux media backend/encoder and WebView QA (M3/M4) |
| Production GUI | Source/session/media controls and diagnostics | Media settings (M1), invite UX (M5), and platform QA |
| Backpressure/weak-network handling | Bounded queues, latest-only diagnostics, write timeout, keyframe recovery | Network-aware pacing/drop policy and adaptive bitrate (M2) |
| NAT traversal | Custom ICE configuration and direct/relay hints | Real STUN/TURN/NAT tests, services, and managed credentials (M6) |
| Packaging/release | macOS bundle configuration, icons, and build scripts | Verified installation/startup, signing/notarization, and Linux bundles (M7) |

## NOT DONE

### Current Bug Priority

Consult [current issues](docs/BUGS.md#current-issues) before feature work, then follow the baseline QA prerequisites below. Bug status and closure evidence live only in the register. The baseline `BASE-*` references identify existing behavior/tooling and do not count as new milestone features.

### Baseline QA Prerequisites

The [2026-10-19 actual-window GUI run](docs/qa/actual-window-0.1.0-0185449.md) verifies a real TextEdit window between two local debug bundles, visible content changes, playback with host Auto Refresh disabled, and capture stop/restart. The [BUG-013 regression](docs/qa/BUG-013-command-line-capture.md) additionally covers standalone window/application/display samples and audio opt-in/out. The [BUG-014 regression](docs/qa/BUG-014-audio-isolation.md) covers concurrent fixture audio isolation. Broader application/device audio behavior, audible output, permission transitions and distribution acceptance remain open.

Run the [QA checklist](docs/QA_CHECKLIST.md) on real devices. These are verification tasks, not new feature IDs:

- [ ] Actual macOS permission deny/grant/relaunch/retry in the app/CLI.
- [ ] Actual window/application/display capture and audio filtering/routing, including multiple monitors and changing/closing a source.
- [ ] Actual native capture → desktop viewer video/audio, autoplay/user gestures, and sound output devices.
- [ ] Packaged Tauri build/install/startup, playback, and stop/reset/close/startup cleanup.
- [ ] Regression coverage for existing desktop controls, preferences, recovery, mock demo, and diagnostic CLI paths; record all checklist sections with explicit results.

The [2026-10-12 GUI record](docs/qa/gui-client-0.1.0-6bd1a31.md) verifies real Tauri controls, persisted settings and decoded synthetic playback in debug and bundled builds after the BUG-008/BUG-009 fixes. The [original failed run](docs/qa/tauri-gui-0.1.0-74a8e37.md) remains preserved. This prerequisite stays open: Screen Recording, Accessibility, actual screen/audio capture, audible devices, multiple monitors, installation/signing and cross-network QA are not verified. The runner and fixes advance no new feature ID.

Resolve discovered defects before extending the affected path. Where hardware/permissions are unavailable, keep the checks blocked and work on the next independent task; required blocked QA prevents a release tag.

### Ordered Feature Milestones

Each milestone is an independently verifiable outcome made of small coherent task commits. Proposed versions assume this order; if an independent milestone ships earlier or a block is split, assign the next unused minor version and update the plan. Patch releases do not change the milestone order.

| Order | Proposed tag | Functional block | New feature IDs | Depends on |
| --- | --- | --- | --- | --- |
| M1 | `v0.2.0` | Measurable macOS streaming and media controls | M1-F1–F3 (3) | Baseline QA for the affected media path |
| M2 | `v0.3.0` | Streaming under congestion and packet loss | M2-F1–F2 (2) | M1 metrics and controls |
| M3 | `v0.4.0` | Linux Wayland capture-to-playback | M3-F1–F4 (4) | Shared media contracts; Linux environment |
| M4 | `v0.5.0` | Linux X11 capture fallback | M4-F1 (1) | M3 encoder and Linux playback validation |
| M5 | `v0.6.0` | Bounded, protected signaling and invite access | M5-F1–F3 (3) | Existing signaling/session contracts |
| M6 | `v0.7.0` | Deployed direct/relay connectivity | M6-F1–F2 (2) | M5 authentication and deployment |
| M7 | `v0.8.0` | Installable distribution on supported platforms | M7-F1–F2 (2) | Required platform/network milestones |

Every block ends with the [full-project QA/version gate](docs/QA_CHECKLIST.md#full-project-qa-and-version-gate), even below five features. Review all checklist sections; unsupported future capabilities may be `N/A` with an explicit scope reason. Already supported scenarios remain required regression checks.

### M1 — Measurable macOS Streaming and Media Controls

- [ ] **M1-F1:** A/V sync monitoring and capture-to-render latency instrumentation with explicit units, diagnostics, and measurements on actual media; browser playout already exists, skew measurement does not.
- [ ] **M1-F2:** Quality/FPS/resolution/bitrate presets applied through UI and backend configuration, with validation and persisted settings; current defaults are fixed in the backend.
- [ ] **M1-F3:** Audio-routing settings and supported-source behavior with clear platform limitations; preserve existing Include Audio behavior.

Acceptance: baseline permission/source/playback/preference checks pass; record real latency, A/V skew, and CPU under a stated load. Test preset/audio changes and lifecycle cleanup. Full-project QA is required before `v0.2.0`.

### M2 — Congestion and Packet-Loss Handling

- [ ] **M2-F1:** Network-aware frame pacing and a bounded congestion/drop policy, with deterministic overload/loss tests and lifecycle validation.
- [ ] **M2-F2:** Adaptive bitrate/quality driven by measured network feedback, with recovery, bounds, and user-setting behavior.

Acceptance: sustained loss/congestion/outage tests show measured latency, CPU, A/V skew, and recovery on actual media. The initial-keyframe-drop probe alone is insufficient. Full-project QA is required before `v0.3.0`.

### M3 — Linux Wayland Capture-to-Playback

- [ ] **M3-F1:** Portal ScreenCast session, user permission/source selection, cancellation, and resource cleanup.
- [ ] **M3-F2:** PipeWire video consumption using shared raw-frame contracts, with format/lifetime/backpressure handling.
- [ ] **M3-F3:** Linux audio capture/selection, timestamp handling, and opt-out behavior.
- [ ] **M3-F4:** Linux H.264 encoder compatible with the negotiated media path, including unsupported-codec errors and encoder lifecycle tests.

Acceptance: actual GNOME Wayland and KDE Wayland capture/video/audio reach the desktop WebView; H.264/Opus support, error handling, permissions, and stop/drop are verified. Full-project QA, including macOS regression checks, is required before `v0.4.0`.

### M4 — Linux X11 Media Fallback

- [ ] **M4-F1:** Actual X11 fallback media capture with source/audio selection, lifecycle cleanup, and reuse of the Linux encoder/playback path; `wmctrl` currently provides metadata only.

Acceptance: actual X11 capture-to-playback and unsupported-environment errors pass; rerun Wayland and macOS regressions. Full-project QA is required before `v0.5.0`.

### M5 — Protected Signaling and Invite Access

- [ ] **M5-F1:** Bounded production room/message/replay lifecycle, TTL, rate limits, and deployment observability, with abuse/cleanup tests.
- [ ] **M5-F2:** TLS signaling and corresponding client/deployment configuration; the current TCP server provides no HTTPS/WSS.
- [ ] **M5-F3:** Authenticated room access with invite code/link UX, expiry/rejection behavior, and documented access rules.

Acceptance: valid peers connect securely, unauthorized/expired invites and oversized/rate-limited traffic fail predictably, and rooms/resources are reclaimed. Rerun desktop/CLI signaling and media regressions. Full-project QA is required before `v0.6.0`.

### M6 — Deployed Direct and Relay Connectivity

- [ ] **M6-F1:** Documented/deployed STUN/TURN services and client configuration, including deployment defaults and operational diagnostics.
- [ ] **M6-F2:** Authenticated short-lived TURN credential issuance, expiry/refresh handling, and credential validation tests.

Acceptance: demonstrate STUN direct and TURN relay paths across different networks/restrictive NAT, selected-candidate diagnostics, connectivity loss/change/reconnect, and sustained congestion. Full-project QA is required before `v0.7.0`.

### M7 — Installable Distribution

- [ ] **M7-F1:** Linux bundle targets/configuration and a reproducible build/install workflow for the supported platforms.
- [ ] **M7-F2:** macOS signing/notarization and distribution/install validation.

Acceptance: build/install/launch packaged applications, verify permissions, actual capture/playback/audio, preferences, and stop/close cleanup on supported systems. Full-project QA is required before `v0.8.0`. After the required MVP scope and distribution QA are complete, assess `1.0.0` with documented supported behavior/API compatibility.

Detailed limitations: [capture](docs/MACOS_CAPTURE.md), [media](docs/MEDIA_PIPELINE.md), [viewer](docs/VIEWER_PLAYBACK.md), and [networking](docs/NETWORKING.md). A completed checklist document does not mean its checks have passed.
