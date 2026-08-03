use crate::capture_catalog::{
    CaptureCatalogSnapshot, current_capture_catalog, describe_permission_state,
    new_capture_runtime, selected_source_label,
};
use crate::ice_servers::{IceServerEntry, format_ice_server_entries, summarize_ice_server_entries};
use crate::preferences::{PersistedSessionConfig, PreferencesStore, UiPreferences};
use crate::protocol::{
    IceCandidate, PeerAnnouncement, Role, SdpType, SessionDescription, SignalingMessage,
};
use crate::signaling::{SignalingConnection, SignalingEvent};
use capture_core::{
    AudioBuffer, CapturePermissionState, CaptureSelection, CaptureStreamConfig, CaptureStreamEvent,
    CaptureStreamRuntime, CaptureStreamStatus, VideoFrame, VideoPixelFormat,
};
use std::time::{SystemTime, UNIX_EPOCH};
use transport_webrtc::{
    DescriptionKind, TransportSession, TransportStage, WebRtcConfig, WebRtcSignal,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    Idle,
    Host,
    Viewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStage {
    Idle,
    Configured,
    AwaitingPeer,
    MockStreaming,
    NegotiatingWebRtc,
    LiveWebRtc,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTransport {
    MockUdp,
    LiveWebRtc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionIntent {
    pub room: String,
    pub signaling_addr: String,
    pub source_label: Option<String>,
    pub ice_servers: Vec<IceServerEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSnapshot {
    pub mode: SessionMode,
    pub stage: SessionStage,
    pub transport: SessionTransport,
    pub room: Option<String>,
    pub signaling_addr: Option<String>,
    pub ice_server_count: usize,
    pub ice_server_summary: String,
    pub ice_servers: String,
    pub source_label: Option<String>,
    pub selected_source_id: Option<String>,
    pub selected_source_audio: bool,
    pub capture_backend: String,
    pub capture_permission_state: String,
    pub capture_runtime_status: String,
    pub available_source_count: usize,
    pub active_peer: Option<String>,
    pub logs: Vec<String>,
    pub next_action: String,
    pub transport_state: String,
    pub transport_stage: Option<String>,
    pub signaling_connected: bool,
    pub local_description_ready: bool,
    pub local_description_kind: Option<String>,
    pub remote_description_ready: bool,
    pub remote_description_kind: Option<String>,
    pub local_media_track_count: usize,
    pub local_video_track_attached: bool,
    pub local_audio_track_attached: bool,
    pub published_video_sample_count: usize,
    pub published_audio_sample_count: usize,
    pub last_video_sample_bytes: usize,
    pub last_audio_sample_bytes: usize,
    pub last_video_capture_summary: Option<String>,
    pub last_audio_capture_summary: Option<String>,
    pub local_data_channel_ready: bool,
    pub transport_stats_report_count: usize,
    pub transport_ice_path_kind: String,
    pub transport_ice_path_summary: String,
    pub transport_rtt_ms: Option<f64>,
    pub transport_available_outgoing_bitrate_bps: Option<f64>,
    pub transport_available_incoming_bitrate_bps: Option<f64>,
    pub transport_bytes_sent: Option<u64>,
    pub transport_bytes_received: Option<u64>,
    pub transport_packet_loss_fraction: Option<f64>,
    pub transport_packets_lost: Option<i64>,
    pub transport_notes: Vec<String>,
    pub local_offer_ready: bool,
    pub remote_answer_ready: bool,
    pub local_candidate_count: usize,
    pub remote_candidate_count: usize,
    pub last_signaling_message: Option<String>,
    pub recovery_state: String,
    pub recovery_reason: String,
    pub can_reconnect: bool,
    pub reconnect_recommended: bool,
    pub ui_auto_refresh_enabled: bool,
    pub ui_refresh_interval_secs: u32,
}

pub struct SessionManager {
    state: SessionState,
    preferences: PreferencesStore,
}

struct SessionState {
    mode: SessionMode,
    stage: SessionStage,
    transport: SessionTransport,
    room: Option<String>,
    signaling_addr: Option<String>,
    ice_servers: Vec<IceServerEntry>,
    source_label: Option<String>,
    capture_catalog: CaptureCatalogSnapshot,
    capture_selection: Option<CaptureSelection>,
    ui_preferences: UiPreferences,
    active_peer: Option<String>,
    logs: Vec<String>,
    webrtc: Option<TransportSession>,
    signaling: Option<SignalingConnection>,
    signaling_connected: bool,
    last_signaling_message: Option<String>,
    capture_runtime: Option<Box<dyn CaptureStreamRuntime + Send>>,
}

struct RecoveryDiagnostics {
    state: &'static str,
    reason: String,
    can_reconnect: bool,
    reconnect_recommended: bool,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            mode: SessionMode::Idle,
            stage: SessionStage::Idle,
            transport: SessionTransport::MockUdp,
            room: None,
            signaling_addr: None,
            ice_servers: Vec::new(),
            source_label: None,
            capture_catalog: current_capture_catalog(),
            capture_selection: None,
            ui_preferences: UiPreferences::default(),
            active_peer: None,
            logs: vec![stamp("session manager initialized")],
            webrtc: None,
            signaling: None,
            signaling_connected: false,
            last_signaling_message: None,
            capture_runtime: None,
        }
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    fn new_for_tests(preferences: PreferencesStore) -> Self {
        Self::with_preferences(preferences)
    }

    pub fn start_host(&mut self, intent: SessionIntent) -> SessionSnapshot {
        self.sync_capture_catalog();
        self.ensure_default_host_capture_selection();
        let selected_source_label = selected_source_label(
            &self.state.capture_catalog,
            self.state.capture_selection.as_ref(),
        );
        self.replace_state(
            SessionMode::Host,
            SessionStage::Configured,
            SessionTransport::LiveWebRtc,
            intent.clone(),
        );
        self.state.source_label = Some(
            intent
                .source_label
                .clone()
                .or(selected_source_label)
                .unwrap_or_else(|| "window selection pending".to_string()),
        );
        self.push_log(format!(
            "host session configured for room={} signaling={}",
            intent.room, intent.signaling_addr
        ));
        self.log_ice_server_configuration();
        self.log_host_capture_readiness();
        self.initialize_transport("host");
        self.connect_signaling();
        self.ensure_host_offer("local SDP offer created and sent automatically");
        self.snapshot()
    }

    pub fn start_viewer(&mut self, intent: SessionIntent) -> SessionSnapshot {
        self.sync_capture_catalog();
        self.replace_state(
            SessionMode::Viewer,
            SessionStage::AwaitingPeer,
            SessionTransport::LiveWebRtc,
            intent.clone(),
        );
        self.push_log(format!(
            "viewer session configured for room={} signaling={}",
            intent.room, intent.signaling_addr
        ));
        self.log_ice_server_configuration();
        self.initialize_transport("viewer");
        self.connect_signaling();
        self.snapshot()
    }

    pub fn mark_mock_streaming(&mut self, peer: String) -> SessionSnapshot {
        self.state.stage = SessionStage::MockStreaming;
        self.state.transport = SessionTransport::MockUdp;
        self.state.active_peer = Some(peer.clone());
        self.push_log(format!("mock streaming active with peer={peer}"));
        self.snapshot()
    }

    pub fn mark_webrtc_ready(&mut self) -> SessionSnapshot {
        self.state.stage = SessionStage::NegotiatingWebRtc;
        self.state.transport = SessionTransport::LiveWebRtc;
        self.push_log("session moved to live WebRTC negotiation stage".to_string());
        self.snapshot()
    }

    pub fn create_local_offer(&mut self) -> SessionSnapshot {
        self.issue_local_offer("local SDP offer created and sent");
        self.snapshot()
    }

    pub fn accept_remote_answer(&mut self, sdp: String) -> SessionSnapshot {
        if let Some(webrtc) = self.state.webrtc.as_mut() {
            if let Err(error) = webrtc.accept_remote_answer(sdp) {
                self.push_log(format!("failed to accept remote answer: {error}"));
            } else {
                self.state.stage = SessionStage::NegotiatingWebRtc;
                self.state.transport = SessionTransport::LiveWebRtc;
                self.state.last_signaling_message = Some("remote answer accepted".to_string());
                self.push_log("remote SDP answer accepted".to_string());
            }
        } else {
            self.push_log("cannot accept answer before session is configured".to_string());
        }
        self.snapshot()
    }

    pub fn add_remote_ice_candidate(
        &mut self,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    ) -> SessionSnapshot {
        if let Some(webrtc) = self.state.webrtc.as_mut() {
            if let Err(error) = webrtc.add_remote_ice_candidate(candidate, sdp_mid, sdp_mline_index)
            {
                self.push_log(format!("failed to add remote ICE candidate: {error}"));
            } else {
                self.state.stage = SessionStage::NegotiatingWebRtc;
                self.state.transport = SessionTransport::LiveWebRtc;
                self.state.last_signaling_message = Some("remote ICE candidate added".to_string());
                self.push_log("remote ICE candidate registered".to_string());
            }
        } else {
            self.push_log("cannot add ICE candidate before session is configured".to_string());
        }
        self.snapshot()
    }

    pub fn update_config(
        &mut self,
        room: Option<String>,
        signaling_addr: Option<String>,
        source_label: Option<String>,
        ice_servers: Option<Vec<IceServerEntry>>,
    ) -> SessionSnapshot {
        if let Some(room) = room {
            self.state.room = Some(room.clone());
            self.push_log(format!("room updated to {room}"));
        }
        if let Some(signaling_addr) = signaling_addr {
            self.state.signaling_addr = Some(signaling_addr.clone());
            self.push_log(format!("signaling updated to {signaling_addr}"));
        }
        if let Some(source_label) = source_label {
            self.state.source_label = Some(source_label.clone());
            self.push_log(format!("source label updated to {source_label}"));
        }
        if let Some(ice_servers) = ice_servers {
            self.state.ice_servers = ice_servers;
            self.push_log(format!(
                "ICE server configuration updated: {} entries",
                self.state.ice_servers.len()
            ));
        }
        self.persist_preferences();
        self.snapshot()
    }

    pub fn capture_catalog(&mut self) -> CaptureCatalogSnapshot {
        self.sync_capture_catalog();
        self.state.capture_catalog.clone()
    }

    pub fn update_ui_preferences(
        &mut self,
        auto_refresh_enabled: Option<bool>,
        refresh_interval_secs: Option<u32>,
    ) -> Result<SessionSnapshot, String> {
        if let Some(auto_refresh_enabled) = auto_refresh_enabled {
            self.state.ui_preferences.auto_refresh_enabled = auto_refresh_enabled;
            self.push_log(format!(
                "UI auto-refresh {}",
                if auto_refresh_enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            ));
        }
        if let Some(refresh_interval_secs) = refresh_interval_secs {
            if refresh_interval_secs == 0 {
                return Err("refresh interval must be greater than zero seconds".to_string());
            }
            self.state.ui_preferences.refresh_interval_secs = refresh_interval_secs;
            self.push_log(format!(
                "UI refresh interval updated to {}s",
                refresh_interval_secs
            ));
        }
        self.persist_preferences();
        Ok(self.snapshot())
    }

    pub fn select_capture_source(
        &mut self,
        source_id: String,
        include_audio: bool,
    ) -> SessionSnapshot {
        self.sync_capture_catalog();
        let Some(source) = self
            .state
            .capture_catalog
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .cloned()
        else {
            self.push_log(format!("capture source not found: {source_id}"));
            return self.snapshot();
        };

        let include_audio = include_audio && source.has_audio;
        self.state.capture_selection = Some(CaptureSelection {
            source_id: source.id.clone(),
            include_audio,
        });
        self.state.source_label = Some(source.label());
        self.push_log(format!(
            "capture source selected: id={} include_audio={}",
            source.id, include_audio
        ));
        self.persist_preferences();
        self.snapshot()
    }

    pub fn refresh(&mut self) -> SessionSnapshot {
        self.sync_capture_catalog();
        self.process_signaling_events();
        self.poll_capture_runtime_events();
        self.ensure_host_offer("local SDP offer created and sent automatically");
        self.flush_local_transport_signals();
        self.update_stage_from_transport();
        self.snapshot()
    }

    pub fn start_capture_stream(&mut self) -> Result<SessionSnapshot, String> {
        self.sync_capture_catalog();

        if self.state.mode != SessionMode::Host {
            let message =
                "native capture stream can only be started from a host session".to_string();
            self.push_log(message.clone());
            return Err(message);
        }

        let Some(selection) = self.state.capture_selection.clone() else {
            let message =
                "native capture stream cannot start before selecting a capture source".to_string();
            self.push_log(message.clone());
            return Err(message);
        };

        if let Err(message) = self.validate_capture_publish_request(&selection.source_id, false) {
            self.push_log(message.clone());
            return Err(message);
        }

        if self.state.capture_runtime.is_none() {
            self.state.capture_runtime = new_capture_runtime();
        }

        let Some(runtime) = self.state.capture_runtime.as_mut() else {
            let message =
                "native capture stream runtime is not available for this platform".to_string();
            self.push_log(message.clone());
            return Err(message);
        };

        let source_id = selection.source_id.clone();
        let include_audio = selection.include_audio;
        let result = runtime.start(CaptureStreamConfig {
            selection,
            target_fps: Some(30),
            max_width: Some(1280),
            max_height: Some(720),
        });

        match result {
            Ok(()) => {
                self.push_log(format!(
                    "native capture stream start requested for source={source_id} include_audio={include_audio}"
                ));
                self.poll_capture_runtime_events();
                Ok(self.snapshot())
            }
            Err(error) => {
                let message = format!(
                    "failed to start native capture stream for source={source_id}: {error}"
                );
                self.push_log(message.clone());
                Err(message)
            }
        }
    }

    pub fn poll_capture_stream(&mut self) -> SessionSnapshot {
        self.poll_capture_runtime_events();
        self.snapshot()
    }

    pub fn stop_capture_stream(&mut self) -> SessionSnapshot {
        let result = match self.state.capture_runtime.as_mut() {
            Some(runtime) => runtime.stop(),
            None => {
                self.push_log("native capture stream is not running".to_string());
                return self.snapshot();
            }
        };

        match result {
            Ok(()) => {
                self.push_log("native capture stream stop requested".to_string());
                self.poll_capture_runtime_events();
            }
            Err(error) => {
                self.push_log(format!("failed to stop native capture stream: {error}"));
            }
        }

        self.snapshot()
    }

    pub fn publish_capture_video_frame(
        &mut self,
        source_id: String,
        frame: VideoFrame,
    ) -> SessionSnapshot {
        if let Err(message) = self.validate_capture_publish_request(&source_id, false) {
            self.push_log(message);
            return self.snapshot();
        }

        match self.state.webrtc.as_mut() {
            Some(webrtc) => match webrtc.publish_video_frame(frame) {
                Ok(()) => {
                    self.push_log(format!(
                        "capture video frame published for source={source_id}"
                    ));
                }
                Err(error) => {
                    self.push_log(format!(
                        "failed to publish capture video frame for source={source_id}: {error}"
                    ));
                }
            },
            None => self
                .push_log("cannot publish capture video before session is configured".to_string()),
        }

        self.snapshot()
    }

    pub fn publish_capture_audio_buffer(
        &mut self,
        source_id: String,
        buffer: AudioBuffer,
    ) -> SessionSnapshot {
        if let Err(message) = self.validate_capture_publish_request(&source_id, true) {
            self.push_log(message);
            return self.snapshot();
        }

        match self.state.webrtc.as_mut() {
            Some(webrtc) => match webrtc.publish_audio_buffer(buffer) {
                Ok(()) => {
                    self.push_log(format!(
                        "capture audio buffer published for source={source_id}"
                    ));
                }
                Err(error) => {
                    self.push_log(format!(
                        "failed to publish capture audio buffer for source={source_id}: {error}"
                    ));
                }
            },
            None => self
                .push_log("cannot publish capture audio before session is configured".to_string()),
        }

        self.snapshot()
    }

    pub fn ingest_capture_stream_event(&mut self, event: CaptureStreamEvent) -> SessionSnapshot {
        match event {
            CaptureStreamEvent::Started { source_id } => {
                if let Err(message) = self.validate_capture_publish_request(&source_id, false) {
                    self.push_log(message);
                } else {
                    self.push_log(format!("capture stream started for source={source_id}"));
                }
                self.snapshot()
            }
            CaptureStreamEvent::StatusChanged {
                source_id,
                status,
                message,
            } => {
                let source = source_id.unwrap_or_else(|| "unknown".to_string());
                let detail = message.unwrap_or_else(|| "no detail".to_string());
                self.push_log(format!(
                    "capture stream status changed for source={source}: {} ({detail})",
                    format_capture_stream_status(status)
                ));
                self.snapshot()
            }
            CaptureStreamEvent::VideoFrame { source_id, frame } => {
                self.publish_capture_video_frame(source_id, frame)
            }
            CaptureStreamEvent::AudioBuffer { source_id, buffer } => {
                self.publish_capture_audio_buffer(source_id, buffer)
            }
            CaptureStreamEvent::Stopped { source_id, reason } => {
                let source = source_id.unwrap_or_else(|| "unknown".to_string());
                let reason = reason.unwrap_or_else(|| "no reason provided".to_string());
                self.push_log(format!(
                    "capture stream stopped for source={source}: {reason}"
                ));
                self.snapshot()
            }
            CaptureStreamEvent::Error { source_id, message } => {
                let source = source_id.unwrap_or_else(|| "unknown".to_string());
                self.push_log(format!(
                    "capture stream error for source={source}: {message}"
                ));
                self.snapshot()
            }
        }
    }

    pub fn publish_debug_capture_samples(&mut self) -> SessionSnapshot {
        let Some(selection) = self.state.capture_selection.clone() else {
            self.push_log(
                "cannot publish debug capture samples before selecting a capture source"
                    .to_string(),
            );
            return self.snapshot();
        };

        self.publish_capture_video_frame(
            selection.source_id.clone(),
            debug_video_frame(&selection.source_id),
        );

        if selection.include_audio {
            self.publish_capture_audio_buffer(selection.source_id.clone(), debug_audio_buffer());
            self.push_log(format!(
                "debug capture video/audio payloads published for source={}",
                selection.source_id
            ));
        } else {
            self.push_log(format!(
                "debug capture video payload published for source={}; audio skipped by capture selection",
                selection.source_id
            ));
        }

        self.snapshot()
    }

    pub fn publish_placeholder_media(&mut self) -> SessionSnapshot {
        self.publish_debug_capture_samples()
    }

    pub fn reconnect(&mut self) -> Result<SessionSnapshot, String> {
        let mode = self.state.mode;
        if mode == SessionMode::Idle {
            return Err("cannot reconnect before preparing a host or viewer session".to_string());
        }

        let room = self
            .state
            .room
            .clone()
            .ok_or_else(|| "cannot reconnect without a configured room".to_string())?;
        let signaling_addr =
            self.state.signaling_addr.clone().ok_or_else(|| {
                "cannot reconnect without a configured signaling address".to_string()
            })?;
        let source_label = self.state.source_label.clone();
        let ice_servers = self.state.ice_servers.clone();

        self.disconnect_active_session();

        let snapshot = match mode {
            SessionMode::Host => {
                self.push_log(format!(
                    "reconnecting host session for room={} signaling={}",
                    room, signaling_addr
                ));
                self.start_host(SessionIntent {
                    room,
                    signaling_addr,
                    source_label,
                    ice_servers,
                })
            }
            SessionMode::Viewer => {
                self.push_log(format!(
                    "reconnecting viewer session for room={} signaling={}",
                    room, signaling_addr
                ));
                self.start_viewer(SessionIntent {
                    room,
                    signaling_addr,
                    source_label: None,
                    ice_servers,
                })
            }
            SessionMode::Idle => unreachable!(),
        };

        Ok(snapshot)
    }

    pub fn stop(&mut self) -> SessionSnapshot {
        self.stop_capture_runtime_without_snapshot();
        if let Some(webrtc) = self.state.webrtc.as_mut()
            && let Err(error) = webrtc.close()
        {
            self.push_log(format!("failed to close WebRTC transport: {error}"));
        }
        self.state.stage = SessionStage::Stopped;
        self.state.active_peer = None;
        self.state.last_signaling_message = None;
        self.state.signaling = None;
        self.state.signaling_connected = false;
        self.push_log("session stopped".to_string());
        self.snapshot()
    }

    pub fn clear_logs(&mut self) -> SessionSnapshot {
        self.state.logs.clear();
        self.push_log("session log cleared".to_string());
        self.snapshot()
    }

    pub fn reset(&mut self) -> SessionSnapshot {
        self.stop_capture_runtime_without_snapshot();
        if let Some(webrtc) = self.state.webrtc.as_mut() {
            let _ = webrtc.close();
        }
        self.state = SessionState::default();
        self.load_persisted_preferences();
        self.push_log("session reset to idle state".to_string());
        self.snapshot()
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        let transport_snapshot = self.state.webrtc.as_ref().map(|session| session.snapshot());
        let recovery = self.recovery_diagnostics(transport_snapshot.as_ref());
        let (local_description_kind, remote_description_kind) = transport_snapshot
            .as_ref()
            .map(|snapshot| {
                (
                    snapshot.local_description_kind.map(format_description_kind),
                    snapshot
                        .remote_description_kind
                        .map(format_description_kind),
                )
            })
            .unwrap_or((None, None));

        SessionSnapshot {
            mode: self.state.mode,
            stage: self.state.stage,
            transport: self.state.transport,
            room: self.state.room.clone(),
            signaling_addr: self.state.signaling_addr.clone(),
            ice_server_count: self.state.ice_servers.len(),
            ice_server_summary: summarize_ice_server_entries(&self.state.ice_servers),
            ice_servers: format_ice_server_entries(&self.state.ice_servers),
            source_label: self.state.source_label.clone(),
            selected_source_id: self
                .state
                .capture_selection
                .as_ref()
                .map(|selection| selection.source_id.clone()),
            selected_source_audio: self
                .state
                .capture_selection
                .as_ref()
                .map(|selection| selection.include_audio)
                .unwrap_or(false),
            capture_backend: self.state.capture_catalog.backend.clone(),
            capture_permission_state: describe_permission_state(
                self.state.capture_catalog.permission_state,
            )
            .to_string(),
            capture_runtime_status: self
                .state
                .capture_runtime
                .as_ref()
                .map(|runtime| format_capture_stream_status(runtime.status()).to_string())
                .unwrap_or_else(|| "not_started".to_string()),
            available_source_count: self.state.capture_catalog.sources.len(),
            active_peer: self.state.active_peer.clone(),
            logs: self.state.logs.clone(),
            next_action: self.next_action().to_string(),
            transport_state: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.connection_state.clone())
                .unwrap_or_else(|| "not_initialized".to_string()),
            transport_stage: transport_snapshot
                .as_ref()
                .map(|snapshot| format_transport_stage(snapshot.stage)),
            signaling_connected: self.state.signaling_connected,
            local_description_ready: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.local_description_ready)
                .unwrap_or(false),
            local_description_kind,
            remote_description_ready: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.remote_description_ready)
                .unwrap_or(false),
            remote_description_kind,
            local_media_track_count: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.local_media_track_count)
                .unwrap_or(0),
            local_video_track_attached: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.local_video_track_attached)
                .unwrap_or(false),
            local_audio_track_attached: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.local_audio_track_attached)
                .unwrap_or(false),
            published_video_sample_count: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.published_video_sample_count)
                .unwrap_or(0),
            published_audio_sample_count: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.published_audio_sample_count)
                .unwrap_or(0),
            last_video_sample_bytes: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.last_video_sample_bytes)
                .unwrap_or(0),
            last_audio_sample_bytes: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.last_audio_sample_bytes)
                .unwrap_or(0),
            last_video_capture_summary: transport_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.last_video_capture_summary.clone()),
            last_audio_capture_summary: transport_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.last_audio_capture_summary.clone()),
            local_data_channel_ready: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.local_data_channel_ready)
                .unwrap_or(false),
            transport_stats_report_count: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.stats_report_count)
                .unwrap_or(0),
            transport_ice_path_kind: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.ice_path_kind.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            transport_ice_path_summary: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.ice_path_summary.clone())
                .unwrap_or_else(|| "candidate pair not selected yet".to_string()),
            transport_rtt_ms: transport_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.rtt_ms),
            transport_available_outgoing_bitrate_bps: transport_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.available_outgoing_bitrate_bps),
            transport_available_incoming_bitrate_bps: transport_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.available_incoming_bitrate_bps),
            transport_bytes_sent: transport_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.bytes_sent),
            transport_bytes_received: transport_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.bytes_received),
            transport_packet_loss_fraction: transport_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.packet_loss_fraction),
            transport_packets_lost: transport_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.packets_lost),
            transport_notes: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.notes.clone())
                .unwrap_or_default(),
            local_offer_ready: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.local_description_kind == Some(DescriptionKind::Offer))
                .unwrap_or(false),
            remote_answer_ready: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.remote_description_kind == Some(DescriptionKind::Answer))
                .unwrap_or(false),
            local_candidate_count: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.local_candidate_count)
                .unwrap_or(0),
            remote_candidate_count: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.remote_candidate_count)
                .unwrap_or(0),
            last_signaling_message: self.state.last_signaling_message.clone(),
            recovery_state: recovery.state.to_string(),
            recovery_reason: recovery.reason,
            can_reconnect: recovery.can_reconnect,
            reconnect_recommended: recovery.reconnect_recommended,
            ui_auto_refresh_enabled: self.state.ui_preferences.auto_refresh_enabled,
            ui_refresh_interval_secs: self.state.ui_preferences.refresh_interval_secs,
        }
    }

    pub fn logs(&self) -> Vec<String> {
        self.state.logs.clone()
    }

    fn with_preferences(preferences: PreferencesStore) -> Self {
        let mut manager = Self {
            state: SessionState::default(),
            preferences,
        };
        manager.load_persisted_preferences();
        manager
    }

    fn replace_state(
        &mut self,
        mode: SessionMode,
        stage: SessionStage,
        transport: SessionTransport,
        intent: SessionIntent,
    ) {
        self.state.mode = mode;
        self.state.stage = stage;
        self.state.transport = transport;
        self.state.room = Some(intent.room);
        self.state.signaling_addr = Some(intent.signaling_addr);
        self.state.ice_servers = intent.ice_servers;
        self.state.source_label = intent.source_label;
        self.state.active_peer = None;
        self.state.webrtc = None;
        self.state.signaling = None;
        self.state.signaling_connected = false;
        self.state.last_signaling_message = None;
        self.state.capture_runtime = None;
    }

    fn initialize_transport(&mut self, role: &str) {
        let room = self.state.room.clone().unwrap_or_default();
        let signaling_addr = self.state.signaling_addr.clone().unwrap_or_default();
        match TransportSession::new(WebRtcConfig {
            room,
            role: role.to_string(),
            signaling_url: signaling_addr,
            ice_servers: self
                .state
                .ice_servers
                .iter()
                .map(IceServerEntry::to_transport)
                .collect(),
        }) {
            Ok(session) => {
                self.state.webrtc = Some(session);
                self.push_log(format!(
                    "real WebRTC PeerConnection initialized with {} ICE server entries",
                    self.state.ice_servers.len()
                ));
            }
            Err(error) => {
                self.state.webrtc = None;
                self.push_log(format!("failed to initialize WebRTC transport: {error}"));
            }
        }
    }

    fn ensure_default_host_capture_selection(&mut self) {
        if self.state.capture_selection.is_some() {
            return;
        }

        self.select_first_available_host_capture_source(
            None,
            "default capture source selected for host",
        );
    }

    fn select_first_available_host_capture_source(
        &mut self,
        preferred_audio: Option<bool>,
        log_prefix: &str,
    ) -> bool {
        let Some(source) = self.state.capture_catalog.sources.first().cloned() else {
            return false;
        };

        let include_audio = preferred_audio.unwrap_or(source.has_audio) && source.has_audio;
        self.state.capture_selection = Some(CaptureSelection {
            source_id: source.id.clone(),
            include_audio,
        });
        self.state.source_label = Some(source.label());
        self.push_log(format!(
            "{log_prefix}: id={} include_audio={}",
            source.id, include_audio
        ));
        true
    }

    fn ensure_host_offer(&mut self, success_message: &str) {
        if self.state.mode != SessionMode::Host || !self.state.signaling_connected {
            return;
        }

        let local_description_ready = self
            .state
            .webrtc
            .as_ref()
            .map(|session| session.snapshot().local_description_ready)
            .unwrap_or(false);
        if local_description_ready {
            return;
        }

        self.issue_local_offer(success_message);
    }

    fn issue_local_offer(&mut self, success_message: &str) {
        let offer = match self.state.webrtc.as_mut() {
            Some(webrtc) => match webrtc.create_local_offer() {
                Ok(signal) => Some(signal),
                Err(error) => {
                    self.push_log(format!("failed to create local offer: {error}"));
                    None
                }
            },
            None => {
                self.push_log("cannot create offer before session is configured".to_string());
                None
            }
        };

        if let Some(signal) = offer {
            self.state.transport = SessionTransport::LiveWebRtc;
            self.state.stage = SessionStage::NegotiatingWebRtc;
            self.send_transport_signal(signal);
            self.flush_local_transport_signals();
            self.push_log(success_message.to_string());
        }
    }

    fn connect_signaling(&mut self) {
        let Some(signaling_addr) = self.state.signaling_addr.clone() else {
            return;
        };
        let Some(room) = self.state.room.clone() else {
            return;
        };
        let role = match self.state.mode {
            SessionMode::Host => Role::Sender,
            SessionMode::Viewer => Role::Receiver,
            SessionMode::Idle => return,
        };

        match SignalingConnection::connect(&signaling_addr, &room, role, 0) {
            Ok(connection) => {
                self.state.signaling = Some(connection);
                self.state.signaling_connected = true;
                self.push_log("connected to signaling server".to_string());
                self.process_signaling_events();
            }
            Err(error) => {
                self.state.signaling = None;
                self.state.signaling_connected = false;
                self.push_log(format!("failed to connect signaling server: {error}"));
            }
        }
    }

    fn sync_capture_catalog(&mut self) {
        let previous_catalog = self.state.capture_catalog.clone();
        self.state.capture_catalog = current_capture_catalog();
        let mut persist_preferences = false;

        if previous_catalog.backend != self.state.capture_catalog.backend
            || previous_catalog.permission_state != self.state.capture_catalog.permission_state
            || previous_catalog.origin != self.state.capture_catalog.origin
        {
            self.push_log(format!(
                "capture catalog refreshed: backend={} permission={} origin={}",
                self.state.capture_catalog.backend,
                describe_permission_state(self.state.capture_catalog.permission_state),
                self.state.capture_catalog.origin
            ));
        }

        if let Some(selection) = self.state.capture_selection.clone() {
            if let Some(label) =
                selected_source_label(&self.state.capture_catalog, Some(&selection))
            {
                self.state.source_label = Some(label);
            } else {
                let preferred_audio = selection.include_audio;
                self.state.capture_selection = None;
                self.state.source_label = None;
                persist_preferences = true;
                self.push_log(format!(
                    "capture source dropped from refreshed catalog: {}",
                    selection.source_id
                ));
                if self.state.mode == SessionMode::Host {
                    persist_preferences |= self.select_first_available_host_capture_source(
                        Some(preferred_audio),
                        "host capture source rebound after catalog refresh",
                    );
                }
            }
        } else if self.state.mode == SessionMode::Host {
            let had_selection = self.state.capture_selection.is_some();
            self.ensure_default_host_capture_selection();
            persist_preferences = !had_selection && self.state.capture_selection.is_some();
        }

        if persist_preferences {
            self.persist_preferences();
        }
    }

    fn disconnect_active_session(&mut self) {
        self.stop_capture_runtime_without_snapshot();
        if let Some(webrtc) = self.state.webrtc.as_mut()
            && let Err(error) = webrtc.close()
        {
            self.push_log(format!("failed to close WebRTC transport: {error}"));
        }
        self.state.signaling = None;
        self.state.signaling_connected = false;
        self.state.active_peer = None;
        self.state.last_signaling_message = None;
    }

    fn poll_capture_runtime_events(&mut self) {
        let events = match self.state.capture_runtime.as_mut() {
            Some(runtime) => match runtime.poll_events() {
                Ok(events) => events,
                Err(error) => {
                    self.push_log(format!("failed to poll native capture stream: {error}"));
                    return;
                }
            },
            None => return,
        };

        for event in events {
            self.ingest_capture_stream_event(event);
        }
    }

    fn stop_capture_runtime_without_snapshot(&mut self) {
        let Some(mut runtime) = self.state.capture_runtime.take() else {
            return;
        };

        match runtime.stop() {
            Ok(()) => {
                self.push_log("native capture stream stopped".to_string());
                match runtime.poll_events() {
                    Ok(events) => {
                        for event in events {
                            self.ingest_capture_stream_event(event);
                        }
                    }
                    Err(error) => {
                        self.push_log(format!("failed to poll native capture stream: {error}"));
                    }
                }
            }
            Err(error) => {
                self.push_log(format!("failed to stop native capture stream: {error}"));
            }
        }
    }

    fn validate_capture_publish_request(
        &self,
        source_id: &str,
        require_audio: bool,
    ) -> Result<(), String> {
        if self.state.mode != SessionMode::Host {
            return Err("capture media can only be published from a host session".to_string());
        }

        let Some(selection) = self.state.capture_selection.as_ref() else {
            return Err(
                "capture media cannot be published before selecting a capture source".to_string(),
            );
        };

        if selection.source_id != source_id {
            return Err(format!(
                "capture media source mismatch: selected={} published={}",
                selection.source_id, source_id
            ));
        }

        if require_audio && !selection.include_audio {
            return Err(format!(
                "capture audio buffer ignored because audio is disabled for source={source_id}"
            ));
        }

        Ok(())
    }

    fn log_host_capture_readiness(&mut self) {
        match self.state.capture_catalog.permission_state {
            CapturePermissionState::Granted => {
                self.push_log(
                    "capture catalog is ready for host-side source selection".to_string(),
                );
            }
            CapturePermissionState::Required => {
                self.push_log(
                    "capture permission is still required; runtime capture may stay on fallback sources until access is granted"
                        .to_string(),
                );
            }
            CapturePermissionState::Denied => {
                self.push_log(
                    "capture permission appears denied; OS approval is required before real host capture can start"
                        .to_string(),
                );
            }
            CapturePermissionState::Unknown => {
                self.push_log(
                    "capture permission state is unknown; verify the desktop session and refresh the catalog before relying on real host capture"
                        .to_string(),
                );
            }
        }
    }

    fn process_signaling_events(&mut self) {
        let events = match self.state.signaling.as_mut() {
            Some(connection) => match connection.poll() {
                Ok(events) => events,
                Err(error) => {
                    self.state.signaling_connected = false;
                    self.push_log(format!("signaling poll error: {error}"));
                    return;
                }
            },
            None => return,
        };

        for event in events {
            match event {
                SignalingEvent::Waiting => {
                    self.state.last_signaling_message = Some("waiting for peer".to_string());
                    self.push_log("signaling server is waiting for the second peer".to_string());
                }
                SignalingEvent::Peer(peer) => {
                    self.handle_peer_announcement(peer);
                }
                SignalingEvent::Message(message) => {
                    self.handle_signaling_message(message);
                }
            }
        }
    }

    fn handle_peer_announcement(&mut self, peer: PeerAnnouncement) {
        self.state.active_peer = Some(peer.addr.to_string());
        self.state.stage = SessionStage::NegotiatingWebRtc;
        self.state.last_signaling_message = Some(format!(
            "peer announced: role={} addr={}",
            peer.role, peer.addr
        ));
        self.push_log(format!("peer discovered via signaling: {}", peer.addr));
    }

    fn handle_signaling_message(&mut self, message: SignalingMessage) {
        match message {
            SignalingMessage::SessionDescription(description) => match description.sdp_type {
                SdpType::Offer => {
                    self.state.last_signaling_message =
                        Some("remote SDP offer received".to_string());
                    self.push_log("remote SDP offer received".to_string());
                    let answer = match self.state.webrtc.as_mut() {
                        Some(webrtc) => match webrtc.accept_remote_offer(description.sdp) {
                            Ok(signal) => Some(signal),
                            Err(error) => {
                                self.push_log(format!("failed to accept remote offer: {error}"));
                                None
                            }
                        },
                        None => {
                            self.push_log(
                                "cannot process remote offer without transport".to_string(),
                            );
                            None
                        }
                    };
                    if let Some(answer) = answer {
                        self.send_transport_signal(answer);
                        self.flush_local_transport_signals();
                        self.state.stage = SessionStage::NegotiatingWebRtc;
                        self.push_log("local SDP answer created and sent".to_string());
                    }
                }
                SdpType::Answer => {
                    if let Some(webrtc) = self.state.webrtc.as_mut() {
                        if let Err(error) = webrtc.accept_remote_answer(description.sdp) {
                            self.push_log(format!("failed to accept remote answer: {error}"));
                        } else {
                            self.state.last_signaling_message =
                                Some("remote SDP answer received".to_string());
                            self.state.stage = SessionStage::NegotiatingWebRtc;
                            self.push_log("remote SDP answer received and applied".to_string());
                        }
                    }
                }
            },
            SignalingMessage::IceCandidate(candidate) => {
                if let Some(webrtc) = self.state.webrtc.as_mut() {
                    if let Err(error) = webrtc.add_remote_ice_candidate(
                        candidate.candidate,
                        candidate.sdp_mid,
                        candidate.sdp_mline_index,
                    ) {
                        self.push_log(format!("failed to apply remote ICE candidate: {error}"));
                    } else {
                        self.state.last_signaling_message =
                            Some("remote ICE candidate received".to_string());
                        self.state.stage = SessionStage::NegotiatingWebRtc;
                        self.push_log("remote ICE candidate applied".to_string());
                    }
                }
            }
        }
    }

    fn flush_local_transport_signals(&mut self) {
        let signals = match self.state.webrtc.as_mut() {
            Some(webrtc) => webrtc.drain_local_signals(),
            None => return,
        };

        for signal in signals {
            self.send_transport_signal(signal);
        }
    }

    fn send_transport_signal(&mut self, signal: WebRtcSignal) {
        let message = map_transport_signal(signal.clone());
        match self.state.signaling.as_mut() {
            Some(connection) => match connection.send(&message) {
                Ok(()) => {
                    self.state.last_signaling_message = Some(describe_outgoing_signal(&signal));
                }
                Err(error) => {
                    self.state.signaling_connected = false;
                    self.push_log(format!("failed to send signaling message: {error}"));
                }
            },
            None => {
                self.push_log(
                    "cannot send signaling message before signaling is connected".to_string(),
                );
            }
        }
    }

    fn update_stage_from_transport(&mut self) {
        let Some(snapshot) = self.state.webrtc.as_ref().map(|session| session.snapshot()) else {
            return;
        };

        if snapshot.connection_state == "connected" {
            self.state.stage = SessionStage::LiveWebRtc;
            self.state.transport = SessionTransport::LiveWebRtc;
        } else if self.state.transport == SessionTransport::LiveWebRtc
            && snapshot.local_description_ready
        {
            self.state.stage = SessionStage::NegotiatingWebRtc;
        }
    }

    fn push_log(&mut self, message: String) {
        self.state.logs.push(stamp(&message));
        if self.state.logs.len() > 200 {
            let drain = self.state.logs.len() - 200;
            self.state.logs.drain(0..drain);
        }
    }

    fn log_ice_server_configuration(&mut self) {
        if self.state.ice_servers.is_empty() {
            self.push_log("ICE server configuration: none; local-only candidates only".to_string());
        } else {
            self.push_log(format!(
                "ICE server configuration: {}",
                summarize_ice_server_entries(&self.state.ice_servers)
            ));
        }
    }

    fn load_persisted_preferences(&mut self) {
        let persisted = match self.preferences.load() {
            Ok(Some(config)) => config,
            Ok(None) => return,
            Err(error) => {
                self.push_log(format!(
                    "failed to load persisted session preferences: {error}"
                ));
                return;
            }
        };

        self.apply_persisted_preferences(persisted);
        self.push_log("restored persisted session preferences".to_string());
    }

    fn apply_persisted_preferences(&mut self, persisted: PersistedSessionConfig) {
        self.state.room = persisted.room;
        self.state.signaling_addr = persisted.signaling_addr;
        self.state.ice_servers = persisted.ice_servers;
        self.state.source_label = persisted.source_label;
        self.state.capture_selection = persisted.capture_selection;
        self.state.ui_preferences = persisted.ui_preferences;

        if let Some(label) = selected_source_label(
            &self.state.capture_catalog,
            self.state.capture_selection.as_ref(),
        ) {
            self.state.source_label = Some(label);
        }
    }

    fn persist_preferences(&mut self) {
        let persisted = PersistedSessionConfig {
            room: self.state.room.clone(),
            signaling_addr: self.state.signaling_addr.clone(),
            source_label: self.state.source_label.clone(),
            capture_selection: self.state.capture_selection.clone(),
            ice_servers: self.state.ice_servers.clone(),
            ui_preferences: self.state.ui_preferences,
        };

        if let Err(error) = self.preferences.save(&persisted) {
            self.push_log(format!("failed to persist session preferences: {error}"));
        }
    }

    fn recovery_diagnostics(
        &self,
        transport_snapshot: Option<&transport_webrtc::TransportSnapshot>,
    ) -> RecoveryDiagnostics {
        if self.state.mode == SessionMode::Idle {
            return RecoveryDiagnostics {
                state: "idle",
                reason: "Prepare a host or viewer session before reconnect is available."
                    .to_string(),
                can_reconnect: false,
                reconnect_recommended: false,
            };
        }

        if self.state.stage == SessionStage::Stopped {
            return RecoveryDiagnostics {
                state: "stopped",
                reason:
                    "Session is stopped. Reconnect can rebuild the current role with the active configuration."
                        .to_string(),
                can_reconnect: true,
                reconnect_recommended: true,
            };
        }

        if self.state.transport == SessionTransport::LiveWebRtc && !self.state.signaling_connected {
            return RecoveryDiagnostics {
                state: "signaling_unavailable",
                reason:
                    "Signaling is disconnected. Verify the signaling server or address, then reconnect."
                        .to_string(),
                can_reconnect: true,
                reconnect_recommended: true,
            };
        }

        let Some(transport_snapshot) = transport_snapshot else {
            return RecoveryDiagnostics {
                state: "not_initialized",
                reason:
                    "Transport is not initialized yet. Prepare or reconnect the session to build a peer connection."
                        .to_string(),
                can_reconnect: true,
                reconnect_recommended: true,
            };
        };

        match transport_snapshot.connection_state.as_str() {
            "failed" => RecoveryDiagnostics {
                state: "transport_failed",
                reason:
                    "Peer connection entered failed state. Reconnect to rebuild signaling and transport."
                        .to_string(),
                can_reconnect: true,
                reconnect_recommended: true,
            },
            "closed" => RecoveryDiagnostics {
                state: "transport_closed",
                reason:
                    "Peer connection is closed. Reconnect to initialize a fresh transport session."
                        .to_string(),
                can_reconnect: true,
                reconnect_recommended: true,
            },
            "disconnected" => RecoveryDiagnostics {
                state: "transport_disconnected",
                reason:
                    "Peer connection is temporarily disconnected. Keep refreshing or reconnect to force a fresh session."
                        .to_string(),
                can_reconnect: true,
                reconnect_recommended: true,
            },
            "connected" => RecoveryDiagnostics {
                state: "healthy",
                reason: "Peer connection is live.".to_string(),
                can_reconnect: true,
                reconnect_recommended: false,
            },
            _ => {
                let reason = match self.state.mode {
                    SessionMode::Host if !transport_snapshot.remote_description_ready => {
                        "Negotiation is still in progress while waiting for the viewer answer."
                    }
                    SessionMode::Viewer if !transport_snapshot.remote_description_ready => {
                        "Negotiation is still in progress while waiting for the host offer."
                    }
                    _ => "Negotiation is still in progress.",
                };
                RecoveryDiagnostics {
                    state: "negotiating",
                    reason: reason.to_string(),
                    can_reconnect: true,
                    reconnect_recommended: false,
                }
            }
        }
    }

    fn next_action(&self) -> &'static str {
        let transport_snapshot = self.state.webrtc.as_ref().map(|session| session.snapshot());
        let connected = transport_snapshot
            .as_ref()
            .map(|snapshot| snapshot.connection_state == "connected")
            .unwrap_or(false);
        let local_description_ready = transport_snapshot
            .as_ref()
            .map(|snapshot| snapshot.local_description_ready)
            .unwrap_or(false);
        let remote_description_ready = transport_snapshot
            .as_ref()
            .map(|snapshot| snapshot.remote_description_ready)
            .unwrap_or(false);

        match (self.state.mode, self.state.stage, self.state.transport) {
            (SessionMode::Idle, _, _) => "configure host or viewer session",
            (_, _, SessionTransport::MockUdp) => {
                "legacy mock UDP mode is still available for media scaffold work"
            }
            (_, SessionStage::Stopped, _) => "reconnect, restart, or reset session",
            (_, SessionStage::LiveWebRtc, SessionTransport::LiveWebRtc) if connected => {
                "peer connection is live; feed capture samples into the attached tracks or push debug capture samples for transport smoke testing"
            }
            (SessionMode::Host, _, SessionTransport::LiveWebRtc)
                if self.state.capture_catalog.permission_state
                    == CapturePermissionState::Denied =>
            {
                "grant capture permission in the OS and refresh the catalog before relying on real host capture"
            }
            (SessionMode::Host, _, SessionTransport::LiveWebRtc)
                if self.state.capture_catalog.permission_state
                    == CapturePermissionState::Unknown =>
            {
                "verify the desktop session or capture tooling, then refresh the catalog before relying on real host capture"
            }
            (SessionMode::Host, _, SessionTransport::LiveWebRtc)
                if self.state.capture_catalog.permission_state
                    == CapturePermissionState::Required =>
            {
                "grant capture permission or continue with fallback metadata while real host capture is unavailable"
            }
            (SessionMode::Host, _, SessionTransport::LiveWebRtc)
                if self.state.capture_selection.is_none()
                    && !self.state.capture_catalog.sources.is_empty()
                    && !local_description_ready =>
            {
                "choose a capture source before starting the host workflow"
            }
            (SessionMode::Host, _, SessionTransport::LiveWebRtc)
                if !self.state.signaling_connected =>
            {
                "start signaling server or fix the signaling address, then reconnect"
            }
            (SessionMode::Host, _, SessionTransport::LiveWebRtc)
                if transport_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.local_media_track_count)
                    .unwrap_or(0)
                    == 0 =>
            {
                "attach local media tracks before creating the offer"
            }
            (SessionMode::Host, _, SessionTransport::LiveWebRtc) if !local_description_ready => {
                "keep refreshing signaling while the host prepares the local offer"
            }
            (SessionMode::Host, _, SessionTransport::LiveWebRtc) if !remote_description_ready => {
                "wait for viewer answer and keep refreshing signaling"
            }
            (SessionMode::Viewer, _, SessionTransport::LiveWebRtc)
                if !self.state.signaling_connected =>
            {
                "start signaling server or fix the signaling address, then reconnect"
            }
            (SessionMode::Viewer, _, SessionTransport::LiveWebRtc) if !remote_description_ready => {
                "wait for host offer and keep refreshing signaling"
            }
            _ => "keep refreshing signaling until the peer connection is connected",
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::with_preferences(PreferencesStore::discover())
    }
}

