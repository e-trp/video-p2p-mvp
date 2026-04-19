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
