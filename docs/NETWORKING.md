# Networking And Deployment

## Implemented Topology

```text
Rust host <---- TCP SDP/ICE ----> signaling-server <---- TCP SDP/ICE ----> Rust viewer bridge
    |                                                                            |
    +-------------------- WebRTC media ---------------------------- WebView viewer
```

The normal media path is peer-to-peer. The desktop viewer owns its peer in the WebView; a CLI diagnostic viewer uses a Rust peer. Custom ICE servers may provide STUN discovery or TURN relay when needed. No services/default ICE entries are bundled.

The separate legacy CLI `sender`/`receiver` flow uses mock UDP packets, not WebRTC or captured media.

## Current Signaling Behavior

- Plain TCP, configured as `host:port`; default listen address is `127.0.0.1:7000`.
- One sender/host and one receiver/viewer per room, identified by a manually entered room string.
- Validated `SIG|SDP|...` and `SIG|ICE|...` envelopes are relayed and stored for late-join replay.
- Disconnect clears that role's stored signals; a room is removed when both slots are empty.
- Server listen address comes from its command argument. Client address and ICE settings are independent desktop/CLI configuration.

There is no TLS/HTTPS/WSS signaling, authentication, invite authorization, room TTL, rate limit, or bounded server replay/message history. Protocol parsing and two-slot cleanup are not a production room-management implementation. If one side replaces an established peer, rebuild both peers when necessary to avoid stale SDP/DTLS state.

Start a server for local testing:

```bash
cargo run -p signaling-server -- 127.0.0.1:7000
```

For different machines, use an address reachable by both clients and configure network access to the chosen TCP port. Loopback addresses only work on the server's own machine. Real cross-network streaming remains unvalidated.

## ICE Configuration And Diagnostics

Both Rust and WebView peers accept configured STUN/TURN entries. Supported schemes are `stun:`, `stuns:`, `turn:`, and `turns:`. Use one entry per desktop text line or repeat CLI `--ice-server` flags:

```text
stun:stun.example.com:3478
turn:turn.example.com:3478?transport=udp|username|credential
```

These are syntax examples; replace them with deployed servers. Multiple URLs may share one entry when comma-separated and using the same credentials.

Snapshots show configured server count/summary and a best-effort selected candidate-pair direct/relay hint. RTT, available bitrate, bytes, and packet loss depend on stats exposed by the Rust/WebView peer; missing values do not prove a failed connection. Current diagnostics do not measure capture-to-render latency or A/V skew.

Configuration support does not establish TURN/NAT interoperability. Direct and relay paths still need real-network QA. A signaling address is not an ICE server URL, and adding STUN does not guarantee a direct connection through every NAT.

## Security And Persistence

WebRTC media uses DTLS-SRTP. Current TCP signaling and legacy mock UDP are plaintext. Room names do not authenticate or authorize peers. `session.conf` stores user-supplied TURN credentials in plaintext along with the session settings.

## Outstanding Deployment Work

| Area | Work required |
| --- | --- |
| Signaling | TLS transport, authentication/room authorization, invite tokens, bounded messages/history, TTL/rate limits |
| STUN/TURN | Deployment defaults, reachable relay services, short-lived credential issuance, restrictive-network testing |
| Operations | Deployment configuration, logs/metrics policy, setup/reconnect success metrics and room/relay monitoring |
| Media QA | Direct/relay tests across NATs, loss/congestion recovery, latency/CPU/A/V measurements |

The intended production path uses authenticated signaling and configured STUN, with TURN carrying encrypted media only when direct connectivity fails. Linux capture/encoding and platform playback QA remain separate prerequisites. See [TODO](../todo.md) and [manual QA](QA_CHECKLIST.md).
