use std::error::Error;
use std::fmt::{Display, Formatter};

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
pub enum CaptureStreamStatus {
    Starting,
    Running,
    Stopped,
    PermissionRequired,
    PermissionDenied,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureStreamConfig {
    pub selection: CaptureSelection,
    pub target_fps: Option<u32>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
}

impl CaptureStreamConfig {
    pub fn source_id(&self) -> &str {
        &self.selection.source_id
    }

    pub fn includes_audio(&self) -> bool {
        self.selection.include_audio
    }
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

#[derive(Debug, Clone, PartialEq)]
pub enum CaptureStreamEvent {
    Started {
        source_id: String,
    },
    StatusChanged {
        source_id: Option<String>,
        status: CaptureStreamStatus,
        message: Option<String>,
    },
    VideoFrame {
        source_id: String,
        frame: VideoFrame,
    },
    AudioBuffer {
        source_id: String,
        buffer: AudioBuffer,
    },
    Stopped {
        source_id: Option<String>,
        reason: Option<String>,
    },
    Error {
        source_id: Option<String>,
        message: String,
    },
}

impl CaptureStreamEvent {
    pub fn source_id(&self) -> Option<&str> {
        match self {
            Self::Started { source_id }
            | Self::VideoFrame { source_id, .. }
            | Self::AudioBuffer { source_id, .. } => Some(source_id),
            Self::StatusChanged { source_id, .. }
            | Self::Stopped { source_id, .. }
            | Self::Error { source_id, .. } => source_id.as_deref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureStreamError {
    message: String,
}

impl CaptureStreamError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for CaptureStreamError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for CaptureStreamError {}

pub type CaptureStreamResult<T> = Result<T, CaptureStreamError>;

pub trait CaptureStreamRuntime {
    fn start(&mut self, config: CaptureStreamConfig) -> CaptureStreamResult<()>;
    fn poll_events(&mut self) -> CaptureStreamResult<Vec<CaptureStreamEvent>>;
    fn stop(&mut self) -> CaptureStreamResult<()>;
    fn status(&self) -> CaptureStreamStatus;
}

#[cfg(test)]
mod tests {
    use super::{
        AudioBuffer, CapturePermissionState, CaptureSelection, CaptureSource, CaptureSourceKind,
        CaptureStreamConfig, CaptureStreamError, CaptureStreamEvent, CaptureStreamResult,
        CaptureStreamRuntime, CaptureStreamStatus, VideoFrame, VideoPixelFormat,
    };

    struct RecordingRuntime {
        status: CaptureStreamStatus,
        events: Vec<CaptureStreamEvent>,
    }

    impl CaptureStreamRuntime for RecordingRuntime {
        fn start(&mut self, config: CaptureStreamConfig) -> CaptureStreamResult<()> {
            self.status = CaptureStreamStatus::Running;
            self.events.push(CaptureStreamEvent::Started {
                source_id: config.source_id().to_string(),
            });
            Ok(())
        }

        fn poll_events(&mut self) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
            Ok(std::mem::take(&mut self.events))
        }

        fn stop(&mut self) -> CaptureStreamResult<()> {
            self.status = CaptureStreamStatus::Stopped;
            Ok(())
        }

        fn status(&self) -> CaptureStreamStatus {
            self.status
        }
    }

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

    #[test]
    fn stream_config_exposes_selection_helpers() {
        let config = CaptureStreamConfig {
            selection: CaptureSelection {
                source_id: "window-1".to_string(),
                include_audio: true,
            },
            target_fps: Some(30),
            max_width: Some(1280),
            max_height: Some(720),
        };

        assert_eq!(config.source_id(), "window-1");
        assert!(config.includes_audio());
    }

    #[test]
    fn stream_events_expose_optional_source_identity() {
        let frame_event = CaptureStreamEvent::VideoFrame {
            source_id: "window-1".to_string(),
            frame: VideoFrame {
                format: VideoPixelFormat::Nv12,
                width: 2,
                height: 2,
                timestamp_micros: 10,
                bytes: vec![0; 6],
            },
        };
        let status_event = CaptureStreamEvent::StatusChanged {
            source_id: None,
            status: CaptureStreamStatus::PermissionRequired,
            message: Some("screen recording approval is required".to_string()),
        };

        assert_eq!(frame_event.source_id(), Some("window-1"));
        assert_eq!(status_event.source_id(), None);
    }

    #[test]
    fn stream_runtime_contract_drains_events_and_tracks_status() {
        let mut runtime = RecordingRuntime {
            status: CaptureStreamStatus::Stopped,
            events: Vec::new(),
        };

        runtime
            .start(CaptureStreamConfig {
                selection: CaptureSelection {
                    source_id: "window-1".to_string(),
                    include_audio: false,
                },
                target_fps: None,
                max_width: None,
                max_height: None,
            })
            .expect("start runtime");

        assert_eq!(runtime.status(), CaptureStreamStatus::Running);
        assert_eq!(runtime.poll_events().expect("poll events").len(), 1);
        assert!(runtime.poll_events().expect("poll again").is_empty());

        runtime.stop().expect("stop runtime");
        assert_eq!(runtime.status(), CaptureStreamStatus::Stopped);
    }

    #[test]
    fn stream_error_displays_message() {
        let error = CaptureStreamError::new("capture backend unavailable");

        assert_eq!(error.message(), "capture backend unavailable");
        assert_eq!(error.to_string(), "capture backend unavailable");
    }
}
