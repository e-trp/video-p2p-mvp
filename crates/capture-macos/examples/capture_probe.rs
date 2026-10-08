//! Opt-in manual capture check. No signaling, network, saved frames, or media encoding.
use capture_core::{
    CaptureSelection, CaptureStreamConfig, CaptureStreamEvent, CaptureStreamRuntime,
};
use std::{
    error::Error,
    thread,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let catalog = capture_macos::current_catalog();
    println!(
        "{} / permission={:?} / origin={:?}",
        catalog.backend_label, catalog.permission_state, catalog.origin
    );
    if args.is_empty() || args == ["--list"] {
        for source in catalog.sources {
            println!(
                "{}\t{}\taudio={}",
                source.id,
                source.label(),
                source.has_audio
            );
        }
        println!(
            "Capture: cargo run -p capture-macos --example capture_probe -- SOURCE_ID [--include-audio]"
        );
        return Ok(());
    }
    if args.len() > 2 || (args.len() == 2 && args[1] != "--include-audio") {
        return Err("usage: capture_probe [--list | SOURCE_ID [--include-audio]]".into());
    }
    let mut runtime = capture_macos::runtime();
    runtime.start(CaptureStreamConfig {
        selection: CaptureSelection {
            source_id: args[0].clone(),
            include_audio: args.len() == 2,
        },
        target_fps: Some(30),
        max_width: Some(1280),
        max_height: Some(720),
    })?;
    let start = Instant::now();
    let (mut video, mut audio) = (0_u64, 0_u64);
    let mut failure = None;
    while start.elapsed() < Duration::from_secs(10) {
        for event in runtime.poll_events()? {
            match event {
                CaptureStreamEvent::VideoFrame { frame, .. } => {
                    video += 1;
                    if video == 1 {
                        println!(
                            "first video: {}x{} / {} bytes / {}us",
                            frame.width,
                            frame.height,
                            frame.bytes.len(),
                            frame.timestamp_micros
                        );
                    }
                }
                CaptureStreamEvent::AudioBuffer { buffer, .. } => {
                    audio += 1;
                    if audio == 1 {
                        println!(
                            "first audio: {}Hz / {}ch / {} frames / {}us",
                            buffer.sample_rate_hz,
                            buffer.channels,
                            buffer.frames,
                            buffer.timestamp_micros
                        );
                    }
                }
                CaptureStreamEvent::Error { message, .. } => {
                    failure = Some(message);
                }
                event => println!("{event:?}"),
            }
        }
        if failure.is_some() {
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    runtime.stop()?;
    println!("captured video={video} audio={audio}; raw capture probe only");
    if let Some(message) = failure {
        return Err(message.into());
    }
    if video == 0 {
        return Err(
            "no video frames received; check Screen Recording permission and source availability"
                .into(),
        );
    }
    Ok(())
}
