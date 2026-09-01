use capture_core::{
    CapturePermissionState, CaptureSource, CaptureSourceKind, CaptureStreamConfig,
    CaptureStreamError, CaptureStreamEvent, CaptureStreamResult, CaptureStreamRuntime,
    CaptureStreamStatus,
};
use std::collections::HashSet;
use std::env;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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

pub struct LinuxCaptureRuntime {
    status: CaptureStreamStatus,
    active_source_id: Option<String>,
    pending_events: Vec<CaptureStreamEvent>,
    bridge: Box<dyn LinuxNativeCaptureBridge + Send>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LinuxRuntimeStartPlan {
    PermissionBlocked {
        source: LinuxNativeSourceDescriptor,
        status: CaptureStreamStatus,
        message: String,
    },
    SourceUnavailable(String),
    StartBridge {
        source: LinuxNativeSourceDescriptor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinuxNativeStreamSettings {
    source_id: String,
    source_label: String,
    source_kind: CaptureSourceKind,
    display_name: String,
    app_name: Option<String>,
    source_has_audio: bool,
    include_audio: bool,
    target_fps: u32,
    max_width: u32,
    max_height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinuxNativeSourceDescriptor {
    id: String,
    kind: CaptureSourceKind,
    display_name: String,
    app_name: Option<String>,
    has_audio: bool,
    label: String,
}

trait LinuxNativeCaptureBridge {
    fn start(
        &mut self,
        settings: &LinuxNativeStreamSettings,
    ) -> CaptureStreamResult<Vec<CaptureStreamEvent>>;
    fn poll_events(&mut self) -> CaptureStreamResult<Vec<CaptureStreamEvent>>;
    fn stop(&mut self, source_id: Option<String>) -> CaptureStreamResult<Vec<CaptureStreamEvent>>;
}

#[derive(Debug, Default)]
struct PlannedPortalPipeWireBridge {
    active_source_id: Option<String>,
}

impl Default for LinuxCaptureRuntime {
    fn default() -> Self {
        Self {
            status: CaptureStreamStatus::Stopped,
            active_source_id: None,
            pending_events: Vec::new(),
            bridge: Box::<PlannedPortalPipeWireBridge>::default(),
        }
    }
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

pub fn runtime() -> LinuxCaptureRuntime {
    LinuxCaptureRuntime::default()
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

impl CaptureStreamRuntime for LinuxCaptureRuntime {
    fn start(&mut self, config: CaptureStreamConfig) -> CaptureStreamResult<()> {
        let source_id = config.source_id().to_string();
        self.pending_events.clear();
        self.active_source_id = Some(source_id.clone());

        match plan_runtime_start(&config, &current_catalog()) {
            LinuxRuntimeStartPlan::PermissionBlocked {
                status, message, ..
            } => {
                self.status = status;
                self.pending_events.push(CaptureStreamEvent::StatusChanged {
                    source_id: Some(source_id),
                    status: self.status,
                    message: Some(message),
                });
                Ok(())
            }
            LinuxRuntimeStartPlan::SourceUnavailable(message) => {
                self.status = CaptureStreamStatus::Failed;
                self.pending_events.push(CaptureStreamEvent::Error {
                    source_id: Some(source_id),
                    message: message.clone(),
                });
                Err(CaptureStreamError::new(message))
            }
            LinuxRuntimeStartPlan::StartBridge { source } => {
                self.start_native_bridge(&config, &source)
            }
        }
    }

    fn poll_events(&mut self) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
        let mut events = std::mem::take(&mut self.pending_events);
        events.extend(self.bridge.poll_events()?);
        self.apply_event_status(&events);
        Ok(events)
    }

    fn stop(&mut self) -> CaptureStreamResult<()> {
        let source_id = self.active_source_id.take();
        let events = self.bridge.stop(source_id)?;
        self.apply_event_status(&events);
        self.pending_events.extend(events);
        Ok(())
    }

    fn status(&self) -> CaptureStreamStatus {
        self.status
    }
}

impl LinuxCaptureRuntime {
    #[cfg(test)]
    fn with_bridge(bridge: Box<dyn LinuxNativeCaptureBridge + Send>) -> Self {
        Self {
            status: CaptureStreamStatus::Stopped,
            active_source_id: None,
            pending_events: Vec::new(),
            bridge,
        }
    }

    fn start_native_bridge(
        &mut self,
        config: &CaptureStreamConfig,
        source: &LinuxNativeSourceDescriptor,
    ) -> CaptureStreamResult<()> {
        self.status = CaptureStreamStatus::Starting;
        let settings = LinuxNativeStreamSettings::from_config(config, source);
        let events = match self.bridge.start(&settings) {
            Ok(events) => events,
            Err(error) => {
                self.status = CaptureStreamStatus::Failed;
                self.pending_events.push(CaptureStreamEvent::Error {
                    source_id: Some(source.id.clone()),
                    message: format!("Portal/PipeWire bridge failed to start: {error}"),
                });
                return Err(error);
            }
        };
        self.apply_event_status(&events);
        self.pending_events.extend(events);
        Ok(())
    }

    fn apply_event_status(&mut self, events: &[CaptureStreamEvent]) {
        for event in events {
            match event {
                CaptureStreamEvent::Started { .. } => {
                    if self.status == CaptureStreamStatus::Starting {
                        self.status = CaptureStreamStatus::Running;
                    }
                }
                CaptureStreamEvent::StatusChanged { status, .. } => {
                    self.status = *status;
                }
                CaptureStreamEvent::Stopped { .. } => {
                    self.status = CaptureStreamStatus::Stopped;
                }
                CaptureStreamEvent::Error { .. } => {
                    self.status = CaptureStreamStatus::Failed;
                }
                CaptureStreamEvent::VideoFrame { .. } | CaptureStreamEvent::AudioBuffer { .. } => {}
            }
        }
    }
}

impl LinuxNativeStreamSettings {
    fn from_config(config: &CaptureStreamConfig, source: &LinuxNativeSourceDescriptor) -> Self {
        Self {
            source_id: source.id.clone(),
            source_label: source.label.clone(),
            source_kind: source.kind,
            display_name: source.display_name.clone(),
            app_name: source.app_name.clone(),
            source_has_audio: source.has_audio,
            include_audio: config.includes_audio() && source.has_audio,
            target_fps: normalize_target_fps(config.target_fps),
            max_width: normalize_dimension(config.max_width, 1280),
            max_height: normalize_dimension(config.max_height, 720),
        }
    }
}

impl LinuxNativeSourceDescriptor {
    fn from_capture_source(source: &CaptureSource) -> Self {
        Self {
            id: source.id.clone(),
            kind: source.kind,
            display_name: source.display_name.clone(),
            app_name: source.app_name.clone(),
            has_audio: source.has_audio,
            label: source.label(),
        }
    }
}

impl LinuxNativeCaptureBridge for PlannedPortalPipeWireBridge {
    fn start(
        &mut self,
        settings: &LinuxNativeStreamSettings,
    ) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
        let source_id = settings.source_id.clone();
        self.active_source_id = Some(source_id.clone());
        Ok(vec![
            CaptureStreamEvent::Started {
                source_id: source_id.clone(),
            },
            CaptureStreamEvent::StatusChanged {
                source_id: Some(source_id),
                status: CaptureStreamStatus::Failed,
                message: Some(format!(
                    "Portal/PipeWire bridge boundary reached for {} ({:?}); target={}fps max={}x{} audio={}; native sample delivery is not implemented yet",
                    settings.source_label,
                    settings.source_kind,
                    settings.target_fps,
                    settings.max_width,
                    settings.max_height,
                    settings.include_audio
                )),
            },
        ])
    }

    fn poll_events(&mut self) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
        Ok(Vec::new())
    }

    fn stop(&mut self, source_id: Option<String>) -> CaptureStreamResult<Vec<CaptureStreamEvent>> {
        let source_id = source_id.or_else(|| self.active_source_id.take());
        Ok(vec![CaptureStreamEvent::Stopped {
            source_id,
            reason: Some("Portal/PipeWire bridge stopped".to_string()),
        }])
    }
}

fn plan_runtime_start(
    config: &CaptureStreamConfig,
    catalog: &LinuxCaptureCatalog,
) -> LinuxRuntimeStartPlan {
    let source_id = config.source_id();
    let Some(source) = catalog.sources.iter().find(|source| source.id == source_id) else {
        return LinuxRuntimeStartPlan::SourceUnavailable(format!(
            "capture source is no longer available in the Linux catalog: {source_id}"
        ));
    };

    match catalog.permission_state {
        CapturePermissionState::Granted => LinuxRuntimeStartPlan::StartBridge {
            source: LinuxNativeSourceDescriptor::from_capture_source(source),
        },
        CapturePermissionState::Denied => LinuxRuntimeStartPlan::PermissionBlocked {
            source: LinuxNativeSourceDescriptor::from_capture_source(source),
            status: CaptureStreamStatus::PermissionDenied,
            message: "Linux capture permission appears denied; approve the source through the desktop portal or session environment".to_string(),
        },
        CapturePermissionState::Required => LinuxRuntimeStartPlan::PermissionBlocked {
            source: LinuxNativeSourceDescriptor::from_capture_source(source),
            status: CaptureStreamStatus::PermissionRequired,
            message: "Linux capture permission is required before starting Portal/PipeWire capture"
                .to_string(),
        },
        CapturePermissionState::Unknown => LinuxRuntimeStartPlan::PermissionBlocked {
            source: LinuxNativeSourceDescriptor::from_capture_source(source),
            status: CaptureStreamStatus::PermissionRequired,
            message: "Linux capture permission could not be verified in this environment"
                .to_string(),
        },
    }
}

fn normalize_target_fps(target_fps: Option<u32>) -> u32 {
    target_fps.unwrap_or(30).clamp(1, 120)
}

fn normalize_dimension(value: Option<u32>, default: u32) -> u32 {
    value.unwrap_or(default).max(1)
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

    if lower.contains("cannot open display") || lower.contains("cannot get client list properties")
    {
        if has_display {
            CapturePermissionState::Required
        } else {
            CapturePermissionState::Unknown
        }
    } else if lower.contains("failed to launch wmctrl")
        || lower.contains("no such file or directory")
    {
        CapturePermissionState::Unknown
    } else {
        CapturePermissionState::Required
    }
}

fn enumerate_runtime_sources() -> Result<Vec<CaptureSource>, String> {
    let mut command = Command::new("wmctrl");
    command.arg("-lp");
    let output = command_output_with_timeout(
        command,
        Duration::from_millis(800),
        "wmctrl runtime catalog",
    )?;

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
    let mut command = Command::new("ps");
    command.arg("-p").arg(pid).arg("-o").arg("comm=");
    let output = command_output_with_timeout(command, Duration::from_millis(250), "ps")?;

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

fn command_output_with_timeout(
    mut command: Command,
    timeout: Duration,
    label: &str,
) -> Result<Output, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to launch {label}: {error}"))?;
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|error| format!("failed to collect {label} output: {error}"));
            }
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait_with_output();
                return Err(format!("{label} timed out after {}ms", timeout.as_millis()));
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(format!("failed to poll {label}: {error}")),
        }
    }
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
        LinuxCaptureBackend, LinuxCaptureCatalog, LinuxNativeCaptureBridge,
        LinuxNativeSourceDescriptor, LinuxNativeStreamSettings, LinuxRuntimeStartPlan,
        LinuxSourceCatalogOrigin, PlannedPortalPipeWireBridge, blueprint,
        infer_permission_state_from_error, infer_permission_state_from_error_with_display,
        make_source_id, normalize_dimension, normalize_target_fps, parse_window_listing,
        parse_window_row, plan_runtime_start, runtime, slugify,
    };
    use capture_core::{
        CapturePermissionState, CaptureSelection, CaptureSource, CaptureSourceKind,
        CaptureStreamConfig, CaptureStreamError, CaptureStreamEvent, CaptureStreamRuntime,
        CaptureStreamStatus,
    };
    use std::sync::{Arc, Mutex};

    #[test]
    fn blueprint_exposes_required_permission_and_example_sources() {
        let blueprint = blueprint();
        assert_eq!(
            blueprint.preferred_backend,
            LinuxCaptureBackend::PortalPipeWire
        );
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
            infer_permission_state_from_error(
                "failed to launch wmctrl: No such file or directory (os error 2)"
            ),
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

    #[test]
    fn planned_runtime_reports_permission_required_until_bridge_exists() {
        let mut runtime = runtime();
        runtime
            .start(CaptureStreamConfig {
                selection: CaptureSelection {
                    source_id: "linux-window-player".to_string(),
                    include_audio: true,
                },
                target_fps: Some(30),
                max_width: Some(1920),
                max_height: Some(1080),
            })
            .expect("start planned Linux runtime");

        let events = runtime.poll_events().expect("poll planned events");
        assert_eq!(runtime.status(), CaptureStreamStatus::PermissionRequired);
        assert!(matches!(
            events.as_slice(),
            [CaptureStreamEvent::StatusChanged {
                status: CaptureStreamStatus::PermissionRequired,
                ..
            }]
        ));
        assert!(format!("{:?}", events[0]).contains("Linux capture permission"));

        runtime.stop().expect("stop planned Linux runtime");
        let stop_events = runtime.poll_events().expect("poll stop event");
        assert_eq!(runtime.status(), CaptureStreamStatus::Stopped);
        assert!(matches!(
            stop_events.as_slice(),
            [CaptureStreamEvent::Stopped { .. }]
        ));
    }

    #[test]
    fn runtime_start_plan_rejects_missing_source() {
        let catalog = test_catalog(CapturePermissionState::Granted);
        let plan = plan_runtime_start(&test_config("missing-source"), &catalog);

        assert!(matches!(
            plan,
            LinuxRuntimeStartPlan::SourceUnavailable(message)
                if message.contains("missing-source")
        ));
    }

    #[test]
    fn runtime_start_plan_requires_portal_permission() {
        let catalog = test_catalog(CapturePermissionState::Required);
        let plan = plan_runtime_start(&test_config("linux-window-player"), &catalog);

        assert!(matches!(
            plan,
            LinuxRuntimeStartPlan::PermissionBlocked {
                source,
                status: CaptureStreamStatus::PermissionRequired,
                message,
            }
                if source.label == "mpv - Video Player"
                    && source.kind == CaptureSourceKind::Window
                    && message.contains("Linux capture permission is required")
        ));
    }

    #[test]
    fn runtime_start_plan_preserves_denied_portal_status() {
        let catalog = test_catalog(CapturePermissionState::Denied);
        let plan = plan_runtime_start(&test_config("linux-window-player"), &catalog);

        assert!(matches!(
            plan,
            LinuxRuntimeStartPlan::PermissionBlocked {
                status: CaptureStreamStatus::PermissionDenied,
                message,
                ..
            } if message.contains("permission appears denied")
        ));
    }

    #[test]
    fn runtime_start_plan_allows_bridge_start_when_permission_is_granted() {
        let catalog = test_catalog(CapturePermissionState::Granted);
        let plan = plan_runtime_start(&test_config("linux-window-player"), &catalog);

        assert!(matches!(
            plan,
            LinuxRuntimeStartPlan::StartBridge { source }
                if source.label == "mpv - Video Player"
                    && source.kind == CaptureSourceKind::Window
        ));
    }

    #[test]
    fn native_stream_settings_normalize_capture_config_for_bridge() {
        let settings = LinuxNativeStreamSettings::from_config(
            &CaptureStreamConfig {
                selection: CaptureSelection {
                    source_id: "linux-display-1".to_string(),
                    include_audio: true,
                },
                target_fps: Some(0),
                max_width: Some(0),
                max_height: None,
            },
            &LinuxNativeSourceDescriptor {
                id: "linux-display-1".to_string(),
                kind: CaptureSourceKind::Display,
                display_name: "Display 1".to_string(),
                app_name: None,
                has_audio: false,
                label: "Display 1".to_string(),
            },
        );

        assert_eq!(settings.source_id, "linux-display-1");
        assert_eq!(settings.source_label, "Display 1");
        assert_eq!(settings.source_kind, CaptureSourceKind::Display);
        assert_eq!(settings.display_name, "Display 1");
        assert_eq!(settings.app_name, None);
        assert!(!settings.source_has_audio);
        assert!(!settings.include_audio);
        assert_eq!(settings.target_fps, 1);
        assert_eq!(settings.max_width, 1);
        assert_eq!(settings.max_height, 720);
        assert_eq!(normalize_target_fps(Some(240)), 120);
        assert_eq!(normalize_dimension(None, 1280), 1280);
    }

    #[test]
    fn planned_bridge_reports_native_boundary_without_media_samples() {
        let mut bridge = PlannedPortalPipeWireBridge::default();
        let settings = LinuxNativeStreamSettings::from_config(
            &test_config("linux-window-player"),
            &test_source_descriptor(),
        );
        let events = bridge.start(&settings).expect("start planned bridge");

        assert!(matches!(
            events.as_slice(),
            [
                CaptureStreamEvent::Started { .. },
                CaptureStreamEvent::StatusChanged {
                    status: CaptureStreamStatus::Failed,
                    ..
                }
            ]
        ));
        assert!(
            format!("{:?}", events).contains(
                "Portal/PipeWire bridge boundary reached for mpv - Video Player (Window); target=30fps max=1920x1080 audio=true"
            )
        );
        assert!(
            bridge
                .poll_events()
                .expect("poll planned bridge")
                .is_empty()
        );
    }

    #[test]
    fn runtime_hands_normalized_settings_to_injected_bridge() {
        let seen_settings = Arc::new(Mutex::new(Vec::new()));
        let mut runtime = super::LinuxCaptureRuntime::with_bridge(Box::new(RecordingBridge {
            seen_settings: seen_settings.clone(),
            start_events: vec![CaptureStreamEvent::Started {
                source_id: "linux-window-player".to_string(),
            }],
            poll_events: Vec::new(),
        }));

        runtime
            .start_native_bridge(
                &test_config("linux-window-player"),
                &test_source_descriptor(),
            )
            .expect("start recording bridge");

        let settings = seen_settings.lock().expect("settings lock");
        assert_eq!(settings.len(), 1);
        assert_eq!(settings[0].source_id, "linux-window-player");
        assert_eq!(settings[0].source_label, "mpv - Video Player");
        assert_eq!(settings[0].target_fps, 30);
        assert_eq!(settings[0].max_width, 1920);
        assert_eq!(settings[0].max_height, 1080);
        assert!(settings[0].include_audio);
        drop(settings);

        let events = runtime.poll_events().expect("poll recording bridge");
        assert_eq!(runtime.status(), CaptureStreamStatus::Running);
        assert!(matches!(
            events.as_slice(),
            [CaptureStreamEvent::Started { .. }]
        ));
    }

    #[test]
    fn runtime_polls_injected_bridge_events_and_updates_status() {
        let seen_settings = Arc::new(Mutex::new(Vec::new()));
        let mut runtime = super::LinuxCaptureRuntime::with_bridge(Box::new(RecordingBridge {
            seen_settings,
            start_events: vec![CaptureStreamEvent::Started {
                source_id: "linux-window-player".to_string(),
            }],
            poll_events: vec![CaptureStreamEvent::StatusChanged {
                source_id: Some("linux-window-player".to_string()),
                status: CaptureStreamStatus::Running,
                message: Some("PipeWire stream loop is active".to_string()),
            }],
        }));

        runtime
            .start_native_bridge(
                &test_config("linux-window-player"),
                &test_source_descriptor(),
            )
            .expect("start recording bridge");

        let first_events = runtime.poll_events().expect("poll started event");
        assert_eq!(first_events.len(), 2);
        assert_eq!(runtime.status(), CaptureStreamStatus::Running);
        assert!(format!("{:?}", first_events).contains("PipeWire stream loop is active"));

        runtime.stop().expect("stop runtime");
        let stop_events = runtime.poll_events().expect("poll stop event");
        assert_eq!(runtime.status(), CaptureStreamStatus::Stopped);
        assert!(matches!(
            stop_events.as_slice(),
            [CaptureStreamEvent::Stopped { .. }]
        ));
    }

    #[test]
    fn runtime_marks_failed_when_injected_bridge_start_fails() {
        let mut runtime = super::LinuxCaptureRuntime::with_bridge(Box::new(FailingBridge));

        let error = runtime
            .start_native_bridge(
                &test_config("linux-window-player"),
                &test_source_descriptor(),
            )
            .expect_err("bridge start should fail");

        assert_eq!(error.message(), "native bridge unavailable");
        assert_eq!(runtime.status(), CaptureStreamStatus::Failed);
        let events = runtime.poll_events().expect("poll failure event");
        assert!(matches!(
            events.as_slice(),
            [CaptureStreamEvent::Error { source_id, message }]
                if source_id.as_deref() == Some("linux-window-player")
                    && message.contains("Portal/PipeWire bridge failed to start")
        ));
    }

    fn test_config(source_id: &str) -> CaptureStreamConfig {
        CaptureStreamConfig {
            selection: CaptureSelection {
                source_id: source_id.to_string(),
                include_audio: true,
            },
            target_fps: Some(30),
            max_width: Some(1920),
            max_height: Some(1080),
        }
    }

    fn test_catalog(permission_state: CapturePermissionState) -> LinuxCaptureCatalog {
        LinuxCaptureCatalog {
            backend_label: "test".to_string(),
            permission_state,
            sources: vec![CaptureSource {
                id: "linux-window-player".to_string(),
                kind: CaptureSourceKind::Window,
                display_name: "Video Player".to_string(),
                app_name: Some("mpv".to_string()),
                has_audio: true,
            }],
            origin: LinuxSourceCatalogOrigin::Runtime,
            notes: Vec::new(),
        }
    }

    fn test_source_descriptor() -> LinuxNativeSourceDescriptor {
        LinuxNativeSourceDescriptor {
            id: "linux-window-player".to_string(),
            kind: CaptureSourceKind::Window,
            display_name: "Video Player".to_string(),
            app_name: Some("mpv".to_string()),
            has_audio: true,
            label: "mpv - Video Player".to_string(),
        }
    }

    struct RecordingBridge {
        seen_settings: Arc<Mutex<Vec<LinuxNativeStreamSettings>>>,
        start_events: Vec<CaptureStreamEvent>,
        poll_events: Vec<CaptureStreamEvent>,
    }

    impl LinuxNativeCaptureBridge for RecordingBridge {
        fn start(
            &mut self,
            settings: &LinuxNativeStreamSettings,
        ) -> capture_core::CaptureStreamResult<Vec<CaptureStreamEvent>> {
            self.seen_settings
                .lock()
                .expect("settings lock")
                .push(settings.clone());
            Ok(self.start_events.clone())
        }

        fn poll_events(&mut self) -> capture_core::CaptureStreamResult<Vec<CaptureStreamEvent>> {
            Ok(std::mem::take(&mut self.poll_events))
        }

        fn stop(
            &mut self,
            source_id: Option<String>,
        ) -> capture_core::CaptureStreamResult<Vec<CaptureStreamEvent>> {
            Ok(vec![CaptureStreamEvent::Stopped {
                source_id,
                reason: Some("recording bridge stopped".to_string()),
            }])
        }
    }

    struct FailingBridge;

    impl LinuxNativeCaptureBridge for FailingBridge {
        fn start(
            &mut self,
            _settings: &LinuxNativeStreamSettings,
        ) -> capture_core::CaptureStreamResult<Vec<CaptureStreamEvent>> {
            Err(CaptureStreamError::new("native bridge unavailable"))
        }

        fn poll_events(&mut self) -> capture_core::CaptureStreamResult<Vec<CaptureStreamEvent>> {
            Ok(Vec::new())
        }

        fn stop(
            &mut self,
            source_id: Option<String>,
        ) -> capture_core::CaptureStreamResult<Vec<CaptureStreamEvent>> {
            Ok(vec![CaptureStreamEvent::Stopped {
                source_id,
                reason: Some("failing bridge stopped".to_string()),
            }])
        }
    }
}
