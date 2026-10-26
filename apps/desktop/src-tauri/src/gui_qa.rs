//! Opt-in DOM automation inside the real Tauri WebView, with production IPC and player code.
//! This plugin is absent from default builds and requires an isolated configuration directory.

use serde::Serialize;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Manager, State, WebviewWindow, plugin::TauriPlugin, webview::PageLoadEvent};

#[derive(Clone, Serialize)]
pub(crate) struct Configuration {
    scenario: String,
    signaling_addr: String,
    native_source_id: Option<String>,
    native_alternate_source_id: Option<String>,
    native_audio: bool,
}

pub(crate) struct QaState {
    configuration: Configuration,
    report_path: PathBuf,
    stop_file: Option<PathBuf>,
    session_held: Arc<AtomicBool>,
}

#[derive(Serialize)]
pub(crate) struct WindowState {
    focused: bool,
    visible: bool,
}

#[tauri::command]
pub(crate) fn gui_qa_window_state(
    window: WebviewWindow,
    _state: State<'_, QaState>,
) -> Result<WindowState, String> {
    Ok(WindowState {
        focused: window.is_focused().map_err(|error| error.to_string())?,
        visible: window.is_visible().map_err(|error| error.to_string())?,
    })
}

#[tauri::command]
pub(crate) fn gui_qa_configuration(state: State<'_, QaState>) -> Configuration {
    state.configuration.clone()
}

#[tauri::command]
pub(crate) fn gui_qa_hold_session(app: tauri::AppHandle, state: State<'_, QaState>) {
    let held = Arc::clone(&state.session_held);
    tauri::async_runtime::spawn_blocking(move || {
        let session = app.state::<Mutex<app_core::SessionManager>>();
        let _guard = session.lock().expect("QA session state poisoned");
        held.store(true, Ordering::SeqCst);
        // Deliberate, bounded contention lets the WebView verify that native window
        // commands still execute while a production session command waits here.
        std::thread::sleep(std::time::Duration::from_secs(2));
        held.store(false, Ordering::SeqCst);
    });
}

#[tauri::command]
pub(crate) fn gui_qa_session_held(state: State<'_, QaState>) -> bool {
    state.session_held.load(Ordering::SeqCst)
}

#[tauri::command]
pub(crate) fn gui_qa_focus(
    window: WebviewWindow,
    _state: State<'_, QaState>,
) -> Result<(), String> {
    window.set_focus().map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn gui_qa_stop_requested(state: State<'_, QaState>) -> Result<bool, String> {
    state
        .stop_file
        .as_ref()
        .ok_or_else(|| "Native GUI QA requires a private stop file".to_owned())?
        .try_exists()
        .map_err(|error| format!("cannot inspect GUI QA stop file: {error}"))
}

#[tauri::command]
pub(crate) fn gui_qa_checkpoint(
    name: String,
    publish: bool,
    state: State<'_, QaState>,
) -> Result<bool, String> {
    if !matches!(
        name.as_str(),
        "initial-viewer"
            | "alternate-host"
            | "alternate-viewer"
            | "restored-host"
            | "restored-viewer"
            | "source-closed"
            | "closure-host"
    ) {
        return Err("Unknown native GUI QA checkpoint".into());
    }
    let path = state
        .stop_file
        .as_ref()
        .ok_or("Native GUI QA requires a private stop file")?
        .with_file_name(name);
    if publish {
        std::fs::write(&path, [])
            .map_err(|error| format!("cannot publish GUI QA checkpoint: {error}"))?;
    }
    path.try_exists()
        .map_err(|error| format!("cannot inspect GUI QA checkpoint: {error}"))
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
                    native_source_id: std::env::var("VIDEO_P2P_GUI_QA_NATIVE_SOURCE").ok(),
                    native_alternate_source_id: std::env::var("VIDEO_P2P_GUI_QA_ALTERNATE_SOURCE")
                        .ok(),
                    native_audio: std::env::var("VIDEO_P2P_GUI_QA_NATIVE_AUDIO")
                        .is_ok_and(|value| value == "true"),
                },
                report_path: std::env::var_os("VIDEO_P2P_GUI_QA_REPORT")
                    .ok_or("GUI QA requires VIDEO_P2P_GUI_QA_REPORT")?
                    .into(),
                stop_file: std::env::var_os("VIDEO_P2P_GUI_QA_STOP_FILE").map(PathBuf::from),
                session_held: Arc::new(AtomicBool::new(false)),
            });
            Ok(())
        })
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Finished
                && let Err(error) = webview
                    .window()
                    .set_focus()
                    .and_then(|()| webview.eval(include_str!("../../tests/gui-scenarios.js")))
            {
                eprintln!("cannot start GUI QA scenarios: {error}");
                webview.app_handle().exit(1);
            }
        })
        .build()
}
