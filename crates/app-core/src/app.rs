use crate::mock_media::{ReceiverConfig, SenderConfig};
use crate::session_flow::{WebRtcHostConfig, WebRtcViewerConfig};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum AppCommand {
    Help,
    PrintSpec,
    Sender(SenderConfig),
    Receiver(ReceiverConfig),
    WebRtcHost(WebRtcHostConfig),
    WebRtcViewer(WebRtcViewerConfig),
}

#[derive(Debug)]
pub struct CliError(pub String);

impl Display for CliError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for CliError {}

pub fn parse_cli_args(args: &[String]) -> Result<AppCommand, Box<dyn Error>> {
    let Some(command) = args.first() else {
        return Ok(AppCommand::Help);
    };

    match command.as_str() {
        "help" | "--help" | "-h" => Ok(AppCommand::Help),
        "spec" => Ok(AppCommand::PrintSpec),
        "sender" => {
            let room = required_flag(&args[1..], "--room")?;
            let signaling_addr =
                parse_flag(&args[1..], "--signal")?.unwrap_or_else(|| "127.0.0.1:7000".to_string());
            let udp_bind =
                parse_flag(&args[1..], "--udp-bind")?.unwrap_or_else(|| "0.0.0.0:0".to_string());
            let fps = parse_flag(&args[1..], "--fps")?
                .map(|value| value.parse())
                .transpose()?
                .unwrap_or(10);
            let frames = parse_flag(&args[1..], "--frames")?
                .map(|value| value.parse())
                .transpose()?
                .unwrap_or(120);
            let source =
                parse_flag(&args[1..], "--source")?.unwrap_or_else(|| "mock-window".to_string());

            Ok(AppCommand::Sender(SenderConfig {
                room,
                signaling_addr,
                udp_bind,
                fps,
                frames,
                source,
            }))
        }
        "receiver" => {
            let room = required_flag(&args[1..], "--room")?;
            let signaling_addr =
                parse_flag(&args[1..], "--signal")?.unwrap_or_else(|| "127.0.0.1:7000".to_string());
            let udp_bind =
                parse_flag(&args[1..], "--udp-bind")?.unwrap_or_else(|| "0.0.0.0:0".to_string());
            let expected_frames = parse_flag(&args[1..], "--expected-frames")?
                .map(|value| value.parse())
                .transpose()?;

            Ok(AppCommand::Receiver(ReceiverConfig {
                room,
                signaling_addr,
                udp_bind,
                expected_frames,
            }))
        }
        "webrtc-host" => {
            let room = required_flag(&args[1..], "--room")?;
            let signaling_addr =
                parse_flag(&args[1..], "--signal")?.unwrap_or_else(|| "127.0.0.1:7000".to_string());
            let source_label =
                parse_flag(&args[1..], "--source")?.unwrap_or_else(|| "mock-window".to_string());
            let timeout_ms = parse_flag(&args[1..], "--timeout-ms")?
                .map(|value| value.parse())
                .transpose()?
                .unwrap_or(10_000);
            let push_debug_capture = has_flag(&args[1..], "--push-debug-capture");

            Ok(AppCommand::WebRtcHost(WebRtcHostConfig {
                room,
                signaling_addr,
                source_label,
                timeout_ms,
                push_debug_capture,
            }))
        }
        "webrtc-viewer" => {
            let room = required_flag(&args[1..], "--room")?;
            let signaling_addr =
                parse_flag(&args[1..], "--signal")?.unwrap_or_else(|| "127.0.0.1:7000".to_string());
            let timeout_ms = parse_flag(&args[1..], "--timeout-ms")?
                .map(|value| value.parse())
                .transpose()?
                .unwrap_or(10_000);

            Ok(AppCommand::WebRtcViewer(WebRtcViewerConfig {
                room,
                signaling_addr,
                timeout_ms,
            }))
        }
        other => Err(Box::new(CliError(format!("unknown command: {other}")))),
    }
}

fn parse_flag(args: &[String], flag: &str) -> Result<Option<String>, Box<dyn Error>> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == flag {
            let value = args
                .get(index + 1)
                .ok_or_else(|| CliError(format!("missing value for {flag}")))?;
            return Ok(Some(value.clone()));
        }
        index += 1;
    }
    Ok(None)
}

fn required_flag(args: &[String], flag: &str) -> Result<String, Box<dyn Error>> {
    parse_flag(args, flag)?.ok_or_else(|| {
        Box::new(CliError(format!("required flag missing: {flag}"))) as Box<dyn Error>
    })
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|value| value == flag)
}

pub fn print_help() {
    println!(
        "\
video-p2p-mvp

Commands:
  help
  spec
  sender --room demo --signal 127.0.0.1:7000 [--udp-bind 0.0.0.0:0] [--fps 10] [--frames 120] [--source mock-window]
  receiver --room demo --signal 127.0.0.1:7000 [--udp-bind 0.0.0.0:0] [--expected-frames 120]
  webrtc-host --room demo --signal 127.0.0.1:7000 [--source mock-window] [--timeout-ms 10000] [--push-debug-capture]
  webrtc-viewer --room demo --signal 127.0.0.1:7000 [--timeout-ms 10000]

This MVP uses TCP for signaling and direct UDP for mock media packets.
It now also has a live WebRTC negotiation path that uses the same signaling server,
with attached host audio/video tracks and optional debug `capture-core` payload publishing."
    );
}

#[cfg(test)]
mod tests {
    use super::{AppCommand, parse_cli_args};

    #[test]
    fn webrtc_host_flag_enables_debug_capture() {
        let args = vec![
            "webrtc-host".to_string(),
            "--room".to_string(),
            "demo".to_string(),
            "--push-debug-capture".to_string(),
        ];

        let command = parse_cli_args(&args).expect("parse host command");
        let AppCommand::WebRtcHost(config) = command else {
            panic!("expected host command");
        };

        assert!(config.push_debug_capture);
    }

    #[test]
    fn webrtc_host_defaults_debug_capture_to_disabled() {
        let args = vec![
            "webrtc-host".to_string(),
            "--room".to_string(),
            "demo".to_string(),
        ];

        let command = parse_cli_args(&args).expect("parse host command");
        let AppCommand::WebRtcHost(config) = command else {
            panic!("expected host command");
        };

        assert!(!config.push_debug_capture);
    }
}
