# video-p2p-mvp

Rust + Tauri prototype for peer-to-peer window and audio streaming. The macOS host has native ScreenCaptureKit capture, H.264/Opus encoding, and WebRTC publication. The desktop viewer receives and plays media in its WebView.

Capture/encoding and playback have automated coverage using synthetic media, including real WKWebView and Chromium decoding. Actual window/audio capture through the packaged app, cross-network streaming, and Linux playback still need manual validation. Linux capture and video encoding are not implemented.

The [current GUI verification](docs/qa/gui-client-0.1.0-6bd1a31.md) tests real Tauri controls, saved preferences, native window layout, and decoded synthetic video/audio in debug and bundled builds. Real screen/audio capture and device/network acceptance remain separate requirements.

## Current Capabilities

- TCP signaling for one host and one viewer per room, SDP/ICE relay, and late-join replay.
- Native macOS window/application/display capture; audio requires macOS 13+.
- Continuous H.264/Opus publication independent of GUI diagnostic refresh after negotiation.
- Desktop viewer video/audio playback, muted startup, explicit sound controls, and lifecycle cleanup.
- RTCP PLI/FIR keyframe recovery with coalescing, a 500 ms cooldown, and host counters.
- Source selection, stop/reset/reconnect, session diagnostics, and local preferences.
- Custom STUN/TURN configuration. Services and deployment defaults are not bundled.

The local GUI demo only simulates session state. Debug media bursts contain non-playable payloads. CLI WebRTC commands are short negotiation/packetization checks that exit after connection; they are not streaming players. Legacy CLI `sender`/`receiver` commands transfer mock packets over UDP separately from WebRTC.

## Run The Desktop Prototype

Requirements: Rust, Tauri CLI/platform prerequisites, CMake and a C compiler for vendored Opus, and macOS 12.3+ for capture (13+ for audio). See [installation](docs/INSTALLATION.md).

Start signaling from the repository root:

```bash
cargo run -p signaling-server -- 127.0.0.1:7000
```

Launch the desktop app:

```bash
cd apps/desktop/src-tauri
cargo tauri dev
```

Run two desktop instances with the same room and signaling address. On the host, grant Screen Recording access, refresh/select a native source, and click **Start Host Prototype**. On the viewer, click **Join Viewer**; playback starts muted. Click **Enable Sound** for audio. See the [user guide](docs/USER_GUIDE.md) and [viewer guide](docs/VIEWER_PLAYBACK.md).

## Backend Smoke Test

With signaling running, start these in separate terminals:

```bash
cargo run -p p2p-cli -- webrtc-host --room demo --signal 127.0.0.1:7000 --push-debug-capture
cargo run -p p2p-cli -- webrtc-viewer --room demo --signal 127.0.0.1:7000
```

These commands verify negotiation and optionally inject a debug burst. For decoded video/audio checks, use the [synthetic browser playback probe](docs/VIEWER_PLAYBACK.md#verification).

## Checks

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all --all-features
cargo test -p app-core --example playback_probe --all-features
node --test apps/desktop/tests/viewer-player.test.mjs
```

The Tauri crate is a separate workspace; check it explicitly:

```bash
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features
```

Local sockets and macOS codec services must be available for media integration tests. Root CI runs Rust and viewer lifecycle checks on Linux/macOS, plus native sample extraction tests on macOS. It does not build/package Tauri or run the GUI/browser probe.

[AGENTS.md](AGENTS.md) defines the complete per-change gate, available native/browser probes, English-only project content, and final `cargo clean` for both workspaces after all checks.

The current version is `0.1.0`, an untagged development baseline. [TODO](todo.md#ordered-feature-milestones) groups remaining features into verifiable milestones. Each completed task receives a commit; annotated `v0.x.y` tags require the [full-project QA gate](docs/QA_CHECKLIST.md#full-project-qa-and-version-gate). Run that gate at each milestone and at most every five completed features. Minor versions add features, patch versions deliver fixes, and the major stays zero until MVP/release readiness. See the [version policy](AGENTS.md#commits-versions-and-git-tags).

## Repository And Documentation

| Path | Responsibility |
| --- | --- |
| `crates/app-core` | Protocol, signaling client, sessions, preferences, capture/encode/publish worker |
| `crates/capture-core` | Source, permission, raw media, and runtime contracts |
| `crates/capture-macos` | Native ScreenCaptureKit capture |
| `crates/capture-linux` | X11 catalog discovery and runtime scaffold; no media capture |
| `crates/media-codec` | macOS VideoToolbox H.264 and vendored Opus |
| `crates/transport-webrtc` | Rust host/diagnostic viewer peers, RTP publication, RTCP, stats |
| `crates/signaling-server` | TCP room pairing and signaling relay |
| `apps/cli` | Negotiation checks and legacy mock UDP flows |
| `apps/desktop` | Tauri shell and WebView player |

- [Agent instructions](AGENTS.md): Rust style, safe FFI, feature order, tests, documentation, and artifact cleanup.
- [TODO](todo.md): fully completed work, incomplete work, and next steps.
- [Bug register](docs/BUGS.md): observed failures, feature ownership, priorities, and closure evidence; actionable bugs precede new features.
- [Specification](docs/SPECIFICATION.md): product requirements and current scope.
- [Architecture](docs/ARCHITECTURE.md): components and data flow.
- [Desktop UI](docs/GUI_TAURI.md): current shell and backend responsibilities.
- [Installation](docs/INSTALLATION.md) and [user guide](docs/USER_GUIDE.md): setup and operation.
- [macOS capture](docs/MACOS_CAPTURE.md), [media pipeline](docs/MEDIA_PIPELINE.md), and [viewer playback](docs/VIEWER_PLAYBACK.md): implementation and validation.
- [Networking](docs/NETWORKING.md): current TCP signaling, ICE configuration, and deployment gaps.
- [Manual QA](docs/QA_CHECKLIST.md) and [local demo check](docs/DEMO_MANUAL_TEST.md): checks still to perform.
