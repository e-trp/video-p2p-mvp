# Product Specification

## Product Goal

Build a Rust desktop application that streams a chosen application window, including its picture and available audio, to another user. Target platforms are macOS and Linux. Users may have dynamic IP addresses.

This document separates product requirements from the implemented prototype. [TODO](../todo.md) records completion status; [manual QA](QA_CHECKLIST.md) records validation still required. The CLI `spec` command and desktop specification panel display this file.

## Requirements

- Let the host select a window/source and whether to include audio.
- Let a viewer join the host's session and play its video/audio.
- Send media directly over WebRTC when the network permits it.
- Use signaling for SDP/ICE exchange and STUN for address discovery when configured.
- Allow TURN fallback for restrictive networks; TURN relays media only when a direct path cannot be established.
- Support start, stop, reconnect, and useful connection/capture diagnostics.
- Respect platform capture permissions.

The normal path does not require a central media server. Signaling and optional TURN remain part of the intended architecture; direct connectivity on every network is not guaranteed.

## Platform Strategy

| Platform | Intended capture path | Current implementation |
| --- | --- | --- |
| macOS | ScreenCaptureKit video and available system/application audio | Native catalog/SCStream bridge, BGRA/PCM extraction, H.264/Opus publication |
| Linux Wayland | XDG Desktop Portal ScreenCast and PipeWire | Runtime scaffold only; no portal session or media delivery |
| Linux X11 | Compatibility window/audio capture fallback | `wmctrl` window metadata only; no media capture |

macOS video requires 12.3+ and audio requires 13+. Exact source/audio behavior, protected content, permissions, and relaunch requirements still need manual app testing.

## Implemented Prototype

- TCP signaling with two roles per room, SDP/ICE relay, and late-join replay.
- Real Rust host WebRTC and a WebView-owned desktop viewer peer.
- macOS capture/encode/publish worker using H.264 and Opus; video defaults to 30 fps, up to 1280x720, and 2 Mbit/s.
- Desktop video/audio playback, muted startup, explicit sound controls, and stale-callback/stop cleanup.
- Feedback-driven H.264 refresh through RTCP PLI/FIR.
- Source selection, manual reconnect/recovery guidance, custom ICE configuration, and local preferences.
- Automated synthetic native media and WKWebView/Chromium playback verification.

The repository also retains a local state-only GUI demo, non-playable debug bursts, and legacy mock UDP CLI commands. These are test tools outside the real capture/playback path. CLI WebRTC smoke commands exit after negotiation and do not provide continuous native streaming or playback.

## Outstanding Product Work

- Validate actual macOS capture-to-desktop playback, audio routing, permissions, and packaged behavior.
- Implement Linux Portal/PipeWire capture, X11 fallback capture, video encoding, and playback validation.
- Add bitrate presets, adaptive quality, network-aware pacing/backpressure, and A/V sync/latency monitoring.
- Add invite UX, authenticated/encrypted signaling, bounded room lifecycle, and managed STUN/TURN deployment.
- Validate cross-network and weak-network behavior and measure latency/CPU.
- Complete platform release packaging, signing/notarization, and installation QA.

Implementation details live in [architecture](ARCHITECTURE.md), [media pipeline](MEDIA_PIPELINE.md), and [networking](NETWORKING.md).
