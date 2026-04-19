#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportStage {
    Planned,
    SignalingReady,
    PeerConnecting,
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
