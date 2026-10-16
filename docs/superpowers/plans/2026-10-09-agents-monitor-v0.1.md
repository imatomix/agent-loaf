# agents-monitor v0.1 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** マシン上の Claude Code セッションの状態を macOS のメニューバーに常時表示し、要対応・エラー停止を OS 通知で知らせる v0.1 を作る。

**Architecture:** Claude Code プラグインの hooks が Rust 製の `agents-monitor-hook` を呼び、必要な項目だけを 1 イベント 1 ファイルで `~/.agents-monitor/events/` に書き出す。Tauri 製のメニューバーアプリがそのファイルを 1 秒ごとに読み取り、純粋関数の状態遷移でセッション一覧を更新し、`ps` でプロセスの生死を照合する。ドメインロジックはすべて `agents-monitor-core` クレートに置き、hook とアプリはそれを薄く呼ぶだけにする。

**Tech Stack:** Rust 1.99（workspace）、serde / serde_json、dirs、tempfile（テストのみ）、Tauri 2（tray-icon）、tauri-plugin-notification / positioner / autostart、Vite + 素の TypeScript、macOS の `ps` コマンド

**Spec:** `docs/superpowers/specs/2026-10-09-agents-monitor-design.md`

## Global Constraints

- hook（`agents-monitor-hook`）は何が起きても終了コード 0、標準出力・標準エラーに何も出さない
- `~/.agents-monitor/` とその下の `events/`、`raw/` は 0700 で作る
- イベントの `schema` は `1`。`summary` は最大 120 文字（超えたら 119 文字＋`…`）
- 生の `tool_input` はイベントファイルに保存しない（`summary` のみ）
- 表示の状態は `needs_action`（要対応・オレンジ）、`error`（エラー停止・赤）、`done`（完了・青）、`running`（実行中・緑）、`ended`（終了・グレー塗り）、`unknown`（状態不明・グレー枠線のみ）
- 終了したセッションは 5 分後に一覧から消す
- 通知の初期値は要対応とエラー停止。同じセッションが同じ状態のままなら再通知しない
- メニューバー: 総数（終了を除く）を常に表示し、要対応＋エラー停止の件数をバッジで重ねる。バッジ色は要対応があればオレンジ、エラー停止だけなら赤。10 件以上は `9+`
- 一覧の並び順は v0.1 では最初に見えた順（固定）。状態で並びを変えない
- 対象は macOS のみ。Windows / Linux 用のコードは書かない
- 承認済み以外のライブラリを追加しない（追加が必要になったら作業を止めて確認する）
- コミットメッセージは日本語。コメントは WHY が非自明なときだけ書く
- v0.1 の範囲外: 初回セットアップと自動インストール、設定画面の設定ディレクトリ一覧と `claude` コマンドの場所（自動インストールと一緒に v0.2）、ドラッグでの並び替え、画面移動、通知クリック時の移動、署名・公証・自動アップデート（spec 13・14 章）

## Review Focus

1. **巨大な hook 入力**（Write ツールで数 MB のファイル内容）→ hook は速く終わり、イベントファイルは小さく、内容は一切残らない（Task 4 でテスト）
2. **壊れた・空の標準入力、書き込めないホーム**→ hook は終了コード 0、出力なし、ファイルなし（Task 4 でテスト）
3. **順序の前後・遅れて届くイベント**（async hook、アプリ停止中に溜まったイベント）→ 古いイベントが新しい状態を上書きしない。起動時にまとめて読んだ古いイベントで通知を乱発しない（Task 7、Task 12 でテスト）
4. **同じ Claude プロセスで `/clear` などにより session_id が変わる**→ 古い行は終了扱いになり、同じプロセスの行が 2 つ残らない（Task 7 でテスト）
5. **一覧に表示する文字列に HTML が含まれる**（コマンドやパスは任意の文字列）→ そのまま文字として表示され、HTML として解釈されない（Task 12 で `textContent` のみ使用、手動確認項目に含める）

---

## ファイル構成

```
Cargo.toml                         ワークスペース
.claude-plugin/marketplace.json    このリポジトリをマーケットプレイスにする
plugin/.claude-plugin/plugin.json  プラグイン定義
plugin/hooks/hooks.json            hook 登録
crates/core/                       ドメインロジック（ライブラリ）
  src/lib.rs
  src/event.rs        イベントの型
  src/hook_input.rs   hook の入力 JSON → イベント
  src/ps.rs           ps の解析、Claude プロセスの判定と親たどり
  src/git.rs          ブランチ名の取得
  src/paths.rs        ディレクトリの場所、現在時刻
  src/spool.rs        イベントファイルの書き出しと読み取り
  src/state.rs        Status、Session、状態遷移の表
  src/store.rs        SessionStore（apply / reconcile / prune / summary）
  src/transcript.rs   会話ログ末尾の中断検知
  src/view.rs         画面用の SessionView
  src/notify.rs       通知文の組み立て
  src/settings.rs     設定の型
  src/persist.rs      JSON の保存と読み込み
  src/install.rs      hook プログラムの配置
  tests/fixtures/     実イベントの記録（Task 6）
  tests/fixtures_replay.rs
crates/hook/                       agents-monitor-hook（バイナリ）
  src/main.rs
  tests/cli.rs
  tests/plugin_manifest.rs
app/                               Tauri アプリ
  package.json, tsconfig.json, vite.config.ts
  index.html, settings.html
  src/list.ts, src/settings.ts, src/format.ts, src/types.ts, src/style.css
  src-tauri/Cargo.toml, build.rs, tauri.conf.json, capabilities/default.json
  src-tauri/src/main.rs, tray.rs, tray_icon.rs, monitor.rs, commands.rs
scripts/
  dev-install-hook.sh   hook をビルドして ~/.agents-monitor/bin に置く
  build-hook-sidecar.sh hook をビルドして Tauri の externalBin に置く
  collect-fixture.sh    記録したイベントをテストデータにまとめる
  make-app-icon.mjs     アプリアイコンの元画像を作る
docs/superpowers/notes/2026-10-09-hook-event-findings.md  Task 6 の調査結果
```

---

### Task 1: ワークスペースとイベントの型

**Files:**
- Create: `Cargo.toml`
- Create: `crates/core/Cargo.toml`
- Create: `crates/core/src/lib.rs`
- Create: `crates/core/src/event.rs`
- Create: `crates/core/src/hook_input.rs`
- Modify: `.gitignore`

**Interfaces:**
- Produces:
  - `agents_monitor_core::event::{MonitorEvent, EventDetail, TerminalInfo, SCHEMA_VERSION, SUMMARY_MAX_CHARS}`
  - `agents_monitor_core::hook_input::{HookContext, build_event(input: &serde_json::Value, ctx: HookContext) -> Option<MonitorEvent>, summarize_tool(tool_name: &str, tool_input: &Value) -> Option<String>, truncate_summary(text: &str) -> String}`

- [ ] **Step 1: ワークスペースを作る**

`Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/core"]
```

（`crates/hook` は Task 4、`app/src-tauri` は Task 11 で members に追加する）

`crates/core/Cargo.toml`:

```toml
[package]
name = "agents-monitor-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dirs = "6"

[dev-dependencies]
tempfile = "3"
```

`crates/core/src/lib.rs`:

```rust
pub mod event;
pub mod hook_input;
```

`.gitignore` に追記:

```
/target/
```

- [ ] **Step 2: イベントの型を書く**

`crates/core/src/event.rs`:

```rust
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const SUMMARY_MAX_CHARS: usize = 120;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorEvent {
    pub schema: u32,
    pub ts_ms: u64,
    pub event: String,
    pub session_id: String,
    pub agent_id: Option<String>,
    pub claude_pid: Option<u32>,
    pub config_dir: String,
    #[serde(default)]
    pub cwd: String,
    pub transcript_path: Option<String>,
    #[serde(default)]
    pub detail: EventDetail,
    pub terminal: Option<TerminalInfo>,
    pub git_branch: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EventDetail {
    pub tool_name: Option<String>,
    pub summary: Option<String>,
    pub notification_type: Option<String>,
    pub error_type: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub term_program: Option<String>,
    pub iterm_session_id: Option<String>,
    pub tmux: Option<String>,
    pub tmux_pane: Option<String>,
    pub entrypoint: Option<String>,
    pub tty: Option<String>,
}
```

- [ ] **Step 3: 失敗するテストを書く**

`crates/core/src/hook_input.rs`（テスト部分を先に書き、本体は空の関数にしておく）:

```rust
use serde_json::Value;

use crate::event::{EventDetail, MonitorEvent, TerminalInfo, SCHEMA_VERSION, SUMMARY_MAX_CHARS};

pub struct HookContext {
    pub ts_ms: u64,
    pub claude_pid: Option<u32>,
    pub config_dir: String,
    pub terminal: Option<TerminalInfo>,
    pub git_branch: Option<String>,
}

pub fn build_event(_input: &Value, _ctx: HookContext) -> Option<MonitorEvent> {
    None
}

pub fn summarize_tool(_tool_name: &str, _tool_input: &Value) -> Option<String> {
    None
}

pub fn truncate_summary(_text: &str) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx() -> HookContext {
        HookContext {
            ts_ms: 1_000,
            claude_pid: Some(42),
            config_dir: "/home/u/.claude".into(),
            terminal: Some(TerminalInfo { term_program: Some("iTerm.app".into()), ..Default::default() }),
            git_branch: Some("main".into()),
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
    fn copies_notification_and_error_types() {
        let notification = json!({ "hook_event_name": "Notification", "session_id": "s1", "notification_type": "permission_prompt" });
        let failure = json!({ "hook_event_name": "StopFailure", "session_id": "s1", "error_type": "rate_limit" });
        assert_eq!(build_event(&notification, ctx()).unwrap().detail.notification_type.as_deref(), Some("permission_prompt"));
        assert_eq!(build_event(&failure, ctx()).unwrap().detail.error_type.as_deref(), Some("rate_limit"));
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
        assert_eq!(summarize_tool("Edit", &json!({ "file_path": "src/a.rs" })).as_deref(), Some("src/a.rs"));
        assert_eq!(summarize_tool("NotebookEdit", &json!({ "notebook_path": "n.ipynb" })).as_deref(), Some("n.ipynb"));
        assert_eq!(summarize_tool("Grep", &json!({ "pattern": "TODO" })).as_deref(), Some("TODO"));
        assert_eq!(summarize_tool("WebFetch", &json!({ "url": "https://x.dev" })).as_deref(), Some("https://x.dev"));
        assert_eq!(summarize_tool("Agent", &json!({ "description": "調査" })).as_deref(), Some("調査"));
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
}
```

`crates/core/src/lib.rs` は Step 1 のままでよい。`EventDetail` と `SUMMARY_MAX_CHARS` を本体でまだ使っていない警告は Step 5 で解消する。

- [ ] **Step 4: テストが失敗することを確認する**

Run: `cargo test -p agents-monitor-core hook_input`
Expected: FAIL（`builds_pre_tool_use_with_bash_summary` などが `called Option::unwrap() on a None value` で失敗）

- [ ] **Step 5: 本体を実装する**

`crates/core/src/hook_input.rs` の 3 つの関数を次で置き換える:

```rust
pub fn build_event(input: &Value, ctx: HookContext) -> Option<MonitorEvent> {
    let event = str_field(input, "hook_event_name")?;
    let session_id = str_field(input, "session_id")?;
    let tool_name = str_field(input, "tool_name");
    let summary = match (&tool_name, input.get("tool_input")) {
        (Some(name), Some(tool_input)) => summarize_tool(name, tool_input),
        _ => None,
    };
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
        },
        terminal: if is_session_start { ctx.terminal } else { None },
        git_branch: if is_session_start { ctx.git_branch } else { None },
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
```

- [ ] **Step 6: テストが通ることを確認する**

Run: `cargo test -p agents-monitor-core hook_input`
Expected: PASS（7 tests）

- [ ] **Step 7: コミット**

```bash
git add Cargo.toml .gitignore crates/core
git commit -m "core: イベントの型と hook 入力の変換を追加"
```

---

### Task 2: プロセス一覧の解析とブランチ名の取得

**Files:**
- Create: `crates/core/src/ps.rs`
- Create: `crates/core/src/git.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces:
  - `ps::ProcRow { pid: u32, ppid: u32, tty: Option<String>, args: String }`
  - `ps::parse_ps(output: &str) -> Vec<ProcRow>`
  - `ps::run_ps() -> Option<Vec<ProcRow>>`
  - `ps::is_claude_command(args: &str) -> bool`
  - `ps::is_interactive_session(args: &str) -> bool`
  - `ps::find_claude_ancestor(rows: &[ProcRow], start_pid: u32) -> Option<&ProcRow>`
  - `git::git_branch(cwd: &Path) -> Option<String>`

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/lib.rs` に追加:

```rust
pub mod git;
pub mod ps;
```

`crates/core/src/ps.rs`:

```rust
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub struct ProcRow {
    pub pid: u32,
    pub ppid: u32,
    pub tty: Option<String>,
    pub args: String,
}

pub fn parse_ps(_output: &str) -> Vec<ProcRow> {
    Vec::new()
}

pub fn run_ps() -> Option<Vec<ProcRow>> {
    None
}

pub fn is_claude_command(_args: &str) -> bool {
    false
}

pub fn is_interactive_session(_args: &str) -> bool {
    false
}

pub fn find_claude_ancestor(_rows: &[ProcRow], _start_pid: u32) -> Option<&ProcRow> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pid: u32, ppid: u32, args: &str) -> ProcRow {
        ProcRow { pid, ppid, tty: None, args: args.into() }
    }

    #[test]
    fn parses_ps_output() {
        let output = "  101     1 ttys003  /Users/u/.local/bin/claude --model opus\n  202   101 ??       /bin/sh -c hook\nbroken line\n";
        let rows = parse_ps(output);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], ProcRow { pid: 101, ppid: 1, tty: Some("ttys003".into()), args: "/Users/u/.local/bin/claude --model opus".into() });
        assert_eq!(rows[1].tty, None);
    }

    #[test]
    fn recognizes_claude_commands() {
        assert!(is_claude_command("/Users/u/.local/bin/claude"));
        assert!(is_claude_command("claude --resume"));
        assert!(is_claude_command("node /opt/lib/node_modules/@anthropic-ai/claude-code/cli.js"));
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
        assert!(!is_interactive_session("node /x/@anthropic-ai/claude-code/cli.js mcp list"));
        assert!(!is_interactive_session("/bin/zsh"));
    }

    #[test]
    fn finds_claude_ancestor_through_shells() {
        let rows = vec![
            row(1, 0, "/sbin/launchd"),
            row(100, 1, "/Users/u/.local/bin/claude"),
            row(200, 100, "/bin/sh -c hook"),
            row(300, 200, "/Users/u/.agents-monitor/bin/agents-monitor-hook"),
        ];
        assert_eq!(find_claude_ancestor(&rows, 300).map(|r| r.pid), Some(100));
    }

    #[test]
    fn returns_none_without_claude_ancestor() {
        let rows = vec![row(1, 0, "/sbin/launchd"), row(300, 1, "agents-monitor-hook")];
        assert_eq!(find_claude_ancestor(&rows, 300), None);
        assert_eq!(find_claude_ancestor(&rows, 999), None);
    }
}
```

