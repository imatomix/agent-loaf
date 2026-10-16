use serde_json::Value;

use crate::agent::Agent;

use crate::event::{EventDetail, MonitorEvent, TerminalInfo, SCHEMA_VERSION, SUMMARY_MAX_CHARS};

pub struct HookContext {
    pub ts_ms: u64,
    pub claude_pid: Option<u32>,
    pub config_dir: String,
    pub terminal: Option<TerminalInfo>,
    pub git_branch: Option<String>,
    pub agent: Agent,
}

pub fn build_event(input: &Value, ctx: HookContext) -> Option<MonitorEvent> {
    let event = str_field(input, "hook_event_name")?;
    let session_id = str_field(input, "session_id")?;
    let tool_name = str_field(input, "tool_name");
    // Codex は許可を求める理由の文を付けることがあり、コマンドよりも何を許すのかが分かりやすい
    let reason = (ctx.agent == Agent::Codex && event == "PermissionRequest")
        .then(|| {
            input
                .pointer("/tool_input/description")
                .and_then(|v| v.as_str())
                .map(truncate_summary)
        })
        .flatten();
    let summary = reason.or_else(|| match (&tool_name, input.get("tool_input")) {
        (Some(name), Some(tool_input)) => summarize_tool(name, tool_input),
        _ => None,
    });
    let is_session_start = event == "SessionStart";
    Some(MonitorEvent {
        schema: SCHEMA_VERSION,
        ts_ms: ctx.ts_ms,
        event,
        session_id,
        agent_id: str_field(input, "agent_id"),
        claude_pid: ctx.claude_pid,
        config_dir: ctx.config_dir,
        cwd: str_field(input, "cwd").unwrap_or_default(),
        transcript_path: str_field(input, "transcript_path"),
        detail: EventDetail {
            tool_name,
            summary,
            notification_type: str_field(input, "notification_type"),
            error_type: str_field(input, "error_type"),
            source: str_field(input, "source"),
        },
        terminal: if is_session_start { ctx.terminal } else { None },
        git_branch: if is_session_start { ctx.git_branch } else { None },
        agent: ctx.agent,
    })
}

pub fn summarize_tool(tool_name: &str, tool_input: &Value) -> Option<String> {
    let key = match tool_name {
        "Bash" => "command",
        "Edit" | "MultiEdit" | "Write" | "Read" => "file_path",
        "NotebookEdit" => "notebook_path",
        "Grep" | "Glob" => "pattern",
        "WebFetch" => "url",
        "WebSearch" => "query",
        "Task" | "Agent" => "description",
        _ => return None,
    };
    tool_input.get(key)?.as_str().map(truncate_summary)
}

pub fn truncate_summary(text: &str) -> String {
    // 巨大なコマンド文字列でも処理時間を一定に抑えるため、先に必要な分だけ取り出す
    let head: String = text.chars().take(SUMMARY_MAX_CHARS * 4).collect();
    let single_line = head.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.chars().count() <= SUMMARY_MAX_CHARS {
        return single_line;
    }
    let mut cut: String = single_line.chars().take(SUMMARY_MAX_CHARS - 1).collect();
    cut.push('…');
    cut
}

fn str_field(input: &Value, key: &str) -> Option<String> {
    input.get(key)?.as_str().map(str::to_string)
}

// claude -p・Agent SDK・codex exec・codex review からの実行は、人が向き合う会話ではないので一覧にも通知にも出さない。
// entrypoint は外側の Claude から引き継がれる（VS Code の会話の中で動かした claude -p も claude-vscode になる）ので、引数も見る。
// 引数は 1 つずつ（argv）で見る。空白でつなぐと「review the auth changes」のような依頼の文をサブコマンドと取り違える
pub fn is_conversation(agent: Agent, argv: Option<&[String]>, entrypoint: Option<&str>) -> bool {
    let args = argv.map(|argv| argv.get(1..).unwrap_or_default()).unwrap_or_default();
    match agent {
        Agent::ClaudeCode => {
            !entrypoint.is_some_and(|e| e.starts_with("sdk-")) && !args.iter().any(|a| a == "-p" || a == "--print")
        }
        // Codex の entrypoint の環境変数はないので、Claude Code から引き継いだものは見ない
        Agent::Codex => !matches!(codex_subcommand(args), Some("exec" | "e" | "review")),
    }
}

