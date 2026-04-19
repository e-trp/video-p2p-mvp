use app_core::{
    JoinRequest, PeerAnnouncement, Role, encode_error, encode_peer, encode_waiting, parse_join_request,
};
use std::collections::HashMap;
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone)]
struct Participant {
    role: Role,
    udp_addr: SocketAddr,
    writer: Arc<Mutex<TcpStream>>,
}

#[derive(Default)]
struct Room {
    sender: Option<Participant>,
    receiver: Option<Participant>,
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

fn handle_client(stream: TcpStream, rooms: Arc<Mutex<HashMap<String, Room>>>) -> Result<(), Box<dyn Error>> {
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

    register_participant(stream, peer_addr, request, rooms)
}

fn register_participant(
    stream: TcpStream,
    peer_addr: SocketAddr,
    request: JoinRequest,
    rooms: Arc<Mutex<HashMap<String, Room>>>,
) -> Result<(), Box<dyn Error>> {
    let udp_addr = SocketAddr::new(peer_addr.ip(), request.udp_port);
    let participant = Participant {
        role: request.role,
        udp_addr,
        writer: Arc::new(Mutex::new(stream)),
    };

    let mut rooms = rooms.lock().expect("rooms poisoned");
    let room = rooms.entry(request.room.clone()).or_default();

    let slot = match request.role {
        Role::Sender => &mut room.sender,
        Role::Receiver => &mut room.receiver,
    };

    if slot.is_some() {
        write_message_locked(&participant.writer, &encode_error("role already occupied in this room"))?;
        return Ok(());
    }

    *slot = Some(participant.clone());
    write_message_locked(&participant.writer, &encode_waiting())?;

    if let (Some(sender), Some(receiver)) = (&room.sender, &room.receiver) {
        let sender = sender.clone();
        let receiver = receiver.clone();
        let sender_msg = encode_peer(&PeerAnnouncement {
            role: receiver.role,
            addr: receiver.udp_addr,
        });
        let receiver_msg = encode_peer(&PeerAnnouncement {
            role: sender.role,
            addr: sender.udp_addr,
        });

        write_message_locked(&sender.writer, &sender_msg)?;
        write_message_locked(&receiver.writer, &receiver_msg)?;
        rooms.remove(&request.room);
        println!(
            "room {} paired: sender={} receiver={}",
            request.room, sender.udp_addr, receiver.udp_addr
        );
    } else {
        println!(
            "room {} waiting for {}",
            request.room,
            request.role.opposite()
        );
    }

    Ok(())
}

fn write_message(stream: &TcpStream, message: &str) -> Result<(), Box<dyn Error>> {
    let mut writable = stream.try_clone()?;
    writable.write_all(message.as_bytes())?;
    writable.flush()?;
    Ok(())
}

fn write_message_locked(stream: &Arc<Mutex<TcpStream>>, message: &str) -> Result<(), Box<dyn Error>> {
    let mut stream = stream.lock().expect("stream poisoned");
    stream.write_all(message.as_bytes())?;
    stream.flush()?;
    Ok(())
}
