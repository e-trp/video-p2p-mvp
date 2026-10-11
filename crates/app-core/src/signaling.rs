use crate::protocol::{
    PeerAnnouncement, Role, SignalingMessage, decode_signaling_message, encode_signaling_message,
    parse_peer_message,
};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
#[cfg(test)]
use std::os::unix::net::UnixStream;
use std::time::Duration;

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
    stream: SignalingStream,
    read_buffer: Vec<u8>,
    read_closed: bool,
}

impl SignalingConnection {
    const TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);

    /// Connect to TCP signaling and send the room/role join request.
    ///
    /// The connection becomes nonblocking after the request is sent.
    ///
    /// # Errors
    /// Returns an error if address resolution, connection, the join write, or socket setup fails.
    pub fn connect(
        signaling_addr: &str,
        room: &str,
        role: Role,
        udp_port: u16,
    ) -> Result<Self, SignalingError> {
        let socket_addr = signaling_addr
            .to_socket_addrs()
            .map_err(|error| {
                SignalingError(format!("failed to resolve signaling address: {error}"))
            })?
            .next()
            .ok_or_else(|| {
                SignalingError("failed to resolve signaling address: no socket address".to_string())
            })?;
        let mut stream = TcpStream::connect_timeout(&socket_addr, Self::TCP_CONNECT_TIMEOUT)
            .map_err(|error| {
                SignalingError(format!("failed to connect signaling server: {error}"))
            })?;
        let request = format!("JOIN {room} {role} {udp_port}\n");
        stream
            .write_all(request.as_bytes())
            .and_then(|_| stream.flush())
            .map_err(|error| {
                SignalingError(format!("failed to send signaling join request: {error}"))
            })?;
        stream.set_nonblocking(true).map_err(|error| {
            SignalingError(format!(
                "failed to switch signaling socket to nonblocking: {error}"
            ))
        })?;

        Ok(Self {
            stream: SignalingStream::Tcp(stream),
            read_buffer: Vec::new(),
            read_closed: false,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_unix_stream_for_tests(
        mut stream: UnixStream,
        room: &str,
        role: Role,
        udp_port: u16,
    ) -> Result<Self, SignalingError> {
        let request = format!("JOIN {room} {role} {udp_port}\n");
        stream
            .write_all(request.as_bytes())
            .and_then(|_| stream.flush())
            .map_err(|error| {
                SignalingError(format!("failed to send signaling join request: {error}"))
            })?;
        stream.set_nonblocking(true).map_err(|error| {
            SignalingError(format!(
                "failed to switch signaling socket to nonblocking: {error}"
            ))
        })?;

        Ok(Self {
            stream: SignalingStream::Unix(stream),
            read_buffer: Vec::new(),
            read_closed: false,
        })
    }

    /// Read available signaling events without waiting for additional data.
    ///
    /// Idle reads and incomplete lines remain pending while the server is connected.
    /// Complete final events are returned once; closure is reported on the following poll.
    ///
    /// # Errors
    /// Returns an error for socket failures, invalid messages, server rejection, or EOF.
    /// EOF with an incomplete final line reports a truncated message. Once EOF is observed,
    /// sends fail even if final events were returned before the closure error.
    pub fn poll(&mut self) -> Result<Vec<SignalingEvent>, SignalingError> {
        if self.read_closed {
            return Err(self.closed_error());
        }
        let mut chunk = [0_u8; 4096];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    self.read_closed = true;
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
                    decode_signaling_message(trimmed).map_err(|error| {
                        SignalingError(format!("invalid signaling message: {error}"))
                    })?,
                ));
                continue;
            }

            match parse_peer_message(trimmed) {
                Ok(Some(peer)) => events.push(SignalingEvent::Peer(peer)),
                Ok(None) => events.push(SignalingEvent::Waiting),
                Err(error) => {
                    return Err(SignalingError(format!(
                        "unexpected signaling server response: {error}"
                    )));
                }
            }
        }

        if self.read_closed && events.is_empty() {
            return Err(self.closed_error());
        }
        Ok(events)
    }

    /// Send a signaling envelope on the current connection.
    ///
    /// # Errors
    /// Returns an error after observed EOF or if the socket write/flush fails.
    pub fn send(&mut self, message: &SignalingMessage) -> Result<(), SignalingError> {
        if self.read_closed {
            return Err(self.closed_error());
        }
        let encoded = encode_signaling_message(message);
        self.stream
            .write_all(encoded.as_bytes())
            .and_then(|_| self.stream.flush())
            .map_err(|error| SignalingError(format!("failed to send signaling message: {error}")))
    }

    fn closed_error(&self) -> SignalingError {
        let message = if self.read_buffer.is_empty() {
            "signaling server closed the connection"
        } else {
            "signaling server closed the connection with an incomplete message"
        };
        SignalingError(message.into())
    }
}

enum SignalingStream {
    Tcp(TcpStream),
    #[cfg(test)]
    Unix(UnixStream),
}

impl Read for SignalingStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.read(buf),
            #[cfg(test)]
            Self::Unix(stream) => stream.read(buf),
        }
    }
}