fn map_transport_signal(signal: WebRtcSignal) -> SignalingMessage {
    match signal {
        WebRtcSignal::SessionDescription { kind, sdp } => {
            SignalingMessage::SessionDescription(SessionDescription {
                sdp_type: match kind {
                    DescriptionKind::Offer => SdpType::Offer,
                    DescriptionKind::Answer => SdpType::Answer,
                },
                sdp,
            })
        }
        WebRtcSignal::IceCandidate {
            candidate,
            sdp_mid,
            sdp_mline_index,
        } => SignalingMessage::IceCandidate(IceCandidate {
            candidate,
            sdp_mid,
            sdp_mline_index,
        }),
    }
}

fn describe_outgoing_signal(signal: &WebRtcSignal) -> String {
    match signal {
        WebRtcSignal::SessionDescription { kind, .. } => match kind {
            DescriptionKind::Offer => "local SDP offer sent".to_string(),
            DescriptionKind::Answer => "local SDP answer sent".to_string(),
        },
        WebRtcSignal::IceCandidate { .. } => "local ICE candidate sent".to_string(),
    }
}

fn format_description_kind(kind: DescriptionKind) -> String {
    match kind {
        DescriptionKind::Offer => "offer",
        DescriptionKind::Answer => "answer",
    }
    .to_string()
}

