use capture_core::{
    CapturePermissionState, CaptureSource, CaptureSourceKind, CaptureStreamConfig,
    CaptureStreamError, CaptureStreamEvent, CaptureStreamResult, CaptureStreamRuntime,
    CaptureStreamStatus,
};
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

pub struct MacCaptureRuntime {
    status: CaptureStreamStatus,
    active_source_id: Option<String>,
    pending_events: Vec<CaptureStreamEvent>,
    bridge: Box<dyn MacNativeCaptureBridge + Send>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MacRuntimeStartPlan {
    PermissionRequired {
        source: MacNativeSourceDescriptor,
        message: String,
    },
    SourceUnavailable(String),
    StartBridge {
        source: MacNativeSourceDescriptor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MacPermissionResolution {
    Granted,
    StillRequired(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MacNativeStreamSettings {
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
struct MacNativeSourceDescriptor {
    id: String,
    kind: CaptureSourceKind,
    display_name: String,
    app_name: Option<String>,
    has_audio: bool,
    label: String,
}

trait MacNativeCaptureBridge {
    fn start(
        &mut self,
        settings: &MacNativeStreamSettings,
    ) -> CaptureStreamResult<Vec<CaptureStreamEvent>>;
    fn poll_events(&mut self) -> CaptureStreamResult<Vec<CaptureStreamEvent>>;
    fn stop(&mut self, source_id: Option<String>) -> CaptureStreamResult<Vec<CaptureStreamEvent>>;
}

#[derive(Debug, Default)]
struct PlannedScreenCaptureKitBridge {
    active_source_id: Option<String>,
}

impl Default for MacCaptureRuntime {
    fn default() -> Self {
        Self {
            status: CaptureStreamStatus::Stopped,
            active_source_id: None,
            pending_events: Vec::new(),
            bridge: Box::<PlannedScreenCaptureKitBridge>::default(),
        }
    }
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

pub fn runtime() -> MacCaptureRuntime {
    MacCaptureRuntime::default()
}

pub fn current_catalog() -> MacCaptureCatalog {
    let blueprint = blueprint();

    match enumerate_runtime_sources() {
        Ok(sources) if !sources.is_empty() => {
            let permission_state = screen_recording_permission_state();
            MacCaptureCatalog {
                backend_label: format!("{} runtime catalog", blueprint.sources_api),
                permission_state,
                notes: vec![
                    format!(
                        "runtime catalog enumerated {} sources through osascript/System Events",
                        sources.len()
                    ),
                    format!(
                        "Screen Recording preflight state: {}",
                        describe_permission_state(permission_state)
                    ),
                ],
                sources,
                origin: MacSourceCatalogOrigin::Runtime,
            }
        }
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

impl CaptureStreamRuntime for MacCaptureRuntime {
    fn start(&mut self, config: CaptureStreamConfig) -> CaptureStreamResult<()> {
        let source_id = config.source_id().to_string();
        self.pending_events.clear();
        self.active_source_id = Some(source_id.clone());

        match plan_runtime_start(&config, &current_catalog()) {
            MacRuntimeStartPlan::PermissionRequired { source, message } => {
                match resolve_permission_requirement(message) {
                    MacPermissionResolution::Granted => self.start_native_bridge(&config, &source),
                    MacPermissionResolution::StillRequired(message) => {
                        self.status = CaptureStreamStatus::PermissionRequired;
                        self.pending_events.push(CaptureStreamEvent::StatusChanged {
                            source_id: Some(source_id),
                            status: self.status,
                            message: Some(message),
                        });
                        Ok(())
                    }
                }
            }
            MacRuntimeStartPlan::SourceUnavailable(message) => {
                self.status = CaptureStreamStatus::Failed;
                self.pending_events.push(CaptureStreamEvent::Error {
                    source_id: Some(source_id),
                    message: message.clone(),
                });
                Err(CaptureStreamError::new(message))
            }
            MacRuntimeStartPlan::StartBridge { source } => {
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

impl MacCaptureRuntime {
    fn start_native_bridge(
        &mut self,
        config: &CaptureStreamConfig,
        source: &MacNativeSourceDescriptor,
    ) -> CaptureStreamResult<()> {
        self.status = CaptureStreamStatus::Starting;
        let settings = MacNativeStreamSettings::from_config(config, source);
        let events = self.bridge.start(&settings)?;
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

impl MacNativeStreamSettings {
    fn from_config(config: &CaptureStreamConfig, source: &MacNativeSourceDescriptor) -> Self {
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

impl MacNativeSourceDescriptor {
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

impl MacNativeCaptureBridge for PlannedScreenCaptureKitBridge {
    fn start(
        &mut self,
        settings: &MacNativeStreamSettings,
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
                    "ScreenCaptureKit bridge boundary reached for {} ({:?}); target={}fps max={}x{} audio={}; native sample delivery is not implemented yet",
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
            reason: Some("ScreenCaptureKit bridge stopped".to_string()),
        }])
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

fn plan_runtime_start(
    config: &CaptureStreamConfig,
    catalog: &MacCaptureCatalog,
) -> MacRuntimeStartPlan {
    let source_id = config.source_id();
    let Some(source) = catalog.sources.iter().find(|source| source.id == source_id) else {
        return MacRuntimeStartPlan::SourceUnavailable(format!(
            "capture source is no longer available in the macOS catalog: {source_id}"
        ));
    };

    match catalog.permission_state {
        CapturePermissionState::Granted => MacRuntimeStartPlan::StartBridge {
            source: MacNativeSourceDescriptor::from_capture_source(source),
        },
        CapturePermissionState::Denied => MacRuntimeStartPlan::PermissionRequired {
            source: MacNativeSourceDescriptor::from_capture_source(source),
            message: "Screen Recording permission appears denied; enable it for this app in macOS Privacy & Security settings".to_string(),
        },
        CapturePermissionState::Required => MacRuntimeStartPlan::PermissionRequired {
            source: MacNativeSourceDescriptor::from_capture_source(source),
            message: "Screen Recording permission is required before starting ScreenCaptureKit capture"
                .to_string(),
        },
        CapturePermissionState::Unknown => MacRuntimeStartPlan::PermissionRequired {
            source: MacNativeSourceDescriptor::from_capture_source(source),
            message: "Screen Recording permission could not be verified in this environment".to_string(),
        },
    }
}

fn normalize_target_fps(target_fps: Option<u32>) -> u32 {
    target_fps.unwrap_or(30).clamp(1, 120)
}

fn normalize_dimension(value: Option<u32>, default: u32) -> u32 {
    value.unwrap_or(default).max(1)
}

fn resolve_permission_requirement(message: String) -> MacPermissionResolution {
    if request_screen_recording_permission() {
        MacPermissionResolution::Granted
    } else {
        MacPermissionResolution::StillRequired(format!(
            "{message}; macOS did not grant Screen Recording permission"
        ))
    }
}

fn screen_recording_permission_state() -> CapturePermissionState {
    if screen_recording_preflight_granted() {
        CapturePermissionState::Granted
    } else {
        CapturePermissionState::Required
    }
}

#[cfg(target_os = "macos")]
fn screen_recording_preflight_granted() -> bool {
    unsafe { CGPreflightScreenCaptureAccess() }
}

#[cfg(not(target_os = "macos"))]
fn screen_recording_preflight_granted() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn request_screen_recording_permission() -> bool {
    unsafe { CGRequestScreenCaptureAccess() }
}

#[cfg(not(target_os = "macos"))]
fn request_screen_recording_permission() -> bool {
    false
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
}

fn describe_permission_state(state: CapturePermissionState) -> &'static str {
    match state {
        CapturePermissionState::Unknown => "unknown",
        CapturePermissionState::Required => "required",
        CapturePermissionState::Granted => "granted",
        CapturePermissionState::Denied => "denied",
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
        MacCaptureCatalog, MacCaptureStage, MacNativeCaptureBridge, MacNativeSourceDescriptor,
        MacNativeStreamSettings, MacPermissionResolution, MacRuntimeStartPlan,
        MacSourceCatalogOrigin, PlannedScreenCaptureKitBridge, blueprint,
        describe_permission_state, infer_permission_state_from_error, normalize_dimension,
        normalize_target_fps, parse_runtime_listing, plan_runtime_start,
        resolve_permission_requirement, runtime, runtime_catalog_script, slugify,
    };
    use capture_core::{
        CapturePermissionState, CaptureSelection, CaptureSource, CaptureSourceKind,
        CaptureStreamConfig, CaptureStreamEvent, CaptureStreamRuntime, CaptureStreamStatus,
    };

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

    #[test]
    fn permission_state_description_is_stable() {
        assert_eq!(
            describe_permission_state(CapturePermissionState::Granted),
            "granted"
        );
    }

    #[test]
    fn runtime_start_plan_rejects_missing_source() {
        let catalog = test_catalog(CapturePermissionState::Granted);
        let plan = plan_runtime_start(&test_config("missing-source"), &catalog);

        assert!(matches!(
            plan,
            MacRuntimeStartPlan::SourceUnavailable(message)
                if message.contains("missing-source")
        ));
    }

    #[test]
    fn runtime_start_plan_requires_screen_recording_permission() {
        let catalog = test_catalog(CapturePermissionState::Required);
        let plan = plan_runtime_start(&test_config("mac-window-vlc"), &catalog);

        assert!(matches!(
            plan,
            MacRuntimeStartPlan::PermissionRequired {
                source,
                message,
            }
                if source.label == "VLC - VLC Player"
                    && source.kind == CaptureSourceKind::Window
                    && message.contains("Screen Recording permission is required")
        ));
    }

    #[test]
    fn permission_resolution_keeps_context_when_access_is_not_granted() {
        let resolution =
            resolve_permission_requirement("Screen Recording permission is required".to_string());

        assert!(matches!(
            resolution,
            MacPermissionResolution::StillRequired(message)
                if message.contains("macOS did not grant Screen Recording permission")
        ));
    }

    #[test]
    fn runtime_start_plan_allows_bridge_start_after_permission() {
        let catalog = test_catalog(CapturePermissionState::Granted);
        let plan = plan_runtime_start(&test_config("mac-window-vlc"), &catalog);

        assert!(matches!(
            plan,
            MacRuntimeStartPlan::StartBridge { source }
                if source.label == "VLC - VLC Player"
                    && source.kind == CaptureSourceKind::Window
        ));
    }

    #[test]
    fn planned_bridge_reports_native_boundary_without_media_samples() {
        let mut bridge = PlannedScreenCaptureKitBridge::default();
        let settings = MacNativeStreamSettings::from_config(
            &test_config("mac-window-vlc"),
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
            format!("{:?}", events)
                .contains("ScreenCaptureKit bridge boundary reached for VLC - VLC Player (Window); target=30fps max=1920x1080 audio=true")
        );
        assert!(
            bridge
                .poll_events()
                .expect("poll planned bridge")
                .is_empty()
        );
    }

    #[test]
    fn native_stream_settings_normalize_capture_config_for_bridge() {
        let settings = MacNativeStreamSettings::from_config(
            &CaptureStreamConfig {
                selection: CaptureSelection {
                    source_id: "mac-window-vlc".to_string(),
                    include_audio: false,
                },
                target_fps: Some(0),
                max_width: Some(0),
                max_height: None,
            },
            &test_source_descriptor(),
        );

        assert_eq!(settings.source_id, "mac-window-vlc");
        assert_eq!(settings.source_label, "VLC - VLC Player");
        assert_eq!(settings.source_kind, CaptureSourceKind::Window);
        assert_eq!(settings.display_name, "VLC Player");
        assert_eq!(settings.app_name.as_deref(), Some("VLC"));
        assert!(settings.source_has_audio);
        assert!(!settings.include_audio);
        assert_eq!(settings.target_fps, 1);
        assert_eq!(settings.max_width, 1);
        assert_eq!(settings.max_height, 720);
        assert_eq!(normalize_target_fps(Some(240)), 120);
        assert_eq!(normalize_dimension(None, 1280), 1280);
    }

    #[test]
    fn planned_runtime_reports_permission_or_bridge_state() {
        let mut runtime = runtime();
        runtime
            .start(CaptureStreamConfig {
                selection: CaptureSelection {
                    source_id: "mac-window-vlc".to_string(),
                    include_audio: true,
                },
                target_fps: Some(30),
                max_width: Some(1920),
                max_height: Some(1080),
            })
            .expect("start planned macOS runtime");

        let events = runtime.poll_events().expect("poll planned events");
        assert!(matches!(
            runtime.status(),
            CaptureStreamStatus::PermissionRequired | CaptureStreamStatus::Failed
        ));
        assert!(!events.is_empty());

        runtime.stop().expect("stop planned macOS runtime");
        let stop_events = runtime.poll_events().expect("poll stop event");
        assert_eq!(runtime.status(), CaptureStreamStatus::Stopped);
        assert!(matches!(
            stop_events.as_slice(),
            [CaptureStreamEvent::Stopped { .. }]
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

    fn test_catalog(permission_state: CapturePermissionState) -> MacCaptureCatalog {
        MacCaptureCatalog {
            backend_label: "test".to_string(),
            permission_state,
            sources: vec![CaptureSource {
                id: "mac-window-vlc".to_string(),
                kind: CaptureSourceKind::Window,
                display_name: "VLC Player".to_string(),
                app_name: Some("VLC".to_string()),
                has_audio: true,
            }],
            origin: MacSourceCatalogOrigin::Runtime,
            notes: Vec::new(),
        }
    }

    fn test_source_descriptor() -> MacNativeSourceDescriptor {
        MacNativeSourceDescriptor {
            id: "mac-window-vlc".to_string(),
            kind: CaptureSourceKind::Window,
            display_name: "VLC Player".to_string(),
            app_name: Some("VLC".to_string()),
            has_audio: true,
            label: "VLC - VLC Player".to_string(),
        }
    }
}
