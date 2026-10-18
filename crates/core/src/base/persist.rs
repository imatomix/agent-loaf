use std::fs;
use std::io;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    crate::base::paths::write_private(&tmp, &serde_json::to_vec_pretty(value)?)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::settings::Settings;
    use crate::session::state::Status;
    use crate::session::store::test_support::ev;
    use crate::session::store::SessionStore;

    #[test]
    fn round_trips_settings_and_store() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            notify_on: vec![Status::Done],
            launch_at_login: true,
            ..Settings::default()
        };
        save_json(&dir.path().join("settings.json"), &settings).unwrap();
        assert_eq!(load_json::<Settings>(&dir.path().join("settings.json")), settings);

        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1));
        save_json(&dir.path().join("state.json"), &store).unwrap();
        let loaded: SessionStore = load_json(&dir.path().join("state.json"));
        assert_eq!(loaded.sessions(), store.sessions());
    }

    #[test]
    fn missing_or_broken_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load_json::<Settings>(&dir.path().join("none.json")),
            Settings::default()
        );
        fs::write(dir.path().join("broken.json"), "{oops").unwrap();
        assert_eq!(
            load_json::<Settings>(&dir.path().join("broken.json")),
            Settings::default()
        );
    }

    #[test]
    fn partial_settings_keep_defaults_for_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("s.json"), r#"{"launch_at_login":true}"#).unwrap();
        let loaded: Settings = load_json(&dir.path().join("s.json"));
        assert!(loaded.launch_at_login);
        assert_eq!(loaded.notify_on, Settings::default().notify_on);
    }

    #[test]
    fn v02_state_with_string_detail_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let old = r#"{"sessions":[{"id":"s1","status":"needs_action","detail":"許可待ち · Bash: ls","status_since_ms":1,"first_seen_ms":1,"last_event_ms":1,"cwd":"/w/api","config_dir":"/u/.claude","git_branch":null,"claude_pid":7,"terminal":null,"transcript_path":null,"active_subagents":0}]}"#;
        fs::write(dir.path().join("state.json"), old).unwrap();
        let store: SessionStore = load_json(&dir.path().join("state.json"));
        let session = &store.sessions()[0];
        assert_eq!(session.status, Status::NeedsAction);
        assert_eq!(session.detail_kind, None);
        assert_eq!(session.detail_text, None);
    }

    #[test]
    fn v03_state_with_subagent_count_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let old = r#"{"sessions":[{"id":"s1","status":"done","detail_kind":null,"detail_text":null,"status_since_ms":1,"first_seen_ms":1,"last_event_ms":1,"cwd":"/w/api","start_cwd":"/w/api","config_dir":"/u/.claude","git_branch":null,"claude_pid":7,"terminal":null,"transcript_path":null,"active_subagents":2}]}"#;
        fs::write(dir.path().join("state.json"), old).unwrap();
        let store: SessionStore = load_json(&dir.path().join("state.json"));
        assert_eq!(store.sessions().len(), 1);
        assert!(store.sessions()[0].running_subagents.is_empty());
        assert_eq!(store.sessions()[0].agent, crate::base::agent::Agent::ClaudeCode);
    }
}
