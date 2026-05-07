use capture_core::{CapturePermissionState, CaptureSource, CaptureSourceKind};
use std::collections::HashSet;
use std::env;
use std::process::Command;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxSourceCatalogOrigin {
    Runtime,
    BlueprintFallback,
}

#[derive(Debug, Clone)]
pub struct LinuxCaptureCatalog {
    pub backend_label: String,
    pub permission_state: CapturePermissionState,
    pub sources: Vec<CaptureSource>,
    pub origin: LinuxSourceCatalogOrigin,
    pub notes: Vec<String>,
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

pub fn current_catalog() -> LinuxCaptureCatalog {
    let blueprint = blueprint();

    match enumerate_runtime_sources() {
        Ok(sources) if !sources.is_empty() => LinuxCaptureCatalog {
            backend_label: "x11_runtime_catalog".to_string(),
            permission_state: CapturePermissionState::Granted,
            notes: vec![format!(
                "runtime catalog enumerated {} windows through wmctrl",
                sources.len()
            )],
            sources,
            origin: LinuxSourceCatalogOrigin::Runtime,
        },
        Err(error) => LinuxCaptureCatalog {
            backend_label: format!("{:?}_blueprint_fallback", blueprint.preferred_backend),
            permission_state: infer_permission_state_from_error(&error),
            notes: vec![format!("runtime catalog fallback: {error}")],
            sources: blueprint.example_sources,
            origin: LinuxSourceCatalogOrigin::BlueprintFallback,
        },
        Ok(_) => LinuxCaptureCatalog {
            backend_label: format!("{:?}_blueprint_fallback", blueprint.preferred_backend),
            permission_state: CapturePermissionState::Required,
            notes: vec!["runtime catalog fallback: runtime source list was empty".to_string()],
            sources: blueprint.example_sources,
            origin: LinuxSourceCatalogOrigin::BlueprintFallback,
        },
    }
}

fn infer_permission_state_from_error(error: &str) -> CapturePermissionState {
    let has_display = env::var_os("DISPLAY").is_some() || env::var_os("WAYLAND_DISPLAY").is_some();
    infer_permission_state_from_error_with_display(error, has_display)
}

fn infer_permission_state_from_error_with_display(
    error: &str,
    has_display: bool,
) -> CapturePermissionState {
    let lower = error.to_ascii_lowercase();

    if lower.contains("cannot open display") || lower.contains("cannot get client list properties") {
        if has_display {
            CapturePermissionState::Required
        } else {
            CapturePermissionState::Unknown
        }
    } else if lower.contains("failed to launch wmctrl") || lower.contains("no such file or directory") {
        CapturePermissionState::Unknown
    } else {
        CapturePermissionState::Required
    }
}

fn enumerate_runtime_sources() -> Result<Vec<CaptureSource>, String> {
    let output = Command::new("wmctrl")
        .arg("-lp")
        .output()
        .map_err(|error| format!("failed to launch wmctrl: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("wmctrl returned {}: {stderr}", output.status));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let rows = parse_window_listing(&stdout);
    if rows.is_empty() {
        return Err("wmctrl output was empty".to_string());
    }

    Ok(rows)
}

fn parse_window_listing(raw: &str) -> Vec<CaptureSource> {
    let mut seen_ids = HashSet::new();
    let mut sources = Vec::new();

    for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let Some(source) = parse_window_row(line) else {
            continue;
        };

        if seen_ids.insert(source.id.clone()) {
            sources.push(source);
        }
    }

    sources
}

fn parse_window_row(line: &str) -> Option<CaptureSource> {
    let mut parts = line.split_whitespace();
    let window_id = parts.next()?;
    let _desktop = parts.next()?;
    let pid = parts.next()?;
    let _host = parts.next()?;
    let title = parts.collect::<Vec<_>>().join(" ");
    let display_name = title.trim();
    if display_name.is_empty() {
        return None;
    }

    let app_name = process_name_for_pid(pid)
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "unknown-app".to_string());

    Some(CaptureSource {
        id: make_source_id(window_id, &app_name, display_name),
        kind: CaptureSourceKind::Window,
        display_name: display_name.to_string(),
        app_name: Some(app_name),
        has_audio: true,
    })
}

fn process_name_for_pid(pid: &str) -> Result<String, String> {
    let output = Command::new("ps")
        .arg("-p")
        .arg(pid)
        .arg("-o")
        .arg("comm=")
        .output()
        .map_err(|error| format!("failed to launch ps: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("ps returned {}: {stderr}", output.status));
    }

    let raw_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw_name.is_empty() {
        return Err("ps returned empty process name".to_string());
    }

    let app_name = raw_name
        .rsplit('/')
        .next()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("unknown-app");
    Ok(app_name.to_string())
}

fn make_source_id(window_id: &str, app_name: &str, display_name: &str) -> String {
    let app_slug = slugify(app_name);
    let title_slug = slugify(display_name);
    format!("linux-window-{window_id}-{app_slug}-{title_slug}")
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
        LinuxCaptureBackend, blueprint, infer_permission_state_from_error,
        infer_permission_state_from_error_with_display, make_source_id, parse_window_listing,
        parse_window_row, slugify,
    };
    use capture_core::CapturePermissionState;

    #[test]
    fn blueprint_exposes_required_permission_and_example_sources() {
        let blueprint = blueprint();
        assert_eq!(blueprint.preferred_backend, LinuxCaptureBackend::PortalPipeWire);
        assert_eq!(blueprint.permission_state, CapturePermissionState::Required);
        assert_eq!(blueprint.example_sources.len(), 2);
    }

    #[test]
    fn parse_window_listing_maps_wmctrl_rows() {
        let sources = parse_window_listing(
            "0x01200007  0  4242 host Video Player\n0x01400003  0  4343 host Browser\n",
        );

        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].display_name, "Video Player");
        assert!(sources[0].id.starts_with("linux-window-0x01200007-"));
    }

    #[test]
    fn parse_window_listing_skips_rows_without_title() {
        let source = parse_window_row("0x01200007  0  4242");
        assert!(source.is_none());
    }

    #[test]
    fn slugify_normalizes_linux_runtime_source_identifiers() {
        assert_eq!(slugify("org.gnome.Nautilus"), "org-gnome-nautilus");
        assert_eq!(slugify("***"), "unnamed");
    }

    #[test]
    fn make_source_id_keeps_window_identity() {
        assert_eq!(
            make_source_id("0x01", "mpv", "Playlist"),
            "linux-window-0x01-mpv-playlist"
        );
    }

    #[test]
    fn permission_probe_maps_runtime_errors() {
        assert_eq!(
            infer_permission_state_from_error("failed to launch wmctrl: No such file or directory (os error 2)"),
            CapturePermissionState::Unknown
        );
        assert_eq!(
            infer_permission_state_from_error_with_display(
                "wmctrl returned 1: Cannot open display.",
                true
            ),
            CapturePermissionState::Required
        );
        assert_eq!(
            infer_permission_state_from_error_with_display(
                "wmctrl returned 1: Cannot open display.",
                false
            ),
            CapturePermissionState::Unknown
        );
        assert_eq!(
            infer_permission_state_from_error("runtime catalog output was empty"),
            CapturePermissionState::Required
        );
    }
}
