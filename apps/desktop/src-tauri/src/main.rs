#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use app_core::{
    CaptureCatalogSnapshot, SessionIntent, SessionManager, SessionMode, SessionSnapshot,
    SessionStage, SessionTransport,
};
use serde::Serialize;
use std::sync::Mutex;

#[derive(Serialize)]
struct ProjectStatus {
    stage: String,
    gui: &'static str,
    transport: String,
    capture_macos: &'static str,
    capture_linux: &'static str,
}

#[derive(Serialize)]
struct SessionView {
    mode: String,
    stage: String,
    transport: String,
    transport_state: String,
    transport_stage: Option<String>,
    signaling_connected: bool,
    room: Option<String>,
    signaling_addr: Option<String>,
    source_label: Option<String>,
    selected_source_id: Option<String>,
    selected_source_audio: bool,
    capture_backend: String,
    capture_permission_state: String,
    available_source_count: usize,
    active_peer: Option<String>,
    next_action: String,
    local_description_ready: bool,
    local_description_kind: Option<String>,
    remote_description_ready: bool,
    remote_description_kind: Option<String>,
    local_media_track_count: usize,
    local_video_track_attached: bool,
    local_audio_track_attached: bool,
    published_video_sample_count: usize,
    published_audio_sample_count: usize,
    last_video_sample_bytes: usize,
    last_audio_sample_bytes: usize,
    local_data_channel_ready: bool,
    transport_stats_report_count: usize,
    transport_notes: Vec<String>,
    local_offer_ready: bool,
    remote_answer_ready: bool,
    local_candidate_count: usize,
    remote_candidate_count: usize,
    last_signaling_message: Option<String>,
    logs: Vec<String>,
}

#[derive(Serialize)]
struct CommandResult {
    ok: bool,
    message: String,
    session: SessionView,
}

#[derive(Serialize)]
struct CaptureSourceView {
    id: String,
    kind: String,
    label: String,
    app_name: Option<String>,
    has_audio: bool,
}

#[derive(Serialize)]
struct CaptureCatalogView {
    backend: String,
    permission_state: String,
    selected_source_id: Option<String>,
    selected_source_audio: bool,
    sources: Vec<CaptureSourceView>,
}

#[tauri::command]
fn project_status(state: tauri::State<'_, Mutex<SessionManager>>) -> ProjectStatus {
    let snapshot = state.lock().expect("session state poisoned").snapshot();
    ProjectStatus {
        stage: format_session_stage(snapshot.stage).to_string(),
        gui: "tauri shell wired",
        transport: format_session_transport(snapshot.transport).to_string(),
        capture_macos: "capture-core contracts ready; ScreenCaptureKit bridge still pending",
        capture_linux: "capture-core contracts ready; Portal + PipeWire bridge still pending",
    }
}

#[tauri::command]
fn session_snapshot(state: tauri::State<'_, Mutex<SessionManager>>) -> SessionView {
    let snapshot = state.lock().expect("session state poisoned").refresh();
    map_snapshot(snapshot)
}

#[tauri::command]
fn capture_catalog(state: tauri::State<'_, Mutex<SessionManager>>) -> CaptureCatalogView {
    let state = state.lock().expect("session state poisoned");
    let catalog = state.capture_catalog();
    let snapshot = state.snapshot();
    map_capture_catalog(catalog, snapshot)
}

