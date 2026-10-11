use app_core::{
    PeerAnnouncement, Role, decode_signaling_message, encode_error, encode_peer, encode_waiting,
    parse_join_request,
};
use std::collections::HashMap;
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

type SharedRooms = Arc<Mutex<HashMap<String, Room>>>;
type SharedWriter = Arc<Mutex<Box<dyn Write + Send>>>;
type QueuedWrite = (SharedWriter, String);

#[derive(Clone)]
struct Participant {
    role: Role,
    udp_addr: SocketAddr,
    writer: SharedWriter,
}

#[derive(Clone)]
struct StoredSignal {
    from_role: Role,
    message: String,
}

#[derive(Default)]
struct Room {
    sender: Option<Participant>,
    receiver: Option<Participant>,
    signaling_history: Vec<StoredSignal>,
}

enum RegistrationStatus {
    Waiting,
    Paired {
        sender: SocketAddr,
        receiver: SocketAddr,
    },
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let bind = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:7000".to_string());
    let listener = TcpListener::bind(&bind)?;
    let rooms = Arc::new(Mutex::new(HashMap::<String, Room>::new()));

    println!("signaling server listening on {bind}");

    for stream in listener.incoming() {
        let rooms = Arc::clone(&rooms);
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    if let Err(error) = handle_client(stream, rooms) {
                        eprintln!("signaling client error: {error}");
                    }
                });
            }
            Err(error) => eprintln!("accept error: {error}"),
        }
    }

    Ok(())
}

fn handle_client(stream: TcpStream, rooms: SharedRooms) -> Result<(), Box<dyn Error>> {
    let peer_addr = stream.peer_addr()?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut first_line = String::new();
    reader.read_line(&mut first_line)?;

    let request = match parse_join_request(first_line.trim()) {
        Ok(request) => request,
        Err(error) => {
            write_message(&stream, &encode_error(&error.to_string()))?;
            return Err(Box::new(error));
        }
    };

    let room_name = request.room.clone();
    let writer: SharedWriter = Arc::new(Mutex::new(Box::new(stream)));
    let participant = Participant {
        role: request.role,
        udp_addr: SocketAddr::new(peer_addr.ip(), request.udp_port),
        writer: writer.clone(),
    };

    register_participant(&room_name, participant.clone(), &rooms)?;
    let result = relay_client_messages(&mut reader, &room_name, participant.role, &writer, &rooms);
    unregister_participant(&room_name, participant.role, &rooms);
    result
}

fn register_participant(
    room_name: &str,
    participant: Participant,
    rooms: &SharedRooms,
) -> Result<(), Box<dyn Error>> {
    let (writes, status) = {
        let mut rooms = rooms.lock().expect("rooms poisoned");
        let room = rooms.entry(room_name.to_string()).or_default();
        match register_participant_in_room(room, participant.clone()) {
            Ok(value) => value,
            Err(message) => {
                drop(rooms);
                write_message_locked(&participant.writer, &encode_error(&message))?;
                return Err(message.into());
            }
        }
    };

    if let Err(error) = apply_writes(writes) {
        unregister_participant(room_name, participant.role, rooms);
        return Err(error);
    }

    match status {
        RegistrationStatus::Waiting => {
            println!(
                "room {room_name} waiting for {}",
                participant.role.opposite()
            );
        }
        RegistrationStatus::Paired { sender, receiver } => {
            println!("room {room_name} paired: sender={sender} receiver={receiver}");
        }
    }

    Ok(())
}

fn relay_client_messages(
    reader: &mut BufReader<TcpStream>,
    room_name: &str,
    role: Role,
    writer: &SharedWriter,
    rooms: &SharedRooms,
) -> Result<(), Box<dyn Error>> {
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Err(error) = decode_signaling_message(trimmed) {
            write_message_locked(writer, &encode_error(&error.to_string()))?;
            continue;
        }

        relay_signaling_message(room_name, role, normalize_message(&line), rooms)?;
    }
}