`crates/core/src/git.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

pub fn git_branch(_cwd: &Path) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_branch_from_head() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".git")).unwrap();
        fs::write(repo.path().join(".git/HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        let sub = repo.path().join("src/deep");
        fs::create_dir_all(&sub).unwrap();
        assert_eq!(git_branch(&sub).as_deref(), Some("feature/x"));
    }

    #[test]
    fn follows_worktree_gitdir_file() {
        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("main/.git/worktrees/wt");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("HEAD"), "ref: refs/heads/wt-branch\n").unwrap();
        let wt = root.path().join("wt");
        fs::create_dir_all(&wt).unwrap();
        fs::write(wt.join(".git"), format!("gitdir: {}\n", real.display())).unwrap();
        assert_eq!(git_branch(&wt).as_deref(), Some("wt-branch"));
    }

    #[test]
    fn shows_short_hash_when_detached() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".git")).unwrap();
        fs::write(repo.path().join(".git/HEAD"), "0123456789abcdef\n").unwrap();
        assert_eq!(git_branch(repo.path()).as_deref(), Some("0123456"));
    }

    #[test]
    fn returns_none_outside_repo() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(git_branch(dir.path()), None);
    }
}
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p agents-monitor-core -- ps:: git::`
Expected: FAIL（`parses_ps_output`、`reads_branch_from_head` など）

- [ ] **Step 3: 本体を実装する**

`ps.rs` の関数を置き換える:

```rust
const PS_ARGS: [&str; 2] = ["-axo", "pid=,ppid=,tty=,args="];
const NON_SESSION_SUBCOMMANDS: [&str; 7] = ["plugin", "plugins", "mcp", "config", "update", "doctor", "install"];
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
    Some(ProcRow { pid, ppid, tty: (tty != "??").then(|| tty.to_string()), args })
}

pub fn run_ps() -> Option<Vec<ProcRow>> {
    let output = Command::new("ps").args(PS_ARGS).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(parse_ps(&String::from_utf8_lossy(&output.stdout)))
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
        if is_claude_command(&row.args) {
            return Some(row);
        }
        if row.ppid == current || row.ppid == 0 {
            return None;
        }
        current = row.ppid;
    }
    None
}
```

`git.rs` の関数を置き換える:

```rust
pub fn git_branch(cwd: &Path) -> Option<String> {
    let git_dir = find_git_dir(cwd)?;
    let head = fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    match head.strip_prefix("ref: refs/heads/") {
        Some(branch) => Some(branch.to_string()),
        None => Some(head.chars().take(7).collect()),
    }
}

fn find_git_dir(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        let dot_git = dir.join(".git");
        if dot_git.is_dir() {
            return Some(dot_git);
        }
        if dot_git.is_file() {
            let content = fs::read_to_string(&dot_git).ok()?;
            let target = Path::new(content.trim().strip_prefix("gitdir: ")?);
            return Some(if target.is_absolute() { target.to_path_buf() } else { dir.join(target) });
        }
    }
    None
}
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p agents-monitor-core -- ps:: git::`
Expected: PASS（9 tests）

- [ ] **Step 5: 実機の ps で Claude が見つかることを確認する**

Run: `ps -axo pid=,ppid=,tty=,args= | grep -i claude | grep -v grep`
Expected: 今動いている Claude Code（この作業をしている VS Code 拡張のものを含む）の行が出る。その `args` を `is_claude_command` のテストに 1 行追加し、通ることを確認する。通らなければ `claude_args` を直す。

- [ ] **Step 6: コミット**

```bash
git add crates/core
git commit -m "core: ps の解析と Claude プロセスの判定、ブランチ名の取得を追加"
```

---

### Task 3: イベントファイルの書き出しと読み取り

**Files:**
- Create: `crates/core/src/paths.rs`
- Create: `crates/core/src/spool.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `event::MonitorEvent`
- Produces:
  - `paths::monitor_home() -> Option<PathBuf>`（`AGENTS_MONITOR_HOME` があればそれ、なければ `~/.agents-monitor`）
  - `paths::prepare_private_dir(dir: &Path) -> io::Result<()>`（作成して 0700）
  - `paths::events_dir(home: &Path) -> PathBuf`、`paths::raw_dir`、`paths::bin_path`、`paths::state_path`、`paths::settings_path`
  - `paths::default_config_dir() -> Option<String>`
  - `paths::now_ms() -> u64`
  - `spool::write_event(events_dir: &Path, event: &MonitorEvent) -> io::Result<PathBuf>`
  - `spool::SpoolBatch { events: Vec<MonitorEvent>, skipped: usize }`
  - `spool::drain_spool(events_dir: &Path, max_files: usize) -> SpoolBatch`
  - `spool::MAX_SPOOL_FILES: usize = 10_000`

- [ ] **Step 1: paths を書く（ロジックがほぼないので先に実装する）**

`crates/core/src/lib.rs` に追加:

```rust
pub mod paths;
pub mod spool;
```

`crates/core/src/paths.rs`:

```rust
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn monitor_home() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("AGENTS_MONITOR_HOME") {
        return Some(PathBuf::from(dir));
    }
    Some(dirs::home_dir()?.join(".agents-monitor"))
}

pub fn prepare_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn events_dir(home: &Path) -> PathBuf {
    home.join("events")
}

pub fn raw_dir(home: &Path) -> PathBuf {
    home.join("raw")
}

pub fn bin_path(home: &Path) -> PathBuf {
    home.join("bin").join("agents-monitor-hook")
}

pub fn state_path(home: &Path) -> PathBuf {
    home.join("state.json")
}

pub fn settings_path(home: &Path) -> PathBuf {
    home.join("settings.json")
}

pub fn default_config_dir() -> Option<String> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Some(dir.to_string_lossy().into_owned());
    }
    Some(dirs::home_dir()?.join(".claude").to_string_lossy().into_owned())
}

pub fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}
```

- [ ] **Step 2: spool の失敗するテストを書く**

`crates/core/src/spool.rs`:

```rust
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::event::MonitorEvent;

pub const MAX_SPOOL_FILES: usize = 10_000;

#[derive(Debug, Default)]
pub struct SpoolBatch {
    pub events: Vec<MonitorEvent>,
    pub skipped: usize,
}

pub fn write_event(_events_dir: &Path, _event: &MonitorEvent) -> io::Result<PathBuf> {
    Err(io::Error::other("unimplemented"))
}

pub fn drain_spool(_events_dir: &Path, _max_files: usize) -> SpoolBatch {
    SpoolBatch::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventDetail;

    fn ev(event: &str, ts_ms: u64) -> MonitorEvent {
        MonitorEvent {
            schema: 1,
            ts_ms,
            event: event.into(),
            session_id: "s1".into(),
            agent_id: None,
            claude_pid: None,
            config_dir: "/c".into(),
            cwd: "/w".into(),
            transcript_path: None,
            detail: EventDetail::default(),
            terminal: None,
            git_branch: None,
        }
    }

    #[test]
    fn writes_and_drains_in_time_order() {
        let dir = tempfile::tempdir().unwrap();
        write_event(dir.path(), &ev("Stop", 3_000)).unwrap();
        write_event(dir.path(), &ev("SessionStart", 1_000)).unwrap();
        write_event(dir.path(), &ev("UserPromptSubmit", 2_000)).unwrap();
        let batch = drain_spool(dir.path(), MAX_SPOOL_FILES);
        let names: Vec<_> = batch.events.iter().map(|e| e.event.as_str()).collect();
        assert_eq!(names, ["SessionStart", "UserPromptSubmit", "Stop"]);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn leaves_no_tmp_file_after_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_event(dir.path(), &ev("Stop", 1)).unwrap();
        assert_eq!(path.extension().unwrap(), "json");
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn skips_broken_files_and_ignores_tmp() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("0000000000001-1-1.json"), "{broken").unwrap();
        fs::write(dir.path().join("0000000000002-1-1.json.tmp"), "{}").unwrap();
        write_event(dir.path(), &ev("Stop", 5)).unwrap();
        let batch = drain_spool(dir.path(), MAX_SPOOL_FILES);
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.skipped, 1);
        assert!(dir.path().join("0000000000002-1-1.json.tmp").exists());
    }

    #[test]
    fn drops_oldest_files_beyond_limit() {
        let dir = tempfile::tempdir().unwrap();
        for ts in 1..=5 {
            write_event(dir.path(), &ev("Stop", ts)).unwrap();
        }
        let batch = drain_spool(dir.path(), 2);
        let ts: Vec<_> = batch.events.iter().map(|e| e.ts_ms).collect();
        assert_eq!(ts, [4, 5]);
        assert_eq!(batch.skipped, 3);
    }

    #[test]
    fn missing_directory_yields_empty_batch() {
        let batch = drain_spool(Path::new("/nonexistent/agents-monitor"), 10);
        assert!(batch.events.is_empty());
    }
}
```

- [ ] **Step 3: テストが失敗することを確認する**

Run: `cargo test -p agents-monitor-core spool`
Expected: FAIL（`writes_and_drains_in_time_order` が `unimplemented` で失敗）

- [ ] **Step 4: 本体を実装する**

```rust
pub fn write_event(events_dir: &Path, event: &MonitorEvent) -> io::Result<PathBuf> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let name = format!("{:013}-{}-{:09}.json", event.ts_ms, std::process::id(), nanos);
    let final_path = events_dir.join(&name);
    // 途中まで書かれたファイルをアプリに読ませないため、一時ファイルに書いてから名前を変える
    let tmp_path = events_dir.join(format!("{name}.tmp"));
    fs::write(&tmp_path, serde_json::to_vec(event)?)?;
    fs::rename(&tmp_path, &final_path)?;
    Ok(final_path)
}

pub fn drain_spool(events_dir: &Path, max_files: usize) -> SpoolBatch {
    let Ok(entries) = fs::read_dir(events_dir) else {
        return SpoolBatch::default();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();

    let mut batch = SpoolBatch::default();
    let overflow = paths.len().saturating_sub(max_files);
    for path in paths.drain(..overflow) {
        let _ = fs::remove_file(path);
        batch.skipped += 1;
    }
    for path in paths {
        match fs::read(&path).ok().and_then(|bytes| serde_json::from_slice::<MonitorEvent>(&bytes).ok()) {
            Some(event) => batch.events.push(event),
            None => batch.skipped += 1,
        }
        let _ = fs::remove_file(&path);
    }
    batch.events.sort_by_key(|e| e.ts_ms);
    batch
}
```

- [ ] **Step 5: テストが通ることを確認する**

Run: `cargo test -p agents-monitor-core spool`
Expected: PASS（5 tests）

- [ ] **Step 6: コミット**

```bash
git add crates/core
git commit -m "core: イベントファイルの書き出しと読み取りを追加"
```

---

### Task 4: hook プログラム（agents-monitor-hook）

**Files:**
- Create: `crates/hook/Cargo.toml`
- Create: `crates/hook/src/main.rs`
- Create: `crates/hook/tests/cli.rs`
- Modify: `Cargo.toml`（members を `["crates/core", "crates/hook"]` にする）

**Interfaces:**
- Consumes: `hook_input::{build_event, HookContext}`、`ps::{run_ps, find_claude_ancestor}`、`git::git_branch`、`paths::*`、`spool::write_event`、`event::TerminalInfo`
- Produces: 実行ファイル `agents-monitor-hook`。環境変数 `AGENTS_MONITOR_HOME`（保存先の上書き）、`AGENTS_MONITOR_DEBUG_RAW`（設定時は生の入力も `raw/` に保存。Task 6 の調査専用）

- [ ] **Step 1: 失敗するテストを書く**

`crates/hook/Cargo.toml`:

```toml
[package]
name = "agents-monitor-hook"
version = "0.1.0"
edition = "2021"

[dependencies]
agents-monitor-core = { path = "../core" }
serde_json = "1"

[dev-dependencies]
tempfile = "3"
```

`crates/hook/src/main.rs`（仮）:

```rust
fn main() {}
```

`crates/hook/tests/cli.rs`:

```rust
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn run_hook(home: &Path, stdin: &[u8]) -> (Output, Duration) {
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_agents-monitor-hook"))
        .env("AGENTS_MONITOR_HOME", home)
        .env("CLAUDE_CONFIG_DIR", "/tmp/test-claude-config")
        .env_remove("AGENTS_MONITOR_DEBUG_RAW")
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
    assert!(output.stdout.is_empty(), "stdout: {:?}", String::from_utf8_lossy(&output.stdout));
    assert!(output.stderr.is_empty(), "stderr: {:?}", String::from_utf8_lossy(&output.stderr));
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
    for input in [&b"not json"[..], &b""[..], &[0xff, 0xfe, 0x00][..], &br#"{"no":"fields"}"#[..]] {
        let home = tempfile::tempdir().unwrap();
        let (output, _) = run_hook(home.path(), input);
        assert_silent_success(&output);
        assert!(event_files(home.path()).is_empty());
    }
}

#[test]
fn unwritable_home_still_exits_zero() {
    let (output, _) = run_hook(Path::new("/dev/null/agents-monitor"), br#"{"hook_event_name":"Stop","session_id":"s1"}"#);
    assert_silent_success(&output);
}
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p agents-monitor-hook`
Expected: FAIL（`writes_one_event_file_silently` でファイルが 0 件）

- [ ] **Step 3: 本体を実装する**

`crates/hook/src/main.rs`:

```rust
use std::fs;
use std::io::Read;
use std::path::Path;

use agents_monitor_core::event::TerminalInfo;
use agents_monitor_core::git::git_branch;
use agents_monitor_core::hook_input::{build_event, HookContext};
use agents_monitor_core::paths;
use agents_monitor_core::ps::{find_claude_ancestor, run_ps};
use agents_monitor_core::spool::write_event;

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
    if std::env::var_os("AGENTS_MONITOR_DEBUG_RAW").is_some() {
        save_raw(&home, ts_ms, &raw);
    }
    let input: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    let is_session_start = input.get("hook_event_name").and_then(|v| v.as_str()) == Some("SessionStart");

    let rows = run_ps().unwrap_or_default();
    let claude = find_claude_ancestor(&rows, std::process::id());
    let terminal = is_session_start.then(|| terminal_info(claude.and_then(|c| c.tty.clone())));
    let branch = if is_session_start {
        input.get("cwd").and_then(|v| v.as_str()).and_then(|cwd| git_branch(Path::new(cwd)))
    } else {
        None
    };

    let ctx = HookContext {
        ts_ms,
        claude_pid: claude.map(|c| c.pid),
        config_dir: paths::default_config_dir()?,
        terminal,
        git_branch: branch,
    };
    let event = build_event(&input, ctx)?;
    paths::prepare_private_dir(&home).ok()?;
    let events_dir = paths::events_dir(&home);
    paths::prepare_private_dir(&events_dir).ok()?;
    write_event(&events_dir, &event).ok()?;
    Some(())
}

fn terminal_info(tty: Option<String>) -> TerminalInfo {
    let var = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    TerminalInfo {
        term_program: var("TERM_PROGRAM"),
        iterm_session_id: var("ITERM_SESSION_ID"),
        tmux: var("TMUX"),
        tmux_pane: var("TMUX_PANE"),
        entrypoint: var("CLAUDE_CODE_ENTRYPOINT"),
        tty,
    }
}

fn save_raw(home: &Path, ts_ms: u64, raw: &[u8]) {
    let dir = paths::raw_dir(home);
    if paths::prepare_private_dir(home).is_err() || paths::prepare_private_dir(&dir).is_err() {
        return;
    }
    let _ = fs::write(dir.join(format!("{ts_ms:013}-{}.json", std::process::id())), raw);
}
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p agents-monitor-hook`
Expected: PASS（5 tests）

- [ ] **Step 5: 実行時間を測る**

Run: `cargo build --release -p agents-monitor-hook && time (echo '{"hook_event_name":"Stop","session_id":"t"}' | AGENTS_MONITOR_HOME=$(mktemp -d) ./target/release/agents-monitor-hook)`
Expected: `real` が 0.1 秒未満。超える場合は計測値を記録し、作業を止めて報告する（`ps` の呼び出しが主な原因になるはず）。

- [ ] **Step 6: コミット**

```bash
git add Cargo.toml crates/hook
git commit -m "hook: 状態をイベントファイルに書き出す agents-monitor-hook を追加"
```

---

### Task 5: プラグインとマーケットプレイス

**Files:**
- Create: `plugin/.claude-plugin/plugin.json`
- Create: `plugin/hooks/hooks.json`
- Create: `.claude-plugin/marketplace.json`
- Create: `scripts/dev-install-hook.sh`
- Create: `crates/hook/tests/plugin_manifest.rs`

**Interfaces:**
- Consumes: Task 4 の実行ファイルを `~/.agents-monitor/bin/agents-monitor-hook` に置く前提
- Produces: プラグイン `agents-monitor@agents-monitor`（マーケットプレイス名・プラグイン名ともに `agents-monitor`）

- [ ] **Step 1: 失敗するテストを書く**

`crates/hook/tests/plugin_manifest.rs`:

```rust
use std::fs;

const REQUIRED_EVENTS: [&str; 15] = [
    "SessionStart", "SessionEnd", "UserPromptSubmit", "PreToolUse", "PostToolUse",
    "PostToolUseFailure", "PermissionRequest", "Notification", "Stop", "StopFailure",
    "SubagentStart", "SubagentStop", "PreCompact", "Elicitation", "ElicitationResult",
];

fn load(path: &str) -> serde_json::Value {
    let full = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&fs::read_to_string(full).unwrap()).unwrap()
}

