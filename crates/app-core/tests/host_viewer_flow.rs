use app_core::{
    PeerAnnouncement, Role, SessionIntent, SessionManager, SessionSnapshot, SessionStage,
    decode_signaling_message, encode_error, encode_peer, encode_waiting, parse_join_request,
};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

type SharedRooms = Arc<Mutex<HashMap<String, TestRoom>>>;
type SharedStream = Arc<Mutex<TcpStream>>;
type QueuedWrite = (SharedStream, String);

#[derive(Clone)]
struct TestParticipant {
    role: Role,
    udp_addr: SocketAddr,
    writer: SharedStream,
}

#[derive(Clone)]
struct StoredSignal {
    from_role: Role,
    message: String,
}

#[derive(Default)]
struct TestRoom {
    sender: Option<TestParticipant>,
    receiver: Option<TestParticipant>,
    signaling_history: Vec<StoredSignal>,
}

struct TestTcpSignalingServer {
    addr: String,
    shutdown: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
}

const CONFIG_DIR_OVERRIDE: &str = "VIDEO_P2P_MVP_CONFIG_DIR";

impl TestTcpSignalingServer {
    fn start() -> Option<Self> {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
            Err(error) => panic!("bind test signaling server: {error}"),
        };
        listener
            .set_nonblocking(true)
            .expect("set listener nonblocking");
        let addr = listener.local_addr().expect("listener addr").to_string();
        let rooms = Arc::new(Mutex::new(HashMap::<String, TestRoom>::new()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_for_thread = Arc::clone(&shutdown);
        let rooms_for_thread = Arc::clone(&rooms);

        let accept_thread = thread::spawn(move || {
            while !shutdown_for_thread.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, peer_addr)) => {
                        stream
                            .set_nonblocking(false)
                            .expect("set accepted stream blocking");
                        let rooms = Arc::clone(&rooms_for_thread);
                        thread::spawn(move || {
                            handle_test_client(stream, peer_addr, rooms);
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => break,
                }
            }
        });

        Some(Self {
            addr,
            shutdown,
            accept_thread: Some(accept_thread),
        })
    }

    fn addr(&self) -> &str {
        &self.addr
    }
}

impl Drop for TestTcpSignalingServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(&self.addr);
        if let Some(handle) = self.accept_thread.take() {
            let _ = handle.join();
        }
    }
}

#[test]
fn host_and_viewer_negotiate_over_real_tcp_signaling() {
    with_isolated_preferences("host-viewer-negotiate", || {
        let Some(server) = TestTcpSignalingServer::start() else {
            return;
        };

        let mut host = SessionManager::new();
        let host_start = host.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: server.addr().to_string(),
            source_label: Some("runtime-source".to_string()),
            ice_servers: Vec::new(),
        });
        assert!(host_start.signaling_connected);

        let mut viewer = SessionManager::new();
        let viewer_start = viewer.start_viewer(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: server.addr().to_string(),
            source_label: None,
            ice_servers: Vec::new(),
        });
        assert!(viewer_start.signaling_connected);

        let (host_snapshot, viewer_snapshot) =
            drive_sessions_until_negotiated(&mut host, &mut viewer, Duration::from_secs(10));

        assert!(matches!(
            host_snapshot.stage,
            SessionStage::NegotiatingWebRtc | SessionStage::LiveWebRtc
        ));
        assert!(matches!(
            viewer_snapshot.stage,
            SessionStage::NegotiatingWebRtc | SessionStage::LiveWebRtc
        ));
        assert_eq!(
            host_snapshot.local_description_kind.as_deref(),
            Some("offer")
        );
        assert_eq!(
            host_snapshot.remote_description_kind.as_deref(),
            Some("answer")
        );
        assert_eq!(
            viewer_snapshot.remote_description_kind.as_deref(),
            Some("offer")
        );
        assert!(host_snapshot.active_peer.is_some());
        assert!(viewer_snapshot.active_peer.is_some());
        assert!(
            host_snapshot
                .logs
                .iter()
                .any(|line| line.contains("connected to signaling server"))
        );
        assert!(
            viewer_snapshot
                .logs
                .iter()
                .any(|line| line.contains("remote SDP offer received"))
        );
    });
}

