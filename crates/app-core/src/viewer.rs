//! Signaling adapter for a receive-only WebView peer. Media stays in WebRTC, outside IPC.
use crate::{IceServerEntry, SdpType, SignalingMessage};
use std::{
    collections::VecDeque,
    sync::atomic::{AtomicU64, Ordering},
};
use transport_webrtc::{DescriptionKind, TransportSnapshot, TransportStage};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ViewerSignalingBatch {
    pub generation: u64,
    pub active: bool,
    pub ice_servers: Vec<IceServerEntry>,
    pub signals: Vec<ViewerSignal>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ViewerSignal {
    Offer {
        sdp: String,
    },
    Answer {
        sdp: String,
    },
    Ice {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
}
impl From<SignalingMessage> for ViewerSignal {
    fn from(message: SignalingMessage) -> Self {
        match message {
            SignalingMessage::SessionDescription(description) => match description.sdp_type {
                SdpType::Offer => Self::Offer {
                    sdp: description.sdp,
                },
                SdpType::Answer => Self::Answer {
                    sdp: description.sdp,
                },
            },
            SignalingMessage::IceCandidate(ice) => Self::Ice {
                candidate: ice.candidate,
                sdp_mid: ice.sdp_mid,
                sdp_mline_index: ice.sdp_mline_index,
            },
        }
    }
}
impl From<ViewerSignal> for SignalingMessage {
    fn from(signal: ViewerSignal) -> Self {
        match signal {
            ViewerSignal::Offer { sdp } => Self::SessionDescription(crate::SessionDescription {
                sdp_type: SdpType::Offer,
                sdp,
            }),
            ViewerSignal::Answer { sdp } => Self::SessionDescription(crate::SessionDescription {
                sdp_type: SdpType::Answer,
                sdp,
            }),
            ViewerSignal::Ice {
                candidate,
                sdp_mid,
                sdp_mline_index,
            } => Self::IceCandidate(crate::IceCandidate {
                candidate,
                sdp_mid,
                sdp_mline_index,
            }),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ViewerPlaybackReport {
    pub connection_state: String,
    pub playback_state: String,
    pub error: Option<String>,
    pub video_packets: usize,
    pub audio_packets: usize,
    pub video_frames: u64,
    pub audio_samples: u64,
    pub bytes_received: u64,
    pub packets_lost: i64,
    pub rtt_ms: Option<f64>,
    pub ice_path_kind: String,
    pub ice_path_summary: String,
    pub data_channel_ready: bool,
    pub stats_reports: usize,
}

pub(crate) struct BrowserViewer {
    pub generation: u64,
    pub transport: TransportSnapshot,
    pub playback_state: String,
    pub error: Option<String>,
    pub video_frames: u64,
    pub audio_samples: u64,
    pending: VecDeque<SignalingMessage>,
}

impl BrowserViewer {
    pub fn new(room: String, signaling_url: String) -> Self {
        static GENERATION: AtomicU64 = AtomicU64::new(1);
        Self {
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
            playback_state: "waiting".into(),
            error: None,
            video_frames: 0,
            audio_samples: 0,
            pending: VecDeque::new(),
            transport: TransportSnapshot {
                room,
                signaling_url,
                role: "viewer".into(),
                stage: TransportStage::SignalingReady,
                connection_state: "new".into(),
                local_description_kind: None,
                remote_description_kind: None,
                local_description_ready: false,
                remote_description_ready: false,
                local_candidate_count: 0,
                remote_candidate_count: 0,
                local_media_track_count: 0,
                local_video_track_attached: false,
                local_audio_track_attached: false,
                published_video_sample_count: 0,
                published_audio_sample_count: 0,
                encoded_video_frame_count: 0,
                encoded_audio_packet_count: 0,
                received_video_packet_count: 0,
                received_audio_packet_count: 0,
                last_video_sample_bytes: 0,
                last_audio_sample_bytes: 0,
                last_video_capture_summary: None,
                last_audio_capture_summary: None,
                local_data_channel_ready: false,
                stats_report_count: 0,
                ice_path_kind: "unknown".into(),
                ice_path_summary: "candidate pair not selected yet".into(),
                rtt_ms: None,
                available_outgoing_bitrate_bps: None,
                available_incoming_bitrate_bps: None,
                bytes_sent: None,
                bytes_received: None,
                packet_loss_fraction: None,
                packets_lost: None,
                notes: vec![
                    "WebView receives and decodes H264/Opus; Rust relays signaling only".into(),
                ],
            },
        }
    }

    pub fn is_active(&self) -> bool {
        !matches!(
            self.transport.connection_state.as_str(),
            "failed" | "closed"
        ) && !matches!(
            self.playback_state.as_str(),
            "failed" | "unsupported" | "stopped"
        )
    }

    pub fn enqueue(&mut self, signal: SignalingMessage) -> Result<(), String> {
        if !self.is_active() {
            return Err("viewer playback is stopped or failed; reconnect to retry".into());
        }
        if self.pending.len() >= 128 {
            return Err("viewer signaling queue exceeded 128 messages".into());
        }
        match &signal {
            SignalingMessage::SessionDescription(description)
                if description.sdp_type == SdpType::Offer =>
            {
                self.transport.remote_description_kind = Some(DescriptionKind::Offer);
                self.transport.remote_description_ready = true;
            }
            SignalingMessage::IceCandidate(_) => self.transport.remote_candidate_count += 1,
            _ => return Err("playback viewer accepts only host offers and ICE candidates".into()),
        }
        self.pending.push_back(signal);
        Ok(())
    }

    pub fn drain(&mut self) -> Vec<SignalingMessage> {
        self.pending.drain(..).collect()
    }

    pub fn sent(&mut self, signal: &SignalingMessage) {
        match signal {
            SignalingMessage::SessionDescription(_) => {
                self.transport.local_description_ready = true;
                self.transport.local_description_kind = Some(DescriptionKind::Answer);
                self.transport.stage = TransportStage::AnswerCreated;
            }
            SignalingMessage::IceCandidate(_) => self.transport.local_candidate_count += 1,
        }
    }

    pub fn report(&mut self, report: ViewerPlaybackReport) -> Result<(), String> {
        if !self.is_active() {
            return Err("viewer playback is stopped or failed; reconnect to retry".into());
        }
        if ![
            "new",
            "connecting",
            "connected",
            "disconnected",
            "failed",
            "closed",
        ]
        .contains(&report.connection_state.as_str())
            || ![
                "waiting",
                "playing",
                "paused",
                "blocked",
                "unsupported",
                "failed",
            ]
            .contains(&report.playback_state.as_str())
            || report
                .rtt_ms
                .is_some_and(|rtt| !rtt.is_finite() || rtt < 0.0)
            || !["unknown", "direct", "relay"].contains(&report.ice_path_kind.as_str())
        {
            return Err("invalid viewer playback report".into());
        }
        self.transport.stage = match report.connection_state.as_str() {
            "connected" => TransportStage::Streaming,
            "connecting" => TransportStage::PeerConnecting,
            "closed" => TransportStage::Closed,
            _ => self.transport.stage,
        };
        self.transport.connection_state = report.connection_state;
        self.transport.received_video_packet_count = self
            .transport
            .received_video_packet_count
            .max(report.video_packets);
        self.transport.received_audio_packet_count = self
            .transport
            .received_audio_packet_count
            .max(report.audio_packets);
        self.transport.bytes_received = Some(
            self.transport
                .bytes_received
                .unwrap_or(0)
                .max(report.bytes_received),
        );
        self.transport.packets_lost = Some(report.packets_lost);
        self.transport.rtt_ms = report.rtt_ms;
        self.transport.ice_path_kind = report.ice_path_kind;
        self.transport.ice_path_summary = report.ice_path_summary.chars().take(256).collect();
        self.transport.local_data_channel_ready = report.data_channel_ready;
        self.transport.stats_report_count = report.stats_reports;
        self.playback_state = report.playback_state;
        self.error = report.error.map(|error| error.chars().take(512).collect());
        self.video_frames = self.video_frames.max(report.video_frames);
        self.audio_samples = self.audio_samples.max(report.audio_samples);
        Ok(())
    }

    pub fn close(&mut self) {
        self.pending.clear();
        self.transport.stage = TransportStage::Closed;
        self.transport.connection_state = "closed".into();
        self.transport.local_data_channel_ready = false;
        self.playback_state = "stopped".into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IceCandidate, SessionDescription};
    #[test]
    fn bridge_preserves_offer_ice_order_and_bounds_pending_signals() {
        let mut viewer = BrowserViewer::new("test".into(), "local".into());
        let offer = SignalingMessage::SessionDescription(SessionDescription {
            sdp_type: SdpType::Offer,
            sdp: "v=0".into(),
        });
        let ice = SignalingMessage::IceCandidate(IceCandidate {
            candidate: "candidate:1".into(),
            sdp_mid: Some("0".into()),
            sdp_mline_index: Some(0),
        });
        viewer.enqueue(offer.clone()).unwrap();
        viewer.enqueue(ice.clone()).unwrap();
        assert_eq!(viewer.drain(), vec![offer, ice.clone()]);
        for _ in 0..128 {
            viewer.enqueue(ice.clone()).unwrap();
        }
        assert!(viewer.enqueue(ice).is_err());
        viewer.close();
        assert!(viewer.drain().is_empty());
    }
    #[test]
    fn reports_reject_invalid_state_and_keep_media_counters_monotonic() {
        let mut viewer = BrowserViewer::new("test".into(), "local".into());
        let report = ViewerPlaybackReport {
            connection_state: "connected".into(),
            playback_state: "playing".into(),
            ice_path_kind: "direct".into(),
            video_frames: 5,
            audio_samples: 960,
            ..Default::default()
        };
        viewer.report(report.clone()).unwrap();
        viewer
            .report(ViewerPlaybackReport {
                video_frames: 1,
                ..report.clone()
            })
            .unwrap();
        assert_eq!(viewer.video_frames, 5);
        assert_eq!(viewer.transport.stage, TransportStage::Streaming);
        assert!(
            viewer
                .report(ViewerPlaybackReport {
                    rtt_ms: Some(f64::NAN),
                    ..report
                })
                .is_err()
        );
    }

    #[test]
    fn terminal_failure_rejects_delayed_reports_and_signals() {
        let mut viewer = BrowserViewer::new("test".into(), "local".into());
        let connected = ViewerPlaybackReport {
            connection_state: "connected".into(),
            playback_state: "playing".into(),
            ice_path_kind: "direct".into(),
            ..Default::default()
        };
        viewer.report(connected.clone()).unwrap();
        viewer
            .report(ViewerPlaybackReport {
                connection_state: "failed".into(),
                playback_state: "failed".into(),
                error: Some("Peer connection failed".into()),
                ..connected.clone()
            })
            .unwrap();
        assert!(viewer.report(connected.clone()).is_err());
        assert!(
            viewer
                .enqueue(SignalingMessage::SessionDescription(SessionDescription {
                    sdp_type: SdpType::Offer,
                    sdp: "v=0".into(),
                }))
                .is_err()
        );
        assert_eq!(viewer.transport.connection_state, "failed");
        assert_eq!(viewer.error.as_deref(), Some("Peer connection failed"));
        viewer.close();
        assert!(viewer.report(connected).is_err());
        assert_eq!(viewer.transport.connection_state, "closed");
    }
}
