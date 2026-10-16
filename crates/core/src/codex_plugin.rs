use std::path::Path;

use serde::Deserialize;

use crate::claude_cli::CliError;
use crate::plugin_status::{Inspection, MarketplacePresence, PluginPresence, MARKETPLACE_NAME, PLUGIN_ID};

// Codex が許可の対象にするイベント。hooks.json に Claude Code にしかない名前があっても、Codex は数えない
pub const CODEX_HOOK_EVENTS: [&str; 12] = [
    "session_start",
    "session_end",
    "user_prompt_submit",
    "pre_tool_use",
    "permission_request",
    "post_tool_use",
    "stop",
    "subagent_start",
    "subagent_stop",
    "pre_compact",
    "post_compact",
    "interrupt",
];

#[derive(Deserialize)]
struct PluginList {
    #[serde(default)]
    installed: Vec<PluginEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PluginEntry {
    plugin_id: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    version: String,
}

#[derive(Deserialize)]
struct MarketplaceList {
    #[serde(default)]
    marketplaces: Vec<MarketplaceEntry>,
}

#[derive(Deserialize)]
struct MarketplaceEntry {
    name: String,
    #[serde(default)]
    root: String,
}

pub fn parse_codex_inspection(
    plugins_json: &str,
    marketplaces_json: &str,
    bundled_dir: &Path,
) -> Result<Inspection, CliError> {
    let plugins: PluginList = serde_json::from_str(plugins_json).map_err(|_| CliError::Unreadable)?;
    let marketplaces: MarketplaceList = serde_json::from_str(marketplaces_json).map_err(|_| CliError::Unreadable)?;
    let plugin = match plugins.installed.into_iter().find(|p| p.plugin_id == PLUGIN_ID) {
        None => PluginPresence::Absent,
        Some(p) if p.enabled => PluginPresence::Enabled { version: p.version },
        Some(_) => PluginPresence::Disabled,
    };
    let bundled = bundled_dir.to_string_lossy();
    let marketplace = match marketplaces
        .marketplaces
        .into_iter()
        .find(|m| m.name == MARKETPLACE_NAME)
    {
        None => MarketplacePresence::Absent,
        Some(m) if m.root.trim_end_matches('/') == bundled.trim_end_matches('/') => MarketplacePresence::Bundled,
        Some(_) => MarketplacePresence::Other,
    };
    Ok(Inspection { plugin, marketplace })
}

fn snake_case(event: &str) -> String {
    let mut out = String::new();
    for (i, c) in event.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

// TOML のライブラリを足さないため、許可の記録の見出しの行だけを読む。指紋（trusted_hash）は Codex の計算を真似ることになるので見ない
pub fn hooks_trusted(config_toml: &str, registered: &[&str]) -> bool {
    let prefix = format!("[hooks.state.\"{PLUGIN_ID}:hooks/hooks.json:");
    let listed: Vec<&str> = config_toml
        .lines()
        .filter_map(|line| line.trim().strip_prefix(prefix.as_str()))
        .filter_map(|rest| rest.split(':').next())
        .collect();
    let expected: Vec<String> = registered
        .iter()
        .map(|e| snake_case(e))
        .filter(|e| CODEX_HOOK_EVENTS.contains(&e.as_str()))
        .collect();
    !expected.is_empty() && expected.iter().all(|event| listed.contains(&event.as_str()))
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

// Codex の許可の画面は VS Code の中にないので、ターミナルで開く。リポジトリの hook を一緒に許可しないよう、空のフォルダーで開く
pub fn trust_command_script(codex: &Path, codex_home: &Path, work_dir: &Path) -> String {
    format!(
        "#!/bin/sh\ncd {}\nCODEX_HOME={} exec {} --no-daemon\n",
        quoted(work_dir),
        quoted(codex_home),
        quoted(codex)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = "/Users/u/.agent-loaf/marketplace";

    fn plugins(version: &str, enabled: bool) -> String {
        format!(
            r#"{{"installed":[{{"pluginId":"openai-templates@openai-curated-remote","version":"0.1.1","installed":true,"enabled":true}},{{"pluginId":"agent-loaf@agent-loaf","version":"{version}","installed":true,"enabled":{enabled},"source":{{"source":"local","path":"{BUNDLED}/plugin"}}}}],"available":[]}}"#
        )
    }

    fn marketplaces(root: &str) -> String {
        format!(
            r#"{{"marketplaces":[{{"name":"agent-loaf","root":"{root}","marketplaceSource":{{"sourceType":"local","source":"{root}"}}}}]}}"#
        )
    }

    #[test]
    fn reads_codex_plugin_and_marketplace_lists() {
        let i = parse_codex_inspection(&plugins("0.5.0", true), &marketplaces(BUNDLED), Path::new(BUNDLED)).unwrap();
        assert_eq!(
            i,
            Inspection {
                plugin: PluginPresence::Enabled {
                    version: "0.5.0".into()
                },
                marketplace: MarketplacePresence::Bundled
            }
        );
        let other =
            parse_codex_inspection(&plugins("0.5.0", true), &marketplaces("/elsewhere"), Path::new(BUNDLED)).unwrap();
        assert_eq!(other.marketplace, MarketplacePresence::Other);
        let disabled =
            parse_codex_inspection(&plugins("0.5.0", false), &marketplaces(BUNDLED), Path::new(BUNDLED)).unwrap();
        assert_eq!(disabled.plugin, PluginPresence::Disabled);
        let none = parse_codex_inspection(
            r#"{"installed":[],"available":[]}"#,
            r#"{"marketplaces":[]}"#,
            Path::new(BUNDLED),
        )
        .unwrap();
        assert_eq!(
            none,
            Inspection {
                plugin: PluginPresence::Absent,
                marketplace: MarketplacePresence::Absent
            }
        );
        assert_eq!(
            parse_codex_inspection("oops", "{}", Path::new(BUNDLED)),
            Err(CliError::Unreadable)
        );
    }

    const REGISTERED: [&str; 17] = [
        "SessionStart",
        "SessionEnd",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PostToolUseFailure",
        "PermissionRequest",
        "Notification",
        "Stop",
        "StopFailure",
        "SubagentStart",
        "SubagentStop",
        "PreCompact",
        "Elicitation",
        "ElicitationResult",
        "Interrupt",
        "PostCompact",
    ];

    fn trusted(events: &[&str]) -> String {
        let mut text = String::from("model = \"gpt-6-sol\"\n\n[hooks.state]\n");
        for event in events {
            text.push_str(&format!(
                "[hooks.state.\"agent-loaf@agent-loaf:hooks/hooks.json:{event}:0:0\"]\ntrusted_hash = \"sha256:00\"\n"
            ));
        }
        text
    }

    #[test]
    fn trusted_only_when_every_codex_hook_is_listed() {
        assert!(hooks_trusted(&trusted(&CODEX_HOOK_EVENTS), &REGISTERED));
        assert!(
            !hooks_trusted(&trusted(&CODEX_HOOK_EVENTS[..11]), &REGISTERED),
            "1 つ足りない"
        );
        assert!(!hooks_trusted("model = \"x\"\n", &REGISTERED), "許可の記録がない");
        assert!(!hooks_trusted("[[[broken", &REGISTERED), "壊れていても落ちない");
        let other_plugin = trusted(&CODEX_HOOK_EVENTS).replace("agent-loaf@agent-loaf", "other@other");
        assert!(
            !hooks_trusted(&other_plugin, &REGISTERED),
            "ほかのプラグインの許可は数えない"
        );
    }

    #[test]
    fn trust_script_opens_codex_in_an_empty_folder_with_quoted_paths() {
        let script = trust_command_script(
            Path::new("/Users/u/.vscode/extensions/openai.chatgpt-1/bin/codex"),
            Path::new("/Users/u/.codex"),
            Path::new("/Users/u/.agent-loaf/codex-trust"),
        );
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains("cd '/Users/u/.agent-loaf/codex-trust'"));
        assert!(script.contains(
            "CODEX_HOME='/Users/u/.codex' exec '/Users/u/.vscode/extensions/openai.chatgpt-1/bin/codex' --no-daemon"
        ));
        let tricky = trust_command_script(Path::new("/x/it's/codex"), Path::new("/c"), Path::new("/w"));
        assert!(
            tricky.contains("'/x/it'\\''s/codex'"),
            "シングルクォートを含むパスも壊さない"
        );
    }
}
