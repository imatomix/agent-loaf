# agents-monitor v0.4（公開前の仕上げ）実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 点検（2026-10-11）で見つかった状態の誤り・無駄・壊れやすさ・文書の食い違いを直し、版を 0.4.0 にそろえ、Figma の新しいアイコンを入れる。機能は増やさない。

**Architecture:** 判断はこれまでどおり core の純粋な関数に置き、アプリ（Tauri）と hook はそれを呼ぶだけにする。hook は「書かない」判断（上限・会話以外）を足し、アプリは 3 秒ごとの確認を一覧の開閉で切り替える。保存の形（`state.json`）は前の版のものも読めるようにする。

**Tech Stack:** Rust（core・hook・Tauri 2 アプリ）、libc の proc_* / sysctl、Vite + TypeScript、Figma MCP（アイコンの書き出し）、rsvg-convert、`npx tauri icon`

**Spec:** `docs/superpowers/specs/2026-10-11-agents-monitor-v0.4-design.md`

## Global Constraints

- 常駐するアプリなので、定期的な処理・外部コマンド・描き直しは負荷を見積もってから入れる。軽さのためにやむを得ず妥協するときは、ユーザーに確かめる
- アプリの操作で Claude Code のセッションを作らない
- Claude Code の設定ファイルは直接書き換えない
- 新しいライブラリを追加しない（Rust の crate も npm も）
- 画面の文字色に状態色を使わない。任意の文字列は `textContent` で入れる
- 外部コマンドは引数で渡し、シェルを通さない
- コミットメッセージは日本語。末尾に `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`。コメントは WHY が非自明なときだけ（日本語）
- 版は 0.4.0。公開も 0.x のまま
- push しない（ユーザーの指示まで）

## Review Focus

1. **v0.3 の `state.json`（`"active_subagents": 0` の数）から上げる**→ セッションが消えずに読める（Task 4 でテスト）
2. **ツールの出力に目印があり、その後に本当に中断した**→ 中断と判定する。ツールの出力だけなら判定しない（Task 1 でテスト）
3. **イベントのフォルダーが上限で止まった後にアプリを起動する**→ アプリが読み取って空になれば、hook はまた書き始める（Task 2 でテスト）
4. **一覧を閉じている間に始まった「状態不明」の claude**→ 一覧を開いた直後に出る。閉じている間の部分的な確認で、既知のセッションや状態不明の行が消えない（Task 6 でテストと実機確認）
5. **設定のチェックを続けて 2 つ押す・保存に失敗する**→ 両方が保存される。失敗したら画面は実際に保存された値に戻る（Task 8 で実機確認）

---

## ファイル構成

| ファイル | 役割 | Task |
|---|---|---|
| `crates/core/src/transcript.rs` | 中断の判定を利用者の入力の文だけにする | 1 |
| `crates/core/src/spool.rs` | 上限の確認、古い `.tmp` の削除、600 で書く | 2, 7 |
| `crates/core/src/hook_input.rs` | 会話以外の実行を見分ける `is_conversation` | 2 |
| `crates/hook/src/main.rs`, `crates/hook/tests/cli.rs` | 上限・会話以外で書かない | 2 |
| `crates/core/src/store.rs` | pid なしの 24 時間、終了直後の猶予、中断の時刻、サブエージェントの集まり、`tracked_pids` | 3, 4, 6 |
| `crates/core/src/state.rs` | `Session.running_subagents` | 4 |
| `crates/core/src/procinfo.rs`, `crates/core/src/ps.rs` | 指定した pid だけを読む、引数の読み取りの領域を使い回す | 6 |
| `crates/core/src/paths.rs`, `crates/core/src/persist.rs` | 600 で書く `write_private`、`crash_log_path` | 7 |
| `crates/core/src/crash_log.rs`（新規） | panic の記録（64KB で古い分を消す） | 7 |
| `crates/core/src/messages.rs`, `app/src/i18n.ts` | 置き換えを一度で行う、表を読めなくても動く | 8 |
| `crates/core/src/view.rs` | 詳細の文の組み立てのテスト | 8 |
| `crates/core/src/claude_cli.rs` | 保存した場所が古いと自動で探す | 9 |
| `crates/core/src/jump.rs` | pid が claude か確かめる、`.app/Contents/`、Windsurf・iTerm2 のテスト | 10 |
| `app/src-tauri/src/sync.rs`（新規） | Mutex が壊れても使い続ける `LockExt` | 7 |
| `app/src-tauri/src/monitor.rs` | アイコンを変わったときだけ、確認の切り替え、panic で止まらない | 5, 6, 7 |
| `app/src-tauri/src/tray_icon.rs` | 前に描いた状態を覚える `IconCache` | 5 |
| `app/src-tauri/src/tray.rs` | 一覧の開閉の記録、`show_settings` を消す | 6, 11 |
| `app/src-tauri/src/installer.rs` | 起動時の回数、ないフォルダー、再読み込みのまとめ、claude なしで外す | 5, 9 |
| `app/src-tauri/src/commands.rs` | ログイン時の起動は変わったときだけ | 5 |
| `app/src-tauri/src/jumper.rs` | 移動の途中は受け付けない | 10 |
| `app/src-tauri/src/main.rs` | panic の記録、`windows_subsystem` を消す | 7, 11 |
| `app/src/settings.ts`, `app/src/list.ts` | 設定の順番の保存、開いた詳細の整理 | 8 |
| `app/src-tauri/Info.plist`, `app/src-tauri/tauri.conf.json` | `LSUIElement`、CSP、版 | 11 |
| `app/src-tauri/tests/versions.rs`（新規） | 版がそろっているか | 11 |
| `app/src-tauri/icons/**`, `scripts/make-tray-icons.sh`, `scripts/make-app-icon.mjs`（削除） | アイコン | 12 |
| `README.md`, `README.en.md`, `docs/superpowers/specs/*.md` | 文書 | 14 |

---

### Task 1: 中断の判定を利用者の入力の文だけにする

**Files:**
- Modify: `crates/core/src/transcript.rs`

**Interfaces:**
- Produces: `transcript_ends_with_interrupt(path: &Path) -> bool`（名前と型は今のまま）

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/transcript.rs` の `mod tests` に足す。

```rust
    #[test]
    fn ignores_marker_inside_tool_output() {
        let (_dir, path) = write(&[
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Read","input":{}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"pub const INTERRUPT_MARKER: &str = \"[Request interrupted by user\";"}]}}"#,
        ]);
        assert!(!transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn ignores_marker_in_the_middle_of_a_prompt() {
        let (_dir, path) = write(&[r#"{"type":"user","message":{"content":"ログに [Request interrupted by user] と出る理由は？"}}"#]);
        assert!(!transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn detects_marker_given_as_plain_string_content() {
        let (_dir, path) = write(&[r#"{"type":"user","message":{"content":"[Request interrupted by user]"}}"#]);
        assert!(transcript_ends_with_interrupt(&path));
    }

    #[test]
    fn detects_real_interrupt_after_tool_output_with_marker() {
        let (_dir, path) = write(&[
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"[Request interrupted by user"}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"text","text":"[Request interrupted by user for tool use]"}]}}"#,
            r#"{"type":"system","subtype":"turn_duration"}"#,
        ]);
        assert!(transcript_ends_with_interrupt(&path));
    }
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core transcript`
Expected: `ignores_marker_inside_tool_output` と `ignores_marker_in_the_middle_of_a_prompt` が FAIL（今は行のどこかに含まれれば中断とみなすため）

- [ ] **Step 3: 実装する**

`transcript_ends_with_interrupt` の末尾と `is_message_line` を、次のものに置き換える（1 行を 1 回だけ解析する）。

```rust
    // 中断の目印の後ろには管理用の行（type が system や mode など）が続くため、最後のメッセージ行を探す
    String::from_utf8_lossy(&tail)
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(is_message)
        .is_some_and(|message| is_interrupt(&message))
}

fn is_message(value: &serde_json::Value) -> bool {
    matches!(value.get("type").and_then(|t| t.as_str()), Some("user" | "assistant"))
}

// ツールの結果（tool_result）も type が user の行に入る。ファイルの中身などに目印の文字が含まれることがあるので、利用者の入力の文の先頭だけを見る
fn is_interrupt(message: &serde_json::Value) -> bool {
    if message.get("type").and_then(|t| t.as_str()) != Some("user") {
        return false;
    }
    let starts_with_marker = |text: &str| text.starts_with(INTERRUPT_MARKER);
    match message.pointer("/message/content") {
        Some(serde_json::Value::String(text)) => starts_with_marker(text),
        Some(serde_json::Value::Array(blocks)) => blocks.iter().any(|block| {
            block.get("type").and_then(|t| t.as_str()) == Some("text")
                && block.get("text").and_then(|t| t.as_str()).is_some_and(starts_with_marker)
        }),
        _ => false,
    }
}
```

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test -p agents-monitor-core transcript`
Expected: すべて PASS（前からある `detects_marker_followed_by_bookkeeping_lines`・`detects_tool_use_rejection_marker`・`ignores_marker_before_later_messages`・`missing_file_is_false` も）

- [ ] **Step 5: コミット**

```bash
git add crates/core/src/transcript.rs
git commit -m "中断の判定を、ツールの出力ではなく利用者の入力の文だけで行う

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: hook が上限を超えたときと会話以外の実行では書かない

**Files:**
- Modify: `crates/core/src/spool.rs`
- Modify: `crates/core/src/hook_input.rs`
- Modify: `crates/hook/src/main.rs`
- Test: `crates/hook/tests/cli.rs`

**Interfaces:**
- Produces: `spool::spool_is_full(events_dir: &Path, max_files: usize) -> bool`
- Produces: `hook_input::is_conversation(claude_args: Option<&str>, entrypoint: Option<&str>) -> bool`

- [ ] **Step 1: 失敗するテストを書く（core）**

`crates/core/src/spool.rs` の `mod tests` に足す。

```rust
    #[test]
    fn reports_full_only_at_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!spool_is_full(dir.path(), 2));
        write_event(dir.path(), &ev("Stop", 1)).unwrap();
        assert!(!spool_is_full(dir.path(), 2));
        write_event(dir.path(), &ev("Stop", 2)).unwrap();
        assert!(spool_is_full(dir.path(), 2));
        drain_spool(dir.path(), MAX_SPOOL_FILES);
        assert!(!spool_is_full(dir.path(), 2), "アプリが読み取ったらまた書ける");
        assert!(!spool_is_full(Path::new("/nonexistent/agents-monitor"), 2));
    }
