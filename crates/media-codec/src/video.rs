use crate::{CodecError, CodecResult, EncodedVideoFrame};
use capture_core::{VideoFrame, VideoPixelFormat};
#[cfg(target_os = "macos")]
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoEncoderSettings {
    pub frame_rate: u32,
    pub bitrate_bps: u32,
}
impl Default for VideoEncoderSettings {
    fn default() -> Self {
        Self {
            frame_rate: 30,
            bitrate_bps: 2_000_000,
        }
    }
}

pub struct H264Encoder {
    settings: VideoEncoderSettings,
    last_timestamp: Option<u64>,
    #[cfg(target_os = "macos")]
    native: Option<ffi::NativeEncoder>,
}

fn validate_frame(frame: &VideoFrame) -> CodecResult<()> {
    if frame.format != VideoPixelFormat::Bgra8
        || frame.width == 0
        || frame.height == 0
        || frame.width > 1280
        || frame.height > 720
        || frame.timestamp_micros > i64::MAX as u64
        || frame.bytes.len() != frame.width as usize * frame.height as usize * 4
    {
        return Err(CodecError("H.264 input must be packed BGRA within 1280x720, with a valid timestamp and exact byte count".to_string()));
    }
    Ok(())
}

impl H264Encoder {
    pub fn new(settings: VideoEncoderSettings) -> CodecResult<Self> {
        if !(1..=30).contains(&settings.frame_rate)
            || !(64_000..=10_000_000).contains(&settings.bitrate_bps)
        {
            return Err(CodecError(
                "H.264 settings require 1-30 fps and 64 kbit/s-10 Mbit/s".to_string(),
            ));
        }
        Ok(Self {
            settings,
            last_timestamp: None,
            #[cfg(target_os = "macos")]
            native: None,
        })
    }

    /// Frames must have increasing PTS. Recreate the session when the source dimensions change.
    pub fn encode(
        &mut self,
        frame: &VideoFrame,
        force_keyframe: bool,
    ) -> CodecResult<Option<EncodedVideoFrame>> {
        validate_frame(frame)?;
        if self
            .last_timestamp
            .is_some_and(|previous| frame.timestamp_micros <= previous)
        {
            return Err(CodecError(
                "H.264 frame timestamps must increase".to_string(),
            ));
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (force_keyframe, self.settings);
            Err(CodecError(
                "native H.264 encoding is currently implemented only on macOS".to_string(),
            ))
        }
        #[cfg(target_os = "macos")]
        {
            let dimensions = ((frame.width + 1) & !1, (frame.height + 1) & !1);
            let changed = self
                .native
                .as_ref()
                .is_none_or(|encoder| encoder.dimensions != dimensions);
            if changed {
                self.native = Some(ffi::NativeEncoder::new(dimensions, self.settings)?);
            }
            let duration = self
                .last_timestamp
                .map(|previous| Duration::from_micros(frame.timestamp_micros - previous))
                .unwrap_or_else(|| {
                    Duration::from_micros(1_000_000 / u64::from(self.settings.frame_rate))
                });
            let encoded = self.native.as_mut().expect("encoder initialized").encode(
                frame,
                force_keyframe || changed,
                duration,
            )?;
            self.last_timestamp = Some(frame.timestamp_micros);
            Ok(encoded)
        }
    }
}

#[cfg(target_os = "macos")]
mod ffi {
    use super::*;
    use std::{
        ffi::{CStr, c_char, c_void},
        ptr::NonNull,
    };
    type OutputCallback = unsafe extern "C" fn(*mut c_void, *const u8, usize, bool);
    unsafe extern "C" {
        fn vpp_h264_create(
            width: u32,
            height: u32,
            fps: u32,
            bitrate: u32,
            error: *mut c_char,
            size: usize,
        ) -> *mut c_void;
        fn vpp_h264_encode(
            handle: *mut c_void,
            bytes: *const u8,
            width: u32,
            height: u32,
            pts: u64,
            force_keyframe: bool,
            callback: OutputCallback,
            context: *mut c_void,
            error: *mut c_char,
            size: usize,
        ) -> i32;
        fn vpp_h264_destroy(handle: *mut c_void);
    }
    pub(super) struct NativeEncoder {
        handle: NonNull<c_void>,
        pub dimensions: (u32, u32),
    }
    // A unique encoder is accessed sequentially; VideoToolbox callbacks stay inside the native bridge.
    unsafe impl Send for NativeEncoder {}