#[test]
fn registers_every_required_event_with_safe_command() {
    let hooks = load("plugin/hooks/hooks.json");
    for event in REQUIRED_EVENTS {
        let command = hooks["hooks"][event][0]["hooks"][0]["command"].as_str()
            .unwrap_or_else(|| panic!("{event} が登録されていない"));
        assert!(command.contains("agents-monitor-hook"), "{event}");
        assert!(command.trim_end().ends_with("exit 0"), "{event} は必ず exit 0 で終わること");
    }
}

#[test]
fn marketplace_points_to_plugin_directory() {
    let marketplace = load(".claude-plugin/marketplace.json");
    assert_eq!(marketplace["name"], "agents-monitor");
    assert_eq!(marketplace["plugins"][0]["name"], "agents-monitor");
    assert_eq!(marketplace["plugins"][0]["source"], "./plugin");
    let plugin = load("plugin/.claude-plugin/plugin.json");
    assert_eq!(plugin["name"], "agents-monitor");
}
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p agents-monitor-hook --test plugin_manifest`
Expected: FAIL（ファイルが存在しない）

- [ ] **Step 3: プラグインとマーケットプレイスのファイルを書く**

`plugin/.claude-plugin/plugin.json`:

```json
{
  "name": "agents-monitor",
  "description": "Claude Code セッションの状態を agents-monitor メニューバーアプリに送る hooks",
  "version": "0.1.0"
}
```

`.claude-plugin/marketplace.json`:

```json
{
  "name": "agents-monitor",
  "owner": { "name": "imatomix" },
  "plugins": [
    {
      "name": "agents-monitor",
      "description": "Claude Code セッションの状態を agents-monitor メニューバーアプリに送る hooks",
      "source": "./plugin"
    }
  ]
}
```

`plugin/hooks/hooks.json`（15 イベントすべてに同じ hook を登録する。アプリ未導入でも Claude Code にエラーを出さないよう、存在確認してから呼び、最後に必ず `exit 0`）:

```json
{
  "hooks": {
    "SessionStart": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "SessionEnd": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "PreToolUse": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "PostToolUse": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "PostToolUseFailure": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "PermissionRequest": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "Notification": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "StopFailure": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "SubagentStart": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "SubagentStop": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "PreCompact": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "Elicitation": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }],
    "ElicitationResult": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agents-monitor/bin/agents-monitor-hook\" ] && \"$HOME/.agents-monitor/bin/agents-monitor-hook\"; exit 0", "timeout": 5 }] }]
  }
}
```

`scripts/dev-install-hook.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release -p agents-monitor-hook
install -d -m 700 "$HOME/.agents-monitor" "$HOME/.agents-monitor/bin"
install -m 755 target/release/agents-monitor-hook "$HOME/.agents-monitor/bin/agents-monitor-hook"
echo "installed: $HOME/.agents-monitor/bin/agents-monitor-hook"
```

Run: `chmod +x scripts/dev-install-hook.sh`

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p agents-monitor-hook --test plugin_manifest`
Expected: PASS（2 tests）

- [ ] **Step 5: プラグインを実際に入れて検証する**

```bash
./scripts/dev-install-hook.sh
CLAUDE_CONFIG_DIR=$HOME/.claude-personal claude plugin validate ./plugin 2>&1 | tail -5 || true
CLAUDE_CONFIG_DIR=$HOME/.claude-personal claude plugin marketplace add "$PWD"
CLAUDE_CONFIG_DIR=$HOME/.claude-personal claude plugin install agents-monitor@agents-monitor
CLAUDE_CONFIG_DIR=$HOME/.claude-personal claude plugin list
```

Expected: `agents-monitor@agents-monitor` が `enabled`。`claude plugin validate` が存在しないサブコマンドならその行は無視してよい。

続けてターミナルで `CLAUDE_CONFIG_DIR=$HOME/.claude-personal claude -p "hello"` を実行し、`ls ~/.agents-monitor/events` にファイルが増えることを確認する（`-p` でも hook は動く）。確認後 `rm ~/.agents-monitor/events/*.json` で消す。

- [ ] **Step 6: コミット**

```bash
git add plugin .claude-plugin scripts/dev-install-hook.sh crates/hook/tests/plugin_manifest.rs
git commit -m "プラグインとマーケットプレイスの定義を追加"
```

---

### Task 6: 実イベントの記録と確認（調査）

コードの成果物はテストデータと調査メモ。spec 4.3 の確認項目に答え、Task 7 以降の前提を固める。**人の操作が必要なタスク**。実行者がエージェントの場合は、各シナリオの操作手順をユーザーに示して実施してもらう。

**Files:**
- Create: `scripts/collect-fixture.sh`
- Create: `crates/core/tests/fixtures/*.jsonl`
- Create: `docs/superpowers/notes/2026-10-09-hook-event-findings.md`

**Interfaces:**
- Produces: `crates/core/tests/fixtures/<期待する最終状態>__<シナリオ名>.jsonl`。1 行 1 イベント（`MonitorEvent` の JSON）。期待する最終状態は `needs_action` / `error` / `done` / `running` / `ended` のいずれか。Task 7 の再生テストがこの名前を使う。

- [ ] **Step 1: 収集スクリプトを書く**

`scripts/collect-fixture.sh`:

```bash
#!/usr/bin/env bash
# 使い方: scripts/collect-fixture.sh <期待する最終状態>__<シナリオ名>
set -euo pipefail
cd "$(dirname "$0")/.."
name="$1"
src="$HOME/.agents-monitor/events"
dest="crates/core/tests/fixtures/$name.jsonl"
mkdir -p "$(dirname "$dest")"
: > "$dest"
for f in $(ls "$src"/*.json | sort); do
  # 個人のパスをリポジトリに残さないため、ホームディレクトリを置き換える
  sed "s#$HOME#/Users/test#g" "$f" >> "$dest"
  echo >> "$dest"
  rm "$f"
done
echo "$(wc -l < "$dest") events -> $dest"
```

Run: `chmod +x scripts/collect-fixture.sh && rm -f ~/.agents-monitor/events/*.json`

- [ ] **Step 2: シナリオを 1 つずつ記録する**

準備: 記録中は `~/.claude-personal` を使う他の Claude Code セッションを新しく起動しない（別セッションのイベントが混ざるため）。新しいターミナルで `export AGENTS_MONITOR_DEBUG_RAW=1` をしてから、このリポジトリで `CLAUDE_CONFIG_DIR=$HOME/.claude-personal claude` を起動する（hook が生の入力も `~/.agents-monitor/raw/` に残す）。

各シナリオの前に `rm -f ~/.agents-monitor/events/*.json`、終わったら `scripts/collect-fixture.sh <名前>` を実行する。

| 名前 | 操作 |
|---|---|
| `done__simple-prompt` | 起動して「こんにちは」と送り、応答が終わるのを待つ |
| `needs_action__permission-pending` | 「`ls -la /tmp` を実行して」と送り、許可ダイアログが出た状態で収集する（許可しない） |
| `done__permission-approved` | 続けて許可し、応答が終わるのを待つ |
| `done__permission-denied` | 「`ls /` を実行して」と送り、拒否して応答が終わるのを待つ |
| `needs_action__ask-user-question` | 「AskUserQuestion ツールで好きな色を 2 択で聞いて」と送り、質問が出た状態で収集する |
| `needs_action__plan-approval` | Shift+Tab でプランモードにし、「README を作る計画を立てて」と送り、承認待ちで収集する |
| `done__subagent` | 「Explore サブエージェントで Cargo.toml の場所を調べて」と送り、終わるのを待つ |
| `?__esc-interrupt` | 「1 から 100 まで 1 行ずつゆっくり数えて」と送り、途中で Esc を押して 10 秒待つ。届いたイベントを見て、最後が `Stop` なら `done__esc-interrupt`、そうでなければ `running__esc-interrupt` として収集する |
| `done__background-bash` | 「`sleep 60` をバックグラウンドで実行して」と送り、応答が終わった直後に収集する |
| `ended__exit` | `/exit` で終了して収集する |

- [ ] **Step 3: 確認項目を調べてメモに書く**

`docs/superpowers/notes/2026-10-09-hook-event-findings.md` を作り、次の見出しごとに、実際に届いたイベント名の並び（`raw/` の `hook_event_name` と主な項目）と結論を書く:

```markdown
# hook イベントの調査結果（2026-10-09）

## 1. Esc で中断したとき
届いたイベント: （例: UserPromptSubmit → PreToolUse → （Stop なし））
会話ログ（transcript_path）の最後の行: （該当行をそのまま貼る）
結論: Stop が届く / 届かない。届かない場合の中断の目印となる文字列: 「…」

## 2. 許可待ちのイベント順
届いたイベント: （例: PreToolUse → PermissionRequest → Notification(permission_prompt)）
結論:

## 3. バックグラウンド作業が残っているとき
届いたイベント:
結論:

## 4. サブエージェント
SubagentStart に agent_id が付くか:
サブエージェント内の PreToolUse に agent_id が付くか:
結論:

## 5. VS Code 拡張のセッション
VS Code 拡張で新しい会話を開き、`ps -axo pid=,ppid=,tty=,args= | grep -i claude` の結果を貼る:
SessionStart の terminal.entrypoint:
hook が見つけた claude_pid がこのプロセスと一致するか:
結論:

## 6. hook の所要時間と async
`raw/` のファイル時刻と応答の体感から、hook による遅れが気になるか:
結論: 同期のままにする / hooks.json に "async": true を付ける
```

5 のために、VS Code 拡張で新しい会話を 1 つ開き、「こんにちは」と送ってから `~/.agents-monitor/events/` の SessionStart を確認する。

- [ ] **Step 4: 結論を反映する**

- 1 で Stop が届かない場合: 目印の文字列を Task 8 の `INTERRUPT_MARKER` に使う（計画の値と違えば Task 8 の定数を書き換える）。Stop が届く場合: Task 8 の中断検知（Step 1〜4 の `transcript` 部分と `detect_interrupts`）は実装を省き、その旨をメモに書く。
- 2〜4 の結果が Task 7 の対応表（`transition_for`）や `accepts` の前提と違う場合: Task 7 に着手する前に計画の該当コードを直し、直した点をメモに書く。
- 5 で `is_claude_command` が VS Code 拡張の Claude を判定できない場合: Task 2 の `claude_args` とテストを直す。
- 6 で async にすると決めた場合: `plugin/hooks/hooks.json` の各 hook に `"async": true` を足し、`plugin_manifest` テストが通ることを確認する。

- [ ] **Step 5: 生の入力を消す**

Run: `rm -rf ~/.agents-monitor/raw && unset AGENTS_MONITOR_DEBUG_RAW`
（`raw/` には生のツール入力が入っているため、調査が終わったら必ず消す）

- [ ] **Step 6: コミット**

```bash
git add scripts/collect-fixture.sh crates/core/tests/fixtures docs/superpowers/notes plugin crates
git commit -m "実イベントを記録したテストデータと調査結果を追加"
```

---

### Task 7: 状態遷移とセッション管理

**Files:**
- Create: `crates/core/src/state.rs`
- Create: `crates/core/src/store.rs`
- Create: `crates/core/tests/fixtures_replay.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `event::MonitorEvent`、Task 6 のテストデータ
- Produces:
  - `state::Status { NeedsAction, Error, Done, Running, Ended, Unknown }`（serde は snake_case）、`Status::label(self) -> &'static str`
  - `state::Transition { status: Status, detail: Option<String> }`、`state::transition_for(ev: &MonitorEvent) -> Option<Transition>`
  - `state::Session`（フィールドは下記）、`Session::project(&self) -> String`
  - `store::SessionStore`（`Default + Serialize + Deserialize`）、`sessions(&self) -> &[Session]`、`apply(&mut self, ev: &MonitorEvent) -> Option<StatusChange>`、`summary(&self) -> TraySummary`
  - `store::StatusChange { session_id: String, project: String, from: Option<Status>, to: Status, detail: Option<String>, ts_ms: u64 }`
  - `store::TraySummary { total: usize, urgent: usize, badge: Option<Status> }`
  - `store::UNKNOWN_ID_PREFIX: &str = "pid:"`

- [ ] **Step 1: state.rs を書く（対応表）**

`crates/core/src/lib.rs` に追加:

```rust
pub mod state;
pub mod store;
```

`crates/core/src/state.rs`:

```rust
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::event::{MonitorEvent, TerminalInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    NeedsAction,
    Error,
    Done,
    Running,
    Ended,
    Unknown,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::NeedsAction => "要対応",
            Status::Error => "エラー停止",
            Status::Done => "完了",
            Status::Running => "実行中",
            Status::Ended => "終了",
            Status::Unknown => "状態不明",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub status: Status,
    pub detail: Option<String>,
}

pub fn transition_for(ev: &MonitorEvent) -> Option<Transition> {
    let detail = &ev.detail;
    let tool_label = match (&detail.tool_name, &detail.summary) {
        (Some(tool), Some(summary)) => Some(format!("{tool}: {summary}")),
        (Some(tool), None) => Some(tool.clone()),
        _ => None,
    };
    let to = |status: Status, detail: Option<String>| Some(Transition { status, detail });
    match ev.event.as_str() {
        "SessionStart" => to(Status::Done, Some("新規".into())),
        "UserPromptSubmit" => to(Status::Running, None),
        "PreToolUse" => match detail.tool_name.as_deref() {
            Some("AskUserQuestion") => to(Status::NeedsAction, Some("質問待ち".into())),
            Some("ExitPlanMode") => to(Status::NeedsAction, Some("プラン承認待ち".into())),
            _ => to(Status::Running, tool_label),
        },
        "PermissionRequest" => to(Status::NeedsAction, Some(join_label("許可待ち", tool_label))),
        "Notification" => match detail.notification_type.as_deref() {
            Some("permission_prompt") => to(Status::NeedsAction, Some("許可待ち".into())),
            Some("elicitation_dialog" | "elicitation_url_dialog" | "agent_needs_input") => {
                to(Status::NeedsAction, Some("入力要求".into()))
            }
            Some("idle_prompt") => to(Status::Done, None),
            _ => None,
        },
        "Elicitation" => to(Status::NeedsAction, Some("入力要求".into())),
        "PostToolUse" | "PostToolUseFailure" | "ElicitationResult" => to(Status::Running, None),
        "SubagentStart" => to(Status::Running, Some("サブエージェント実行中".into())),
        "PreCompact" => to(Status::Running, Some("圧縮中".into())),
        "Stop" => to(Status::Done, None),
        "StopFailure" => to(Status::Error, detail.error_type.clone()),
        "SessionEnd" => to(Status::Ended, None),
        _ => None,
    }
}

fn join_label(head: &str, tail: Option<String>) -> String {
    match tail {
        Some(tail) => format!("{head} · {tail}"),
        None => head.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub status: Status,
    pub detail: Option<String>,
    pub status_since_ms: u64,
    pub first_seen_ms: u64,
    pub last_event_ms: u64,
    pub cwd: String,
    pub config_dir: String,
    pub git_branch: Option<String>,
    pub claude_pid: Option<u32>,
    pub terminal: Option<TerminalInfo>,
    pub transcript_path: Option<String>,
    pub active_subagents: u32,
}

impl Session {
    pub fn project(&self) -> String {
        if let Some(name) = Path::new(&self.cwd).file_name() {
            return name.to_string_lossy().into_owned();
        }
        match self.claude_pid {
            Some(pid) => format!("Claude (pid {pid})"),
            None => "Claude".to_string(),
        }
    }
}
```

