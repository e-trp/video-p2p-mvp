# Media Pipeline

## H.264 Encoder

`media-codec` separates raw capture payloads from encoded media. On macOS, `H264Encoder` uses VideoToolbox to produce `EncodedVideoFrame` access units in H.264 Annex B format. No FFmpeg binary is required. Linux currently reports an explicit unsupported-video-encoder error.

The initial encoder targets H.264 Baseline Level 3.1: up to 1280x720 at 1-30 fps. The default is 30 fps and 2 Mbit/s. Frame reordering is disabled, realtime mode is enabled, and a keyframe is requested at least every two seconds. Odd dimensions are padded by copying their final row/column. Source size changes recreate the session and force a keyframe.

Input must be packed BGRA with exact byte counts and strictly increasing CoreMedia presentation timestamps. Encoded frames preserve PTS and expose sample duration, padded dimensions, and keyframe status. VideoToolbox AVCC output is converted to Annex B; SPS/PPS are prepended to each keyframe for RTP delivery and late recovery. Native callbacks retain no Rust pointers; encode completes callbacks before returning and encoder drop invalidates/releases the session.

The encoder is currently standalone. Native session diagnostics remain separate from WebRTC debug publication until continuous media processing and codec-aware transport publication are integrated. Opus and viewer decoding/playback are still pending.

## Validation

```bash
cargo test -p media-codec
cargo clippy --all-targets --all-features -- -D warnings
```

macOS tests encode synthetic frames through the real system codec and verify SPS/PPS/IDR output, compression, frame durations, source resize, and malformed input. They do not request screen-recording access. VideoToolbox requires access to macOS codec services; sandboxed test environments may need to allow those services. Linux tests verify explicit unsupported behavior until a video backend is provided.

References: [Apple VideoToolbox](https://developer.apple.com/documentation/videotoolbox), [realtime compression](https://developer.apple.com/documentation/videotoolbox/kvtcompressionpropertykey_realtime), and [frame reordering](https://developer.apple.com/documentation/videotoolbox/kvtcompressionpropertykey_allowframereordering).
