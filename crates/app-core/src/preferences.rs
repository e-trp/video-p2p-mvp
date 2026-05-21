use crate::ice_servers::{IceServerEntry, format_ice_server_entries, parse_ice_server_entries};
use capture_core::CaptureSelection;
use std::fs;
use std::path::PathBuf;

const HEADER: &str = "video-p2p-mvp-session-v1";
const APP_DIR: &str = "video-p2p-mvp";
const CONFIG_FILE: &str = "session.conf";
const CONFIG_DIR_OVERRIDE: &str = "VIDEO_P2P_MVP_CONFIG_DIR";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PersistedSessionConfig {
    pub room: Option<String>,
    pub signaling_addr: Option<String>,
    pub source_label: Option<String>,
    pub capture_selection: Option<CaptureSelection>,
    pub ice_servers: Vec<IceServerEntry>,
}

#[derive(Clone, Debug)]
pub struct PreferencesStore {
    path: Option<PathBuf>,
}

impl PreferencesStore {
    pub fn discover() -> Self {
        Self {
            path: default_config_file_path(),
        }
    }

    #[cfg(test)]
    pub(crate) fn from_config_dir(path: PathBuf) -> Self {
        Self {
            path: Some(path.join(CONFIG_FILE)),
        }
    }

    pub fn load(&self) -> Result<Option<PersistedSessionConfig>, String> {
        let Some(path) = self.path.as_ref() else {
            return Ok(None);
        };
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path).map_err(|error| {
            format!(
                "failed to read preferences from {}: {error}",
                path.display()
            )
        })?;
        parse_preferences(&content).map(Some).map_err(|error| {
            format!(
                "failed to parse preferences from {}: {error}",
                path.display()
            )
        })
    }

    pub fn save(&self, config: &PersistedSessionConfig) -> Result<(), String> {
        let Some(path) = self.path.as_ref() else {
            return Err("preferences path is unavailable on this system".to_string());
        };
        let Some(parent) = path.parent() else {
            return Err(format!(
                "preferences path has no parent: {}",
                path.display()
            ));
        };

        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create preferences directory {}: {error}",
                parent.display()
            )
        })?;
        fs::write(path, format_preferences(config))
            .map_err(|error| format!("failed to write preferences to {}: {error}", path.display()))
    }
}

fn default_config_file_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(CONFIG_DIR_OVERRIDE).filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(path).join(CONFIG_FILE));
    }

    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let config_dir = if cfg!(target_os = "macos") {
        home.join("Library")
            .join("Application Support")
            .join(APP_DIR)
    } else if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|value| !value.is_empty())
    {
        PathBuf::from(xdg).join(APP_DIR)
    } else {
        home.join(".config").join(APP_DIR)
    };

    Some(config_dir.join(CONFIG_FILE))
}

fn format_preferences(config: &PersistedSessionConfig) -> String {
    let mut lines = vec![HEADER.to_string()];
    push_optional_line(&mut lines, "room", config.room.as_deref());
    push_optional_line(
        &mut lines,
        "signaling_addr",
        config.signaling_addr.as_deref(),
    );
    push_optional_line(&mut lines, "source_label", config.source_label.as_deref());
    if !config.ice_servers.is_empty() {
        push_optional_line(
            &mut lines,
            "ice_servers",
            Some(&format_ice_server_entries(&config.ice_servers)),
        );
    }
    if let Some(selection) = config.capture_selection.as_ref() {
        push_optional_line(&mut lines, "selected_source_id", Some(&selection.source_id));
        lines.push(format!(
            "selected_source_audio={}",
            if selection.include_audio {
                "true"
            } else {
                "false"
            }
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn push_optional_line(lines: &mut Vec<String>, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        lines.push(format!("{key}={}", escape_value(value)));
    }
}

fn parse_preferences(content: &str) -> Result<PersistedSessionConfig, String> {
    let mut lines = content.lines();
    let Some(header) = lines.next() else {
        return Err("preferences file is empty".to_string());
    };
    if header.trim() != HEADER {
        return Err(format!("unsupported preferences header: {header}"));
    }

    let mut config = PersistedSessionConfig::default();
    let mut selected_source_id = None;
    let mut selected_source_audio = false;

    for raw_line in lines {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("invalid preferences line: {line}"));
        };
        let value = unescape_value(value);
        match key {
            "room" => config.room = Some(value),
            "signaling_addr" => config.signaling_addr = Some(value),
            "source_label" => config.source_label = Some(value),
            "ice_servers" => {
                config.ice_servers = parse_ice_server_entries(&value)
                    .map_err(|error| format!("invalid ice_servers: {error}"))?
            }
            "selected_source_id" => selected_source_id = Some(value),
            "selected_source_audio" => {
                selected_source_audio = match value.as_str() {
                    "true" => true,
                    "false" => false,
                    _ => return Err(format!("invalid selected_source_audio value: {value}")),
                };
            }
            _ => {}
        }
    }

    if let Some(source_id) = selected_source_id {
        config.capture_selection = Some(CaptureSelection {
            source_id,
            include_audio: selected_source_audio,
        });
    }

    Ok(config)
}

fn escape_value(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('=', "%3D")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn unescape_value(value: &str) -> String {
    value
        .replace("%0A", "\n")
        .replace("%0D", "\r")
        .replace("%3D", "=")
        .replace("%25", "%")
}

#[cfg(test)]
mod tests {
    use super::{PersistedSessionConfig, PreferencesStore, format_preferences, parse_preferences};
    use crate::ice_servers::IceServerEntry;
    use capture_core::CaptureSelection;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn preferences_roundtrip_preserves_special_characters() {
        let config = PersistedSessionConfig {
            room: Some("demo=room".to_string()),
            signaling_addr: Some("127.0.0.1:7000".to_string()),
            source_label: Some("vlc\nwindow".to_string()),
            ice_servers: vec![
                IceServerEntry {
                    urls: vec!["stun:stun.example.com:3478".to_string()],
                    username: None,
                    credential: None,
                },
                IceServerEntry {
                    urls: vec!["turn:turn.example.com:3478?transport=udp".to_string()],
                    username: Some("demo".to_string()),
                    credential: Some("s3cr3t".to_string()),
                },
            ],
            capture_selection: Some(CaptureSelection {
                source_id: "window%201".to_string(),
                include_audio: true,
            }),
        };

        let encoded = format_preferences(&config);
        let decoded = parse_preferences(&encoded).expect("roundtrip preferences");
        assert_eq!(decoded, config);
    }

    #[test]
    fn store_loads_saved_preferences() {
        let config_dir = unique_temp_dir("preferences-store");
        let store = PreferencesStore::from_config_dir(config_dir.clone());
        let config = PersistedSessionConfig {
            room: Some("demo".to_string()),
            signaling_addr: Some("127.0.0.1:7000".to_string()),
            source_label: Some("VLC".to_string()),
            ice_servers: vec![IceServerEntry {
                urls: vec!["stun:stun.example.com:3478".to_string()],
                username: None,
                credential: None,
            }],
            capture_selection: Some(CaptureSelection {
                source_id: "window-1".to_string(),
                include_audio: false,
            }),
        };

        store.save(&config).expect("save preferences");
        let loaded = store
            .load()
            .expect("load preferences")
            .expect("stored preferences");
        assert_eq!(loaded, config);

        let _ = fs::remove_dir_all(config_dir);
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        let dir = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