- [ ] **Step 2: store の失敗するテストを書く**

`crates/core/src/store.rs`（本体は空、テストを先に）:

```rust
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::event::MonitorEvent;
use crate::state::{transition_for, Session, Status, Transition};

pub const UNKNOWN_ID_PREFIX: &str = "pid:";

#[derive(Debug, Clone, PartialEq)]
pub struct StatusChange {
    pub session_id: String,
    pub project: String,
    pub from: Option<Status>,
    pub to: Status,
    pub detail: Option<String>,
    pub ts_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraySummary {
    pub total: usize,
    pub urgent: usize,
    pub badge: Option<Status>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SessionStore {
    sessions: Vec<Session>,
    #[serde(default)]
    dismissed_pids: HashSet<u32>,
}

impl SessionStore {
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    pub fn apply(&mut self, _ev: &MonitorEvent) -> Option<StatusChange> {
        None
    }

    pub fn summary(&self) -> TraySummary {
        TraySummary { total: 0, urgent: 0, badge: None }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use crate::event::{EventDetail, MonitorEvent};

    pub fn ev(event: &str, session: &str, ts_ms: u64) -> MonitorEvent {
        MonitorEvent {
            schema: 1,
            ts_ms,
            event: event.into(),
            session_id: session.into(),
            agent_id: None,
            claude_pid: Some(100),
            config_dir: "/Users/test/.claude".into(),
            cwd: "/work/my-api".into(),
            transcript_path: None,
            detail: EventDetail::default(),
            terminal: None,
            git_branch: None,
        }
    }

    pub fn tool(mut event: MonitorEvent, name: &str, summary: Option<&str>) -> MonitorEvent {
        event.detail.tool_name = Some(name.into());
        event.detail.summary = summary.map(str::to_string);
        event
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{ev, tool};
    use super::*;

    fn status_of(store: &SessionStore, id: &str) -> (Status, Option<String>) {
        let s = store.sessions().iter().find(|s| s.id == id).unwrap();
        (s.status, s.detail.clone())
    }

    #[test]
    fn session_start_creates_new_done_session() {
        let mut store = SessionStore::default();
        let change = store.apply(&ev("SessionStart", "s1", 1)).unwrap();
        assert_eq!(change.from, None);
        assert_eq!(change.to, Status::Done);
        assert_eq!(change.project, "my-api");
        assert_eq!(status_of(&store, "s1"), (Status::Done, Some("新規".into())));
    }

    #[test]
    fn prompt_tool_stop_cycle() {
        let mut store = SessionStore::default();
        store.apply(&ev("SessionStart", "s1", 1));
        store.apply(&ev("UserPromptSubmit", "s1", 2));
        store.apply(&tool(ev("PreToolUse", "s1", 3), "Bash", Some("npm test")));
        assert_eq!(status_of(&store, "s1"), (Status::Running, Some("Bash: npm test".into())));
        let change = store.apply(&ev("Stop", "s1", 4)).unwrap();
        assert_eq!(change.from, Some(Status::Running));
        assert_eq!(status_of(&store, "s1"), (Status::Done, None));
    }

    #[test]
    fn permission_request_then_resume() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "s1", 1));
        store.apply(&tool(ev("PermissionRequest", "s1", 2), "Bash", Some("rm -rf dist")));
        assert_eq!(status_of(&store, "s1"), (Status::NeedsAction, Some("許可待ち · Bash: rm -rf dist".into())));
        store.apply(&tool(ev("PostToolUse", "s1", 3), "Bash", None));
        assert_eq!(status_of(&store, "s1").0, Status::Running);
    }

    #[test]
    fn question_and_plan_approval_need_action() {
        let mut store = SessionStore::default();
        store.apply(&tool(ev("PreToolUse", "s1", 1), "AskUserQuestion", None));
        assert_eq!(status_of(&store, "s1"), (Status::NeedsAction, Some("質問待ち".into())));
        store.apply(&tool(ev("PreToolUse", "s2", 1), "ExitPlanMode", None));
        assert_eq!(status_of(&store, "s2"), (Status::NeedsAction, Some("プラン承認待ち".into())));
    }

    #[test]
    fn stop_failure_is_error_with_type() {
        let mut store = SessionStore::default();
        let mut failure = ev("StopFailure", "s1", 1);
        failure.detail.error_type = Some("rate_limit".into());
        store.apply(&failure);
        assert_eq!(status_of(&store, "s1"), (Status::Error, Some("rate_limit".into())));
    }

    #[test]
    fn idle_prompt_does_not_downgrade_needs_action_or_error() {
        let mut store = SessionStore::default();
        store.apply(&tool(ev("PermissionRequest", "s1", 1), "Bash", None));
        let mut idle = ev("Notification", "s1", 2);
        idle.detail.notification_type = Some("idle_prompt".into());
        assert!(store.apply(&idle).is_none());
        assert_eq!(status_of(&store, "s1").0, Status::NeedsAction);
    }

    #[test]
    fn subagent_tool_use_does_not_change_main_status() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "s1", 1));
        let mut start = ev("SubagentStart", "s1", 2);
        start.agent_id = Some("a1".into());
        store.apply(&start);
        let mut inner = tool(ev("PreToolUse", "s1", 3), "Bash", Some("ls"));
        inner.agent_id = Some("a1".into());
        assert!(store.apply(&inner).is_none());
        assert_eq!(status_of(&store, "s1"), (Status::Running, Some("サブエージェント実行中".into())));
    }

    #[test]
    fn subagent_permission_request_needs_action() {
        let mut store = SessionStore::default();
        store.apply(&ev("UserPromptSubmit", "s1", 1));
        let mut request = tool(ev("PermissionRequest", "s1", 2), "Bash", Some("ls"));
        request.agent_id = Some("a1".into());
        store.apply(&request);
        assert_eq!(status_of(&store, "s1").0, Status::NeedsAction);
        let mut resumed = tool(ev("PostToolUse", "s1", 3), "Bash", None);
        resumed.agent_id = Some("a1".into());
        store.apply(&resumed);
        assert_eq!(status_of(&store, "s1").0, Status::Running);
    }

    #[test]
    fn stop_with_active_subagent_mentions_background() {
        let mut store = SessionStore::default();
        let mut start = ev("SubagentStart", "s1", 1);
        start.agent_id = Some("a1".into());
        store.apply(&start);
        store.apply(&ev("Stop", "s1", 2));
        assert_eq!(status_of(&store, "s1"), (Status::Done, Some("バックグラウンド作業あり".into())));
        let mut stop = ev("SubagentStop", "s1", 3);
        stop.agent_id = Some("a1".into());
        store.apply(&stop);
        store.apply(&ev("UserPromptSubmit", "s1", 4));
        store.apply(&ev("Stop", "s1", 5));
        assert_eq!(status_of(&store, "s1"), (Status::Done, None));
    }

    #[test]
    fn stale_event_does_not_override_newer_status() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 10));
        assert!(store.apply(&ev("UserPromptSubmit", "s1", 5)).is_none());
        assert_eq!(status_of(&store, "s1").0, Status::Done);
    }

    #[test]
    fn new_session_on_same_process_ends_previous() {
        let mut store = SessionStore::default();
        store.apply(&ev("SessionStart", "old", 1));
        store.apply(&ev("SessionStart", "new", 2));
        assert_eq!(status_of(&store, "old").0, Status::Ended);
        assert_eq!(status_of(&store, "new").0, Status::Done);
    }

    #[test]
    fn events_after_end_are_ignored_except_session_start() {
        let mut store = SessionStore::default();
        store.apply(&ev("SessionEnd", "s1", 1));
        assert!(store.apply(&ev("UserPromptSubmit", "s1", 2)).is_none());
        assert_eq!(status_of(&store, "s1").0, Status::Ended);
        store.apply(&ev("SessionStart", "s1", 3));
        assert_eq!(status_of(&store, "s1").0, Status::Done);
    }

    #[test]
    fn unknown_event_is_ignored_but_session_is_tracked() {
        let mut store = SessionStore::default();
        assert!(store.apply(&ev("SomethingNew", "s1", 1)).is_none());
        assert_eq!(status_of(&store, "s1").0, Status::Unknown);
    }

    #[test]
    fn summary_counts_and_badge() {
        let mut store = SessionStore::default();
        let mut a = ev("Stop", "a", 1);
        a.claude_pid = Some(1);
        let mut b = ev("StopFailure", "b", 1);
        b.claude_pid = Some(2);
        let mut c = ev("SessionEnd", "c", 1);
        c.claude_pid = Some(3);
        store.apply(&a);
        store.apply(&b);
        store.apply(&c);
        assert_eq!(store.summary(), TraySummary { total: 2, urgent: 1, badge: Some(Status::Error) });
        let mut d = tool(ev("PermissionRequest", "d", 1), "Bash", None);
        d.claude_pid = Some(4);
        store.apply(&d);
        assert_eq!(store.summary(), TraySummary { total: 3, urgent: 2, badge: Some(Status::NeedsAction) });
    }
}
```

`crates/core/tests/fixtures_replay.rs`:

```rust
use std::fs;
use std::path::Path;

use agents_monitor_core::event::MonitorEvent;
use agents_monitor_core::state::Status;
use agents_monitor_core::store::SessionStore;

fn parse_status(name: &str) -> Option<Status> {
    match name {
        "needs_action" => Some(Status::NeedsAction),
        "error" => Some(Status::Error),
        "done" => Some(Status::Done),
        "running" => Some(Status::Running),
        "ended" => Some(Status::Ended),
        _ => None,
    }
}

#[test]
fn recorded_sessions_reach_expected_status() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut checked = 0;
    for entry in fs::read_dir(&dir).expect("Task 6 のテストデータがあること") {
        let path = entry.unwrap().path();
        let file_name = path.file_stem().unwrap().to_string_lossy().into_owned();
        let Some((expected, _)) = file_name.split_once("__") else { continue };
        let expected = parse_status(expected).unwrap_or_else(|| panic!("{file_name}: 不明な状態名"));

        let mut store = SessionStore::default();
        let mut main_session = None;
        for line in fs::read_to_string(&path).unwrap().lines().filter(|l| !l.trim().is_empty()) {
            let event: MonitorEvent = serde_json::from_str(line).unwrap();
            main_session.get_or_insert_with(|| event.session_id.clone());
            store.apply(&event);
        }
        let main_session = main_session.unwrap_or_else(|| panic!("{file_name}: イベントがない"));
        let actual = store.sessions().iter().find(|s| s.id == main_session).unwrap().status;
        assert_eq!(actual, expected, "{file_name}");
        checked += 1;
    }
    assert!(checked > 0, "テストデータが 1 件もない");
}
```

- [ ] **Step 3: テストが失敗することを確認する**

Run: `cargo test -p agents-monitor-core store && cargo test -p agents-monitor-core --test fixtures_replay`
Expected: FAIL（`session_start_creates_new_done_session` など、および再生テスト）

- [ ] **Step 4: store を実装する**

`SessionStore` の `apply` と `summary` を置き換え、残りを追加する:

```rust
impl SessionStore {
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    pub fn apply(&mut self, ev: &MonitorEvent) -> Option<StatusChange> {
        if let Some(pid) = ev.claude_pid {
            self.retire_sessions_sharing_pid(pid, &ev.session_id, ev.ts_ms);
        }
        let (index, is_new) = self.find_or_insert(ev);
        let session = &mut self.sessions[index];
        observe(session, ev);
        let next = transition_for(ev)?;
        if !accepts(session, ev, &next) {
            return None;
        }
        let from = (!is_new).then_some(session.status);
        enter(session, next, ev);
        Some(StatusChange {
            session_id: session.id.clone(),
            project: session.project(),
            from,
            to: session.status,
            detail: session.detail.clone(),
            ts_ms: ev.ts_ms,
        })
    }

    pub fn summary(&self) -> TraySummary {
        let visible: Vec<&Session> = self.sessions.iter().filter(|s| s.status != Status::Ended).collect();
        let has = |status: Status| visible.iter().any(|s| s.status == status);
        TraySummary {
            total: visible.len(),
            urgent: visible.iter().filter(|s| matches!(s.status, Status::NeedsAction | Status::Error)).count(),
            badge: if has(Status::NeedsAction) {
                Some(Status::NeedsAction)
            } else if has(Status::Error) {
                Some(Status::Error)
            } else {
                None
            },
        }
    }

    fn find_or_insert(&mut self, ev: &MonitorEvent) -> (usize, bool) {
        if let Some(index) = self.sessions.iter().position(|s| s.id == ev.session_id) {
            return (index, false);
        }
        self.sessions.push(new_session(ev));
        (self.sessions.len() - 1, true)
    }

    // /clear などで同じ Claude プロセスの session_id が変わると、古い行が残り続けるため
    fn retire_sessions_sharing_pid(&mut self, pid: u32, session_id: &str, ts_ms: u64) {
        self.sessions.retain(|s| !(s.id.starts_with(UNKNOWN_ID_PREFIX) && s.claude_pid == Some(pid)));
        for session in self.sessions.iter_mut() {
            if session.claude_pid == Some(pid) && session.id != session_id && session.status != Status::Ended {
                mark_ended(session, ts_ms);
            }
        }
    }
}

fn new_session(ev: &MonitorEvent) -> Session {
    Session {
        id: ev.session_id.clone(),
        status: Status::Unknown,
        detail: None,
        status_since_ms: ev.ts_ms,
        first_seen_ms: ev.ts_ms,
        last_event_ms: ev.ts_ms,
        cwd: ev.cwd.clone(),
        config_dir: ev.config_dir.clone(),
        git_branch: None,
        claude_pid: ev.claude_pid,
        terminal: None,
        transcript_path: None,
        active_subagents: 0,
    }
}

fn observe(session: &mut Session, ev: &MonitorEvent) {
    session.last_event_ms = session.last_event_ms.max(ev.ts_ms);
    if !ev.cwd.is_empty() {
        session.cwd = ev.cwd.clone();
    }
    session.config_dir = ev.config_dir.clone();
    if ev.claude_pid.is_some() {
        session.claude_pid = ev.claude_pid;
    }
    if ev.git_branch.is_some() {
        session.git_branch = ev.git_branch.clone();
    }
    if ev.terminal.is_some() {
        session.terminal = ev.terminal.clone();
    }
    if ev.transcript_path.is_some() {
        session.transcript_path = ev.transcript_path.clone();
    }
    match ev.event.as_str() {
        "SubagentStart" => session.active_subagents += 1,
        "SubagentStop" => session.active_subagents = session.active_subagents.saturating_sub(1),
        _ => {}
    }
}

fn accepts(session: &Session, ev: &MonitorEvent, next: &Transition) -> bool {
    if ev.ts_ms < session.status_since_ms {
        return false;
    }
    if session.status == Status::Ended && ev.event != "SessionStart" {
        return false;
    }
    let from_subagent = ev.agent_id.is_some() && ev.event != "SubagentStart";
    if from_subagent {
        return next.status == Status::NeedsAction
            || (session.status == Status::NeedsAction && next.status == Status::Running);
    }
    if ev.event == "Notification"
        && next.status == Status::Done
        && matches!(session.status, Status::NeedsAction | Status::Error)
    {
        return false;
    }
    true
}

fn enter(session: &mut Session, next: Transition, ev: &MonitorEvent) {
    if session.status != next.status {
        session.status_since_ms = ev.ts_ms;
    }
    session.status = next.status;
    session.detail = if next.status == Status::Done && ev.event == "Stop" && session.active_subagents > 0 {
        Some("バックグラウンド作業あり".into())
    } else {
        next.detail
    };
}

pub(crate) fn mark_ended(session: &mut Session, ts_ms: u64) {
    session.status = Status::Ended;
    session.status_since_ms = ts_ms;
    session.detail = None;
}
```