fn relay_signaling_message(
    room_name: &str,
    from_role: Role,
    message: String,
    rooms: &SharedRooms,
) -> Result<(), Box<dyn Error>> {
    let writes = {
        let mut rooms = rooms.lock().expect("rooms poisoned");
        let Some(room) = rooms.get_mut(room_name) else {
            return Ok(());
        };
        relay_signaling_in_room(room, from_role, message)
    };

    apply_writes(writes)?;
    println!(
        "room {room_name} relayed signaling from {} to {}",
        from_role,
        from_role.opposite()
    );
    Ok(())
}

fn register_participant_in_room(
    room: &mut Room,
    participant: Participant,
) -> Result<(Vec<QueuedWrite>, RegistrationStatus), String> {
    let slot = participant_slot(room, participant.role);
    if slot.is_some() {
        return Err("role already occupied in this room".to_string());
    }
    *slot = Some(participant.clone());

    let mut writes = Vec::new();
    if let Some(opposite) = participant_for_role(room, participant.role.opposite()).cloned() {
        writes.push((
            participant.writer.clone(),
            encode_peer(&PeerAnnouncement {
                role: opposite.role,
                addr: opposite.udp_addr,
            }),
        ));
        writes.push((
            opposite.writer.clone(),
            encode_peer(&PeerAnnouncement {
                role: participant.role,
                addr: participant.udp_addr,
            }),
        ));

        for stored in room
            .signaling_history
            .iter()
            .filter(|stored| stored.from_role == participant.role.opposite())
        {
            writes.push((participant.writer.clone(), stored.message.clone()));
        }

        let sender = room
            .sender
            .as_ref()
            .map(|value| value.udp_addr)
            .expect("sender set when room is paired");
        let receiver = room
            .receiver
            .as_ref()
            .map(|value| value.udp_addr)
            .expect("receiver set when room is paired");

        Ok((writes, RegistrationStatus::Paired { sender, receiver }))
    } else {
        writes.push((participant.writer.clone(), encode_waiting()));
        Ok((writes, RegistrationStatus::Waiting))
    }
}

fn relay_signaling_in_room(room: &mut Room, from_role: Role, message: String) -> Vec<QueuedWrite> {
    room.signaling_history.push(StoredSignal {
        from_role,
        message: message.clone(),
    });

    participant_for_role(room, from_role.opposite())
        .map(|participant| vec![(participant.writer.clone(), message)])
        .unwrap_or_default()
}

fn unregister_participant(room_name: &str, role: Role, rooms: &SharedRooms) {
    let mut rooms = rooms.lock().expect("rooms poisoned");
    let mut drop_room = false;

    if let Some(room) = rooms.get_mut(room_name) {
        *participant_slot(room, role) = None;
        room.signaling_history
            .retain(|stored| stored.from_role != role);
        drop_room = room.sender.is_none() && room.receiver.is_none();
    }

    if drop_room {
        rooms.remove(room_name);
    }
}

fn participant_slot(room: &mut Room, role: Role) -> &mut Option<Participant> {
    match role {
        Role::Sender => &mut room.sender,
        Role::Receiver => &mut room.receiver,
    }
}

fn participant_for_role(room: &Room, role: Role) -> Option<&Participant> {
    match role {
        Role::Sender => room.sender.as_ref(),
        Role::Receiver => room.receiver.as_ref(),
    }
}

fn normalize_message(message: &str) -> String {
    if message.ends_with('\n') {
        message.to_string()
    } else {
        format!("{message}\n")
    }
}

fn apply_writes(writes: Vec<QueuedWrite>) -> Result<(), Box<dyn Error>> {
    for (writer, message) in writes {
        write_message_locked(&writer, &message)?;
    }
    Ok(())
}

fn write_message(stream: &TcpStream, message: &str) -> Result<(), Box<dyn Error>> {
    let mut writable = stream.try_clone()?;
    writable.write_all(message.as_bytes())?;
    writable.flush()?;
    Ok(())
}

