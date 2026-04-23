use interceptor::registry::Registry;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportStage {
    Planned,
    SignalingReady,
    PeerConnecting,
    OfferCreated,
    AnswerCreated,
    AnswerAccepted,
    Streaming,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptionKind {
    Offer,
    Answer,
}

#[derive(Debug, Clone)]
pub struct IceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WebRtcConfig {
    pub room: String,
    pub role: String,
    pub signaling_url: String,
    pub ice_servers: Vec<IceServer>,
}

#[derive(Debug, Clone)]
pub struct TransportBlueprint {
    pub config: WebRtcConfig,
    pub stage: TransportStage,
    pub notes: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebRtcSignal {
    SessionDescription {
        kind: DescriptionKind,
        sdp: String,
    },
    IceCandidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
}

pub struct TransportSession {
    config: WebRtcConfig,
    runtime: Runtime,
    peer_connection: Arc<RTCPeerConnection>,
    shared: Arc<Mutex<SharedState>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportSnapshot {
    pub room: String,
    pub role: String,
    pub signaling_url: String,
    pub stage: TransportStage,
    pub connection_state: String,
    pub local_description_kind: Option<DescriptionKind>,
    pub remote_description_kind: Option<DescriptionKind>,
    pub local_description_ready: bool,
    pub remote_description_ready: bool,
    pub local_candidate_count: usize,
    pub remote_candidate_count: usize,
    pub local_data_channel_ready: bool,
    pub stats_report_count: usize,
    pub notes: Vec<String>,
}

#[derive(Debug, Default)]
struct SharedState {
    stage: Option<TransportStage>,
    connection_state: String,
    local_description_kind: Option<DescriptionKind>,
    remote_description_kind: Option<DescriptionKind>,
    local_candidates: Vec<WebRtcSignal>,
    remote_candidate_count: usize,
    local_data_channel_ready: bool,
    stats_report_count: usize,
}

#[derive(Debug)]
pub struct TransportError(String);

impl Display for TransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for TransportError {}

pub type TransportResult<T> = Result<T, TransportError>;

pub fn blueprint(config: WebRtcConfig) -> TransportBlueprint {
    TransportBlueprint {
        config,
        stage: TransportStage::Planned,
        notes: vec![
            "peer connection is created eagerly with real ICE gathering",
            "a bootstrap data channel keeps negotiation real before media tracks land",
            "signaling exchange still runs through the project signaling server",
            "future capture backends should replace the bootstrap data channel with media tracks",
        ],
    }
}

impl TransportSession {
    pub fn new(config: WebRtcConfig) -> TransportResult<Self> {
        let runtime = tokio_runtime()?;
        let shared = Arc::new(Mutex::new(SharedState {
            stage: Some(TransportStage::SignalingReady),
            connection_state: RTCPeerConnectionState::New.to_string(),
            ..Default::default()
        }));

        let peer_connection = runtime.block_on(async {
            let mut media_engine = MediaEngine::default();
            media_engine
                .register_default_codecs()
                .map_err(map_webrtc_error)?;

            let mut registry = Registry::new();
            registry = register_default_interceptors(registry, &mut media_engine)
                .map_err(map_webrtc_error)?;

            let api = APIBuilder::new()
                .with_media_engine(media_engine)
                .with_interceptor_registry(registry)
                .build();

            let connection = api
                .new_peer_connection(rtc_configuration(&config))
                .await
                .map_err(map_webrtc_error)?;
            Ok::<Arc<RTCPeerConnection>, TransportError>(Arc::new(connection))
        })?;

        install_callbacks(&peer_connection, &shared);

        if role_is_offerer(&config.role) {
            let data_channel = runtime
                .block_on(peer_connection.create_data_channel("bootstrap-control", None))
                .map_err(map_webrtc_error)?;
            install_data_channel_callbacks(&data_channel, &shared);
        }

        Ok(Self {
            config,
            runtime,
            peer_connection,
            shared,
        })
    }

    pub fn create_local_offer(&mut self) -> TransportResult<WebRtcSignal> {
        let offer = self
            .runtime
            .block_on(async {
                let offer = self
                    .peer_connection
                    .create_offer(None)
                    .await
                    .map_err(map_webrtc_error)?;
                self.peer_connection
                    .set_local_description(offer.clone())
                    .await
                    .map_err(map_webrtc_error)?;
                Ok::<RTCSessionDescription, TransportError>(offer)
            })?;

        let mut shared = self.shared.lock().expect("transport shared state poisoned");
        shared.stage = Some(TransportStage::OfferCreated);
        shared.local_description_kind = Some(DescriptionKind::Offer);

        Ok(WebRtcSignal::SessionDescription {
            kind: DescriptionKind::Offer,
            sdp: offer.sdp,
        })
    }

    pub fn accept_remote_offer(&mut self, sdp: String) -> TransportResult<WebRtcSignal> {
        let answer = self.runtime.block_on(async {
            let offer = RTCSessionDescription::offer(sdp).map_err(map_webrtc_error)?;
            self.peer_connection
                .set_remote_description(offer)
                .await
                .map_err(map_webrtc_error)?;
            let answer = self
                .peer_connection
                .create_answer(None)
                .await
                .map_err(map_webrtc_error)?;
            self.peer_connection
                .set_local_description(answer.clone())
                .await
                .map_err(map_webrtc_error)?;
            Ok::<RTCSessionDescription, TransportError>(answer)
        })?;

        let mut shared = self.shared.lock().expect("transport shared state poisoned");
        shared.stage = Some(TransportStage::AnswerCreated);
        shared.local_description_kind = Some(DescriptionKind::Answer);
        shared.remote_description_kind = Some(DescriptionKind::Offer);

        Ok(WebRtcSignal::SessionDescription {
            kind: DescriptionKind::Answer,
            sdp: answer.sdp,
        })
    }

    pub fn accept_remote_answer(&mut self, sdp: String) -> TransportResult<()> {
        self.runtime.block_on(async {
            let answer = RTCSessionDescription::answer(sdp).map_err(map_webrtc_error)?;
            self.peer_connection
                .set_remote_description(answer)
                .await
                .map_err(map_webrtc_error)?;
            Ok::<(), TransportError>(())
        })?;

        let mut shared = self.shared.lock().expect("transport shared state poisoned");
        shared.stage = Some(TransportStage::AnswerAccepted);
        shared.remote_description_kind = Some(DescriptionKind::Answer);
        Ok(())
    }

    pub fn add_remote_ice_candidate(
        &mut self,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    ) -> TransportResult<()> {
        self.runtime.block_on(async {
            self.peer_connection
                .add_ice_candidate(RTCIceCandidateInit {
                    candidate,
                    sdp_mid,
                    sdp_mline_index,
                    username_fragment: None,
                })
                .await
                .map_err(map_webrtc_error)?;
            Ok::<(), TransportError>(())
        })?;

        let mut shared = self.shared.lock().expect("transport shared state poisoned");
        shared.remote_candidate_count += 1;
        if shared.remote_description_kind.is_some() {
            shared.stage = Some(TransportStage::PeerConnecting);
        }
        Ok(())
    }

    pub fn drain_local_signals(&mut self) -> Vec<WebRtcSignal> {
        let mut shared = self.shared.lock().expect("transport shared state poisoned");
        shared.local_candidates.drain(..).collect()
    }

    pub fn snapshot(&self) -> TransportSnapshot {
        let stats_report_count = self
            .runtime
            .block_on(async { self.peer_connection.get_stats().await.reports.len() });

        let mut shared = self.shared.lock().expect("transport shared state poisoned");
        shared.stats_report_count = stats_report_count;

        let stage = shared.stage.unwrap_or(TransportStage::SignalingReady);
        let mut notes = Vec::new();
        notes.push(format!("peer connection state={}", shared.connection_state));
        notes.push(format!("stats reports={}", shared.stats_report_count));
        if shared.local_data_channel_ready {
            notes.push("bootstrap data channel opened".to_string());
        } else {
            notes.push("bootstrap data channel not open yet".to_string());
        }

        TransportSnapshot {
            room: self.config.room.clone(),
            role: self.config.role.clone(),
            signaling_url: self.config.signaling_url.clone(),
            stage,
            connection_state: shared.connection_state.clone(),
            local_description_kind: shared.local_description_kind,
            remote_description_kind: shared.remote_description_kind,
            local_description_ready: shared.local_description_kind.is_some(),
            remote_description_ready: shared.remote_description_kind.is_some(),
            local_candidate_count: shared.local_candidates.len(),
            remote_candidate_count: shared.remote_candidate_count,
            local_data_channel_ready: shared.local_data_channel_ready,
            stats_report_count: shared.stats_report_count,
            notes,
        }
    }

    pub fn close(&mut self) -> TransportResult<()> {
        self.runtime
            .block_on(self.peer_connection.close())
            .map_err(map_webrtc_error)?;
        let mut shared = self.shared.lock().expect("transport shared state poisoned");
        shared.stage = Some(TransportStage::Closed);
        shared.connection_state = RTCPeerConnectionState::Closed.to_string();
        Ok(())
    }
}

fn tokio_runtime() -> TransportResult<Runtime> {
    Runtime::new().map_err(|error| TransportError(format!("failed to create tokio runtime: {error}")))
}

fn rtc_configuration(config: &WebRtcConfig) -> RTCConfiguration {
    RTCConfiguration {
        ice_servers: config
            .ice_servers
            .iter()
            .map(|server| RTCIceServer {
                urls: server.urls.clone(),
                username: server.username.clone().unwrap_or_default(),
                credential: server.credential.clone().unwrap_or_default(),
            })
            .collect(),
        ..Default::default()
    }
}

fn role_is_offerer(role: &str) -> bool {
    matches!(role, "host" | "sender")
}

fn install_callbacks(peer_connection: &Arc<RTCPeerConnection>, shared: &Arc<Mutex<SharedState>>) {
    let shared_state = Arc::clone(shared);
    peer_connection.on_ice_candidate(Box::new(move |candidate| {
        let shared_state = Arc::clone(&shared_state);
        Box::pin(async move {
            let Some(candidate) = candidate else {
                return;
            };

            if let Ok(candidate) = candidate.to_json() {
                let mut shared = shared_state.lock().expect("transport shared state poisoned");
                shared.local_candidates.push(WebRtcSignal::IceCandidate {
                    candidate: candidate.candidate,
                    sdp_mid: candidate.sdp_mid,
                    sdp_mline_index: candidate.sdp_mline_index,
                });
            }
        })
    }));

    let shared_state = Arc::clone(shared);
    peer_connection.on_peer_connection_state_change(Box::new(move |state| {
        let shared_state = Arc::clone(&shared_state);
        Box::pin(async move {
            let mut shared = shared_state.lock().expect("transport shared state poisoned");
            shared.connection_state = state.to_string();
            shared.stage = Some(match state {
                RTCPeerConnectionState::Connected => TransportStage::Streaming,
                RTCPeerConnectionState::Connecting => TransportStage::PeerConnecting,
                RTCPeerConnectionState::Closed => TransportStage::Closed,
                _ => shared.stage.unwrap_or(TransportStage::SignalingReady),
            });
        })
    }));

    let shared_state = Arc::clone(shared);
    peer_connection.on_data_channel(Box::new(move |data_channel| {
        let shared_state = Arc::clone(&shared_state);
        Box::pin(async move {
            install_data_channel_callbacks(&data_channel, &shared_state);
        })
    }));
}

fn install_data_channel_callbacks(
    data_channel: &Arc<webrtc::data_channel::RTCDataChannel>,
    shared: &Arc<Mutex<SharedState>>,
) {
    let shared_state = Arc::clone(shared);
    data_channel.on_open(Box::new(move || {
        let shared_state = Arc::clone(&shared_state);
        Box::pin(async move {
            let mut shared = shared_state.lock().expect("transport shared state poisoned");
            shared.local_data_channel_ready = true;
        })
    }));
}

fn map_webrtc_error<E: Display>(error: E) -> TransportError {
    TransportError(format!("webrtc transport error: {error}"))
}

impl From<RTCSdpType> for DescriptionKind {
    fn from(value: RTCSdpType) -> Self {
        match value {
            RTCSdpType::Offer => DescriptionKind::Offer,
            RTCSdpType::Answer => DescriptionKind::Answer,
            _ => DescriptionKind::Offer,
        }
    }
}
