//! ScreenCaptureKit C ABI. Native queues never retain Rust callbacks or pointers.
#[cfg(target_os = "macos")]
use super::{MacNativeCaptureBridge, MacNativeStreamSettings};
use capture_core::{
    AudioBuffer, CaptureSourceKind, CaptureStreamError, CaptureStreamEvent, CaptureStreamResult,
    VideoFrame, VideoPixelFormat,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NativeTarget {
    kind: u32,
    identifier: u32,
    display: u32,
}

impl NativeTarget {
    fn parse(id: &str, kind: CaptureSourceKind) -> CaptureStreamResult<Self> {
        let (prefix, code) = match kind {
            CaptureSourceKind::Window => ("macos-window-", 0),
            CaptureSourceKind::Application => ("macos-application-", 1),
            CaptureSourceKind::Display => ("macos-display-", 2),
        };
        let invalid = || {
            CaptureStreamError::new(format!(
                "not a native ScreenCaptureKit source: {id}; refresh the source catalog after granting Screen Recording permission"
            ))
        };
        let mut fields = id.strip_prefix(prefix).ok_or_else(invalid)?.split('-');
        let identifier = fields
            .next()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|id| *id > 0)
            .ok_or_else(invalid)?;
        let display = if code == 1 {
            fields
                .next()
                .and_then(|value| value.parse::<u32>().ok())
                .filter(|id| *id > 0)
                .ok_or_else(invalid)?
        } else {
            0
        };
        if fields.next().is_some() {
            return Err(invalid());
        }
        Ok(Self {
            kind: code,
            identifier,
            display,
        })
    }
}

fn decode_event(
    source_id: &str,
    kind: u32,
    data: &[u8],
    a: u32,
    b: u32,
    c: u32,
    timestamp: u64,
) -> CaptureStreamEvent {
    let invalid = || CaptureStreamEvent::Error {
        source_id: Some(source_id.to_string()),
        message: "ScreenCaptureKit returned an invalid native sample".to_string(),
    };
    match kind {
        0 => CaptureStreamEvent::Started {
            source_id: source_id.to_string(),
        },
        1 if a > 0
            && b > 0
            && a <= 4096
            && b <= 4096
            && data.len() == a as usize * b as usize * 4 =>
        {
            CaptureStreamEvent::VideoFrame {
                source_id: source_id.to_string(),
                frame: VideoFrame {
                    format: VideoPixelFormat::Bgra8,
                    width: a,
                    height: b,
                    timestamp_micros: timestamp,
                    bytes: data.to_vec(),
                },
            }
        }
        2 if a == 48000
            && (1..=2).contains(&b)
            && (1..=48000).contains(&c)
            && data.len() == b as usize * c as usize * 4 =>
        {
            let samples: Vec<f32> = data
                .as_chunks::<4>()
                .0
                .iter()
                .map(|bytes| f32::from_le_bytes(*bytes))
                .collect();
            if samples.iter().any(|sample| !sample.is_finite()) {
                return invalid();
            }
            CaptureStreamEvent::AudioBuffer {
                source_id: source_id.to_string(),
                buffer: AudioBuffer {
                    sample_rate_hz: a,
                    channels: b as u16,
                    frames: c,
                    timestamp_micros: timestamp,
                    samples,
                },
            }
        }
        3 => CaptureStreamEvent::Error {
            source_id: Some(source_id.to_string()),
            message: String::from_utf8_lossy(data).into_owned(),
        },
        _ => invalid(),
    }
}

#[cfg(target_os = "macos")]
mod ffi {
    use super::*;
    use capture_core::{CaptureSource, CaptureStreamStatus};
    use std::{
        ffi::{CStr, c_char, c_void},
        ptr::NonNull,
    };

