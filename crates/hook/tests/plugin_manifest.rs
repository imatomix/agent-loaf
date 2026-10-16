use std::fs;

const REQUIRED_EVENTS: [&str; 17] = [
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
    // Codex の知らせ（Claude Code は Interrupt を読み飛ばす）
    "Interrupt",
    "PostCompact",
];

fn load(path: &str) -> serde_json::Value {
    let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&fs::read_to_string(full).unwrap()).unwrap()
}

#[test]
fn registers_every_required_event_with_safe_command() {
    let hooks = load("plugin/hooks/hooks.json");
    for event in REQUIRED_EVENTS {
        let command = hooks["hooks"][event][0]["hooks"][0]["command"]
            .as_str()
            .unwrap_or_else(|| panic!("{event} が登録されていない"));
        assert!(command.contains("agent-loaf-hook"), "{event}");
        assert!(
            command.trim_end().ends_with("exit 0"),
            "{event} は必ず exit 0 で終わること"
        );
    }
}

#[test]
fn marketplace_points_to_plugin_directory() {
    let marketplace = load(".claude-plugin/marketplace.json");
    assert_eq!(marketplace["name"], "agent-loaf");
    assert_eq!(marketplace["plugins"][0]["name"], "agent-loaf");
    assert_eq!(marketplace["plugins"][0]["source"], "./plugin");
    let plugin = load("plugin/.claude-plugin/plugin.json");
    assert_eq!(plugin["name"], "agent-loaf");
}

// Codex は SessionEnd と Interrupt の待ち時間を 3 秒までしか受け付けない
#[test]
fn short_lived_events_wait_at_most_three_seconds() {
    let hooks = load("plugin/hooks/hooks.json");
    for event in ["SessionEnd", "Interrupt"] {
        let timeout = hooks["hooks"][event][0]["hooks"][0]["timeout"]
            .as_u64()
            .unwrap_or_else(|| panic!("{event} に timeout がない"));
        assert!(timeout <= 3, "{event} の timeout は 3 以下にすること（{timeout}）");
    }
}
