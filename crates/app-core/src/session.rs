use crate::protocol::{IceCandidate, PeerAnnouncement, Role, SdpType, SessionDescription, SignalingMessage};
use crate::signaling::{SignalingConnection, SignalingEvent};
use std::time::{SystemTime, UNIX_EPOCH};
use transport_webrtc::{
    DescriptionKind, TransportSession, WebRtcConfig, WebRtcSignal,
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
    pub active_peer: Option<String>,
    pub logs: Vec<String>,
    pub next_action: String,
    pub transport_state: String,
    pub signaling_connected: bool,
    pub local_description_ready: bool,
    pub local_description_kind: Option<String>,
    pub remote_description_ready: bool,
    pub remote_description_kind: Option<String>,
    pub local_media_track_count: usize,
    pub local_video_track_attached: bool,
    pub local_audio_track_attached: bool,
    pub local_offer_ready: bool,
    pub remote_answer_ready: bool,
    pub local_candidate_count: usize,
    pub remote_candidate_count: usize,
    pub last_signaling_message: Option<String>,
}

#[derive(Default)]
pub struct SessionManager {
    state: SessionState,
}

struct SessionState {
    mode: SessionMode,
    stage: SessionStage,
    transport: SessionTransport,
    room: Option<String>,
    signaling_addr: Option<String>,
    source_label: Option<String>,
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

    pub fn start_host(&mut self, intent: SessionIntent) -> SessionSnapshot {
        self.replace_state(
            SessionMode::Host,
            SessionStage::Configured,
            SessionTransport::LiveWebRtc,
            intent.clone(),
        );
        self.state.source_label = Some(
            intent
                .source_label
                .unwrap_or_else(|| "window selection pending".to_string()),
        );
        self.push_log(format!(
            "host session configured for room={} signaling={}",
            intent.room, intent.signaling_addr
        ));
        self.initialize_transport("host");
        self.connect_signaling();
        self.snapshot()
    }

    pub fn start_viewer(&mut self, intent: SessionIntent) -> SessionSnapshot {
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
            self.push_log("local SDP offer created and sent".to_string());
        }

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
            if let Err(error) = webrtc.add_remote_ice_candidate(candidate, sdp_mid, sdp_mline_index) {
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
        self.snapshot()
    }

    pub fn refresh(&mut self) -> SessionSnapshot {
        self.process_signaling_events();
        self.flush_local_transport_signals();
        self.update_stage_from_transport();
        self.snapshot()
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
                    snapshot.remote_description_kind.map(format_description_kind),
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
            active_peer: self.state.active_peer.clone(),
            logs: self.state.logs.clone(),
            next_action: self.next_action().to_string(),
            transport_state: transport_snapshot
                .as_ref()
                .map(|snapshot| snapshot.connection_state.clone())
                .unwrap_or_else(|| "not_initialized".to_string()),
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
        self.state.last_signaling_message =
            Some(format!("peer announced: role={} addr={}", peer.role, peer.addr));
        self.push_log(format!("peer discovered via signaling: {}", peer.addr));
    }

