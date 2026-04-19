#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use app_core::{SessionIntent, SessionManager, SessionMode, SessionSnapshot, SessionStage, SessionTransport};
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
    room: Option<String>,
    signaling_addr: Option<String>,
    source_label: Option<String>,
    active_peer: Option<String>,
    logs: Vec<String>,
}

#[derive(Serialize)]
struct CommandResult {
    ok: bool,
    message: String,
    session: SessionView,
}

#[tauri::command]
fn project_status(state: tauri::State<'_, Mutex<SessionManager>>) -> ProjectStatus {
    let snapshot = state.lock().expect("session state poisoned").snapshot();
    ProjectStatus {
        stage: format_session_stage(snapshot.stage).to_string(),
        gui: "tauri shell wired",
        transport: format_session_transport(snapshot.transport).to_string(),
        capture_macos: "planned via ScreenCaptureKit bridge",
        capture_linux: "planned via Portal + PipeWire",
    }
}

#[tauri::command]
fn session_snapshot(state: tauri::State<'_, Mutex<SessionManager>>) -> SessionView {
    let snapshot = state.lock().expect("session state poisoned").snapshot();
    map_snapshot(snapshot)
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

    CommandResult {
        ok: true,
        message: format!("host session prepared for room={room}"),
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
fn specification_markdown() -> String {
    include_str!("../../../../docs/SPECIFICATION.md").to_string()
}

fn main() {
    tauri::Builder::default()
        .manage(Mutex::new(SessionManager::new()))
        .invoke_handler(tauri::generate_handler![
            project_status,
            session_snapshot,
            start_host,
            join_room,
            mark_mock_streaming,
            mark_webrtc_planned,
            stop_session,
            session_logs,
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
        room: snapshot.room,
        signaling_addr: snapshot.signaling_addr,
        source_label: snapshot.source_label,
        active_peer: snapshot.active_peer,
        logs: snapshot.logs,
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
        SessionStage::PlannedWebRtc => "planned_webrtc",
        SessionStage::Stopped => "stopped",
    }
}

fn format_session_transport(transport: SessionTransport) -> &'static str {
    match transport {
        SessionTransport::MockUdp => "mock_udp",
        SessionTransport::PlannedWebRtc => "planned_webrtc",
    }
}
