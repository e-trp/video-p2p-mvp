//! Opt-in manual capture check. No signaling, network, saved frames, or media encoding.
use capture_core::{
    CaptureSelection, CaptureStreamConfig, CaptureStreamEvent, CaptureStreamRuntime,
};
use std::{
    error::Error,
    thread,
    time::{Duration, Instant},
};

const USAGE: &str = "usage: capture_probe [--list | SOURCE_ID [--include-audio | --require-audio]]";
// This is a signal-presence check, not a loudness or audible-output measurement.
const MIN_AUDIO_RMS: f64 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioMode {
    Exclude,
    Include,
    RequireSignal,
}

#[derive(Debug, PartialEq, Eq)]
struct CaptureRequest {
    source_id: String,
    audio: AudioMode,
}

fn parse_args(args: &[String]) -> Result<Option<CaptureRequest>, &'static str> {
    if args.is_empty() || args == ["--list"] {
        return Ok(None);
    }
    let audio = match args.get(1).map(String::as_str) {
        None => AudioMode::Exclude,
        Some("--include-audio") => AudioMode::Include,
        Some("--require-audio") => AudioMode::RequireSignal,
        _ => return Err(USAGE),
    };
    if args.len() > 2 || args[0].is_empty() || args[0].starts_with('-') {
        return Err(USAGE);
    }
    Ok(Some(CaptureRequest {
        source_id: args[0].clone(),
        audio,
    }))
}

#[derive(Default)]
struct AudioMeasurements {
    buffers: u64,
    samples: u64,
    square_sum: f64,
    peak: f32,
}

impl AudioMeasurements {
    fn observe(&mut self, samples: &[f32]) -> Result<(), &'static str> {
        if samples.is_empty() || samples.iter().any(|sample| !sample.is_finite()) {
            return Err("capture returned empty or non-finite audio samples");
        }
        let count = u64::try_from(samples.len()).map_err(|_| "audio sample count overflow")?;
        let total = self
            .samples
            .checked_add(count)
            .ok_or("audio sample count overflow")?;
        let buffers = self
            .buffers
            .checked_add(1)
            .ok_or("audio buffer count overflow")?;
        for &sample in samples {
            self.peak = self.peak.max(sample.abs());
            self.square_sum += f64::from(sample).powi(2);
        }
        self.samples = total;
        self.buffers = buffers;
        Ok(())
    }

    fn rms(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            (self.square_sum / self.samples as f64).sqrt()
        }
    }

    fn verify(&self, mode: AudioMode) -> Result<(), &'static str> {
        match mode {
            AudioMode::Exclude if self.buffers != 0 => {
                Err("audio buffers arrived with audio capture disabled")
            }
            AudioMode::RequireSignal if self.rms() <= MIN_AUDIO_RMS => Err(
                "no audio signal above RMS 0.001; play sound in the selected source and check audio routing",
            ),
            _ => Ok(()),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let request = parse_args(&args)?;
    let catalog = capture_macos::current_catalog();
    println!(
        "{} / permission={:?} / origin={:?}",
        catalog.backend_label, catalog.permission_state, catalog.origin
    );
    let Some(request) = request else {
        for source in catalog.sources {
            println!(
                "{}\t{}\taudio={}",
                source.id,
                source.label(),
                source.has_audio
            );
        }
        println!(
            "Capture: cargo run -p capture-macos --example capture_probe -- SOURCE_ID [--include-audio | --require-audio]"
        );
        return Ok(());
    };
    let mut runtime = capture_macos::runtime();
    runtime.start(CaptureStreamConfig {
        selection: CaptureSelection {
            source_id: request.source_id,
            include_audio: request.audio != AudioMode::Exclude,
        },
        target_fps: Some(30),
        max_width: Some(1280),
        max_height: Some(720),
    })?;
    let observed = observe_capture(&mut runtime);
    // Stop explicitly even when polling or sample validation fails.
    let stopped = runtime.stop();
    let (video, audio) = observed?;
    stopped?;
    println!(
        "captured video={video} audio={} samples={} peak={:.6} rms={:.6}; raw capture probe only",
        audio.buffers,
        audio.samples,
        audio.peak,
        audio.rms()
    );
    if video == 0 {
        return Err(
            "no video frames received; check Screen Recording permission and source availability"
                .into(),
        );
    }
    audio.verify(request.audio)?;
    Ok(())
}

