use capture_core::{CapturePermissionState, CaptureSource, CaptureSourceKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxCaptureBackend {
    PortalPipeWire,
    X11Fallback,
}

#[derive(Debug, Clone)]
pub struct LinuxCaptureBlueprint {
    pub preferred_backend: LinuxCaptureBackend,
    pub permission_state: CapturePermissionState,
    pub example_sources: Vec<CaptureSource>,
    pub notes: Vec<&'static str>,
}

pub fn blueprint() -> LinuxCaptureBlueprint {
    LinuxCaptureBlueprint {
        preferred_backend: LinuxCaptureBackend::PortalPipeWire,
        permission_state: CapturePermissionState::Required,
        example_sources: vec![
            CaptureSource {
                id: "linux-window-player".to_string(),
                kind: CaptureSourceKind::Window,
                display_name: "Video Player".to_string(),
                app_name: Some("mpv".to_string()),
                has_audio: true,
            },
            CaptureSource {
                id: "linux-display-1".to_string(),
                kind: CaptureSourceKind::Display,
                display_name: "Display 1".to_string(),
                app_name: None,
                has_audio: false,
            },
        ],
        notes: vec![
            "use XDG Desktop Portal ScreenCast for Wayland source selection",
            "consume resulting PipeWire stream for video and available audio",
            "fallback to X11 capture only where Wayland path is unavailable",
            "keep GUI flow aligned with portal-driven user permission model",
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::{LinuxCaptureBackend, blueprint};
    use capture_core::CapturePermissionState;

    #[test]
    fn blueprint_exposes_required_permission_and_example_sources() {
        let blueprint = blueprint();
        assert_eq!(blueprint.preferred_backend, LinuxCaptureBackend::PortalPipeWire);
        assert_eq!(blueprint.permission_state, CapturePermissionState::Required);
        assert_eq!(blueprint.example_sources.len(), 2);
    }
}
