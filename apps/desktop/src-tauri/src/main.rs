#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;

#[derive(Serialize)]
struct ProjectStatus {
    stage: &'static str,
    gui: &'static str,
    transport: &'static str,
    capture_macos: &'static str,
    capture_linux: &'static str,
}

#[tauri::command]
fn project_status() -> ProjectStatus {
    ProjectStatus {
        stage: "scaffold",
        gui: "tauri shell wired",
        transport: "mock udp MVP, webrtc planned",
        capture_macos: "planned via ScreenCaptureKit bridge",
        capture_linux: "planned via Portal + PipeWire",
    }
}

#[tauri::command]
fn specification_markdown() -> String {
    include_str!("../../../../docs/SPECIFICATION.md").to_string()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![project_status, specification_markdown])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