```

`crates/core/src/hook_input.rs` の `mod tests` に足す。

```rust
    #[test]
    fn tells_conversations_from_scripted_runs() {
        let vscode = "/Users/u/.vscode/extensions/anthropic.claude-code-2.1.296-darwin-arm64/resources/native-binary/claude --output-format stream-json --input-format stream-json";
        assert!(is_conversation(Some("/Users/u/.local/bin/claude"), Some("cli")));
        assert!(is_conversation(Some(vscode), Some("claude-vscode")));
        assert!(!is_conversation(Some("/Users/u/.local/bin/claude -p hello"), Some("cli")));
        assert!(!is_conversation(Some("/Users/u/.local/bin/claude --print hello"), None));
        assert!(!is_conversation(Some(vscode), Some("sdk-ts")));
        assert!(!is_conversation(None, Some("sdk-cli")));
        assert!(is_conversation(None, None), "claude が見つからないときは見分けられないので書く");
    }
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core spool_is_full tells_conversations`
Expected: コンパイルエラー（`spool_is_full`・`is_conversation` がない）

- [ ] **Step 3: 実装する（core）**

`crates/core/src/spool.rs` の `write_event` の前に足す。

```rust
// アプリが止まっていても hook は動き続けるので、上限を超えたら書くのをやめて ~/.agents-monitor が際限なく大きくならないようにする
pub fn spool_is_full(events_dir: &Path, max_files: usize) -> bool {
    fs::read_dir(events_dir).is_ok_and(|entries| entries.take(max_files).count() >= max_files)
}
```

`crates/core/src/hook_input.rs` に足す（`use crate::ps::is_interactive_session;` を先頭の use に加える）。

```rust
// claude -p や Agent SDK からの実行は、人が向き合う会話ではないので一覧にも通知にも出さない
pub fn is_conversation(claude_args: Option<&str>, entrypoint: Option<&str>) -> bool {
    if entrypoint.is_some_and(|e| e.starts_with("sdk-")) {
        return false;
    }
    claude_args.is_none_or(is_interactive_session)
}
```

- [ ] **Step 4: 通ることを確かめる（core）**

Run: `cargo test -p agents-monitor-core`
Expected: PASS

- [ ] **Step 5: 失敗するテストを書く（hook）**

`crates/hook/tests/cli.rs` の `run_hook` を、環境変数を足せる形にする。

```rust
fn run_hook(home: &Path, stdin: &[u8]) -> (Output, Duration) {
    run_hook_with(home, stdin, &[])
}

fn run_hook_with(home: &Path, stdin: &[u8], envs: &[(&str, &str)]) -> (Output, Duration) {
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_agents-monitor-hook"))
        .env("AGENTS_MONITOR_HOME", home)
        .env("CLAUDE_CONFIG_DIR", "/tmp/test-claude-config")
        .env_remove("AGENTS_MONITOR_DEBUG_RAW")
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
```

テストを足す。

```rust
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
    for i in 0..agents_monitor_core::spool::MAX_SPOOL_FILES {
        fs::write(events.join(format!("{i:013}-1-1.json")), b"{}").unwrap();
    }
    let (output, _) = run_hook(home.path(), PRE_TOOL_USE);
    assert_silent_success(&output);
    assert_eq!(event_files(home.path()).len(), agents_monitor_core::spool::MAX_SPOOL_FILES);
}
```

hook の dev-dependencies に core がないときは、`crates/hook/Cargo.toml` の `[dependencies]` に core があるので `agents_monitor_core::spool::MAX_SPOOL_FILES` はそのまま使える。

- [ ] **Step 6: 失敗を確かめる**

Run: `cargo test -p agents-monitor-hook`
Expected: `writes_nothing_for_agent_sdk_runs` と `writes_nothing_when_spool_is_full` が FAIL

- [ ] **Step 7: 実装する（hook）**

`crates/hook/src/main.rs` の `run` を変える。`claude` を見つけた直後に会話かどうかを見て、ブランチを読むなどの手間の前に終える。書く前に上限を見る。

```rust
    // hook はツールを使うたびに動くので、全プロセスではなく自分の親だけをたどる
    let rows = ancestor_rows(std::process::id());
    let claude = find_claude_ancestor(&rows, std::process::id());
    let entrypoint = std::env::var("CLAUDE_CODE_ENTRYPOINT").ok();
    if !is_conversation(claude.map(|c| c.args.as_str()), entrypoint.as_deref()) {
        return None;
    }
```

```rust
    let events_dir = paths::events_dir(&home);
    paths::prepare_private_dir(&events_dir).ok()?;
    if spool_is_full(&events_dir, MAX_SPOOL_FILES) {
        return None;
    }
    write_event(&events_dir, &event).ok()?;
```

use を足す: `use agents_monitor_core::hook_input::{build_event, is_conversation, HookContext};`、`use agents_monitor_core::spool::{spool_is_full, write_event, MAX_SPOOL_FILES};`

- [ ] **Step 8: 通ることを確かめる**

Run: `cargo test -p agents-monitor-hook`
Expected: PASS。前からある「hook が速い」テストも通ること（上限の確認は、空に近いフォルダーなら一瞬）

- [ ] **Step 9: コミット**

```bash
git add crates/core/src/spool.rs crates/core/src/hook_input.rs crates/hook/src/main.rs crates/hook/tests/cli.rs
git commit -m "hook は、イベントが上限まで溜まったときと、claude -p や Agent SDK からの実行では書かない

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: セッションの寿命と時刻の誤りを直す

**Files:**
- Modify: `crates/core/src/store.rs`

**Interfaces:**
- Produces: `pub const ORPHAN_TIMEOUT_MS: u64`、`pub const ENDED_PID_GRACE_MS: u64`（store.rs）
- `reconcile`・`detect_interrupts` の引数は今のまま

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/store.rs` の `mod tests` に足す（`ProcRow` は `crate::ps::ProcRow`）。

```rust
    fn claude_row(pid: u32) -> crate::ps::ProcRow {
        crate::ps::ProcRow { pid, ppid: 1, tty: None, args: "/Users/u/.local/bin/claude".into() }
    }

    #[test]
    fn session_without_pid_ends_after_a_day_of_silence() {
        let mut store = SessionStore::default();
        let mut e = ev("UserPromptSubmit", "s1", 1_000);
        e.claude_pid = None;
        store.apply(&e);
        assert!(!store.reconcile(&[], 1_000 + ORPHAN_TIMEOUT_MS - 1, |_| None));
        assert_eq!(store.sessions()[0].status, Status::Running);
        assert!(store.reconcile(&[], 1_000 + ORPHAN_TIMEOUT_MS, |_| None));
        assert_eq!(store.sessions()[0].status, Status::Ended);
    }

    #[test]
    fn idle_session_with_live_pid_stays() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1_000));
        store.reconcile(&[claude_row(100)], 1_000 + 7 * ORPHAN_TIMEOUT_MS, |_| None);
        assert_eq!(store.sessions()[0].status, Status::Done);
    }

    #[test]
    fn just_ended_session_does_not_turn_into_an_unknown_row() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1_000));
        store.apply(&ev("SessionEnd", "s1", 2_000));
        store.reconcile(&[claude_row(100)], 2_000 + ENDED_PID_GRACE_MS - 1, |_| None);
        assert!(store.sessions().iter().all(|s| !s.id.starts_with(UNKNOWN_ID_PREFIX)));
        store.reconcile(&[claude_row(100)], 2_000 + ENDED_PID_GRACE_MS, |_| None);
        assert!(store.sessions().iter().any(|s| s.id == "pid:100"), "猶予の後もまだ動いていれば状態不明として出す");
    }

    #[test]
    fn prompt_written_just_before_interrupt_check_is_kept() {
        let mut store = SessionStore::default();
        let mut e = ev("UserPromptSubmit", "s1", 1_000);
        e.transcript_path = Some("/t.jsonl".into());
        store.apply(&e);
        let now = 1_000 + INTERRUPT_QUIET_MS + 500;
        assert!(store.detect_interrupts(now, INTERRUPT_QUIET_MS, |_| true));
        // 判定の直前（now より前）に書かれ、判定の後に読まれた入力の知らせ
        store.apply(&ev("UserPromptSubmit", "s1", now - 100));
        assert_eq!(store.sessions()[0].status, Status::Running);
    }
```

`SessionEnd` が `Ended` になること（`transition_for`）は今のまま。`ev` の `claude_pid` は `Some(100)`。

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core store::tests`
Expected: コンパイルエラー（定数がない）。定数だけ先に足すと、4 つとも FAIL

- [ ] **Step 3: 実装する**

定数を足す。

```rust
// pid が一度も分からなかったセッションは生きているか確かめられない。強制終了で終了の知らせが来ないと残り続けるので、長く静かなら終わったとみなす
pub const ORPHAN_TIMEOUT_MS: u64 = 24 * 60 * 60 * 1000;
// 終了の知らせの後もプロセスは後片付けでしばらく残り、/clear では次の開始の知らせまで間がある。その間に状態不明の行を出さない
pub const ENDED_PID_GRACE_MS: u64 = 30_000;
```

`reconcile` のループと `known` を変える。

```rust
        for session in self.sessions.iter_mut() {
            let dead = session.claude_pid.is_some_and(|pid| !alive.contains(&pid));
            let orphaned = session.claude_pid.is_none() && now_ms.saturating_sub(session.last_event_ms) >= ORPHAN_TIMEOUT_MS;
            if (dead || orphaned) && session.status != Status::Ended {
                mark_ended(session, now_ms);
                changed = true;
            }
        }
        let known: HashSet<u32> = self
            .sessions
            .iter()
            .filter(|s| s.status != Status::Ended || now_ms.saturating_sub(s.status_since_ms) < ENDED_PID_GRACE_MS)
            .filter_map(|s| s.claude_pid)
            .collect();
```

`detect_interrupts` の時刻を変える。

```rust
                session.status = Status::Done;
                // now にすると、判定の直前に書かれて直後に読まれた入力の知らせを古いものとして捨ててしまう
                session.status_since_ms = session.last_event_ms + 1;
```

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test -p agents-monitor-core`
Expected: PASS（前からある reconcile・detect_interrupts のテストも）

- [ ] **Step 5: コミット**

```bash
git add crates/core/src/store.rs
git commit -m "pid の分からないセッションを 24 時間で終了とし、終了直後の状態不明の行と、中断の判定の時刻のずれを直す

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: サブエージェントを ID の集まりで持つ

**Files:**
- Modify: `crates/core/src/state.rs:105`
- Modify: `crates/core/src/store.rs`（`new_session`・`observe`・`enter`・`unknown_process_session`・テスト）
- Modify: `crates/core/src/jump.rs:91`（テストの Session）
- Modify: `crates/core/src/persist.rs`（前の版の読み込みテスト）

**Interfaces:**
- Produces: `Session.running_subagents: BTreeSet<String>`（`#[serde(default)]`）。`active_subagents: u32` はなくす

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/store.rs` の `mod tests` に足す。

```rust
    fn subagent(event: &str, session: &str, ts_ms: u64, agent: &str) -> MonitorEvent {
        let mut e = ev(event, session, ts_ms);
        e.agent_id = Some(agent.into());
        e
    }

    #[test]
    fn unrelated_subagent_stop_keeps_the_background_label() {
        let mut store = SessionStore::default();
        store.apply(&subagent("SubagentStart", "s1", 1, "a1"));
        store.apply(&subagent("SubagentStop", "s1", 2, "other"));
        store.apply(&ev("Stop", "s1", 3));
        assert_eq!(status_of(&store, "s1"), (Status::Done, Some("バックグラウンド作業あり".into())));
    }

    #[test]
    fn new_session_start_forgets_subagents_whose_stop_was_missed() {
        let mut store = SessionStore::default();
        store.apply(&subagent("SubagentStart", "s1", 1, "a1"));
        store.apply(&start("s1", 2, "clear"));
        store.apply(&ev("Stop", "s1", 3));
        assert_eq!(status_of(&store, "s1"), (Status::Done, None));
    }

    #[test]
    fn compact_keeps_running_subagents() {
        let mut store = SessionStore::default();
        store.apply(&subagent("SubagentStart", "s1", 1, "a1"));
        store.apply(&start("s1", 2, "compact"));
        assert_eq!(store.sessions()[0].running_subagents.len(), 1);
    }
