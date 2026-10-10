//! Permission-free playback test peer and JSON/stdio adapter for the real viewer session manager.
use app_core::{SessionIntent, SessionManager, SessionStage, ViewerPlaybackReport, ViewerSignal};
use serde_json::{Value, json};
use std::io::{BufRead, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let address = args
        .get(2)
        .ok_or("usage: playback_probe <host|bridge> <signaling address>")?;
    match args.get(1).map(String::as_str) {
        Some("host") => host(address),
        Some("bridge") => bridge(address),
        _ => Err("expected host or bridge mode".into()),
    }
}

fn bridge(address: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut manager = SessionManager::new();
    let mut stdout = std::io::stdout().lock();
    for line in std::io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line?)?;
        let args = &request["args"];
        let result = (|| -> Result<Value, Box<dyn std::error::Error>> {
            let generation = args["generation"].as_u64().unwrap_or(0);
            Ok(
                match request["command"].as_str().ok_or("missing command")? {
                    "start" => {
                        let session = manager.start_playback_viewer(SessionIntent {
                            room: "playback-probe".into(),
                            signaling_addr: address.into(),
                            source_label: None,
                            ice_servers: Vec::new(),
                        });
                        json!({"mode": "viewer", "stage": "awaiting_peer", "viewer_generation": session.viewer_generation})
                    }
                    "viewer_poll_signals" => {
                        serde_json::to_value(manager.poll_viewer_signaling(generation)?)?
                    }
                    "viewer_send_signal" => {
                        let signal: ViewerSignal = serde_json::from_value(args["signal"].clone())?;
                        manager.send_viewer_signal(generation, signal.into())?;
                        Value::Null
                    }
                    "viewer_report_playback" => {
                        let report: ViewerPlaybackReport =
                            serde_json::from_value(args["report"].clone())?;
                        manager.report_viewer_playback(generation, report)?;
                        Value::Null
                    }
                    "snapshot" | "refresh" => {
                        let session = if request["command"] == "refresh" {
                            manager.refresh()
                        } else {
                            manager.snapshot()
                        };
                        json!({"mode":"viewer","stage":if session.stage == SessionStage::Stopped {"stopped"} else {"active"},"viewer_generation":session.viewer_generation,"video_frames":session.decoded_video_frame_count,"audio_samples":session.received_audio_sample_count,"state":session.viewer_playback_state,"connection":session.transport_state})
                    }
                    "stop" => {
                        manager.stop();
                        Value::Null
                    }
                    _ => return Err("unknown command".into()),
                },
            )
        })();
        let response = match result {
            Ok(value) => json!({"id":request["id"],"result":value}),
            Err(error) => json!({"id":request["id"],"error":error.to_string()}),
        };
        writeln!(stdout, "{response}")?;
        stdout.flush()?;
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn host(_: &str) -> Result<(), Box<dyn std::error::Error>> {
    Err("synthetic H264 host currently requires macOS".into())
}

#[cfg(target_os = "macos")]
fn host(address: &str) -> Result<(), Box<dyn std::error::Error>> {
    use app_core::{
        IceCandidate, Role, SdpType, SessionDescription, SignalingConnection, SignalingEvent,
        SignalingMessage,
    };
    use capture_core::{AudioBuffer, VideoFrame, VideoPixelFormat};
    use media_codec::{H264Encoder, OpusEncoder};
    use std::time::{Duration, Instant};
    use transport_webrtc::{DescriptionKind, TransportSession, WebRtcConfig, WebRtcSignal};
    fn message(signal: WebRtcSignal) -> SignalingMessage {
        match signal {
            WebRtcSignal::SessionDescription { kind, sdp } => {
                SignalingMessage::SessionDescription(SessionDescription {
                    sdp_type: if kind == DescriptionKind::Offer {
                        SdpType::Offer
                    } else {
                        SdpType::Answer
                    },
                    sdp,
                })
            }
            WebRtcSignal::IceCandidate {
                candidate,
                sdp_mid,
                sdp_mline_index,
            } => SignalingMessage::IceCandidate(IceCandidate {
                candidate,
                sdp_mid,
                sdp_mline_index,
            }),
        }
    }
    let mut peer = TransportSession::new(WebRtcConfig {
        room: "playback-probe".into(),
        role: "host".into(),
        signaling_url: address.into(),
        ice_servers: Vec::new(),
    })?;
    let publisher = peer.encoded_publisher()?;
    let mut signaling = SignalingConnection::connect(address, "playback-probe", Role::Sender, 0)?;
    signaling.send(&message(peer.create_local_offer()?))?;
    let mut video = H264Encoder::new(Default::default())?;
    let mut audio = OpusEncoder::new(Default::default())?;
    let mut video_pts = 1_000_000;
    let mut audio_pts = 1_000_000;
    let mut audio_index = 0_u64;
    let start = Instant::now();
    let mut next_video = start;
    let mut next_audio = start;
    while start.elapsed() < Duration::from_secs(40) {
        for event in signaling.poll()? {
            if let SignalingEvent::Message(signal) = event {
                match signal {
                    SignalingMessage::SessionDescription(description)
                        if description.sdp_type == SdpType::Answer =>
                    {
                        peer.accept_remote_answer(description.sdp)?
                    }
                    SignalingMessage::IceCandidate(ice) => peer.add_remote_ice_candidate(
                        ice.candidate,
                        ice.sdp_mid,
                        ice.sdp_mline_index,
                    )?,
                    _ => {}
                }
            }
        }
        for signal in peer.drain_local_signals() {
            signaling.send(&message(signal))?;
        }
        let now = Instant::now();
        if publisher.is_connected() {
            if now >= next_video {
                let frame = VideoFrame {
                    format: VideoPixelFormat::Bgra8,
                    width: 64,
                    height: 36,
                    timestamp_micros: video_pts,
                    bytes: [32, 64, 192, 255].repeat(64 * 36),
                };
                if let Some(frame) = video.encode(&frame, video_pts == 1_000_000)? {
                    publisher.publish_video(frame)?;
                }
                video_pts += 33_333;
                next_video = now + Duration::from_micros(33_333);
            }
            if now >= next_audio {
                let mut samples = Vec::with_capacity(1920);
                for _ in 0..960 {
                    let sample = ((audio_index as f32 * 440.0 * std::f32::consts::TAU) / 48_000.0)
                        .sin()
                        * 0.1;
                    samples.extend([sample, sample]);
                    audio_index += 1;
                }
                for packet in audio.encode(&AudioBuffer {
                    sample_rate_hz: 48_000,
                    channels: 2,
                    frames: 960,
                    timestamp_micros: audio_pts,
                    samples,
                })? {
                    publisher.publish_audio(packet)?;
                }
                audio_pts += 20_000;
                next_audio = now + Duration::from_millis(20);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    peer.close()?;
    Ok(())
}
