# Agent Loaf の Codex 対応 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Codex のセッションも、Claude Code と同じ一覧・通知・メニューバーのアイコンで見られるようにする。導入（プラグイン）と hook の許可の案内も、設定の画面の同じ欄で扱う。

**Architecture:** エージェントの種類（`Agent`）を core に足し、hook・イベント・セッション・画面の行に持たせる。hook は親をたどって `claude` か `codex` かを決める。Codex の会話は `session_id` で区別し、pid は生きているかの確認だけに使う。導入は、設定ディレクトリの中身（`config.toml` の有無）で種類を決め、`claude` か `codex` のコマンドで同じ手順の形で行う。hook の許可は `config.toml` の `[hooks.state]` を読むだけで判断する。

**Tech Stack:** Rust（core・hook・Tauri 2 アプリ）、Vite + TypeScript、Figma MCP（アイコンの書き出し）。新しいライブラリは足さない

**Spec:** `docs/superpowers/specs/2026-10-12-agent-loaf-codex-design.md`

## Global Constraints

- 新しいライブラリを足さない（TOML も自前の行の読み取りで扱う）
- Claude Code の監視の動きを変えない（今のテストがすべて通ること）
- Codex の設定ファイル（`config.toml`）は読むだけ。書き換えない
- 常駐するアプリなので、定期的な処理を増やさない（hook の許可の確認は、設定の画面を開いたときだけ）
- 画面の文字色に状態色を使わない。任意の文字列は `textContent` で入れる。Figma（`156:1265` の案 A、`156:1613`）にない見た目を足さない
- 版は 0.5.0 のまま
- コミットメッセージは日本語。末尾に `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`。コメントは WHY が非自明なときだけ（日本語）
- push しない

## Review Focus

1. **v0.5 までの `state.json`・イベント（`agent` がない）**→ Claude Code として読み、表示も動きも変わらない（Task 1・3 でテスト）
2. **同じ VS Code のウィンドウで 2 つの Codex の会話が並ぶ**→ どちらも残り、片方が終了にならない（Task 3 でテスト）
3. **Claude Code のセッションの中から `codex exec` を動かした**（環境変数が Claude Code のものを引き継ぐ）→ Codex として見分け、会話ではないので書かない（Task 2 でテスト）
4. **`config.toml` に agent-loaf の許可が一部だけある・壊れている・ない**→ 「許可が必要」と出す。読めないときも落ちない（Task 6 でテスト）
5. **ユーザーが `~/.codex` を一覧から外した後に起動する**→ もう足さない（Task 6 でテスト）

---

## 実験で確かめた前提（2026-10-12、計画を書く前に確かめた）

- Codex の `plugin list --json` の形: `{"installed":[{"pluginId":"agent-loaf@agent-loaf","version":"0.5.0","installed":true,"enabled":true,"source":{"source":"local","path":"…/marketplace/plugin"},"marketplaceSource":{"sourceType":"local","source":"…/marketplace"}}],"available":[…]}`
- Codex の `plugin marketplace list --json` の形: `{"marketplaces":[{"name":"agent-loaf","root":"…/marketplace","marketplaceSource":{"sourceType":"local","source":"…/marketplace"}}]}`
- **更新**: 版を上げたマーケットプレイスで `codex plugin add agent-loaf@agent-loaf` をもう一度動かすと、新しい版（キャッシュの `0.5.1`）に置き換わった
- Codex のコマンドは `plugin add` / `plugin remove` / `plugin marketplace add|remove`。`enable` はない
- Claude Code の `claude plugin validate` は、`hooks.json` の `Interrupt` を「知らない hook。動かすときは無視する」の警告で通した（`PostCompact` は Claude Code にもある）
- `[hooks.state]` の項目の名前: `[hooks.state."agent-loaf@agent-loaf:hooks/hooks.json:<イベントの snake_case>:0:0"]`。Codex は知らない名前の hook（`Notification` など）を数えず、12 種類だけを許可の対象にした

---

## ファイル構成

| ファイル | 役割 | Task |
|---|---|---|
| `crates/core/src/agent.rs`（新規） | `Agent`（`claude_code` / `codex`）と、設定ディレクトリの種類の決め方 | 1 |
| `crates/core/src/ps.rs` | `codex` の見分け、親のたどり方（`find_agent_ancestor`）、生きているかの確認（`is_agent_command`） | 1 |
| `crates/core/src/paths.rs` | `default_codex_dir` | 1 |
| `crates/core/src/event.rs`, `hook_input.rs`, `crates/hook/src/main.rs` | イベントに `agent`、会話以外を除く、許可の理由の文 | 2 |
| `crates/core/src/state.rs`, `store.rs`, `view.rs` | `Interrupt`、同じ pid の扱い、中断の推測、セッションと行の `agent` | 3 |
| `crates/core/src/jump.rs` | Codex の移動 | 4 |
| `plugin/hooks/hooks.json`, `crates/hook/tests/plugin_manifest.rs` | `Interrupt`・`PostCompact`、待ち時間 | 5 |
| `crates/core/src/codex_plugin.rs`（新規） | Codex の一覧の読み取り、`[hooks.state]` の読み取り、許可のための `.command` の中身 | 6 |
| `crates/core/src/install_plan.rs`, `plugin_status.rs`, `settings.rs`, `config_dirs.rs`, `attention.rs`, `claude_cli.rs` | 手順のエージェントごとの引数、`NeedsTrust`、`codex_dir_offered`・`codex_path`、`~/.codex` の自動の追加、`find_codex` | 6 |
| `app/src-tauri/src/installer.rs`, `commands.rs`, `main.rs` | 設定ディレクトリの種類ごとの導入、許可の確認、`open_codex_trust`、`set_codex_path` | 7 |
| `app/index.html`, `app/src/settings.ts`, `app/src/list.ts`, `app/src/types.ts`, `app/src/style.css`, `app/src/assets/*.svg`（新規）, `crates/core/locales/*.json`, `crates/core/src/messages.rs` | 画面 | 8 |
| `README.md`, `README.en.md`, `docs/superpowers/specs/*.md` | 文書 | 9 |

---

### Task 1: エージェントの種類と、プロセスの見分け

**Files:**
- Create: `crates/core/src/agent.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/ps.rs`, `crates/core/src/paths.rs`

**Interfaces:**
- Produces:
  - `pub enum Agent { ClaudeCode, Codex }`（`Serialize`・`Deserialize`、`#[serde(rename_all = "snake_case")]`、`Default` は `ClaudeCode`、`Clone`・`Copy`・`PartialEq`・`Eq`・`Debug`）
  - `pub fn agent_of_config_dir(dir: &Path) -> Agent`（`config.toml` があれば Codex）
  - `ps::agent_of(args: &str) -> Option<Agent>`
  - `ps::is_agent_command(args: &str) -> bool`（`claude` か `codex`）
  - `ps::find_agent_ancestor(rows: &[ProcRow], start_pid: u32) -> Option<(&ProcRow, Agent)>`
  - `paths::default_codex_dir() -> Option<String>`（`CODEX_HOME`、なければ `~/.codex`）

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/agent.rs`:

```rust
use std::path::Path;

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_records_without_agent_are_claude_code() {
        assert_eq!(Agent::default(), Agent::ClaudeCode);
        assert_eq!(serde_json::to_value(Agent::Codex).unwrap(), "codex");
        assert_eq!(serde_json::from_str::<Agent>("\"claude_code\"").unwrap(), Agent::ClaudeCode);
    }

    #[test]
    fn config_dir_with_config_toml_is_codex() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(agent_of_config_dir(dir.path()), Agent::ClaudeCode);
        std::fs::write(dir.path().join("config.toml"), "model = \"x\"\n").unwrap();
        assert_eq!(agent_of_config_dir(dir.path()), Agent::Codex);
        assert_eq!(agent_of_config_dir(Path::new("/nonexistent/x")), Agent::ClaudeCode);
    }
}
```

`crates/core/src/lib.rs` に `pub mod agent;` を足す。

`crates/core/src/ps.rs` の `mod tests` に足す。

```rust
    const CODEX_IN_VSCODE: &str = "/Users/u/.vscode/extensions/openai.chatgpt-26.1007.21434-darwin-arm64/bin/macos-aarch64/codex -c features.code_mode_host=true app-server --analytics-default-enabled";

    #[test]
    fn tells_codex_from_claude_and_other_programs() {
        use crate::agent::Agent;
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
        use crate::agent::Agent;
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
        assert_eq!(find_claude_ancestor(&rows, 400).map(|r| r.pid), Some(300), "名前は今のまま、codex も見つける");
    }