#[test]
fn late_viewer_join_replays_offer_over_real_tcp_signaling() {
    with_isolated_preferences("late-viewer-replay", || {
        let Some(server) = TestTcpSignalingServer::start() else {
            return;
        };

        let mut host = SessionManager::new();
        host.start_host(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: server.addr().to_string(),
            source_label: Some("runtime-source".to_string()),
            ice_servers: Vec::new(),
        });

        let host_offer = wait_until(
            &mut host,
            Duration::from_secs(5),
            |snapshot| snapshot.local_offer_ready,
            "host local offer to be created before viewer joins",
        );
        assert_eq!(host_offer.local_description_kind.as_deref(), Some("offer"));
        assert!(host_offer.active_peer.is_none());
        assert!(
            host_offer
                .logs
                .iter()
                .any(|line| line.contains("signaling server is waiting for the second peer"))
        );

        let mut viewer = SessionManager::new();
        viewer.start_viewer(SessionIntent {
            room: "demo".to_string(),
            signaling_addr: server.addr().to_string(),
            source_label: None,
            ice_servers: Vec::new(),
        });

        let (host_snapshot, viewer_snapshot) =
            drive_sessions_until_negotiated(&mut host, &mut viewer, Duration::from_secs(10));

        assert_eq!(
            host_snapshot.remote_description_kind.as_deref(),
            Some("answer")
        );
        assert_eq!(
            viewer_snapshot.remote_description_kind.as_deref(),
            Some("offer")
        );
        assert!(
            viewer_snapshot
                .logs
                .iter()
                .any(|line| line.contains("remote SDP offer received"))
        );
    });
}

fn with_isolated_preferences(prefix: &str, test: impl FnOnce()) {
    let _guard = preferences_test_lock()
        .lock()
        .expect("preferences test lock poisoned");
    let config_dir = unique_temp_dir(prefix);
    // The process-wide env mutation is serialized behind a test lock.
    unsafe {
        std::env::set_var(CONFIG_DIR_OVERRIDE, &config_dir);
    }
    test();
    unsafe {
        std::env::remove_var(CONFIG_DIR_OVERRIDE);
    }
    let _ = std::fs::remove_dir_all(config_dir);
}

fn preferences_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    static COUNTER: OnceLock<std::sync::atomic::AtomicUsize> = OnceLock::new();
    let counter = COUNTER.get_or_init(|| std::sync::atomic::AtomicUsize::new(0));
    let unique = counter.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "video-p2p-mvp-{prefix}-{}-{}",
        std::process::id(),
        unique
    ))
}