- [ ] **Step 5: テストが通ることを確認する**

Run: `cargo test -p agents-monitor-core`
Expected: PASS（store 14 tests と fixtures_replay を含む全テスト）。再生テストだけが失敗する場合は、失敗したシナリオのイベント列と Task 6 のメモを見比べ、`transition_for` / `accepts` を直す（テストデータ側の期待値は変えない。`running__esc-interrupt` は Task 8 の中断検知で扱うので、ここでは `running` になれば正しい）。

- [ ] **Step 6: コミット**

```bash
git add crates/core
git commit -m "core: 状態遷移とセッション管理を追加"
```

---

### Task 8: プロセスの照合、中断の検知、終了セッションの掃除

**Files:**
- Create: `crates/core/src/transcript.rs`
- Modify: `crates/core/src/store.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `ps::{ProcRow, is_interactive_session}`、`store::mark_ended`
- Produces:
  - `store::SessionStore::reconcile(&mut self, rows: &[ProcRow], now_ms: u64) -> bool`
  - `store::SessionStore::detect_interrupts(&mut self, now_ms: u64, quiet_ms: u64, ends_with_interrupt: impl Fn(&Path) -> bool) -> bool`
  - `store::SessionStore::prune(&mut self, now_ms: u64) -> bool`
  - `store::SessionStore::dismiss(&mut self, id: &str) -> bool`
  - `store::ENDED_RETENTION_MS: u64 = 300_000`、`store::INTERRUPT_QUIET_MS: u64 = 15_000`
  - `transcript::transcript_ends_with_interrupt(path: &Path) -> bool`、`transcript::INTERRUPT_MARKER`

（Task 6 で「Esc でも Stop が届く」と分かった場合は、`transcript.rs` と `detect_interrupts` とそのテストを作らない）

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/lib.rs` に追加:

```rust
pub mod transcript;
```

`crates/core/src/transcript.rs`:

```rust
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

// Task 6 の調査で会話ログに残ると確認した中断の目印
pub const INTERRUPT_MARKER: &str = "[Request interrupted by user";
const TAIL_BYTES: u64 = 8 * 1024;

pub fn transcript_ends_with_interrupt(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_marker_on_last_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.jsonl");
        fs::write(&path, format!("{{\"a\":1}}\n{{\"text\":\"{INTERRUPT_MARKER}]\"}}\n")).unwrap();
        assert!(transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn ignores_marker_on_earlier_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.jsonl");
        fs::write(&path, format!("{{\"text\":\"{INTERRUPT_MARKER}]\"}}\n{{\"a\":1}}\n")).unwrap();
        assert!(!transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn missing_file_is_false() {
        assert!(!transcript_ends_with_interrupt(Path::new("/nonexistent/t.jsonl")));
    }
}
```

`crates/core/src/store.rs` の先頭に定数を追加し、`impl SessionStore` に空のメソッドを追加:

```rust
use std::path::Path;

use crate::ps::{is_interactive_session, ProcRow};

pub const ENDED_RETENTION_MS: u64 = 5 * 60 * 1000;
pub const INTERRUPT_QUIET_MS: u64 = 15_000;
```

```rust
    pub fn reconcile(&mut self, _rows: &[ProcRow], _now_ms: u64) -> bool {
        false
    }

    pub fn detect_interrupts(&mut self, _now_ms: u64, _quiet_ms: u64, _ends_with_interrupt: impl Fn(&Path) -> bool) -> bool {
        false
    }

    pub fn prune(&mut self, _now_ms: u64) -> bool {
        false
    }

    pub fn dismiss(&mut self, _id: &str) -> bool {
        false
    }
```

`store.rs` の `mod tests` に追加:

```rust
    fn proc(pid: u32, args: &str) -> ProcRow {
        ProcRow { pid, ppid: 1, tty: None, args: args.into() }
    }

    #[test]
    fn dead_process_ends_session() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1));
        assert!(store.reconcile(&[proc(999, "claude")], 50));
        assert_eq!(status_of(&store, "s1").0, Status::Ended);
        assert_eq!(store.sessions().iter().find(|s| s.id == "s1").unwrap().status_since_ms, 50);
    }

    #[test]
    fn untracked_interactive_claude_becomes_unknown_row() {
        let mut store = SessionStore::default();
        let rows = [proc(500, "/Users/u/.local/bin/claude"), proc(501, "claude -p hi"), proc(502, "/bin/zsh")];
        assert!(store.reconcile(&rows, 10));
        let ids: Vec<_> = store.sessions().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["pid:500"]);
        assert_eq!(status_of(&store, "pid:500").0, Status::Unknown);
        assert!(!store.reconcile(&rows, 11), "2 回目は変化なし");
    }

    #[test]
    fn unknown_row_disappears_with_process_or_real_event() {
        let mut store = SessionStore::default();
        store.reconcile(&[proc(100, "claude")], 1);
        store.apply(&ev("SessionStart", "s1", 2));
        let ids: Vec<_> = store.sessions().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["s1"]);

        let mut other = SessionStore::default();
        other.reconcile(&[proc(700, "claude")], 1);
        assert!(other.reconcile(&[], 2));
        assert!(other.sessions().is_empty());
    }

    #[test]
    fn dismissed_unknown_process_is_not_readded() {
        let mut store = SessionStore::default();
        store.reconcile(&[proc(700, "claude")], 1);
        assert!(store.dismiss("pid:700"));
        store.reconcile(&[proc(700, "claude")], 2);
        assert!(store.sessions().is_empty());
    }

    #[test]
    fn dismiss_only_removes_ended_or_unknown() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1));
        assert!(!store.dismiss("s1"));
        store.apply(&ev("SessionEnd", "s1", 2));
        assert!(store.dismiss("s1"));
        assert!(store.sessions().is_empty());
    }

    #[test]
    fn prune_removes_sessions_ended_long_ago() {
        let mut store = SessionStore::default();
        store.apply(&ev("SessionEnd", "s1", 1_000));
        assert!(!store.prune(1_000 + ENDED_RETENTION_MS - 1));
        assert!(store.prune(1_000 + ENDED_RETENTION_MS));
        assert!(store.sessions().is_empty());
    }

    #[test]
    fn quiet_running_session_with_interrupt_marker_becomes_done() {
        let mut store = SessionStore::default();
        let mut prompt = ev("UserPromptSubmit", "s1", 1_000);
        prompt.transcript_path = Some("/t/s1.jsonl".into());
        store.apply(&prompt);
        assert!(!store.detect_interrupts(1_000 + INTERRUPT_QUIET_MS - 1, INTERRUPT_QUIET_MS, |_| true));
        assert!(!store.detect_interrupts(1_000 + INTERRUPT_QUIET_MS, INTERRUPT_QUIET_MS, |_| false));
        assert!(store.detect_interrupts(1_000 + INTERRUPT_QUIET_MS, INTERRUPT_QUIET_MS, |p| p == Path::new("/t/s1.jsonl")));
        assert_eq!(status_of(&store, "s1"), (Status::Done, Some("中断".into())));
    }
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p agents-monitor-core -- store:: transcript::`
Expected: FAIL（`dead_process_ends_session`、`detects_marker_on_last_line` など）

- [ ] **Step 3: 本体を実装する**

`transcript.rs`:

```rust
pub fn transcript_ends_with_interrupt(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else { return false };
    let Ok(len) = file.metadata().map(|m| m.len()) else { return false };
    if file.seek(SeekFrom::Start(len.saturating_sub(TAIL_BYTES))).is_err() {
        return false;
    }
    let mut tail = Vec::new();
    if file.read_to_end(&mut tail).is_err() {
        return false;
    }
    let tail = String::from_utf8_lossy(&tail);
    tail.lines().rev().find(|line| !line.trim().is_empty()).is_some_and(|line| line.contains(INTERRUPT_MARKER))
}
```

`store.rs` の 4 つのメソッドを置き換える:

```rust
    pub fn reconcile(&mut self, rows: &[ProcRow], now_ms: u64) -> bool {
        let alive: HashSet<u32> = rows.iter().map(|r| r.pid).collect();
        let before = self.sessions.len();
        self.sessions.retain(|s| {
            !(s.id.starts_with(UNKNOWN_ID_PREFIX) && !s.claude_pid.is_some_and(|pid| alive.contains(&pid)))
        });
        let mut changed = before != self.sessions.len();
        for session in self.sessions.iter_mut() {
            let dead = session.claude_pid.is_some_and(|pid| !alive.contains(&pid));
            if dead && session.status != Status::Ended {
                mark_ended(session, now_ms);
                changed = true;
            }
        }
        self.dismissed_pids.retain(|pid| alive.contains(pid));
        let known: HashSet<u32> = self
            .sessions
            .iter()
            .filter(|s| s.status != Status::Ended)
            .filter_map(|s| s.claude_pid)
            .collect();
        for row in rows {
            if is_interactive_session(&row.args) && !known.contains(&row.pid) && !self.dismissed_pids.contains(&row.pid) {
                self.sessions.push(unknown_process_session(row.pid, now_ms));
                changed = true;
            }
        }
        changed
    }

    pub fn detect_interrupts(&mut self, now_ms: u64, quiet_ms: u64, ends_with_interrupt: impl Fn(&Path) -> bool) -> bool {
        let mut changed = false;
        for session in self.sessions.iter_mut() {
            let quiet = now_ms.saturating_sub(session.last_event_ms) >= quiet_ms;
            if session.status != Status::Running || !quiet {
                continue;
            }
            let Some(transcript) = session.transcript_path.as_deref() else { continue };
            if ends_with_interrupt(Path::new(transcript)) {
                session.status = Status::Done;
                session.status_since_ms = now_ms;
                session.detail = Some("中断".into());
                changed = true;
            }
        }
        changed
    }

    pub fn prune(&mut self, now_ms: u64) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|s| {
            !(s.status == Status::Ended && now_ms.saturating_sub(s.status_since_ms) >= ENDED_RETENTION_MS)
        });
        before != self.sessions.len()
    }

    pub fn dismiss(&mut self, id: &str) -> bool {
        let Some(index) = self
            .sessions
            .iter()
            .position(|s| s.id == id && matches!(s.status, Status::Ended | Status::Unknown))
        else {
            return false;
        };
        let removed = self.sessions.remove(index);
        if let Some(pid) = removed.claude_pid {
            self.dismissed_pids.insert(pid);
        }
        true
    }
```

ファイル末尾（`mark_ended` の下）に追加:

```rust
fn unknown_process_session(pid: u32, now_ms: u64) -> Session {
    Session {
        id: format!("{UNKNOWN_ID_PREFIX}{pid}"),
        status: Status::Unknown,
        detail: Some("開き直すと監視されます".into()),
        status_since_ms: now_ms,
        first_seen_ms: now_ms,
        last_event_ms: now_ms,
        cwd: String::new(),
        config_dir: String::new(),
        git_branch: None,
        claude_pid: Some(pid),
        terminal: None,
        transcript_path: None,
        active_subagents: 0,
    }
}
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p agents-monitor-core`
Expected: PASS（全テスト）

- [ ] **Step 5: コミット**

```bash
git add crates/core
git commit -m "core: プロセスの照合、中断の検知、終了セッションの掃除を追加"
```

---

### Task 9: 画面用データと通知文

**Files:**
- Create: `crates/core/src/view.rs`
- Create: `crates/core/src/notify.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `state::{Session, Status}`、`store::StatusChange`
- Produces:
  - `view::SessionView { id, status: Status, project, detail: Option<String>, status_since_ms: u64, branch: Option<String>, profile: Option<String>, cwd, config_dir, claude_pid: Option<u32> }`（`Serialize`）
  - `view::session_views(sessions: &[Session]) -> Vec<SessionView>`
  - `notify::Notice { title: String, body: String }`
  - `notify::notice_for(change: &StatusChange, notify_on: &[Status]) -> Option<Notice>`

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/lib.rs` に追加:

```rust
pub mod notify;
pub mod view;
```

`crates/core/src/view.rs`:

```rust
use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;

use crate::state::{Session, Status};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionView {
    pub id: String,
    pub status: Status,
    pub project: String,
    pub detail: Option<String>,
    pub status_since_ms: u64,
    pub branch: Option<String>,
    pub profile: Option<String>,
    pub cwd: String,
    pub config_dir: String,
    pub claude_pid: Option<u32>,
}

pub fn session_views(_sessions: &[Session]) -> Vec<SessionView> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::ev;
    use crate::store::SessionStore;

    fn store_with(events: &[(&str, &str, &str, &str, u32)]) -> SessionStore {
        let mut store = SessionStore::default();
        for (i, (id, cwd, config_dir, branch, pid)) in events.iter().enumerate() {
            let mut e = ev("SessionStart", id, i as u64 + 1);
            e.cwd = cwd.to_string();
            e.config_dir = config_dir.to_string();
            e.git_branch = Some(branch.to_string());
            e.claude_pid = Some(*pid);
            store.apply(&e);
        }
        store
    }

    #[test]
    fn keeps_order_and_hides_branch_and_profile_when_unambiguous() {
        let store = store_with(&[("a", "/w/api", "/u/.claude", "main", 1), ("b", "/w/web", "/u/.claude", "dev", 2)]);
        let views = session_views(store.sessions());
        assert_eq!(views.iter().map(|v| v.project.as_str()).collect::<Vec<_>>(), ["api", "web"]);
        assert!(views.iter().all(|v| v.branch.is_none() && v.profile.is_none()));
    }

    #[test]
    fn shows_branch_when_same_project_runs_twice() {
        let store = store_with(&[("a", "/w/api", "/u/.claude", "main", 1), ("b", "/w/api", "/u/.claude", "fix", 2)]);
        let views = session_views(store.sessions());
        assert_eq!(views[0].branch.as_deref(), Some("main"));
        assert_eq!(views[1].branch.as_deref(), Some("fix"));
    }

    #[test]
    fn shows_profile_when_multiple_config_dirs_are_active() {
        let store = store_with(&[("a", "/w/api", "/u/.claude", "main", 1), ("b", "/w/web", "/u/.claude-personal", "main", 2)]);
        let views = session_views(store.sessions());
        assert_eq!(views[0].profile.as_deref(), Some("claude"));
        assert_eq!(views[1].profile.as_deref(), Some("claude-personal"));
    }
}
```

`crates/core/src/notify.rs`:

```rust
use crate::state::Status;
use crate::store::StatusChange;

#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

pub fn notice_for(_change: &StatusChange, _notify_on: &[Status]) -> Option<Notice> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(from: Option<Status>, to: Status, detail: Option<&str>) -> StatusChange {
        StatusChange {
            session_id: "s1".into(),
            project: "my-api".into(),
            from,
            to,
            detail: detail.map(str::to_string),
            ts_ms: 1,
        }
    }

    const DEFAULT: [Status; 2] = [Status::NeedsAction, Status::Error];

    #[test]
    fn permission_notice_splits_title_and_body() {
        let notice = notice_for(&change(Some(Status::Running), Status::NeedsAction, Some("許可待ち · Bash: rm -rf dist")), &DEFAULT).unwrap();
        assert_eq!(notice, Notice { title: "my-api — 許可待ち".into(), body: "Bash: rm -rf dist".into() });
    }

    #[test]
    fn question_notice_without_body() {
        let notice = notice_for(&change(Some(Status::Running), Status::NeedsAction, Some("質問待ち")), &DEFAULT).unwrap();
        assert_eq!(notice, Notice { title: "my-api — 質問待ち".into(), body: String::new() });
    }

    #[test]
    fn error_notice_uses_status_label() {
        let notice = notice_for(&change(Some(Status::Running), Status::Error, Some("rate_limit")), &DEFAULT).unwrap();
        assert_eq!(notice, Notice { title: "my-api — エラー停止".into(), body: "rate_limit".into() });
    }

    #[test]
    fn no_notice_when_status_unchanged_or_not_selected() {
        assert!(notice_for(&change(Some(Status::NeedsAction), Status::NeedsAction, Some("許可待ち")), &DEFAULT).is_none());
        assert!(notice_for(&change(Some(Status::Running), Status::Done, None), &DEFAULT).is_none());
        assert!(notice_for(&change(Some(Status::Running), Status::Done, None), &[Status::Done]).is_some());
    }
}
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p agents-monitor-core -- view:: notify::`
Expected: FAIL

- [ ] **Step 3: 本体を実装する**

`view.rs` の関数を置き換える:

```rust
pub fn session_views(sessions: &[Session]) -> Vec<SessionView> {
    let visible: Vec<&Session> = sessions.iter().collect();
    let config_dirs: HashSet<&str> = visible
        .iter()
        .filter(|s| s.status != Status::Ended && !s.config_dir.is_empty())
        .map(|s| s.config_dir.as_str())
        .collect();
    let show_profile = config_dirs.len() > 1;
    visible
        .iter()
        .map(|session| {
            let shares_project = !session.cwd.is_empty()
                && visible.iter().filter(|other| other.cwd == session.cwd && other.status != Status::Ended).count() > 1;
            SessionView {
                id: session.id.clone(),
                status: session.status,
                project: session.project(),
                detail: session.detail.clone(),
                status_since_ms: session.status_since_ms,
                branch: if shares_project { session.git_branch.clone() } else { None },
                profile: if show_profile && !session.config_dir.is_empty() { Some(profile_name(&session.config_dir)) } else { None },
                cwd: session.cwd.clone(),
                config_dir: session.config_dir.clone(),
                claude_pid: session.claude_pid,
            }
        })
        .collect()
}

fn profile_name(config_dir: &str) -> String {
    let name = Path::new(config_dir).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    name.trim_start_matches('.').to_string()
}
```

`notify.rs` の関数を置き換える:

```rust
pub fn notice_for(change: &StatusChange, notify_on: &[Status]) -> Option<Notice> {
    if change.from == Some(change.to) || !notify_on.contains(&change.to) {
        return None;
    }
    let (headline, body) = match (change.to, change.detail.as_deref()) {
        (Status::NeedsAction, Some(detail)) => match detail.split_once(" · ") {
            Some((head, tail)) => (head.to_string(), tail.to_string()),
            None => (detail.to_string(), String::new()),
        },
        (status, detail) => (status.label().to_string(), detail.unwrap_or_default().to_string()),
    };
    Some(Notice { title: format!("{} — {headline}", change.project), body })
}
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p agents-monitor-core`
Expected: PASS

- [ ] **Step 5: コミット**

```bash
git add crates/core
git commit -m "core: 画面用データと通知文の組み立てを追加"
```

---

### Task 10: 設定・状態の保存と hook プログラムの配置

**Files:**
- Create: `crates/core/src/settings.rs`
- Create: `crates/core/src/persist.rs`
- Create: `crates/core/src/install.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces:
  - `settings::Settings { notify_on: Vec<Status>, launch_at_login: bool }`（`Default`: `[NeedsAction, Error]`、`false`。serde は欠けた項目を既定値で補う）
  - `persist::load_json<T: DeserializeOwned + Default>(path: &Path) -> T`
  - `persist::save_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()>`
  - `install::install_hook_binary(source: &Path, dest: &Path) -> io::Result<bool>`（内容が同じなら何もせず `false`）

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/lib.rs` に追加:

```rust
pub mod install;
pub mod persist;
pub mod settings;
```

`crates/core/src/settings.rs`:

```rust
use serde::{Deserialize, Serialize};

use crate::state::Status;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub notify_on: Vec<Status>,
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { notify_on: vec![Status::NeedsAction, Status::Error], launch_at_login: false }
    }
}
```

`crates/core/src/persist.rs`:

```rust
use std::fs;
use std::io;
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

pub fn load_json<T: DeserializeOwned + Default>(_path: &Path) -> T {
    T::default()
}

pub fn save_json<T: Serialize>(_path: &Path, _value: &T) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use crate::state::Status;
    use crate::store::test_support::ev;
    use crate::store::SessionStore;

    #[test]
    fn round_trips_settings_and_store() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings { notify_on: vec![Status::Done], launch_at_login: true };
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
        assert_eq!(load_json::<Settings>(&dir.path().join("none.json")), Settings::default());
        fs::write(dir.path().join("broken.json"), "{oops").unwrap();
        assert_eq!(load_json::<Settings>(&dir.path().join("broken.json")), Settings::default());
    }

    #[test]
    fn partial_settings_keep_defaults_for_missing_fields() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("s.json"), r#"{"launch_at_login":true}"#).unwrap();
        let loaded: Settings = load_json(&dir.path().join("s.json"));
        assert!(loaded.launch_at_login);
        assert_eq!(loaded.notify_on, Settings::default().notify_on);
    }
}
```

`crates/core/src/install.rs`:

```rust
use std::fs;
use std::io;
use std::path::Path;

pub fn install_hook_binary(_source: &Path, _dest: &Path) -> io::Result<bool> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_executable_and_skips_when_identical() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("src-hook");
        fs::write(&source, b"v1").unwrap();
        let dest = dir.path().join("home/bin/agents-monitor-hook");
        assert!(install_hook_binary(&source, &dest).unwrap());
        assert_eq!(fs::read(&dest).unwrap(), b"v1");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&dest).unwrap().permissions().mode() & 0o777, 0o755);
        }
        assert!(!install_hook_binary(&source, &dest).unwrap());
        fs::write(&source, b"v2").unwrap();
        assert!(install_hook_binary(&source, &dest).unwrap());
        assert_eq!(fs::read(&dest).unwrap(), b"v2");
    }

    #[test]
    fn missing_source_is_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(install_hook_binary(&dir.path().join("none"), &dir.path().join("dest")).is_err());
    }
}
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p agents-monitor-core -- persist:: install::`
Expected: FAIL

- [ ] **Step 3: 本体を実装する**

`persist.rs`:

```rust
pub fn load_json<T: DeserializeOwned + Default>(path: &Path) -> T {
    fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

pub fn save_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    fs::rename(&tmp, path)
}
```

`install.rs`:

```rust
pub fn install_hook_binary(source: &Path, dest: &Path) -> io::Result<bool> {
    let new_bytes = fs::read(source)?;
    if fs::read(dest).ok().as_deref() == Some(new_bytes.as_slice()) {
        return Ok(false);
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    // hook が実行中でも安全に差し替えられるよう、別名で書いてから置き換える
    let tmp = dest.with_extension("tmp");
    fs::write(&tmp, &new_bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&tmp, dest)?;
    Ok(true)
}
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p agents-monitor-core`
Expected: PASS

- [ ] **Step 5: コミット**

```bash
git add crates/core
git commit -m "core: 設定と状態の保存、hook プログラムの配置を追加"
```

---

### Task 11: Tauri アプリの骨組みとメニューバーアイコンの描画

**Files:**
- Create: `app/package.json`、`app/tsconfig.json`、`app/vite.config.ts`、`app/index.html`、`app/settings.html`、`app/src/types.ts`、`app/src/style.css`
- Create: `app/src-tauri/Cargo.toml`、`app/src-tauri/build.rs`、`app/src-tauri/tauri.conf.json`、`app/src-tauri/capabilities/default.json`
- Create: `app/src-tauri/src/main.rs`、`app/src-tauri/src/tray_icon.rs`
- Create: `scripts/build-hook-sidecar.sh`、`scripts/make-app-icon.mjs`
- Modify: `Cargo.toml`、`.gitignore`

**Interfaces:**
- Consumes: `state::Status`
- Produces:
  - `tray_icon::render_tray_icon(badge: Option<(usize, Status)>) -> (Vec<u8>, u32, u32)`（RGBA、36×36）
  - `tray_icon::badge_text(count: usize) -> String`
  - フロントエンドの型 `app/src/types.ts`: `Status`、`SessionView`、`Settings`、`STATUS_LABEL`

- [ ] **Step 1: フロントエンドの土台を作る**

```bash
mkdir -p app/src && cd app
npm init -y >/dev/null
npm install @tauri-apps/api@^2
npm install -D @tauri-apps/cli@^2 typescript vite
cd ..
```

`app/package.json` を次の内容にする（`dependencies` / `devDependencies` のバージョンは npm install が書いたものを残す）:

```json
{
  "name": "agents-monitor-app",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "tauri": "tauri"
  }
}
```

`app/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "strict": true,
    "noEmit": true,
    "skipLibCheck": true
  },
  "include": ["src", "vite.config.ts"]
}
```

`app/vite.config.ts`:

```ts
import { resolve } from "node:path";
import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        settings: resolve(__dirname, "settings.html"),
      },
    },
  },
});
```

`app/src/types.ts`:

```ts
export type Status = "needs_action" | "error" | "done" | "running" | "ended" | "unknown";

export interface SessionView {
  id: string;
  status: Status;
  project: string;
  detail: string | null;
  status_since_ms: number;
  branch: string | null;
  profile: string | null;
  cwd: string;
  config_dir: string;
  claude_pid: number | null;
}

export interface Settings {
  notify_on: Status[];
  launch_at_login: boolean;
}

export const STATUS_LABEL: Record<Status, string> = {
  needs_action: "要対応",
  error: "エラー停止",
  done: "完了",
  running: "実行中",
  ended: "終了",
  unknown: "状態不明",
};
```

`app/index.html`:

```html
<!doctype html>
<html lang="ja">
  <head>
    <meta charset="UTF-8" />
    <title>agents-monitor</title>
    <link rel="stylesheet" href="/src/style.css" />
  </head>
  <body class="list-window">
    <ul id="sessions" class="sessions"></ul>
    <footer class="footer">
      <button id="open-settings" type="button">設定…</button>
      <button id="quit" type="button">終了</button>
    </footer>
    <script type="module" src="/src/list.ts"></script>
  </body>
</html>
```

`app/settings.html`:

```html
<!doctype html>
<html lang="ja">
  <head>
    <meta charset="UTF-8" />
    <title>agents-monitor 設定</title>
    <link rel="stylesheet" href="/src/style.css" />
  </head>
  <body class="settings-window">
    <h1>設定</h1>
    <fieldset>
      <legend>通知する状態</legend>
      <div id="notify-options"></div>
    </fieldset>
    <label class="check"><input id="launch-at-login" type="checkbox" /> ログイン時に起動する</label>
    <p id="settings-error" class="error-text" hidden></p>
    <script type="module" src="/src/settings.ts"></script>
  </body>
</html>
```

`app/src/style.css`:

```css
:root {
  --bg: #f6f6f8;
  --fg: #1c1c1e;
  --muted: #6e6e73;
  --hover: rgba(0, 0, 0, 0.05);
  --line: rgba(0, 0, 0, 0.08);
  --needs-action: #f76b15;
  --error: #e5484d;
  --done: #3e63dd;
  --running: #30a46c;
  --ended: #8b8d98;
  color-scheme: light dark;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #1f1f22;
    --fg: #ececee;
    --muted: #a0a0a8;
    --hover: rgba(255, 255, 255, 0.06);
    --line: rgba(255, 255, 255, 0.1);
  }
}
* { box-sizing: border-box; }
body {
  margin: 0;
  background: var(--bg);
  color: var(--fg);
  font: 13px -apple-system, BlinkMacSystemFont, "Hiragino Sans", sans-serif;
  -webkit-user-select: none;
  user-select: none;
}
.sessions { list-style: none; margin: 0; padding: 6px 0; max-height: 380px; overflow-y: auto; }
.row { padding: 6px 14px; cursor: default; }
.row:hover { background: var(--hover); }
.row-main { display: flex; gap: 9px; align-items: flex-start; }
.status-needs_action { background: color-mix(in srgb, var(--needs-action) 10%, transparent); }
.status-error { background: color-mix(in srgb, var(--error) 10%, transparent); }
.dot { width: 9px; height: 9px; border-radius: 50%; margin-top: 4px; flex: none; }
.dot-needs_action { background: var(--needs-action); }
.dot-error { background: var(--error); }
.dot-done { background: var(--done); }
.dot-running { background: var(--running); }
.dot-ended { background: var(--ended); }
.dot-unknown { border: 1.5px solid var(--ended); }
.text { min-width: 0; flex: 1; }
.project { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.meta { font-size: 11.5px; color: var(--muted); margin-top: 1px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.elapsed { font-size: 11.5px; color: var(--muted); white-space: nowrap; }
.details { margin: 6px 0 2px 18px; font-size: 11.5px; color: var(--muted); display: grid; gap: 2px; -webkit-user-select: text; user-select: text; }
.details button { justify-self: start; margin-top: 4px; }
.empty { padding: 16px 14px; color: var(--muted); }
.footer { display: flex; justify-content: space-between; border-top: 1px solid var(--line); padding: 6px 10px; }
.footer button, .details button { background: none; border: none; color: var(--fg); font: inherit; padding: 2px 4px; border-radius: 4px; }
.footer button:hover, .details button:hover { background: var(--hover); }
.settings-window { padding: 16px 20px; }
.settings-window h1 { font-size: 15px; margin: 0 0 12px; }
fieldset { border: 1px solid var(--line); border-radius: 6px; margin: 0 0 12px; }
.check { display: flex; gap: 6px; align-items: center; margin: 4px 0; }
.error-text { color: var(--error); }
```

`app/src/list.ts` と `app/src/settings.ts` は Task 12・13 で書く。ここでは空のファイルを作る:

```bash
printf 'export {};\n' > app/src/list.ts
printf 'export {};\n' > app/src/settings.ts
```

- [ ] **Step 2: Rust 側の土台を作る**

`Cargo.toml` の members を更新:

```toml
[workspace]
resolver = "2"
members = ["crates/core", "crates/hook", "app/src-tauri"]
```

`.gitignore` に追記:

```
/app/node_modules/
/app/dist/
/app/src-tauri/binaries/
/app/src-tauri/gen/
```

`app/src-tauri/Cargo.toml`:

```toml
[package]
name = "agents-monitor-app"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
agents-monitor-core = { path = "../../crates/core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-autostart = "2"
tauri-plugin-notification = "2"
tauri-plugin-positioner = { version = "2", features = ["tray-icon"] }
```

`app/src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build()
}
```

`app/src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "agents-monitor",
  "version": "0.1.0",
  "identifier": "io.github.imatomix.agentsmonitor",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "label": "list",
        "url": "index.html",
        "width": 340,
        "height": 440,
        "visible": false,
        "decorations": false,
        "resizable": false,
        "alwaysOnTop": true,
        "skipTaskbar": true
      },
      {
        "label": "settings",
        "url": "settings.html",
        "title": "agents-monitor 設定",
        "width": 420,
        "height": 300,
        "visible": false,
        "resizable": false
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": true,
    "targets": ["app", "dmg"],
    "icon": ["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.png"],
    "externalBin": ["binaries/agents-monitor-hook"]
  }
}
```

`app/src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "windows": ["list", "settings"],
  "permissions": ["core:default"]
}
```

`scripts/build-hook-sidecar.sh`:

```bash
#!/usr/bin/env bash
# Tauri の externalBin は「名前-ターゲットトリプル」のファイルを要求する
set -euo pipefail
cd "$(dirname "$0")/.."
triple="$(rustc -vV | sed -n 's/^host: //p')"
cargo build --release -p agents-monitor-hook
mkdir -p app/src-tauri/binaries
cp target/release/agents-monitor-hook "app/src-tauri/binaries/agents-monitor-hook-$triple"
echo "sidecar: app/src-tauri/binaries/agents-monitor-hook-$triple"
```

`scripts/make-app-icon.mjs`（外部ライブラリなしで 1024×1024 の PNG を作る）:

```js
import { writeFileSync } from "node:fs";
import { crc32, deflateSync } from "node:zlib";

const size = 1024;
const pixels = Buffer.alloc((size * 4 + 1) * size);
const center = size / 2;
for (let y = 0; y < size; y++) {
  const rowStart = y * (size * 4 + 1);
  pixels[rowStart] = 0;
  for (let x = 0; x < size; x++) {
    const d = Math.hypot(x + 0.5 - center, y + 0.5 - center);
    const ring = d < 300 && d > 220;
    const background = [31, 41, 51, 255];
    const color = ring ? [48, 164, 108, 255] : background;
    color.forEach((v, i) => (pixels[rowStart + 1 + x * 4 + i] = v));
  }
}
const chunk = (type, data) => {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
};
const header = Buffer.alloc(13);
header.writeUInt32BE(size, 0);
header.writeUInt32BE(size, 4);
header[8] = 8;
header[9] = 6;
const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", header),
  chunk("IDAT", deflateSync(pixels)),
  chunk("IEND", Buffer.alloc(0)),
]);
writeFileSync(process.argv[2], png);
```

```bash
chmod +x scripts/build-hook-sidecar.sh
node scripts/make-app-icon.mjs /tmp/agents-monitor-icon.png
cd app && npx tauri icon /tmp/agents-monitor-icon.png -o src-tauri/icons && cd ..
./scripts/build-hook-sidecar.sh
```

Expected: `app/src-tauri/icons/` に `icon.icns` などができ、`app/src-tauri/binaries/` に sidecar ができる。

- [ ] **Step 3: アイコン描画の失敗するテストを書く**

`app/src-tauri/src/tray_icon.rs`:

```rust
use agents_monitor_core::state::Status;

pub const ICON_SIZE: u32 = 36;

pub fn render_tray_icon(_badge: Option<(usize, Status)>) -> (Vec<u8>, u32, u32) {
    (vec![0; (ICON_SIZE * ICON_SIZE * 4) as usize], ICON_SIZE, ICON_SIZE)
}

pub fn badge_text(_count: usize) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
        let i = ((y * ICON_SIZE + x) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
    }

    #[test]
    fn badge_text_caps_at_nine_plus() {
        assert_eq!(badge_text(3), "3");
        assert_eq!(badge_text(9), "9");
        assert_eq!(badge_text(10), "9+");
    }

    #[test]
    fn base_icon_has_ring_and_no_badge() {
        let (rgba, w, h) = render_tray_icon(None);
        assert_eq!((w, h, rgba.len()), (ICON_SIZE, ICON_SIZE, (ICON_SIZE * ICON_SIZE * 4) as usize));
        assert_eq!(pixel(&rgba, 16, 10), GLYPH, "リングの上端");
        assert_eq!(pixel(&rgba, 30, 6)[3], 0, "バッジ位置は透明");
    }

    #[test]
    fn badge_color_follows_status() {
        let (orange, _, _) = render_tray_icon(Some((3, Status::NeedsAction)));
        let (red, _, _) = render_tray_icon(Some((1, Status::Error)));
        assert_eq!(pixel(&orange, 30, 6), ORANGE);
        assert_eq!(pixel(&red, 30, 6), RED);
    }

    #[test]
    fn badge_contains_white_digit_pixels() {
        let (rgba, _, _) = render_tray_icon(Some((8, Status::NeedsAction)));
        let white = (0..ICON_SIZE).flat_map(|y| (0..ICON_SIZE).map(move |x| (x, y))).filter(|&(x, y)| pixel(&rgba, x, y) == WHITE).count();
        assert!(white > 20, "white pixels: {white}");
    }
}
```

`app/src-tauri/src/main.rs`（このタスクでは起動確認用の最小限）:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray_icon;

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run agents-monitor");
}
```

- [ ] **Step 4: テストが失敗することを確認する**

Run: `cargo test -p agents-monitor-app tray_icon`
Expected: コンパイルエラー（`GLYPH` / `ORANGE` / `RED` / `WHITE` が未定義）。これが「失敗」に当たる。

- [ ] **Step 5: 本体を実装する**

`tray_icon.rs` の関数を置き換え、定数と描画処理を追加する:

```rust
pub const GLYPH: [u8; 4] = [142, 142, 147, 255];
pub const ORANGE: [u8; 4] = [247, 107, 21, 255];
pub const RED: [u8; 4] = [229, 72, 77, 255];
pub const WHITE: [u8; 4] = [255, 255, 255, 255];

const RING_CENTER: (f32, f32) = (16.0, 21.0);
const RING_RADIUS: f32 = 11.0;
const RING_THICKNESS: f32 = 3.0;
const BADGE_CENTER: (i32, i32) = (26, 10);
const BADGE_RADIUS: f32 = 10.0;
const GLYPH_SCALE: i32 = 2;

// 3×5 のビットマップ数字。各行の下位 3 ビットが左から右の画素
const FONT: [(char, [u8; 5]); 11] = [
    ('0', [0b111, 0b101, 0b101, 0b101, 0b111]),
    ('1', [0b010, 0b110, 0b010, 0b010, 0b111]),
    ('2', [0b111, 0b001, 0b111, 0b100, 0b111]),
    ('3', [0b111, 0b001, 0b111, 0b001, 0b111]),
    ('4', [0b101, 0b101, 0b111, 0b001, 0b001]),
    ('5', [0b111, 0b100, 0b111, 0b001, 0b111]),
    ('6', [0b111, 0b100, 0b111, 0b101, 0b111]),
    ('7', [0b111, 0b001, 0b010, 0b010, 0b010]),
    ('8', [0b111, 0b101, 0b111, 0b101, 0b111]),
    ('9', [0b111, 0b101, 0b111, 0b001, 0b111]),
    ('+', [0b000, 0b010, 0b111, 0b010, 0b000]),
];

pub fn render_tray_icon(badge: Option<(usize, Status)>) -> (Vec<u8>, u32, u32) {
    let mut canvas = Canvas::new(ICON_SIZE);
    canvas.fill_where(GLYPH, |x, y| {
        let d = distance((x, y), RING_CENTER);
        d <= RING_RADIUS && d >= RING_RADIUS - RING_THICKNESS
    });
    if let Some((count, status)) = badge {
        let color = if status == Status::NeedsAction { ORANGE } else { RED };
        let center = (BADGE_CENTER.0 as f32, BADGE_CENTER.1 as f32);
        canvas.fill_where(color, |x, y| distance((x, y), center) <= BADGE_RADIUS);
        canvas.draw_text(&badge_text(count), BADGE_CENTER, WHITE);
    }
    (canvas.pixels, ICON_SIZE, ICON_SIZE)
}

pub fn badge_text(count: usize) -> String {
    if count > 9 { "9+".to_string() } else { count.to_string() }
}

fn distance((x, y): (u32, u32), (cx, cy): (f32, f32)) -> f32 {
    ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt()
}

struct Canvas {
    size: u32,
    pixels: Vec<u8>,
}

impl Canvas {
    fn new(size: u32) -> Self {
        Canvas { size, pixels: vec![0; (size * size * 4) as usize] }
    }

    fn put(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.size as i32 || y >= self.size as i32 {
            return;
        }
        let i = ((y as u32 * self.size + x as u32) * 4) as usize;
        self.pixels[i..i + 4].copy_from_slice(&color);
    }

    fn fill_where(&mut self, color: [u8; 4], inside: impl Fn(u32, u32) -> bool) {
        for y in 0..self.size {
            for x in 0..self.size {
                if inside(x, y) {
                    self.put(x as i32, y as i32, color);
                }
            }
        }
    }

    fn draw_text(&mut self, text: &str, (cx, cy): (i32, i32), color: [u8; 4]) {
        let glyph_width = 3 * GLYPH_SCALE;
        let spacing = GLYPH_SCALE;
        let count = text.chars().count() as i32;
        let total_width = count * glyph_width + (count - 1) * spacing;
        let mut left = cx - total_width / 2;
        let top = cy - (5 * GLYPH_SCALE) / 2;
        for ch in text.chars() {
            if let Some((_, rows)) = FONT.iter().find(|(c, _)| *c == ch) {
                for (row, bits) in rows.iter().enumerate() {
                    for col in 0..3 {
                        if bits >> (2 - col) & 1 == 1 {
                            for dy in 0..GLYPH_SCALE {
                                for dx in 0..GLYPH_SCALE {
                                    self.put(left + col * GLYPH_SCALE + dx, top + row as i32 * GLYPH_SCALE + dy, color);
                                }
                            }
                        }
                    }
                }
            }
            left += glyph_width + spacing;
        }
    }
}
```

- [ ] **Step 6: テストが通ることを確認する**

Run: `cargo test -p agents-monitor-app tray_icon`
Expected: PASS（4 tests）。座標 (16, 10) はリング上端（中心 (16,21) から距離約 10.5）、(30, 6) はリングの外かつバッジの円の中で数字に重ならない位置にあたる。失敗した場合は座標ではなく描画を確認する。

- [ ] **Step 7: アプリが起動することを確認する**

Run: `cd app && npm run tauri dev`
Expected: ビルドが通り、エラーなく起動したままになる（このタスクでは窓もアイコンも見えなくてよい）。Ctrl+C で止める。

- [ ] **Step 8: コミット**

```bash
git add Cargo.toml Cargo.lock .gitignore app scripts/build-hook-sidecar.sh scripts/make-app-icon.mjs
git commit -m "アプリの骨組みとメニューバーアイコンの描画を追加"
```

---

### Task 12: 監視ループ、メニューバー、一覧ウィンドウ、通知

**Files:**
- Create: `app/src-tauri/src/monitor.rs`
- Create: `app/src-tauri/src/tray.rs`
- Create: `app/src-tauri/src/commands.rs`
- Modify: `app/src-tauri/src/main.rs`
- Create: `app/src/format.ts`
- Modify: `app/src/list.ts`

**Interfaces:**
- Consumes: core の `store::SessionStore`、`spool::{drain_spool, MAX_SPOOL_FILES}`、`ps::run_ps`、`transcript::transcript_ends_with_interrupt`、`store::INTERRUPT_QUIET_MS`、`view::session_views`、`notify::notice_for`、`persist::{load_json, save_json}`、`install::install_hook_binary`、`paths::*`、`settings::Settings`、`tray_icon::render_tray_icon`
- Produces:
  - `monitor::AppState { home: PathBuf, store: Mutex<SessionStore>, settings: Mutex<Settings>, list_hidden_at_ms: AtomicU64 }`
  - `monitor::spawn(app: AppHandle)`、`monitor::publish(app: &AppHandle)`
  - `monitor::NOTIFY_FRESHNESS_MS: u64 = 30_000`、`monitor::should_notify(change_ts_ms: u64, now_ms: u64) -> bool`
  - Tauri コマンド: `get_sessions() -> Vec<SessionView>`、`dismiss_session(id: String)`、`get_settings() -> Settings`、`save_settings(settings: Settings) -> Result<(), String>`、`open_settings()`、`quit_app()`
  - イベント: `sessions-updated`（ペイロード `Vec<SessionView>`）

- [ ] **Step 1: 通知の鮮度判定の失敗するテストを書く**

`app/src-tauri/src/monitor.rs`:

```rust
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use agents_monitor_core::install::install_hook_binary;
use agents_monitor_core::notify::{notice_for, Notice};
use agents_monitor_core::paths;
use agents_monitor_core::persist::{load_json, save_json};
use agents_monitor_core::ps::run_ps;
use agents_monitor_core::settings::Settings;
use agents_monitor_core::spool::{drain_spool, MAX_SPOOL_FILES};
use agents_monitor_core::store::{SessionStore, INTERRUPT_QUIET_MS};
use agents_monitor_core::transcript::transcript_ends_with_interrupt;
use agents_monitor_core::view::session_views;
use tauri::image::Image;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::tray::TRAY_ID;
use crate::tray_icon::render_tray_icon;

// アプリ停止中に溜まった古いイベントで通知を乱発しないため、この時間より古い変化は通知しない
pub const NOTIFY_FRESHNESS_MS: u64 = 30_000;
const TICK: Duration = Duration::from_secs(1);
const PROCESS_CHECK_EVERY_TICKS: u64 = 3;

pub struct AppState {
    pub home: PathBuf,
    pub store: Mutex<SessionStore>,
    pub settings: Mutex<Settings>,
    pub list_hidden_at_ms: AtomicU64,
}

pub fn should_notify(_change_ts_ms: u64, _now_ms: u64) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notifies_only_fresh_changes() {
        assert!(should_notify(100_000, 100_000));
        assert!(should_notify(100_000 - NOTIFY_FRESHNESS_MS, 100_000));
        assert!(!should_notify(100_000 - NOTIFY_FRESHNESS_MS - 1, 100_000));
        assert!(should_notify(100_500, 100_000), "時計のずれで未来の時刻でも通知する");
    }
}
```

`main.rs` に `mod monitor; mod tray;` を足す必要があるため、先に `app/src-tauri/src/tray.rs` を最小限で作る:

```rust
pub const TRAY_ID: &str = "main";
```

`main.rs` の `mod tray_icon;` の下に追加:

```rust
mod monitor;
mod tray;
```

- [ ] **Step 2: テストが失敗することを確認する**

Run: `cargo test -p agents-monitor-app monitor`
Expected: FAIL（`notifies_only_fresh_changes`）

- [ ] **Step 3: 監視ループを実装する**

`monitor.rs` の `should_notify` を置き換え、以下を追加する:

```rust
pub fn should_notify(change_ts_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(change_ts_ms) <= NOTIFY_FRESHNESS_MS
}

pub fn load_state(home: PathBuf) -> AppState {
    AppState {
        store: Mutex::new(load_json(&paths::state_path(&home))),
        settings: Mutex::new(load_json(&paths::settings_path(&home))),
        list_hidden_at_ms: AtomicU64::new(0),
        home,
    }
}

pub fn install_bundled_hook(home: &std::path::Path) {
    let Some(source) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join("agents-monitor-hook"))) else {
        return;
    };
    if let Err(error) = install_hook_binary(&source, &paths::bin_path(home)) {
        eprintln!("agents-monitor-hook の配置に失敗しました: {error}");
    }
}

pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        let mut tick: u64 = 0;
        loop {
            if run_tick(&app, tick % PROCESS_CHECK_EVERY_TICKS == 0) {
                publish(&app);
            }
            tick = tick.wrapping_add(1);
            thread::sleep(TICK);
        }
    });
}

fn run_tick(app: &AppHandle, check_processes: bool) -> bool {
    let state = app.state::<AppState>();
    let now = paths::now_ms();
    let batch = drain_spool(&paths::events_dir(&state.home), MAX_SPOOL_FILES);
    if batch.skipped > 0 {
        eprintln!("読めない、または上限超過のイベントファイルを {} 件捨てました", batch.skipped);
    }
    let notify_on = state.settings.lock().unwrap().notify_on.clone();
    let mut notices: Vec<Notice> = Vec::new();
    let mut changed = false;
    {
        let mut store = state.store.lock().unwrap();
        for event in &batch.events {
            if let Some(change) = store.apply(event) {
                changed = true;
                if should_notify(change.ts_ms, now) {
                    notices.extend(notice_for(&change, &notify_on));
                }
            }
        }
        if check_processes {
            if let Some(rows) = run_ps() {
                changed |= store.reconcile(&rows, now);
            }
            changed |= store.detect_interrupts(now, INTERRUPT_QUIET_MS, transcript_ends_with_interrupt);
        }
        changed |= store.prune(now);
        if changed {
            if let Err(error) = save_json(&paths::state_path(&state.home), &*store) {
                eprintln!("状態の保存に失敗しました: {error}");
            }
        }
    }
    for notice in notices {
        let _ = app.notification().builder().title(&notice.title).body(&notice.body).show();
    }
    changed
}