```

`crates/core/src/paths.rs` の `mod tests` に足す。

```rust
    #[test]
    fn codex_dir_defaults_to_dot_codex() {
        let dir = default_codex_dir().unwrap();
        match std::env::var_os("CODEX_HOME") {
            Some(home) => assert_eq!(dir, home.to_string_lossy()),
            None => assert!(dir.ends_with("/.codex"), "{dir}"),
        }
    }
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agent-loaf-core --lib agent ps paths`
Expected: コンパイルエラー（`Agent`・`agent_of`・`find_agent_ancestor`・`default_codex_dir` がない）

- [ ] **Step 3: 実装する**

`crates/core/src/agent.rs` のテストの前に足す。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Agent {
    // 前の版のイベントや state.json には種類がないので、Claude Code として読む
    #[default]
    ClaudeCode,
    Codex,
}

// 保存する設定の形を変えないため、種類はそのつどフォルダーの中身で決める
pub fn agent_of_config_dir(dir: &Path) -> Agent {
    if dir.join("config.toml").is_file() {
        Agent::Codex
    } else {
        Agent::ClaudeCode
    }
}
```

`crates/core/src/ps.rs`:
- `use crate::agent::Agent;` を足す
- 次を足す。

```rust
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
```

- `find_claude_ancestor` の中の `if is_claude_command(&row.args)` を `if is_agent_command(&row.args)` にする（名前は今のまま。Claude Code も Codex も見つける）
- 注: Codex の app-server は空白区切りの最初が実行ファイルの完全なパスで、空白を含まない（拡張の中の `bin/macos-aarch64/codex`）。空白を含むパスの `codex` は見分けられないが、今の `claude` と同じ扱い

`crates/core/src/paths.rs` の `default_config_dir` の後に足す。

```rust
pub fn default_codex_dir() -> Option<String> {
    if let Some(dir) = std::env::var_os("CODEX_HOME") {
        return Some(dir.to_string_lossy().into_owned());
    }
    Some(dirs::home_dir()?.join(".codex").to_string_lossy().into_owned())
}
```

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test --workspace`
Expected: PASS（今の `ps` のテスト、`returns_none_without_claude_ancestor` なども）

- [ ] **Step 5: コミット**

```bash
git add crates/core/src/agent.rs crates/core/src/lib.rs crates/core/src/ps.rs crates/core/src/paths.rs
git commit -m "エージェントの種類（Claude Code・Codex）と、codex のプロセスの見分けを足す

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: hook がエージェントの種類を書き、Codex の会話以外を除く

**Files:**
- Modify: `crates/core/src/event.rs`, `crates/core/src/hook_input.rs`, `crates/hook/src/main.rs`
- Test: `crates/core/src/hook_input.rs`、`crates/hook/tests/cli.rs`

**Interfaces:**
- Consumes: Task 1 の `Agent`、`find_agent_ancestor`、`default_codex_dir`
- Produces:
  - `MonitorEvent.agent: Agent`（`#[serde(default)]`）
  - `HookContext.agent: Agent`
  - `hook_input::is_conversation(agent: Agent, agent_args: Option<&str>, entrypoint: Option<&str>) -> bool`（引数が 1 つ増える）

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/hook_input.rs` の `tells_conversations_from_scripted_runs` の呼び出しを、すべて `is_conversation(Agent::ClaudeCode, …)` に直し、次のテストを足す（`use crate::agent::Agent;` をテストに足す）。

```rust
    #[test]
    fn tells_codex_conversations_from_scripted_runs() {
        let app_server = "/Users/u/.vscode/extensions/openai.chatgpt-26.1007.21434-darwin-arm64/bin/macos-aarch64/codex -c features.code_mode_host=true app-server";
        assert!(is_conversation(Agent::Codex, Some(app_server), None));
        assert!(is_conversation(Agent::Codex, Some("/opt/homebrew/bin/codex"), None));
        assert!(!is_conversation(Agent::Codex, Some("/opt/homebrew/bin/codex exec hi"), None));
        assert!(!is_conversation(Agent::Codex, Some("/opt/homebrew/bin/codex review"), None));
        // Claude Code の会話の中で動かすと、Claude Code の entrypoint を引き継ぐ。Codex では見ない
        assert!(is_conversation(Agent::Codex, Some(app_server), Some("sdk-cli")));
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
        assert_eq!(event.detail.summary.as_deref(), Some("https://example.com へのヘッダー取得を許可しますか？"));
        let mut no_reason = input.clone();
        no_reason["tool_input"]["description"] = serde_json::Value::Null;
        let mut context = ctx();
        context.agent = Agent::Codex;
        assert_eq!(build_event(&no_reason, context).unwrap().detail.summary.as_deref(), Some("curl -sI https://example.com"), "理由がなければコマンド");
    }
```

（`ctx()` は今のテストの関数。`HookContext` に `agent` を足すので、`ctx()` の中で `agent: Agent::ClaudeCode` を入れる。）

`crates/core/src/event.rs` の前の版のイベントを読むテストは `spool.rs` などにある。`crates/core/src/spool.rs` の `mod tests` に足す。

```rust
    #[test]
    fn events_from_older_hooks_are_claude_code() {
        let old = r#"{"schema":1,"ts_ms":1,"event":"Stop","session_id":"s1","agent_id":null,"claude_pid":7,"config_dir":"/c","cwd":"/w","transcript_path":null,"detail":{"tool_name":null,"summary":null,"notification_type":null,"error_type":null},"terminal":null,"git_branch":null}"#;
        let event: MonitorEvent = serde_json::from_str(old).unwrap();
        assert_eq!(event.agent, crate::agent::Agent::ClaudeCode);
    }
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agent-loaf-core --lib`
Expected: コンパイルエラー（`agent` がない、`is_conversation` の引数）

- [ ] **Step 3: 実装する**

`crates/core/src/event.rs` の `MonitorEvent` に足す（`use crate::agent::Agent;`）。

```rust
    #[serde(default)]
    pub agent: Agent,
```

`crates/core/src/hook_input.rs`:
- `HookContext` に `pub agent: Agent,` を足し、`build_event` で `agent: ctx.agent,` を入れる
- 許可の理由の文: `summary` を決めるところを次にする。

```rust
    let reason = (ctx.agent == Agent::Codex && event == "PermissionRequest")
        .then(|| input.pointer("/tool_input/description").and_then(|v| v.as_str()).map(truncate_summary))
        .flatten();
    let summary = reason.or_else(|| match (&tool_name, input.get("tool_input")) {
        (Some(name), Some(tool_input)) => summarize_tool(name, tool_input),
        _ => None,
    });
```

（`event` を `MonitorEvent` に入れる前に使うので、`let event = str_field(input, "hook_event_name")?;` の後に置く。）

- `is_conversation` を次にする。

```rust
// claude -p・Agent SDK・codex exec・codex review からの実行は、人が向き合う会話ではないので一覧にも通知にも出さない。
// entrypoint は外側の Claude から引き継がれる（VS Code の会話の中で動かした claude -p も claude-vscode になる）ので、引数も見る。
// サブコマンドの一覧（is_interactive_session）は使わない。引数は空白でつないだ形なので、「update the changelog」のような依頼の文を取り違える
pub fn is_conversation(agent: Agent, agent_args: Option<&str>, entrypoint: Option<&str>) -> bool {
    let has = |words: &[&str]| agent_args.is_some_and(|args| args.split_whitespace().any(|token| words.contains(&token)));
    match agent {
        Agent::ClaudeCode => !entrypoint.is_some_and(|e| e.starts_with("sdk-")) && !has(&["-p", "--print"]),
        // Codex の entrypoint の環境変数はないので、Claude Code から引き継いだものは見ない。別名の codex e は見分けない
        Agent::Codex => !has(&["exec", "review"]),
    }
}
```

`crates/hook/src/main.rs` の `run` の、親をたどるところからを次にする。

```rust
    // hook はツールを使うたびに動くので、全プロセスではなく自分の親だけをたどる
    let rows = ancestor_rows(std::process::id());
    let found = find_agent_ancestor(&rows, std::process::id());
    let agent = found.map(|(_, agent)| agent).unwrap_or_default();
    let process = found.map(|(row, _)| row);
    let entrypoint = std::env::var("CLAUDE_CODE_ENTRYPOINT").ok();
    if !is_conversation(agent, process.map(|p| p.args.as_str()), entrypoint.as_deref()) {
        return None;
    }
    let terminal = is_session_start.then(|| terminal_info(agent, process.and_then(|p| p.tty.clone())));