    fn handle_signaling_message(&mut self, message: SignalingMessage) {
        match message {
            SignalingMessage::SessionDescription(description) => match description.sdp_type {
                SdpType::Offer => {
                    self.state.last_signaling_message = Some("remote SDP offer received".to_string());
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
                            self.push_log("cannot process remote offer without transport".to_string());
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
                self.push_log("cannot send signaling message before signaling is connected".to_string());
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
            (_, _, SessionTransport::MockUdp) => "legacy mock UDP mode is still available for media scaffold work",
            (_, SessionStage::Stopped, _) => "restart or reset session",
            (_, SessionStage::LiveWebRtc, SessionTransport::LiveWebRtc) if connected => {
                "peer connection is live; next step is feeding capture samples into the attached tracks"
            }
            (SessionMode::Host, _, SessionTransport::LiveWebRtc) if !self.state.signaling_connected => {
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
                "create and send the local offer"
            }
            (SessionMode::Host, _, SessionTransport::LiveWebRtc) if !remote_description_ready => {
                "wait for viewer answer and keep refreshing signaling"
            }
            (SessionMode::Viewer, _, SessionTransport::LiveWebRtc) if !self.state.signaling_connected => {
                "start signaling server or fix the signaling address"
            }
            (SessionMode::Viewer, _, SessionTransport::LiveWebRtc) if !remote_description_ready => {
                "wait for host offer and keep refreshing signaling"
            }
            _ => "keep refreshing signaling until the peer connection is connected",
        }
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

fn stamp(message: &str) -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    format!("[{seconds}] {message}")
}

#[cfg(test)]
mod tests {
    use super::{SessionIntent, SessionManager, SessionMode, SessionStage, SessionTransport};
    use crate::protocol::{
        PeerAnnouncement, Role, decode_signaling_message, encode_peer, encode_waiting,
        parse_join_request,
    };
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
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
        assert_eq!(snapshot.local_media_track_count, 2);
        assert!(snapshot.local_video_track_attached);
        assert!(snapshot.local_audio_track_attached);
        assert!(snapshot.transport_state == "new" || snapshot.transport_state == "not_initialized");
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
        assert!(snapshot.logs.iter().any(|line| line.contains("session stopped")));
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
    fn late_join_replay_drives_host_and_viewer_to_live_webrtc() {
        let server = TestSignalingServer::spawn();

        let mut host = SessionManager::new();
        let host_start = host.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: server.addr(),
            source_label: Some("vlc".to_string()),
        });
        assert!(host_start.signaling_connected);

        let host_offer = host.create_local_offer();
        assert!(host_offer.local_offer_ready);
        assert_eq!(host_offer.local_description_kind.as_deref(), Some("offer"));

        thread::sleep(Duration::from_millis(150));

        let mut viewer = SessionManager::new();
        let viewer_start = viewer.start_viewer(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: server.addr(),
            source_label: None,
        });
        assert!(viewer_start.signaling_connected);

        let (host_snapshot, viewer_snapshot) =
            drive_sessions_until_connected(&mut host, &mut viewer, Duration::from_secs(10));

        assert_eq!(host_snapshot.stage, SessionStage::LiveWebRtc);
        assert_eq!(viewer_snapshot.stage, SessionStage::LiveWebRtc);
        assert_eq!(host_snapshot.transport_state, "connected");
        assert_eq!(viewer_snapshot.transport_state, "connected");
        assert_eq!(host_snapshot.local_media_track_count, 2);
        assert_eq!(viewer_snapshot.local_media_track_count, 0);
        assert_eq!(host_snapshot.remote_description_kind.as_deref(), Some("answer"));
        assert_eq!(viewer_snapshot.remote_description_kind.as_deref(), Some("offer"));
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

    fn drive_sessions_until_connected(
        host: &mut SessionManager,
        viewer: &mut SessionManager,
        timeout: Duration,
    ) -> (super::SessionSnapshot, super::SessionSnapshot) {
        let started = Instant::now();
        loop {
            let host_snapshot = host.refresh();
            let viewer_snapshot = viewer.refresh();

            if host_snapshot.transport_state == "connected"
                && viewer_snapshot.transport_state == "connected"
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
        writer: Arc<Mutex<TcpStream>>,
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
        addr: String,
        _accept_thread: thread::JoinHandle<()>,
    }

    impl TestSignalingServer {
        fn spawn() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind test signaling server");
            let addr = listener.local_addr().expect("listener addr").to_string();
            let rooms = Arc::new(Mutex::new(HashMap::<String, TestRoom>::new()));
            let accept_thread = thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(stream) = stream else {
                        break;
                    };
                    let rooms = Arc::clone(&rooms);
                    thread::spawn(move || {
                        handle_test_client(stream, rooms);
                    });
                }
            });

            Self {
                addr,
                _accept_thread: accept_thread,
            }
        }

        fn addr(&self) -> String {
            self.addr.clone()
        }
    }

    fn handle_test_client(stream: TcpStream, rooms: SharedRooms) {
        let peer_addr = stream.peer_addr().expect("peer addr");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut first_line = String::new();
        reader.read_line(&mut first_line).expect("read join request");
        let request = parse_join_request(first_line.trim()).expect("valid join request");

        let room_name = request.room.clone();
        let participant = TestParticipant {
            role: request.role,
            addr: SocketAddr::new(peer_addr.ip(), request.udp_port),
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
            relay_test_signaling_message(&room_name, participant.role, normalize_message(&line), &rooms);
        }
    }

    fn register_test_participant(room_name: &str, participant: TestParticipant, rooms: &SharedRooms) {
        let mut writes = Vec::new();
        {
            let mut rooms = rooms.lock().expect("rooms poisoned");
            let room = rooms.entry(room_name.to_string()).or_default();
            let slot = participant_slot(room, participant.role);
            assert!(slot.is_none(), "role already occupied in room");
            *slot = Some(participant.clone());

            if let Some(opposite) = participant_for_role(room, participant.role.opposite()).cloned() {
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

    fn apply_test_writes(writes: Vec<(Arc<Mutex<TcpStream>>, String)>) {
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
}
