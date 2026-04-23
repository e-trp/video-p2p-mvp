use capture_core::{CapturePermissionState, CaptureSelection, CaptureSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureCatalogSnapshot {
    pub backend: String,
    pub permission_state: CapturePermissionState,
    pub sources: Vec<CaptureSource>,
}

pub fn current_capture_catalog() -> CaptureCatalogSnapshot {
    #[cfg(target_os = "macos")]
    {
        let blueprint = capture_macos::blueprint();
        return CaptureCatalogSnapshot {
            backend: blueprint.sources_api.to_string(),
            permission_state: blueprint.permission_state,
            sources: blueprint.example_sources,
        };
    }

    #[cfg(target_os = "linux")]
    {
        let blueprint = capture_linux::blueprint();
        return CaptureCatalogSnapshot {
            backend: format!("{:?}", blueprint.preferred_backend),
            permission_state: blueprint.permission_state,
            sources: blueprint.example_sources,
        };
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        CaptureCatalogSnapshot {
            backend: "unsupported_platform".to_string(),
            permission_state: CapturePermissionState::Unknown,
            sources: Vec::new(),
        }
    }
}

pub fn describe_permission_state(state: CapturePermissionState) -> &'static str {
    match state {
        CapturePermissionState::Unknown => "unknown",
        CapturePermissionState::Required => "required",
        CapturePermissionState::Granted => "granted",
        CapturePermissionState::Denied => "denied",
    }
}

pub fn selected_source_label(
    catalog: &CaptureCatalogSnapshot,
    selection: Option<&CaptureSelection>,
) -> Option<String> {
    let selection = selection?;
    catalog
        .sources
        .iter()
        .find(|source| source.id == selection.source_id)
        .map(CaptureSource::label)
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureCatalogSnapshot, current_capture_catalog, describe_permission_state,
        selected_source_label,
    };
    use capture_core::{
        CapturePermissionState, CaptureSelection, CaptureSource, CaptureSourceKind,
    };

    #[test]
    fn selected_source_label_uses_catalog_source_label() {
        let catalog = CaptureCatalogSnapshot {
            backend: "test".to_string(),
            permission_state: CapturePermissionState::Granted,
            sources: vec![CaptureSource {
                id: "source-1".to_string(),
                kind: CaptureSourceKind::Window,
                display_name: "Player".to_string(),
                app_name: Some("mpv".to_string()),
                has_audio: true,
            }],
        };

        let label = selected_source_label(
            &catalog,
            Some(&CaptureSelection {
                source_id: "source-1".to_string(),
                include_audio: true,
            }),
        );

        assert_eq!(label.as_deref(), Some("mpv - Player"));
    }

    #[test]
    fn permission_state_description_is_stable() {
        assert_eq!(describe_permission_state(CapturePermissionState::Required), "required");
    }

    #[test]
    fn current_platform_catalog_has_backend_name() {
        let catalog = current_capture_catalog();
        assert!(!catalog.backend.is_empty());
    }
}
