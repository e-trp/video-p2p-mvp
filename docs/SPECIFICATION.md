# Product Specification

## User Request Snapshot

The target product is a Rust application for peer-to-peer streaming of a captured application window over the network. A user should be able to stream a video player window, including picture and audio, to another user. Media should flow directly between peers rather than through a central media relay. Users may have dynamic IP addresses.

The operating systems in scope are:

- macOS
- Linux

## Constraints

- Media transport should be peer-to-peer.
- Dynamic IP addresses must be supported.
- The product should capture a chosen application window.
- Audio should travel alongside video.
- No central media server should relay the stream.

## Clarified Engineering Position

Pure peer-to-peer media is realistic, but a production system still typically needs:

- signaling for offer/answer and ICE candidate exchange
- STUN for public address discovery
- TURN as a fallback for restrictive NAT environments

That means "no intermediate server" should be interpreted as:

- no central media relay in the normal path
- a lightweight signaling service is acceptable
- TURN may still be required for difficult networks

## Platform Strategy

### macOS

- capture via `ScreenCaptureKit`
- bridge captured video/audio samples into Rust
- send via WebRTC

### Linux Wayland

- capture via `XDG Desktop Portal` and `PipeWire`
- user selects the target window through the system portal
- send via WebRTC

### Linux X11

- compatibility fallback
- window capture via X11 or `ximagesrc`
- audio via PulseAudio/PipeWire monitor source

## MVP Direction

This repository's MVP is intentionally smaller than the final product. It provides:

- a TCP signaling server
- sender and receiver CLI roles
- direct UDP transfer of mock media packets between peers
- a live WebRTC negotiation path with attached host audio/video tracks
- debug `capture-core` payload publishing for transport smoke tests
- saved specification and documentation

This MVP does **not** yet provide:

- real screen capture
- real audio capture
- real captured media flowing through WebRTC
- STUN/TURN
- codecs
- encryption

## Target Production Architecture

1. Rust application core
2. signaling service
3. WebRTC transport
4. platform capture backends
5. UI for source selection and connection lifecycle

## Functional Goals For The Next Iteration

1. Replace mock UDP media with WebRTC tracks.
2. Replace debug `capture-core` bursts with real macOS capture using ScreenCaptureKit.
3. Implement Linux Wayland capture using Portal + PipeWire.
4. Add room lifecycle and authentication to signaling.
5. Add connection diagnostics and bitrate controls.
