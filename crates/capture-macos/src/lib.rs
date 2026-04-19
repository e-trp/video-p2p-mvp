#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacCaptureStage {
    Planned,
    PermissionsRequired,
    SourceSelection,
    Capturing,
}

#[derive(Debug, Clone)]
pub struct MacCaptureSource {
    pub id: String,
    pub app_name: String,
    pub window_title: String,
}

#[derive(Debug, Clone)]
pub struct MacCaptureBlueprint {
    pub stage: MacCaptureStage,
    pub sources_api: &'static str,
    pub media_bridge: &'static str,
    pub notes: Vec<&'static str>,
}

pub fn blueprint() -> MacCaptureBlueprint {
    MacCaptureBlueprint {
        stage: MacCaptureStage::Planned,
        sources_api: "ScreenCaptureKit",
        media_bridge: "Swift/Objective-C bridge feeding Rust-owned transport",
        notes: vec![
            "enumerate windows through SCShareableContent",
            "request Screen Recording permission on first capture",
            "stream screen frames and audio buffers into Rust",
            "surface selected sources to the Tauri GUI",
        ],
    }
}
