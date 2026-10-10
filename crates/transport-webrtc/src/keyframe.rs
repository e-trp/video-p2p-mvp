//! Coalesce decoder refresh requests and bound feedback-driven keyframe frequency.
use std::time::{Duration, Instant};
use webrtc::rtcp::{
    packet::Packet,
    payload_feedbacks::{
        full_intra_request::FullIntraRequest, picture_loss_indication::PictureLossIndication,
    },
};

const MIN_KEYFRAME_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Default)]
pub(crate) struct KeyframeRecovery {
    pub requests: usize,
    pub published: usize,
    pub pending: bool,
    last_published: Option<Instant>,
    last_fir: Option<(u32, u8)>,
}

impl KeyframeRecovery {
    pub fn observe(&mut self, packet: &(dyn Packet + Send + Sync), video_ssrc: u32) {
        if let Some(pli) = packet.as_any().downcast_ref::<PictureLossIndication>() {
            if pli.media_ssrc != video_ssrc {
                return;
            }
        } else if let Some(fir) = packet.as_any().downcast_ref::<FullIntraRequest>() {
            // FIR addresses streams in its FCI entries; the header media SSRC is normally zero.
            let Some(entry) = fir.fir.iter().find(|entry| entry.ssrc == video_ssrc) else {
                return;
            };
            let request = (fir.sender_ssrc, entry.sequence_number);
            if self.last_fir == Some(request) {
                return;
            }
            self.last_fir = Some(request);
        } else {
            return;
        }
        self.requests = self.requests.saturating_add(1);
        self.pending = true;
    }

    pub fn should_force(&self, now: Instant) -> bool {
        self.pending
            && self
                .last_published
                .is_none_or(|last| now.saturating_duration_since(last) >= MIN_KEYFRAME_INTERVAL)
    }

    pub fn published(&mut self, now: Instant, requests_before_write: usize) {
        self.published = self.published.saturating_add(1);
        self.last_published = Some(now);
        // A request arriving during publication still needs a later refresh frame.
        self.pending = self.requests > requests_before_write;
    }

    pub fn reset_pending(&mut self) {
        self.pending = false;
        self.last_published = None;
        self.last_fir = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use webrtc::rtcp::{
        payload_feedbacks::full_intra_request::FirEntry, receiver_report::ReceiverReport,
    };

    fn pli(ssrc: u32) -> PictureLossIndication {
        PictureLossIndication {
            sender_ssrc: 7,
            media_ssrc: ssrc,
        }
    }

    #[test]
    fn only_video_pli_and_addressed_fir_request_a_keyframe() {
        let mut recovery = KeyframeRecovery::default();
        recovery.observe(&ReceiverReport::default(), 42);
        recovery.observe(&pli(99), 42);
        recovery.observe(
            &FullIntraRequest {
                fir: vec![FirEntry {
                    ssrc: 99,
                    sequence_number: 1,
                }],
                ..Default::default()
            },
            42,
        );
        assert_eq!(recovery.requests, 0);
        assert!(!recovery.pending);
        recovery.observe(&pli(42), 42);
        let mut fir = FullIntraRequest {
            sender_ssrc: 7,
            media_ssrc: 0,
            fir: vec![FirEntry {
                ssrc: 42,
                sequence_number: 255,
            }],
        };
        recovery.observe(&fir, 42);
        recovery.observe(&fir, 42); // Retransmission of the same FIR is not a new command.
        assert_eq!(recovery.requests, 2);
        fir.fir[0].sequence_number = 0; // Sequence numbers wrap.
        recovery.observe(&fir, 42);
        assert_eq!(recovery.requests, 3);
        assert!(recovery.should_force(Instant::now()));
    }

    #[test]
    fn bursts_wait_for_the_cooldown_and_clear_only_after_publication() {
        let mut recovery = KeyframeRecovery::default();
        let now = Instant::now();
        recovery.published(now, 0);
        for _ in 0..20 {
            recovery.observe(&pli(42), 42);
        }
        assert!(recovery.pending);
        assert!(!recovery.should_force(now + Duration::from_millis(499)));
        assert!(recovery.should_force(now + MIN_KEYFRAME_INTERVAL));
        assert!(recovery.should_force(now + MIN_KEYFRAME_INTERVAL)); // Checking does not consume it.
        recovery.published(now + MIN_KEYFRAME_INTERVAL, recovery.requests);
        assert!(!recovery.pending);
        assert_eq!(recovery.published, 2);
        recovery.observe(&pli(42), 42);
        assert!(!recovery.should_force(now + Duration::from_millis(999)));
        assert!(recovery.should_force(now + Duration::from_millis(1000)));
    }

    #[test]
    fn feedback_during_publication_survives_and_disconnect_clears_pending_work() {
        let mut recovery = KeyframeRecovery::default();
        let now = Instant::now();
        recovery.observe(&pli(42), 42);
        let requests_before_write = recovery.requests;
        recovery.observe(&pli(42), 42);
        recovery.published(now, requests_before_write);
        assert!(recovery.pending);
        assert!(!recovery.should_force(now));
        recovery.reset_pending();
        assert!(!recovery.should_force(now + MIN_KEYFRAME_INTERVAL));
        assert_eq!(recovery.requests, 2);
        assert_eq!(recovery.published, 1);
        recovery.observe(&pli(42), 42);
        assert!(recovery.should_force(now));
    }
}
