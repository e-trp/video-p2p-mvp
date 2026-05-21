use transport_webrtc::IceServer;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IceServerEntry {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

impl IceServerEntry {
    pub fn to_transport(&self) -> IceServer {
        IceServer {
            urls: self.urls.clone(),
            username: self.username.clone(),
            credential: self.credential.clone(),
        }
    }

    fn to_spec_line(&self) -> String {
        let mut line = self.urls.join(",");
        if let Some(username) = self.username.as_deref() {
            line.push('|');
            line.push_str(username);
            line.push('|');
            line.push_str(self.credential.as_deref().unwrap_or_default());
        }
        line
    }
}

pub fn parse_ice_server_entries(input: &str) -> Result<Vec<IceServerEntry>, String> {
    let mut entries = Vec::new();

    for (index, raw_line) in input.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        entries.push(
            parse_ice_server_line(line).map_err(|error| format!("line {}: {error}", index + 1))?,
        );
    }

    Ok(entries)
}

pub fn format_ice_server_entries(entries: &[IceServerEntry]) -> String {
    entries
        .iter()
        .map(IceServerEntry::to_spec_line)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn summarize_ice_server_entries(entries: &[IceServerEntry]) -> String {
    if entries.is_empty() {
        return "none".to_string();
    }

    entries
        .iter()
        .map(|entry| {
            let urls = entry.urls.join(", ");
            if let Some(username) = entry.username.as_deref() {
                format!("{urls} (auth user {username})")
            } else {
                urls
            }
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn parse_ice_server_line(line: &str) -> Result<IceServerEntry, String> {
    let mut parts = line.split('|');
    let urls_part = parts.next().unwrap_or_default();
    let username_part = parts.next().map(str::trim);
    let credential_part = parts.next().map(str::trim);
    if parts.next().is_some() {
        return Err(
            "expected `urls` or `urls|username|credential` format for ICE server entry".to_string(),
        );
    }

    let urls = urls_part
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if urls.is_empty() {
        return Err("missing ICE server URL".to_string());
    }

    let username = username_part.filter(|value| !value.is_empty());
    let credential = credential_part.filter(|value| !value.is_empty());
    if username.is_some() ^ credential.is_some() {
        return Err("TURN credentials require both username and credential fields".to_string());
    }

    Ok(IceServerEntry {
        urls,
        username: username.map(ToOwned::to_owned),
        credential: credential.map(ToOwned::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        IceServerEntry, format_ice_server_entries, parse_ice_server_entries,
        summarize_ice_server_entries,
    };

    #[test]
    fn parse_ice_server_entries_accepts_stun_and_turn_lines() {
        let entries = parse_ice_server_entries(
            "stun:stun.example.com:3478\nturn:turn.example.com:3478?transport=udp|demo|secret",
        )
        .expect("parse ice servers");

        assert_eq!(
            entries,
            vec![
                IceServerEntry {
                    urls: vec!["stun:stun.example.com:3478".to_string()],
                    username: None,
                    credential: None,
                },
                IceServerEntry {
                    urls: vec!["turn:turn.example.com:3478?transport=udp".to_string()],
                    username: Some("demo".to_string()),
                    credential: Some("secret".to_string()),
                },
            ]
        );
    }

    #[test]
    fn format_ice_server_entries_roundtrips() {
        let entries = vec![
            IceServerEntry {
                urls: vec![
                    "stun:stun-1.example.com:3478".to_string(),
                    "stun:stun-2.example.com:3478".to_string(),
                ],
                username: None,
                credential: None,
            },
            IceServerEntry {
                urls: vec!["turn:turn.example.com:3478?transport=tcp".to_string()],
                username: Some("viewer".to_string()),
                credential: Some("p@ss".to_string()),
            },
        ];

        let formatted = format_ice_server_entries(&entries);
        let reparsed = parse_ice_server_entries(&formatted).expect("reparse ice servers");

        assert_eq!(reparsed, entries);
    }

    #[test]
    fn parse_ice_server_entries_rejects_partial_turn_credentials() {
        let error = parse_ice_server_entries("turn:turn.example.com:3478|demo")
            .expect_err("partial credentials should fail");

        assert!(error.contains("TURN credentials require both username and credential"));
    }

    #[test]
    fn summarize_ice_server_entries_reports_auth_usage() {
        let summary = summarize_ice_server_entries(&[
            IceServerEntry {
                urls: vec!["stun:stun.example.com:3478".to_string()],
                username: None,
                credential: None,
            },
            IceServerEntry {
                urls: vec!["turn:turn.example.com:3478?transport=udp".to_string()],
                username: Some("demo".to_string()),
                credential: Some("secret".to_string()),
            },
        ]);

        assert!(summary.contains("stun:stun.example.com:3478"));
        assert!(summary.contains("auth user demo"));
    }
}
