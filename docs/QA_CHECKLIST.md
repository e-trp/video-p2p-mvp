# Manual QA Checklist

Use this checklist before treating a build as release-ready. The current project is still an MVP, so the checklist focuses on the behavior implemented today: signaling, WebRTC negotiation, desktop session control, source metadata, debug media publishing, recovery, preferences, and packaging.

## Backend Smoke Tests

Run from the repository root.

1. Check the default workspace:

```bash
cargo check
```

2. Run the test suite:

```bash
cargo test
```

3. After any substantial code change, format and lint the Rust workspace:

```bash
cargo fmt
cargo clippy
```

4. Start signaling:

```bash
cargo run -p signaling-server -- 127.0.0.1:7000
```

5. In two other terminals, verify WebRTC negotiation:

```bash
cargo run -p p2p-cli -- webrtc-host --room qa-demo --signal 127.0.0.1:7000 --push-debug-capture
```

```bash
cargo run -p p2p-cli -- webrtc-viewer --room qa-demo --signal 127.0.0.1:7000
```

Expected result:

- host and viewer connect to signaling
- host creates a local offer
- viewer accepts the offer and creates an answer
- host accepts the answer
- both peers report active peer metadata
- debug capture publishing increments video and, when selected audio is enabled, audio sample counters
- focused backend tests for `capture-core` stream events and session-manager capture event ingestion pass

## Desktop Development Run

1. Start the signaling server.
2. Launch the desktop shell:

```bash
cd apps/desktop/src-tauri
cargo tauri dev
```

3. Verify the app shows `Tauri runtime connected`, not browser preview.
4. Prepare a host with default room/signaling values.
5. Launch a second desktop instance or CLI viewer with the same room.
6. Confirm the desktop session snapshot shows:

- signaling connected
- local offer ready on host
- remote answer ready on host after viewer joins
- remote offer ready on viewer
- host media tracks attached
- transport diagnostics panel populated with stage, ICE path, stats count, and notes

## Capture Metadata

### macOS

Verify:

- capture backend is macOS
- catalog origin is `runtime` when application/window enumeration is available
- catalog falls back to `blueprint_fallback` with an explanatory note when runtime enumeration is blocked
- permission state is one of `granted`, `required`, `denied`, or `unknown`
- changing the selected source updates the session snapshot immediately

### Linux

Verify:

- capture backend is Linux
- X11 sessions with `wmctrl` available show runtime window metadata when possible
- unsupported or blocked environments fall back to `blueprint_fallback`
- Wayland portal capture is still reported as not implemented in current docs and troubleshooting notes
- changing the selected source updates the session snapshot immediately

## Debug Capture Publishing

On a prepared host session:

1. Select a source with audio enabled.
2. Click `Push Debug Capture Burst`.
3. Confirm video and audio counters increase.
4. Disable `Include Audio`.
5. Click `Push Debug Capture Burst` again.
6. Confirm video counter increases and audio counter does not.

Expected logs:

- video payload publication records the selected source id
- audio publication is skipped when `Include Audio` is disabled
- source mismatch errors do not appear during normal GUI operation

## Recovery And Reconnect

1. Prepare a host or viewer.
2. Click `Stop`.
3. Confirm recovery state is `stopped`, reconnect is available, and reconnect is recommended.
4. Click `Reconnect`.
5. Confirm the same role is rebuilt with the active room, signaling address, and ICE server list.
6. Stop the signaling server while a session is active.
7. Refresh the desktop shell.
8. Confirm recovery state becomes `signaling_unavailable` and reconnect guidance points to fixing signaling reachability.

## Preferences

1. Set a non-default room.
2. Set a non-default signaling address.
3. Add at least one STUN or TURN ICE server entry.
4. Change the auto-refresh setting and interval.
5. Select a capture source.
6. Save config or prepare a session.
7. Restart the desktop shell.

Expected result:

- room, signaling address, ICE servers, selected source metadata, and auto-refresh preferences are restored from `session.conf`
- unsaved form edits are not overwritten by background refresh polling
- `Reset` returns the live session to idle and reloads persisted preferences

## Network Traversal

For local tests, an empty ICE server list is acceptable. For cross-network tests:

1. Add a known-good STUN server.
2. Confirm candidate gathering still occurs.
3. Add a TURN server with `url|username|credential` format.
4. Confirm the configured ICE server count and summary are visible in CLI or desktop snapshots.
5. Check the ICE path summary for direct-vs-relay hints when a selected candidate pair is available.
6. Try an invalid ICE URL such as `https://turn.example.com` and confirm CLI or desktop validation rejects it before starting the session.

Current limitation:

- the repository does not ship a TURN service or auth flow
- real media delivery still uses placeholder tracks plus debug payload publishing; the live capture event ingestion API exists, but no OS backend feeds it yet

Use `docs/NETWORKING.md` as the reference for expected production signaling/STUN/TURN topology and security assumptions.

## Release Bundle

From the repository root:

```bash
./scripts/build-desktop-release.sh
```

Verify:

- generated icon assets exist under `apps/desktop/src-tauri/icons`
- `cargo tauri build` completes
- macOS bundle targets include `.app` and `.dmg`
- signing and notarization remain manual or external to this repository

## Known MVP Limits To Reconfirm

- no real ScreenCaptureKit media stream yet
- no Linux Portal + PipeWire media stream yet
- no real codec pipeline for captured OS samples yet
- no production invite/auth flow yet
- no bundled TURN deployment defaults yet
