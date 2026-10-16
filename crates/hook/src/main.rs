use std::io::Read;
use std::path::Path;

use agent_loaf_core::agent::Agent;
use agent_loaf_core::event::TerminalInfo;
use agent_loaf_core::git::git_branch;
use agent_loaf_core::hook_input::{build_event, is_conversation, HookContext};
use agent_loaf_core::paths;
use agent_loaf_core::procinfo::{ancestor_rows, process_argv};
use agent_loaf_core::ps::find_agent_ancestor;
use agent_loaf_core::spool::{spool_is_full, write_event, MAX_SPOOL_FILES};

fn main() {
    // Claude Code の動作を妨げないため、パニックも含めて何も出力せず常に 0 で終わる
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(run);
    std::process::exit(0);
}

fn run() -> Option<()> {
    let ts_ms = paths::now_ms();
    let mut raw = Vec::new();
    std::io::stdin().read_to_end(&mut raw).ok()?;
    let home = paths::monitor_home()?;
    if std::env::var_os("AGENT_LOAF_DEBUG_RAW").is_some() {
        save_raw(&home, ts_ms, &raw);
    }
    let input: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    let is_session_start = input.get("hook_event_name").and_then(|v| v.as_str()) == Some("SessionStart");

    // hook はツールを使うたびに動くので、全プロセスではなく自分の親だけをたどる
    let rows = ancestor_rows(std::process::id());
    let found = find_agent_ancestor(&rows, std::process::id());
    let agent = found.map(|(_, agent)| agent).unwrap_or_default();
    let process = found.map(|(row, _)| row);
    let entrypoint = std::env::var("CLAUDE_CODE_ENTRYPOINT").ok();
    // 引数は 1 つずつ読み直す（ProcRow の args は空白でつないだ形）。読めなければ、つないだ形を空白で分けて代わりにする
    let argv =
        process.map(|p| process_argv(p.pid).unwrap_or_else(|| p.args.split_whitespace().map(String::from).collect()));
    if !is_conversation(agent, argv.as_deref(), entrypoint.as_deref()) {
        return None;
    }
    let terminal =
        is_session_start.then(|| terminal_info(agent, process.and_then(|p| p.tty.clone()), process.map(|p| p.ppid)));
    let branch = if is_session_start {
        input
            .get("cwd")
            .and_then(|v| v.as_str())
            .and_then(|cwd| git_branch(Path::new(cwd)))
    } else {
        None
    };

    let ctx = HookContext {
        ts_ms,
        claude_pid: process.map(|p| p.pid),
        config_dir: match agent {
            Agent::ClaudeCode => paths::default_config_dir()?,
            Agent::Codex => paths::default_codex_dir()?,
        },
        terminal,
        git_branch: branch,
        agent,
    };
    let event = build_event(&input, ctx)?;
    paths::prepare_private_dir(&home).ok()?;
    let events_dir = paths::events_dir(&home);
    paths::prepare_private_dir(&events_dir).ok()?;
    if spool_is_full(&events_dir, MAX_SPOOL_FILES) {
        return None;
    }
    write_event(&events_dir, &event).ok()?;
    Some(())
}

fn terminal_info(agent: Agent, tty: Option<String>, host_pid: Option<u32>) -> TerminalInfo {
    let var = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    TerminalInfo {
        term_program: var("TERM_PROGRAM"),
        iterm_session_id: var("ITERM_SESSION_ID"),
        tmux: var("TMUX"),
        tmux_pane: var("TMUX_PANE"),
        // Codex は Claude Code の会話の中から動かすと、その値を引き継ぐ。移動のときにエディターと取り違えないよう使わない
        entrypoint: if agent == Agent::ClaudeCode {
            var("CLAUDE_CODE_ENTRYPOINT")
        } else {
            None
        },
        tty,
        host_pid,
    }
}

fn save_raw(home: &Path, ts_ms: u64, raw: &[u8]) {
    let dir = paths::raw_dir(home);
    if paths::prepare_private_dir(home).is_err() || paths::prepare_private_dir(&dir).is_err() {
        return;
    }
    let _ = paths::write_private(&dir.join(format!("{ts_ms:013}-{}.json", std::process::id())), raw);
}