    fn failure(error: &[c_char; 512]) -> CodecError {
        // SAFETY: native error writers always NUL-terminate this nonempty buffer.
        CodecError(
            unsafe { CStr::from_ptr(error.as_ptr()) }
                .to_string_lossy()
                .into_owned(),
        )
    }
    impl NativeEncoder {
        pub fn new(dimensions: (u32, u32), settings: VideoEncoderSettings) -> CodecResult<Self> {
            let mut error = [0 as c_char; 512];
            // SAFETY: scalar validated settings and a live fixed-size output buffer.
            let handle = unsafe {
                vpp_h264_create(
                    dimensions.0,
                    dimensions.1,
                    settings.frame_rate,
                    settings.bitrate_bps,
                    error.as_mut_ptr(),
                    error.len(),
                )
            };
            Ok(Self {
                handle: NonNull::new(handle).ok_or_else(|| failure(&error))?,
                dimensions,
            })
        }
        pub fn encode(
            &mut self,
            frame: &VideoFrame,
            force: bool,
            duration: Duration,
        ) -> CodecResult<Option<EncodedVideoFrame>> {
            let mut output = None::<(Vec<u8>, bool)>;
            let mut error = [0 as c_char; 512];
            // SAFETY: native encode copies the input, completes callbacks before returning, and never retains Rust pointers.
            let status = unsafe {
                vpp_h264_encode(
                    self.handle.as_ptr(),
                    frame.bytes.as_ptr(),
                    frame.width,
                    frame.height,
                    frame.timestamp_micros,
                    force,
                    receive_output,
                    (&mut output as *mut Option<(Vec<u8>, bool)>).cast(),
                    error.as_mut_ptr(),
                    error.len(),
                )
            };
            if status != 0 {
                return Err(failure(&error));
            }
            Ok(output.map(|(bytes, keyframe)| EncodedVideoFrame {
                bytes,
                width: self.dimensions.0,
                height: self.dimensions.1,
                timestamp_micros: frame.timestamp_micros,
                duration,
                keyframe,
            }))
        }
    }
    impl Drop for NativeEncoder {
        fn drop(&mut self) {
            // SAFETY: destroy consumes the unique native session and completes/invalidate callbacks.
            unsafe { vpp_h264_destroy(self.handle.as_ptr()) };
        }
    }
    unsafe extern "C" fn receive_output(
        context: *mut c_void,
        data: *const u8,
        length: usize,
        keyframe: bool,
    ) {
        // SAFETY: callback is synchronous, data is a live native NSData, context is an exclusive stack borrow.
        let (output, data) = unsafe {
            (
                &mut *context.cast::<Option<(Vec<u8>, bool)>>(),
                std::slice::from_raw_parts(data, length),
            )
        };
        *output = Some((data.to_vec(), keyframe));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(width: u32, height: u32, pts: u64) -> VideoFrame {
        VideoFrame {
            format: VideoPixelFormat::Bgra8,
            width,
            height,
            timestamp_micros: pts,
            bytes: [32, 64, 192, 255].repeat((width * height) as usize),
        }
    }
    #[test]
    fn invalid_raw_frames_are_rejected_before_encoding() {
        let mut encoder = H264Encoder::new(VideoEncoderSettings::default()).unwrap();
        let mut invalid = frame(64, 36, 1);
        invalid.bytes.pop();
        assert!(
            encoder
                .encode(&invalid, false)
                .unwrap_err()
                .to_string()
                .contains("exact byte count")
        );
        invalid = frame(1, 1, u64::MAX);
        assert!(encoder.encode(&invalid, false).is_err());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn actual_h264_encoder_outputs_parameter_sets_idr_and_increasing_pts() {
        let mut encoder = H264Encoder::new(VideoEncoderSettings::default()).unwrap();
        let encoded = encoder
            .encode(&frame(63, 35, 1_000_000), true)
            .unwrap()
            .unwrap();
        assert!(encoded.keyframe);
        assert_eq!((encoded.width, encoded.height), (64, 36));
        let nal_types: Vec<u8> = encoded
            .bytes
            .windows(5)
            .filter(|bytes| bytes[..4] == [0, 0, 0, 1])
            .map(|bytes| bytes[4] & 0x1f)
            .collect();
        assert!(
            nal_types.contains(&7) && nal_types.contains(&8) && nal_types.contains(&5),
            "{nal_types:?}"
        );
        let sps = encoded
            .bytes
            .windows(8)
            .find(|bytes| bytes[..4] == [0, 0, 0, 1] && bytes[4] & 0x1f == 7)
            .unwrap();
        assert_eq!(sps[5], 66); // Baseline profile
        assert_ne!(sps[6] & 0x40, 0); // constraint_set1: Constrained Baseline, matching WebRTC SDP
        assert!(sps[7] <= 31); // AutoLevel fits the SDP ceiling under the enforced 720p/30fps limits.
        assert!(encoded.bytes.len() < 63 * 35 * 4);
        let next = encoder
            .encode(&frame(63, 35, 1_033_333), false)
            .unwrap()
            .unwrap();
        assert!(!next.keyframe);
        assert_eq!(next.duration, Duration::from_micros(33_333));
        assert!(encoder.encode(&frame(63, 35, 1_033_333), false).is_err());
        let resized = encoder
            .encode(&frame(128, 72, 1_066_666), false)
            .unwrap()
            .unwrap();
        assert!(resized.keyframe);
    }
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn unsupported_backend_is_explicit() {
        let mut encoder = H264Encoder::new(VideoEncoderSettings::default()).unwrap();
        assert!(
            encoder
                .encode(&frame(64, 36, 1), false)
                .unwrap_err()
                .to_string()
                .contains("only on macOS")
        );
    }
}
