# Networking And Deployment

This project is designed for peer-to-peer media transport. The production goal is direct media between users whenever the network allows it, with signaling and NAT traversal services used only to establish or recover that path.

## Current MVP Topology

```text
host desktop/CLI <---- TCP signaling ----> signaling-server <---- TCP signaling ----> viewer desktop/CLI
       |                                                                            |
       +------------------------- WebRTC peer connection ---------------------------+
```

Implemented today:

- TCP signaling server for room join, peer announcement, SDP relay, ICE relay, and late-join signaling replay
- WebRTC `RTCPeerConnection` bootstrap with host-side placeholder audio/video tracks
- optional user-provided ICE servers in CLI and desktop UI
- ICE path diagnostics when WebRTC stats expose a selected candidate pair
- direct UDP mock media flow retained as a legacy scaffold

Not implemented today:

- bundled STUN or TURN service
- production authentication for signaling rooms
- HTTPS/WSS signaling deployment
- real OS-captured media samples
- production TURN credential issuance

## Production Topology

```text
                         +----------------------+
                         | signaling service    |
                         | auth + room control  |
                         +----------+-----------+
                                    |
                  offer/answer/ICE | metadata only
                                    |
+------------------+          +----+-----+          +------------------+
| host desktop app |<-------->| STUN/TURN|<-------->| viewer desktop app|
+------------------+  ICE     +----------+   ICE    +------------------+
        |                                                       |
        +---------------- direct WebRTC media when possible ----+
        +--------------- TURN-relayed media only if required ---+
```

Expected service responsibilities:

- `signaling service`: authenticate users, authorize room access, exchange SDP/ICE, expire rooms, and expose deployment-specific signaling addresses
- `STUN`: help peers discover public reflexive candidates
- `TURN`: relay encrypted WebRTC media only when direct connectivity fails
- `desktop app`: capture selected source, publish encoded media into WebRTC, render diagnostics and recovery controls

The normal media path should be direct between peers. TURN is a fallback, not the preferred media route.

## Signaling Deployment

The current signaling server is intentionally minimal. A production signaling deployment should add:

- TLS transport, preferably WebSocket over HTTPS for desktop compatibility
- authenticated room creation and join authorization
- short-lived room identifiers or invite tokens
- room expiry and replay-history limits
- rate limits per user/IP
- bounded SDP/ICE message sizes
- structured logs and metrics without recording SDP bodies by default
- separate environment config for listen address, public URL, allowed origins, token keys, and room TTL

The desktop runtime should not hard-code deployment defaults. It should receive a signaling URL from persisted user settings, an app config file, or a release-channel-specific installer setting.

## STUN And TURN Deployment

Recommended production behavior:

- ship with one or more configured STUN URLs for basic NAT traversal
- provide TURN URLs only for authenticated users or trusted deployments
- issue short-lived TURN credentials through the signaling/auth service
- prefer `turns:` or TURN over TLS/TCP for restrictive networks when available
- show direct-vs-relay path state in the desktop diagnostics

Current accepted ICE URL schemes:

- `stun:`
- `stuns:`
- `turn:`
- `turns:`

Current entry format:

```text
stun:stun.example.com:3478
turn:turn.example.com:3478?transport=udp|username|credential
```

Multiple URLs can be placed on one entry with commas when they share the same credentials.

## Security Assumptions

WebRTC provides DTLS/SRTP encryption for media once the peer connection is established. The application should still treat signaling as security-sensitive because signaling controls which peer receives SDP and ICE metadata.

Production assumptions:

- signaling must authenticate users before room access
- signaling must authorize both peers for the same room
- TURN credentials must be scoped and short-lived
- persisted desktop config must not store long-lived shared TURN secrets when avoidable
- logs should avoid storing credentials, SDP bodies, and full ICE candidate details unless explicitly enabled for diagnostics
- capture permission state and selected source metadata are local app state, not proof that real capture is active

## Operational Diagnostics

The desktop UI should continue exposing:

- signaling reachability
- recovery state and reconnect recommendation
- configured ICE server count and summary
- selected ICE path summary
- direct-vs-relay hint when stats identify a candidate pair
- RTT, bitrate, byte counters, and packet loss when available

Future production metrics should add:

- signaling join success/failure counts
- room expiry and reconnect rates
- TURN allocation success/failure counts
- relay usage percentage
- connection setup duration
- media publish/render latency once real capture is implemented

## Current Gaps

The current MVP is ready for local and controlled-network smoke testing. It is not yet ready for open internet deployment because it still lacks:

- authenticated signaling
- TLS/WSS signaling transport
- bundled STUN defaults
- TURN deployment and short-lived credential flow
- production room lifecycle
- real captured media and codec pipeline
