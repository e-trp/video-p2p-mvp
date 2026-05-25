use capture_core::{CapturePermissionState, CaptureSource, CaptureSourceKind};
use std::collections::HashSet;
use std::process::Command;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacSourceCatalogOrigin {
    Runtime,
    BlueprintFallback,
}

#[derive(Debug, Clone)]
pub struct MacCaptureCatalog {
    pub backend_label: String,
    pub permission_state: CapturePermissionState,
    pub sources: Vec<CaptureSource>,
    pub origin: MacSourceCatalogOrigin,
    pub notes: Vec<String>,
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

pub fn current_catalog() -> MacCaptureCatalog {
    let blueprint = blueprint();

    match enumerate_runtime_sources() {
        Ok(sources) if !sources.is_empty() => MacCaptureCatalog {
            backend_label: format!("{} runtime catalog", blueprint.sources_api),
            permission_state: CapturePermissionState::Granted,
            notes: vec![format!(
                "runtime catalog enumerated {} sources through osascript/System Events",
                sources.len()
            )],
            sources,
            origin: MacSourceCatalogOrigin::Runtime,
        },
        Err(error) => MacCaptureCatalog {
            backend_label: format!("{} blueprint fallback", blueprint.sources_api),
            permission_state: infer_permission_state_from_error(&error),
            notes: vec![format!("runtime catalog fallback: {error}")],
            sources: blueprint.example_sources,
            origin: MacSourceCatalogOrigin::BlueprintFallback,
        },
        Ok(_) => MacCaptureCatalog {
            backend_label: format!("{} blueprint fallback", blueprint.sources_api),
            permission_state: CapturePermissionState::Required,
            notes: vec!["runtime catalog fallback: runtime source list was empty".to_string()],
            sources: blueprint.example_sources,
            origin: MacSourceCatalogOrigin::BlueprintFallback,
        },
    }
}

fn infer_permission_state_from_error(error: &str) -> CapturePermissionState {
    let lower = error.to_ascii_lowercase();

    if lower.contains("-1743")
        || lower.contains("-1744")
        || lower.contains("not authorized")
        || lower.contains("not permitted")
    {
        CapturePermissionState::Denied
    } else if lower.contains("-10827")
        || lower.contains("failed to launch osascript")
        || lower.contains("application isn’t running")
    {
        CapturePermissionState::Unknown
    } else {
        CapturePermissionState::Required
    }
}

fn enumerate_runtime_sources() -> Result<Vec<CaptureSource>, String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(runtime_catalog_script())
        .output()
        .map_err(|error| format!("failed to launch osascript: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("osascript returned {}: {stderr}", output.status));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let sources = parse_runtime_listing(&stdout);
    if sources.is_empty() {
        return Err("runtime catalog output was empty".to_string());
    }

    Ok(sources)
}

fn runtime_catalog_script() -> &'static str {
    r#"
set outputLines to {}
tell application "System Events"
	repeat with proc in (application processes whose background only is false)
		set appName to (name of proc) as text
		try
			set windowNames to name of every window of proc
			if (count of windowNames) is 0 then
				set end of outputLines to "application" & tab & appName & tab & appName
			else
				repeat with windowName in windowNames
					set windowNameText to (windowName as text)
					if windowNameText is "" then
						set windowNameText to appName
					end if
					set end of outputLines to "window" & tab & appName & tab & windowNameText
				end repeat
			end if
		on error
			set end of outputLines to "application" & tab & appName & tab & appName
		end try
	end repeat
end tell
set AppleScript's text item delimiters to linefeed
set outputText to outputLines as text
set AppleScript's text item delimiters to ""
return outputText
"#
}

fn parse_runtime_listing(raw: &str) -> Vec<CaptureSource> {
    let mut seen_ids = HashSet::new();
    let mut sources = Vec::new();

    for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let mut fields = line.split('\t');
        let kind = fields.next().unwrap_or_default();
        let app_name = fields.next().unwrap_or_default().trim();
        let display_name = fields.next().unwrap_or_default().trim();
        if app_name.is_empty() || display_name.is_empty() {
            continue;
        }

        let kind = match kind {
            "window" => CaptureSourceKind::Window,
            "application" => CaptureSourceKind::Application,
            _ => continue,
        };

        let source = CaptureSource {
            id: make_source_id(kind, app_name, display_name),
            kind,
            display_name: display_name.to_string(),
            app_name: Some(app_name.to_string()),
            has_audio: true,
        };

        if seen_ids.insert(source.id.clone()) {
            sources.push(source);
        }
    }

    sources
}

fn make_source_id(kind: CaptureSourceKind, app_name: &str, display_name: &str) -> String {
    let kind_label = match kind {
        CaptureSourceKind::Window => "window",
        CaptureSourceKind::Application => "application",
        CaptureSourceKind::Display => "display",
    };
    let app_slug = slugify(app_name);
    let display_slug = slugify(display_name);
    format!("macos-{kind_label}-{app_slug}-{display_slug}")
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for character in input.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "unnamed".to_string()
    } else {
        slug.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MacCaptureStage, blueprint, infer_permission_state_from_error, parse_runtime_listing,
        runtime_catalog_script, slugify,
    };
    use capture_core::CapturePermissionState;

    #[test]
    fn blueprint_exposes_permission_state_and_example_sources() {
        let blueprint = blueprint();
        assert_eq!(blueprint.stage, MacCaptureStage::Planned);
        assert_eq!(blueprint.permission_state, CapturePermissionState::Required);
        assert_eq!(blueprint.example_sources.len(), 2);
    }

    #[test]
    fn runtime_listing_parser_maps_window_and_application_sources() {
        let sources = parse_runtime_listing(
            "window\tVLC\tNow Playing\napplication\tSafari\tSafari\nwindow\tMusic\t\n",
        );

        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].id, "macos-window-vlc-now-playing");
        assert_eq!(sources[0].label(), "VLC - Now Playing");
        assert_eq!(sources[1].id, "macos-application-safari-safari");
        assert_eq!(sources[1].label(), "Safari - Safari");
    }

    #[test]
    fn runtime_listing_parser_deduplicates_identical_rows() {
        let sources = parse_runtime_listing("window\tVLC\tNow Playing\nwindow\tVLC\tNow Playing\n");

        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn slugify_normalizes_runtime_source_identifiers() {
        assert_eq!(slugify("QuickTime Player"), "quicktime-player");
        assert_eq!(slugify("###"), "unnamed");
    }

    #[test]
    fn runtime_catalog_script_mentions_system_events() {
        assert!(runtime_catalog_script().contains("System Events"));
    }

    #[test]
    fn permission_probe_maps_authorization_and_runtime_errors() {
        assert_eq!(
            infer_permission_state_from_error(
                "execution error: Not authorized to send Apple events to System Events. (-1743)"
            ),
            CapturePermissionState::Denied
        );
        assert_eq!(
            infer_permission_state_from_error(
                "execution error: An error of type -10827 has occurred. (-10827)"
            ),
            CapturePermissionState::Unknown
        );
        assert_eq!(
            infer_permission_state_from_error("runtime catalog output was empty"),
            CapturePermissionState::Required
        );
    }
}