fn write_message_locked(stream: &SharedWriter, message: &str) -> Result<(), Box<dyn Error>> {
    let mut stream = stream.lock().expect("stream poisoned");
    stream.write_all(message.as_bytes())?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Participant, RegistrationStatus, Room, SharedRooms, SharedWriter, apply_writes,
        handle_client, register_participant, register_participant_in_room, relay_signaling_in_room,
        relay_signaling_message, unregister_participant,
    };
    use app_core::Role;
    use std::collections::HashMap;
    use std::io::{BufRead, BufReader, ErrorKind, Write};
    use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn rejected_clients_cannot_relay_or_remove_registered_peers() {
        for role in [Role::Sender, Role::Receiver] {
            let rooms: SharedRooms = Arc::new(Mutex::new(HashMap::new()));
            let (accepted_writer, accepted_buffer) = memory_writer();
            let (opposite_writer, opposite_buffer) = memory_writer();
            let (accepted, _) = make_participant(role, 4100, accepted_writer.clone());
            let (opposite, _) = make_participant(role.opposite(), 4200, opposite_writer);
            register_participant("duplicate", accepted, &rooms).expect("register accepted peer");
            register_participant("duplicate", opposite, &rooms).expect("register opposite peer");
            relay_signaling_message(
                "duplicate",
                role,
                "SIG|ICE|0|0|candidate:accepted\n".to_string(),
                &rooms,
            )
            .expect("store accepted peer history");
            let accepted_before = buffer_text(&accepted_buffer);
            let opposite_before = buffer_text(&opposite_buffer);

            let (reply, result) = serve_client_input(
                &rooms,
                &format!("JOIN duplicate {role} 4300\nSIG|ICE|0|0|candidate:rejected\n"),
            );

            assert_eq!(reply, "ERROR role already occupied in this room\n");
            assert_eq!(buffer_text(&accepted_buffer), accepted_before);
            assert_eq!(buffer_text(&opposite_buffer), opposite_before);
            let rooms = rooms.lock().expect("rooms poisoned");
            let room = rooms.get("duplicate").expect("accepted room remains");
            let accepted = super::participant_for_role(room, role).expect("accepted peer remains");
            assert!(Arc::ptr_eq(&accepted.writer, &accepted_writer));
            assert_eq!(room.signaling_history.len(), 1);
            assert_eq!(
                room.signaling_history[0].message,
                "SIG|ICE|0|0|candidate:accepted\n"
            );
            assert!(
                result
                    .expect_err("registration must fail")
                    .contains("role already occupied")
            );
        }
    }

    #[test]
    fn accepted_client_relays_then_disconnects_without_removing_opposite_peer() {
        for role in [Role::Sender, Role::Receiver] {
            let rooms: SharedRooms = Arc::new(Mutex::new(HashMap::new()));
            let (writer, buffer) = memory_writer();
            let (opposite, _) = make_participant(role.opposite(), 4100, writer.clone());
            register_participant("disconnect", opposite, &rooms).expect("register opposite peer");
            relay_signaling_message(
                "disconnect",
                role.opposite(),
                "SIG|ICE|0|0|candidate:opposite\n".to_string(),
                &rooms,
            )
            .expect("store opposite history");

            let (reply, result) = serve_client_input(
                &rooms,
                &format!("JOIN disconnect {role} 4200\nSIG|ICE|0|0|candidate:accepted\n"),
            );
            result.expect("accepted client completes normally");
            assert_eq!(reply, format!("PEER {} 127.0.0.1 4100\n", role.opposite()));
            assert_eq!(
                buffer_text(&buffer),
                format!("WAITING\nPEER {role} 127.0.0.1 4200\nSIG|ICE|0|0|candidate:accepted\n")
            );
            {
                let rooms = rooms.lock().expect("rooms poisoned");
                let room = rooms.get("disconnect").expect("opposite room remains");
                assert!(super::participant_for_role(room, role).is_none());
                let opposite = super::participant_for_role(room, role.opposite())
                    .expect("opposite peer remains");
                assert!(Arc::ptr_eq(&opposite.writer, &writer));
                assert_eq!(room.signaling_history.len(), 1);
                assert_eq!(room.signaling_history[0].from_role, role.opposite());
            }
            unregister_participant("disconnect", role.opposite(), &rooms);
            assert!(rooms.lock().expect("rooms poisoned").is_empty());
        }
    }

    #[test]
    fn failed_waiting_write_releases_room_and_allows_retry() {
        for role in [Role::Sender, Role::Receiver] {
            let rooms: SharedRooms = Arc::new(Mutex::new(HashMap::new()));
            let (failed, _) = make_participant(role, 4100, failing_writer(0));
            assert!(register_participant("retry", failed, &rooms).is_err());
            assert!(rooms.lock().expect("rooms poisoned").is_empty());

            let (writer, buffer) = memory_writer();
            let (replacement, _) = make_participant(role, 4200, writer);
            register_participant("retry", replacement, &rooms).expect("reuse released role");
            assert_eq!(buffer_text(&buffer), "WAITING\n");
        }
    }

    #[test]
    fn failed_pairing_or_replay_preserves_opposite_peer_and_history() {
        for role in [Role::Sender, Role::Receiver] {
            // Fail the new peer announcement, the existing peer announcement, or replay.
            for phase in 0..3 {
                let rooms: SharedRooms = Arc::new(Mutex::new(HashMap::new()));
                let opposite_writer = if phase == 1 {
                    failing_writer(1)
                } else {
                    memory_writer().0
                };
                let (opposite, _) =
                    make_participant(role.opposite(), 4100, opposite_writer.clone());
                register_participant("paired", opposite, &rooms).expect("register opposite peer");
                relay_signaling_message(
                    "paired",
                    role.opposite(),
                    "SIG|ICE|0|0|candidate:opposite\n".to_string(),
                    &rooms,
                )
                .expect("store opposite history");
                let writer = match phase {
                    0 => failing_writer(0),
                    1 => memory_writer().0,
                    _ => failing_writer(1),
                };
                let (failed, _) = make_participant(role, 4200, writer);
                assert!(register_participant("paired", failed, &rooms).is_err());
                {
                    let rooms = rooms.lock().expect("rooms poisoned");
                    let room = rooms.get("paired").expect("opposite room remains");
                    assert!(super::participant_for_role(room, role).is_none());
                    let opposite = super::participant_for_role(room, role.opposite())
                        .expect("opposite peer remains");
                    assert!(Arc::ptr_eq(&opposite.writer, &opposite_writer));
                    assert_eq!(room.signaling_history.len(), 1);
                    assert_eq!(room.signaling_history[0].from_role, role.opposite());
                    assert_eq!(
                        room.signaling_history[0].message,
                        "SIG|ICE|0|0|candidate:opposite\n"
                    );
                }

                if phase != 1 {
                    let (writer, buffer) = memory_writer();
                    let (replacement, _) = make_participant(role, 4300, writer);
                    register_participant("paired", replacement, &rooms)
                        .expect("retry pairing after rollback");
                    assert!(buffer_text(&buffer).ends_with("SIG|ICE|0|0|candidate:opposite\n"));
                }
            }
        }
    }

    fn serve_client_input(rooms: &SharedRooms, input: &str) -> (String, Result<(), String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let mut client = TcpStream::connect(listener.local_addr().expect("listen address"))
            .expect("connect client");
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("bound client read");
        let (server_stream, _) = listener.accept().expect("accept client");
        server_stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("bound server read");
        let server_rooms = Arc::clone(rooms);
        let handler = thread::spawn(move || {
            handle_client(server_stream, server_rooms).map_err(|error| error.to_string())
        });
        client
            .write_all(input.as_bytes())
            .expect("send client input");
        client.shutdown(Shutdown::Write).expect("end client input");
        let mut reader = BufReader::new(client);
        let mut reply = String::new();
        reader
            .read_line(&mut reply)
            .expect("read registration reply");
        let result = handler.join().expect("join client handler");
        (reply, result)
    }

    fn failing_writer(successful_writes: usize) -> SharedWriter {
        Arc::new(Mutex::new(Box::new(FailingWriter { successful_writes })))
    }

    struct FailingWriter {
        successful_writes: usize,
    }

    impl Write for FailingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if self.successful_writes == 0 {
                return Err(std::io::Error::new(
                    ErrorKind::BrokenPipe,
                    "test write failure",
                ));
            }
            self.successful_writes -= 1;
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn relay_sends_message_to_paired_peer() {
        let (sender_writer, sender_buffer) = memory_writer();
        let (receiver_writer, receiver_buffer) = memory_writer();
        let mut room = Room::default();

        let sender_addr = parse_addr(4100);
        let receiver_addr = parse_addr(4200);

        apply_writes(
            register_participant_in_room(
                &mut room,
                Participant {
                    role: Role::Sender,
                    udp_addr: sender_addr,
                    writer: sender_writer,
                },
            )
            .expect("register sender")
            .0,
        )
        .expect("write sender waiting");
        apply_writes(
            register_participant_in_room(
                &mut room,
                Participant {
                    role: Role::Receiver,
                    udp_addr: receiver_addr,
                    writer: receiver_writer,
                },
            )
            .expect("register receiver")
            .0,
        )
        .expect("write paired state");

        assert_eq!(
            buffer_text(&sender_buffer),
            "WAITING\nPEER receiver 127.0.0.1 4200\n"
        );
        assert_eq!(
            buffer_text(&receiver_buffer),
            "PEER sender 127.0.0.1 4100\n"
        );

        apply_writes(relay_signaling_in_room(
            &mut room,
            Role::Sender,
            "SIG|SDP|offer|v=0\n".to_string(),
        ))
        .expect("relay offer");

        assert_eq!(
            buffer_text(&receiver_buffer),
            "PEER sender 127.0.0.1 4100\nSIG|SDP|offer|v=0\n"
        );
        assert_eq!(room.signaling_history.len(), 1);
    }

    #[test]
    fn late_peer_receives_replayed_signaling_history() {
        let (sender_writer, sender_buffer) = memory_writer();
        let (receiver_writer, receiver_buffer) = memory_writer();
        let mut room = Room::default();

        let (sender, _) = make_participant(Role::Sender, 5100, sender_writer);
        let (receiver, _) = make_participant(Role::Receiver, 5200, receiver_writer);

        let (writes, status) =
            register_participant_in_room(&mut room, sender.clone()).expect("register sender");
        apply_writes(writes).expect("write waiting");
        assert!(matches!(status, RegistrationStatus::Waiting));
        assert_eq!(buffer_text(&sender_buffer), "WAITING\n");

        apply_writes(relay_signaling_in_room(
            &mut room,
            Role::Sender,
            "SIG|ICE|0|0|candidate:demo\n".to_string(),
        ))
        .expect("store early ice");

        let (writes, status) =
            register_participant_in_room(&mut room, receiver).expect("register receiver");
        apply_writes(writes).expect("write pair and replay");
        assert!(matches!(status, RegistrationStatus::Paired { .. }));

        assert_eq!(
            buffer_text(&sender_buffer),
            "WAITING\nPEER receiver 127.0.0.1 5200\n"
        );
        assert_eq!(
            buffer_text(&receiver_buffer),
            "PEER sender 127.0.0.1 5100\nSIG|ICE|0|0|candidate:demo\n"
        );
    }

    fn make_participant(role: Role, port: u16, writer: SharedWriter) -> (Participant, SocketAddr) {
        let addr = parse_addr(port);
        (
            Participant {
                role,
                udp_addr: addr,
                writer,
            },
            addr,
        )
    }

    fn parse_addr(port: u16) -> SocketAddr {
        format!("127.0.0.1:{port}").parse().expect("socket addr")
    }

    fn memory_writer() -> (SharedWriter, Arc<Mutex<Vec<u8>>>) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let writer: SharedWriter = Arc::new(Mutex::new(Box::new(BufferWriter {
            buffer: buffer.clone(),
        })));
        (writer, buffer)
    }

    fn buffer_text(buffer: &Arc<Mutex<Vec<u8>>>) -> String {
        String::from_utf8(buffer.lock().expect("buffer poisoned").clone()).expect("utf8 output")
    }

    struct BufferWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for BufferWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.buffer
                .lock()
                .expect("buffer poisoned")
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}
