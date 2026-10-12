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
- An occupied role receives `ERROR role already occupied in this room` and its connection ends before relay; the accepted participant and replay history stay intact.
- Validated `SIG|SDP|...` and `SIG|ICE|...` envelopes are relayed and stored for late-join replay.
- If the initial waiting/peer announcement or replay write fails, the joining participant's slot is released. The opposite participant and its history remain; a later client can retry the released role.
- Disconnect clears that role's stored signals; a room is removed when both slots are empty.
- Server listen address comes from its command argument. Client address and ICE settings are independent desktop/CLI configuration.

There is no TLS/HTTPS/WSS signaling, authentication, invite authorization, room TTL, rate limit, or bounded server replay/message history. Protocol parsing and two-slot cleanup are not a production room-management implementation. If one side replaces an established peer, rebuild both peers when necessary to avoid stale SDP/DTLS state.

Start a server for local testing:

```bash
cargo run -p signaling-server -- 127.0.0.1:7000
```

Run the focused registration/relay tests with `cargo test -p signaling-server`. They cover both roles, rejected-client isolation over loopback TCP, normal relay/disconnect, initial announcement/replay write failures, role reuse, and late-join history. See the [BUG-003 verification record](qa/BUG-003-registration-lifecycle.md). This does not establish authenticated access or production resource bounds.

Run `cargo test -p app-core --test host_viewer_flow` for session negotiation and late-viewer replay. Before joining the viewer, the test waits for both the local offer and the server's `WAITING` acknowledgement: local offer creation does not establish that an asynchronous TCP reply has arrived. A test-only registration gate also covers an offer created before the server responds, without fixed sleeps or larger timeouts. See [BUG-010 verification](qa/BUG-010-late-viewer-ordering.md); Ubuntu Actions acceptance remains pending.

For different machines, use an address reachable by both clients and configure network access to the chosen TCP port. Loopback addresses only work on the server's own machine. Real cross-network streaming remains unvalidated.

Client polling distinguishes a temporarily idle socket from a server that has closed its output. Complete final messages are delivered once before a closure error on the next poll; final server rejection reasons are preserved. EOF with an unfinished line reports an incomplete message, and further sends fail after observed EOF. The session releases failed signaling once and shows `signaling_unavailable` with reconnect guidance, while retaining the existing transport/player. Host detection uses Refresh; desktop viewer detection uses its independent signaling pump. Reconnect creates a new connection and viewer generation. Run `cargo test -p app-core signaling::tests` and `cargo test -p app-core graceful_signaling_close` for the [EOF/recovery regressions](qa/BUG-004-signaling-eof.md).

## ICE Configuration And Diagnostics

Both Rust and WebView peers accept configured STUN/TURN entries. Supported schemes are `stun:`, `stuns:`, `turn:`, and `turns:`. Use one entry per desktop text line or repeat CLI `--ice-server` flags:

```text
stun:stun.example.com:3478
turn:turn.example.com:3478?transport=udp|username|credential
```

These are syntax examples; replace them with deployed servers. Multiple URLs may share one entry when comma-separated and using the same credentials.

Native peers advertise numeric ICE candidates and disable remote mDNS queries. The browser answerer checks those candidates, allowing the native host to learn its authenticated peer-reflexive address. This prevents a late, differently routed mDNS result from stranding nomination on the earlier address in the current ICE library. No loopback-only gathering, SDP address rewriting, or relaxed STUN/DTLS verification is used. This path is covered by a native mDNS-style answer regression and real Tauri/WKWebView/Chrome playback; cross-network STUN/TURN acceptance remains open. See the [GUI verification record](qa/gui-client-0.1.0-6bd1a31.md) and [RFC 8445 peer-reflexive discovery](https://www.rfc-editor.org/rfc/rfc8445.html#section-7.3.1.3).

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