    type SourceCallback =
        unsafe extern "C" fn(*mut c_void, u32, *const c_char, *const c_char, *const c_char, bool);
    type EventCallback =
        unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, u32, u32, u32, u64);
    unsafe extern "C" {
        fn vpp_sck_catalog(
            callback: SourceCallback,
            context: *mut c_void,
            error: *mut c_char,
            size: usize,
        ) -> i32;
        fn vpp_sck_start(
            kind: u32,
            identifier: u32,
            display: u32,
            audio: bool,
            fps: u32,
            width: u32,
            height: u32,
        ) -> *mut c_void;
        fn vpp_sck_poll(handle: *mut c_void, callback: EventCallback, context: *mut c_void);
        fn vpp_sck_destroy(handle: *mut c_void);
    }

    #[derive(Default)]
    pub(crate) struct ScreenCaptureKitBridge {
        handle: Option<NonNull<c_void>>,
        source_id: Option<String>,
    }
    // The opaque handle owns a serial dispatch queue. All C entry points synchronize with it.
    unsafe impl Send for ScreenCaptureKitBridge {}

    impl ScreenCaptureKitBridge {
        fn close(&mut self) {
            if let Some(handle) = self.handle.take() {
                // SAFETY: this is the unique retained handle returned by start; destroy consumes it once.
                unsafe { vpp_sck_destroy(handle.as_ptr()) };
            }
        }
    }

    impl Drop for ScreenCaptureKitBridge {
        fn drop(&mut self) {
            self.close();
        }
    }

    impl MacNativeCaptureBridge for ScreenCaptureKitBridge {
        fn start(
            &mut self,
            settings: &MacNativeStreamSettings,
        ) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
            self.close();
            let target = NativeTarget::parse(&settings.source_id, settings.source_kind)?;
            // SAFETY: scalar settings only. No Rust data is passed into asynchronous native work.
            self.handle = NonNull::new(unsafe {
                vpp_sck_start(
                    target.kind,
                    target.identifier,
                    target.display,
                    settings.include_audio,
                    settings.target_fps,
                    settings.max_width,
                    settings.max_height,
                )
            });
            if self.handle.is_none() {
                return Err(CaptureStreamError::new(
                    "ScreenCaptureKit allocation failed",
                ));
            }
            self.source_id = Some(settings.source_id.clone());
            Ok(vec![CaptureStreamEvent::StatusChanged { source_id: self.source_id.clone(), status: CaptureStreamStatus::Starting,
                message: Some("ScreenCaptureKit native capture is starting; raw BGRA/PCM samples require encoding before WebRTC playback".to_string()) }])
        }

        fn poll_events(&mut self) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
            let Some(handle) = self.handle else {
                return Ok(Vec::new());
            };
            let mut context = EventContext {
                source_id: self.source_id.as_deref().expect("active capture source"),
                events: Vec::new(),
            };
            // SAFETY: poll invokes callbacks synchronously and retains neither callback nor context.
            unsafe {
                vpp_sck_poll(
                    handle.as_ptr(),
                    receive_event,
                    (&mut context as *mut EventContext<'_>).cast(),
                )
            };
            let events = context.events;
            if events
                .iter()
                .any(|event| matches!(event, CaptureStreamEvent::Error { .. }))
            {
                self.close();
            }
            Ok(events)
        }

        fn stop(
            &mut self,
            source_id: Option<String>,
        ) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
            self.close();
            let active = self.source_id.take();
            Ok(vec![CaptureStreamEvent::Stopped {
                source_id: source_id.or(active),
                reason: Some("ScreenCaptureKit native stream stopped".to_string()),
            }])
        }
    }

    struct EventContext<'a> {
        source_id: &'a str,
        events: Vec<CaptureStreamEvent>,
    }

    unsafe extern "C" fn receive_event(
        context: *mut c_void,
        kind: u32,
        data: *const u8,
        length: usize,
        a: u32,
        b: u32,
        c: u32,
        timestamp: u64,
    ) {
        // SAFETY: the native poll owns data for this synchronous callback; context is its stack-local exclusive borrow.
        let context = unsafe { &mut *context.cast::<EventContext<'_>>() };
        let bytes = if length == 0 {
            &[]
        } else {
            // SAFETY: native packets own a contiguous NSData of length bytes throughout this call.
            unsafe { std::slice::from_raw_parts(data, length) }
        };
        context.events.push(decode_event(
            context.source_id,
            kind,
            bytes,
            a,
            b,
            c,
            timestamp,
        ));
    }

    unsafe extern "C" fn receive_source(
        context: *mut c_void,
        kind: u32,
        id: *const c_char,
        label: *const c_char,
        app: *const c_char,
        audio: bool,
    ) {
        let kind = match kind {
            0 => CaptureSourceKind::Window,
            1 => CaptureSourceKind::Application,
            2 => CaptureSourceKind::Display,
            _ => return,
        };
        // SAFETY: native catalog supplies non-null, NUL-terminated UTF-8 strings valid for this callback.
        let (id, label, app) = unsafe {
            (
                CStr::from_ptr(id),
                CStr::from_ptr(label),
                CStr::from_ptr(app),
            )
        };
        let app = app.to_string_lossy().into_owned();
        // SAFETY: catalog calls synchronously with the exclusive Vec borrow below.
        unsafe { &mut *context.cast::<Vec<CaptureSource>>() }.push(CaptureSource {
            id: id.to_string_lossy().into_owned(),
            kind,
            display_name: label.to_string_lossy().into_owned(),
            app_name: (!app.is_empty()).then_some(app),
            has_audio: audio,
        });
    }

    pub(crate) fn catalog() -> Result<Vec<CaptureSource>, String> {
        let mut sources = Vec::<CaptureSource>::new();
        let mut error = [0 as c_char; 1024];
        // SAFETY: output buffers/context remain alive for this synchronous call; native callbacks do not retain them.
        let status = unsafe {
            vpp_sck_catalog(
                receive_source,
                (&mut sources as *mut Vec<CaptureSource>).cast(),
                error.as_mut_ptr(),
                error.len(),
            )
        };
        if status != 0 {
            // SAFETY: native error writer always NUL-terminates a nonempty buffer.
            return Err(unsafe { CStr::from_ptr(error.as_ptr()) }
                .to_string_lossy()
                .into_owned());
        }
        Ok(sources)
    }
}

