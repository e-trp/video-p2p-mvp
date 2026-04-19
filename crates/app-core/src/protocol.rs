use std::error::Error;
use std::fmt::{Display, Formatter};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Sender,
    Receiver,
}

impl Role {
    pub fn opposite(self) -> Self {
        match self {
            Self::Sender => Self::Receiver,
            Self::Receiver => Self::Sender,
        }
    }
}

impl Display for Role {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sender => write!(f, "sender"),
            Self::Receiver => write!(f, "receiver"),
        }
    }
}

impl FromStr for Role {
    type Err = ProtocolError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "sender" => Ok(Self::Sender),
            "receiver" => Ok(Self::Receiver),
            _ => Err(ProtocolError(format!("unsupported role: {value}"))),
        }
    }
}

#[derive(Debug)]
pub struct ProtocolError(pub String);

impl Display for ProtocolError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for ProtocolError {}

#[derive(Clone, Debug)]
pub struct JoinRequest {
    pub room: String,
    pub role: Role,
    pub udp_port: u16,
}

#[derive(Clone, Debug)]
pub struct PeerAnnouncement {
    pub role: Role,
    pub addr: SocketAddr,
}

#[derive(Clone, Debug)]
pub struct MediaPacket {
    pub sequence: u64,
    pub timestamp_ms: u128,
    pub source: String,
    pub payload: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SdpType {
    Offer,
    Answer,
}

impl Display for SdpType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Offer => write!(f, "offer"),
            Self::Answer => write!(f, "answer"),
        }
    }
}

