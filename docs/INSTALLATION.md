# Installation And Troubleshooting

## Scope

This project is still an MVP.

The current repository can already provide:

- a TCP signaling server
- CLI smoke-test flows
- a Tauri desktop shell for host/viewer session control
- live WebRTC negotiation between host and viewer
- configurable STUN/TURN ICE server settings for CLI and desktop runs
- debug `capture-core` media bursts for transport validation

It does **not** yet provide:

- real OS screen capture
- real system audio capture
- STUN/TURN deployment defaults
- production packaging/signing/notarization flow

## Prerequisites

You need:

- a Rust toolchain with `cargo`
- Tauri CLI available as `cargo tauri`
- platform prerequisites required by Tauri on your OS

Quick checks:

```bash
cargo --version
cargo tauri --help
```

If `cargo tauri` is missing, install the Tauri CLI before trying to run the desktop shell.

## Repository Setup

Clone the repository and enter it:

```bash
git clone https://github.com/e-trp/video-p2p-mvp.git
cd video-p2p-mvp
```

Optional backend sanity check:

```bash
cargo check
```

## Run The Signaling Server

Open one terminal in the repository root:

```bash
cargo run -p signaling-server -- 127.0.0.1:7000
```

Leave it running while you test the CLI or desktop shell.

## Run The Desktop Shell

Open another terminal:

```bash
cd apps/desktop/src-tauri
cargo tauri dev
```

This is the supported dev path for the desktop shell in the current repo.

## Build A Release Bundle

From the repository root:

```bash
./scripts/build-desktop-release.sh
```

This currently:

- regenerates desktop icon assets
- changes into `apps/desktop/src-tauri`
- runs `cargo tauri build`

## Saved Session Preferences

The desktop shell persists room, signaling address, configured ICE servers, and selected source metadata into a local preferences file.

Default locations:

- macOS: `~/Library/Application Support/video-p2p-mvp/session.conf`
- Linux with `XDG_CONFIG_HOME`: `$XDG_CONFIG_HOME/video-p2p-mvp/session.conf`
- Linux fallback: `~/.config/video-p2p-mvp/session.conf`

Override location for testing:

```bash
export VIDEO_P2P_MVP_CONFIG_DIR=/tmp/video-p2p-mvp-config
```

The app writes `session.conf` inside that directory.

ICE server entries are stored as newline-separated values in one of these forms:

- `stun:stun.l.google.com:19302`
- `turn:turn.example.com:3478?transport=udp|username|credential`

## STUN/TURN Configuration

The current MVP does not ship a TURN service, but both CLI and Tauri can now pass explicit ICE server entries through to the underlying `RTCPeerConnection`.

CLI examples:

```bash
cargo run -p p2p-cli -- webrtc-host --room demo --signal 127.0.0.1:7000 \
  --ice-server 'stun:stun.l.google.com:19302'
```

```bash
cargo run -p p2p-cli -- webrtc-host --room demo --signal 127.0.0.1:7000 \
  --ice-server 'turn:turn.example.com:3478?transport=udp|username|credential'
```

In the Tauri desktop shell, enter one ICE server per line in the `ICE Servers` field, then save or prepare the session.

## Troubleshooting

### `cargo tauri` is not found

Symptoms:

- `cargo tauri --help` fails
- `cargo tauri dev` fails immediately

What to check:

- Rust is installed and working
- the Tauri CLI is installed for the active toolchain
- you are using the same shell environment where the CLI was installed

### The desktop app starts in preview mode or commands do nothing

Symptoms:

- the static frontend renders
- no Rust-side session actions happen
- UI does not reflect real session state

Cause:

- the HTML was opened directly instead of starting the Tauri runtime

Fix:

- run `cargo tauri dev` from `apps/desktop/src-tauri`

### The viewer or host does not connect

What to check:

- `signaling-server` is running
- both peers use the same room name
- both peers use the same signaling address
- the signaling address is reachable from the desktop shell or CLI process
- if you are testing across networks, the STUN/TURN entries are valid and reachable

Start from the known local default first:

```bash
127.0.0.1:7000
```

For NAT-traversal tests, start with a known-good STUN entry before assuming TURN is required:

```bash
stun:stun.l.google.com:19302
```

### The desktop shell shows sources, but no real media is streamed

This is expected in the current MVP.

Today the repo supports:

- live signaling
- live WebRTC negotiation
- attached placeholder tracks
- debug `capture-core` payload publishing

It does not yet support real OS-captured audio/video delivery into those tracks.

### macOS source listing falls back or permission looks blocked

Current behavior:

- runtime application/window enumeration is attempted first
- if that fails, the app falls back to blueprint metadata

What to check:

- screen recording permission for the app/terminal session
- whether the session log reports `required` or `denied`

Important limitation:

- even successful runtime enumeration is still metadata-only today; it does not mean live capture is implemented

### Linux source listing is incomplete or unavailable

Current behavior:

- runtime enumeration currently depends on `wmctrl`-style X11 metadata when available
- unsupported environments fall back to blueprint metadata

Expected limitation:

- Wayland portal + PipeWire capture is still not implemented

### The saved config seems wrong or stale

Reset options:

- delete the persisted `session.conf`
- point `VIDEO_P2P_MVP_CONFIG_DIR` to a clean directory for a test run
- use the in-app reset flow to return the live session state to idle

### Release bundling fails

Check these first:

- `cargo tauri --help` works
- you are running `./scripts/build-desktop-release.sh` from the repository root
- Tauri platform prerequisites are installed on the current machine

Current limitation:

- signing/notarization details are still not fully documented or automated in this repo
