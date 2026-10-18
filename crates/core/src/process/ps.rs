use std::collections::HashMap;

use crate::base::agent::Agent;
#[cfg(not(target_os = "macos"))]
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub struct ProcRow {
    pub pid: u32,
    pub ppid: u32,
    pub tty: Option<String>,
    pub args: String,
}

#[cfg(not(target_os = "macos"))]
const PS_ARGS: [&str; 2] = ["-axo", "pid=,ppid=,tty=,args="];
const NON_SESSION_SUBCOMMANDS: [&str; 11] = [
    "plugin",
    "plugins",
    "mcp",
    "config",
    "update",
    "doctor",
    "install",
    // Claude Code が裏で動かす常駐プロセスと、エージェント一覧の画面
    "daemon",
    "bg-pty-host",
    "bg-spare",
    "agents",
];
const MAX_ANCESTOR_DEPTH: usize = 16;

pub fn parse_ps(output: &str) -> Vec<ProcRow> {
    output.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<ProcRow> {
    let mut parts = line.split_whitespace();
    let pid = parts.next()?.parse().ok()?;
    let ppid = parts.next()?.parse().ok()?;
    let tty = parts.next()?;
    let args = parts.collect::<Vec<_>>().join(" ");
    if args.is_empty() {
        return None;
    }
    Some(ProcRow {
        pid,
        ppid,
        tty: (tty != "??").then(|| tty.to_string()),
        args,
    })
}

pub fn run_ps() -> Option<Vec<ProcRow>> {
    #[cfg(target_os = "macos")]
    {
        crate::process::procinfo::list_processes()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let output = Command::new("ps").args(PS_ARGS).output().ok()?;
        if !output.status.success() {
            return None;
        }
        Some(parse_ps(&String::from_utf8_lossy(&output.stdout)))
    }
}

pub fn tracked_process_rows(pids: &[u32]) -> Option<Vec<ProcRow>> {
    #[cfg(target_os = "macos")]
    {
        Some(crate::process::procinfo::process_rows(pids))
    }
    #[cfg(not(target_os = "macos"))]
    {
        run_ps().map(|rows| rows.into_iter().filter(|row| pids.contains(&row.pid)).collect())
    }
}

fn claude_args(args: &str) -> Option<Vec<&str>> {
    let tokens: Vec<&str> = args.split_whitespace().collect();
    let program = *tokens.first()?;
    let name = program.rsplit('/').next().unwrap_or(program);
    if name == "claude" {
        return Some(tokens[1..].to_vec());
    }
    if name.starts_with("node") {
        let entry = tokens.iter().position(|t| t.contains("@anthropic-ai/claude-code"))?;
        return Some(tokens[entry + 1..].to_vec());
    }
    None
}

pub fn is_claude_command(args: &str) -> bool {
    claude_args(args).is_some()
}

fn program_name(args: &str) -> Option<&str> {
    let program = args.split_whitespace().next()?;
    Some(program.rsplit('/').next().unwrap_or(program))
}

pub fn agent_of(args: &str) -> Option<Agent> {
    if claude_args(args).is_some() {
        return Some(Agent::ClaudeCode);
    }
    (program_name(args)? == "codex").then_some(Agent::Codex)
}

// 生きているかの確認で数えるプロセス。pid が使い回されたときに、関係のないプロセスを生きているとみなさない
pub fn is_agent_command(args: &str) -> bool {
    agent_of(args).is_some()
}

pub fn find_agent_ancestor(rows: &[ProcRow], start_pid: u32) -> Option<(&ProcRow, Agent)> {
    let row = find_claude_ancestor(rows, start_pid)?;
    Some((row, agent_of(&row.args)?))
}

pub fn is_interactive_session(args: &str) -> bool {
    let Some(rest) = claude_args(args) else { return false };
    if rest.iter().any(|t| *t == "-p" || *t == "--print") {
        return false;
    }
    match rest.iter().find(|t| !t.starts_with('-')) {
        Some(first) => !NON_SESSION_SUBCOMMANDS.contains(first),
        None => true,
    }
}

pub fn find_claude_ancestor(rows: &[ProcRow], start_pid: u32) -> Option<&ProcRow> {
    let by_pid: HashMap<u32, &ProcRow> = rows.iter().map(|r| (r.pid, r)).collect();
    let mut current = by_pid.get(&start_pid)?.ppid;
    for _ in 0..MAX_ANCESTOR_DEPTH {
        let row = by_pid.get(&current)?;
        if is_agent_command(&row.args) {
            return Some(row);
        }
        if row.ppid == current || row.ppid == 0 {
            return None;
        }
        current = row.ppid;
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchOrigin {
    VsCode,
    Cursor,
    Terminal,
}

impl LaunchOrigin {
    // エディターの名前は翻訳しない。ターミナルだけは言語ごとの言い方がある
    pub fn editor_name(self) -> Option<&'static str> {
        match self {
            LaunchOrigin::VsCode => Some("VS Code"),
            LaunchOrigin::Cursor => Some("Cursor"),
            LaunchOrigin::Terminal => None,
        }
    }
}

pub fn launch_origin(args: &str) -> LaunchOrigin {
    let program = args.split_whitespace().next().unwrap_or_default();
    if program.contains("/.vscode/extensions/") {
        LaunchOrigin::VsCode
    } else if program.contains("/.cursor/extensions/") {
        LaunchOrigin::Cursor
    } else {
        LaunchOrigin::Terminal
    }
}

pub fn process_cwd(pid: u32) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        crate::process::procinfo::working_directory(pid)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let output = Command::new("lsof")
            .args(["-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|line| line.strip_prefix('n').map(str::to_string))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pid: u32, ppid: u32, args: &str) -> ProcRow {
        ProcRow {
            pid,
            ppid,
            tty: None,
            args: args.into(),
        }
    }

    #[test]
    fn parses_ps_output() {
        let output = "  101     1 ttys003  /Users/u/.local/bin/claude --model opus\n  202   101 ??       /bin/sh -c hook\nbroken line\n";
        let rows = parse_ps(output);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0],
            ProcRow {
                pid: 101,
                ppid: 1,
                tty: Some("ttys003".into()),
                args: "/Users/u/.local/bin/claude --model opus".into()
            }
        );
        assert_eq!(rows[1].tty, None);
    }

    #[test]
    fn recognizes_claude_commands() {
        assert!(is_claude_command("/Users/u/.local/bin/claude"));
        assert!(is_claude_command("claude --resume"));
        assert!(is_claude_command(
            "node /opt/lib/node_modules/@anthropic-ai/claude-code/cli.js"
        ));
        assert!(!is_claude_command("/bin/zsh"));
        assert!(!is_claude_command("node server.js"));
        assert!(!is_claude_command("/Applications/Claude.app/Contents/MacOS/Claude"));
    }

    #[test]
    fn excludes_non_interactive_runs() {
        assert!(is_interactive_session("claude"));
        assert!(is_interactive_session("claude --model opus"));
        assert!(is_interactive_session("claude fix the bug"));
        assert!(!is_interactive_session("claude -p hello"));
        assert!(!is_interactive_session("claude --print hello"));
        assert!(!is_interactive_session("claude plugin install x"));
        assert!(!is_interactive_session(
            "node /x/@anthropic-ai/claude-code/cli.js mcp list"
        ));
        assert!(!is_interactive_session("/bin/zsh"));
    }

    #[test]
    fn recognizes_real_world_processes() {
        let vscode = "/Users/u/.vscode/extensions/anthropic.claude-code-2.1.289-darwin-arm64/resources/native-binary/claude --output-format stream-json --verbose --input-format stream-json --max-thinking-tokens 0";
        assert!(is_interactive_session(vscode));
        assert!(!is_interactive_session(
            "/Users/u/.local/bin/claude daemon run --origin transient --spawned-by {}"
        ));
        assert!(!is_interactive_session("claude bg-pty-host --bg-pty-host /tmp/cc/x.pty.sock 200 50 -- /Users/u/.local/share/claude/versions/2.1.286"));
        assert!(!is_interactive_session(
            "claude bg-spare --bg-spare /tmp/cc/x.claim.sock"
        ));
        assert!(!is_interactive_session("claude agents"));
    }

    #[test]
    fn tells_launch_origin_from_executable_path() {
        assert_eq!(launch_origin("/Users/u/.vscode/extensions/anthropic.claude-code-2.1.289-darwin-arm64/resources/native-binary/claude --output-format stream-json"), LaunchOrigin::VsCode);
        assert_eq!(
            launch_origin("/Users/u/.cursor/extensions/anthropic.claude-code-2.1.289/resources/native-binary/claude"),
            LaunchOrigin::Cursor
        );
        assert_eq!(launch_origin("/Users/u/.local/bin/claude"), LaunchOrigin::Terminal);
    }

    #[test]
    fn reads_cwd_of_own_process() {
        let cwd = std::env::current_dir().unwrap();
        let found = process_cwd(std::process::id()).expect("自分自身の作業ディレクトリは読めること");
        assert_eq!(
            std::fs::canonicalize(found).unwrap(),
            std::fs::canonicalize(cwd).unwrap()
        );
        assert_eq!(process_cwd(u32::MAX), None);
    }

    #[test]
    fn finds_claude_ancestor_through_shells() {
        let rows = vec![
            row(1, 0, "/sbin/launchd"),
            row(100, 1, "/Users/u/.local/bin/claude"),
            row(200, 100, "/bin/sh -c hook"),
            row(300, 200, "/Users/u/.agent-loaf/bin/agent-loaf-hook"),
        ];
        assert_eq!(find_claude_ancestor(&rows, 300).map(|r| r.pid), Some(100));
    }

    #[test]
    fn returns_none_without_claude_ancestor() {
        let rows = vec![row(1, 0, "/sbin/launchd"), row(300, 1, "agent-loaf-hook")];
        assert_eq!(find_claude_ancestor(&rows, 300), None);
        assert_eq!(find_claude_ancestor(&rows, 999), None);
    }

    const CODEX_IN_VSCODE: &str = "/Users/u/.vscode/extensions/openai.chatgpt-26.1007.21434-darwin-arm64/bin/macos-aarch64/codex -c features.code_mode_host=true app-server --analytics-default-enabled";

    #[test]
    fn tells_codex_from_claude_and_other_programs() {
        use crate::base::agent::Agent;
        assert_eq!(agent_of("/Users/u/.local/bin/claude"), Some(Agent::ClaudeCode));
        assert_eq!(agent_of(CODEX_IN_VSCODE), Some(Agent::Codex));
        assert_eq!(agent_of("/opt/homebrew/bin/codex exec hi"), Some(Agent::Codex));
        assert_eq!(agent_of("/Users/u/.vscode/extensions/openai.chatgpt-26.1007.21434-darwin-arm64/bin/macos-aarch64/codex-code-mode-host"), None);
        assert_eq!(agent_of("/bin/zsh"), None);
        assert!(is_agent_command(CODEX_IN_VSCODE));
        assert!(!is_interactive_session(CODEX_IN_VSCODE), "Codex は状態不明の行にしない");
    }

    #[test]
    fn finds_the_nearest_agent_ancestor() {
        use crate::base::agent::Agent;
        // Claude Code の会話の中で codex を動かしたときは、近い方（codex）を取る
        let rows = vec![
            row(1, 0, "/sbin/launchd"),
            row(100, 1, "/Users/u/.local/bin/claude"),
            row(200, 100, "/bin/zsh"),
            row(300, 200, "/opt/homebrew/bin/codex exec hi"),
            row(400, 300, "/Users/u/.agent-loaf/bin/agent-loaf-hook"),
        ];
        let (found, agent) = find_agent_ancestor(&rows, 400).unwrap();
        assert_eq!((found.pid, agent), (300, Agent::Codex));
        assert_eq!(
            find_claude_ancestor(&rows, 400).map(|r| r.pid),
            Some(300),
            "名前は今のまま、codex も見つける"
        );
    }
}
