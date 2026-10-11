//! Opt-in DOM automation inside the real Tauri WebView, with production IPC and player code.
//! This plugin is absent from default builds and requires an isolated configuration directory.

use serde::Serialize;
use std::path::PathBuf;
use tauri::{Manager, State, WebviewWindow, plugin::TauriPlugin, webview::PageLoadEvent};

#[derive(Clone, Serialize)]
pub(crate) struct Configuration {
    scenario: String,
    signaling_addr: String,
}

pub(crate) struct QaState {
    configuration: Configuration,
    report_path: PathBuf,
}

#[tauri::command]
pub(crate) fn gui_qa_configuration(state: State<'_, QaState>) -> Configuration {
    state.configuration.clone()
}

#[tauri::command]
pub(crate) fn gui_qa_report(payload: String, state: State<'_, QaState>) -> Result<(), String> {
    std::fs::write(&state.report_path, payload)
        .map_err(|error| format!("cannot save GUI QA evidence: {error}"))
}

#[tauri::command]
pub(crate) fn gui_qa_resize(
    width: u32,
    height: u32,
    window: WebviewWindow,
    _state: State<'_, QaState>,
) -> Result<(), String> {
    if !(320..=1600).contains(&width) || !(400..=1200).contains(&height) {
        return Err("GUI QA size is outside the supported test range".into());
    }
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|error| format!("cannot resize GUI QA window: {error}"))
}

#[tauri::command]
pub(crate) fn gui_qa_finish(success: bool, app: tauri::AppHandle, _state: State<'_, QaState>) {
    app.exit(if success { 0 } else { 1 });
}

pub(crate) fn plugin() -> TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("gui-qa")
        .setup(|app, _| {
            let config_dir = std::env::var_os("VIDEO_P2P_MVP_CONFIG_DIR")
                .ok_or("GUI QA requires an isolated VIDEO_P2P_MVP_CONFIG_DIR")?;
            std::fs::create_dir_all(config_dir)?;
            app.manage(QaState {
                configuration: Configuration {
                    scenario: std::env::var("VIDEO_P2P_GUI_QA_SCENARIO")?,
                    signaling_addr: std::env::var("VIDEO_P2P_GUI_QA_SIGNALING")?,
                },
                report_path: std::env::var_os("VIDEO_P2P_GUI_QA_REPORT")
                    .ok_or("GUI QA requires VIDEO_P2P_GUI_QA_REPORT")?
                    .into(),
            });
            Ok(())
        })
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Finished
                && let Err(error) = webview.eval(include_str!("../../tests/gui-scenarios.js"))
            {
                eprintln!("cannot start GUI QA scenarios: {error}");
                webview.app_handle().exit(1);
            }
        })
        .build()
}