```

「バックグラウンド作業あり」は `locales/ja.json` の `detail.background` の文言に合わせる（違えば、その文言に直す）。

前からある `stop_with_active_subagent_mentions_background` と `subagent_stop_without_start_keeps_count_at_zero` は、`SubagentStart`・`SubagentStop` に `agent_id` を入れる形（上の `subagent(...)`）に直し、後者は名前を `subagent_stop_without_start_is_ignored`、確かめる値を `assert!(store.sessions()[0].running_subagents.is_empty())` にする。

`crates/core/src/persist.rs` の `v02_state_with_string_detail_still_loads` の後に足す。

```rust
    #[test]
    fn v03_state_with_subagent_count_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let old = r#"{"sessions":[{"id":"s1","status":"done","detail_kind":null,"detail_text":null,"status_since_ms":1,"first_seen_ms":1,"last_event_ms":1,"cwd":"/w/api","start_cwd":"/w/api","config_dir":"/u/.claude","git_branch":null,"claude_pid":7,"terminal":null,"transcript_path":null,"active_subagents":2}]}"#;
        fs::write(dir.path().join("state.json"), old).unwrap();
        let store: SessionStore = load_json(&dir.path().join("state.json"));
        assert_eq!(store.sessions().len(), 1);
        assert!(store.sessions()[0].running_subagents.is_empty());
    }
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core`
Expected: コンパイルエラー（`running_subagents` がない）

- [ ] **Step 3: 実装する**

`crates/core/src/state.rs` の `Session` で、`active_subagents: u32` を次に置き換える（`use std::collections::BTreeSet;` を足す）。

```rust
    // 数で数えると、開始の知らせのない終了の知らせ（毎ターンのように届く）で、動いているものまで打ち消してしまう
    #[serde(default)]
    pub running_subagents: BTreeSet<String>,
```

前の版の `active_subagents` は、serde が知らない項目として読み飛ばす（`deny_unknown_fields` は付いていない）。

`crates/core/src/store.rs`:
- `new_session` と `unknown_process_session` の `active_subagents: 0,` を `running_subagents: Default::default(),` にする
- `observe` の `match ev.event.as_str() { … }` を置き換える。

```rust
    match (ev.event.as_str(), ev.agent_id.as_deref()) {
        ("SubagentStart", Some(agent)) => {
            session.running_subagents.insert(agent.to_string());
        }
        ("SubagentStop", Some(agent)) => {
            session.running_subagents.remove(agent);
        }
        // 会話が始め直されたら前のサブエージェントは残っていない。圧縮では会話が続くので残す
        ("SessionStart", _) if ev.detail.source.as_deref() != Some("compact") => session.running_subagents.clear(),
        _ => {}
    }
```

- `enter` の `session.active_subagents > 0` を `!session.running_subagents.is_empty()` にする

`crates/core/src/jump.rs:91` のテストの `active_subagents: 0,` を `running_subagents: Default::default(),` にする。ほかにも残っていないか確かめる。

Run: `grep -rn "active_subagents" crates app/src-tauri/src`
Expected: `persist.rs` のテストの JSON 文字列の中だけ

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test --workspace`
Expected: PASS（`fixtures_replay` の「バックグラウンド作業あり」の場面も）。`fixtures_replay` が数の前提で落ちたら、その fixture の `SubagentStart`／`SubagentStop` の `agent_id` の対応を見て、期待する表示が正しいほうに直す

- [ ] **Step 5: コミット**

```bash
git add crates/core/src/state.rs crates/core/src/store.rs crates/core/src/jump.rs crates/core/src/persist.rs
git commit -m "動いているサブエージェントを、数ではなく ID の集まりで覚える

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: アイコン・起動時の claude・ログイン時の起動の無駄をなくす

**Files:**
- Modify: `app/src-tauri/src/tray_icon.rs`
- Modify: `app/src-tauri/src/monitor.rs`（`AppState`、`load_state`、`publish`）
- Modify: `app/src-tauri/src/installer.rs`（`startup`、`apply`、`run_plan`）
- Modify: `app/src-tauri/src/commands.rs`（`save_settings`）

**Interfaces:**
- Produces: `tray_icon::IconCache`（`Default`、`fn needs_update(&mut self, status: Option<Status>) -> bool`）
- Produces: `AppState.tray_icon: Mutex<IconCache>`
- `run_plan` は `Result<usize, InstallError>`（実行した手順の数）を返す

- [ ] **Step 1: 失敗するテストを書く**

`app/src-tauri/src/tray_icon.rs` の `mod tests` に足す。

```rust
    #[test]
    fn icon_is_redrawn_only_when_status_changes() {
        let mut cache = IconCache::default();
        assert!(cache.needs_update(None), "最初の 1 回は描く");
        assert!(!cache.needs_update(None));
        assert!(cache.needs_update(Some(Status::Running)));
        assert!(!cache.needs_update(Some(Status::Running)));
        assert!(cache.needs_update(Some(Status::NeedsAction)));
    }
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-app tray_icon`
Expected: コンパイルエラー（`IconCache` がない）

- [ ] **Step 3: 実装する（アイコン）**

`app/src-tauri/src/tray_icon.rs` に足す。

```rust
// ツールを使うたびに一覧が変わるが、アイコンが変わるのは状態の種類が変わったときだけ。毎回 PNG を読み直して設定し直さない
#[derive(Default)]
pub struct IconCache {
    shown: Option<Option<Status>>,
}

impl IconCache {
    pub fn needs_update(&mut self, status: Option<Status>) -> bool {
        if self.shown == Some(status) {
            return false;
        }
        self.shown = Some(status);
        true
    }
}
```

`monitor.rs` の `AppState` に `pub tray_icon: Mutex<IconCache>,` を足し、`load_state` で `tray_icon: Mutex::new(IconCache::default()),` を入れる（`use crate::tray_icon::{tray_icon, IconCache};`）。`publish` のアイコンの部分を変える。

```rust
    if state.tray_icon.lock().unwrap().needs_update(tray_status) {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            // set_icon だけだとテンプレート指定が外れ、明るいメニューバーで黒く表示されなくなる
            let _ = tray.set_icon_with_as_template(Some(tray_icon(tray_status)), true);
        }
    }
```

`tray::setup_tray` は起動時に `tray_icon(None)` で作るので、`main.rs` の最初の `monitor::publish` で `None` の後に実際の状態が描かれる。最初の `needs_update(None)` が true になるのは、作ったときと同じ絵をもう一度描くだけなので害はない。

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test -p agents-monitor-app`
Expected: PASS

- [ ] **Step 5: 起動時の claude の回数を減らす**

`installer.rs` の `run_plan` を、実行した手順の数を返すようにする。

```rust
fn run_plan(app: &AppHandle, cli: &ClaudeCli, path: &str, steps: impl Fn(&Inspection) -> Vec<CliStep>) -> Result<usize, InstallError> {
    let plan = steps(&inspect(app, cli, path)?);
    if plan.iter().any(CliStep::needs_bundled_copy) {
        if let Some(error) = app.state::<InstallerState>().marketplace_error.lock().unwrap().clone() {
            return Err(InstallError::MarketplaceUnavailable(error));
        }
    }
    let bundled = paths::marketplace_dir(&app.state::<AppState>().home);
    plan.iter().try_for_each(|step| cli.run(Path::new(path), &step.args(&bundled), CLI_TIMEOUT).map(|_| ()))?;
    Ok(plan.len())
}
```

`apply` を変える。ないフォルダーでは claude を動かさず、何もしなかったときは読み直さない。

```rust
fn apply(app: &AppHandle, path: &str, steps: impl Fn(&Inspection) -> Vec<CliStep>) -> Result<(), InstallError> {
    // ないフォルダーに claude を動かしても失敗するだけ。画面には「見つからない」と出る
    if !Path::new(path).is_dir() {
        return Ok(());
    }
    let installer = app.state::<InstallerState>();
    let cli = cli(app).ok_or(InstallError::ClaudeNotFound)?;
    // 状態を読む間も含めて押せなくする。読み終わるまで押せると、同じ操作が重ねて積まれる
    installer.busy.lock().unwrap().insert(path.to_string());
    publish(app);
    let result = run_plan(app, &cli, path, steps);
    installer.busy.lock().unwrap().remove(path);
    // 何も実行しなかったときは、計画を決めるときに読んだ状態がそのまま今の状態
    if !matches!(result, Ok(0)) {
        let _ = inspect(app, &cli, path);
    }
    result.map(|_| ())
}
```

`startup` の `refresh_all(app);` と、その後の 2 つの `apply` を次に置き換える。`apply` が最初に状態を読むので、前もって全部を読む必要はない。

```rust
    if let Some(dir) = auto_dir {
        let _ = apply(app, &dir, install_steps);
    }
    match bundled_plugin_version(&paths::marketplace_dir(&state.home)) {
        Some(version) => {
            for dir in config_dirs(app) {
                let _ = apply(app, &dir, |inspection| update_steps(inspection, &version));
            }
        }
        None => refresh_all(app),
    }
```

`resolve_claude(app)` は `startup` の前半ですでに呼んでいるので、`refresh_all` を通らなくても claude は見つかっている。

- [ ] **Step 6: ログイン時の起動を、変わったときだけ書き直す**

`commands.rs` の `save_settings` の先頭 3 行を置き換える。

```rust
    let was_enabled = state.settings.lock().unwrap().launch_at_login;
    // 通知の設定だけを変えたときにログイン項目を書き直さない
    if settings.launch_at_login != was_enabled {
        let autostart = app.autolaunch();
        let result = if settings.launch_at_login { autostart.enable() } else { autostart.disable() };
        result.map_err(|error| messages::format(state.lang, Msg::ErrorAutostart, &[("error", &error.to_string())]))?;
    }
```

- [ ] **Step 7: 確かめる**

Run: `cargo test --workspace && cargo build -p agents-monitor-app`
Expected: PASS・ビルドが通る

- [ ] **Step 8: コミット**

```bash
git add app/src-tauri/src/tray_icon.rs app/src-tauri/src/monitor.rs app/src-tauri/src/installer.rs app/src-tauri/src/commands.rs
git commit -m "アイコンは状態が変わったときだけ描き、起動時の claude の呼び出しとログイン項目の書き直しを減らす

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: 一覧を閉じている間は、知っている pid だけを確かめる

**Files:**
- Modify: `crates/core/src/procinfo.rs`
- Modify: `crates/core/src/ps.rs`
- Modify: `crates/core/src/store.rs`
- Modify: `app/src-tauri/src/monitor.rs`
- Modify: `app/src-tauri/src/tray.rs`

**Interfaces:**
- Produces: `procinfo::process_rows(pids: &[u32]) -> Vec<ProcRow>`（端末名なし）
- Produces: `ps::tracked_process_rows(pids: &[u32]) -> Option<Vec<ProcRow>>`
- Produces: `SessionStore::tracked_pids(&self) -> Vec<u32>`
- Produces: `AppState.list_open: AtomicBool`、`tray::set_list_open(app: &AppHandle, open: bool)`、`monitor::check_processes_now(app: &AppHandle)`

- [ ] **Step 1: 直す前の負荷を測る**

今の main のアプリ（`target/release/bundle/macos/agents-monitor.app`）が動いている状態で、一覧を閉じたまま 5 分間の CPU 時間の増え方を測り、記録する。

```bash
PID=$(pgrep -f "agents-monitor.app/Contents/MacOS/agents-monitor-app"); ps -o time= -p $PID; sleep 300; ps -o time= -p $PID
```

2 つの値の差（5 分間に使った CPU 時間）を、このあとの比べる値としてメモする。Claude Code のセッションが動いている（`claude` か `node` のプロセスがいくつかある）状態で測る。

- [ ] **Step 2: 失敗するテストを書く**

`crates/core/src/procinfo.rs` の `mod tests` に足す。

```rust
    #[test]
    fn reads_only_the_requested_processes() {
        let rows = process_rows(&[me(), u32::MAX]);
        assert_eq!(rows.len(), 1, "ないプロセスは返さない");
        assert_eq!(rows[0].pid, me());
        assert_eq!(rows[0].ppid, std::os::unix::process::parent_id());
        assert!(rows[0].args.starts_with(std::env::current_exe().unwrap().to_str().unwrap()));
        assert_eq!(rows[0].tty, None);
    }
