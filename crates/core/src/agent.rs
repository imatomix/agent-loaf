use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Agent {
    // 前の版のイベントや state.json には種類がないので、Claude Code として読む
    #[default]
    ClaudeCode,
    Codex,
}

// 保存する設定の形を変えないため、種類はそのつどフォルダーの中身で決める。
// 設定を何も変えていない Codex には config.toml がないことがあるので、ログインの情報（auth.json）と、既定の名前（.codex）も見る
pub fn agent_of_config_dir(dir: &Path) -> Agent {
    let codex = dir.join("config.toml").is_file()
        || dir.join("auth.json").is_file()
        || dir.file_name().is_some_and(|name| name == ".codex");
    if codex {
        Agent::Codex
    } else {
        Agent::ClaudeCode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_records_without_agent_are_claude_code() {
        assert_eq!(Agent::default(), Agent::ClaudeCode);
        assert_eq!(serde_json::to_value(Agent::Codex).unwrap(), "codex");
        assert_eq!(
            serde_json::from_str::<Agent>("\"claude_code\"").unwrap(),
            Agent::ClaudeCode
        );
    }

    #[test]
    fn config_dir_with_config_toml_is_codex() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(agent_of_config_dir(dir.path()), Agent::ClaudeCode);
        std::fs::write(dir.path().join("config.toml"), "model = \"x\"\n").unwrap();
        assert_eq!(agent_of_config_dir(dir.path()), Agent::Codex);
        assert_eq!(agent_of_config_dir(Path::new("/nonexistent/x")), Agent::ClaudeCode);
    }

    #[test]
    fn fresh_codex_dir_without_config_toml_is_still_codex() {
        // VS Code の拡張だけで使い、設定を何も変えていないと config.toml がないことがある
        let home = tempfile::tempdir().unwrap();
        let logged_in = home.path().join("work-codex");
        std::fs::create_dir(&logged_in).unwrap();
        std::fs::write(logged_in.join("auth.json"), "{}").unwrap();
        assert_eq!(agent_of_config_dir(&logged_in), Agent::Codex);
        let default_dir = home.path().join(".codex");
        std::fs::create_dir(&default_dir).unwrap();
        assert_eq!(
            agent_of_config_dir(&default_dir),
            Agent::Codex,
            "名前が .codex なら中身がなくても Codex"
        );
        let claude = home.path().join(".claude");
        std::fs::create_dir(&claude).unwrap();
        assert_eq!(agent_of_config_dir(&claude), Agent::ClaudeCode);
    }
}
