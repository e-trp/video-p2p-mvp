use crate::{CodecError, CodecResult, EncodedAudioPacket};
use capture_core::AudioBuffer;
use std::{collections::VecDeque, time::Duration};

const PACKET_FRAMES: usize = 960;
const PACKET_MICROS: u64 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioEncoderSettings {
    pub channels: u16,
    pub bitrate_bps: u32,
}
impl Default for AudioEncoderSettings {
    fn default() -> Self {
        Self {
            channels: 2,
            bitrate_bps: 64_000,
        }
    }
}

pub struct OpusEncoder {
    encoder: opus::Encoder,
    channels: u16,
    pending: VecDeque<f32>,
    pending_timestamp: Option<u64>,
    expected_input_timestamp: Option<u64>,
    discontinuity_count: u64,
}

fn opus_error(error: opus::Error) -> CodecError {
    CodecError(format!("Opus encoding failed: {error}"))
}

impl OpusEncoder {
    pub fn new(settings: AudioEncoderSettings) -> CodecResult<Self> {
        let channels = match settings.channels {
            1 => opus::Channels::Mono,
            2 => opus::Channels::Stereo,
            _ => {
                return Err(CodecError(
                    "Opus supports one or two capture channels".to_string(),
                ));
            }
        };
        if !(16_000..=128_000).contains(&settings.bitrate_bps) {
            return Err(CodecError(
                "Opus bitrate must be within 16-128 kbit/s".to_string(),
            ));
        }
        let mut encoder =
            opus::Encoder::new(48_000, channels, opus::Application::Audio).map_err(opus_error)?;
        encoder
            .set_bitrate(opus::Bitrate::Bits(settings.bitrate_bps as i32))
            .map_err(opus_error)?;
        encoder.set_complexity(5).map_err(opus_error)?;
        Ok(Self {
            encoder,
            channels: settings.channels,
            pending: VecDeque::new(),
            pending_timestamp: None,
            expected_input_timestamp: None,
            discontinuity_count: 0,
        })
    }

    pub fn discontinuity_count(&self) -> u64 {
        self.discontinuity_count
    }

    pub fn reset(&mut self) -> CodecResult<()> {
        self.pending.clear();
        self.pending_timestamp = None;
        self.expected_input_timestamp = None;
        self.encoder.reset_state().map_err(opus_error)
    }

    /// Aggregate arbitrary native buffer sizes into 20 ms Opus packets. A capture gap drops any partial packet.
    pub fn encode(&mut self, buffer: &AudioBuffer) -> CodecResult<Vec<EncodedAudioPacket>> {
        if buffer.sample_rate_hz != 48_000
            || buffer.channels != self.channels
            || buffer.frames == 0
            || buffer.frames > 48_000
            || buffer.samples.len() != buffer.frames as usize * usize::from(self.channels)
            || buffer.samples.iter().any(|sample| !sample.is_finite())
        {
            return Err(CodecError("Opus input must be finite interleaved 48 kHz PCM with matching channels and exact sample count".to_string()));
        }
        let duration = u64::from(buffer.frames) * 1_000_000 / 48_000;
        let expected = buffer
            .timestamp_micros
            .checked_add(duration)
            .ok_or_else(|| CodecError("Opus timestamp overflow".to_string()))?;
        if self
            .expected_input_timestamp
            .is_some_and(|previous| buffer.timestamp_micros.abs_diff(previous) > 1_000)
        {
            self.reset()?;
            self.discontinuity_count += 1;
        }
        if self.pending.is_empty() {
            self.pending_timestamp = Some(buffer.timestamp_micros);
        }
        self.expected_input_timestamp = Some(expected);
        self.pending
            .extend(buffer.samples.iter().map(|sample| sample.clamp(-1.0, 1.0)));
        let count = PACKET_FRAMES * usize::from(self.channels);
        let mut packets = Vec::new();
        while self.pending.len() >= count {
            let pcm: Vec<f32> = self.pending.drain(..count).collect();
            let mut bytes = vec![0; 1275];
            let length = self
                .encoder
                .encode_float(&pcm, &mut bytes)
                .map_err(opus_error)?;
            bytes.truncate(length);
            let timestamp = self.pending_timestamp.expect("PCM timestamp initialized");
            self.pending_timestamp = Some(
                timestamp
                    .checked_add(PACKET_MICROS)
                    .ok_or_else(|| CodecError("Opus timestamp overflow".to_string()))?,
            );
            packets.push(EncodedAudioPacket {
                bytes,
                channels: self.channels,
                timestamp_micros: timestamp,
                duration: Duration::from_micros(PACKET_MICROS),
            });
        }
        Ok(packets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn audio(frames: u32, timestamp: u64) -> AudioBuffer {
        let mut samples = Vec::new();
        for i in 0..frames {
            let wave = (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.2;
            samples.extend([wave, -wave]);
        }
        AudioBuffer {
            sample_rate_hz: 48_000,
            channels: 2,
            frames,
            timestamp_micros: timestamp,
            samples,
        }
    }
    #[test]
    fn arbitrary_capture_buffers_form_decodable_twenty_millisecond_packets() {
        let mut encoder = OpusEncoder::new(AudioEncoderSettings::default()).unwrap();
        assert!(encoder.encode(&audio(480, 1_000_000)).unwrap().is_empty());
        let packets = encoder.encode(&audio(1440, 1_010_000)).unwrap();
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].timestamp_micros, 1_000_000);
        assert_eq!(packets[1].timestamp_micros, 1_020_000);
        let mut decoder = opus::Decoder::new(48_000, opus::Channels::Stereo).unwrap();
        let mut decoded = vec![0.0; 1920];
        for packet in packets {
            assert_eq!(
                decoder
                    .decode_float(&packet.bytes, &mut decoded, false)
                    .unwrap(),
                960
            );
            assert_eq!(packet.duration, Duration::from_millis(20));
        }
        assert!(decoded.iter().any(|sample| sample.abs() > 0.05));
    }
    #[test]
    fn gaps_drop_partial_pcm_instead_of_mixing_unrelated_timestamps() {
        let mut encoder = OpusEncoder::new(AudioEncoderSettings::default()).unwrap();
        assert!(encoder.encode(&audio(480, 1_000_000)).unwrap().is_empty());
        let packets = encoder.encode(&audio(960, 2_000_000)).unwrap();
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].timestamp_micros, 2_000_000);
        assert_eq!(encoder.discontinuity_count(), 1);
    }
    #[test]
    fn malformed_audio_is_rejected_without_poisoning_encoder_state() {
        let mut encoder = OpusEncoder::new(AudioEncoderSettings::default()).unwrap();
        let mut invalid = audio(960, 1);
        invalid.samples[0] = f32::NAN;
        assert!(encoder.encode(&invalid).is_err());
        invalid = audio(960, 1);
        invalid.samples.pop();
        assert!(encoder.encode(&invalid).is_err());
        assert_eq!(encoder.encode(&audio(960, 1)).unwrap().len(), 1);
        encoder.reset().unwrap();
        assert!(encoder.pending.is_empty());
    }
}