```

`crates/core/src/store.rs` の `mod tests` に足す。

```rust
    #[test]
    fn partial_check_keeps_known_and_unknown_rows() {
        let mut store = SessionStore::default();
        store.apply(&ev("Stop", "s1", 1_000));
        store.reconcile(&[claude_row(100), claude_row(200)], 2_000, |_| None);
        assert!(store.sessions().iter().any(|s| s.id == "pid:200"));
        let pids = store.tracked_pids();
        assert_eq!(pids, vec![100, 200]);
        // 一覧を閉じている間は、知っている pid の行だけを渡す
        store.reconcile(&[claude_row(100), claude_row(200)], 5_000, |_| None);
        assert_eq!(status_of(&store, "s1").0, Status::Done);
        assert!(store.sessions().iter().any(|s| s.id == "pid:200"));
        store.reconcile(&[claude_row(100)], 8_000, |_| None);
        assert!(store.sessions().iter().all(|s| s.id != "pid:200"), "終わった状態不明の行は消える");
    }
```

- [ ] **Step 3: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core reads_only_the_requested partial_check`
Expected: コンパイルエラー（`process_rows`・`tracked_pids` がない）

- [ ] **Step 4: 実装する（core）**

`crates/core/src/procinfo.rs` の macOS の `imp` に足す。あわせて、引数を読む領域（`KERN_ARGMAX`、1MB ほど）を毎回確保し直さないようにする。

```rust
    thread_local! {
        // KERN_ARGMAX は 1MB ほどあり、claude や node のプロセスごとに確保し直すと 3 秒ごとに数 MB を埋めることになる
        static ARGS_BUFFER: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    pub fn process_args(pid: u32) -> Option<String> {
        ARGS_BUFFER.with_borrow_mut(|buf| {
            buf.resize(arg_max(), 0);
            let mut size = buf.len();
            let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid as libc::c_int];
            let ok = unsafe { libc::sysctl(mib.as_mut_ptr(), 3, buf.as_mut_ptr().cast(), &mut size, std::ptr::null_mut(), 0) } == 0;
            if !ok {
                return None;
            }
            parse_procargs2(&buf[..size])
        })
    }

    pub fn process_rows(pids: &[u32]) -> Vec<ProcRow> {
        pids.iter()
            .filter_map(|&pid| {
                let info = bsd_info(pid)?;
                let args = process_args(pid).unwrap_or_else(|| info.comm.clone());
                Some(row(pid, info, args, false))
            })
            .collect()
    }
```

（今の `process_args` はこの形に置き換える。）macOS 以外の `imp` に `pub fn process_rows(_pids: &[u32]) -> Vec<ProcRow> { Vec::new() }` を足し、`pub use imp::{ancestor_rows, list_processes, process_rows, working_directory};` にする。

`crates/core/src/ps.rs` の `run_ps` の後に足す。

```rust
pub fn tracked_process_rows(pids: &[u32]) -> Option<Vec<ProcRow>> {
    #[cfg(target_os = "macos")]
    {
        Some(crate::procinfo::process_rows(pids))
    }
    #[cfg(not(target_os = "macos"))]
    {
        run_ps().map(|rows| rows.into_iter().filter(|row| pids.contains(&row.pid)).collect())
    }
}
```

`crates/core/src/store.rs` の `impl SessionStore` に足す。

```rust
    pub fn tracked_pids(&self) -> Vec<u32> {
        let mut pids: Vec<u32> = self.sessions.iter().filter_map(|s| s.claude_pid).collect();
        pids.sort_unstable();
        pids.dedup();
        pids
    }
```

- [ ] **Step 5: 通ることを確かめる（core）**

Run: `cargo test -p agents-monitor-core`
Expected: PASS

- [ ] **Step 6: 実装する（アプリ）**

`monitor.rs` の `AppState` に `pub list_open: AtomicBool,` を足し、`load_state` で `list_open: AtomicBool::new(false),` を入れる。

`run_tick` の `if check_processes { … }` を、次の関数を呼ぶ形にする。

```rust
        if check_processes {
            changed |= reconcile_processes(&mut store, now, state.list_open.load(Ordering::Relaxed));
            changed |= store.detect_interrupts(now, INTERRUPT_QUIET_MS, transcript_ends_with_interrupt);
        }
```

```rust
// 全プロセスを読むのは状態不明の行を見つけるためだけで、それは一覧にしか出ない。閉じている間は知っている pid が生きているかだけを見る
fn reconcile_processes(store: &mut SessionStore, now: u64, list_open: bool) -> bool {
    let rows = if list_open { run_ps() } else { tracked_process_rows(&store.tracked_pids()) };
    rows.is_some_and(|rows| store.reconcile(&rows, now, process_cwd))
}

pub fn check_processes_now(app: &AppHandle) {
    let state = app.state::<AppState>();
    let changed = reconcile_processes(&mut state.store.lock().unwrap(), paths::now_ms(), true);
    if changed {
        publish(app);
    }
}
```

use を足す: `use std::sync::atomic::Ordering;`、`use agents_monitor_core::ps::{process_cwd, run_ps, tracked_process_rows};`

`tray.rs` に足し、一覧を開け閉めするすべての場所で呼ぶ。

```rust
// 監視のスレッドから window.is_visible() を呼ぶと、メインスレッドを待つ間に store のロックを持ったままになりうるので、開閉は自分で記録する
pub fn set_list_open(app: &AppHandle, open: bool) {
    app.state::<AppState>().list_open.store(open, Ordering::Relaxed);
}
```

- `setup_windows` のフォーカスを失って閉じるところ（`let _ = window.hide();` の後）で `set_list_open(&handle, false);`
- `toggle_list` の `window.hide()` の後で `set_list_open(app, false);`
- `hide_list` の `window.hide()` の後で `set_list_open(app, false);`
- `show_list` の最後に次を足す。開いた直後に状態不明の行を出すため、全プロセスを 1 回読む（メインスレッドを止めないよう別のスレッドで）。

```rust
    set_list_open(app, true);
    let handle = app.clone();
    std::thread::spawn(move || crate::monitor::check_processes_now(&handle));
```

- [ ] **Step 7: 確かめる**

Run: `cargo test --workspace && cargo build -p agents-monitor-app`
Expected: PASS・ビルドが通る

- [ ] **Step 8: 直した後の負荷を測り、比べる**

`./scripts/build-hook-sidecar.sh && cd app && npm run tauri build` で作り直し、動いているアプリを終了してから `target/release/bundle/macos/agents-monitor.app` を開く。Step 1 と同じ条件（同じくらいのセッションの数、一覧を閉じたまま 5 分）で測る。

- Step 1 より CPU 時間が目に見えて減った（目安: 2 割以上）: 次へ進む
- ほとんど変わらない: この Task の変更を `git checkout -- .` で取り消し、測った値をユーザーに報告して、取りやめるか確かめる（設計書 4.4）

あわせて実機で確かめる: 一覧を閉じている間に新しいターミナルで `claude` を起動し（プラグインを外した設定ディレクトリで `CLAUDE_CONFIG_DIR=… claude`）、一覧を開くとすぐ Unknown タブに出ること。

- [ ] **Step 9: コミット**

