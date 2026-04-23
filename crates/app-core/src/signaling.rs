use crate::protocol::{
    PeerAnnouncement, Role, SignalingMessage, decode_signaling_message, encode_signaling_message,
    parse_peer_message,
};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::{Read, Write};
use std::net::TcpStream;

#[derive(Debug)]
pub struct SignalingError(pub String);

impl Display for SignalingError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for SignalingError {}

#[derive(Debug)]
pub enum SignalingEvent {
    Waiting,
    Peer(PeerAnnouncement),
    Message(SignalingMessage),
}

pub struct SignalingConnection {
    stream: TcpStream,
    read_buffer: Vec<u8>,
}

impl SignalingConnection {
    pub fn connect(
        signaling_addr: &str,
        room: &str,
        role: Role,
        udp_port: u16,
    ) -> Result<Self, SignalingError> {
        let mut stream = TcpStream::connect(signaling_addr)
            .map_err(|error| SignalingError(format!("failed to connect signaling server: {error}")))?;
        let request = format!("JOIN {room} {role} {udp_port}\n");
        stream
            .write_all(request.as_bytes())
            .and_then(|_| stream.flush())
            .map_err(|error| SignalingError(format!("failed to send signaling join request: {error}")))?;
        stream
            .set_nonblocking(true)
            .map_err(|error| SignalingError(format!("failed to switch signaling socket to nonblocking: {error}")))?;

        Ok(Self {
            stream,
            read_buffer: Vec::new(),
        })
    }

    pub fn poll(&mut self) -> Result<Vec<SignalingEvent>, SignalingError> {
        let mut chunk = [0_u8; 4096];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    break;
                }
                Ok(size) => {
                    self.read_buffer.extend_from_slice(&chunk[..size]);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(error) => {
                    return Err(SignalingError(format!(
                        "failed to read signaling socket: {error}"
                    )));
                }
            }
        }

        let mut events = Vec::new();
        while let Some(position) = self.read_buffer.iter().position(|byte| *byte == b'\n') {
            let line = self.read_buffer.drain(..=position).collect::<Vec<_>>();
            let line = String::from_utf8(line)
                .map_err(|error| SignalingError(format!("invalid signaling payload: {error}")))?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with("SIG|") {
                events.push(SignalingEvent::Message(
                    decode_signaling_message(trimmed)
                        .map_err(|error| SignalingError(format!("invalid signaling message: {error}")))?,
                ));
                continue;
            }

            match parse_peer_message(trimmed) {
                Ok(Some(peer)) => events.push(SignalingEvent::Peer(peer)),
                Ok(None) => events.push(SignalingEvent::Waiting),
                Err(error) => {
                    return Err(SignalingError(format!(
                        "unexpected signaling server response: {error}"
                    )))
                }
            }
        }

        Ok(events)
    }

    pub fn send(&mut self, message: &SignalingMessage) -> Result<(), SignalingError> {
        let encoded = encode_signaling_message(message);
        self.stream
            .write_all(encoded.as_bytes())
            .and_then(|_| self.stream.flush())
            .map_err(|error| SignalingError(format!("failed to send signaling message: {error}")))
    }
}
