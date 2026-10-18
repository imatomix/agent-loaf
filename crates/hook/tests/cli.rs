use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn run_hook(home: &Path, stdin: &[u8]) -> (Output, Duration) {
    run_hook_with(home, stdin, &[])
}

// hook は親のプロセスをたどって Claude Code か Codex かを決めるので、そのまま動かすと、テストを動かした側（Codex の中など）に左右される。
// 名前が claude のシェルの子として動かし、いちばん近い親を Claude Code にそろえる
fn fake_claude() -> std::path::PathBuf {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/bin/sh", dir.path().join("claude")).unwrap();
        dir
    });
    dir.path().join("claude")
}

fn run_hook_with(home: &Path, stdin: &[u8], envs: &[(&str, &str)]) -> (Output, Duration) {
    let started = Instant::now();
    // 最後のコマンドだと、シェルが hook に置き換わって親でなくなるので、後ろに exit を置く
    let mut child = Command::new(fake_claude())
        .args(["-c", "\"$0\"; exit $?", env!("CARGO_BIN_EXE_agent-loaf-hook")])
        .env("AGENT_LOAF_HOME", home)
        .env("CLAUDE_CONFIG_DIR", "/tmp/test-claude-config")
        .env_remove("AGENT_LOAF_DEBUG_RAW")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .envs(envs.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let output = child.wait_with_output().unwrap();
    (output, started.elapsed())
}

fn event_files(home: &Path) -> Vec<std::path::PathBuf> {
    match fs::read_dir(home.join("events")) {
        Ok(entries) => entries.map(|e| e.unwrap().path()).collect(),
        Err(_) => Vec::new(),
    }
}

fn assert_silent_success(output: &Output) {
    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stdout.is_empty(),
        "stdout: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.is_empty(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn writes_one_event_file_silently() {
    let home = tempfile::tempdir().unwrap();
    let input = br#"{"hook_event_name":"PreToolUse","session_id":"s1","cwd":"/w/my-api","tool_name":"Bash","tool_input":{"command":"npm test"}}"#;
    let (output, _) = run_hook(home.path(), input);
    assert_silent_success(&output);
    let files = event_files(home.path());
    assert_eq!(files.len(), 1);
    let event: serde_json::Value = serde_json::from_slice(&fs::read(&files[0]).unwrap()).unwrap();
    assert_eq!(event["event"], "PreToolUse");
    assert_eq!(event["config_dir"], "/tmp/test-claude-config");
    assert_eq!(event["detail"]["summary"], "npm test");
}

#[cfg(unix)]
#[test]
fn creates_private_directories() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let home_dir = home.path().join("am");
    run_hook(&home_dir, br#"{"hook_event_name":"Stop","session_id":"s1"}"#);
    let mode = fs::metadata(&home_dir).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
}

#[test]
fn huge_input_is_fast_and_not_stored() {
    let home = tempfile::tempdir().unwrap();
    let content = "SECRET-CONTENT ".repeat(400_000);
    let input = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "session_id": "s1",
        "tool_name": "Write",
        "tool_input": { "file_path": "/w/.env", "content": content }
    });
    let (output, elapsed) = run_hook(home.path(), input.to_string().as_bytes());
    assert_silent_success(&output);
    assert!(elapsed < Duration::from_millis(1500), "elapsed {elapsed:?}");
    let files = event_files(home.path());
    let stored = fs::read_to_string(&files[0]).unwrap();
    assert!(stored.len() < 2048);
    assert!(!stored.contains("SECRET-CONTENT"));
}

#[test]
fn garbage_and_empty_input_exit_zero_without_files() {
    for input in [
        &b"not json"[..],
        &b""[..],
        &[0xff, 0xfe, 0x00][..],
        &br#"{"no":"fields"}"#[..],
    ] {
        let home = tempfile::tempdir().unwrap();
        let (output, _) = run_hook(home.path(), input);
        assert_silent_success(&output);
        assert!(event_files(home.path()).is_empty());
    }
}

#[test]
fn unwritable_home_still_exits_zero() {
    let (output, _) = run_hook(
        Path::new("/dev/null/agent-loaf"),
        br#"{"hook_event_name":"Stop","session_id":"s1"}"#,
    );
    assert_silent_success(&output);
}

const PRE_TOOL_USE: &[u8] = br#"{"hook_event_name":"PreToolUse","session_id":"s1","cwd":"/w/my-api","tool_name":"Bash","tool_input":{"command":"npm test"}}"#;

#[test]
fn writes_nothing_for_agent_sdk_runs() {
    let home = tempfile::tempdir().unwrap();
    let (output, _) = run_hook_with(home.path(), PRE_TOOL_USE, &[("CLAUDE_CODE_ENTRYPOINT", "sdk-ts")]);
    assert_silent_success(&output);
    assert!(event_files(home.path()).is_empty());
}

#[test]
fn writes_nothing_when_spool_is_full() {
    let home = tempfile::tempdir().unwrap();
    let events = home.path().join("events");
    fs::create_dir_all(&events).unwrap();
    for i in 0..agent_loaf_core::session::spool::MAX_SPOOL_FILES {
        fs::write(events.join(format!("{i:013}-1-1.json")), b"{}").unwrap();
    }
    let (output, _) = run_hook(home.path(), PRE_TOOL_USE);
    assert_silent_success(&output);
    assert_eq!(
        event_files(home.path()).len(),
        agent_loaf_core::session::spool::MAX_SPOOL_FILES
    );
}

#[test]
fn codex_home_is_ignored_for_claude_sessions() {
    let home = tempfile::tempdir().unwrap();
    let (output, _) = run_hook_with(home.path(), PRE_TOOL_USE, &[("CODEX_HOME", "/tmp/not-used")]);
    assert_silent_success(&output);
    let files = event_files(home.path());
    let event: serde_json::Value = serde_json::from_slice(&fs::read(&files[0]).unwrap()).unwrap();
    assert_ne!(event["config_dir"], "/tmp/not-used");
}