impl FromStr for SdpType {
    type Err = ProtocolError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "offer" => Ok(Self::Offer),
            "answer" => Ok(Self::Answer),
            _ => Err(ProtocolError(format!("unsupported sdp type: {value}"))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionDescription {
    pub sdp_type: SdpType,
    pub sdp: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IceCandidate {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignalingMessage {
    SessionDescription(SessionDescription),
    IceCandidate(IceCandidate),
}

pub fn parse_join_request(line: &str) -> Result<JoinRequest, ProtocolError> {
    let parts: Vec<_> = line.split_whitespace().collect();
    if parts.len() != 4 || parts[0] != "JOIN" {
        return Err(ProtocolError("expected: JOIN <room> <sender|receiver> <udp_port>".to_string()));
    }

    Ok(JoinRequest {
        room: parts[1].to_string(),
        role: parts[2].parse()?,
        udp_port: parts[3]
            .parse()
            .map_err(|_| ProtocolError(format!("invalid udp port: {}", parts[3])))?,
    })
}

pub fn encode_waiting() -> String {
    "WAITING\n".to_string()
}

pub fn encode_peer(announcement: &PeerAnnouncement) -> String {
    format!(
        "PEER {} {} {}\n",
        announcement.role,
        announcement.addr.ip(),
        announcement.addr.port()
    )
}

pub fn encode_error(message: &str) -> String {
    format!("ERROR {message}\n")
}

pub fn parse_peer_message(line: &str) -> Result<Option<PeerAnnouncement>, ProtocolError> {
    let trimmed = line.trim();
    if trimmed == "WAITING" {
        return Ok(None);
    }

    let parts: Vec<_> = trimmed.split_whitespace().collect();
    if parts.len() == 4 && parts[0] == "PEER" {
        let role = parts[1].parse()?;
        let ip: IpAddr = parts[2]
            .parse()
            .map_err(|_| ProtocolError(format!("invalid peer ip: {}", parts[2])))?;
        let port: u16 = parts[3]
            .parse()
            .map_err(|_| ProtocolError(format!("invalid peer port: {}", parts[3])))?;
        return Ok(Some(PeerAnnouncement {
            role,
            addr: SocketAddr::new(ip, port),
        }));
    }

    if let Some(message) = trimmed.strip_prefix("ERROR ") {
        return Err(ProtocolError(message.to_string()));
    }

    Err(ProtocolError(format!("unsupported signaling message: {trimmed}")))
}

pub fn encode_media_packet(packet: &MediaPacket) -> Vec<u8> {
    format!(
        "FRAME|{}|{}|{}|{}",
        packet.sequence, packet.timestamp_ms, packet.source, packet.payload
    )
    .into_bytes()
}

pub fn decode_media_packet(bytes: &[u8]) -> Result<MediaPacket, ProtocolError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ProtocolError("media packet is not valid utf-8".to_string()))?;
    let parts: Vec<_> = text.splitn(5, '|').collect();
    if parts.len() != 5 || parts[0] != "FRAME" {
        return Err(ProtocolError("unsupported media packet".to_string()));
    }

    Ok(MediaPacket {
        sequence: parts[1]
            .parse()
            .map_err(|_| ProtocolError(format!("invalid sequence: {}", parts[1])))?,
        timestamp_ms: parts[2]
            .parse()
            .map_err(|_| ProtocolError(format!("invalid timestamp: {}", parts[2])))?,
        source: parts[3].to_string(),
        payload: parts[4].to_string(),
    })
}

pub fn encode_signaling_message(message: &SignalingMessage) -> String {
    match message {
        SignalingMessage::SessionDescription(description) => {
            format!(
                "SIG|SDP|{}|{}\n",
                description.sdp_type,
                escape_payload(&description.sdp)
            )
        }
        SignalingMessage::IceCandidate(candidate) => format!(
            "SIG|ICE|{}|{}|{}\n",
            escape_optional(candidate.sdp_mid.as_deref()),
            candidate
                .sdp_mline_index
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            escape_payload(&candidate.candidate)
        ),
    }
}

pub fn decode_signaling_message(line: &str) -> Result<SignalingMessage, ProtocolError> {
    let trimmed = line.trim();
    let parts: Vec<_> = trimmed.split('|').collect();
    if parts.len() < 4 || parts[0] != "SIG" {
        return Err(ProtocolError(format!("unsupported signaling envelope: {trimmed}")));
    }

    match parts[1] {
        "SDP" if parts.len() == 4 => Ok(SignalingMessage::SessionDescription(SessionDescription {
            sdp_type: parts[2].parse()?,
            sdp: unescape_payload(parts[3]),
        })),
        "ICE" if parts.len() == 5 => Ok(SignalingMessage::IceCandidate(IceCandidate {
            sdp_mid: unescape_optional(parts[2]),
            sdp_mline_index: if parts[3] == "-" {
                None
            } else {
                Some(
                    parts[3]
                        .parse()
                        .map_err(|_| ProtocolError(format!("invalid sdp mline index: {}", parts[3])))?,
                )
            },
            candidate: unescape_payload(parts[4]),
        })),
        other => Err(ProtocolError(format!("unsupported signaling message type: {other}"))),
    }
}

fn escape_optional(value: Option<&str>) -> String {
    value
        .map(escape_payload)
        .unwrap_or_else(|| "-".to_string())
}

fn unescape_optional(value: &str) -> Option<String> {
    if value == "-" {
        None
    } else {
        Some(unescape_payload(value))
    }
}

fn escape_payload(value: &str) -> String {
    value.replace('%', "%25").replace('|', "%7C").replace('\n', "%0A")
}

fn unescape_payload(value: &str) -> String {
    value
        .replace("%0A", "\n")
        .replace("%7C", "|")
        .replace("%25", "%")
}

#[cfg(test)]
mod tests {
    use super::{
        IceCandidate, SdpType, SessionDescription, SignalingMessage, decode_signaling_message,
        encode_signaling_message,
    };

    #[test]
    fn roundtrip_sdp_envelope() {
        let encoded = encode_signaling_message(&SignalingMessage::SessionDescription(
            SessionDescription {
                sdp_type: SdpType::Offer,
                sdp: "v=0\no=- 0 0 IN IP4 127.0.0.1".to_string(),
            },
        ));
        let decoded = decode_signaling_message(encoded.trim()).expect("decode sdp");
        assert_eq!(
            decoded,
            SignalingMessage::SessionDescription(SessionDescription {
                sdp_type: SdpType::Offer,
                sdp: "v=0\no=- 0 0 IN IP4 127.0.0.1".to_string(),
            })
        );
    }

    #[test]
    fn roundtrip_ice_envelope() {
        let encoded = encode_signaling_message(&SignalingMessage::IceCandidate(IceCandidate {
            candidate: "candidate:1 1 udp 123 127.0.0.1 5000 typ host".to_string(),
            sdp_mid: Some("0".to_string()),
            sdp_mline_index: Some(0),
        }));
        let decoded = decode_signaling_message(encoded.trim()).expect("decode ice");
        assert_eq!(
            decoded,
            SignalingMessage::IceCandidate(IceCandidate {
                candidate: "candidate:1 1 udp 123 127.0.0.1 5000 typ host".to_string(),
                sdp_mid: Some("0".to_string()),
                sdp_mline_index: Some(0),
            })
        );
    }
}