```bash
git add crates/core/src/procinfo.rs crates/core/src/ps.rs crates/core/src/store.rs app/src-tauri/src/monitor.rs app/src-tauri/src/tray.rs
git commit -m "一覧を閉じている間は、全プロセスではなく知っているセッションの pid だけを確かめる

測った値: 直す前 <Step 1 の値> → 直した後 <Step 8 の値>（一覧を閉じて 5 分）

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

（`<…>` は測った値に書き換える。）

---

### Task 7: panic・Mutex・一時ファイル・ファイルの権限

**Files:**
- Create: `crates/core/src/crash_log.rs`
- Modify: `crates/core/src/lib.rs`
- Modify: `crates/core/src/paths.rs`
- Modify: `crates/core/src/persist.rs`
- Modify: `crates/core/src/spool.rs`
- Modify: `crates/hook/src/main.rs`（`save_raw`）
- Create: `app/src-tauri/src/sync.rs`
- Modify: `app/src-tauri/src/*.rs`（`.lock().unwrap()` を置き換え）
- Modify: `app/src-tauri/src/main.rs`、`app/src-tauri/src/monitor.rs`

**Interfaces:**
- Produces: `paths::write_private(path: &Path, bytes: &[u8]) -> io::Result<()>`（600 で作る）
- Produces: `paths::crash_log_path(home: &Path) -> PathBuf`
- Produces: `crash_log::append(path: &Path, now_ms: u64, message: &str) -> io::Result<()>`、`crash_log::MAX_BYTES: usize`
- Produces: `spool::STALE_TMP_AGE: Duration`
- Produces: アプリの `sync::LockExt`（`fn locked(&self) -> MutexGuard<'_, T>`）

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/crash_log.rs` を作る（テストだけ先に）。

```rust
use std::fs;
use std::io;
use std::path::Path;

use crate::paths::write_private;

pub const MAX_BYTES: usize = 64 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_lines_and_trims_the_oldest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crash.log");
        append(&path, 1, "first").unwrap();
        append(&path, 2, "second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "1 first\n2 second\n");
        let long = "x".repeat(MAX_BYTES / 3);
        for ts in 3..10 {
            append(&path, ts, &long).unwrap();
        }
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.len() <= MAX_BYTES);
        assert!(text.ends_with(&format!("9 {long}\n")));
        assert!(!text.contains("first"));
        assert!(text.starts_with(|c: char| c.is_ascii_digit()), "行の途中から始めない");
    }
}
```

`crates/core/src/lib.rs` に `pub mod crash_log;` を足す。

`crates/core/src/paths.rs` の末尾に足す。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn private_files_are_readable_only_by_the_owner() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.json");
        write_private(&path, b"{}").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        write_private(&path, b"[]").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"[]");
    }
}
```

`crates/core/src/spool.rs` の `mod tests` の `skips_broken_files_and_ignores_tmp` の後に足す。

```rust
    #[test]
    fn removes_tmp_files_left_behind_by_killed_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = dir.path().join("0000000000002-1-1.json.tmp");
        let stale = dir.path().join("0000000000001-1-1.json.tmp");
        fs::write(&fresh, "{}").unwrap();
        fs::write(&stale, "{}").unwrap();
        let old = SystemTime::now() - STALE_TMP_AGE - std::time::Duration::from_secs(1);
        fs::File::options().write(true).open(&stale).unwrap().set_modified(old).unwrap();
        drain_spool(dir.path(), MAX_SPOOL_FILES);
        assert!(fresh.exists(), "書いている途中かもしれないので消さない");
        assert!(!stale.exists());
    }
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core crash_log private_files removes_tmp`
Expected: コンパイルエラー（`append`・`write_private`・`STALE_TMP_AGE` がない）

- [ ] **Step 3: 実装する（core）**

`crates/core/src/paths.rs` に足す。

```rust
// フォルダーは 700 だが、中のファイルも本人だけにしておく（README の「本人だけが読み書きできる」に合わせる）
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

pub fn crash_log_path(home: &Path) -> PathBuf {
    home.join("crash.log")
}
```

`OpenOptions::mode` は新しく作るときだけ効く。`state.json` などは一時ファイルに書いて名前を変えるので、毎回新しく作られる。

`crates/core/src/crash_log.rs` に足す。

```rust
// panic のときにだけ書く。メニューバーのアプリの標準エラーはどこにも残らないので、ここに残して原因を追えるようにする
pub fn append(path: &Path, now_ms: u64, message: &str) -> io::Result<()> {
    let mut text = fs::read_to_string(path).unwrap_or_default();
    text.push_str(&format!("{now_ms} {message}\n"));
    if text.len() > MAX_BYTES {
        let cut = text.len() - MAX_BYTES;
        let start = text[cut..].find('\n').map_or(text.len(), |i| cut + i + 1);
        text.drain(..start);
    }
    write_private(path, text.as_bytes())
}
```

`crates/core/src/persist.rs` の `save_json` の `fs::write(&tmp, …)?;` を `crate::paths::write_private(&tmp, &serde_json::to_vec_pretty(value)?)?;` にする。

`crates/core/src/spool.rs`:
- `write_event` の `fs::write(&tmp_path, serde_json::to_vec(event)?)?;` を `crate::paths::write_private(&tmp_path, &serde_json::to_vec(event)?)?;` にする
- 定数を足す: `pub const STALE_TMP_AGE: std::time::Duration = std::time::Duration::from_secs(60);`（hook は 5 秒で打ち切られるので、1 分たった `.tmp` は書き終わることがない）
- `drain_spool` の最初、`paths` を集めるところで `.tmp` を片付ける。

```rust
    let now = SystemTime::now();
    let mut paths: Vec<PathBuf> = Vec::new();
    for path in entries.filter_map(|entry| entry.ok().map(|e| e.path())) {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => paths.push(path),
            // hook が書いている途中で打ち切られると、一時ファイルだけが残る
            Some("tmp") if is_stale(&path, now) => {
                let _ = fs::remove_file(&path);
            }
            _ => {}
        }
    }
    paths.sort();
```

```rust
fn is_stale(path: &Path, now: SystemTime) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .is_ok_and(|modified| now.duration_since(modified).is_ok_and(|age| age >= STALE_TMP_AGE))
}
```

`crates/hook/src/main.rs` の `save_raw` の `fs::write(…)` を `paths::write_private(&dir.join(format!("{ts_ms:013}-{}.json", std::process::id())), raw)` にする（`use std::fs;` が要らなくなれば消す）。

- [ ] **Step 4: 通ることを確かめる（core）**

Run: `cargo test --workspace`
Expected: PASS

- [ ] **Step 5: Mutex が壊れても使い続ける**

`app/src-tauri/src/sync.rs` を作る。

```rust
use std::sync::{Mutex, MutexGuard, PoisonError};

// ほかのスレッドが panic してロックが壊れても、中身はそのまま使える。unwrap で連鎖して落ちないようにする
pub trait LockExt<T> {
    fn locked(&self) -> MutexGuard<'_, T>;
}

impl<T> LockExt<T> for Mutex<T> {
    fn locked(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
```

`main.rs` に `mod sync;` を足す。置き換える。

```bash
sed -i '' 's/\.lock()\.unwrap()/.locked()/g' app/src-tauri/src/*.rs
grep -ln "\.locked()" app/src-tauri/src/*.rs
```

出てきた各ファイルの use に `use crate::sync::LockExt;` を足す。

- [ ] **Step 6: panic で監視を止めず、記録を残す**

`monitor.rs` の `spawn` のループを変える（`use std::panic::{catch_unwind, AssertUnwindSafe};`）。

```rust
        loop {
            // 1 回分の処理で panic しても、次の回からは動き続ける。ロックは LockExt で壊れても使える
            let changed = catch_unwind(AssertUnwindSafe(|| run_tick(&app, tick.is_multiple_of(PROCESS_CHECK_EVERY_TICKS)))).unwrap_or(false);
            if changed {
                publish(&app);
            }
            tick = tick.wrapping_add(1);
            thread::sleep(TICK);
        }
```

`main.rs` の `main` の最初（`let home = …` の後）に足す（`use agents_monitor_core::crash_log;`）。

```rust
    let crash_log = paths::crash_log_path(&home);
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crash_log::append(&crash_log, paths::now_ms(), &info.to_string());
        default_hook(info);
    }));
```

- [ ] **Step 7: 確かめる**

Run: `cargo test --workspace && cargo build -p agents-monitor-app && ! grep -rn "\.lock()\.unwrap()" app/src-tauri/src`
Expected: PASS・ビルドが通る・grep は何も出さない

- [ ] **Step 8: コミット**

```bash
git add crates/core/src/crash_log.rs crates/core/src/lib.rs crates/core/src/paths.rs crates/core/src/persist.rs crates/core/src/spool.rs crates/hook/src/main.rs app/src-tauri/src
git commit -m "panic で監視を止めずに crash.log に残し、残った一時ファイルを片付け、書くファイルを本人だけが読める権限にする

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: 文言の置き換えと画面の操作

**Files:**
- Modify: `crates/core/src/messages.rs`
- Modify: `crates/core/src/view.rs`（テスト）
- Modify: `app/src/i18n.ts`
- Modify: `app/src/settings.ts`
- Modify: `app/src/list.ts`

**Interfaces:**
- `messages::format(lang, msg, args)` の型は今のまま
- `t(key, args)` の型は今のまま

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/messages.rs` の `mod tests` に足す。

```rust
    #[test]
    fn substitutes_each_placeholder_once() {
        assert_eq!(fill("{dir}: {reason}", &[("dir", "/x/{reason}"), ("reason", "boom")]), "/x/{reason}: boom");
        assert_eq!(fill("{missing} {dir}", &[("dir", "d")]), "{missing} d");
        assert_eq!(fill("open { brace", &[]), "open { brace");
    }
```

`crates/core/src/view.rs` の `mod tests` に足す（文言は `locales/ja.json` のものに合わせる。違えば ja.json の文言に直す）。

```rust
    #[test]
    fn composes_every_detail_kind() {
        use crate::messages::Lang::Ja;
        use crate::state::DetailKind::*;
        let label = |kind, text: Option<&str>| detail_label(Ja, Some(kind), text);
        assert_eq!(detail_label(Ja, None, Some("x")), None);
        assert_eq!(label(Tool, Some("Bash: ls")).as_deref(), Some("Bash: ls"));
        assert_eq!(label(Tool, None), None);
        assert_eq!(label(Error, Some("rate_limit")).as_deref(), Some("rate_limit"));
        assert_eq!(label(Permission, Some("Bash: rm")).as_deref(), Some(&*format!("{} · Bash: rm", text(Ja, Msg::DetailPermission))));
        assert_eq!(label(NotMonitored, None).as_deref(), Some(&*format!("{} - {}", text(Ja, Msg::DetailNotMonitored), text(Ja, Msg::DetailTerminal))));
        assert_eq!(label(NotMonitored, Some("VS Code")).as_deref(), Some(&*format!("{} - VS Code", text(Ja, Msg::DetailNotMonitored))));
        for (kind, msg) in [
            (New, Msg::DetailNew),
            (Question, Msg::DetailQuestion),
            (PlanApproval, Msg::DetailPlanApproval),
            (Input, Msg::DetailInput),
            (Interrupted, Msg::DetailInterrupted),
            (Subagent, Msg::DetailSubagent),
            (Compacting, Msg::DetailCompacting),
            (Background, Msg::DetailBackground),
        ] {
            assert_eq!(label(kind, None), Some(text(Ja, msg)), "{kind:?}");
        }
    }
```

（`view.rs` の tests で `text`・`Msg` が use されていなければ、`use crate::messages::{text, Msg};` を足す。）

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core substitutes_each composes_every`
Expected: `substitutes_each_placeholder_once` がコンパイルエラー（`fill` がない）。`composes_every_detail_kind` は PASS してよい（足りなかったテストを足すもの）

- [ ] **Step 3: 実装する（Rust）**

`crates/core/src/messages.rs` の `format` を置き換える。

```rust
pub fn format(lang: Lang, msg: Msg, args: &[(&str, &str)]) -> String {
    fill(&text(lang, msg), args)
}

// 順に replace すると、先に入れた値（パスやエラーの文）の中の {reason} なども置き換えてしまうので、文言を 1 回だけ読む
fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let value = after.find('}').and_then(|close| {
            let name = &after[..close];
            args.iter().find(|(n, _)| *n == name).map(|(_, value)| (*value, close))
        });
        match value {
            Some((value, close)) => {
                out.push_str(value);
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}
```

Run: `cargo test -p agents-monitor-core`
Expected: PASS

- [ ] **Step 4: 実装する（画面の文言）**

`app/src/i18n.ts`:

```ts
export async function loadMessages(): Promise<void> {
  try {
    messages = await invoke<Record<string, string>>("get_messages");
  } catch (error) {
    // 対応表がなくてもキーの文字列で表示して動き続ける。一覧が開かないよりはよい
    console.error("failed to load messages", error);
  }
  document.documentElement.lang = messages["meta.lang"] ?? "en";
}

export function t(key: string, args: Record<string, string | number> = {}): string {
  const template = messages[key] ?? key;
  // 1 回で置き換え、入れた値（パスなど）の中の {…} は置き換えない
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => (name in args ? String(args[name]) : whole));
}
```

- [ ] **Step 5: 設定を順番に保存する**

`app/src/settings.ts` の `save` と、チェックの `change` を置き換える。押したときの最新の値から作り、保存は 1 つずつ順に行う。

```ts
let saving: Promise<void> = Promise.resolve();

// 描いたときの値から作ると、保存が終わる前に続けて押した変更が古い値で上書きされる。押した時点の最新の値から作り、順に保存する
function update(change: (prefs: Preferences) => Preferences): void {
  if (!current) return;
  const next = change(current);
  current = next;
  render();
  saving = saving.then(() => persist(next));
}

async function persist(next: Preferences): Promise<void> {
  try {
    await invoke("save_settings", { settings: next });
    showError(null);
  } catch (error) {
    showError(String(error));
    // 保存できなかったので、画面を実際に保存されている値に戻す
    current = await invoke<Preferences>("get_settings");
    render();
  }
}
```

`render` の中の `change` の処理:

```ts
      input.addEventListener("change", () => {
        update((prefs) => ({
          ...prefs,
          notify_on: input.checked ? [...prefs.notify_on, status] : prefs.notify_on.filter((s) => s !== status),
        }));
      });
```

`render` の `const settings = current;` は `input.checked` と `launchEl.checked` を決めるためにだけ残す。`setupSettingsMenu` の `launchEl` の処理:

```ts
  launchEl.addEventListener("change", () => update((prefs) => ({ ...prefs, launch_at_login: launchEl.checked })));
```

古い `save` 関数は消す。

- [ ] **Step 6: 開いた詳細の集まりを整理する**

`app/src/list.ts` の `sessions-updated` の受け取りを変える。

```ts
  void listen<SessionView[]>("sessions-updated", (event) => {
    sessions = event.payload;
    const ids = new Set(sessions.map((session) => session.id));
    for (const id of expanded) if (!ids.has(id)) expanded.delete(id);
    render();
  });
```

- [ ] **Step 7: 確かめる**

Run: `cd app && npx tsc --noEmit && cd .. && cargo test --workspace`
Expected: 型エラーなし・PASS

`npm run tauri dev` で確かめる: 設定で「完了」と「エラー停止」のチェックをすばやく続けて外す→一覧を閉じて開き直し、両方とも外れたままであること。`~/.agents-monitor/settings.json` の `notify_on` にも反映されていること。

- [ ] **Step 8: コミット**

```bash
git add crates/core/src/messages.rs crates/core/src/view.rs app/src/i18n.ts app/src/settings.ts app/src/list.ts
git commit -m "文言の置き換えを一度で行い、設定の保存を順番にし、もうないセッションの開いた詳細を忘れる

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: 設定ディレクトリの操作の取りこぼしを直す

**Files:**
- Modify: `crates/core/src/claude_cli.rs`
- Modify: `app/src-tauri/src/installer.rs`
- Modify: `crates/core/locales/ja.json`, `crates/core/locales/en.json`, `crates/core/src/messages.rs`

**Interfaces:**
- `find_claude` の型は今のまま（保存した場所が実行できなければ自動で探す）
- Produces: `Msg::InfoRemovedWithoutClaude`（キー `info.removed_without_claude`、置き換え `{dir}`）

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/claude_cli.rs` の tests で、保存した場所が実行できないときに `None` を期待しているテストを探し、次の期待に直す（なければ足す）。

```rust
    #[test]
    fn stale_configured_path_falls_back_to_auto_detection() {
        let home = Path::new("/Users/u");
        let found = find_claude(Some("/old/claude"), home, |p| p == Path::new("/opt/homebrew/bin/claude"));
        assert_eq!(found, Some(PathBuf::from("/opt/homebrew/bin/claude")));
        let configured = find_claude(Some("/my/claude"), home, |p| p == Path::new("/my/claude") || p == Path::new("/opt/homebrew/bin/claude"));
        assert_eq!(configured, Some(PathBuf::from("/my/claude")), "使える場所が保存されていればそれを使う");
    }
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core claude_cli`
Expected: `stale_configured_path_falls_back_to_auto_detection` が FAIL

- [ ] **Step 3: 実装する**

`find_claude` の最初を置き換える。

```rust
    // 保存した場所は、claude を入れ直すと古くなる。使えなければ、よくある場所から探し直す
    if let Some(path) = configured.map(PathBuf::from).filter(|path| is_executable(path)) {
        return Some(path);
    }
```

Run: `cargo test -p agents-monitor-core`
Expected: PASS

- [ ] **Step 4: 再読み込みをまとめ、claude がなくても外せるようにする**

`installer.rs`:

`InstallerState` に `refresh_queued: std::sync::atomic::AtomicBool,` を足す。

```rust
pub fn send(app: &AppHandle, job: Job) {
    let installer = app.state::<InstallerState>();
    // 設定画面を開くたびに積まれるが、まだ始まっていない再読み込みが 1 つあれば足りる
    if matches!(job, Job::Refresh) && installer.refresh_queued.swap(true, Ordering::Relaxed) {
        return;
    }
    if let Some(sender) = installer.sender.locked().as_ref() {
        let _ = sender.send(job);
    }
}
```

`run` の `Job::Refresh => refresh_all(&app),` を次にする。

```rust
            Job::Refresh => {
                app.state::<InstallerState>().refresh_queued.store(false, Ordering::Relaxed);
                refresh_all(&app);
            }
```

`remove` の最初を次にする。claude が見つからないときは、一覧からは外し、プラグインが残ることを伝える。

```rust
fn remove(app: &AppHandle, path: &str) {
    if Path::new(path).is_dir() {
        match apply(app, path, remove_steps) {
            Ok(()) => {}
            Err(InstallError::ClaudeNotFound) => {
                let lang = app.state::<AppState>().lang;
                alert(app, &messages::format(lang, Msg::InfoRemovedWithoutClaude, &[("dir", &display(path))]));
            }
            Err(reason) => {
                alert(app, &failure_message(app, Msg::ErrorRemoveFailed, path, &reason));
                return;
            }
        }
    }
```

（以降の `update_settings` と `inspections` の削除は今のまま。）

`crates/core/locales/ja.json` に足す。

```json
  "info.removed_without_claude": "{dir} を一覧から外しました。claude が見つからないため、プラグインはその設定ディレクトリに残っています",
```

`crates/core/locales/en.json` に足す。

```json
  "info.removed_without_claude": "Removed {dir} from the list. The plugin is still installed there because claude was not found",
```

`crates/core/src/messages.rs` の `Msg` の定義（マクロ）に `InfoRemovedWithoutClaude => "info.removed_without_claude"` を、既存の項目と同じ書き方で足す。

`settings.ts` の削除ボタンは、claude がないときも押せることを確かめる（`dir.busy || checking` だけで無効にしていて、claude の有無では無効にしていないこと）。claude がないとき、状態が `checking` のまま押せなくなるなら、`installer::view` で claude がない（`startup_done` かつ `claude` が `None`）ときの状態を `DirState::NotInstalled` にする。

```rust
            let state = if !Path::new(&path).is_dir() {
                DirState::Missing
            } else {
                match inspections.get(&path) {
                    Some(Ok(inspection)) => inspection.state(),
                    Some(Err(_)) => DirState::NotInstalled,
                    // claude がなければ調べられないので、いつまでも「確かめています」にしない
                    None if started && installer.claude.locked().is_none() => DirState::NotInstalled,
                    None => DirState::Checking,
                }
            };
```

- [ ] **Step 5: 確かめる**

Run: `cargo test --workspace && cargo build -p agents-monitor-app`
Expected: PASS（`messages_used`・日英のキーがそろっているかのテストも）

- [ ] **Step 6: コミット**

```bash
git add crates/core/src/claude_cli.rs crates/core/locales crates/core/src/messages.rs app/src-tauri/src/installer.rs
git commit -m "古い claude の場所では探し直し、再読み込みをまとめ、claude がなくても設定ディレクトリを外せるようにする

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: 移動の確かさ

**Files:**
- Modify: `crates/core/src/jump.rs`
- Modify: `app/src-tauri/src/jumper.rs`

**Interfaces:**
- `jump_target(session: &Session, rows: &[ProcRow]) -> JumpTarget` の型は今のまま（claude の行でなければ `None`）

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/jump.rs` の `mod tests` に足す（`session(id, pid, entrypoint, tmux_pane)` は今のテストの関数）。

```rust
    #[test]
    fn reused_pid_is_not_followed() {
        let rows = vec![row(1, 0, "/sbin/launchd"), row(100, 1, "/Applications/Visual Studio Code.app/Contents/MacOS/Electron"), row(300, 100, "/usr/bin/python3 server.py")];
        assert_eq!(jump_target(&session("s1", 300, Some("claude-vscode"), None), &rows), JumpTarget::None);
    }

    #[test]
    fn app_path_inside_an_argument_is_not_taken() {
        assert_eq!(app_in_args("/usr/local/bin/tool --open /Users/u/My.app/data"), None);
        assert_eq!(app_in_args("/Applications/iTerm.app/Contents/MacOS/iTerm2"), Some(PathBuf::from("/Applications/iTerm.app")));
    }

    #[test]
    fn windsurf_opens_the_project_window() {
        let rows = vec![
            row(1, 0, "/sbin/launchd"),
            row(100, 1, "/Applications/Windsurf.app/Contents/MacOS/Electron"),
            row(200, 100, "/Applications/Windsurf.app/Contents/Frameworks/Windsurf Helper (Plugin).app/Contents/MacOS/Windsurf Helper (Plugin) --type=utility"),
            row(300, 200, "/Users/u/.windsurf/extensions/anthropic.claude-code-2.1.296-darwin-arm64/resources/native-binary/claude --output-format stream-json"),
        ];
        let mut s = session("s1", 300, Some("claude-vscode"), None);
        s.start_cwd = "/w/api".into();
        assert_eq!(jump_target(&s, &rows), JumpTarget::App { app: PathBuf::from("/Applications/Windsurf.app"), folder: Some("/w/api".into()) });
    }

    #[test]
    fn iterm2_brings_the_app_front() {
        let rows = vec![
            row(1, 0, "/sbin/launchd"),
            row(100, 1, "/Applications/iTerm.app/Contents/MacOS/iTerm2"),
            row(200, 100, "/usr/bin/login -fpl u /Applications/iTerm.app/Contents/MacOS/ShellLauncher --launch_shell"),
            row(300, 200, "-zsh"),
            row(400, 300, "/Users/u/.local/bin/claude"),
        ];
        assert_eq!(jump_target(&session("s1", 400, Some("cli"), None), &rows), JumpTarget::App { app: PathBuf::from("/Applications/iTerm.app"), folder: None });
    }
```

（`session` が `start_cwd` を作らない形なら、今のテストの作り方に合わせる。`row(200, …)` の `login` の行は先頭が `/usr/bin/login` なので、`.app/Contents/` を含んでいても引数の中なので取らない。ただしこのテストは外側の `iTerm2` の行で `.app` が取れることを確かめる。）

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core jump`
Expected: `reused_pid_is_not_followed` と `app_path_inside_an_argument_is_not_taken` が FAIL。`iterm2_brings_the_app_front` は、`login` の行の引数の `.app/` を取ってしまうので FAIL するか、たまたま同じパスで PASS する（どちらでもよい）

- [ ] **Step 3: 実装する**

`app_in_args` を変える。

```rust
// ps の args は実行ファイルのパスの空白を区切らずに持つので、空白で分けずに最初の ".app/Contents/" までを取る。
// 実行ファイルは必ず .app/Contents/ の下にあり、引数の中の ".app/" を取り違えにくい。
// 最初に現れる ".app" がいちばん外側（VS Code の拡張ホストは .app の中の .app で動く）
fn app_in_args(args: &str) -> Option<PathBuf> {
    if !args.starts_with('/') {
        return None;
    }
    let end = args.find(".app/Contents/")? + ".app".len();
    let app = &args[..end];
    // 空白の後ろから始まるパスは、実行ファイルではなく引数
    if app.contains(" /") || app.contains(" -") {
        return None;
    }
    Some(PathBuf::from(app))
}
```

`jump_target` の pid の行を確かめる（`use crate::ps::is_claude_command;`）。

```rust
pub fn jump_target(session: &Session, rows: &[ProcRow]) -> JumpTarget {
    let terminal = session.terminal.as_ref();
    if terminal.is_some_and(|t| t.tmux_pane.is_some()) {
        return JumpTarget::None;
    }
    let Some(pid) = session.claude_pid else { return JumpTarget::None };
    // 3 秒ごとの確認の間に pid が別のプロセスに使い回されると、関係のないアプリを前に出してしまう
    if !rows.iter().find(|r| r.pid == pid).is_some_and(|r| is_claude_command(&r.args)) {
        return JumpTarget::None;
    }
    let Some(app) = outermost_app(rows, pid) else { return JumpTarget::None };
    let in_editor = terminal.and_then(|t| t.entrypoint.as_deref()) == Some(EDITOR_ENTRYPOINT);
    let folder = [&session.start_cwd, &session.cwd].into_iter().find(|dir| dir.starts_with('/')).cloned();
    JumpTarget::App { app, folder: if in_editor { folder } else { None } }
}
```

`/Applications/Visual Studio Code.app/…` は `.app` の前の空白が ` C`（`Visual Studio Code`）なので、` /` と ` -` の確認には当たらない。

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test -p agents-monitor-core jump`
Expected: PASS（前からある VS Code・Terminal・tmux のテストも）

- [ ] **Step 5: 移動の途中は受け付けない**

`app/src-tauri/src/jumper.rs` に足す（`use std::sync::atomic::{AtomicBool, Ordering};`）。

```rust
// 行や通知を続けて押すと open が重ねて動き、ウィンドウが行き来する
static JUMPING: AtomicBool = AtomicBool::new(false);

pub fn jump(app: &AppHandle, session_id: &str) -> bool {
    if JUMPING.swap(true, Ordering::Relaxed) {
        // 前の移動がまだ終わっていない。詳細を開かせないよう、移動したものとして扱う
        return true;
    }
    let jumped = jump_once(app, session_id);
    JUMPING.store(false, Ordering::Relaxed);
    jumped
}
```

今の `pub fn jump` は `fn jump_once` に名前を変える。

- [ ] **Step 6: 確かめる**

Run: `cargo test --workspace && cargo build -p agents-monitor-app`
Expected: PASS・ビルドが通る

- [ ] **Step 7: コミット**

```bash
git add crates/core/src/jump.rs app/src-tauri/src/jumper.rs
git commit -m "移動の前に pid がまだ claude か確かめ、引数の中の .app を取り違えず、移動の途中の押下を受け付けない

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: 版・Info.plist・CSP・使っていないコード

**Files:**
- Create: `app/src-tauri/tests/versions.rs`
- Modify: `app/src-tauri/tauri.conf.json`, `app/src-tauri/Cargo.toml`, `crates/core/Cargo.toml`, `crates/hook/Cargo.toml`, `app/package.json`, `plugin/.claude-plugin/plugin.json`
- Modify: `app/src-tauri/Info.plist`
- Modify: `app/src-tauri/src/main.rs`, `app/src-tauri/src/tray.rs`, `app/src-tauri/src/installer.rs`

- [ ] **Step 1: 失敗するテストを書く**

`app/src-tauri/tests/versions.rs` を作る。

```rust
use std::fs;
use std::path::Path;

fn json_version(path: &Path) -> String {
    let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    value["version"].as_str().unwrap_or_else(|| panic!("{} に version がない", path.display())).to_string()
}

fn cargo_version(path: &Path) -> String {
    let text = fs::read_to_string(path).unwrap();
    let line = text.lines().find(|line| line.starts_with("version = ")).unwrap_or_else(|| panic!("{} に version がない", path.display()));
    line.trim_start_matches("version = ").trim_matches('"').to_string()
}

// プラグインの版が変わらないと、入れてあるプラグインは更新されず、hooks.json の変更が利用者に届かない
#[test]
fn every_version_matches_the_app() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = app.join("../..");
    let expected = env!("CARGO_PKG_VERSION");
    let versions = [
        ("tauri.conf.json", json_version(&app.join("tauri.conf.json"))),
        ("app/package.json", json_version(&repo.join("app/package.json"))),
        ("plugin.json", json_version(&repo.join("plugin/.claude-plugin/plugin.json"))),
        ("crates/core/Cargo.toml", cargo_version(&repo.join("crates/core/Cargo.toml"))),
        ("crates/hook/Cargo.toml", cargo_version(&repo.join("crates/hook/Cargo.toml"))),
    ];
    for (file, version) in versions {
        assert_eq!(version, expected, "{file} の版をアプリ（{expected}）に合わせること");
    }
}
```

- [ ] **Step 2: 通ることを確かめてから版を上げる**

Run: `cargo test -p agents-monitor-app --test versions`
Expected: PASS（今はすべて 0.1.0）

7 つのファイルの `0.1.0` を `0.4.0` にする: `app/src-tauri/tauri.conf.json`、`app/src-tauri/Cargo.toml`、`crates/core/Cargo.toml`、`crates/hook/Cargo.toml`、`app/package.json`、`plugin/.claude-plugin/plugin.json`。`Cargo.lock` と `app/package-lock.json` は `cargo build` と `npm install` で更新する。

```bash
cargo build --workspace && (cd app && npm install)
```

`app/src-tauri/Cargo.toml` だけ上げ忘れていないかを、いったん 1 つを 0.1.0 に戻してテストが FAIL することで確かめ、また 0.4.0 にする。

Run: `cargo test -p agents-monitor-app --test versions`
Expected: PASS

- [ ] **Step 3: Info.plist・CSP・使っていないコード**

`app/src-tauri/Info.plist` の `<dict>` の中に足す。

```xml
  <key>LSUIElement</key>
  <true/>
```

`tauri.conf.json` の `"security": { "csp": null }` を次にする。

```json
    "security": {
      "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src ipc: http://ipc.localhost"
    }
```

`app/src-tauri/src/main.rs` の 1 行目 `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` を消す。

`app/src-tauri/src/tray.rs` の `show_settings` を消し、`installer.rs` の `crate::tray::show_settings(app);` を `crate::tray::show_list(app);` にする。

- [ ] **Step 4: 確かめる**

Run: `cargo test --workspace && cd app && npx tsc --noEmit && npm run tauri dev`
Expected: PASS。dev の画面で、一覧・タブ・設定・アイコン（innerHTML の SVG）が表示され、Web インスペクター（右クリック → 要素の詳細を表示）のコンソールに CSP の違反が出ないこと。違反が出たら、その種類（`style-src` など）だけを足し、足した理由を報告に書く

`./scripts/build-hook-sidecar.sh && npm run tauri build` で作り、`open target/release/bundle/macos/agents-monitor.app` で起動の瞬間に Dock にアイコンが出ないこと、同じく CSP の違反がないことを確かめる。

- [ ] **Step 5: コミット**

```bash
git add app/src-tauri/tests/versions.rs app/src-tauri/tauri.conf.json app/src-tauri/Cargo.toml crates/core/Cargo.toml crates/hook/Cargo.toml app/package.json app/package-lock.json Cargo.lock plugin/.claude-plugin/plugin.json app/src-tauri/Info.plist app/src-tauri/src/main.rs app/src-tauri/src/tray.rs app/src-tauri/src/installer.rs
git commit -m "版を 0.4.0 にそろえて確かめるテストを足し、Dock のアイコンを出さず、CSP を入れ、使っていないコードを消す

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Figma の新しいアイコンを入れる

**Files:**
- Modify: `app/src-tauri/icons/tray/{default,error,play,required}.svg` と `.png`
- Modify: `scripts/make-tray-icons.sh`
- Modify: `app/src-tauri/icons/*`（アプリのアイコン）
- Delete: `scripts/make-app-icon.mjs`

**前提:** アプリのアイコンは、Figma の AppIcon（`139:1982`、1024 × 1024 の正方形）をそのまま使う。角丸の形は描き込まない（macOS 26 が角丸に収めて表示する。実機で見比べて正方形のままの方を選んだ）。

- [ ] **Step 1: メニューバーのアイコンを書き出す**

Figma MCP の `download_assets`（fileKey は非公開、`defaultFormat: "svg"`）で、次の node の `export` の SVG をそれぞれ取る。

| node | 保存先 |
|---|---|
| `87:3316`（State=Default） | `app/src-tauri/icons/tray/default.svg` |
| `87:3318`（State=Error） | `app/src-tauri/icons/tray/error.svg` |
| `87:3352`（State=Play） | `app/src-tauri/icons/tray/play.svg` |
| `87:3386`（State=Required） | `app/src-tauri/icons/tray/required.svg` |

書き出した SVG には、Figma のページの表示用の背景（`<rect width="18" height="18" fill="#BBBBBB"/>`）と、コンポーネントの枠の点線（`stroke-dasharray` のある `rect`）が入っているので消す。

```bash
for name in default error play required; do
  f=app/src-tauri/icons/tray/$name.svg
  sed -i '' -e '/fill="#BBBBBB"/d' -e '/stroke-dasharray/d' "$f"
  ! grep -q -e BBBBBB -e stroke-dasharray "$f"
done
```

- [ ] **Step 2: 書き出しのスクリプトを今の SVG に合わせる**

今のスクリプトは「状態つきの SVG は枠より幅が狭く、左上に寄っている」前提で 18×18 の枠に入れ直している。新しい SVG は 18×18 の枠ごと書き出されているので、入れ直しは要らない。`scripts/make-tray-icons.sh` の for の中を次にする。

```bash
for name in default error play required; do
  # Figma の 0.85 はメニューバーでの見え方を再現するためのもの。テンプレート画像の濃さは OS が決めるので外す
  plain=$(mktemp)
  sed -e 's/ fill-opacity="[^"]*"//g' "$dir/$name.svg" > "$plain"
  rsvg-convert -w 36 -h 36 "$plain" -o "$dir/$name.png"
  rm "$plain"
done
```

- [ ] **Step 3: PNG にしてテストする**

Run: `./scripts/make-tray-icons.sh && cargo test -p agents-monitor-app tray_icon`
Expected: PASS（36px 四方・形が不透明・状態ごとに違う）

`open app/src-tauri/icons/tray/required.png` で、顔と記号が見えることを目で確かめる。

- [ ] **Step 4: コミット**

```bash
git add app/src-tauri/icons/tray scripts/make-tray-icons.sh
git commit -m "メニューバーのアイコンを Figma の新しいもの（顔つき）にする

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 5: アプリのアイコンを書き出す**

`download_assets`（node `139:1982`、`defaultFormat: "png"`、`defaultScale: 1`。node は 1024 × 1024）で PNG を取り、`app/src-tauri/icons/app-icon-source.png` に置く。

Run: `sips -g pixelWidth -g pixelHeight -g hasAlpha app/src-tauri/icons/app-icon-source.png`
Expected: 1024 × 1024

- [ ] **Step 6: 各サイズを作る**

```bash
cd app && npx tauri icon src-tauri/icons/app-icon-source.png && cd ..
git status --short app/src-tauri/icons
```

`tauri.conf.json` の `bundle.icon` にあるもの（`32x32.png`・`128x128.png`・`128x128@2x.png`・`icon.icns`・`icon.png`）と、前からある `64x64.png`・`icon.ico` は残す。新しく増えたもの（`Square*.png`・`StoreLogo.png`・`android/`・`ios/` など）は消す。`app-icon-source.png` は元の絵としてコミットする。

`scripts/make-app-icon.mjs` を消す（仮のアイコンを作るためのもの）。README などから参照されていないことを確かめる。

Run: `git rm scripts/make-app-icon.mjs && ! grep -rn make-app-icon --exclude-dir=node_modules --exclude-dir=target . | grep -v docs/superpowers/plans`

- [ ] **Step 7: 確かめる**

`./scripts/build-hook-sidecar.sh && cd app && npm run tauri build`、`open ../target/release/bundle/macos/`。Finder と、通知（新しいセッションを始めて「完了（新規）」の通知を出す）で新しいアイコンが出ること。Finder がアイコンを覚えていて古いままなら `touch ../target/release/bundle/macos/agents-monitor.app`。

- [ ] **Step 8: コミット**

```bash
git add app/src-tauri/icons
git commit -m "アプリのアイコンを Figma の AppIcon にする

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

リポジトリの直下の `icon.ai` は `git add` しない。

---

### Task 13: 一覧を閉じたら画面を捨てる（試して、測ってから決める）

**Files:**
- Modify: `app/src-tauri/src/tray.rs`, `app/src-tauri/src/main.rs`, `app/src/list.ts`

設計書 4.5。まず最小の形で試し、メモリーと開くまでの時間を測る。結果をユーザーに報告し、採るかどうかを決めてもらう。採らないときは変更を捨てる。

- [ ] **Step 1: 直す前を測る**

Task 12 までを入れたアプリを `npm run tauri build` で作って起動し、一覧を 1 回開いて閉じた状態で測る。

```bash
APP=$(pgrep -f "agents-monitor.app/Contents/MacOS/agents-monitor-app")
footprint $APP | grep Footprint
for p in $(pgrep -f "com.apple.WebKit"); do ps -o ppid=,pid=,rss= -p $p; done
```

WebKit のプロセスは、親の pid が launchd になるので、`footprint -p <pid>` を `pgrep -f WebKit.WebContent` などの中から、アプリを終了すると消えるものを選んで足し合わせる（アプリを終了して消えた pid が、このアプリのもの）。合計をメモする。

開くまでの時間: メニューバーのアイコンを押してから一覧が出るまでを、画面収録（⌘⇧5）で 10 回撮り、コマ送りで数える（今はほぼ瞬時のはず）。

- [ ] **Step 2: 最小の形で試す**

`tray.rs`:
- 一覧を閉じる 3 か所（フォーカスを失ったとき・`toggle_list`・`hide_list`）で、`window.hide()` の代わりに `window.destroy()` を呼ぶ
- `show_list` で、ウィンドウがなければ設定から作り直す。

```rust
fn list_window(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window("list") {
        return Some(window);
    }
    let config = app.config().app.windows.iter().find(|w| w.label == "list")?.clone();
    let window = tauri::WebviewWindowBuilder::from_config(app, &config).ok()?.build().ok()?;
    watch_focus(app, &window);
    Some(window)
}
```

`setup_windows` のフォーカスの処理は `watch_focus(app, window)` として切り出し、作り直したウィンドウにも付ける。

`main.rs`: 最後のウィンドウを閉じてもアプリが終わらないようにする。

```rust
        .build(tauri::generate_context!())
        .expect("failed to build agents-monitor")
        .run(|_app, event| {
            // 一覧を閉じるとウィンドウがなくなるが、メニューバーのアプリとして動き続ける
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
```

（`quit_app` の `app.exit(0)` は `code: Some(0)` なので止めない。）

`list.ts`: 作り直した画面は、フォーカスの知らせより先に読み込みが終わるとは限らないので、`start()` の最後で今フォーカスがあるかを確かめて描く。

```ts
  if (await getCurrentWindow().isFocused()) {
    listVisible = true;
    ticker ??= window.setInterval(render, 1000);
    render();
  }
```

- [ ] **Step 3: 直した後を測る**

Step 1 と同じ手順で、一覧を閉じた状態のメモリーと、開くまでの時間を測る。WebContent のプロセスが閉じたときに実際に終わっているかも見る（`pgrep -f WebKit.WebContent` の数が減るか）。

- [ ] **Step 4: ユーザーに報告して決める**

次の表で報告し、採るかどうかを確かめる（設計書 4.5。開くのが目に見えて遅くなるなら、採るかはユーザーが決める）。

| | 直す前 | 直した後 |
|---|---|---|
| 閉じているときのメモリー（アプリ + WebKit） | | |
| アイコンを押してから一覧が出るまで | | |
| 閉じたときに WebContent が終わるか | | |

- 採らない: `git checkout -- app/src-tauri/src/tray.rs app/src-tauri/src/main.rs app/src/list.ts` で捨て、この Task を終える
- 採る: Step 5 へ

- [ ] **Step 5: 採るときの仕上げ**

作り直した画面では、前の画面の状態（開いた詳細・見ていたタブ・「次のフォーカスでタブを戻さない」）がなくなる。次を確かめ、困るものだけ直す。
- 通知を押して移動できなかったとき、一覧が開いて詳細が出ること（`show-details` を、画面の読み込みの前に送ると取りこぼす。取りこぼすなら、`AppState` に「開いたら出す詳細」を持たせ、`get_sessions` と一緒に返して画面がそれを開く）
- 設定ディレクトリを選ぶダイアログ・アラートを閉じた後、設定の画面のままであること（`suppress_hide` の間は `destroy` しない）
- 移動に失敗して一覧を開き直したとき、詳細が開くこと

Run: `cargo test --workspace && cd app && npx tsc --noEmit`

```bash
git add app/src-tauri/src/tray.rs app/src-tauri/src/main.rs app/src/list.ts
git commit -m "一覧を閉じたら画面を捨て、開くときに作り直してメモリーを減らす

測った値: 閉じているときのメモリー <前> → <後>、開くまで <前> → <後>

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 14: 文書

**Files:**
- Modify: `README.md`, `README.en.md`
- Modify: `docs/superpowers/specs/2026-10-09-agents-monitor-design.md`, `2026-10-11-agents-monitor-v0.2-design.md`, `2026-10-11-agents-monitor-v0.2.1-i18n-design.md`, `2026-10-11-agents-monitor-v0.3-design.md`, `2026-10-11-agents-monitor-v0.4-design.md`

- [ ] **Step 1: README.md を書き直す**

今の節（状態の見方・画面移動・導入・既知の制約・保存するデータ・開発・ライセンス）は残し、次を足す・直す。文言は今の README の書き方（です・ます、短い文）に合わせる。

1. 冒頭の説明の後に「## 必要な環境」
   - macOS（macOS 26 で確かめています）
   - Claude Code（`claude` コマンド。プラグインを入れるのに使います）。アプリは `~/.local/bin`・`/opt/homebrew/bin`・`/usr/local/bin`・`~/.npm-global/bin`・`~/.claude/local` を探し、見つからなければ設定の画面で場所を入れます
2. 「## 導入」の前に「## 入手」
   - 今はソースから作ります。手順: `./scripts/build-hook-sidecar.sh` → `cd app && npm install && npm run tauri build` → `target/release/bundle/macos/agents-monitor.app` を「アプリケーション」に入れる
   - 配布用の版（ダウンロード）は準備中です
3. 「## 導入」
   - 初めて起動すると、macOS が通知の許可を尋ねます。許可すると、対応が必要になったときなどに通知が届きます
   - 「≡ →「Claude Code の設定」→ …」を「一覧の ≡ を押すと設定の画面が開きます。「Claude Code の設定」の欄の「＋ 設定ディレクトリを追加」で選んでください」に直す
4. 「## 設定」（新しい節）: 通知する状態（要対応・エラー停止・完了。はじめは要対応とエラー停止）、ログイン時に起動、Claude Code の設定（設定ディレクトリの追加・再試行・削除、`claude` が見つからないときの場所）、終了
5. 「## 消し方」（新しい節）
   1. 設定の画面で、すべての設定ディレクトリを削除のアイコンで外す（プラグインが外れます）
   2. 「ログイン時に起動」を外し、「終了」で終わる
   3. アプリをゴミ箱に入れる
   4. `~/.agents-monitor` を消す
   - アプリを先に消した場合は、各設定ディレクトリで `claude plugin uninstall agents-monitor@agents-monitor` と `claude plugin marketplace remove agents-monitor` を実行してください（`CLAUDE_CONFIG_DIR` を分けているときは、それぞれに付けて実行）
6. 「## 保存するデータ」に足す: panic の記録（`crash.log`、64KB まで）。開発用の `AGENTS_MONITOR_DEBUG_RAW` を付けて動かすと、hook が受け取った入力の全文を `~/.agents-monitor/raw` に保存します（調べもの用。使い終わったら消してください）
7. 「## 開発」: `npm run tauri dev` の前に `./scripts/build-hook-sidecar.sh` が要ること（アプリに同梱する hook がないと作れない）。`tauri dev` では通知が出ないこと
8. 「設計:」の行を `docs/superpowers/specs/` にする

- [ ] **Step 2: README.en.md を同じ構成で書き直す**

Step 1 と同じ節・同じ内容を英語で。英語の README は今の README.en.md の言い方に合わせる。両方を並べて、節の数と中身がそろっていることを確かめる。

Run: `diff <(grep '^#' README.md | wc -l) <(grep '^#' README.en.md | wc -l)`
Expected: 差がない

- [ ] **Step 3: 設計書を直す**

- 4 つの設計書（2026-10-09・v0.2・v0.2.1・v0.3）の `- 状態: レビュー待ち` を `- 状態: 実装済み` にする。v0.4 は `- 状態: 実装済み（このブランチ）` にする
- v0.3: 2.1 節で取りやめた URI の記述を、今の作り（ウィンドウを前に出すだけ）に書き直す
  - 1 章の成功の基準の 1 行目は今のままでよい（すでに「会話のタブは選ばない」）
  - 3 章の表の `jump` の行、3.1 節の「URI に入れるセッション ID は…」「`Info.plist` から読むのは URL スキームだけ」の 2 つは消す
  - 4 章の「フォルダーは開けたが、URI を開けなかった」「`Info.plist` が読めない・スキームがない」の 2 行は消す
  - 5 章の「`plutil` の JSON から…」「URI の組み立て…」の 2 つは消し、Windsurf の確認は「そのプロジェクトのウィンドウが前に出る」にする
  - 6 章の Windsurf の URI ハンドラーの項目は消す
  - 7 章の「7.3 節: VS Code 系はセッション ID の URI で会話まで」を「7.3 節: VS Code 系はプロジェクトのウィンドウ、ターミナルはアプリを前面に出すだけ」にする
  - 3.2 節の「一覧ウィンドウを閉じずに、その行の詳細を開く」を「一覧ウィンドウを閉じてから移動し、失敗したら一覧を開き直して、その行の詳細を開く」にする（4 章と合わせる）
  - 2 章の頭の「（1〜5 行目は設計の時点のもの…）」の注記は残す（調べた記録として）
- v0.2: 3.2 節の更新の手順を `claude plugin update agents-monitor@agents-monitor` だけにする（`marketplace update` は要らないことを notes に記録済み、と添える）
- 元の設計書: 4.2 節の hook の表に `SubagentStop` を足す（`hooks.json` と同じ）。14 章の各段階に「（完了）」を付け、v0.4 の行（公開前の仕上げ、`2026-10-11-agents-monitor-v0.4-design.md`）を足す。「実装計画は v0.1 から立てる」の行は消す

- [ ] **Step 4: 確かめる**

Run: `grep -rn "レビュー待ち" docs/superpowers/specs; grep -n "URI" docs/superpowers/specs/2026-10-11-agents-monitor-v0.3-design.md`
Expected: 「レビュー待ち」はない。URI は 2 章・2.1 節（調べた記録と取りやめた理由）だけ

- [ ] **Step 5: コミット**

```bash
git add README.md README.en.md docs/superpowers/specs
git commit -m "README に入手・環境・設定・消し方を足し、設計書を実装に合わせる

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 15: 実機で確かめて仕上げる

- [ ] **Step 1: すべてのテストとビルド**

Run: `cargo test --workspace && cd app && npx tsc --noEmit && cd .. && ./scripts/build-hook-sidecar.sh && cd app && npm run tauri build`
Expected: PASS・ビルドが通る

- [ ] **Step 2: 前の版から上げる**

動いている v0.3 のアプリを終了し（`~/.agents-monitor/state.json` は残す）、新しいアプリを起動する。一覧に前のセッションがそのまま出ること（Review Focus 1）。プラグインが 0.4.0 に更新されること（`claude plugin list --json` の `version`）。

- [ ] **Step 3: 設計書 8 章の実機確認**

- アプリを終了してから Claude Code を使い、`ls ~/.agents-monitor/events | wc -l` が増え、上限（10,000）を超えないこと（上限まで溜めるのは時間がかかるので、`MAX_SPOOL_FILES` 個の空のファイルを置いてから 1 回使い、増えないことで確かめてよい）。アプリを起動すると空になり、また書かれること（Review Focus 3）
- `claude -p "hello"` を実行しても一覧に出ず、通知も来ないこと
- このリポジトリで Claude Code に `crates/core/src/transcript.rs` を読ませた直後に、15 秒以上かかる作業をさせて、「完了（中断）」にならないこと（Review Focus 2）
- アプリを終了した状態で、通知センターに残っている agents-monitor の通知を押す。アプリが起動し、落ちないこと（移動するか、一覧が開くか、どちらでもよい。何が起きたかを報告に書く）
- 起動の瞬間に Dock にアイコンが出ないこと

- [ ] **Step 4: 全体の見直し**

最も能力の高いモデルの、新しいエージェントに、`main` からこのブランチまでの差分全体を見直してもらう（設計書と計画を渡す）。出た指摘を直し、直したものはコミットする。

- [ ] **Step 5: ユーザーに報告する**

Task 6・Task 13 で測った値、実機確認の結果、見直しで直したもの・残したものを報告する。マージと push はユーザーの指示を待つ。
