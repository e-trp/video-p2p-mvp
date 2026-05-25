use crate::protocol::{
    MediaPacket, PeerAnnouncement, Role, decode_media_packet, encode_media_packet,
    parse_peer_message,
};
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, UdpSocket};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct SenderConfig {
    pub room: String,
    pub signaling_addr: String,
    pub udp_bind: String,
    pub fps: u32,
    pub frames: u64,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct ReceiverConfig {
    pub room: String,
    pub signaling_addr: String,
    pub udp_bind: String,
    pub expected_frames: Option<u64>,
}

pub fn run_sender(config: SenderConfig) -> Result<(), Box<dyn Error>> {
    let socket = UdpSocket::bind(&config.udp_bind)?;
    let local_addr = socket.local_addr()?;
    let peer = wait_for_peer(
        &config.signaling_addr,
        &config.room,
        Role::Sender,
        local_addr.port(),
    )?;
    let frame_interval = frame_interval(config.fps);

    println!(
        "sender ready: udp={}, peer={}, fps={}, frames={}, source={}",
        local_addr, peer.addr, config.fps, config.frames, config.source
    );

    for sequence in 0..config.frames {
        let packet = MediaPacket {
            sequence,
            timestamp_ms: now_ms(),
            source: config.source.clone(),
            payload: format!("mock-frame-{sequence}"),
        };
        socket.send_to(&encode_media_packet(&packet), peer.addr)?;
        println!("sent frame {} to {}", packet.sequence, peer.addr);
        thread::sleep(frame_interval);
    }

    println!("sender finished");
    Ok(())
}

pub fn run_receiver(config: ReceiverConfig) -> Result<(), Box<dyn Error>> {
    let socket = UdpSocket::bind(&config.udp_bind)?;
    let local_addr = socket.local_addr()?;
    let peer = wait_for_peer(
        &config.signaling_addr,
        &config.room,
        Role::Receiver,
        local_addr.port(),
    )?;

    println!("receiver ready: udp={}, peer={}", local_addr, peer.addr);

    let mut buffer = [0_u8; 2048];
    let started = Instant::now();
    let mut received = 0_u64;
    let mut last_sequence = None;

    loop {
        let (size, from) = socket.recv_from(&mut buffer)?;
        if from.ip() != peer.addr.ip() {
            println!("ignored packet from unexpected peer {from}");
            continue;
        }

        let packet = decode_media_packet(&buffer[..size])?;
        let gap = last_sequence
            .map(|previous| packet.sequence.saturating_sub(previous + 1))
            .unwrap_or(0);

        received += 1;
        last_sequence = Some(packet.sequence);

        println!(
            "received frame={} gap={} source={} payload={} age_ms={}",
            packet.sequence,
            gap,
            packet.source,
            packet.payload,
            now_ms().saturating_sub(packet.timestamp_ms)
        );

        if let Some(expected) = config.expected_frames {
            if received >= expected {
                break;
            }
        }
    }

    let elapsed = started.elapsed();
    println!(
        "receiver finished: received={} elapsed_ms={}",
        received,
        elapsed.as_millis()
    );
    Ok(())
}

fn wait_for_peer(
    signaling_addr: &str,
    room: &str,
    role: Role,
    udp_port: u16,
) -> Result<PeerAnnouncement, Box<dyn Error>> {
    let mut stream = TcpStream::connect(signaling_addr)?;
    let request = format!("JOIN {room} {role} {udp_port}\n");
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Err("signaling connection closed before peer announcement".into());
        }

        if let Some(peer) = parse_peer_message(line.trim())? {
            if peer.role != role.opposite() {
                return Err(format!("unexpected peer role: {}", peer.role).into());
            }
            return Ok(peer);
        }
    }
}

fn frame_interval(fps: u32) -> Duration {
    let fps = fps.max(1);
    Duration::from_millis((1000 / fps) as u64)
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_millis()
}