```

- `HookContext` を作るところ: `claude_pid: process.map(|p| p.pid),`、`config_dir: match agent { Agent::ClaudeCode => paths::default_config_dir()?, Agent::Codex => paths::default_codex_dir()? },`、`agent,`
- `terminal_info(agent: Agent, tty: Option<String>)`: `entrypoint` は `agent == Agent::ClaudeCode` のときだけ `var("CLAUDE_CODE_ENTRYPOINT")`、Codex では `None`（Claude Code から引き継いだ値で、移動のときにエディターと取り違えないため）
- use: `agent_loaf_core::agent::Agent`、`agent_loaf_core::ps::find_agent_ancestor`（`find_claude_ancestor` の use は消す）

`crates/hook/tests/cli.rs` に足す。`cargo test` の下からは Codex の形の親を作れないので、Claude Code として呼ばれたときに `CODEX_HOME` を設定ディレクトリに使わないことだけを確かめる（Codex から呼ばれたときの `agent` と `config_dir` は、Task 10 の実機確認で `state.json` を見て確かめる）。

```rust
#[test]
fn codex_home_is_ignored_for_claude_sessions() {
    let home = tempfile::tempdir().unwrap();
    let (output, _) = run_hook_with(home.path(), PRE_TOOL_USE, &[("CODEX_HOME", "/tmp/not-used")]);
    assert_silent_success(&output);
    let files = event_files(home.path());
    let event: serde_json::Value = serde_json::from_slice(&fs::read(&files[0]).unwrap()).unwrap();
    assert_ne!(event["config_dir"], "/tmp/not-used");
}
```

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test --workspace`
Expected: PASS

- [ ] **Step 5: コミット**

```bash
git add crates/core/src/event.rs crates/core/src/hook_input.rs crates/core/src/spool.rs crates/hook
git commit -m "hook が Claude Code か Codex かを書き、codex exec などの会話以外を除く

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Codex の会話の状態と寿命

**Files:**
- Modify: `crates/core/src/state.rs`, `crates/core/src/store.rs`, `crates/core/src/view.rs`

**Interfaces:**
- Consumes: Task 1 の `Agent`、`is_agent_command`。Task 2 の `MonitorEvent.agent`
- Produces: `Session.agent: Agent`（`#[serde(default)]`）、`SessionView.agent: Agent`

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/store.rs` の `mod tests` に足す。

```rust
    fn codex(event: &str, session: &str, ts_ms: u64) -> MonitorEvent {
        let mut e = ev(event, session, ts_ms);
        e.agent = crate::agent::Agent::Codex;
        e.claude_pid = Some(500);
        e
    }

    fn codex_row(pid: u32) -> crate::ps::ProcRow {
        crate::ps::ProcRow { pid, ppid: 1, tty: None, args: "/Users/u/.vscode/extensions/openai.chatgpt-26.1007.21434-darwin-arm64/bin/macos-aarch64/codex -c features.code_mode_host=true app-server".into() }
    }

    #[test]
    fn codex_conversations_in_one_process_live_side_by_side() {
        let mut store = SessionStore::default();
        store.apply(&codex("SessionStart", "a", 1));
        store.apply(&codex("UserPromptSubmit", "a", 2));
        store.apply(&codex("SessionStart", "b", 3));
        store.apply(&codex("UserPromptSubmit", "b", 4));
        assert_eq!(status_of(&store, "a").0, Status::Running, "同じ app-server の別の会話で終了にしない");
        assert_eq!(status_of(&store, "b").0, Status::Running);
        store.reconcile(&[codex_row(500)], 5, |_| None);
        assert_eq!(status_of(&store, "a").0, Status::Running, "codex のプロセスは生きているとみなす");
        assert!(store.sessions().iter().all(|s| !s.id.starts_with(UNKNOWN_ID_PREFIX)), "codex を状態不明の行にしない");
        store.apply(&codex("SessionEnd", "a", 6));
        assert_eq!(status_of(&store, "a").0, Status::Ended);
        store.reconcile(&[], 7, |_| None);
        assert_eq!(status_of(&store, "b").0, Status::Ended, "プロセスがなくなれば終わり");
    }

    #[test]
    fn claude_sessions_sharing_a_pid_still_retire() {
        let mut store = SessionStore::default();
        store.apply(&ev("SessionStart", "old", 1));
        store.apply(&ev("SessionStart", "new", 2));
        assert_eq!(status_of(&store, "old").0, Status::Ended, "/clear のときの今の動き");
    }

    #[test]
    fn codex_interrupt_is_done_interrupted_and_transcripts_are_not_read() {
        let mut store = SessionStore::default();
        let mut start = codex("UserPromptSubmit", "a", 1_000);
        start.transcript_path = Some("/t.jsonl".into());
        store.apply(&start);
        assert!(!store.detect_interrupts(1_000 + INTERRUPT_QUIET_MS, INTERRUPT_QUIET_MS, |_| true), "Codex の記録は読まない");
        store.apply(&codex("Interrupt", "a", 2_000));
        assert_eq!(status_of(&store, "a"), (Status::Done, Some("完了（中断）".into())));
        assert_eq!(store.sessions()[0].agent, crate::agent::Agent::Codex);
    }
```

（「完了（中断）」は `locales/ja.json` の `detail.interrupted` の文言に合わせる。違えばその文言にする。`status_of` は今のテストの関数で、詳細の文を返す。）

`crates/core/src/persist.rs` の `v03_state_with_subagent_count_still_loads` で、読んだセッションの `agent` が `Agent::ClaudeCode` であることも確かめる（1 行足す）。

`crates/core/src/view.rs` の `mod tests` に足す（今のテストの `store_with` は Claude Code のイベントを作る）。

```rust
    #[test]
    fn views_carry_the_agent() {
        let store = store_with(&[("s1", "/w/api", "/Users/u/.claude", "main", 1)]);
        let (views, _) = session_views(store.sessions(), &[], &Monitored::new(&["/Users/u/.claude".into()]), Lang::Ja);
        assert_eq!(views[0].agent, crate::agent::Agent::ClaudeCode);
    }
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agent-loaf-core --lib`
Expected: コンパイルエラー（`agent` がない）。足した後は `codex_conversations_in_one_process_live_side_by_side` と `codex_interrupt_is_done_interrupted_and_transcripts_are_not_read` が FAIL

- [ ] **Step 3: 実装する**

`crates/core/src/state.rs`:
- `Session` に `#[serde(default)] pub agent: Agent,` を足す（`use crate::agent::Agent;`）
- `transition_for` の `match` に足す。

```rust
        "Interrupt" => to(Status::Done, Some(DetailKind::Interrupted), None),
```

（`PostCompact` は今の `_ => None` で状態を変えない。）

`crates/core/src/store.rs`:
- `new_session` と `unknown_process_session` で `agent: ev.agent,`（状態不明の行は `agent: Agent::ClaudeCode,`）
- `observe` の先頭で `session.agent = ev.agent;`
- `apply` の最初を次にする。

```rust
        // Codex は 1 つのプロセス（VS Code の app-server）で複数の会話を並べて動かすので、同じ pid でも前の会話を終わりにしない
        if let Some(pid) = ev.claude_pid.filter(|_| ev.agent == Agent::ClaudeCode) {
            self.retire_sessions_sharing_pid(pid, &ev.session_id, ev.ts_ms);
        }
```

- `retire_sessions_sharing_pid` の中の終了にする対象に `session.agent == Agent::ClaudeCode` を足す（Claude Code の会話の pid が Codex のと重なることはないが、念のため Codex の会話を巻き込まない）
- `reconcile` の `alive` を `is_agent_command` で数える（`is_claude_command` の use を `is_agent_command` に替える）
- `detect_interrupts` の対象から Codex の会話を外す: `if session.agent == Agent::Codex || !matches!(…)` の形で `continue`

`crates/core/src/view.rs`: `SessionView` に `pub agent: Agent,` を足し、`agent: session.agent,` を入れる。

`crates/core/src/jump.rs` のテストの `Session { … }` に `agent: Default::default(),` を足す（コンパイルを通すため）。ほかに `Session {` を作っているところを `grep -rn "Session {" crates` で探して足す。

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test --workspace`
Expected: PASS（今のテストもすべて）

- [ ] **Step 5: コミット**

```bash
git add crates/core/src
git commit -m "Codex の会話は同じプロセスに並んでいても終わりにせず、Interrupt で中断とし、会話の記録は読まない

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Codex の会話への移動

