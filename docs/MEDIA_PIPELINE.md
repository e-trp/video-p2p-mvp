# Media Pipeline

## H.264 Encoder

`media-codec` separates raw capture payloads from encoded media. On macOS, `H264Encoder` uses VideoToolbox to produce `EncodedVideoFrame` access units in H.264 Annex B format. No FFmpeg binary is required. Linux currently reports an explicit unsupported-video-encoder error.

The initial encoder targets H.264 Baseline Level 3.1: up to 1280x720 at 1-30 fps. The default is 30 fps and 2 Mbit/s. Frame reordering is disabled, realtime mode is enabled, and a keyframe is requested at least every two seconds. Odd dimensions are padded by copying their final row/column. Source size changes recreate the session and force a keyframe.

Input must be packed BGRA with exact byte counts and strictly increasing CoreMedia presentation timestamps. Encoded frames preserve PTS and expose sample duration, padded dimensions, and keyframe status. VideoToolbox AVCC output is converted to Annex B; SPS/PPS are prepended to each keyframe for RTP delivery and late recovery. Native callbacks retain no Rust pointers; encode completes callbacks before returning and encoder drop invalidates/releases the session.

The macOS host uses this encoder in the continuous capture worker. Viewer decoding/playback is still pending.

## Opus Encoder

`OpusEncoder` uses vendored libopus through the Rust `opus` binding, so no system libopus installation is needed. Building requires CMake and a C compiler. It accepts finite mono/stereo interleaved 48 kHz PCM and produces raw Opus packets without Ogg headers. Default stereo bitrate is 64 kbit/s; the initial supported range is 16–128 kbit/s.

Native capture buffers can have arbitrary frame counts. The encoder accumulates PCM until it has 960 frames per channel, then emits one 20 ms packet. Less than one packet remains buffered after each call. Input is clipped to [-1, 1]; mismatched channels/sample counts and non-finite samples are rejected. A PTS gap/overlap above one millisecond discards partial PCM and resets codec history instead of mixing audio across the discontinuity. `discontinuity_count` records these resets.

## Continuous Publication

`app-core` wraps the selected platform runtime in a capture/encode/publish worker. The worker starts capture asynchronously relative to the GUI, drains native events roughly every 5 ms, validates the selected source and audio opt-in, and encodes only while the peer is connected. Native queues retain the newest video frame and at most 32 audio buffers. GUI diagnostics retain only the latest raw frame/buffer; changing desktop refresh cadence does not pace media publication after negotiation.

`transport-webrtc` registers H.264 Baseline Level 3.1 at 90 kHz with packetization mode 1 and Opus at 48 kHz/stereo. `TransportPublisher` accepts encoded access units/packets and writes them to the matching tracks with a 200 ms timeout. Raw native buffers are never used as codec output. The explicit debug burst remains a non-playable packetization test; it does not increment encoded counters.

Every observed connection epoch resets codec state and requests an initial video keyframe. Stop, source/audio changes, reset, session replacement, and manager drop stop and join the worker before closing/dropping its WebRTC runtime. Permission-blocked/failed capture exits the worker while preserving its diagnostic status. Codec/transport failures stop publication and report a capture error; reconnect/start can rebuild the pipeline.

Host snapshots expose `encoded_video_frame_count` and `encoded_audio_packet_count`. Viewer track readers count received RTP packets separately and currently discard their payloads. Video fragmentation means RTP packet counts differ from encoded frame counts. RTCP is drained on host senders so transport interceptors continue processing feedback. Received RTP is evidence of media delivery, not decoded playback.

Remaining work: viewer depacketization/decoding/rendering and audio output, shared A/V timing, adaptive bitrate and pacing, feedback-driven keyframe requests, and a Linux video encoder. Current fixed defaults and bounded queues are initial backpressure measures. ScreenCaptureKit permission and actual source/audio behavior still require manual desktop QA.

## Validation

```bash
cargo test -p media-codec
cargo test -p app-core continuous_encoded_media
cargo clippy --all-targets --all-features -- -D warnings
```

macOS tests encode synthetic frames through the real system codec and verify SPS/PPS/IDR output, compression, frame durations, source resize, and malformed input. They do not request screen-recording access. VideoToolbox requires access to macOS codec services; sandboxed test environments may need to allow those services. Linux tests verify explicit unsupported behavior until a video backend is provided.

Opus tests perform real encode/decode roundtrips and verify output frame counts, packet timestamps, capture gaps, and input validation on both platforms.

The macOS continuous-worker test uses synthetic capture, actual H.264/Opus encoders, local SDP/ICE negotiation, and real RTP reception on a viewer. It verifies publication without GUI polling, stop/join, restart, and audio opt-out. Local UDP sockets and macOS codec services must be allowed by the test environment. It opens no screen capture and contacts no signaling/STUN/TURN service. Unit tests also cover stale source rejection, permission status preservation, and malformed/disconnected encoded publication.

References: [Apple VideoToolbox](https://developer.apple.com/documentation/videotoolbox), [realtime compression](https://developer.apple.com/documentation/videotoolbox/kvtcompressionpropertykey_realtime), and [frame reordering](https://developer.apple.com/documentation/videotoolbox/kvtcompressionpropertykey_allowframereordering).

Opus reference: [encoder API](https://www.opus-codec.org/docs/opus_api-1.6/group__opus__encoder.html).
