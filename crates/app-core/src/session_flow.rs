use crate::ice_servers::IceServerEntry;
use crate::session::{SessionIntent, SessionManager};
use std::error::Error;
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct WebRtcHostConfig {
    pub room: String,
    pub signaling_addr: String,
    pub source_label: String,
    pub ice_servers: Vec<IceServerEntry>,
    pub timeout_ms: u64,
    pub push_debug_capture: bool,
}

#[derive(Debug, Clone)]
pub struct WebRtcViewerConfig {
    pub room: String,
    pub signaling_addr: String,
    pub ice_servers: Vec<IceServerEntry>,
    pub timeout_ms: u64,
}

pub fn run_webrtc_host(config: WebRtcHostConfig) -> Result<(), Box<dyn Error>> {
    let mut manager = SessionManager::new();
    let snapshot = manager.start_host(SessionIntent {
        room: config.room.clone(),
        signaling_addr: config.signaling_addr.clone(),
        source_label: Some(config.source_label.clone()),
        ice_servers: config.ice_servers.clone(),
    });
    println!(
        "webrtc host prepared: room={} signaling={} transport_state={}",
        config.room, config.signaling_addr, snapshot.transport_state
    );

    wait_until_live(&mut manager, config.timeout_ms)?;

    if config.push_debug_capture {
        let snapshot = manager.publish_debug_capture_samples();
        println!(
            "debug capture burst published: video_samples={} audio_samples={} video_payload={:?} audio_payload={:?}",
            snapshot.published_video_sample_count,
            snapshot.published_audio_sample_count,
            snapshot.last_video_capture_summary,
            snapshot.last_audio_capture_summary
        );
    } else {
        println!(
            "host connected without debug capture burst; pass --push-debug-capture to exercise the session media bridge"
        );
    }

    Ok(())
}

pub fn run_webrtc_viewer(config: WebRtcViewerConfig) -> Result<(), Box<dyn Error>> {
    let mut manager = SessionManager::new();
    let snapshot = manager.start_viewer(SessionIntent {
        room: config.room.clone(),
        signaling_addr: config.signaling_addr.clone(),
        source_label: None,
        ice_servers: config.ice_servers.clone(),
    });
    println!(
        "webrtc viewer prepared: room={} signaling={} transport_state={}",
        config.room, config.signaling_addr, snapshot.transport_state
    );
    wait_until_live(&mut manager, config.timeout_ms)?;
    Ok(())
}

fn wait_until_live(
    manager: &mut SessionManager,
    timeout_ms: u64,
) -> Result<crate::session::SessionSnapshot, Box<dyn Error>> {
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
            return Ok(snapshot);
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
