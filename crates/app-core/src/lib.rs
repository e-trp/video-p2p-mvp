pub mod app;
pub mod capture_catalog;
pub mod ice_servers;
pub mod mock_media;
mod preferences;
pub mod protocol;
pub mod session;
pub mod session_flow;
pub mod signaling;

pub use app::{AppCommand, CliError, parse_cli_args, print_help};
pub use capture_catalog::{
    CaptureCatalogSnapshot, current_capture_catalog, describe_permission_state,
};
pub use ice_servers::{
    IceServerEntry, format_ice_server_entries, parse_ice_server_entries,
    summarize_ice_server_entries,
};
pub use mock_media::{ReceiverConfig, SenderConfig, run_receiver, run_sender};
pub use protocol::{
    IceCandidate, JoinRequest, MediaPacket, PeerAnnouncement, ProtocolError, Role, SdpType,
    SessionDescription, SignalingMessage, decode_media_packet, decode_signaling_message,
    encode_error, encode_media_packet, encode_peer, encode_signaling_message, encode_waiting,
    parse_join_request, parse_peer_message,
};
pub use session::{
    SessionIntent, SessionManager, SessionMode, SessionSnapshot, SessionStage, SessionTransport,
};
pub use session_flow::{WebRtcHostConfig, WebRtcViewerConfig, run_webrtc_host, run_webrtc_viewer};
pub use signaling::{SignalingConnection, SignalingError, SignalingEvent};
