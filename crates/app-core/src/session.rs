use std::time::{SystemTime, UNIX_EPOCH};

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
    PlannedWebRtc,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTransport {
    MockUdp,
    PlannedWebRtc,
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
}

#[derive(Debug, Default)]
pub struct SessionManager {
    state: SessionState,
}

#[derive(Debug)]
struct SessionState {
    mode: SessionMode,
    stage: SessionStage,
    transport: SessionTransport,
    room: Option<String>,
    signaling_addr: Option<String>,
    source_label: Option<String>,
    active_peer: Option<String>,
    logs: Vec<String>,
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
        }
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_host(&mut self, intent: SessionIntent) -> SessionSnapshot {
        self.state.mode = SessionMode::Host;
        self.state.stage = SessionStage::Configured;
        self.state.transport = SessionTransport::MockUdp;
        self.state.room = Some(intent.room.clone());
        self.state.signaling_addr = Some(intent.signaling_addr.clone());
        self.state.source_label = Some(
            intent
                .source_label
                .unwrap_or_else(|| "window selection pending".to_string()),
        );
        self.state.active_peer = None;
        self.push_log(format!(
            "host session configured for room={} signaling={}",
            intent.room, intent.signaling_addr
        ));
        self.push_log("next step: create local signaling connection".to_string());
        self.snapshot()
    }

    pub fn start_viewer(&mut self, intent: SessionIntent) -> SessionSnapshot {
        self.state.mode = SessionMode::Viewer;
        self.state.stage = SessionStage::AwaitingPeer;
        self.state.transport = SessionTransport::MockUdp;
        self.state.room = Some(intent.room.clone());
        self.state.signaling_addr = Some(intent.signaling_addr.clone());
        self.state.source_label = None;
        self.state.active_peer = None;
        self.push_log(format!(
            "viewer session configured for room={} signaling={}",
            intent.room, intent.signaling_addr
        ));
        self.push_log("next step: wait for sender and negotiate peer transport".to_string());
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
        self.state.stage = SessionStage::PlannedWebRtc;
        self.state.transport = SessionTransport::PlannedWebRtc;
        self.push_log("session moved to planned WebRTC transport stage".to_string());
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

    pub fn stop(&mut self) -> SessionSnapshot {
        self.state.stage = SessionStage::Stopped;
        self.state.active_peer = None;
        self.push_log("session stopped".to_string());
        self.snapshot()
    }

    pub fn clear_logs(&mut self) -> SessionSnapshot {
        self.state.logs.clear();
        self.push_log("session log cleared".to_string());
        self.snapshot()
    }

    pub fn reset(&mut self) -> SessionSnapshot {
        self.state = SessionState::default();
        self.push_log("session reset to idle state".to_string());
        self.snapshot()
    }

    pub fn snapshot(&self) -> SessionSnapshot {
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
        }
    }

    pub fn logs(&self) -> Vec<String> {
        self.state.logs.clone()
    }

    fn push_log(&mut self, message: String) {
        self.state.logs.push(stamp(&message));
        if self.state.logs.len() > 200 {
            let drain = self.state.logs.len() - 200;
            self.state.logs.drain(0..drain);
        }
    }

    fn next_action(&self) -> &'static str {
        match (self.state.mode, self.state.stage, self.state.transport) {
            (SessionMode::Idle, _, _) => "configure host or viewer session",
            (SessionMode::Host, SessionStage::Configured, SessionTransport::MockUdp) => {
                "connect signaling and start preview stream"
            }
            (SessionMode::Viewer, SessionStage::AwaitingPeer, SessionTransport::MockUdp) => {
                "wait for sender and open direct peer path"
            }
            (_, SessionStage::MockStreaming, SessionTransport::MockUdp) => {
                "replace mock transport with WebRTC tracks"
            }
            (_, SessionStage::PlannedWebRtc, SessionTransport::PlannedWebRtc) => {
                "implement SDP/ICE exchange and real media tracks"
            }
            (_, SessionStage::Stopped, _) => "restart or reset session",
            _ => "continue integration",
        }
    }
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
        assert_eq!(snapshot.transport, SessionTransport::MockUdp);
        assert_eq!(snapshot.room.as_deref(), Some("demo"));
        assert_eq!(snapshot.source_label.as_deref(), Some("vlc"));
        assert_eq!(snapshot.next_action, "connect signaling and start preview stream");
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
}