#[tauri::command]
fn start_host(
    room: String,
    signaling_addr: String,
    source_label: Option<String>,
    state: tauri::State<'_, Mutex<SessionManager>>,
) -> CommandResult {
    let snapshot = state
        .lock()
        .expect("session state poisoned")
        .start_host(SessionIntent {
            room: room.clone(),
            signaling_addr: signaling_addr.clone(),
            source_label,
        });
    let message = if snapshot.local_offer_ready {
        format!("host session prepared for room={room}; local offer sent automatically")
    } else {
        format!("host session prepared for room={room}")
    };

    CommandResult {
        ok: true,
        message,
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn join_room(
    room: String,
    signaling_addr: String,
    state: tauri::State<'_, Mutex<SessionManager>>,
) -> CommandResult {
    let snapshot = state
        .lock()
        .expect("session state poisoned")
        .start_viewer(SessionIntent {
            room: room.clone(),
            signaling_addr: signaling_addr.clone(),
            source_label: None,
        });

    CommandResult {
        ok: true,
        message: format!("viewer session prepared for room={room}"),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn update_session_config(
    room: Option<String>,
    signaling_addr: Option<String>,
    source_label: Option<String>,
    state: tauri::State<'_, Mutex<SessionManager>>,
) -> CommandResult {
    let snapshot = state.lock().expect("session state poisoned").update_config(
        room,
        signaling_addr,
        source_label,
    );

    CommandResult {
        ok: true,
        message: "session configuration updated".to_string(),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn mark_mock_streaming(
    peer: String,
    state: tauri::State<'_, Mutex<SessionManager>>,
) -> CommandResult {
    let snapshot = state
        .lock()
        .expect("session state poisoned")
        .mark_mock_streaming(peer.clone());

    CommandResult {
        ok: true,
        message: format!("mock streaming activated with peer={peer}"),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn mark_webrtc_planned(state: tauri::State<'_, Mutex<SessionManager>>) -> CommandResult {
    let snapshot = state
        .lock()
        .expect("session state poisoned")
        .mark_webrtc_ready();

    CommandResult {
        ok: true,
        message: "session moved to planned WebRTC state".to_string(),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn create_local_offer(state: tauri::State<'_, Mutex<SessionManager>>) -> CommandResult {
    let snapshot = state
        .lock()
        .expect("session state poisoned")
        .create_local_offer();

    CommandResult {
        ok: true,
        message: "local SDP offer created for debugging".to_string(),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn select_capture_source(
    source_id: String,
    include_audio: bool,
    state: tauri::State<'_, Mutex<SessionManager>>,
) -> CommandResult {
    let snapshot = state
        .lock()
        .expect("session state poisoned")
        .select_capture_source(source_id.clone(), include_audio);

    CommandResult {
        ok: true,
        message: format!("capture source selected: {source_id}"),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn publish_placeholder_media(state: tauri::State<'_, Mutex<SessionManager>>) -> CommandResult {
    let snapshot = state
        .lock()
        .expect("session state poisoned")
        .publish_placeholder_media();

    CommandResult {
        ok: true,
        message: "placeholder media samples published".to_string(),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn accept_remote_answer(
    sdp: String,
    state: tauri::State<'_, Mutex<SessionManager>>,
) -> CommandResult {
    let snapshot = state
        .lock()
        .expect("session state poisoned")
        .accept_remote_answer(sdp);

    CommandResult {
        ok: true,
        message: "remote SDP answer accepted".to_string(),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn add_remote_ice_candidate(
    candidate: String,
    sdp_mid: Option<String>,
    sdp_mline_index: Option<u16>,
    state: tauri::State<'_, Mutex<SessionManager>>,
) -> CommandResult {
    let snapshot = state
        .lock()
        .expect("session state poisoned")
        .add_remote_ice_candidate(candidate, sdp_mid, sdp_mline_index);

    CommandResult {
        ok: true,
        message: "remote ICE candidate added".to_string(),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn stop_session(state: tauri::State<'_, Mutex<SessionManager>>) -> CommandResult {
    let snapshot = state.lock().expect("session state poisoned").stop();

    CommandResult {
        ok: true,
        message: "session stopped".to_string(),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn session_logs(state: tauri::State<'_, Mutex<SessionManager>>) -> Vec<String> {
    state.lock().expect("session state poisoned").logs()
}

#[tauri::command]
fn clear_session_logs(state: tauri::State<'_, Mutex<SessionManager>>) -> CommandResult {
    let snapshot = state.lock().expect("session state poisoned").clear_logs();

    CommandResult {
        ok: true,
        message: "session logs cleared".to_string(),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn reset_session(state: tauri::State<'_, Mutex<SessionManager>>) -> CommandResult {
    let snapshot = state.lock().expect("session state poisoned").reset();

    CommandResult {
        ok: true,
        message: "session reset to idle state".to_string(),
        session: map_snapshot(snapshot),
    }
}

#[tauri::command]
fn specification_markdown() -> String {
    include_str!("../../../../docs/SPECIFICATION.md").to_string()
}

fn main() {
    tauri::Builder::default()
        .manage(Mutex::new(SessionManager::new()))
        .invoke_handler(tauri::generate_handler![
            project_status,
            session_snapshot,
            capture_catalog,
            start_host,
            join_room,
            update_session_config,
            mark_mock_streaming,
            mark_webrtc_planned,
            create_local_offer,
            select_capture_source,
            publish_placeholder_media,
            accept_remote_answer,
            add_remote_ice_candidate,
            stop_session,
            session_logs,
            clear_session_logs,
            reset_session,
            specification_markdown
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}

fn map_snapshot(snapshot: SessionSnapshot) -> SessionView {
    SessionView {
        mode: format_session_mode(snapshot.mode).to_string(),
        stage: format_session_stage(snapshot.stage).to_string(),
        transport: format_session_transport(snapshot.transport).to_string(),
        transport_state: snapshot.transport_state,
        transport_stage: snapshot.transport_stage,
        signaling_connected: snapshot.signaling_connected,
        room: snapshot.room,
        signaling_addr: snapshot.signaling_addr,
        source_label: snapshot.source_label,
        selected_source_id: snapshot.selected_source_id,
        selected_source_audio: snapshot.selected_source_audio,
        capture_backend: snapshot.capture_backend,
        capture_permission_state: snapshot.capture_permission_state,
        available_source_count: snapshot.available_source_count,
        active_peer: snapshot.active_peer,
        next_action: snapshot.next_action,
        local_description_ready: snapshot.local_description_ready,
        local_description_kind: snapshot.local_description_kind,
        remote_description_ready: snapshot.remote_description_ready,
        remote_description_kind: snapshot.remote_description_kind,
        local_media_track_count: snapshot.local_media_track_count,
        local_video_track_attached: snapshot.local_video_track_attached,
        local_audio_track_attached: snapshot.local_audio_track_attached,
        published_video_sample_count: snapshot.published_video_sample_count,
        published_audio_sample_count: snapshot.published_audio_sample_count,
        last_video_sample_bytes: snapshot.last_video_sample_bytes,
        last_audio_sample_bytes: snapshot.last_audio_sample_bytes,
        local_data_channel_ready: snapshot.local_data_channel_ready,
        transport_stats_report_count: snapshot.transport_stats_report_count,
        transport_notes: snapshot.transport_notes,
        local_offer_ready: snapshot.local_offer_ready,
        remote_answer_ready: snapshot.remote_answer_ready,
        local_candidate_count: snapshot.local_candidate_count,
        remote_candidate_count: snapshot.remote_candidate_count,
        last_signaling_message: snapshot.last_signaling_message,
        logs: snapshot.logs,
    }
}

fn map_capture_catalog(
    catalog: CaptureCatalogSnapshot,
    snapshot: SessionSnapshot,
) -> CaptureCatalogView {
    CaptureCatalogView {
        backend: catalog.backend,
        permission_state: snapshot.capture_permission_state,
        selected_source_id: snapshot.selected_source_id,
        selected_source_audio: snapshot.selected_source_audio,
        sources: catalog
            .sources
            .into_iter()
            .map(|source| CaptureSourceView {
                id: source.id.clone(),
                kind: format!("{:?}", source.kind).to_lowercase(),
                label: source.label(),
                app_name: source.app_name,
                has_audio: source.has_audio,
            })
            .collect(),
    }
}

fn format_session_mode(mode: SessionMode) -> &'static str {
    match mode {
        SessionMode::Idle => "idle",
        SessionMode::Host => "host",
        SessionMode::Viewer => "viewer",
    }
}

fn format_session_stage(stage: SessionStage) -> &'static str {
    match stage {
        SessionStage::Idle => "idle",
        SessionStage::Configured => "configured",
        SessionStage::AwaitingPeer => "awaiting_peer",
        SessionStage::MockStreaming => "mock_streaming",
        SessionStage::NegotiatingWebRtc => "negotiating_webrtc",
        SessionStage::LiveWebRtc => "live_webrtc",
        SessionStage::Stopped => "stopped",
    }
}

fn format_session_transport(transport: SessionTransport) -> &'static str {
    match transport {
        SessionTransport::MockUdp => "mock_udp",
        SessionTransport::LiveWebRtc => "live_webrtc",
    }
}
