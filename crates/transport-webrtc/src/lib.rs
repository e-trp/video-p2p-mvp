#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportStage {
    Planned,
    SignalingReady,
    PeerConnecting,
    OfferCreated,
    AnswerAccepted,
    Streaming,
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
    LocalOffer(String),
    RemoteAnswer(String),
    RemoteIce {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
}

#[derive(Debug, Clone)]
pub struct TransportSession {
    config: WebRtcConfig,
    stage: TransportStage,
    local_offer: Option<String>,
    remote_answer: Option<String>,
    remote_candidates: Vec<WebRtcSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportSnapshot {
    pub room: String,
    pub role: String,
    pub signaling_url: String,
    pub stage: TransportStage,
    pub local_offer_ready: bool,
    pub remote_answer_ready: bool,
    pub remote_candidate_count: usize,
    pub notes: Vec<String>,
}

pub fn blueprint(config: WebRtcConfig) -> TransportBlueprint {
    TransportBlueprint {
        config,
        stage: TransportStage::Planned,
        notes: vec![
            "replace mock UDP transport with WebRTC PeerConnection",
            "publish video and audio tracks from platform capture backends",
            "exchange SDP and ICE via signaling service",
            "expose bitrate, RTT, packet loss, and connection state to GUI",
        ],
    }
}

impl TransportSession {
    pub fn new(config: WebRtcConfig) -> Self {
        Self {
            config,
            stage: TransportStage::SignalingReady,
            local_offer: None,
            remote_answer: None,
            remote_candidates: Vec::new(),
        }
    }

    pub fn create_local_offer(&mut self) -> WebRtcSignal {
        let offer = format!(
            "v=0\no=- 0 0 IN IP4 127.0.0.1\ns=room:{}\na=role:{}\na=signal:{}",
            self.config.room, self.config.role, self.config.signaling_url
        );
        self.local_offer = Some(offer.clone());
        self.stage = TransportStage::OfferCreated;
        WebRtcSignal::LocalOffer(offer)
    }

    pub fn accept_remote_answer(&mut self, sdp: String) {
        self.remote_answer = Some(sdp);
        self.stage = TransportStage::AnswerAccepted;
    }

    pub fn add_remote_ice_candidate(
        &mut self,
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    ) {
        self.remote_candidates.push(WebRtcSignal::RemoteIce {
            candidate,
            sdp_mid,
            sdp_mline_index,
        });
        if self.remote_answer.is_some() {
            self.stage = TransportStage::PeerConnecting;
        }
    }

    pub fn mark_streaming(&mut self) {
        self.stage = TransportStage::Streaming;
    }

    pub fn snapshot(&self) -> TransportSnapshot {
        TransportSnapshot {
            room: self.config.room.clone(),
            role: self.config.role.clone(),
            signaling_url: self.config.signaling_url.clone(),
            stage: self.stage,
            local_offer_ready: self.local_offer.is_some(),
            remote_answer_ready: self.remote_answer.is_some(),
            remote_candidate_count: self.remote_candidates.len(),
            notes: vec![
                "real PeerConnection integration is still pending".to_string(),
                "session currently models SDP/ICE lifecycle only".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IceServer, TransportSession, TransportStage, WebRtcConfig, WebRtcSignal};

    #[test]
    fn offer_and_answer_advance_state() {
        let mut session = TransportSession::new(WebRtcConfig {
            room: "demo".to_string(),
            role: "host".to_string(),
            signaling_url: "ws://127.0.0.1:7000/ws".to_string(),
            ice_servers: vec![IceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_string()],
                username: None,
                credential: None,
            }],
        });

        let offer = session.create_local_offer();
        assert!(matches!(offer, WebRtcSignal::LocalOffer(_)));
        assert_eq!(session.snapshot().stage, TransportStage::OfferCreated);

        session.accept_remote_answer("v=0\ns=answer".to_string());
        assert_eq!(session.snapshot().stage, TransportStage::AnswerAccepted);

        session.add_remote_ice_candidate("candidate".to_string(), Some("0".to_string()), Some(0));
        assert_eq!(session.snapshot().stage, TransportStage::PeerConnecting);
    }
}
