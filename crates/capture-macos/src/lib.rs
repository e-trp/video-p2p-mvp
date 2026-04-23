use capture_core::{CapturePermissionState, CaptureSource, CaptureSourceKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacCaptureStage {
    Planned,
    PermissionsRequired,
    SourceSelection,
    Capturing,
}

#[derive(Debug, Clone)]
pub struct MacCaptureBlueprint {
    pub stage: MacCaptureStage,
    pub permission_state: CapturePermissionState,
    pub example_sources: Vec<CaptureSource>,
    pub sources_api: &'static str,
    pub media_bridge: &'static str,
    pub notes: Vec<&'static str>,
}

pub fn blueprint() -> MacCaptureBlueprint {
    MacCaptureBlueprint {
        stage: MacCaptureStage::Planned,
        permission_state: CapturePermissionState::Required,
        example_sources: vec![
            CaptureSource {
                id: "mac-window-vlc".to_string(),
                kind: CaptureSourceKind::Window,
                display_name: "VLC Player".to_string(),
                app_name: Some("VLC".to_string()),
                has_audio: true,
            },
            CaptureSource {
                id: "mac-window-browser".to_string(),
                kind: CaptureSourceKind::Window,
                display_name: "Safari".to_string(),
                app_name: Some("Safari".to_string()),
                has_audio: true,
            },
        ],
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

#[cfg(test)]
mod tests {
    use super::{MacCaptureStage, blueprint};
    use capture_core::CapturePermissionState;

    #[test]
    fn blueprint_exposes_permission_state_and_example_sources() {
        let blueprint = blueprint();
        assert_eq!(blueprint.stage, MacCaptureStage::Planned);
        assert_eq!(blueprint.permission_state, CapturePermissionState::Required);
        assert_eq!(blueprint.example_sources.len(), 2);
    }
}