pub fn publish(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (views, summary) = {
        let store = state.store.lock().unwrap();
        (session_views(store.sessions()), store.summary())
    };
    let _ = app.emit("sessions-updated", &views);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let (rgba, width, height) = render_tray_icon(summary.badge.map(|status| (summary.urgent, status)));
        let _ = tray.set_icon(Some(Image::new_owned(rgba, width, height)));
        let _ = tray.set_title(Some(summary.total.to_string()));
    }
}
```

- [ ] **Step 4: テストが通ることを確認する**

Run: `cargo test -p agents-monitor-app`
Expected: PASS（monitor 1 test、tray_icon 4 tests）

- [ ] **Step 5: メニューバーとウィンドウ、コマンドを実装する**

`app/src-tauri/src/tray.rs`:

```rust
use std::sync::atomic::Ordering;

use agents_monitor_core::paths;
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

use crate::monitor::AppState;
use crate::tray_icon::render_tray_icon;

pub const TRAY_ID: &str = "main";
// 一覧が開いているときにアイコンを押すと、フォーカス喪失で閉じた直後にクリックで再び開いてしまうため
const REOPEN_GUARD_MS: u64 = 300;

pub fn setup_tray(app: &App) -> tauri::Result<()> {
    let (rgba, width, height) = render_tray_icon(None);
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::new_owned(rgba, width, height))
        .icon_as_template(false)
        .title("0")
        .on_tray_icon_event(|tray, event| {
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle_list(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn setup_windows(app: &App) {
    if let Some(list) = app.get_webview_window("list") {
        let handle = app.handle().clone();
        let window = list.clone();
        list.on_window_event(move |event| {
            if let WindowEvent::Focused(false) = event {
                let _ = window.hide();
                handle.state::<AppState>().list_hidden_at_ms.store(paths::now_ms(), Ordering::Relaxed);
            }
        });
    }
    if let Some(settings) = app.get_webview_window("settings") {
        let window = settings.clone();
        settings.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        });
    }
}

fn toggle_list(app: &AppHandle) {
    let Some(window) = app.get_webview_window("list") else { return };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    let hidden_at = app.state::<AppState>().list_hidden_at_ms.load(Ordering::Relaxed);
    if paths::now_ms().saturating_sub(hidden_at) < REOPEN_GUARD_MS {
        return;
    }
    let _ = window.move_window(Position::TrayBottomCenter);
    let _ = window.show();
    let _ = window.set_focus();
}
```

`app/src-tauri/src/commands.rs`:

```rust
use agents_monitor_core::paths;
use agents_monitor_core::persist::save_json;
use agents_monitor_core::settings::Settings;
use agents_monitor_core::view::{session_views, SessionView};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::monitor::{publish, AppState};

#[tauri::command]
pub fn get_sessions(state: State<'_, AppState>) -> Vec<SessionView> {
    session_views(state.store.lock().unwrap().sessions())
}

#[tauri::command]
pub fn dismiss_session(app: AppHandle, state: State<'_, AppState>, id: String) {
    let removed = state.store.lock().unwrap().dismiss(&id);
    if removed {
        publish(&app);
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    let autostart = app.autolaunch();
    let result = if settings.launch_at_login { autostart.enable() } else { autostart.disable() };
    result.map_err(|error| format!("ログイン時の起動設定を変更できませんでした: {error}"))?;
    save_json(&paths::settings_path(&state.home), &settings).map_err(|error| format!("設定を保存できませんでした: {error}"))?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    if let Some(list) = app.get_webview_window("list") {
        let _ = list.hide();
    }
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}
```

`app/src-tauri/src/tray.rs` を作ったので、Step 1 で作った仮の `tray.rs`（`TRAY_ID` だけのもの）は上の内容で置き換わっている。

`app/src-tauri/src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod monitor;
mod tray;
mod tray_icon;

use agents_monitor_core::paths;
use tauri_plugin_autostart::MacosLauncher;

fn main() {
    let home = paths::monitor_home().expect("ホームディレクトリが見つかりません");
    if let Err(error) = paths::prepare_private_dir(&home).and_then(|_| paths::prepare_private_dir(&paths::events_dir(&home))) {
        eprintln!("{} を作成できませんでした: {error}", home.display());
    }
    monitor::install_bundled_hook(&home);

    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .manage(monitor::load_state(home))
        .invoke_handler(tauri::generate_handler![
            commands::get_sessions,
            commands::dismiss_session,
            commands::get_settings,
            commands::save_settings,
            commands::open_settings,
            commands::quit_app,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            tray::setup_tray(app)?;
            tray::setup_windows(app);
            monitor::publish(app.handle());
            monitor::spawn(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run agents-monitor");
}
```

- [ ] **Step 6: 一覧画面を実装する**

`app/src/format.ts`:

```ts
export function formatElapsed(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  if (seconds < 60) return `${seconds}秒`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}分`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}時間`;
  return `${Math.floor(hours / 24)}日`;
}
```

`app/src/list.ts`（画面に出す文字列はコマンドやパスなど任意の内容を含むため、`textContent` だけで組み立て、`innerHTML` は使わない）:

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { formatElapsed } from "./format";
import { STATUS_LABEL, type SessionView } from "./types";

const listEl = document.getElementById("sessions")!;
const expanded = new Set<string>();
let sessions: SessionView[] = [];

function el<K extends keyof HTMLElementTagNameMap>(tag: K, className: string, text?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function metaText(session: SessionView): string {
  const parts = [session.detail ?? STATUS_LABEL[session.status]];
  if (session.branch) parts.unshift(session.branch);
  if (session.profile) parts.push(session.profile);
  return parts.join(" · ");
}

function renderDetails(session: SessionView): HTMLElement {
  const details = el("div", "details");
  details.append(
    el("span", "", `状態: ${STATUS_LABEL[session.status]}`),
    el("span", "", `場所: ${session.cwd || "不明"}`),
    el("span", "", `プロフィール: ${session.config_dir || "不明"}`),
    el("span", "", `プロセス: ${session.claude_pid ?? "不明"}`),
  );
  if (session.status === "ended" || session.status === "unknown") {
    const dismiss = el("button", "", "一覧から消す");
    dismiss.type = "button";
    dismiss.addEventListener("click", (event) => {
      event.stopPropagation();
      void invoke("dismiss_session", { id: session.id });
    });
    details.append(dismiss);
  }
  return details;
}

function renderRow(session: SessionView, now: number): HTMLElement {
  const row = el("li", `row status-${session.status}`);
  const main = el("div", "row-main");
  const text = el("div", "text");
  text.append(el("div", "project", session.project), el("div", "meta", metaText(session)));
  main.append(el("span", `dot dot-${session.status}`), text, el("span", "elapsed", formatElapsed(now - session.status_since_ms)));
  row.append(main);
  if (expanded.has(session.id)) row.append(renderDetails(session));
  row.addEventListener("click", () => {
    if (expanded.has(session.id)) expanded.delete(session.id);
    else expanded.add(session.id);
    render();
  });
  return row;
}

function render(): void {
  const now = Date.now();
  if (sessions.length === 0) {
    listEl.replaceChildren(el("li", "empty", "動いている Claude Code セッションはありません"));
    return;
  }
  listEl.replaceChildren(...sessions.map((session) => renderRow(session, now)));
}

document.getElementById("open-settings")!.addEventListener("click", () => void invoke("open_settings"));
document.getElementById("quit")!.addEventListener("click", () => void invoke("quit_app"));

void listen<SessionView[]>("sessions-updated", (event) => {
  sessions = event.payload;
  render();
});
void invoke<SessionView[]>("get_sessions").then((initial) => {
  sessions = initial;
  render();
});
setInterval(render, 1000);
```

- [ ] **Step 7: 型チェックとビルドを確認する**

Run: `cd app && npm run build && cd .. && cargo build -p agents-monitor-app`
Expected: エラーなし

- [ ] **Step 8: 実機で動作を確認する**

```bash
cd app && npm run tauri dev
```

確認すること（Task 5 で `~/.claude-personal` にプラグインを入れてある前提。`~/.claude` 側で動くセッションは「状態不明」で出れば正しい）:

1. メニューバーにリングのアイコンと総数が出る。Dock にアイコンは出ない
2. アイコンをクリックすると、真下に一覧が開く。もう一度クリック、または他の場所をクリックすると閉じる
3. 新しいターミナルで `CLAUDE_CONFIG_DIR=$HOME/.claude-personal claude` を起動すると、数秒以内に「新規」の青い行が増える
4. 「`ls /tmp` を実行して」と送ると、許可ダイアログの間はオレンジの行（背景色付き）、バッジがオレンジで件数 1、OS 通知「<プロジェクト名> — 許可待ち」が出る
5. 許可して応答が終わると青（完了）に戻り、バッジが消える
6. `/exit` で終えると数秒でグレー（終了）になり、5 分後に消える
7. 行をクリックすると詳細が開く。終了・状態不明の行では「一覧から消す」が効く
8. アプリを終了（一覧の「終了」）して、その間にセッションで 1 往復してから再起動すると、通知は出ずに一覧だけ最新になる
9. `echo '<b>x</b>'` を実行させたとき、一覧の補足に `<b>x</b>` が文字のまま出る（太字にならない）

いずれかが期待どおりでなければ、原因を調べて直してから次へ進む（superpowers:systematic-debugging を使う）。

- [ ] **Step 9: コミット**

```bash
git add app crates
git commit -m "メニューバー、一覧ウィンドウ、監視ループ、通知を追加"
```

---

### Task 13: 設定画面

**Files:**
- Modify: `app/src/settings.ts`

**Interfaces:**
- Consumes: コマンド `get_settings`、`save_settings`、型 `Settings`、`STATUS_LABEL`
- Produces: 設定画面（通知する状態の選択、ログイン時の起動）

- [ ] **Step 1: 設定画面を実装する**

`app/src/settings.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import { STATUS_LABEL, type Settings, type Status } from "./types";

const NOTIFIABLE: Status[] = ["needs_action", "error", "done", "running"];
const optionsEl = document.getElementById("notify-options")!;
const launchEl = document.getElementById("launch-at-login") as HTMLInputElement;
const errorEl = document.getElementById("settings-error")!;
let current: Settings;

function showError(message: string | null): void {
  errorEl.hidden = message === null;
  errorEl.textContent = message ?? "";
}

async function save(next: Settings): Promise<void> {
  try {
    await invoke("save_settings", { settings: next });
    current = next;
    showError(null);
  } catch (error) {
    showError(String(error));
    render();
  }
}

function render(): void {
  optionsEl.replaceChildren(
    ...NOTIFIABLE.map((status) => {
      const label = document.createElement("label");
      label.className = "check";
      const input = document.createElement("input");
      input.type = "checkbox";
      input.checked = current.notify_on.includes(status);
      input.addEventListener("change", () => {
        const notify_on = input.checked
          ? [...current.notify_on, status]
          : current.notify_on.filter((s) => s !== status);
        void save({ ...current, notify_on });
      });
      label.append(input, document.createTextNode(` ${STATUS_LABEL[status]}`));
      return label;
    }),
  );
  launchEl.checked = current.launch_at_login;
}

launchEl.addEventListener("change", () => void save({ ...current, launch_at_login: launchEl.checked }));

void invoke<Settings>("get_settings").then((settings) => {
  current = settings;
  render();
});
```

- [ ] **Step 2: 型チェックを確認する**

Run: `cd app && npm run build`
Expected: エラーなし

- [ ] **Step 3: 実機で確認する**

`npm run tauri dev` で起動し、次を確認する:

1. 一覧の「設定…」で設定画面が開き、一覧は閉じる。設定画面の閉じるボタンで閉じ、もう一度「設定…」で開ける
2. 初期状態で「要対応」「エラー停止」にチェックが入っている
3. 「完了」にチェックを入れると、セッションの応答完了でも通知が出る。外すと出なくなる
4. 設定はアプリを再起動しても保たれる（`cat ~/.agents-monitor/settings.json`）
5. 「ログイン時に起動する」をオンにすると `~/Library/LaunchAgents/` に agents-monitor の plist ができ、オフで消える（dev 起動では実行ファイルのパスが開発用になる点は問題ない）

- [ ] **Step 4: コミット**

```bash
git add app/src/settings.ts
git commit -m "設定画面を追加"
```

---

### Task 14: パッケージの作成と README

**Files:**
- Create: `README.md`

**Interfaces:**
- Consumes: これまでのすべて

- [ ] **Step 1: README を書く**

`README.md`:

````markdown
# agents-monitor

マシン上で動いている Claude Code セッションの状態を、macOS のメニューバーに一覧表示します。許可待ちや質問待ちなど、あなたの対応が必要なセッションを見落とさないためのツールです。

## 状態の見方

| 色 | 状態 | 意味 |
|---|---|---|
| オレンジ | 要対応 | 許可待ち・質問待ち・プラン承認待ち・入力要求 |
| 赤 | エラー停止 | API エラーや利用上限で止まった |
| 青 | 完了 | 応答が終わり、次の指示を待っている |
| 緑 | 実行中 | 作業中 |
| グレー | 終了 | 終了したセッション（5 分後に消える） |
| グレーの枠 | 状態不明 | プラグイン導入前から動いているセッション。開き直すと監視される |

メニューバーの数字は動いているセッションの総数、バッジは要対応とエラー停止の件数です。

## 導入（v0.1）

1. アプリを起動する（起動時に `~/.agents-monitor/bin/agents-monitor-hook` が配置されます）
2. 使っている Claude Code の設定ごとに、ターミナルでプラグインを入れる

   ```bash
   claude plugin marketplace add imatomix/agents-monitor
   claude plugin install agents-monitor@agents-monitor
   ```

   `CLAUDE_CONFIG_DIR` で設定を分けている場合は、それぞれ `CLAUDE_CONFIG_DIR=<設定ディレクトリ> claude plugin ...` のように実行してください。

3. 動いている Claude Code セッションを開き直す

## 保存するデータ

`~/.agents-monitor/`（本人だけが読み書きできる権限）に、セッションの状態と短い要約（実行中のコマンドやファイル名、最大 120 文字）だけを保存します。ツールへの入力の全文やファイルの中身は保存しません。

## 開発

```bash
./scripts/dev-install-hook.sh      # hook を ~/.agents-monitor/bin に置く
./scripts/build-hook-sidecar.sh    # アプリ同梱用の hook を作る
cargo test --workspace
cd app && npm install && npm run tauri dev
```

設計: `docs/superpowers/specs/2026-10-09-agents-monitor-design.md`
````

（`imatomix/agents-monitor` は GitHub に公開したときのリポジトリ名。公開前はローカルパスで `claude plugin marketplace add <このリポジトリのパス>` を使う）

- [ ] **Step 2: 全テストを実行する**

Run: `cargo test --workspace && (cd app && npm run build)`
Expected: すべて PASS、ビルドエラーなし

- [ ] **Step 3: パッケージを作って起動を確認する**

```bash
./scripts/build-hook-sidecar.sh
cd app && npm run tauri build && cd ..
open target/release/bundle/macos/agents-monitor.app
```

Expected: 署名なしのため初回は Gatekeeper の警告が出ることがある（右クリック →「開く」で起動）。起動後、`ls -l ~/.agents-monitor/bin/agents-monitor-hook` の更新時刻がパッケージ版の起動時刻になっている。Task 12 Step 8 の 1〜6 をパッケージ版でもう一度確認する。

- [ ] **Step 4: コミット**

```bash
git add README.md
git commit -m "README を追加"
```
