//! Continuously drain capture, encode, and publish independently of GUI refresh.
use capture_core::{
    CaptureStreamConfig, CaptureStreamError, CaptureStreamEvent, CaptureStreamResult,
    CaptureStreamRuntime, CaptureStreamStatus,
};
use media_codec::{AudioEncoderSettings, H264Encoder, OpusEncoder, VideoEncoderSettings};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use transport_webrtc::TransportPublisher;

type Backend = Box<dyn CaptureStreamRuntime + Send>;
type WorkerResult = (Backend, CaptureStreamResult<()>);

struct Shared {
    status: CaptureStreamStatus,
    events: VecDeque<CaptureStreamEvent>,
}
impl Shared {
    fn push(&mut self, event: CaptureStreamEvent) {
        let media_kind = match &event {
            CaptureStreamEvent::VideoFrame { .. } => Some(true),
            CaptureStreamEvent::AudioBuffer { .. } => Some(false),
            _ => None,
        };
        if let Some(video) = media_kind {
            // UI diagnostics retain just the latest samples; encoded packets are published in the worker.
            self.events.retain(|old| {
                if video {
                    !matches!(old, CaptureStreamEvent::VideoFrame { .. })
                } else {
                    !matches!(old, CaptureStreamEvent::AudioBuffer { .. })
                }
            });
        }
        match &event {
            CaptureStreamEvent::Started { .. } => self.status = CaptureStreamStatus::Running,
            CaptureStreamEvent::StatusChanged { status, .. } => self.status = *status,
            CaptureStreamEvent::Error { .. } => self.status = CaptureStreamStatus::Failed,
            CaptureStreamEvent::Stopped { .. } => self.status = CaptureStreamStatus::Stopped,
            _ => {}
        }
        self.events.push_back(event);
        while self.events.len() > 64 {
            self.events.pop_front();
        }
    }
}

pub(crate) struct MediaPipelineRuntime {
    backend: Option<Backend>,
    publisher: TransportPublisher,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<WorkerResult>>,
    source_id: Option<String>,
}

impl MediaPipelineRuntime {
    pub fn new(backend: Backend, publisher: TransportPublisher) -> Self {
        Self {
            backend: Some(backend),
            publisher,
            shared: Arc::new(Mutex::new(Shared {
                status: CaptureStreamStatus::Stopped,
                events: VecDeque::new(),
            })),
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
            source_id: None,
        }
    }
}

struct Encoders {
    video: H264Encoder,
    audio: Option<OpusEncoder>,
    connected: bool,
    force_keyframe: bool,
}