fn observe_capture(
    runtime: &mut dyn CaptureStreamRuntime,
) -> Result<(u64, AudioMeasurements), Box<dyn Error>> {
    let start = Instant::now();
    let mut video = 0_u64;
    let mut audio = AudioMeasurements::default();
    while start.elapsed() < Duration::from_secs(10) {
        for event in runtime.poll_events()? {
            match event {
                CaptureStreamEvent::VideoFrame { frame, .. } => {
                    video = video.checked_add(1).ok_or("video frame count overflow")?;
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
                    audio.observe(&buffer.samples)?;
                    if audio.buffers == 1 {
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
                    return Err(message.into());
                }
                event => println!("{event:?}"),
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok((video, audio))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_audio_modes_preserve_opt_out_and_diagnostic_capture() {
        let parse =
            |args: &[&str]| parse_args(&args.iter().map(|arg| (*arg).into()).collect::<Vec<_>>());
        assert_eq!(parse(&[]).unwrap(), None);
        assert_eq!(parse(&["--list"]).unwrap(), None);
        for (args, expected) in [
            (vec!["source"], AudioMode::Exclude),
            (vec!["source", "--include-audio"], AudioMode::Include),
            (vec!["source", "--require-audio"], AudioMode::RequireSignal),
        ] {
            assert_eq!(parse(&args).unwrap().unwrap().audio, expected);
        }
        for args in [
            vec!["--require-audio"],
            vec!["source", "--unknown"],
            vec!["source", "--include-audio", "--require-audio"],
            vec!["--list", "--include-audio"],
            vec![""],
        ] {
            assert!(
                parse(&args).is_err(),
                "accepted invalid arguments: {args:?}"
            );
        }
    }

    #[test]
    fn rms_weights_samples_across_buffers_and_handles_negative_pcm() {
        let mut measurements = AudioMeasurements::default();
        measurements.observe(&[0.5, -0.5]).unwrap();
        measurements.observe(&[0.0; 6]).unwrap();
        assert_eq!(measurements.peak, 0.5);
        assert_eq!(measurements.rms(), 0.25);
        assert_eq!(measurements.samples, 8);
        assert_eq!(measurements.buffers, 2);
        assert!(measurements.verify(AudioMode::RequireSignal).is_ok());
        assert!(measurements.verify(AudioMode::Exclude).is_err());
    }

    #[test]
    fn no_buffers_and_silence_are_distinct_from_required_audio_signal() {
        let mut measurements = AudioMeasurements::default();
        assert_eq!(measurements.rms(), 0.0);
        assert!(measurements.verify(AudioMode::Exclude).is_ok());
        assert!(measurements.verify(AudioMode::RequireSignal).is_err());
        measurements.observe(&[0.0; 100]).unwrap();
        assert!(measurements.verify(AudioMode::Include).is_ok());
        assert!(measurements.verify(AudioMode::RequireSignal).is_err());
        assert!(measurements.verify(AudioMode::Exclude).is_err());
        for (sample, passes) in [(0.000999, false), (0.001001, true)] {
            let mut measurements = AudioMeasurements::default();
            measurements.observe(&[sample; 100]).unwrap();
            assert_eq!(
                measurements.verify(AudioMode::RequireSignal).is_ok(),
                passes
            );
        }
    }

    #[test]
    fn malformed_audio_does_not_poison_prior_measurements() {
        let mut measurements = AudioMeasurements::default();
        measurements.observe(&[0.5]).unwrap();
        for samples in [
            &[][..],
            &[1.0, f32::NAN],
            &[f32::INFINITY],
            &[f32::NEG_INFINITY],
        ] {
            assert!(measurements.observe(samples).is_err());
            assert_eq!(measurements.samples, 1);
            assert_eq!(measurements.buffers, 1);
            assert_eq!(measurements.peak, 0.5);
            assert_eq!(measurements.rms(), 0.5);
        }
    }
}
