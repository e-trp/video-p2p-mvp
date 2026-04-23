use crate::session::{SessionIntent, SessionManager};
use std::error::Error;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct WebRtcHostConfig {
    pub room: String,
    pub signaling_addr: String,
    pub source_label: String,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone)]
pub struct WebRtcViewerConfig {
    pub room: String,
    pub signaling_addr: String,
    pub timeout_ms: u64,
}

pub fn run_webrtc_host(config: WebRtcHostConfig) -> Result<(), Box<dyn Error>> {
    let mut manager = SessionManager::new();
    let snapshot = manager.start_host(SessionIntent {
        room: config.room.clone(),
        signaling_addr: config.signaling_addr.clone(),
        source_label: Some(config.source_label.clone()),
    });
    println!(
        "webrtc host prepared: room={} signaling={} transport_state={}",
        config.room, config.signaling_addr, snapshot.transport_state
    );

    let snapshot = manager.create_local_offer();
    println!(
        "local offer path armed: signaling_connected={} transport_state={}",
        snapshot.signaling_connected, snapshot.transport_state
    );
    wait_until_live(&mut manager, config.timeout_ms)
}

pub fn run_webrtc_viewer(config: WebRtcViewerConfig) -> Result<(), Box<dyn Error>> {
    let mut manager = SessionManager::new();
    let snapshot = manager.start_viewer(SessionIntent {
        room: config.room.clone(),
        signaling_addr: config.signaling_addr.clone(),
        source_label: None,
    });
    println!(
        "webrtc viewer prepared: room={} signaling={} transport_state={}",
        config.room, config.signaling_addr, snapshot.transport_state
    );
    wait_until_live(&mut manager, config.timeout_ms)
}

fn wait_until_live(manager: &mut SessionManager, timeout_ms: u64) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let mut last_log_len = manager.logs().len();

    loop {
        let snapshot = manager.refresh();
        if snapshot.logs.len() > last_log_len {
            for line in snapshot.logs.iter().skip(last_log_len) {
                println!("{line}");
            }
            last_log_len = snapshot.logs.len();
        }

        if snapshot.transport_state == "connected" {
            println!(
                "peer connection connected: local_desc={:?} remote_desc={:?} local_ice={} remote_ice={}",
                snapshot.local_description_kind,
                snapshot.remote_description_kind,
                snapshot.local_candidate_count,
                snapshot.remote_candidate_count
            );
            return Ok(());
        }

        if started.elapsed() >= Duration::from_millis(timeout_ms) {
            return Err(format!(
                "timed out waiting for peer connection, last transport state={}",
                snapshot.transport_state
            )
            .into());
        }

        thread::sleep(Duration::from_millis(200));
    }
}
