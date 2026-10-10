# QA Checklist

This is a set of checks to perform, not a record that manual QA has passed. Real macOS capture/permissions, packaged playback, Linux playback, different networks, and performance measurements remain open in [TODO](../todo.md).

For each manual run, record commit/build, OS/WebView version, source type, monitor/audio setup, network/ICE servers, steps, result, and logs for any failure. Mark a scenario complete only after recording its result.

## Automated Checks

Run from the repository root with local sockets and macOS codec services available where required:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all --all-features
node --test apps/desktop/tests/viewer-player.test.mjs
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
```

Root CI runs the root Rust/Node checks on Linux/macOS, plus `sh scripts/test-macos-capture.sh` on macOS. It does not check/package Tauri or run native browser playback probes. Native extraction and codec tests use synthetic sample buffers/frames, not actual screen recording.

For synthetic decoded playback/recovery in WKWebView and Chromium, follow [viewer verification](VIEWER_PLAYBACK.md#verification). These probes verify pixels, Opus PCM, RTCP recovery, and cleanup; they do not establish actual capture, audible devices, user gestures, or cross-network behavior.

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