impl Write for SignalingStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Self::Tcp(stream) => stream.write(buf),
            #[cfg(test)]
            Self::Unix(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Self::Tcp(stream) => stream.flush(),
            #[cfg(test)]
            Self::Unix(stream) => stream.flush(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SignalingConnection, SignalingEvent};
    use crate::{IceCandidate, Role, SignalingMessage};
    use std::io::{BufRead, BufReader, Write};
    use std::net::{Shutdown, TcpListener};
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn idle_connection_stays_available_until_server_closes() {
        let (mut connection, server) = open_connection();
        assert!(connection.poll().expect("idle poll succeeds").is_empty());
        server
            .shutdown(Shutdown::Write)
            .expect("close server output");

        assert!(
            connection
                .poll()
                .expect_err("EOF is not idle")
                .0
                .contains("closed")
        );
        assert!(
            connection
                .poll()
                .expect_err("EOF remains terminal")
                .0
                .contains("closed")
        );
        assert!(
            connection
                .send(&candidate())
                .expect_err("closed sends fail")
                .0
                .contains("closed")
        );
    }

    #[test]
    fn final_events_are_delivered_once_before_closed_connection_error() {
        let (mut connection, mut server) = open_connection();
        server
            .write_all(b"WAITING\r\nPEER sender 127.0.0.1 4100\r\nSIG|ICE|0|0|candidate:final\r\n")
            .expect("send final events");
        server
            .shutdown(Shutdown::Write)
            .expect("close server output");

        let events = connection
            .poll()
            .expect("complete final events survive EOF");
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], SignalingEvent::Waiting));
        assert!(
            matches!(&events[1], SignalingEvent::Peer(peer) if peer.role == Role::Sender && peer.addr.port() == 4100)
        );
        assert!(
            matches!(&events[2], SignalingEvent::Message(SignalingMessage::IceCandidate(ice)) if ice.candidate == "candidate:final")
        );
        assert!(
            connection
                .poll()
                .expect_err("next poll reports closure")
                .0
                .contains("closed")
        );
    }

    #[test]
    fn final_server_rejection_keeps_its_reason() {
        let (mut connection, mut server) = open_connection();
        server
            .write_all(b"ERROR role already occupied in this room\n")
            .expect("send rejection");
        server
            .shutdown(Shutdown::Write)
            .expect("close server output");
        let error = connection.poll().expect_err("server rejects join");
        assert_eq!(
            error.0,
            "unexpected signaling server response: role already occupied in this room"
        );
    }

    #[test]
    fn fragmented_messages_are_completed_while_connection_is_open() {
        let (mut connection, mut server) = open_connection();
        server
            .write_all(b"SIG|ICE|0|0|candidate:")
            .expect("send partial line");
        assert!(
            connection
                .poll()
                .expect("partial input is not EOF")
                .is_empty()
        );
        server.write_all(b"final\n").expect("complete line");
        let events = connection.poll().expect("decode complete line");
        assert!(
            matches!(events.as_slice(), [SignalingEvent::Message(message)] if message == &candidate())
        );
        assert!(
            connection
                .poll()
                .expect("open socket remains idle")
                .is_empty()
        );
    }

    #[test]
    fn incomplete_final_message_is_rejected() {
        let (mut connection, mut server) = open_connection();
        server
            .write_all(b"SIG|SDP|offer|v=0")
            .expect("send incomplete line");
        server
            .shutdown(Shutdown::Write)
            .expect("close server output");
        assert!(
            connection
                .poll()
                .expect_err("EOF cannot complete a line")
                .0
                .contains("incomplete message")
        );
    }

    #[test]
    fn complete_final_line_precedes_truncated_tail_error() {
        let (mut connection, mut server) = open_connection();
        server
            .write_all(b"WAITING\nSIG|ICE|0")
            .expect("send complete and partial lines");
        server
            .shutdown(Shutdown::Write)
            .expect("close server output");
        assert!(matches!(
            connection.poll().expect("deliver complete line").as_slice(),
            [SignalingEvent::Waiting]
        ));
        assert!(
            connection
                .poll()
                .expect_err("truncated tail is terminal")
                .0
                .contains("incomplete message")
        );
    }

    #[test]
    fn tcp_eof_reports_closed_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local signaling listener");
        let mut connection = SignalingConnection::connect(
            &listener.local_addr().expect("listen address").to_string(),
            "closure",
            Role::Sender,
            0,
        )
        .expect("connect signaling client");
        let (server, _) = listener.accept().expect("accept signaling client");
        server
            .shutdown(Shutdown::Write)
            .expect("close TCP server output");
        let deadline = Instant::now() + Duration::from_secs(2);
        let error = loop {
            match connection.poll() {
                Err(error) => break error,
                Ok(events) => assert!(events.is_empty()),
            }
            assert!(Instant::now() < deadline, "TCP EOF was not observed");
            thread::sleep(Duration::from_millis(5));
        };
        assert!(error.0.contains("closed"));
    }

    fn open_connection() -> (SignalingConnection, UnixStream) {
        let (client, mut server) = UnixStream::pair().expect("create signaling pair");
        let connection =
            SignalingConnection::from_unix_stream_for_tests(client, "closure", Role::Receiver, 0)
                .expect("prepare signaling client");
        let mut join = String::new();
        BufReader::new(&mut server)
            .read_line(&mut join)
            .expect("read join");
        assert_eq!(join, "JOIN closure receiver 0\n");
        (connection, server)
    }

    fn candidate() -> SignalingMessage {
        SignalingMessage::IceCandidate(IceCandidate {
            candidate: "candidate:final".into(),
            sdp_mid: Some("0".into()),
            sdp_mline_index: Some(0),
        })
    }
}
