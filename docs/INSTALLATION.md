# Installation And Troubleshooting

## Scope And Dependencies

This is a development prototype, not a validated signed release. macOS capture/encoding and desktop viewer playback are implemented; actual capture/packaged behavior still need QA. Linux capture/video encoding are absent and Linux WebView playback is unvalidated.

Required for development:

- Rust with `cargo` (the desktop crate uses edition 2024).
- CMake and a C compiler for vendored Opus; no system libopus or FFmpeg binary is required.
- For Tauri: `cargo tauri` and platform development prerequisites, including the platform WebView.
- On macOS: Xcode Command Line Tools/macOS SDK, macOS 12.3+ for capture, 13+ for audio, and Screen Recording access for actual capture.
- Node 22+ for viewer lifecycle tests and the optional browser probe.
- Python 3 for icon regeneration; the release helper regenerates icons before building.

The repository does not install these dependencies. Check your tools before building:

```bash
cargo --version
cargo tauri --help
cmake --version
```

## Setup And Core Checks

```bash
git clone https://github.com/e-trp/video-p2p-mvp.git
cd video-p2p-mvp
cargo check --workspace --all-targets --all-features
```

The Tauri app is a separate workspace. A root check does not build it:

```bash
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets --all-features
```

Full checks and native/browser test requirements are in [QA](QA_CHECKLIST.md), [macOS capture](MACOS_CAPTURE.md), and [viewer playback](VIEWER_PLAYBACK.md).

## Development Run

Start signaling from the repository root:

```bash
cargo run -p signaling-server -- 127.0.0.1:7000
```

In another terminal:

```bash
cd apps/desktop/src-tauri
cargo tauri dev
```

For actual playback use a host desktop and a viewer desktop with the same room and server address. See [user guide](USER_GUIDE.md). CLI WebRTC commands exit after negotiation and are not persistent streaming peers.

## Release Build Helper

From the repository root:

```bash
./scripts/build-desktop-release.sh
```

The helper runs Python-based icon generation and `cargo tauri icon`, then runs `cargo tauri build` from `apps/desktop/src-tauri`. Standalone icon regeneration is available through:

```bash
./scripts/generate-desktop-icons.sh
```

Current bundle configuration targets macOS `.app` and `.dmg`, uses identifier `dev.videop2p.desktop`, and declares minimum macOS 12.3. Linux release bundle targets are not configured. Signing/notarization automation, build/install validation, and packaged playback QA remain outstanding; the existence of a helper is not evidence of a tested release bundle.

## Saved Preferences

The app stores room, signaling address, ICE entries, source selection (including audio), and Auto Refresh settings in `session.conf`. Default paths:

| Platform | Path |
| --- | --- |
| macOS | `~/Library/Application Support/video-p2p-mvp/session.conf` |
| Linux with XDG_CONFIG_HOME | `$XDG_CONFIG_HOME/video-p2p-mvp/session.conf` |
| Linux fallback | `~/.config/video-p2p-mvp/session.conf` |

For isolated tests:

```bash
export VIDEO_P2P_MVP_CONFIG_DIR=/tmp/video-p2p-mvp-config
```

`session.conf` is written inside that directory. This shared core store is also used by CLI session flows. **Reset** reloads saved preferences and does not delete the file. To test fresh defaults, use a clean override directory or remove the file while the app is stopped.

The file is plaintext and stores configured TURN credentials as supplied. It does not store active capture or peer sessions. Native source IDs can become stale after an app/system restart and may be rebound during catalog refresh.

## ICE Configuration

No STUN/TURN services or default server entries are bundled. Enter one actual server entry per line in **ICE Servers**, or repeat `--ice-server` in CLI smoke commands. Accepted schemes: `stun:`, `stuns:`, `turn:`, `turns:`.

Example syntax (placeholder servers):

```text
stun:stun.example.com:3478
turn:turn.example.com:3478?transport=udp|username|credential
```

Multiple URLs may be comma-separated within an entry sharing credentials. Invalid schemes are rejected before save/start. The **Signaling** field remains a TCP `host:port`; ICE server URLs are separate settings. See [networking](NETWORKING.md).

## Troubleshooting

### Tauri command or build is unavailable

Check that Rust, the Tauri CLI, platform prerequisites, and CMake are installed in the active shell. Run `cargo tauri --help`. Start Tauri from `apps/desktop/src-tauri`; root Cargo checks only cover the core workspace. The release helper additionally requires Python 3 and generates icons before building.

### The page shows Browser preview

Opening static HTML outside Tauri does not execute Rust commands. Run `cargo tauri dev`; the badge should show **Tauri runtime connected**. If the badge reports a command timeout, inspect the Tauri terminal and session logs.

### Peers do not connect

Check that signaling is running, both peers use the same room, and both can reach its TCP address. `127.0.0.1` refers to each machine itself; for different machines use the server's reachable address. A room supports one host and one viewer; stop old peers before reusing their roles.

Keep host Auto Refresh enabled during negotiation or use manual Refresh to process answer/ICE. Viewer signaling has an independent pump. For different networks, configure reachable STUN/TURN servers and inspect ICE diagnostics. Real cross-network/NAT coverage is still outstanding.

### No decoded picture or audible audio

Use a desktop viewer; CLI WebRTC commands are short diagnostics. Check that the host selected a native macOS source and started capture, the runtime is running, and the peer is connected. Change the window content to produce frames. Host encoded counts and viewer decoded counts should increase.

The player starts muted; use **Enable Sound** or the play control if autoplay is blocked. Audio requires macOS 13+, **Include Audio**, and audio from the selected source. Received audio sample counts are not proof of audible device output. Debug bursts are non-playable; avoid them during a playback check.

### Source catalog or permission is unavailable

macOS passive refresh preflights permission and attempts native enumeration when granted. Explicit start can request Screen Recording access. After granting it, relaunch if required, refresh/select a native source, and retry. Example fallback sources cannot start native capture. See [capture guide](MACOS_CAPTURE.md).

Linux catalog discovery uses X11 `wmctrl` metadata when available. Even a populated catalog or granted hint does not mean media capture exists; Portal/PipeWire and X11 capture remain unimplemented.

### Capture stopped after selection or reconnect

Source/audio changes and catalog rebinding stop the old capture. Reconnect rebuilds the peer and also leaves capture stopped. Click **Start Native Capture** again. If one side replaced an established peer, rebuild both sides to refresh SDP/DTLS state.

### Release bundling fails

Check Tauri platform dependencies, Python 3, CMake, and the release helper's terminal output. The current config targets macOS bundles. Signing/notarization and install/playback validation are not automated or verified in this repository. Record observed failures in the [bug register](BUGS.md) with feature ownership and link them from the [QA checklist](QA_CHECKLIST.md) evidence. For the known local artifact-cleanup ownership issue, see BUG-002 in the register.
