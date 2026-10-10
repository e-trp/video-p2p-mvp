//! Encoded media contracts and platform codecs. Raw capture bytes never masquerade as codec output.
use std::{
    error::Error,
    fmt::{Display, Formatter},
    time::Duration,
};

mod video;
pub use video::{H264Encoder, VideoEncoderSettings};
mod audio;
pub use audio::{AudioEncoderSettings, OpusEncoder};

pub const H264_FMTP: &str =
    "level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedVideoFrame {
    /// One H.264 Annex B access unit. Keyframes include SPS/PPS before the IDR.
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp_micros: u64,
    pub duration: Duration,
    pub keyframe: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedAudioPacket {
    /// One raw Opus packet, with no Ogg/container headers.
    pub bytes: Vec<u8>,
    pub channels: u16,
    pub timestamp_micros: u64,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecError(pub String);

impl Display for CodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl Error for CodecError {}
pub type CodecResult<T> = Result<T, CodecError>;
