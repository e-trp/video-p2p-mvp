# Media Pipeline

## H.264 Encoder

`media-codec` separates raw capture payloads from encoded media. On macOS, `H264Encoder` uses VideoToolbox to produce `EncodedVideoFrame` access units in H.264 Annex B format. No FFmpeg binary is required. Linux currently reports an explicit unsupported-video-encoder error.

The initial encoder targets H.264 Baseline Level 3.1: up to 1280x720 at 1-30 fps. The default is 30 fps and 2 Mbit/s. Frame reordering is disabled, realtime mode is enabled, and a keyframe is requested at least every two seconds. Odd dimensions are padded by copying their final row/column. Source size changes recreate the session and force a keyframe.

Input must be packed BGRA with exact byte counts and strictly increasing CoreMedia presentation timestamps. Encoded frames preserve PTS and expose sample duration, padded dimensions, and keyframe status. VideoToolbox AVCC output is converted to Annex B; SPS/PPS are prepended to each keyframe for RTP delivery and late recovery. Native callbacks retain no Rust pointers; encode completes callbacks before returning and encoder drop invalidates/releases the session.

The encoders are currently standalone. Native session diagnostics remain separate from WebRTC debug publication until continuous media processing and codec-aware transport publication are integrated. Viewer decoding/playback is still pending.

## Opus Encoder

`OpusEncoder` uses vendored libopus through the Rust `opus` binding, so no system libopus installation is needed. Building requires CMake and a C compiler. It accepts finite mono/stereo interleaved 48 kHz PCM and produces raw Opus packets without Ogg headers. Default stereo bitrate is 64 kbit/s; the initial supported range is 16–128 kbit/s.

Native capture buffers can have arbitrary frame counts. The encoder accumulates PCM until it has 960 frames per channel, then emits one 20 ms packet. Less than one packet remains buffered after each call. Input is clipped to [-1, 1]; mismatched channels/sample counts and non-finite samples are rejected. A PTS gap/overlap above one millisecond discards partial PCM and resets codec history instead of mixing audio across the discontinuity. `discontinuity_count` records these resets.

## Validation

```bash
cargo test -p media-codec
cargo clippy --all-targets --all-features -- -D warnings
```

macOS tests encode synthetic frames through the real system codec and verify SPS/PPS/IDR output, compression, frame durations, source resize, and malformed input. They do not request screen-recording access. VideoToolbox requires access to macOS codec services; sandboxed test environments may need to allow those services. Linux tests verify explicit unsupported behavior until a video backend is provided.

Opus tests perform real encode/decode roundtrips and verify output frame counts, packet timestamps, capture gaps, and input validation on both platforms.

References: [Apple VideoToolbox](https://developer.apple.com/documentation/videotoolbox), [realtime compression](https://developer.apple.com/documentation/videotoolbox/kvtcompressionpropertykey_realtime), and [frame reordering](https://developer.apple.com/documentation/videotoolbox/kvtcompressionpropertykey_allowframereordering).

Opus reference: [encoder API](https://www.opus-codec.org/docs/opus_api-1.6/group__opus__encoder.html).