**Files:**
- Modify: `crates/core/src/jump.rs`

**Interfaces:**
- Consumes: Task 1 の `is_agent_command`、Task 3 の `Session.agent`

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/jump.rs` の `mod tests` に足す（`session(...)` は今のテストの関数）。

```rust
    const CODEX_IN_VSCODE: &str = "/Users/u/.vscode/extensions/openai.chatgpt-26.1007.21434-darwin-arm64/bin/macos-aarch64/codex -c features.code_mode_host=true app-server";

    #[test]
    fn codex_in_vscode_opens_the_project_window() {
        let rows = vec![
            row(1, 0, "/sbin/launchd"),
            row(100, 1, "/Applications/Visual Studio Code.app/Contents/MacOS/Electron"),
            row(200, 100, VSCODE_HELPER),
            row(300, 200, CODEX_IN_VSCODE),
        ];
        let mut s = session("s1", 300, None, None);
        s.agent = crate::agent::Agent::Codex;
        assert_eq!(
            jump_target(&s, &rows),
            JumpTarget::App { app: PathBuf::from("/Applications/Visual Studio Code.app"), folder: Some("/Users/u/Projects/api".into()) }
        );
    }

    #[test]
    fn codex_without_an_app_ancestor_does_not_jump() {
        // ターミナルの codex が裏のサーバー（launchd の子）で会話を動かしているとき
        let rows = vec![row(1, 0, "/sbin/launchd"), row(300, 1, "/opt/homebrew/bin/codex app-server")];
        let mut s = session("s1", 300, None, None);
        s.agent = crate::agent::Agent::Codex;
        assert_eq!(jump_target(&s, &rows), JumpTarget::None);
    }
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agent-loaf-core --lib jump`
Expected: `codex_in_vscode_opens_the_project_window` が FAIL（今は `claude` しか移動の対象にしない）

- [ ] **Step 3: 実装する**

`jump_target` を変える。

```rust
    // 3 秒ごとの確認の間に pid が別のプロセスに使い回されると、関係のないアプリを前に出してしまう
    let Some(process) = rows.iter().find(|r| r.pid == pid).filter(|r| is_agent_command(&r.args)) else {
        return JumpTarget::None;
    };
    let Some(app) = outermost_app(rows, pid) else { return JumpTarget::None };
    // エディターの中で動いているときは、そのフォルダーのウィンドウを前に出す。Codex は entrypoint がないので、拡張の中の本体かで決める
    let in_editor = match session.agent {
        Agent::ClaudeCode => terminal.and_then(|t| t.entrypoint.as_deref()) == Some(EDITOR_ENTRYPOINT),
        Agent::Codex => process.args.contains("/extensions/openai.chatgpt-"),
    };
```

（use: `crate::agent::Agent`、`crate::ps::{is_agent_command, ProcRow}`。`is_claude_command` の use は使わなければ消す。）

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test --workspace`
Expected: PASS（今の VS Code・Windsurf・iTerm2・tmux・pid の使い回しのテストも）

- [ ] **Step 5: コミット**

```bash
git add crates/core/src/jump.rs
git commit -m "Codex の会話を押すと、VS Code の拡張の中なら VS Code を前に出してそのフォルダーを開く

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: hooks.json に Codex の知らせを足す

**Files:**
- Modify: `plugin/hooks/hooks.json`, `crates/hook/tests/plugin_manifest.rs`

- [ ] **Step 1: 失敗するテストを書く**

`crates/hook/tests/plugin_manifest.rs`:
- `REQUIRED_EVENTS` に `"Interrupt"` と `"PostCompact"` を足す（`[&str; 17]`）
- テストを足す。

```rust
// Codex は SessionEnd と Interrupt の待ち時間を 3 秒までしか受け付けない
#[test]
fn short_lived_events_wait_at_most_three_seconds() {
    let hooks = load("plugin/hooks/hooks.json");
    for event in ["SessionEnd", "Interrupt"] {
        let timeout = hooks["hooks"][event][0]["hooks"][0]["timeout"].as_u64().unwrap();
        assert!(timeout <= 3, "{event} の timeout は 3 以下にすること（{timeout}）");
    }
}
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agent-loaf-hook --test plugin_manifest`
Expected: FAIL（`Interrupt が登録されていない`、SessionEnd の timeout が 5）

- [ ] **Step 3: 実装する**

`plugin/hooks/hooks.json`:
- `SessionEnd` の `"timeout": 5` を `3` にする
- `ElicitationResult` の行の後に、同じ形で `Interrupt`（`"timeout": 3`）と `PostCompact`（`"timeout": 5`）を足す。コマンドはほかの行と同じ

```json
    "Interrupt": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agent-loaf/bin/agent-loaf-hook\" ] && \"$HOME/.agent-loaf/bin/agent-loaf-hook\"; exit 0", "timeout": 3 }] }],
    "PostCompact": [{ "hooks": [{ "type": "command", "command": "[ -x \"$HOME/.agent-loaf/bin/agent-loaf-hook\" ] && \"$HOME/.agent-loaf/bin/agent-loaf-hook\"; exit 0", "timeout": 5 }] }]
```

（前の行の末尾に `,` を足すこと。コマンドの文字列は今の `hooks.json` の行からそのまま写す。）

Run: `~/.local/bin/claude plugin validate plugin`
Expected: `Validation passed`（`Interrupt` の「unknown hook event; entry ignored at runtime」の警告だけが出る）

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test --workspace`
Expected: PASS（`bundle_resources` のテストも）

- [ ] **Step 5: コミット**

```bash
git add plugin/hooks/hooks.json crates/hook/tests/plugin_manifest.rs
git commit -m "プラグインの hook に Codex の Interrupt と PostCompact を足し、SessionEnd の待ち時間を 3 秒にする

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Codex の導入と許可の判断（core）

**Files:**
- Create: `crates/core/src/codex_plugin.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/install_plan.rs`, `crates/core/src/plugin_status.rs`, `crates/core/src/settings.rs`, `crates/core/src/config_dirs.rs`, `crates/core/src/attention.rs`, `crates/core/src/claude_cli.rs`

**Interfaces:**
- Consumes: Task 1 の `Agent`
- Produces:
  - `codex_plugin::parse_codex_inspection(plugins_json: &str, marketplaces_json: &str, bundled_dir: &Path) -> Result<Inspection, CliError>`
  - `codex_plugin::CODEX_HOOK_EVENTS: [&str; 12]`（snake_case の名前）
  - `codex_plugin::hooks_trusted(config_toml: &str, registered: &[&str]) -> bool`（`registered` は `hooks.json` に登録したイベントの名前（`PreToolUse` など）。Codex が扱うものだけを数える）
  - `codex_plugin::trust_command_script(codex: &Path, codex_home: &Path, work_dir: &Path) -> String`
  - `CliStep::args_for(&self, agent: Agent, bundled_dir: &Path) -> Vec<String>`（`args` は `args_for(Agent::ClaudeCode, …)` の別名として残す）
  - `DirState::NeedsTrust`（`#[serde(rename_all = "snake_case")]` で `"needs_trust"`）
  - `Settings.codex_dir_offered: bool`、`Settings.codex_path: Option<String>`
  - `config_dirs::offer_codex_dir(current: Option<&[String]>, offered: bool, codex_dir: &Path) -> Option<Vec<String>>`
  - `claude_cli::find_codex(configured: Option<&str>, home: &Path, is_executable: impl Fn(&Path) -> bool) -> Option<PathBuf>`
  - `ClaudeCli::with_config_env(self, name: &'static str) -> Self`

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/codex_plugin.rs` を作る（テストだけ先に）。

```rust
use std::path::Path;

use serde::Deserialize;