fn format_transport_stage(stage: TransportStage) -> String {
    match stage {
        TransportStage::Planned => "planned",
        TransportStage::SignalingReady => "signaling_ready",
        TransportStage::PeerConnecting => "peer_connecting",
        TransportStage::OfferCreated => "offer_created",
        TransportStage::AnswerCreated => "answer_created",
        TransportStage::AnswerAccepted => "answer_accepted",
        TransportStage::Streaming => "streaming",
        TransportStage::Closed => "closed",
    }
    .to_string()
}

fn format_capture_stream_status(status: CaptureStreamStatus) -> &'static str {
    match status {
        CaptureStreamStatus::Starting => "starting",
        CaptureStreamStatus::Running => "running",
        CaptureStreamStatus::Stopped => "stopped",
        CaptureStreamStatus::PermissionRequired => "permission_required",
        CaptureStreamStatus::PermissionDenied => "permission_denied",
        CaptureStreamStatus::Failed => "failed",
    }
}

fn stamp(message: &str) -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    format!("[{seconds}] {message}")
}

fn timestamp_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_micros() as u64)
        .unwrap_or(0)
}

fn debug_video_frame(source_id: &str) -> VideoFrame {
    let width = 64;
    let height = 36;
    let seed = source_id
        .bytes()
        .fold(0u8, |value, byte| value.wrapping_add(byte));
    let mut bytes = Vec::with_capacity((width * height * 4) as usize);

    for index in 0..(width * height) {
        let channel = (index % 255) as u8;
        bytes.extend_from_slice(&[
            seed.wrapping_add(channel),
            seed.wrapping_add(channel / 2),
            seed.wrapping_sub(channel / 3),
            0xFF,
        ]);
    }

    VideoFrame {
        format: VideoPixelFormat::Bgra8,
        width,
        height,
        timestamp_micros: timestamp_micros(),
        bytes,
    }
}