fn process_event(
    event: &CaptureStreamEvent,
    config: &CaptureStreamConfig,
    publisher: &TransportPublisher,
    encoders: &mut Encoders,
) -> Result<(), String> {
    if event
        .source_id()
        .is_some_and(|source| source != config.source_id())
    {
        return Err("native capture worker rejected an event from a different source".to_string());
    }
    if !publisher.is_connected() {
        return Ok(());
    }
    match event {
        CaptureStreamEvent::VideoFrame { frame, .. } => {
            if let Some(encoded) = encoders
                .video
                .encode(frame, encoders.force_keyframe)
                .map_err(|error| error.to_string())?
            {
                publisher
                    .publish_video(encoded)
                    .map_err(|error| error.to_string())?;
                encoders.force_keyframe = false;
            }
        }
        CaptureStreamEvent::AudioBuffer { buffer, .. } if config.includes_audio() => {
            if encoders.audio.is_none() {
                encoders.audio = Some(
                    OpusEncoder::new(AudioEncoderSettings {
                        channels: buffer.channels,
                        ..Default::default()
                    })
                    .map_err(|error| error.to_string())?,
                );
            }
            for packet in encoders
                .audio
                .as_mut()
                .expect("audio encoder initialized")
                .encode(buffer)
                .map_err(|error| error.to_string())?
            {
                publisher
                    .publish_audio(packet)
                    .map_err(|error| error.to_string())?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn run_worker(
    mut backend: Backend,
    config: CaptureStreamConfig,
    publisher: TransportPublisher,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
) -> WorkerResult {
    let error = |message: String| {
        shared
            .lock()
            .expect("capture worker state poisoned")
            .push(CaptureStreamEvent::Error {
                source_id: Some(config.source_id().to_string()),
                message,
            });
    };
    if let Err(failure) = backend.start(config.clone()) {
        error(failure.to_string());
        let stopped = backend.stop();
        return (backend, stopped);
    }
    let mut encoders = Encoders {
        video: H264Encoder::new(VideoEncoderSettings::default())
            .expect("valid default video settings"),
        audio: None,
        connected: false,
        force_keyframe: true,
    };
    shared.lock().expect("capture worker state poisoned").status = backend.status();
    'capture: while !stop.load(Ordering::Acquire) {
        let connected = publisher.is_connected();
        if connected != encoders.connected {
            // Each connected epoch starts with an independently decodable video/audio stream.
            encoders.video = H264Encoder::new(VideoEncoderSettings::default())
                .expect("valid default video settings");
            encoders.audio = None;
            encoders.force_keyframe = true;
            encoders.connected = connected;
        }
        let events = match backend.poll_events() {
            Ok(events) => events,
            Err(failure) => {
                error(failure.to_string());
                break;
            }
        };
        for event in events {
            if stop.load(Ordering::Acquire) {
                break 'capture;
            }
            if let Err(failure) = process_event(&event, &config, &publisher, &mut encoders) {
                // A concurrent disconnect drops this sample; recovery diagnostics handle the peer state.
                if !publisher.is_connected() && event.source_id() == Some(config.source_id()) {
                    continue;
                }
                error(failure);
                break 'capture;
            }
            if matches!(event, CaptureStreamEvent::AudioBuffer { .. }) && !config.includes_audio() {
                continue;
            }
            let terminal = matches!(
                event,
                CaptureStreamEvent::Error { .. }
                    | CaptureStreamEvent::Stopped { .. }
                    | CaptureStreamEvent::StatusChanged {
                        status: CaptureStreamStatus::Failed
                            | CaptureStreamStatus::PermissionRequired
                            | CaptureStreamStatus::PermissionDenied,
                        ..
                    }
            );
            shared
                .lock()
                .expect("capture worker state poisoned")
                .push(event);
            if terminal {
                break 'capture;
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    let result = backend.stop();
    (backend, result)
}

impl CaptureStreamRuntime for MediaPipelineRuntime {
    fn start(&mut self, config: CaptureStreamConfig) -> CaptureStreamResult<()> {
        if self.worker.is_some() {
            self.stop()?;
        }
        self.source_id = Some(config.source_id().to_string());
        {
            let mut shared = self.shared.lock().expect("capture worker state poisoned");
            shared.events.clear();
            shared.status = CaptureStreamStatus::Starting;
        }
        self.stop.store(false, Ordering::Release);
        let backend = self
            .backend
            .take()
            .ok_or_else(|| CaptureStreamError::new("capture worker backend is unavailable"))?;
        let (publisher, shared, stop) = (
            self.publisher.clone(),
            self.shared.clone(),
            self.stop.clone(),
        );
        self.worker = Some(
            thread::Builder::new()
                .name("capture-encode-publish".to_string())
                .spawn(move || run_worker(backend, config, publisher, shared, stop))
                .map_err(|error| {
                    CaptureStreamError::new(format!("cannot start capture worker: {error}"))
                })?,
        );
        Ok(())
    }
    fn poll_events(&mut self) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
        Ok(std::mem::take(
            &mut self
                .shared
                .lock()
                .expect("capture worker state poisoned")
                .events,
        )
        .into_iter()
        .collect())
    }
    fn stop(&mut self) -> CaptureStreamResult<()> {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let (backend, result) = worker
                .join()
                .map_err(|_| CaptureStreamError::new("capture worker panicked"))?;
            self.backend = Some(backend);
            result?;
        }
        let mut shared = self.shared.lock().expect("capture worker state poisoned");
        shared.events.clear();
        shared.push(CaptureStreamEvent::Stopped {
            source_id: self.source_id.clone(),
            reason: Some("capture/encoding worker stopped".to_string()),
        });
        Ok(())
    }
    fn status(&self) -> CaptureStreamStatus {
        self.shared
            .lock()
            .expect("capture worker state poisoned")
            .status
    }
}
impl Drop for MediaPipelineRuntime {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use capture_core::{CaptureSelection, VideoFrame, VideoPixelFormat};
    use std::{sync::atomic::AtomicUsize, time::Instant};
    use transport_webrtc::{TransportSession, WebRtcConfig};

    struct SyntheticCapture {
        status: CaptureStreamStatus,
        stops: Arc<AtomicUsize>,
        wrong_source: bool,
    }
    impl CaptureStreamRuntime for SyntheticCapture {
        fn start(&mut self, _: CaptureStreamConfig) -> CaptureStreamResult<()> {
            self.status = CaptureStreamStatus::Running;
            Ok(())
        }
        fn poll_events(&mut self) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
            Ok(vec![CaptureStreamEvent::VideoFrame {
                source_id: if self.wrong_source { "other" } else { "source" }.to_string(),
                frame: VideoFrame {
                    format: VideoPixelFormat::Bgra8,
                    width: 64,
                    height: 36,
                    timestamp_micros: 1_000_000,
                    bytes: vec![0; 64 * 36 * 4],
                },
            }])
        }
        fn stop(&mut self) -> CaptureStreamResult<()> {
            self.stops.fetch_add(1, Ordering::SeqCst);
            self.status = CaptureStreamStatus::Stopped;
            Ok(())
        }
        fn status(&self) -> CaptureStreamStatus {
            self.status
        }
    }
    fn session() -> TransportSession {
        TransportSession::new(WebRtcConfig {
            room: "worker-test".to_string(),
            role: "host".to_string(),
            signaling_url: "local".to_string(),
            ice_servers: Vec::new(),
        })
        .unwrap()
    }
    fn config() -> CaptureStreamConfig {
        CaptureStreamConfig {
            selection: CaptureSelection {
                source_id: "source".to_string(),
                include_audio: false,
            },
            target_fps: Some(30),
            max_width: Some(1280),
            max_height: Some(720),
        }
    }
    #[test]
    fn capture_is_drained_without_gui_polling_and_stop_joins_the_worker() {
        let session = session();
        let stops = Arc::new(AtomicUsize::new(0));
        let mut worker = MediaPipelineRuntime::new(
            Box::new(SyntheticCapture {
                status: CaptureStreamStatus::Stopped,
                stops: stops.clone(),
                wrong_source: false,
            }),
            session.encoded_publisher().unwrap(),
        );
        worker.start(config()).unwrap();
        let start = Instant::now();
        while worker.status() != CaptureStreamStatus::Running
            && start.elapsed() < Duration::from_secs(2)
        {
            thread::sleep(Duration::from_millis(5));
        }
        thread::sleep(Duration::from_millis(40));
        assert_eq!(worker.status(), CaptureStreamStatus::Running);
        assert_eq!(worker.poll_events().unwrap().len(), 1); // newest raw diagnostic frame only
        assert_eq!(session.snapshot().encoded_video_frame_count, 0); // no peer: never publish raw media
        worker.stop().unwrap();
        assert_eq!(stops.load(Ordering::SeqCst), 1);
        assert_eq!(worker.status(), CaptureStreamStatus::Stopped);
        assert!(matches!(
            worker.poll_events().unwrap().as_slice(),
            [CaptureStreamEvent::Stopped { .. }]
        ));
    }
    #[test]
    fn worker_rejects_stale_source_even_before_a_peer_connects() {
        let session = session();
        let stops = Arc::new(AtomicUsize::new(0));
        let mut worker = MediaPipelineRuntime::new(
            Box::new(SyntheticCapture {
                status: CaptureStreamStatus::Stopped,
                stops: stops.clone(),
                wrong_source: true,
            }),
            session.encoded_publisher().unwrap(),
        );
        worker.start(config()).unwrap();
        let start = Instant::now();
        while worker.status() != CaptureStreamStatus::Failed
            && start.elapsed() < Duration::from_secs(2)
        {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(worker.status(), CaptureStreamStatus::Failed);
        assert!(matches!(
            worker.poll_events().unwrap().as_slice(),
            [CaptureStreamEvent::Error { .. }]
        ));
        worker.stop().unwrap();
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }

    struct BlockedCapture {
        stops: Arc<AtomicUsize>,
    }
    impl CaptureStreamRuntime for BlockedCapture {
        fn start(&mut self, _: CaptureStreamConfig) -> CaptureStreamResult<()> {
            Ok(())
        }
        fn poll_events(&mut self) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
            Ok(vec![CaptureStreamEvent::StatusChanged {
                source_id: Some("source".into()),
                status: CaptureStreamStatus::PermissionDenied,
                message: Some("permission denied".into()),
            }])
        }
        fn stop(&mut self) -> CaptureStreamResult<()> {
            self.stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn status(&self) -> CaptureStreamStatus {
            CaptureStreamStatus::PermissionDenied
        }
    }
    #[test]
    fn blocked_capture_exits_worker_and_preserves_permission_status() {
        let session = session();
        let stops = Arc::new(AtomicUsize::new(0));
        let mut worker = MediaPipelineRuntime::new(
            Box::new(BlockedCapture {
                stops: stops.clone(),
            }),
            session.encoded_publisher().unwrap(),
        );
        worker.start(config()).unwrap();
        let start = Instant::now();
        while stops.load(Ordering::SeqCst) == 0 {
            assert!(start.elapsed() < Duration::from_secs(2));
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(worker.status(), CaptureStreamStatus::PermissionDenied);
        assert!(matches!(
            worker.poll_events().unwrap().as_slice(),
            [CaptureStreamEvent::StatusChanged {
                status: CaptureStreamStatus::PermissionDenied,
                ..
            }]
        ));
        worker.stop().unwrap();
        assert_eq!(stops.load(Ordering::SeqCst), 1);
    }

    #[cfg(target_os = "macos")]
    struct LiveSyntheticCapture {
        stops: Arc<AtomicUsize>,
        video_pts: u64,
        audio_pts: u64,
        next_video: Instant,
        next_audio: Instant,
    }
    #[cfg(target_os = "macos")]
    impl CaptureStreamRuntime for LiveSyntheticCapture {
        fn start(&mut self, _: CaptureStreamConfig) -> CaptureStreamResult<()> {
            self.next_video = Instant::now();
            self.next_audio = Instant::now();
            Ok(())
        }
        fn poll_events(&mut self) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
            let mut events = Vec::new();
            let now = Instant::now();
            if now >= self.next_video {
                events.push(CaptureStreamEvent::VideoFrame {
                    source_id: "source".into(),
                    frame: VideoFrame {
                        format: VideoPixelFormat::Bgra8,
                        width: 64,
                        height: 36,
                        timestamp_micros: self.video_pts,
                        bytes: [32, 64, 192, 255].repeat(64 * 36),
                    },
                });
                self.video_pts += 33_333;
                self.next_video = now + Duration::from_micros(33_333);
            }
            if now >= self.next_audio {
                events.push(CaptureStreamEvent::AudioBuffer {
                    source_id: "source".into(),
                    buffer: capture_core::AudioBuffer {
                        sample_rate_hz: 48_000,
                        channels: 2,
                        frames: 960,
                        timestamp_micros: self.audio_pts,
                        samples: vec![0.1; 960 * 2],
                    },
                });
                self.audio_pts += 20_000;
                self.next_audio = now + Duration::from_millis(20);
            }
            Ok(events)
        }
        fn stop(&mut self) -> CaptureStreamResult<()> {
            self.stops.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn status(&self) -> CaptureStreamStatus {
            CaptureStreamStatus::Running
        }
    }

    /// Exercise real codecs, SDP, ICE, SRTP/RTP reception, and capture stop without screen access or GUI polling.
    #[cfg(target_os = "macos")]
    #[test]
    fn continuous_encoded_media_reaches_viewer_and_audio_opt_out_survives_restart() {
        use transport_webrtc::WebRtcSignal;
        let mut host = session();
        let mut viewer = TransportSession::new(WebRtcConfig {
            room: "worker-test".into(),
            role: "viewer".into(),
            signaling_url: "local".into(),
            ice_servers: Vec::new(),
        })
        .unwrap();
        let stops = Arc::new(AtomicUsize::new(0));
        let mut worker = MediaPipelineRuntime::new(
            Box::new(LiveSyntheticCapture {
                stops: stops.clone(),
                video_pts: 1_000_000,
                audio_pts: 1_000_000,
                next_video: Instant::now(),
                next_audio: Instant::now(),
            }),
            host.encoded_publisher().unwrap(),
        );
        let mut capture = config();
        capture.selection.include_audio = true;
        worker.start(capture.clone()).unwrap();

        let WebRtcSignal::SessionDescription { sdp, .. } = host.create_local_offer().unwrap()
        else {
            panic!("expected offer");
        };
        let WebRtcSignal::SessionDescription { sdp, .. } = viewer.accept_remote_offer(sdp).unwrap()
        else {
            panic!("expected answer");
        };
        host.accept_remote_answer(sdp).unwrap();
        fn relay_ice(sender: &mut TransportSession, receiver: &mut TransportSession) {
            for signal in sender.drain_local_signals() {
                if let WebRtcSignal::IceCandidate {
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                } = signal
                {
                    receiver
                        .add_remote_ice_candidate(candidate, sdp_mid, sdp_mline_index)
                        .unwrap();
                }
            }
        }
        let start = Instant::now();
        loop {
            relay_ice(&mut host, &mut viewer);
            relay_ice(&mut viewer, &mut host);
            let received = viewer.snapshot();
            if received.received_video_packet_count > 0 && received.received_audio_packet_count > 0
            {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "media delivery timed out: host={:?}; viewer={:?}; capture={:?}",
                host.snapshot(),
                received,
                worker.status()
            );
            thread::sleep(Duration::from_millis(10));
        }
        let published = host.snapshot();
        assert!(published.encoded_video_frame_count > 0);
        assert!(published.encoded_audio_packet_count > 0);
        assert!(
            published
                .last_video_capture_summary
                .unwrap()
                .starts_with("H264")
        );
        assert!(
            published
                .last_audio_capture_summary
                .unwrap()
                .starts_with("Opus")
        );
        assert!(worker.poll_events().unwrap().len() <= 2); // GUI holds latest samples only

        worker.stop().unwrap();
        let stopped = host.snapshot();
        thread::sleep(Duration::from_millis(60));
        assert_eq!(
            host.snapshot().encoded_video_frame_count,
            stopped.encoded_video_frame_count
        );
        assert_eq!(
            host.snapshot().encoded_audio_packet_count,
            stopped.encoded_audio_packet_count
        );
        capture.selection.include_audio = false;
        worker.start(capture).unwrap();
        let start = Instant::now();
        while host.snapshot().encoded_video_frame_count == stopped.encoded_video_frame_count {
            assert!(start.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(60));
        assert_eq!(
            host.snapshot().encoded_audio_packet_count,
            stopped.encoded_audio_packet_count
        );
        assert!(
            !worker
                .poll_events()
                .unwrap()
                .iter()
                .any(|event| matches!(event, CaptureStreamEvent::AudioBuffer { .. }))
        );
        worker.stop().unwrap();
        assert_eq!(stops.load(Ordering::SeqCst), 2);
        host.close().unwrap();
        viewer.close().unwrap();
    }
}