use crate::claude_cli::CliError;
use crate::plugin_status::{Inspection, MarketplacePresence, PluginPresence, MARKETPLACE_NAME, PLUGIN_ID};

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = "/Users/u/.agent-loaf/marketplace";

    fn plugins(version: &str, enabled: bool) -> String {
        format!(r#"{{"installed":[{{"pluginId":"openai-templates@openai-curated-remote","version":"0.1.1","installed":true,"enabled":true}},{{"pluginId":"agent-loaf@agent-loaf","version":"{version}","installed":true,"enabled":{enabled},"source":{{"source":"local","path":"{BUNDLED}/plugin"}}}}],"available":[]}}"#)
    }

    fn marketplaces(root: &str) -> String {
        format!(r#"{{"marketplaces":[{{"name":"agent-loaf","root":"{root}","marketplaceSource":{{"sourceType":"local","source":"{root}"}}}}]}}"#)
    }

    #[test]
    fn reads_codex_plugin_and_marketplace_lists() {
        let i = parse_codex_inspection(&plugins("0.5.0", true), &marketplaces(BUNDLED), Path::new(BUNDLED)).unwrap();
        assert_eq!(i, Inspection { plugin: PluginPresence::Enabled { version: "0.5.0".into() }, marketplace: MarketplacePresence::Bundled });
        let other = parse_codex_inspection(&plugins("0.5.0", true), &marketplaces("/elsewhere"), Path::new(BUNDLED)).unwrap();
        assert_eq!(other.marketplace, MarketplacePresence::Other);
        let disabled = parse_codex_inspection(&plugins("0.5.0", false), &marketplaces(BUNDLED), Path::new(BUNDLED)).unwrap();
        assert_eq!(disabled.plugin, PluginPresence::Disabled);
        let none = parse_codex_inspection(r#"{"installed":[],"available":[]}"#, r#"{"marketplaces":[]}"#, Path::new(BUNDLED)).unwrap();
        assert_eq!(none, Inspection { plugin: PluginPresence::Absent, marketplace: MarketplacePresence::Absent });
        assert_eq!(parse_codex_inspection("oops", "{}", Path::new(BUNDLED)), Err(CliError::Unreadable));
    }

    const REGISTERED: [&str; 17] = [
        "SessionStart", "SessionEnd", "UserPromptSubmit", "PreToolUse", "PostToolUse", "PostToolUseFailure", "PermissionRequest",
        "Notification", "Stop", "StopFailure", "SubagentStart", "SubagentStop", "PreCompact", "Elicitation", "ElicitationResult",
        "Interrupt", "PostCompact",
    ];

    fn trusted(events: &[&str]) -> String {
        let mut text = String::from("model = \"gpt-6-sol\"\n\n[hooks.state]\n");
        for event in events {
            text.push_str(&format!("[hooks.state.\"agent-loaf@agent-loaf:hooks/hooks.json:{event}:0:0\"]\ntrusted_hash = \"sha256:00\"\n"));
        }
        text
    }

    #[test]
    fn trusted_only_when_every_codex_hook_is_listed() {
        assert!(hooks_trusted(&trusted(&CODEX_HOOK_EVENTS), &REGISTERED));
        assert!(!hooks_trusted(&trusted(&CODEX_HOOK_EVENTS[..11]), &REGISTERED), "1 つ足りない");
        assert!(!hooks_trusted("model = \"x\"\n", &REGISTERED), "許可の記録がない");
        assert!(!hooks_trusted("[[[broken", &REGISTERED), "壊れていても落ちない");
        let other_plugin = trusted(&CODEX_HOOK_EVENTS).replace("agent-loaf@agent-loaf", "other@other");
        assert!(!hooks_trusted(&other_plugin, &REGISTERED), "ほかのプラグインの許可は数えない");
    }

    #[test]
    fn trust_script_opens_codex_in_an_empty_folder_with_quoted_paths() {
        let script = trust_command_script(Path::new("/Users/u/.vscode/extensions/openai.chatgpt-1/bin/codex"), Path::new("/Users/u/.codex"), Path::new("/Users/u/.agent-loaf/codex-trust"));
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains("cd '/Users/u/.agent-loaf/codex-trust'"));
        assert!(script.contains("CODEX_HOME='/Users/u/.codex' exec '/Users/u/.vscode/extensions/openai.chatgpt-1/bin/codex' --no-daemon"));
        let tricky = trust_command_script(Path::new("/x/it's/codex"), Path::new("/c"), Path::new("/w"));
        assert!(tricky.contains("'/x/it'\\''s/codex'"), "シングルクォートを含むパスも壊さない");
    }
}
```

`crates/core/src/lib.rs` に `pub mod codex_plugin;` を足す。

`crates/core/src/install_plan.rs` の `mod tests` に足す。

```rust
    #[test]
    fn codex_steps_use_codex_commands() {
        use crate::agent::Agent;
        let dir = Path::new("/b");
        assert_eq!(CliStep::MarketplaceAdd.args_for(Agent::Codex, dir), ["plugin", "marketplace", "add", "/b"]);
        assert_eq!(CliStep::Install.args_for(Agent::Codex, dir), ["plugin", "add", "agent-loaf@agent-loaf"]);
        // Codex には update がなく、版を上げたマーケットプレイスからもう一度 add すると入れ替わる（2026-10-12 に確認）
        assert_eq!(CliStep::Update.args_for(Agent::Codex, dir), ["plugin", "add", "agent-loaf@agent-loaf"]);
        assert_eq!(CliStep::Enable.args_for(Agent::Codex, dir), ["plugin", "add", "agent-loaf@agent-loaf"]);
        assert_eq!(CliStep::Uninstall.args_for(Agent::Codex, dir), ["plugin", "remove", "agent-loaf@agent-loaf"]);
        assert_eq!(CliStep::MarketplaceRemove.args_for(Agent::Codex, dir), ["plugin", "marketplace", "remove", "agent-loaf"]);
        assert_eq!(CliStep::Install.args_for(Agent::ClaudeCode, dir), CliStep::Install.args(dir));
    }
```

`crates/core/src/settings.rs` の `v01_settings_file_loads_with_new_fields_unset` に 2 行足す: `assert!(!loaded.codex_dir_offered);`、`assert_eq!(loaded.codex_path, None);`

`crates/core/src/config_dirs.rs` の `mod tests` に足す。

```rust
    #[test]
    fn offers_codex_dir_once() {
        let home = tempfile::tempdir().unwrap();
        let codex = home.path().join(".codex");
        let claude = "/u/.claude".to_string();
        assert_eq!(offer_codex_dir(Some(&[claude.clone()]), false, &codex), None, "~/.codex がなければ足さない");
        std::fs::create_dir(&codex).unwrap();
        let codex_s = codex.to_string_lossy().into_owned();
        assert_eq!(offer_codex_dir(Some(&[claude.clone()]), false, &codex), Some(vec![claude.clone(), codex_s.clone()]));
        assert_eq!(offer_codex_dir(None, false, &codex), Some(vec![codex_s.clone()]), "まだ決めていなくても足す");
        assert_eq!(offer_codex_dir(Some(&[claude.clone()]), true, &codex), None, "外した後は足さない");
        assert_eq!(offer_codex_dir(Some(&[codex_s.clone()]), false, &codex), None, "もう入っている");
    }
```

`crates/core/src/attention.rs` の `badge_when_something_needs_the_user` に 1 行足す: `assert!(needs_attention(Some(&[Installed, NeedsTrust]), ok, ok), "Codex の hook の許可が要る");`

`crates/core/src/claude_cli.rs` の `mod tests` に足す。

```rust
    #[test]
    fn finds_codex_in_path_dirs_then_the_newest_editor_extension() {
        let home = tempfile::tempdir().unwrap();
        let ext = |editor: &str, version: &str| home.path().join(format!("{editor}/extensions/openai.chatgpt-{version}-darwin-arm64/bin/macos-aarch64/codex"));
        for path in [ext(".vscode", "26.1007.21434"), ext(".vscode", "26.0901.1"), ext(".cursor", "25.1.1")] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "").unwrap();
        }
        let exists = |p: &Path| p.is_file();
        assert_eq!(find_codex(None, home.path(), exists), Some(ext(".vscode", "26.1007.21434")), "いちばん新しい版");
        let local = home.path().join(".local/bin/codex");
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::write(&local, "").unwrap();
        assert_eq!(find_codex(None, home.path(), exists), Some(local.clone()), "PATH のよくある場所を先に");
        assert_eq!(find_codex(Some("/old/codex"), home.path(), exists), Some(local), "保存した場所が古ければ探し直す");
    }
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agent-loaf-core --lib`
Expected: コンパイルエラー（`parse_codex_inspection` などがない）

- [ ] **Step 3: 実装する**

`crates/core/src/codex_plugin.rs` の `use` の後に足す。

```rust
// Codex が許可の対象にするイベント。hooks.json に Claude Code にしかない名前があっても、Codex は数えない
pub const CODEX_HOOK_EVENTS: [&str; 12] = [
    "session_start", "session_end", "user_prompt_submit", "pre_tool_use", "permission_request", "post_tool_use",
    "stop", "subagent_start", "subagent_stop", "pre_compact", "post_compact", "interrupt",
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

pub fn parse_codex_inspection(plugins_json: &str, marketplaces_json: &str, bundled_dir: &Path) -> Result<Inspection, CliError> {
    let plugins: PluginList = serde_json::from_str(plugins_json).map_err(|_| CliError::Unreadable)?;
    let marketplaces: MarketplaceList = serde_json::from_str(marketplaces_json).map_err(|_| CliError::Unreadable)?;
    let plugin = match plugins.installed.into_iter().find(|p| p.plugin_id == PLUGIN_ID) {
        None => PluginPresence::Absent,
        Some(p) if p.enabled => PluginPresence::Enabled { version: p.version },
        Some(_) => PluginPresence::Disabled,
    };
    let bundled = bundled_dir.to_string_lossy();
    let marketplace = match marketplaces.marketplaces.into_iter().find(|m| m.name == MARKETPLACE_NAME) {
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
    let expected: Vec<String> = registered.iter().map(|e| snake_case(e)).filter(|e| CODEX_HOOK_EVENTS.contains(&e.as_str())).collect();
    !expected.is_empty() && expected.iter().all(|event| listed.contains(&event.as_str()))
}

fn quoted(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

// Codex の許可の画面は VS Code の中にないので、ターミナルで開く。リポジトリの hook を一緒に許可しないよう、空のフォルダーで開く
pub fn trust_command_script(codex: &Path, codex_home: &Path, work_dir: &Path) -> String {
    format!("#!/bin/sh\ncd {}\nCODEX_HOME={} exec {} --no-daemon\n", quoted(work_dir), quoted(codex_home), quoted(codex))
}
```

`crates/core/src/install_plan.rs` の `impl CliStep` に足し、今の `args` はそれを呼ぶ形にする（`use crate::agent::Agent;`）。

```rust
    pub fn args(&self, bundled_dir: &Path) -> Vec<String> {
        self.args_for(Agent::ClaudeCode, bundled_dir)
    }

    pub fn args_for(&self, agent: Agent, bundled_dir: &Path) -> Vec<String> {
        if *self == CliStep::MarketplaceAdd {
            return vec!["plugin".into(), "marketplace".into(), "add".into(), bundled_dir.to_string_lossy().into_owned()];
        }
        let words: Vec<&str> = match (agent, self) {
            (_, CliStep::MarketplaceRemove) => vec!["plugin", "marketplace", "remove", MARKETPLACE_NAME],
            (Agent::ClaudeCode, CliStep::Install) => vec!["plugin", "install", PLUGIN_ID],
            (Agent::ClaudeCode, CliStep::Update) => vec!["plugin", "update", PLUGIN_ID],
            (Agent::ClaudeCode, CliStep::Uninstall) => vec!["plugin", "uninstall", PLUGIN_ID],
            (Agent::ClaudeCode, CliStep::Enable) => vec!["plugin", "enable", PLUGIN_ID],
            // Codex には update と enable がなく、もう一度 add すると新しい版で入れ直し、有効にする
            (Agent::Codex, CliStep::Install | CliStep::Update | CliStep::Enable) => vec!["plugin", "add", PLUGIN_ID],
            (Agent::Codex, CliStep::Uninstall) => vec!["plugin", "remove", PLUGIN_ID],
            (_, CliStep::MarketplaceAdd) => unreachable!(),
        };
        words.into_iter().map(String::from).collect()
    }
```

`crates/core/src/plugin_status.rs` の `DirState` に `NeedsTrust,` を足す。`Inspection::state` は今のまま（許可はアプリの側で重ねて判断する）。

`crates/core/src/settings.rs`: `Settings` に足し、`Default` にも入れる。

```rust
    // ~/.codex を一度自動で足したら true。外した後に、起動のたびに入れ直さないため
    pub codex_dir_offered: bool,
    pub codex_path: Option<String>,
```

`crates/core/src/config_dirs.rs` に足す。

```rust
pub fn offer_codex_dir(current: Option<&[String]>, offered: bool, codex_dir: &Path) -> Option<Vec<String>> {
    let codex = codex_dir.to_string_lossy().into_owned();
    if offered || !codex_dir.is_dir() || current.is_some_and(|dirs| dirs.contains(&codex)) {
        return None;
    }
    let mut dirs = current.map(<[String]>::to_vec).unwrap_or_default();
    dirs.push(codex);
    Some(dirs)
}
```

`crates/core/src/attention.rs` の `dir_problem` の `matches!` に `DirState::NeedsTrust` を足す。

`crates/core/src/claude_cli.rs`:
- `ClaudeCli` に `config_env: &'static str` を足し、`new` では `"CLAUDE_CONFIG_DIR"`、`run` ではその名前を使う。足す。

```rust
    // 同じ実行の仕組みで codex も動かす。設定ディレクトリを渡す環境変数の名前だけが違う
    pub fn with_config_env(mut self, name: &'static str) -> Self {
        self.config_env = name;
        self
    }
```

- `find_codex` を足す。

```rust
const CODEX_EXTENSION_ROOTS: [&str; 3] = [".vscode/extensions", ".cursor/extensions", ".windsurf/extensions"];

pub fn find_codex(configured: Option<&str>, home: &Path, is_executable: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    if let Some(path) = configured.map(PathBuf::from).filter(|path| is_executable(path)) {
        return Some(path);
    }
    let in_path = [
        home.join(".local/bin/codex"),
        PathBuf::from("/opt/homebrew/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
        home.join(".npm-global/bin/codex"),
    ];
    if let Some(path) = in_path.into_iter().find(|path| is_executable(path)) {
        return Some(path);
    }
    // VS Code などの Codex の拡張は、本体を拡張のフォルダーに同梱している。古い版のフォルダーが残ることがあるので、新しい版から試す
    let mut bundled: Vec<(String, PathBuf)> = CODEX_EXTENSION_ROOTS
        .iter()
        .filter_map(|root| std::fs::read_dir(home.join(root)).ok())
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let version = name.strip_prefix("openai.chatgpt-")?.to_string();
            Some((version, entry.path().join("bin/macos-aarch64/codex")))
        })
        .collect();
    bundled.sort_by(|a, b| compare_versions(&b.0, &a.0));
    bundled.into_iter().map(|(_, path)| path).find(|path| is_executable(path))
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |v: &str| v.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    parts(a).cmp(&parts(b))
}
```

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test --workspace`
Expected: PASS（今の `install_plan` の `args` のテストも）

- [ ] **Step 5: コミット**

```bash
git add crates/core/src
git commit -m "Codex のプラグインの状態の読み取り、hook の許可の判断、codex の探し方、~/.codex を一度だけ足す決まりを core に足す

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: アプリから Codex を導入し、許可を確かめる

**Files:**
- Modify: `app/src-tauri/src/installer.rs`, `app/src-tauri/src/commands.rs`, `app/src-tauri/src/main.rs`

**Interfaces:**
- Consumes: Task 1 の `agent_of_config_dir`、`default_codex_dir`。Task 6 のすべて
- Produces:
  - `ConfigDirView.agent: Agent`、`SetupView.codex_found: bool`
  - コマンド `open_codex_trust(path: String)`、`set_codex_path(path: String) -> Result<(), String>`
  - 文言のキー（Task 8 で足す）: `error.codex_not_found`、`error.codex_path_invalid`

- [ ] **Step 1: 設定ディレクトリの種類ごとに CLI を選ぶ**

`installer.rs`:
- `InstallerState` に `codex: Mutex<Option<PathBuf>>` と `trust: Mutex<HashMap<String, bool>>`（設定ディレクトリごとの許可の判断）を足す
- `resolve_claude` と同じ形で `resolve_codex` を足し（`settings.codex_path`・`find_codex`）、`resolve_claude` を呼んでいる 2 か所で一緒に呼ぶ
- `cli(app)` を `cli(app, agent: Agent) -> Option<ClaudeCli>` にする。

```rust
fn cli(app: &AppHandle, agent: Agent) -> Option<ClaudeCli> {
    let installer = app.state::<InstallerState>();
    match agent {
        Agent::ClaudeCode => installer.claude.locked().clone().map(ClaudeCli::new),
        Agent::Codex => installer.codex.locked().clone().map(|p| ClaudeCli::new(p).with_config_env("CODEX_HOME")),
    }
}
```

- `apply` の中で、`let agent = agent_of_config_dir(Path::new(path));` を最初に求め、`cli(app, agent).ok_or(InstallError::NotFound(agent))?` にする（`InstallError::ClaudeNotFound` を `NotFound(Agent)` にし、メッセージは `ClaudeCode` なら `Msg::ErrorClaudeNotFound`、`Codex` なら `Msg::ErrorCodexNotFound`）。`remove` の `Err(InstallError::ClaudeNotFound)` も `Err(InstallError::NotFound(_))` にする
- `run_plan`・`inspect` に `agent` を渡し、手順は `step.args_for(agent, &bundled)`、読み取りは `agent` ごとに `parse_inspection`（Claude Code）か `parse_codex_inspection`（Codex）
- Codex の `inspect` の後に許可を確かめて `trust` に入れる。

```rust
fn check_trust(app: &AppHandle, path: &str) {
    let config = std::fs::read_to_string(Path::new(path).join("config.toml")).unwrap_or_default();
    let hooks_json = std::fs::read_to_string(paths::marketplace_dir(&app.state::<AppState>().home).join("plugin/hooks/hooks.json")).unwrap_or_default();
    let registered: Vec<String> = serde_json::from_str::<serde_json::Value>(&hooks_json)
        .ok()
        .and_then(|v| v.get("hooks")?.as_object().map(|m| m.keys().cloned().collect()))
        .unwrap_or_default();
    let registered: Vec<&str> = registered.iter().map(String::as_str).collect();
    app.state::<InstallerState>().trust.locked().insert(path.to_string(), hooks_trusted(&config, &registered));
}
```

- [ ] **Step 2: 起動のときに ~/.codex を足す**

`startup` の `auto_dir` を求めるところの後に足す。

```rust
    let codex_dir = paths::default_codex_dir().map(PathBuf::from);
    let offered_codex = codex_dir.and_then(|codex| {
        update_settings(app, |settings| {
            let next = offer_codex_dir(settings.config_dirs.as_deref(), settings.codex_dir_offered, &codex)?;
            settings.config_dirs = Some(next);
            settings.codex_dir_offered = true;
            Some(codex.to_string_lossy().into_owned())
        })
    });
    if let Some(dir) = offered_codex {
        let _ = apply(app, &dir, install_steps);
    }
```

（`~/.codex` がないときは `codex_dir_offered` を立てない。後で Codex を入れた人にも 1 回足すため。）

- [ ] **Step 3: 画面に渡す状態**

`view`:
- `ConfigDirView` に `pub agent: Agent,` を足し、`agent_of_config_dir` で入れる
- 状態: Codex で `inspection.state()` が `Installed` かつ `trust` が `Some(false)` なら `DirState::NeedsTrust`
- `SetupView` に `pub codex_found: bool,` を足す（`startup_done` の後に `codex` が `None` でも、一覧に Codex の設定ディレクトリがないときは `true` として扱い、案内を出さない）
- `needs_attention` に渡す `claude_found` は、一覧に Claude Code の設定ディレクトリがあるときだけ見る形にする（Codex だけを使う人に「claude がない」の赤い丸を出さない）。`codex_found` も同じ形で `needs_attention` の判断に入れる（`attention.rs` の引数を `tools_found: Option<bool>` として、claude と codex の両方を合わせた値を渡す。`attention.rs` のテストの呼び出しはそのまま通る）

- [ ] **Step 4: 許可の画面を開くコマンドと、codex の場所の設定**

`commands.rs`:

```rust
#[tauri::command]
pub fn open_codex_trust(app: AppHandle, path: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let codex = installer::codex_path(&app).ok_or_else(|| messages::text(state.lang, Msg::ErrorCodexNotFound))?;
    let work_dir = state.home.join("codex-trust");
    let script = state.home.join("codex-trust.command");
    let fail = |error: std::io::Error| messages::format(state.lang, Msg::ErrorSaveSettings, &[("error", &error.to_string())]);
    paths::prepare_private_dir(&work_dir).map_err(fail)?;
    paths::write_private(&script, trust_command_script(&codex, Path::new(&path), &work_dir).as_bytes()).map_err(fail)?;
    // .command は実行できる権限がないと Terminal.app が開かない
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).map_err(fail)?;
    }
    run_command(Path::new("/usr/bin/open"), &[script.to_string_lossy().into_owned()], &[], Duration::from_secs(5))
        .map(|_| ())
        .map_err(|error| error.message(state.lang))
}
```

（`installer::codex_path(app) -> Option<PathBuf>` は `InstallerState.codex` を返す pub 関数として足す。）

`set_codex_path` は今の `set_claude_path` と同じ形で、`settings.codex_path` に入れ、`Msg::ErrorCodexPathInvalid` を使う。

`main.rs` の `invoke_handler` に `commands::open_codex_trust` と `commands::set_codex_path` を足す。

- [ ] **Step 5: 確かめる**

Run: `cargo test --workspace && cargo build -p agent-loaf-app`
Expected: PASS・ビルドが通る（Task 8 の文言のキーがまだないとコンパイルエラーになるので、`Msg::ErrorCodexNotFound` と `Msg::ErrorCodexPathInvalid` は、この Task で `messages.rs` と `locales/{ja,en}.json` に先に足す）

```json
  "error.codex_not_found": "codex コマンドが見つかりません。設定で場所を入力してください。",
  "error.codex_path_invalid": "その場所に codex コマンドがありません。",
```

```json
  "error.codex_not_found": "The codex command was not found. Enter its path in Settings.",
  "error.codex_path_invalid": "There is no codex command at that path.",
```

- [ ] **Step 6: コミット**

```bash
git add app/src-tauri/src crates/core/locales crates/core/src/messages.rs crates/core/src/attention.rs
git commit -m "Codex の設定ディレクトリにも同じ手順でプラグインを入れ、hook の許可を確かめ、ターミナルで許可の画面を開けるようにする

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: 画面（設定の欄と一覧の行のアイコン）

**Files:**
- Create: `app/src/assets/claude-code.svg`, `app/src/assets/codex.svg`
- Modify: `app/index.html`, `app/src/settings.ts`, `app/src/list.ts`, `app/src/types.ts`, `app/src/style.css`, `crates/core/locales/{ja,en}.json`, `crates/core/src/messages.rs`

- [ ] **Step 1: 文言を足す（失敗するテストから）**

`index.html` の `claude-path-form` の後に足す。

```html
          <form id="codex-path-form" class="claude-path" hidden>
            <p class="menu-note" data-i18n="settings.codex_not_found"></p>
            <input id="codex-path" class="text-input" type="text" placeholder="/opt/homebrew/bin/codex" />
          </form>
```

Run: `cargo test -p agent-loaf-app --test messages_used`
Expected: FAIL（`settings.codex_not_found` がない）

`crates/core/locales/ja.json`:
- `settings.claude_configs` の文言を `"エージェントの設定"` にする
- 足す。

```json
  "settings.codex_not_found": "codex コマンドが見つかりません。場所を入力してください。",
  "settings.codex_trust_needed": "Codex の hook の許可が必要です。ターミナルで Codex を開き、Agent Loaf の hook を許可してください。",
  "settings.codex_trust_open": "ターミナルで許可する",
```

`crates/core/locales/en.json`: `settings.claude_configs` を `"Agent configs"` にし、足す。

```json
  "settings.codex_not_found": "The codex command was not found. Enter its path.",
  "settings.codex_trust_needed": "Codex needs you to trust its hooks. Open Codex in Terminal and trust the Agent Loaf hooks.",
  "settings.codex_trust_open": "Trust in Terminal",
```

Run: `cargo test --workspace`
Expected: PASS

- [ ] **Step 2: アイコンを書き出す**

Figma MCP の `download_assets`（fileKey は非公開、`defaultFormat: "svg"`）で、`157:2477`（claude-code 1）を `app/src/assets/claude-code.svg`、`157:2480`（codex 1）を `app/src/assets/codex.svg` に置く。書き出した SVG にページの背景（`<rect … fill="#BBBBBB"/>`）や枠の点線（`stroke-dasharray`）が入っていれば消す（`scripts/make-tray-icons.sh` の前と同じ）。`grep -c BBBBBB app/src/assets/*.svg` が 0 になること。

- [ ] **Step 3: 一覧の行**

`app/src/types.ts`: `export type Agent = "claude_code" | "codex";` を足し、`SessionView` に `agent: Agent;`、`ConfigDirView` に `agent: Agent;`、`DirState` に `"needs_trust"`、`SetupView` に `codex_found: boolean;` を足す。

`app/src/list.ts`:

```ts
import claudeCodeIcon from "./assets/claude-code.svg";
import codexIcon from "./assets/codex.svg";

const AGENT_ICONS: Record<Agent, { src: string; name: string }> = {
  claude_code: { src: claudeCodeIcon, name: "Claude Code" },
  codex: { src: codexIcon, name: "Codex" },
};

function agentIcon(agent: Agent): HTMLImageElement {
  const icon = document.createElement("img");
  icon.className = "agent-icon";
  icon.src = AGENT_ICONS[agent].src;
  // 名前は製品名なので訳さない
  icon.alt = AGENT_ICONS[agent].name;
  icon.title = AGENT_ICONS[agent].name;
  return icon;
}
```

`renderRow` の `subline.append(el("span", "meta", metaText(session)));` を次にする。

```ts
  const meta = el("span", "meta-with-icon");
  meta.append(agentIcon(session.agent), el("span", "meta", metaText(session)));
  subline.append(meta);
```

`app/src/style.css` に足す（Figma: 16px、文字との間 4px）。

```css
.meta-with-icon { display: flex; align-items: center; gap: 4px; min-width: 0; }
.agent-icon { flex: none; width: 16px; height: 16px; }
```

`.meta` が今 `flex: 1` などで伸びる指定を持っていれば、`.meta-with-icon` に移す（`.meta` の省略の指定 `overflow: hidden; text-overflow: ellipsis` はそのまま `.meta` に残す）。

Vite の型: `app/src/vite-env.d.ts` がなければ作り、`/// <reference types="vite/client" />` を書く（`.svg` の import に型を付けるため。新しいライブラリではない）。

- [ ] **Step 4: 設定の欄**

`app/src/settings.ts` の `renderDir` で、`dir.state === "needs_trust"` のときに、行の後に案内と操作を足す（行と同じ `li` の中に入れると押せる範囲が重なるので、`renderDir` が `HTMLElement[]` を返す形にし、呼ぶ側で `flat()` する）。

```ts
function renderDir(dir: ConfigDirView): HTMLElement[] {
  // （今の行を作る処理。retry は "not_installed" のとき、削除は今のまま）
  const elements: HTMLElement[] = [item];
  if (dir.state === "needs_trust") {
    const note = document.createElement("p");
    note.className = "menu-note";
    note.textContent = t("settings.codex_trust_needed");
    const open = document.createElement("button");
    open.type = "button";
    open.className = "menu-item centered";
    open.textContent = t("settings.codex_trust_open");
    open.addEventListener("click", () => {
      page.keepPageOnFocus();
      void invoke("open_codex_trust", { path: dir.path }).catch((error) => {
        setupErrorEl.hidden = false;
        setupErrorEl.textContent = String(error);
      });
    });
    elements.push(note, open);
  }
  return elements;
}
```

`renderSetup` の `dirsEl.replaceChildren(...setup.dirs.map(renderDir));` を `dirsEl.replaceChildren(...setup.dirs.flatMap(renderDir));` にする。`dirsEl` が `ul` なので、案内と操作も `li` で包む（`const wrap = (el: HTMLElement) => { const li = document.createElement("li"); li.append(el); return li; }`）。

codex の場所の入力欄: `claudeForm` と同じ形で `codexForm`（`codex-path-form`、`codex-path`）を足し、`codexForm.hidden = setup.codex_found;`、送信で `invoke("set_codex_path", { path: codexInput.value })`。

- [ ] **Step 5: 確かめる**

Run: `cd app && npx tsc --noEmit && cd .. && cargo test --workspace`
Expected: 型エラーなし・PASS

見た目は Task 10 でユーザーに見てもらう（一覧の行のアイコン、設定の欄の見出し、許可の案内）。

- [ ] **Step 6: コミット**

```bash
git add app crates/core/locales crates/core/src/messages.rs
git commit -m "一覧の行にエージェントのアイコンを出し、設定の欄を「エージェントの設定」にして Codex の許可の案内を出す

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: 文書

**Files:**
- Modify: `README.md`, `README.en.md`, `docs/superpowers/specs/2026-10-12-agent-loaf-v0.5-distribution-design.md`, `docs/superpowers/specs/2026-10-12-agent-loaf-codex-design.md`

- [ ] **Step 1: README（日本語・英語）**

- 冒頭: 「Claude Code と Codex のセッションの状態を…」にする
- 「必要な環境」: 「Claude Code の `claude` コマンド、または Codex（VS Code などの拡張か `codex` コマンド）」。Codex の本体を探す場所を書く
- 「導入」に Codex の節: 「`~/.codex` があれば、起動したときにプラグインが入ります。Codex は hook を使う前に許可が要るので、設定の画面に『Codex の hook の許可が必要です』が出たら、『ターミナルで許可する』を押し、開いた画面で Agent Loaf の hook を許可してください。許可した後、VS Code の Codex は『Developer: Reload Window』で読み込み直してください。」
- 「設定」: 「Claude Code の設定」を「エージェントの設定」にし、Claude Code と Codex の設定ディレクトリを並べること
- 「既知の制約」に足す
  - Codex で別の会話に移った前の会話は、最大 30 分「完了」のまま一覧に残ります
  - Codex では、hook が届いていない会話（状態不明の行）を出しません
  - ターミナルの Codex は、移動できないことがあります（Task 10 で確かめた結果に合わせる）
  - Agent Loaf の版が上がって hook の中身が変わったときは、もう一度 Codex で許可が要ります
- 「消し方」: 「各設定ディレクトリを削除のアイコンで外すと、Claude Code と Codex のプラグインが外れます。アプリを先に消した場合は、Codex の設定ディレクトリでは `codex plugin remove agent-loaf@agent-loaf` と `codex plugin marketplace remove agent-loaf` を実行してください」
- 英語版も同じ節・同じ中身

Run: `diff <(grep '^#' README.md | wc -l) <(grep '^#' README.en.md | wc -l)`
Expected: 差がない

- [ ] **Step 2: 設計書**

- v0.5 の設計書の 1 章「対象外」から「Codex と macOS 以外への対応」の Codex を外し、「Codex 対応は `2026-10-12-agent-loaf-codex-design.md`。v0.5.0 に含める」と書く
- Codex の設計書の状態を、Task 10 の後に「実装済み」にする

- [ ] **Step 3: コミット**

```bash
git add README.md README.en.md docs/superpowers/specs
git commit -m "README と設計書に Codex 対応を書く

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: 実機で確かめる（ユーザーと一緒に）

- [ ] **Step 1: 作って起動する**

```bash
./scripts/build-hook-sidecar.sh
cd app && npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

動いている Agent Loaf を終了し、`target/release/bundle/macos/Agent Loaf.app` を開く。

- [ ] **Step 2: 導入と許可**

- 設定の画面の「エージェントの設定」に `~/.codex` が足され（`settings.json` の `codex_dir_offered` が true）、`codex plugin list --json` に `agent-loaf@agent-loaf` が入っていること
- 「Codex の hook の許可が必要です」と ≡ の赤い丸が出ること
- 「ターミナルで許可する」を押して Terminal.app が開き、Codex の許可の画面が出ること。空のフォルダーの信頼を聞かれたらどうなるか、「Review hooks」で Agent Loaf の hook だけを許可できるかを見て、案内の文が合っているかを確かめる（合わなければ文言を直してコミットする）
- 許可した後、設定の画面を開き直すと案内と赤い丸が消えること

- [ ] **Step 3: 監視**

ユーザーに、VS Code の Codex のウィンドウを読み込み直してから、次を試してもらう。
- 新しい会話を始めると、一覧に Codex のアイコン付きで出る（新規）
- 依頼すると実行中、`curl` で許可を求められると要対応と通知、許可すると実行中、終わると完了
- 途中で止めると完了（中断）
- 行を押すと VS Code が前に出る
- 同じウィンドウで 2 つ目の会話を始めても、1 つ目の行が残る
- Claude Code の会話も今まで通り出る（アイコンが Claude Code）

`~/.agent-loaf/state.json` で Codex の会話の `agent` が `codex`、`config_dir` が `~/.codex` であることも確かめる。

- [ ] **Step 4: ターミナルの Codex**

ユーザーに、ターミナルで `<codex の本体> --no-daemon` と、`--no-daemon` を付けない起動の両方で会話してもらい、一覧に出るか、行を押すとどうなるかを見る。結果を README の既知の制約に反映してコミットする。

- [ ] **Step 5: 30 分の確認（任意）**

別の会話に移った前の会話が、30 分後に一覧から消えることを、ユーザーの都合がよいときに確かめる（`state.json` で `ended` になる）。

- [ ] **Step 6: 仕上げ**

- Codex の設計書の状態を「実装済み」にしてコミットする
- 最も能力の高いモデルの新しいエージェントに、`main` からこのブランチまでの差分全体を見直してもらう（設計書と計画を渡す）