fn drive_sessions_until_negotiated(
    host: &mut SessionManager,
    viewer: &mut SessionManager,
    timeout: Duration,
) -> (SessionSnapshot, SessionSnapshot) {
    let started = Instant::now();
    loop {
        let host_snapshot = host.refresh();
        let viewer_snapshot = viewer.refresh();

        if host_snapshot.remote_description_kind.as_deref() == Some("answer")
            && viewer_snapshot.remote_description_kind.as_deref() == Some("offer")
            && host_snapshot.active_peer.is_some()
            && viewer_snapshot.active_peer.is_some()
        {
            return (host_snapshot, viewer_snapshot);
        }

        assert!(
            started.elapsed() < timeout,
            "timed out waiting for host/viewer negotiation.\nhost={host_snapshot:#?}\nviewer={viewer_snapshot:#?}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn wait_until(
    manager: &mut SessionManager,
    timeout: Duration,
    predicate: impl Fn(&SessionSnapshot) -> bool,
    description: &str,
) -> SessionSnapshot {
    let started = Instant::now();
    loop {
        let snapshot = manager.refresh();
        if predicate(&snapshot) {
            return snapshot;
        }

        assert!(
            started.elapsed() < timeout,
            "timed out waiting for {description}.\nsnapshot={snapshot:#?}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn handle_test_client(stream: TcpStream, peer_addr: SocketAddr, rooms: SharedRooms) {
    let writer = Arc::new(Mutex::new(
        stream.try_clone().expect("clone signaling stream"),
    ));
    let mut reader = BufReader::new(stream);
    let mut first_line = String::new();
    if reader
        .read_line(&mut first_line)
        .expect("read join request")
        == 0
    {
        return;
    }

    let request = match parse_join_request(first_line.trim()) {
        Ok(request) => request,
        Err(error) => {
            let _ = write_message_locked(&writer, &encode_error(&error.to_string()));
            return;
        }
    };

    let room_name = request.room.clone();
    let participant = TestParticipant {
        role: request.role,
        udp_addr: SocketAddr::new(peer_addr.ip(), request.udp_port),
        writer: writer.clone(),
    };

    register_test_participant(&room_name, participant.clone(), &rooms);

    loop {
        let mut line = String::new();
        let bytes = match reader.read_line(&mut line) {
            Ok(bytes) => bytes,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::UnexpectedEof
                        | std::io::ErrorKind::BrokenPipe
                ) =>
            {
                unregister_test_participant(&room_name, participant.role, &rooms);
                return;
            }
            Err(error) => panic!("read signaling line: {error}"),
        };

        if bytes == 0 {
            unregister_test_participant(&room_name, participant.role, &rooms);
            return;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Err(error) = decode_signaling_message(trimmed) {
            let _ = write_message_locked(&writer, &encode_error(&error.to_string()));
            continue;
        }

        relay_test_signaling_message(
            &room_name,
            participant.role,
            normalize_message(&line),
            &rooms,
        );
    }
}

fn register_test_participant(room_name: &str, participant: TestParticipant, rooms: &SharedRooms) {
    let writes = {
        let mut rooms = rooms.lock().expect("rooms poisoned");
        let room = rooms.entry(room_name.to_string()).or_default();
        let slot = participant_slot(room, participant.role);
        if slot.is_some() {
            drop(rooms);
            let _ = write_message_locked(
                &participant.writer,
                &encode_error("role already occupied in this room"),
            );
            return;
        }
        *slot = Some(participant.clone());

        if let Some(opposite) = participant_for_role(room, participant.role.opposite()).cloned() {
            let mut writes = vec![
                (
                    participant.writer.clone(),
                    encode_peer(&PeerAnnouncement {
                        role: opposite.role,
                        addr: opposite.udp_addr,
                    }),
                ),
                (
                    opposite.writer.clone(),
                    encode_peer(&PeerAnnouncement {
                        role: participant.role,
                        addr: participant.udp_addr,
                    }),
                ),
            ];

            for stored in room
                .signaling_history
                .iter()
                .filter(|stored| stored.from_role == participant.role.opposite())
            {
                writes.push((participant.writer.clone(), stored.message.clone()));
            }

            writes
        } else {
            vec![(participant.writer.clone(), encode_waiting())]
        }
    };

    apply_writes(writes);
}

fn relay_test_signaling_message(
    room_name: &str,
    from_role: Role,
    message: String,
    rooms: &SharedRooms,
) {
    let writes = {
        let mut rooms = rooms.lock().expect("rooms poisoned");
        let Some(room) = rooms.get_mut(room_name) else {
            return;
        };

        room.signaling_history.push(StoredSignal {
            from_role,
            message: message.clone(),
        });

        participant_for_role(room, from_role.opposite())
            .map(|participant| vec![(participant.writer.clone(), message)])
            .unwrap_or_default()
    };

    apply_writes(writes);
}

fn unregister_test_participant(room_name: &str, role: Role, rooms: &SharedRooms) {
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

fn participant_slot(room: &mut TestRoom, role: Role) -> &mut Option<TestParticipant> {
    match role {
        Role::Sender => &mut room.sender,
        Role::Receiver => &mut room.receiver,
    }
}

fn participant_for_role(room: &TestRoom, role: Role) -> Option<&TestParticipant> {
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

fn apply_writes(writes: Vec<QueuedWrite>) {
    for (writer, message) in writes {
        let _ = write_message_locked(&writer, &message);
    }
}

fn write_message_locked(stream: &SharedStream, message: &str) -> std::io::Result<()> {
    let mut stream = stream.lock().expect("stream poisoned");
    stream.write_all(message.as_bytes())?;
    stream.flush()?;
    Ok(())
}
