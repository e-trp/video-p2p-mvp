use crate::capture_catalog::{
    CaptureCatalogSnapshot, current_capture_catalog, describe_permission_state,
    selected_source_label,
};
use crate::preferences::{PersistedSessionConfig, PreferencesStore};
use crate::protocol::{
    IceCandidate, PeerAnnouncement, Role, SdpType, SessionDescription, SignalingMessage,
};
use crate::signaling::{SignalingConnection, SignalingEvent};
use capture_core::{
    AudioBuffer, CapturePermissionState, CaptureSelection, VideoFrame, VideoPixelFormat,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub mode: SessionMode,
    pub stage: SessionStage,
    pub transport: SessionTransport,
    pub room: Option<String>,
    pub signaling_addr: Option<String>,
    pub source_label: Option<String>,
    pub selected_source_id: Option<String>,
    pub selected_source_audio: bool,
    pub capture_backend: String,
    pub capture_permission_state: String,
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
    pub transport_notes: Vec<String>,
    pub local_offer_ready: bool,
    pub remote_answer_ready: bool,
    pub local_candidate_count: usize,
    pub remote_candidate_count: usize,
    pub last_signaling_message: Option<String>,
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
    source_label: Option<String>,
    capture_catalog: CaptureCatalogSnapshot,
    capture_selection: Option<CaptureSelection>,
    active_peer: Option<String>,
    logs: Vec<String>,
    webrtc: Option<TransportSession>,
    signaling: Option<SignalingConnection>,
    signaling_connected: bool,
    last_signaling_message: Option<String>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            mode: SessionMode::Idle,
            stage: SessionStage::Idle,
            transport: SessionTransport::MockUdp,
            room: None,
            signaling_addr: None,
            source_label: None,
            capture_catalog: current_capture_catalog(),
            capture_selection: None,
            active_peer: None,
            logs: vec![stamp("session manager initialized")],
            webrtc: None,
            signaling: None,
            signaling_connected: false,
            last_signaling_message: None,
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
        self.persist_preferences();
        self.snapshot()
    }

    pub fn capture_catalog(&mut self) -> CaptureCatalogSnapshot {
        self.sync_capture_catalog();
        self.state.capture_catalog.clone()
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
        self.ensure_host_offer("local SDP offer created and sent automatically");
        self.flush_local_transport_signals();
        self.update_stage_from_transport();
        self.snapshot()
    }

    pub fn publish_capture_video_frame(
        &mut self,
        source_id: String,
        frame: VideoFrame,
    ) -> SessionSnapshot {
        self.sync_capture_catalog();

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
        self.sync_capture_catalog();

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

    pub fn stop(&mut self) -> SessionSnapshot {
        if let Some(webrtc) = self.state.webrtc.as_mut() {
            if let Err(error) = webrtc.close() {
                self.push_log(format!("failed to close WebRTC transport: {error}"));
            }
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
        self.state.source_label = intent.source_label;
        self.state.active_peer = None;
        self.state.webrtc = None;
        self.state.signaling = None;
        self.state.signaling_connected = false;
        self.state.last_signaling_message = None;
    }

    fn initialize_transport(&mut self, role: &str) {
        let room = self.state.room.clone().unwrap_or_default();
        let signaling_addr = self.state.signaling_addr.clone().unwrap_or_default();
        match TransportSession::new(WebRtcConfig {
            room,
            role: role.to_string(),
            signaling_url: signaling_addr,
            ice_servers: Vec::new(),
        }) {
            Ok(session) => {
                self.state.webrtc = Some(session);
                self.push_log("real WebRTC PeerConnection initialized".to_string());
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
        self.state.source_label = persisted.source_label;
        self.state.capture_selection = persisted.capture_selection;

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
        };

        if let Err(error) = self.preferences.save(&persisted) {
            self.push_log(format!("failed to persist session preferences: {error}"));
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
            (_, SessionStage::Stopped, _) => "restart or reset session",
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
                "start signaling server or fix the signaling address"
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
                "start signaling server or fix the signaling address"
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
    use crate::preferences::PreferencesStore;
    use crate::protocol::{
        PeerAnnouncement, Role, decode_signaling_message, encode_peer, encode_waiting,
        parse_join_request,
    };
    use crate::signaling::SignalingConnection;
    use capture_core::{CaptureSource, CaptureSourceKind};
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
        let mut manager = SessionManager::new();
        let snapshot = manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
        });

        assert_eq!(snapshot.mode, SessionMode::Host);
        assert_eq!(snapshot.stage, SessionStage::Configured);
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
    }

    #[test]
    fn host_start_auto_selects_default_capture_source_when_none_chosen() {
        let mut manager = SessionManager::new();
        let first_source = manager
            .capture_catalog()
            .sources
            .first()
            .cloned()
            .expect("at least one source");

        let snapshot = manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: None,
        });

        assert_eq!(
            snapshot.selected_source_id.as_deref(),
            Some(first_source.id.as_str())
        );
        assert_eq!(snapshot.selected_source_audio, first_source.has_audio);
        assert_eq!(
            snapshot.source_label.as_deref(),
            Some(first_source.label().as_str())
        );
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("default capture source selected for host"))
        );
    }

    #[test]
    fn viewer_session_updates_state() {
        let mut manager = SessionManager::new();
        let snapshot = manager.start_viewer(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: None,
        });

        assert_eq!(snapshot.mode, SessionMode::Viewer);
        assert_eq!(snapshot.stage, SessionStage::AwaitingPeer);
        assert_eq!(snapshot.transport, SessionTransport::LiveWebRtc);
        assert_eq!(snapshot.room.as_deref(), Some("demo"));
        assert_eq!(snapshot.signaling_addr.as_deref(), Some("127.0.0.1:7000"));
        assert_eq!(snapshot.local_media_track_count, 0);
        assert!(!snapshot.local_video_track_attached);
        assert!(!snapshot.local_audio_track_attached);
        assert_eq!(
            snapshot.next_action,
            "start signaling server or fix the signaling address"
        );
    }

    #[test]
    fn selecting_capture_source_updates_source_label() {
        let mut manager = SessionManager::new();
        let source = manager
            .capture_catalog()
            .sources
            .first()
            .cloned()
            .expect("at least one source");

        let snapshot = manager.select_capture_source(source.id.clone(), source.has_audio);
        let label = source.label();
        assert_eq!(
            snapshot.selected_source_id.as_deref(),
            Some(source.id.as_str())
        );
        assert_eq!(snapshot.selected_source_audio, source.has_audio);
        assert_eq!(snapshot.source_label.as_deref(), Some(label.as_str()));
    }

    #[test]
    fn publish_debug_capture_requires_selected_source() {
        let mut manager = SessionManager::new();

        let snapshot = manager.publish_debug_capture_samples();

        assert_eq!(snapshot.published_video_sample_count, 0);
        assert_eq!(snapshot.published_audio_sample_count, 0);
        assert!(snapshot.logs.iter().any(|line| line.contains(
            "cannot publish debug capture samples before selecting a capture source"
        )));
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
        });
        let snapshot = manager.stop();

        assert_eq!(snapshot.stage, SessionStage::Stopped);
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("session stopped"))
        );
    }

    #[test]
    fn host_session_can_publish_debug_capture_media() {
        let mut manager = SessionManager::new();
        manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
        });

        let snapshot = manager.publish_debug_capture_samples();
        assert_eq!(snapshot.published_video_sample_count, 1);
        assert_eq!(snapshot.published_audio_sample_count, 1);
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
    }

    #[test]
    fn debug_capture_publish_respects_audio_toggle() {
        let mut manager = SessionManager::new();
        let source = manager
            .capture_catalog()
            .sources
            .iter()
            .find(|source| source.has_audio)
            .cloned()
            .expect("at least one audio-capable source");

        manager.select_capture_source(source.id, false);
        manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
        });

        let snapshot = manager.publish_debug_capture_samples();
        assert_eq!(snapshot.published_video_sample_count, 1);
        assert_eq!(snapshot.published_audio_sample_count, 0);
        assert!(snapshot.last_audio_capture_summary.is_none());
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("audio skipped by capture selection"))
        );
    }

    #[test]
    fn capture_publish_rejects_mismatched_source_id() {
        let mut manager = SessionManager::new();
        let source = manager
            .capture_catalog()
            .sources
            .first()
            .cloned()
            .expect("at least one source");

        manager.select_capture_source(source.id.clone(), source.has_audio);
        manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
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
    }

    #[test]
    fn capture_publish_rejects_audio_when_selection_disables_it() {
        let mut manager = SessionManager::new();
        let source = manager
            .capture_catalog()
            .sources
            .iter()
            .find(|source| source.has_audio)
            .cloned()
            .expect("at least one audio-capable source");

        manager.select_capture_source(source.id.clone(), false);
        manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "127.0.0.1:7000".to_string(),
            source_label: Some("vlc".to_string()),
        });

        let snapshot = manager.publish_capture_audio_buffer(source.id, super::debug_audio_buffer());

        assert_eq!(snapshot.published_audio_sample_count, 0);
        assert!(snapshot.logs.iter().any(|line| line.contains(
            "capture audio buffer ignored because audio is disabled"
        )));
    }

    #[test]
    fn host_refresh_auto_creates_offer_after_signaling_connects() {
        let server = TestSignalingServer::new();
        let mut manager = SessionManager::new();
        let snapshot = manager.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "in-memory-signaling".to_string(),
            source_label: Some("vlc".to_string()),
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
        );
        first.select_capture_source(source.id.clone(), false);

        let restored =
            SessionManager::new_for_tests(PreferencesStore::from_config_dir(config_dir.clone()));
        let snapshot = restored.snapshot();

        assert_eq!(snapshot.room.as_deref(), Some("saved-room"));
        assert_eq!(snapshot.signaling_addr.as_deref(), Some("127.0.0.1:7100"));
        assert_eq!(
            snapshot.selected_source_id.as_deref(),
            Some(source.id.as_str())
        );
        assert!(!snapshot.selected_source_audio);
        assert_eq!(
            snapshot.source_label.as_deref(),
            Some(source.label().as_str())
        );
        assert!(
            snapshot
                .logs
                .iter()
                .any(|line| line.contains("restored persisted session preferences"))
        );

        let _ = fs::remove_dir_all(config_dir);
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
        );
        manager.select_capture_source(source.id.clone(), source.has_audio);
        manager.start_host(SessionIntent {
            room: "transient-room".to_string(),
            signaling_addr: "127.0.0.1:7300".to_string(),
            source_label: Some("temporary".to_string()),
        });

        let snapshot = manager.reset();
        assert_eq!(snapshot.mode, SessionMode::Idle);
        assert_eq!(snapshot.stage, SessionStage::Idle);
        assert_eq!(snapshot.room.as_deref(), Some("saved-room"));
        assert_eq!(snapshot.signaling_addr.as_deref(), Some("127.0.0.1:7200"));
        assert_eq!(
            snapshot.selected_source_id.as_deref(),
            Some(source.id.as_str())
        );

        let _ = fs::remove_dir_all(config_dir);
    }

    #[test]
    fn late_join_replay_drives_host_and_viewer_to_negotiated_webrtc() {
        let server = TestSignalingServer::new();

        let mut host = SessionManager::new();
        let host_start = host.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "in-memory-signaling".to_string(),
            source_label: Some("vlc".to_string()),
        });
        assert!(!host_start.signaling_connected);
        attach_test_signaling(&mut host, &server, "demo", Role::Sender, 4100);
        let host_offer = host.refresh();
        assert!(host_offer.local_offer_ready);
        assert_eq!(host_offer.local_description_kind.as_deref(), Some("offer"));

        thread::sleep(Duration::from_millis(150));

        let mut viewer = SessionManager::new();
        let viewer_start = viewer.start_viewer(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: "in-memory-signaling".to_string(),
            source_label: None,
        });
        assert!(!viewer_start.signaling_connected);
        attach_test_signaling(&mut viewer, &server, "demo", Role::Receiver, 4200);

        let (host_snapshot, viewer_snapshot) =
            drive_sessions_until_negotiated(&mut host, &mut viewer, Duration::from_secs(10));

        assert!(matches!(
            host_snapshot.stage,
            SessionStage::NegotiatingWebRtc | SessionStage::LiveWebRtc
        ));
        assert!(matches!(
            viewer_snapshot.stage,
            SessionStage::NegotiatingWebRtc | SessionStage::LiveWebRtc
        ));
        assert!(matches!(
            host_snapshot.transport_state.as_str(),
            "connecting" | "connected"
        ));
        assert!(matches!(
            viewer_snapshot.transport_state.as_str(),
            "connecting" | "connected"
        ));
        assert_eq!(host_snapshot.local_media_track_count, 2);
        assert_eq!(viewer_snapshot.local_media_track_count, 0);
        assert_eq!(
            host_snapshot.remote_description_kind.as_deref(),
            Some("answer")
        );
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
                .any(|line| line.contains("remote SDP answer received and applied"))
        );
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

    fn drive_sessions_until_negotiated(
        host: &mut SessionManager,
        viewer: &mut SessionManager,
        timeout: Duration,
    ) -> (super::SessionSnapshot, super::SessionSnapshot) {
        let started = Instant::now();
        loop {
            let host_snapshot = host.refresh();
            let viewer_snapshot = viewer.refresh();

            if host_snapshot.remote_description_kind.as_deref() == Some("answer")
                && viewer_snapshot.remote_description_kind.as_deref() == Some("offer")
                && host_snapshot.active_peer.is_some()
                && viewer_snapshot.active_peer.is_some()
            {
                return (host_snapshot, viewer_snapshot);
            }

            assert!(
                started.elapsed() < timeout,
                "timed out waiting for host/viewer connection.\nhost={host_snapshot:#?}\nviewer={viewer_snapshot:#?}"
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
}
