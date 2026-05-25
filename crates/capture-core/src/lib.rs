#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapturePermissionState {
    Unknown,
    Required,
    Granted,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureSourceKind {
    Window,
    Application,
    Display,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSource {
    pub id: String,
    pub kind: CaptureSourceKind,
    pub display_name: String,
    pub app_name: Option<String>,
    pub has_audio: bool,
}

impl CaptureSource {
    pub fn label(&self) -> String {
        match self.app_name.as_deref() {
            Some(app_name) if !app_name.is_empty() => {
                format!("{app_name} - {}", self.display_name)
            }
            _ => self.display_name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSelection {
    pub source_id: String,
    pub include_audio: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoPixelFormat {
    Bgra8,
    Nv12,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoFrame {
    pub format: VideoPixelFormat,
    pub width: u32,
    pub height: u32,
    pub timestamp_micros: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioBuffer {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub frames: u32,
    pub timestamp_micros: u64,
    pub samples: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::{
        AudioBuffer, CapturePermissionState, CaptureSource, CaptureSourceKind, VideoFrame,
        VideoPixelFormat,
    };

    #[test]
    fn capture_source_label_prefers_app_name_when_present() {
        let source = CaptureSource {
            id: "window-1".to_string(),
            kind: CaptureSourceKind::Window,
            display_name: "Meeting".to_string(),
            app_name: Some("VLC".to_string()),
            has_audio: true,
        };

        assert_eq!(source.label(), "VLC - Meeting");
    }

    #[test]
    fn capture_payloads_keep_declared_shape() {
        let frame = VideoFrame {
            format: VideoPixelFormat::Bgra8,
            width: 1280,
            height: 720,
            timestamp_micros: 42,
            bytes: vec![0; 16],
        };
        let audio = AudioBuffer {
            sample_rate_hz: 48_000,
            channels: 2,
            frames: 960,
            timestamp_micros: 84,
            samples: vec![0.0; 8],
        };

        assert_eq!(frame.format, VideoPixelFormat::Bgra8);
        assert_eq!(audio.channels, 2);
        assert_eq!(
            CapturePermissionState::Granted,
            CapturePermissionState::Granted
        );
    }
}
