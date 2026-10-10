# Local Demo Manual Check

The local demo exercises UI/session state only. It starts no ScreenCaptureKit stream, codec pipeline, WebRTC peer, or media delivery. A placeholder peer and active readiness indicator do not prove real streaming. Use [QA](QA_CHECKLIST.md) for actual capture/playback.

## Start The App

```bash
cd apps/desktop/src-tauri
cargo tauri dev
```

Verify the runtime badge shows **Tauri runtime connected**, host/viewer/readiness/overview controls render, and loading text resolves. Static browser preview does not execute these commands.

## Simulated Workflow

1. Click **Run Local Demo**.
2. Verify room `local-demo`, a selected source, placeholder peer `local viewer demo`, and `host / mock_streaming` overview.
3. Check the log says the local GUI demo started without external signaling/peer setup.
4. Click **Stop**; confirm stopped/reconnect guidance.
5. Click **Reset**; confirm idle, no reconnectable role, and saved preferences restored.
6. Start the demo again and change **Available Source**; selection and host/readiness labels should update.

No server, second peer, STUN/TURN, or screen-recording access is needed. Runtime metadata or example fallback sources may be shown; notes should explain the origin. Manual results must be recorded before treating these UI checks as passed.

## Optional Real Session Check

Start signaling from the repository root:

```bash
cargo run -p signaling-server -- 127.0.0.1:7000
```

Reset the demo, explicitly set room `demo` and signaling `127.0.0.1:7000`, refresh/select a native macOS source, and click **Start Host Prototype**. The action prepares signaling and attempts capture; inspect runtime status and permission/source errors.

Join the same room from a second desktop instance. Confirm connected transport, increasing encoded/decoded counts, moving video, and audio after **Enable Sound**. A CLI `webrtc-viewer` can check negotiation but exits after connection and cannot validate continuous playback.

See [user guide](USER_GUIDE.md), [capture setup](MACOS_CAPTURE.md), and [viewer guide](VIEWER_PLAYBACK.md) for requirements. Starting the host does not inject debug samples automatically. **Send Test Frame** / **Push Debug Capture Burst** are non-playable diagnostics and should not be used in this playback check.

## Report A Failure

Record commit/build, OS, exact action/status/error, runtime badge, session/capture/transport diagnostics, and relevant Tauri terminal/log lines. If a loading state persists, record which field and whether the local demo action is available.