// 値を取るオプション（codex --help より）。値をサブコマンドと取り違えないよう飛ばす
const CODEX_VALUE_OPTIONS: [&str; 19] = [
    "-c",
    "--config",
    "--enable",
    "--disable",
    "--remote",
    "--remote-auth-token-env",
    "-i",
    "--image",
    "-m",
    "--model",
    "--local-provider",
    "-p",
    "--profile",
    "-s",
    "--sandbox",
    "-C",
    "--cd",
    "--add-dir",
    "-a",
];

fn codex_subcommand(args: &[String]) -> Option<&str> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if CODEX_VALUE_OPTIONS.contains(&arg.as_str()) || arg == "--ask-for-approval" {
            iter.next();
        } else if !arg.starts_with('-') {
            return Some(arg.as_str());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;

    fn argv(joined: &str) -> Vec<String> {
        joined.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn tells_conversations_from_scripted_runs() {
        let vscode = "/Users/u/.vscode/extensions/anthropic.claude-code-2.1.296-darwin-arm64/resources/native-binary/claude --output-format stream-json --input-format stream-json";
        assert!(is_conversation(
            Agent::ClaudeCode,
            Some(&argv("/Users/u/.local/bin/claude")),
            Some("cli")
        ));
        assert!(is_conversation(
            Agent::ClaudeCode,
            Some(&argv(vscode)),
            Some("claude-vscode")
        ));
        assert!(!is_conversation(
            Agent::ClaudeCode,
            Some(&argv("/Users/u/.local/bin/claude -p hello")),
            Some("cli")
        ));
        assert!(!is_conversation(
            Agent::ClaudeCode,
            Some(&argv("/Users/u/.local/bin/claude --print hello")),
            None
        ));
        assert!(!is_conversation(Agent::ClaudeCode, Some(&argv(vscode)), Some("sdk-ts")));
        assert!(!is_conversation(Agent::ClaudeCode, None, Some("sdk-cli")));
        assert!(
            is_conversation(Agent::ClaudeCode, None, None),
            "claude が見つからないときは見分けられないので書く"
        );
        assert!(
            is_conversation(
                Agent::ClaudeCode,
                Some(&argv("/Users/u/.local/bin/claude update the changelog")),
                Some("cli")
            ),
            "依頼の文の最初の単語がサブコマンドと同じでも会話"
        );
        assert!(is_conversation(
            Agent::ClaudeCode,
            Some(&argv("/Users/u/.local/bin/claude --append-system-prompt config rules")),
            Some("cli")
        ));
        assert!(
            !is_conversation(
                Agent::ClaudeCode,
                Some(&argv("/Users/u/.local/bin/claude --model opus -p hi")),
                Some("claude-vscode")
            ),
            "外側の entrypoint を引き継いでいても -p なら除く"
        );
    }
    use serde_json::json;

    fn ctx() -> HookContext {
        HookContext {
            ts_ms: 1_000,
            claude_pid: Some(42),
            config_dir: "/home/u/.claude".into(),
            terminal: Some(TerminalInfo {
                term_program: Some("iTerm.app".into()),
                ..Default::default()
            }),
            git_branch: Some("main".into()),
            agent: Agent::ClaudeCode,
        }
    }

    #[test]
    fn builds_pre_tool_use_with_bash_summary() {
        let input = json!({
            "hook_event_name": "PreToolUse",
            "session_id": "s1",
            "cwd": "/work/my-api",
            "transcript_path": "/t/s1.jsonl",
            "tool_name": "Bash",
            "tool_input": { "command": "npm test" }
        });
        let ev = build_event(&input, ctx()).unwrap();
        assert_eq!(ev.schema, SCHEMA_VERSION);
        assert_eq!(ev.ts_ms, 1_000);
        assert_eq!(ev.event, "PreToolUse");
        assert_eq!(ev.session_id, "s1");
        assert_eq!(ev.claude_pid, Some(42));
        assert_eq!(ev.cwd, "/work/my-api");
        assert_eq!(ev.transcript_path.as_deref(), Some("/t/s1.jsonl"));
        assert_eq!(ev.detail.tool_name.as_deref(), Some("Bash"));
        assert_eq!(ev.detail.summary.as_deref(), Some("npm test"));
    }

    #[test]
    fn terminal_and_branch_are_kept_only_for_session_start() {
        let start = json!({ "hook_event_name": "SessionStart", "session_id": "s1" });
        let stop = json!({ "hook_event_name": "Stop", "session_id": "s1" });
        let start_ev = build_event(&start, ctx()).unwrap();
        let stop_ev = build_event(&stop, ctx()).unwrap();
        assert!(start_ev.terminal.is_some());
        assert_eq!(start_ev.git_branch.as_deref(), Some("main"));
        assert!(stop_ev.terminal.is_none());
        assert!(stop_ev.git_branch.is_none());
    }

    #[test]
    fn copies_session_start_source() {
        let start = json!({ "hook_event_name": "SessionStart", "session_id": "s1", "source": "resume" });
        assert_eq!(
            build_event(&start, ctx()).unwrap().detail.source.as_deref(),
            Some("resume")
        );
    }

    #[test]
    fn copies_notification_and_error_types() {
        let notification =
            json!({ "hook_event_name": "Notification", "session_id": "s1", "notification_type": "permission_prompt" });
        let failure = json!({ "hook_event_name": "StopFailure", "session_id": "s1", "error_type": "rate_limit" });
        assert_eq!(
            build_event(&notification, ctx())
                .unwrap()
                .detail
                .notification_type
                .as_deref(),
            Some("permission_prompt")
        );
        assert_eq!(
            build_event(&failure, ctx()).unwrap().detail.error_type.as_deref(),
            Some("rate_limit")
        );
    }

    #[test]
    fn rejects_input_without_event_name_or_session() {
        assert!(build_event(&json!({ "session_id": "s1" }), ctx()).is_none());
        assert!(build_event(&json!({ "hook_event_name": "Stop" }), ctx()).is_none());
    }

    #[test]
    fn never_keeps_raw_tool_input() {
        let secret = "SECRET-CONTENT ".repeat(1000);
        let input = json!({
            "hook_event_name": "PreToolUse",
            "session_id": "s1",
            "tool_name": "Write",
            "tool_input": { "file_path": "/work/.env", "content": secret }
        });
        let ev = build_event(&input, ctx()).unwrap();
        let serialized = serde_json::to_string(&ev).unwrap();
        assert!(!serialized.contains("SECRET-CONTENT"));
        assert_eq!(ev.detail.summary.as_deref(), Some("/work/.env"));
    }

    #[test]
    fn summarizes_known_tools() {
        assert_eq!(
            summarize_tool("Edit", &json!({ "file_path": "src/a.rs" })).as_deref(),
            Some("src/a.rs")
        );
        assert_eq!(
            summarize_tool("NotebookEdit", &json!({ "notebook_path": "n.ipynb" })).as_deref(),
            Some("n.ipynb")
        );
        assert_eq!(
            summarize_tool("Grep", &json!({ "pattern": "TODO" })).as_deref(),
            Some("TODO")
        );
        assert_eq!(
            summarize_tool("WebFetch", &json!({ "url": "https://x.dev" })).as_deref(),
            Some("https://x.dev")
        );
        assert_eq!(
            summarize_tool("Agent", &json!({ "description": "調査" })).as_deref(),
            Some("調査")
        );
        assert_eq!(summarize_tool("mcp__x__y", &json!({ "a": 1 })), None);
    }

    #[test]
    fn truncates_to_limit_and_flattens_whitespace() {
        assert_eq!(truncate_summary("npm\n  test"), "npm test");
        let long = "あ".repeat(300);
        let cut = truncate_summary(&long);
        assert_eq!(cut.chars().count(), SUMMARY_MAX_CHARS);
        assert!(cut.ends_with('…'));
    }

    #[test]
    fn tells_codex_conversations_from_scripted_runs() {
        let app_server = "/Users/u/.vscode/extensions/openai.chatgpt-26.1007.21434-darwin-arm64/bin/macos-aarch64/codex -c features.code_mode_host=true app-server";
        assert!(is_conversation(Agent::Codex, Some(&argv(app_server)), None));
        assert!(is_conversation(
            Agent::Codex,
            Some(&argv("/opt/homebrew/bin/codex")),
            None
        ));
        assert!(!is_conversation(
            Agent::Codex,
            Some(&argv("/opt/homebrew/bin/codex exec hi")),
            None
        ));
        assert!(!is_conversation(
            Agent::Codex,
            Some(&argv("/opt/homebrew/bin/codex review")),
            None
        ));
        // Claude Code の会話の中で動かすと、Claude Code の entrypoint を引き継ぐ。Codex では見ない
        assert!(is_conversation(Agent::Codex, Some(&argv(app_server)), Some("sdk-cli")));
    }

    #[test]
    fn codex_permission_request_uses_its_reason() {
        let input = serde_json::json!({
            "hook_event_name": "PermissionRequest", "session_id": "s1", "tool_name": "Bash",
            "tool_input": { "command": "curl -sI https://example.com", "description": "https://example.com へのヘッダー取得を許可しますか？" }
        });
        let mut context = ctx();
        context.agent = Agent::Codex;
        let event = build_event(&input, context).unwrap();
        assert_eq!(event.agent, Agent::Codex);
        assert_eq!(
            event.detail.summary.as_deref(),
            Some("https://example.com へのヘッダー取得を許可しますか？")
        );
        let mut no_reason = input.clone();
        no_reason["tool_input"]["description"] = serde_json::Value::Null;
        let mut context = ctx();
        context.agent = Agent::Codex;
        assert_eq!(
            build_event(&no_reason, context).unwrap().detail.summary.as_deref(),
            Some("curl -sI https://example.com"),
            "理由がなければコマンド"
        );
    }

    #[test]
    fn codex_prompts_that_start_with_subcommand_words_are_conversations() {
        let prompt = |text: &str| vec!["/opt/homebrew/bin/codex".to_string(), text.to_string()];
        assert!(
            is_conversation(Agent::Codex, Some(&prompt("review the auth changes")), None),
            "依頼の文の中の review で除かない"
        );
        assert!(is_conversation(Agent::Codex, Some(&prompt("exec this plan")), None));
        assert!(
            !is_conversation(Agent::Codex, Some(&argv("/opt/homebrew/bin/codex e hi")), None),
            "別名の e も除く"
        );
        assert!(
            !is_conversation(
                Agent::Codex,
                Some(&argv("/opt/homebrew/bin/codex -m gpt-6 -c x=1 exec hi")),
                None
            ),
            "値を取るオプションの後のサブコマンド"
        );
        assert!(
            is_conversation(Agent::Codex, Some(&argv("/opt/homebrew/bin/codex -p review")), None),
            "-p の値（プロファイルの名前）はサブコマンドではない"
        );
        assert!(is_conversation(
            Agent::Codex,
            Some(&argv("/opt/homebrew/bin/codex --no-daemon")),
            None
        ));
        let claude_prompt = vec!["/Users/u/.local/bin/claude".to_string(), "why does -p fail".to_string()];
        assert!(
            is_conversation(Agent::ClaudeCode, Some(&claude_prompt), Some("cli")),
            "依頼の文の中の -p で除かない"
        );
    }
}
