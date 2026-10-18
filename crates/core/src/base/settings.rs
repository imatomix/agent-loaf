use serde::{Deserialize, Serialize};

use crate::session::state::Status;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub notify_on: Vec<Status>,
    pub launch_at_login: bool,
    // None は「まだ決めていない」。すべて削除した空の一覧と区別し、~/.claude を自動で入れるのは None のときだけにする
    pub config_dirs: Option<Vec<String>>,
    pub claude_path: Option<String>,
    pub session_order: Vec<String>,
    // ~/.codex を一度自動で足したら true。外した後に、起動のたびに入れ直さないため
    pub codex_dir_offered: bool,
    pub codex_path: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            notify_on: vec![Status::NeedsAction, Status::Error],
            launch_at_login: false,
            config_dirs: None,
            claude_path: None,
            session_order: Vec::new(),
            codex_dir_offered: false,
            codex_path: None,
        }
    }
}

// 画面から変えられるのはこの 2 つだけ。設定全体を送り返させると、裏で更新した config_dirs や session_order を古い値で上書きしてしまう
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    pub notify_on: Vec<Status>,
    pub launch_at_login: bool,
}

impl Settings {
    pub fn preferences(&self) -> Preferences {
        Preferences {
            notify_on: self.notify_on.clone(),
            launch_at_login: self.launch_at_login,
        }
    }

    pub fn apply_preferences(&mut self, preferences: Preferences) {
        self.notify_on = preferences.notify_on;
        self.launch_at_login = preferences.launch_at_login;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v01_settings_file_loads_with_new_fields_unset() {
        let loaded: Settings = serde_json::from_str(r#"{"notify_on":["done"],"launch_at_login":true}"#).unwrap();
        assert_eq!(loaded.notify_on, vec![Status::Done]);
        assert!(loaded.launch_at_login);
        assert_eq!(loaded.config_dirs, None);
        assert_eq!(loaded.claude_path, None);
        assert!(loaded.session_order.is_empty());
        assert!(!loaded.codex_dir_offered);
        assert_eq!(loaded.codex_path, None);
    }

    #[test]
    fn empty_config_dirs_is_distinct_from_unset() {
        let settings = Settings {
            config_dirs: Some(vec![]),
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.config_dirs, Some(vec![]));
    }

    #[test]
    fn applying_preferences_keeps_other_fields() {
        let mut settings = Settings {
            config_dirs: Some(vec!["/u/.claude".into()]),
            claude_path: Some("/opt/claude".into()),
            session_order: vec!["s1".into()],
            ..Settings::default()
        };
        settings.apply_preferences(Preferences {
            notify_on: vec![Status::Done],
            launch_at_login: true,
        });
        assert_eq!(
            settings.preferences(),
            Preferences {
                notify_on: vec![Status::Done],
                launch_at_login: true
            }
        );
        assert_eq!(settings.config_dirs, Some(vec!["/u/.claude".into()]));
        assert_eq!(settings.claude_path.as_deref(), Some("/opt/claude"));
        assert_eq!(settings.session_order, vec!["s1".to_string()]);
    }
}