fn debug_audio_buffer() -> AudioBuffer {
    let sample_rate_hz = 48_000;
    let channels = 2;
    let frames = 960;
    let mut samples = Vec::with_capacity((frames * u32::from(channels)) as usize);

    for index in 0..frames {
        let wave = (((index % 48) as f32) / 24.0) - 1.0;
        samples.push(wave * 0.05);
        samples.push((-wave) * 0.05);
    }

    AudioBuffer {
        sample_rate_hz,
        channels,
        frames,
        timestamp_micros: timestamp_micros(),
        samples,
    }
}

#[cfg(test)]
mod tests {
    use super::{SessionIntent, SessionManager, SessionMode, SessionStage, SessionTransport};
    use crate::ice_servers::IceServerEntry;
    use crate::preferences::{PreferencesStore, UiPreferences};
    use crate::protocol::{
        PeerAnnouncement, Role, decode_signaling_message, encode_peer, encode_waiting,
        parse_join_request,
    };
    use crate::signaling::SignalingConnection;
    use capture_core::{CaptureSource, CaptureSourceKind, CaptureStreamEvent, CaptureStreamStatus};
    use std::collections::HashMap;
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{Ipv4Addr, SocketAddr};
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn host_session_updates_state() {
        let (mut manager, config_dir) = new_test_manager("host-session-state");
        let snapshot = manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
            ice_servers: Vec::new(),
        });

        assert_eq!(snapshot.mode, SessionMode::Host);
        assert!(matches!(
            snapshot.stage,
            SessionStage::Configured | SessionStage::NegotiatingWebRtc | SessionStage::LiveWebRtc
        ));
        assert_eq!(snapshot.transport, SessionTransport::LiveWebRtc);
        assert_eq!(snapshot.room.as_deref(), Some("demo"));
        assert_eq!(snapshot.source_label.as_deref(), Some("vlc"));
        assert!(matches!(
            snapshot.capture_permission_state.as_str(),
            "required" | "granted" | "denied" | "unknown"
        ));
        assert!(snapshot.available_source_count >= 1);
        assert_eq!(snapshot.local_media_track_count, 2);
        assert!(snapshot.local_video_track_attached);
        assert!(snapshot.local_audio_track_attached);
        assert!(snapshot.transport_state == "new" || snapshot.transport_state == "not_initialized");

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn host_start_auto_selects_default_capture_source_when_none_chosen() {
        let (mut manager, config_dir) = new_test_manager("host-default-source");

        let snapshot = manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: None,
            ice_servers: Vec::new(),
        });

        assert!(snapshot.selected_source_id.is_some());
        assert!(snapshot.source_label.is_some());
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("default capture source selected for host"))
        );

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn viewer_session_updates_state() {
        let (mut manager, config_dir) = new_test_manager("viewer-session-state");
        let snapshot = manager.start_viewer(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: None,
            ice_servers: Vec::new(),
        });

        assert_eq!(snapshot.mode, SessionMode::Viewer);
        assert!(matches!(
            snapshot.stage,
            SessionStage::AwaitingPeer | SessionStage::NegotiatingWebRtc | SessionStage::LiveWebRtc
        ));
        assert_eq!(snapshot.transport, SessionTransport::LiveWebRtc);
        assert_eq!(snapshot.room.as_deref(), Some("demo"));
        assert_eq!(snapshot.signaling_addr.as_deref(), Some("127.0.0.1:7000"));
        assert_eq!(snapshot.local_media_track_count, 0);
        assert!(!snapshot.local_video_track_attached);
        assert!(!snapshot.local_audio_track_attached);
        assert!(matches!(
            snapshot.next_action.as_str(),
            "start signaling server or fix the signaling address, then reconnect"
                | "wait for host offer or refresh signaling"
                | "wait for host offer and keep refreshing signaling"
                | "keep refreshing signaling until the peer connection is connected"
        ));

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn selecting_capture_source_updates_source_label() {
        let (mut manager, config_dir) = new_test_manager("select-source-label");

        let (source, _requested_audio, snapshot) =
            select_source_with_retry(&mut manager, |source| {
                Some((source.clone(), source.has_audio))
            });
        let label = source.label();
        assert_eq!(
            snapshot.selected_source_id.as_deref(),
            Some(source.id.as_str())
        );
        assert_eq!(snapshot.selected_source_audio, source.has_audio);
        assert_eq!(snapshot.source_label.as_deref(), Some(label.as_str()));

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn publish_debug_capture_requires_selected_source() {
        let (mut manager, config_dir) = new_test_manager("publish-debug-no-source");

        let snapshot = manager.publish_debug_capture_samples();

        assert_eq!(snapshot.published_video_sample_count, 0);
        assert_eq!(snapshot.published_audio_sample_count, 0);
        assert!(snapshot.logs.iter().any(|line| {
            line.contains("cannot publish debug capture samples before selecting a capture source")
        }));

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn host_rebind_helper_uses_first_available_source() {
        let mut manager = SessionManager::new();
        manager.state.capture_catalog.sources = vec![CaptureSource {
            id: "replacement-source".to_string(),
            kind: CaptureSourceKind::Window,
            display_name: "Replacement".to_string(),
            app_name: Some("VLC".to_string()),
            has_audio: true,
        }];
        manager.state.capture_selection = None;

        let rebound = manager.select_first_available_host_capture_source(
            Some(false),
            "host capture source rebound after catalog refresh",
        );

        assert!(rebound);
        let snapshot = manager.snapshot();
        assert_eq!(
            snapshot.selected_source_id.as_deref(),
            Some("replacement-source")
        );
        assert!(!snapshot.selected_source_audio);
        assert_eq!(snapshot.source_label.as_deref(), Some("VLC - Replacement"));
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("host capture source rebound after catalog refresh"))
        );
    }

    #[test]
    fn stop_keeps_history() {
        let mut manager = SessionManager::new();
        manager.start_viewer(SessionIntent {
            room: "join".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: None,
            ice_servers: Vec::new(),
        });
        let snapshot = manager.stop();

        assert_eq!(snapshot.stage, SessionStage::Stopped);
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("session stopped"))
        );
        assert_eq!(snapshot.next_action, "reconnect, restart, or reset session");
        assert_eq!(snapshot.recovery_state, "stopped");
        assert!(snapshot.can_reconnect);
        assert!(snapshot.reconnect_recommended);
    }

    #[test]
    fn reconnect_rejects_idle_session() {
        let mut manager = SessionManager::new();

        let error = manager
            .reconnect()
            .expect_err("idle session should not reconnect");
        assert_eq!(
            error,
            "cannot reconnect before preparing a host or viewer session"
        );
    }

    #[test]
    fn stopped_host_session_can_reconnect_with_existing_config() {
        let (mut manager, config_dir) = new_test_manager("reconnect-stopped-host");
        manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
            ice_servers: Vec::new(),
        });
        manager.stop();

        let snapshot = manager.reconnect().expect("reconnect host");

        assert_eq!(snapshot.mode, SessionMode::Host);
        assert_eq!(snapshot.room.as_deref(), Some("demo"));
        assert_eq!(snapshot.signaling_addr.as_deref(), Some("127.0.0.1:7000"));
        assert_eq!(snapshot.transport, SessionTransport::LiveWebRtc);
        assert_eq!(snapshot.local_media_track_count, 2);
        assert!(snapshot.logs.iter().any(|line| {
            line.contains("reconnecting host session for room=demo signaling=127.0.0.1:7000")
        }));

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn stopped_viewer_session_reconnects_with_updated_config() {
        let (mut manager, config_dir) = new_test_manager("reconnect-stopped-viewer");
        manager.start_viewer(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: None,
            ice_servers: Vec::new(),
        });
        manager.stop();
        manager.update_config(
            Some("second-room".to_string()),
            Some("127.0.0.1:7200".to_string()),
            None,
            Some(Vec::new()),
        );

        let snapshot = manager.reconnect().expect("reconnect viewer");

        assert_eq!(snapshot.mode, SessionMode::Viewer);
        assert_eq!(snapshot.room.as_deref(), Some("second-room"));
        assert_eq!(snapshot.signaling_addr.as_deref(), Some("127.0.0.1:7200"));
        assert_eq!(snapshot.transport, SessionTransport::LiveWebRtc);
        assert!(snapshot.logs.iter().any(|line| line.contains(
            "reconnecting viewer session for room=second-room signaling=127.0.0.1:7200"
        )));

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn reconnect_uses_updated_ice_server_configuration() {
        let (mut manager, config_dir) = new_test_manager("reconnect-ice-servers");
        manager.start_viewer(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: None,
            ice_servers: Vec::new(),
        });
        manager.stop();

        manager.update_config(
            None,
            None,
            None,
            Some(vec![
                IceServerEntry {
                    urls: vec!["stun:stun.example.com:3478".to_string()],
                    username: None,
                    credential: None,
                },
                IceServerEntry {
                    urls: vec!["turn:turn.example.com:3478?transport=udp".to_string()],
                    username: Some("viewer".to_string()),
                    credential: Some("secret".to_string()),
                },
            ]),
        );

        let snapshot = manager.reconnect().expect("reconnect viewer");

        assert_eq!(snapshot.ice_server_count, 2);
        assert!(snapshot.ice_servers.contains("stun:stun.example.com:3478"));
        assert!(
            snapshot
                .ice_servers
                .contains("turn:turn.example.com:3478?transport=udp|viewer|secret")
        );
        assert!(snapshot.ice_server_summary.contains("auth user viewer"));
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("ICE server configuration:")
                    && line.contains("auth user viewer"))
        );

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn host_session_can_publish_debug_capture_media() {
        let (mut manager, config_dir) = new_test_manager("debug-capture-publish");
        manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
            ice_servers: Vec::new(),
        });

        let before = manager.snapshot();
        let snapshot = publish_debug_samples_with_audio_enabled(&mut manager);
        assert_eq!(
            snapshot.published_video_sample_count,
            before.published_video_sample_count + 1
        );
        assert_eq!(
            snapshot.published_audio_sample_count,
            before.published_audio_sample_count + 1
        );
        assert_eq!(snapshot.last_video_sample_bytes, 64 * 36 * 4);
        assert_eq!(snapshot.last_audio_sample_bytes, 960 * 2 * 4);
        assert!(
            snapshot
                .last_video_capture_summary
                .as_deref()
                .is_some_and(|summary| summary.starts_with("bgra8 64x36 @ "))
        );
        assert!(
            snapshot
                .last_audio_capture_summary
                .as_deref()
                .is_some_and(|summary| summary.starts_with("48000Hz 2ch 960f @ "))
        );
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("capture video frame published for source="))
        );
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("capture audio buffer published for source="))
        );

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn debug_capture_publish_respects_audio_toggle() {
        let (mut manager, config_dir) = new_test_manager("debug-capture-audio-toggle");
        manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
            ice_servers: Vec::new(),
        });

        let snapshot = publish_debug_samples_with_audio_disabled(&mut manager);
        assert_eq!(snapshot.published_video_sample_count, 1);
        assert_eq!(snapshot.published_audio_sample_count, 0);
        assert!(snapshot.last_audio_capture_summary.is_none());
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("audio skipped by capture selection"))
        );

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn host_session_ingests_capture_stream_media_events() {
        let (mut manager, config_dir) = new_test_manager("capture-stream-events");
        let (source, _requested_audio, _snapshot) =
            select_source_with_retry(&mut manager, |source| {
                source.has_audio.then_some((source, true))
            });

        manager.select_capture_source(source.id.clone(), true);
        let host_snapshot = manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
            ice_servers: Vec::new(),
        });
        let source_id = host_snapshot
            .selected_source_id
            .expect("host source should be selected");

        manager.ingest_capture_stream_event(CaptureStreamEvent::Started {
            source_id: source_id.clone(),
        });
        let after_video = manager.ingest_capture_stream_event(CaptureStreamEvent::VideoFrame {
            source_id: source_id.clone(),
            frame: super::debug_video_frame(&source_id),
        });
        let after_audio = manager.ingest_capture_stream_event(CaptureStreamEvent::AudioBuffer {
            source_id: source_id.clone(),
            buffer: super::debug_audio_buffer(),
        });

        assert_eq!(after_video.published_video_sample_count, 1);
        assert_eq!(after_audio.published_audio_sample_count, 1);
        assert!(
            after_audio
                .logs
                .iter()
                .any(|line| line.contains("capture stream started for source="))
        );

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn host_session_can_start_and_poll_native_capture_runtime() {
        let (mut manager, config_dir) = new_test_manager("native-capture-runtime");
        let (source, _requested_audio, _snapshot) =
            select_source_with_retry(&mut manager, |source| {
                Some((source.clone(), source.has_audio))
            });

        manager.select_capture_source(source.id.clone(), source.has_audio);
        manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
            ice_servers: Vec::new(),
        });

        let started = manager
            .start_capture_stream()
            .expect("platform runtime should be available on supported test targets");
        let polled = manager.poll_capture_stream();

        assert!(
            started
                .logs
                .iter()
                .any(|line| { line.contains("native capture stream start requested for source=") })
        );
        assert!(started.logs.iter().any(|line| {
            line.contains("capture stream status changed for source=")
                && (line.contains("permission_required") || line.contains("permission_denied"))
        }));
        assert!(polled.selected_source_id.is_some());
        assert_ne!(started.capture_runtime_status, "not_started");
        assert_ne!(polled.capture_runtime_status, "not_started");

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn native_capture_runtime_rejects_viewer_sessions() {
        let (mut manager, config_dir) = new_test_manager("native-capture-viewer-rejected");
        manager.start_viewer(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: None,
            ice_servers: Vec::new(),
        });

        let error = manager
            .start_capture_stream()
            .expect_err("viewer sessions must not publish host capture");

        assert_eq!(
            error,
            "native capture stream can only be started from a host session"
        );

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn capture_stream_status_and_error_events_are_logged() {
        let (mut manager, config_dir) = new_test_manager("capture-stream-status-events");

        let status_snapshot =
            manager.ingest_capture_stream_event(CaptureStreamEvent::StatusChanged {
                source_id: Some("source-1".to_string()),
                status: CaptureStreamStatus::PermissionRequired,
                message: Some("screen recording approval is required".to_string()),
            });
        let error_snapshot = manager.ingest_capture_stream_event(CaptureStreamEvent::Error {
            source_id: Some("source-1".to_string()),
            message: "capture backend disconnected".to_string(),
        });

        assert!(status_snapshot.logs.iter().any(|line| {
            line.contains("capture stream status changed for source=source-1")
                && line.contains("permission_required")
        }));
        assert!(error_snapshot.logs.iter().any(|line| {
            line.contains("capture stream error for source=source-1")
                && line.contains("capture backend disconnected")
        }));

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn capture_publish_rejects_mismatched_source_id() {
        let (mut manager, config_dir) = new_test_manager("capture-source-mismatch");
        let (source, _requested_audio, _snapshot) =
            select_source_with_retry(&mut manager, |source| {
                Some((source.clone(), source.has_audio))
            });

        manager.select_capture_source(source.id.clone(), source.has_audio);
        manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
            ice_servers: Vec::new(),
        });

        let snapshot = manager.publish_capture_video_frame(
            "different-source".to_string(),
            super::debug_video_frame("different-source"),
        );

        assert_eq!(snapshot.published_video_sample_count, 0);
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("capture media source mismatch"))
        );

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn capture_publish_rejects_audio_when_selection_disables_it() {
        let (mut manager, config_dir) = new_test_manager("capture-audio-disabled");

        for _ in 0..5 {
            let _ = manager.reset();
            let (_source, _requested_audio, selected_snapshot) =
                select_source_with_retry(&mut manager, |source| {
                    source.has_audio.then_some((source, false))
                });
            assert!(!selected_snapshot.selected_source_audio);

            let host_snapshot = manager.start_host(SessionIntent {
                room: "demo".to_string(),
                signaling_addr: "127.0.0.1:7000".to_string(),
                source_label: Some("vlc".to_string()),
                ice_servers: Vec::new(),
            });

            let Some(selected_source_id) = host_snapshot.selected_source_id.clone() else {
                continue;
            };
            if host_snapshot.selected_source_audio {
                continue;
            }

            let snapshot = manager
                .publish_capture_audio_buffer(selected_source_id, super::debug_audio_buffer());

            if snapshot.published_audio_sample_count == 0 {
                assert!(snapshot.logs.iter().any(|line| {
                    line.contains("capture audio buffer ignored because audio is disabled")
                }));
                let _ = fs::remove_dir_all(config_dir);
                return;
            }
        }

        panic!("failed to keep audio disabled while preparing the host session");
    }

    #[test]
    fn host_refresh_auto_creates_offer_after_signaling_connects() {
        let server = TestSignalingServer::new();
        let mut manager = SessionManager::new();
        let snapshot = manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "in-memory-signaling".to_string(),
            source_label: Some("vlc".to_string()),
            ice_servers: Vec::new(),
        });

        assert!(!snapshot.local_offer_ready);
        attach_test_signaling(&mut manager, &server, "demo", Role::Sender, 4100);

        let refreshed = manager.refresh();
        assert!(refreshed.local_offer_ready);
        assert_eq!(refreshed.local_description_kind.as_deref(), Some("offer"));
        assert!(
            refreshed
                .logs
                .iter()
                .any(|line| line.contains("local SDP offer created and sent automatically"))
        );
    }

    #[test]
    fn reset_returns_idle_state() {
        let mut manager = SessionManager::new();
        manager.mark_mock_streaming("127.0.0.1:9999".to_string());
        let snapshot = manager.reset();

        assert_eq!(snapshot.mode, SessionMode::Idle);
        assert_eq!(snapshot.stage, SessionStage::Idle);
        assert_eq!(snapshot.next_action, "configure host or viewer session");
        assert_eq!(snapshot.recovery_state, "idle");
        assert_eq!(snapshot.capture_runtime_status, "not_started");
        assert!(!snapshot.can_reconnect);
        assert!(!snapshot.reconnect_recommended);
    }

    #[test]
    fn disconnected_signaling_marks_reconnect_as_recommended() {
        let mut manager = SessionManager::new();
        manager.state.mode = SessionMode::Viewer;
        manager.state.stage = SessionStage::AwaitingPeer;
        manager.state.transport = SessionTransport::LiveWebRtc;
        manager.state.room = Some("demo".to_string());
        manager.state.signaling_addr = Some("127.0.0.1:7000".to_string());
        manager.state.signaling_connected = false;

        let snapshot = manager.snapshot();

        assert_eq!(snapshot.recovery_state, "signaling_unavailable");
        assert!(snapshot.can_reconnect);
        assert!(snapshot.reconnect_recommended);
        assert_eq!(
            snapshot.recovery_reason,
            "Signaling is disconnected. Verify the signaling server or address, then reconnect."
        );
    }

    #[test]
    fn session_manager_restores_persisted_preferences() {
        let config_dir = unique_temp_dir("session-prefs");
        let mut first =
            SessionManager::new_for_tests(PreferencesStore::from_config_dir(config_dir.clone()));
        let source = first
            .capture_catalog()
            .sources
            .first()
            .cloned()
            .expect("at least one source");

        first.update_config(
            Some("saved-room".to_string()),
            Some("127.0.0.1:7100".to_string()),
            None,
            Some(Vec::new()),
        );
        first.select_capture_source(source.id.clone(), false);

        let restored =
            SessionManager::new_for_tests(PreferencesStore::from_config_dir(config_dir.clone()));
        let snapshot = restored.snapshot();

        assert_eq!(snapshot.room.as_deref(), Some("saved-room"));
        assert_eq!(snapshot.signaling_addr.as_deref(), Some("127.0.0.1:7100"));
        assert!(snapshot.ui_auto_refresh_enabled);
        assert_eq!(snapshot.ui_refresh_interval_secs, 3);
        if let Some(selected_source_id) = snapshot.selected_source_id.as_deref() {
            assert_eq!(selected_source_id, source.id.as_str());
            assert!(!snapshot.selected_source_audio);
        }
        if let Some(source_label) = snapshot.source_label.as_deref() {
            assert_eq!(source_label, source.label().as_str());
        }
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("restored persisted session preferences"))
        );

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn session_manager_persists_ui_preferences() {
        let config_dir = unique_temp_dir("session-ui-prefs");
        let mut first =
            SessionManager::new_for_tests(PreferencesStore::from_config_dir(config_dir.clone()));

        let snapshot = first
            .update_ui_preferences(Some(false), Some(10))
            .expect("update ui preferences");
        assert!(!snapshot.ui_auto_refresh_enabled);
        assert_eq!(snapshot.ui_refresh_interval_secs, 10);

        let restored =
            SessionManager::new_for_tests(PreferencesStore::from_config_dir(config_dir.clone()));
        let snapshot = restored.snapshot();
        assert!(!snapshot.ui_auto_refresh_enabled);
        assert_eq!(snapshot.ui_refresh_interval_secs, 10);

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn session_manager_rejects_zero_refresh_interval() {
        let (mut manager, _config_dir) = new_test_manager("session-ui-interval");

        let error = manager
            .update_ui_preferences(None, Some(0))
            .expect_err("zero interval should be rejected");
        assert_eq!(error, "refresh interval must be greater than zero seconds");

        let snapshot = manager.snapshot();
        assert_eq!(
            snapshot.ui_refresh_interval_secs,
            UiPreferences::default().refresh_interval_secs
        );
    }

    #[test]
    fn reset_restores_persisted_preferences_after_runtime_changes() {
        let config_dir = unique_temp_dir("session-reset");
        let mut manager =
            SessionManager::new_for_tests(PreferencesStore::from_config_dir(config_dir.clone()));
        let source = manager
            .capture_catalog()
            .sources
            .first()
            .cloned()
            .expect("at least one source");

        manager.update_config(
            Some("saved-room".to_string()),
            Some("127.0.0.1:7200".to_string()),
            None,
            Some(Vec::new()),
        );
        manager.select_capture_source(source.id.clone(), source.has_audio);
        manager.start_host(SessionIntent {
            room: "transient-room".to_string(),
            signaling_addr: "127.0.0.1:7300".to_string(),
            source_label: Some("temporary".to_string()),
            ice_servers: Vec::new(),
        });

        let snapshot = manager.reset();
        assert_eq!(snapshot.mode, SessionMode::Idle);
        assert_eq!(snapshot.stage, SessionStage::Idle);
        assert_eq!(snapshot.room.as_deref(), Some("saved-room"));
        assert_eq!(snapshot.signaling_addr.as_deref(), Some("127.0.0.1:7200"));
        if let Some(selected_source_id) = snapshot.selected_source_id.as_deref() {
            assert_eq!(selected_source_id, source.id.as_str());
        }

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn late_join_replay_drives_host_and_viewer_to_negotiated_webrtc() {
        let server = TestSignalingServer::new();

        let (mut host, host_config_dir) = new_test_manager("late-join-host");
        let host_start = host.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "in-memory-signaling".to_string(),
            source_label: Some("vlc".to_string()),
            ice_servers: Vec::new(),
        });
        assert!(!host_start.signaling_connected);
        attach_test_signaling(&mut host, &server, "demo", Role::Sender, 4100);
        let host_offer = host.refresh();
        assert!(host_offer.local_offer_ready);
        assert_eq!(host_offer.local_description_kind.as_deref(), Some("offer"));

        thread::sleep(Duration::from_millis(150));

        let (mut viewer, viewer_config_dir) = new_test_manager("late-join-viewer");
        let viewer_start = viewer.start_viewer(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "in-memory-signaling".to_string(),
            source_label: None,
            ice_servers: Vec::new(),
        });
        assert!(!viewer_start.signaling_connected);
        attach_test_signaling(&mut viewer, &server, "demo", Role::Receiver, 4200);

        let (host_snapshot, viewer_snapshot) =
            drive_sessions_until_viewer_answers(&mut host, &mut viewer, Duration::from_secs(10));

        assert!(matches!(
            host_snapshot.stage,
            SessionStage::NegotiatingWebRtc | SessionStage::LiveWebRtc
        ));
        assert!(matches!(
            viewer_snapshot.stage,
            SessionStage::NegotiatingWebRtc | SessionStage::LiveWebRtc
        ));
        assert!(host_snapshot.local_offer_ready);
        assert!(viewer_snapshot.local_description_ready);
        assert_eq!(host_snapshot.local_media_track_count, 2);
        assert_eq!(viewer_snapshot.local_media_track_count, 0);
        assert_eq!(
            viewer_snapshot.remote_description_kind.as_deref(),
            Some("offer")
        );
        assert!(
            viewer_snapshot
                .logs
                .iter()
                .any(|line| line.contains("remote SDP offer received"))
        );
        assert!(
            host_snapshot
                .logs
                .iter()
                .any(|line| line.contains("peer discovered via signaling"))
        );

        let _ = fs::remove_dir_all(host_config_dir);
        let _ = fs::remove_dir_all(viewer_config_dir);
    }

    fn attach_test_signaling(
        manager: &mut SessionManager,
        server: &TestSignalingServer,
        room: &str,
        role: Role,
        udp_port: u16,
    ) {
        manager.state.signaling = Some(server.connect(room, role, udp_port));
        manager.state.signaling_connected = true;
        manager.process_signaling_events();
    }

    fn drive_sessions_until_viewer_answers(
        host: &mut SessionManager,
        viewer: &mut SessionManager,
        timeout: Duration,
    ) -> (super::SessionSnapshot, super::SessionSnapshot) {
        let started = Instant::now();
        loop {
            let host_snapshot = host.refresh();
            let viewer_snapshot = viewer.refresh();

            if viewer_snapshot.remote_description_kind.as_deref() == Some("offer")
                && viewer_snapshot.local_description_kind.as_deref() == Some("answer")
                && host_snapshot.active_peer.is_some()
                && viewer_snapshot.active_peer.is_some()
            {
                return (host_snapshot, viewer_snapshot);
            }

            assert!(
                started.elapsed() < timeout,
                "timed out waiting for viewer answer.\nhost={host_snapshot:#?}\nviewer={viewer_snapshot:#?}"
            );
            thread::sleep(Duration::from_millis(50));
        }
    }

    #[derive(Clone)]
    struct TestParticipant {
        role: Role,
        addr: SocketAddr,
        writer: Arc<Mutex<UnixStream>>,
    }

    #[derive(Clone)]
    struct StoredSignal {
        from_role: Role,
        message: String,
    }

    #[derive(Default)]
    struct TestRoom {
        sender: Option<TestParticipant>,
        receiver: Option<TestParticipant>,
        signaling_history: Vec<StoredSignal>,
    }

    type SharedRooms = Arc<Mutex<HashMap<String, TestRoom>>>;

    struct TestSignalingServer {
        rooms: SharedRooms,
    }

    impl TestSignalingServer {
        fn new() -> Self {
            Self {
                rooms: Arc::new(Mutex::new(HashMap::<String, TestRoom>::new())),
            }
        }

        fn connect(&self, room: &str, role: Role, udp_port: u16) -> SignalingConnection {
            let (client, server) = UnixStream::pair().expect("create in-memory signaling pair");
            let rooms = Arc::clone(&self.rooms);
            thread::spawn(move || {
                handle_test_client(server, rooms);
            });

            SignalingConnection::from_unix_stream_for_tests(client, room, role, udp_port)
                .expect("create test signaling connection")
        }
    }

    fn handle_test_client(stream: UnixStream, rooms: SharedRooms) {
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut first_line = String::new();
        reader
            .read_line(&mut first_line)
            .expect("read join request");
        let request = parse_join_request(first_line.trim()).expect("valid join request");

        let room_name = request.room.clone();
        let participant = TestParticipant {
            role: request.role,
            addr: SocketAddr::from((Ipv4Addr::LOCALHOST, request.udp_port)),
            writer: Arc::new(Mutex::new(stream)),
        };
        register_test_participant(&room_name, participant.clone(), &rooms);

        loop {
            let mut line = String::new();
            let bytes = match reader.read_line(&mut line) {
                Ok(bytes) => bytes,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::UnexpectedEof
                            | std::io::ErrorKind::BrokenPipe
                    ) =>
                {
                    unregister_test_participant(&room_name, participant.role, &rooms);
                    return;
                }
                Err(error) => panic!("read signaling line: {error}"),
            };
            if bytes == 0 {
                unregister_test_participant(&room_name, participant.role, &rooms);
                return;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            decode_signaling_message(trimmed).expect("valid signaling message");
            relay_test_signaling_message(
                &room_name,
                participant.role,
                normalize_message(&line),
                &rooms,
            );
        }
    }

    fn register_test_participant(
        room_name: &str,
        participant: TestParticipant,
        rooms: &SharedRooms,
    ) {
        let mut writes = Vec::new();
        {
            let mut rooms = rooms.lock().expect("rooms poisoned");
            let room = rooms.entry(room_name.to_string()).or_default();
            let slot = participant_slot(room, participant.role);
            assert!(slot.is_none(), "role already occupied in room");
            *slot = Some(participant.clone());

            if let Some(opposite) = participant_for_role(room, participant.role.opposite()).cloned()
            {
                writes.push((
                    participant.writer.clone(),
                    encode_peer(&PeerAnnouncement {
                        role: opposite.role,
                        addr: opposite.addr,
                    }),
                ));
                writes.push((
                    opposite.writer.clone(),
                    encode_peer(&PeerAnnouncement {
                        role: participant.role,
                        addr: participant.addr,
                    }),
                ));

                for stored in room
                    .signaling_history
                    .iter()
                    .filter(|stored| stored.from_role == participant.role.opposite())
                {
                    writes.push((participant.writer.clone(), stored.message.clone()));
                }
            } else {
                writes.push((participant.writer.clone(), encode_waiting()));
            }
        }

        apply_test_writes(writes);
    }

    fn relay_test_signaling_message(
        room_name: &str,
        from_role: Role,
        message: String,
        rooms: &SharedRooms,
    ) {
        let mut writes = Vec::new();
        {
            let mut rooms = rooms.lock().expect("rooms poisoned");
            let room = rooms.get_mut(room_name).expect("room must exist");
            room.signaling_history.push(StoredSignal {
                from_role,
                message: message.clone(),
            });

            if let Some(participant) = participant_for_role(room, from_role.opposite()) {
                writes.push((participant.writer.clone(), message));
            }
        }

        apply_test_writes(writes);
    }

    fn unregister_test_participant(room_name: &str, role: Role, rooms: &SharedRooms) {
        let mut rooms = rooms.lock().expect("rooms poisoned");
        let Some(room) = rooms.get_mut(room_name) else {
            return;
        };

        *participant_slot(room, role) = None;
        room.signaling_history
            .retain(|stored| stored.from_role != role);
        if room.sender.is_none() && room.receiver.is_none() {
            rooms.remove(room_name);
        }
    }

    fn participant_slot(room: &mut TestRoom, role: Role) -> &mut Option<TestParticipant> {
        match role {
            Role::Sender => &mut room.sender,
            Role::Receiver => &mut room.receiver,
        }
    }

    fn participant_for_role(room: &TestRoom, role: Role) -> Option<&TestParticipant> {
        match role {
            Role::Sender => room.sender.as_ref(),
            Role::Receiver => room.receiver.as_ref(),
        }
    }

    fn apply_test_writes(writes: Vec<(Arc<Mutex<UnixStream>>, String)>) {
        for (writer, message) in writes {
            let mut writer = writer.lock().expect("writer poisoned");
            writer
                .write_all(message.as_bytes())
                .expect("write signaling message");
            writer.flush().expect("flush signaling message");
        }
    }

    fn normalize_message(message: &str) -> String {
        if message.ends_with('\n') {
            message.to_string()
        } else {
            format!("{message}\n")
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn new_test_manager(prefix: &str) -> (SessionManager, PathBuf) {
        let config_dir = unique_temp_dir(prefix);
        (
            SessionManager::new_for_tests(PreferencesStore::from_config_dir(config_dir.clone())),
            config_dir,
        )
    }

    fn select_source_with_retry(
        manager: &mut SessionManager,
        predicate: impl Fn(CaptureSource) -> Option<(CaptureSource, bool)>,
    ) -> (CaptureSource, bool, super::SessionSnapshot) {
        for _ in 0..5 {
            let maybe_source = manager
                .capture_catalog()
                .sources
                .iter()
                .cloned()
                .find_map(&predicate);
            let Some((source, requested_audio)) = maybe_source else {
                continue;
            };
            let expected_audio = requested_audio && source.has_audio;
            let snapshot = manager.select_capture_source(source.id.clone(), requested_audio);
            if snapshot.selected_source_id.as_deref() == Some(source.id.as_str())
                && snapshot.selected_source_audio == expected_audio
            {
                return (source, requested_audio, snapshot);
            }
        }

        panic!("failed to select a stable capture source during test");
    }

    fn publish_debug_samples_with_audio_disabled(
        manager: &mut SessionManager,
    ) -> super::SessionSnapshot {
        for _ in 0..5 {
            let before = manager.snapshot();
            let (_source, _requested_audio, selected_snapshot) =
                select_source_with_retry(manager, |source| {
                    source.has_audio.then_some((source, false))
                });
            assert!(!selected_snapshot.selected_source_audio);

            let snapshot = manager.publish_debug_capture_samples();
            if snapshot.published_video_sample_count > before.published_video_sample_count
                && snapshot.published_audio_sample_count == before.published_audio_sample_count
            {
                return snapshot;
            }
        }

        panic!("failed to publish debug capture samples with audio disabled");
    }

    fn publish_debug_samples_with_audio_enabled(
        manager: &mut SessionManager,
    ) -> super::SessionSnapshot {
        for _ in 0..5 {
            let before = manager.snapshot();
            let (_source, _requested_audio, selected_snapshot) =
                select_source_with_retry(manager, |source| {
                    source.has_audio.then_some((source, true))
                });
            assert!(selected_snapshot.selected_source_audio);

            let snapshot = manager.publish_debug_capture_samples();
            if snapshot.published_video_sample_count > before.published_video_sample_count
                && snapshot.published_audio_sample_count > before.published_audio_sample_count
            {
                return snapshot;
            }
        }

        panic!("failed to publish debug capture samples with audio enabled");
    }
}