#[cfg(target_os = "macos")]
pub(super) use ffi::{ScreenCaptureKitBridge, catalog};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_targets_use_os_identity_and_display_for_applications() {
        assert_eq!(
            NativeTarget::parse("macos-window-42", CaptureSourceKind::Window)
                .unwrap()
                .identifier,
            42
        );
        assert_eq!(
            NativeTarget::parse("macos-application-123-7", CaptureSourceKind::Application).unwrap(),
            NativeTarget {
                kind: 1,
                identifier: 123,
                display: 7
            }
        );
        assert_eq!(
            NativeTarget::parse("macos-display-7", CaptureSourceKind::Display)
                .unwrap()
                .kind,
            2
        );
        for id in [
            "mac-window-vlc",
            "macos-window-0",
            "macos-window-42-extra",
            "macos-window--1",
            "macos-window-4294967296",
        ] {
            assert!(
                NativeTarget::parse(id, CaptureSourceKind::Window).is_err(),
                "{id}"
            );
        }
        assert!(
            NativeTarget::parse("macos-application-123", CaptureSourceKind::Application).is_err()
        );
    }

    #[test]
    fn native_samples_preserve_bgra_and_interleaved_pcm_timing() {
        let event = decode_event("window", 1, &[1, 2, 3, 255], 1, 1, 0, 12345);
        assert!(
            matches!(event, CaptureStreamEvent::VideoFrame { frame, .. } if frame.bytes == [1, 2, 3, 255] && frame.timestamp_micros == 12345)
        );
        let samples = [0.25_f32, -0.5, 0.75, 1.0];
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let event = decode_event("window", 2, &bytes, 48000, 2, 2, 12345);
        assert!(
            matches!(event, CaptureStreamEvent::AudioBuffer { buffer, .. } if buffer.samples == samples && buffer.frames == 2 && buffer.timestamp_micros == 12345)
        );
    }

    #[test]
    fn malformed_samples_and_nonfinite_audio_become_errors() {
        for event in [
            decode_event("window", 1, &[0; 3], 1, 1, 0, 0),
            decode_event("window", 1, &[], u32::MAX, u32::MAX, 0, 0),
            decode_event("window", 2, &[0; 4], 48000, 2, 1, 0),
            decode_event("window", 2, &f32::NAN.to_le_bytes(), 48000, 1, 1, 0),
        ] {
            assert!(matches!(event, CaptureStreamEvent::Error { .. }));
        }
    }
}
