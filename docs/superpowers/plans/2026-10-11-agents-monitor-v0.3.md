# agents-monitor v0.3（画面移動）実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 一覧の行や通知を押したら、そのセッションが動いている画面を前面に出す。VS Code 系は会話のタブまで、ターミナルはアプリまで。移動できなければ詳細を開くだけ。

**Architecture:** 移動先は core の `jump` が、セッションの情報・プロセスの親子関係・アプリの URL スキーム（読み取り関数を外から渡す）から純粋に決める。アプリの `jumper` は、移動先に応じて `open` を 5 秒の時間制限つきで実行するだけ。通知は識別子にセッション ID を入れ、押されたら同じ `jumper` を呼ぶ。

**Tech Stack:** Rust、Tauri 2、objc2-user-notifications（`UNNotificationResponse`）、macOS の `open`・`plutil`・`ps`、Vite + TypeScript

**Spec:** `docs/superpowers/specs/2026-10-11-agents-monitor-v0.3-design.md`

**前提:** 日英対応（`docs/superpowers/plans/2026-10-11-agents-monitor-v0.2.1-i18n.md`）が済んでいること。文言は `crates/core/locales/{ja,en}.json` に足し、画面では `t()` で引く。

## Global Constraints

- 移動できない・失敗したときは詳細を開くだけ。「移動できません」などのメッセージは出さない
- tmux の中のセッション（`TMUX_PANE` がある）は対象外
- ターミナルはアプリを前面に出すだけ。AppleScript・オートメーションの許可は使わない
- 外部コマンドは引数で渡し、シェルを通さない。5 秒で打ち切る
- URI に入れるセッション ID は、英数字とハイフンだけの 8〜64 文字であることを確かめてから使う
- 画面の文字色に状態色を使わない。任意の文字列は `textContent` で入れる
- 新しいライブラリを追加しない。コミットメッセージは日本語。コメントは WHY が非自明なときだけ

## Review Focus

1. **アプリのパスにスペースが入る**（`/Applications/Visual Studio Code.app/…`）→ 空白で区切らずに `.app` のパスを取り出せる（Task 1 でテスト）
2. **VS Code の拡張ホストは `.app` の中の `.app`（`Code Helper (Plugin).app`）で動く**→ いちばん外側の `.app` を取る（Task 1 でテスト）
3. **詳細ボタンと行の押下・ドラッグが重なる**→ ∨ を押しても移動しない、ドラッグを離しても移動しない（Task 4 の実装と手動確認）
4. **通知を押したときにセッションがもう終わっている・一覧に出ていない**→ 落ちずに一覧を開くだけ（Task 3）
5. **セッション ID に URI として危ない文字が入る**（状態不明の行の `pid:123`、壊れたイベント）→ 会話の URI は作らず、アプリを前面に出すだけ（Task 1 でテスト）

---

## ファイル構成

| ファイル | 役割 | 新規/変更 |
|---|---|---|
| `crates/core/src/jump.rs` | 移動先の決定、`.app` の取り出し、URL スキームの読み取り、セッション ID の確認 | 新規 |
| `crates/core/src/claude_cli.rs` | 時間制限つきの実行を `run_command` として切り出す（`ClaudeCli::run` もこれを使う） | 変更 |
| `app/src-tauri/src/jumper.rs` | 移動の実行、一覧ウィンドウの開け閉め | 新規 |
| `app/src-tauri/src/notifier.rs` | 通知の識別子にセッション ID、押されたときの処理 | 変更 |
| `app/src-tauri/src/commands.rs`, `main.rs`, `tray.rs` | `jump_session` コマンド、詳細を開く指示 | 変更 |
| `app/src/list.ts`, `app/src/style.css`, `app/index.html` | 行を押すと移動、右側の 2 段（経過時間・∨/∧） | 変更 |
| `crates/core/locales/{ja,en}.json`, `crates/core/src/messages.rs` | 「詳細を開く／閉じる」 | 変更 |
| `README.md`, `README.en.md` | VS Code の初回の確認画面の案内 | 変更 |

---

### Task 1: 移動先を決める（core）

**Files:**
- Create: `crates/core/src/jump.rs`
- Modify: `crates/core/src/lib.rs`（`pub mod jump;`）

**Interfaces:**
- Consumes: `state::Session`、`event::TerminalInfo`、`ps::ProcRow`、`store::UNKNOWN_ID_PREFIX`
- Produces:
  - `jump::JumpTarget { EditorSession { app: PathBuf, folder: Option<String>, url: String }, App { app: PathBuf, folder: Option<String> }, None }`
  - `jump::outermost_app(rows: &[ProcRow], pid: u32) -> Option<PathBuf>`
  - `jump::url_scheme_from_plist_json(json: &str) -> Option<String>`
  - `jump::is_session_id(id: &str) -> bool`
  - `jump::jump_target(session: &Session, rows: &[ProcRow], url_scheme: impl Fn(&Path) -> Option<String>) -> JumpTarget`

- [ ] **Step 1: 失敗するテストを書く**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::TerminalInfo;
    use crate::state::Status;

    const VSCODE_HELPER: &str = "/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper (Plugin).app/Contents/MacOS/Code Helper (Plugin) --type=utility";
    const CLAUDE_IN_VSCODE: &str = "/Users/u/.vscode/extensions/anthropic.claude-code-2.1.296-darwin-arm64/resources/native-binary/claude --output-format stream-json";

    fn row(pid: u32, ppid: u32, args: &str) -> ProcRow {
        ProcRow { pid, ppid, tty: None, args: args.into() }
    }

    fn vscode_rows() -> Vec<ProcRow> {
        vec![row(1, 0, "/sbin/launchd"), row(100, 1, "/Applications/Visual Studio Code.app/Contents/MacOS/Electron"), row(200, 100, VSCODE_HELPER), row(300, 200, CLAUDE_IN_VSCODE)]
    }

    fn terminal_rows() -> Vec<ProcRow> {
        vec![row(1, 0, "/sbin/launchd"), row(10, 1, "/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal"), row(11, 10, "login -pf u"), row(12, 11, "-zsh"), row(13, 12, "/Users/u/.local/bin/claude")]
    }

    fn session(id: &str, pid: u32, entrypoint: Option<&str>, tmux_pane: Option<&str>) -> Session {
        Session {
            id: id.into(),
            status: Status::Done,
            detail_kind: None,
            detail_text: None,
            status_since_ms: 0,
            first_seen_ms: 0,
            last_event_ms: 0,
            cwd: "/Users/u/Projects/api".into(),
            config_dir: "/Users/u/.claude".into(),
            git_branch: None,
            claude_pid: Some(pid),
            terminal: Some(TerminalInfo { entrypoint: entrypoint.map(String::from), tmux_pane: tmux_pane.map(String::from), ..Default::default() }),
            transcript_path: None,
            active_subagents: 0,
        }
    }

    const ID: &str = "f94db67f-a0b2-4d0c-a9c9-b746bce595b2";

    #[test]
    fn takes_the_outermost_app_even_with_spaces_in_the_path() {
        assert_eq!(outermost_app(&vscode_rows(), 300), Some(PathBuf::from("/Applications/Visual Studio Code.app")));
        assert_eq!(outermost_app(&terminal_rows(), 13), Some(PathBuf::from("/System/Applications/Utilities/Terminal.app")));
        assert_eq!(outermost_app(&[row(5, 1, "tmux new"), row(6, 5, "claude")], 6), None);
        assert_eq!(outermost_app(&vscode_rows(), 999), None, "プロセスがもうない");
    }

    #[test]
    fn reads_the_url_scheme_from_info_plist_json() {
        let json = r#"{"CFBundleIdentifier":"com.microsoft.VSCode","CFBundleURLTypes":[{"CFBundleURLSchemes":["vscode"]}]}"#;
        assert_eq!(url_scheme_from_plist_json(json).as_deref(), Some("vscode"));
        assert_eq!(url_scheme_from_plist_json(r#"{"CFBundleURLTypes":[{"CFBundleURLSchemes":["file","windsurf"]}]}"#).as_deref(), Some("windsurf"));
        assert_eq!(url_scheme_from_plist_json(r#"{"CFBundleIdentifier":"x"}"#), None);
        assert_eq!(url_scheme_from_plist_json("not json"), None);
    }

    #[test]
    fn session_ids_must_be_plain() {
        assert!(is_session_id(ID));
        assert!(!is_session_id("pid:123"));
        assert!(!is_session_id("abc"));
        assert!(!is_session_id("f94db67f&prompt=rm"));
    }

    #[test]
    fn editor_sessions_open_the_conversation() {
        let target = jump_target(&session(ID, 300, Some("claude-vscode"), None), &vscode_rows(), |_| Some("vscode".into()));
        assert_eq!(
            target,
            JumpTarget::EditorSession {
                app: PathBuf::from("/Applications/Visual Studio Code.app"),
                folder: Some("/Users/u/Projects/api".into()),
                url: format!("vscode://anthropic.claude-code/open?session={ID}"),
            }
        );
    }

    #[test]
    fn editors_without_a_scheme_or_a_plain_id_only_come_to_the_front() {
        let app = PathBuf::from("/Applications/Visual Studio Code.app");
        let folder = Some("/Users/u/Projects/api".to_string());
        assert_eq!(jump_target(&session(ID, 300, Some("claude-vscode"), None), &vscode_rows(), |_| None), JumpTarget::App { app: app.clone(), folder: folder.clone() });
        assert_eq!(jump_target(&session("pid:300", 300, Some("claude-vscode"), None), &vscode_rows(), |_| Some("vscode".into())), JumpTarget::App { app, folder });
    }

    #[test]
    fn terminals_only_come_to_the_front() {
        let target = jump_target(&session(ID, 13, Some("cli"), None), &terminal_rows(), |_| None);
        assert_eq!(target, JumpTarget::App { app: PathBuf::from("/System/Applications/Utilities/Terminal.app"), folder: None });
    }

    #[test]
    fn tmux_and_missing_processes_have_no_target() {
        assert_eq!(jump_target(&session(ID, 13, Some("cli"), Some("%3")), &terminal_rows(), |_| None), JumpTarget::None);
        assert_eq!(jump_target(&session(ID, 999, Some("cli"), None), &terminal_rows(), |_| None), JumpTarget::None);
        let mut no_pid = session(ID, 13, Some("cli"), None);
        no_pid.claude_pid = None;
        assert_eq!(jump_target(&no_pid, &terminal_rows(), |_| None), JumpTarget::None);
    }
}
```

（`Session` の項目は日英対応の後の形。`detail_kind`・`detail_text` がまだなら `detail: None` にする）

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core jump::`
Expected: コンパイルエラー

- [ ] **Step 3: 実装する**（テストの上）

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ps::ProcRow;
use crate::state::Session;

const EDITOR_ENTRYPOINT: &str = "claude-vscode";
const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JumpTarget {
    EditorSession { app: PathBuf, folder: Option<String>, url: String },
    App { app: PathBuf, folder: Option<String> },
    None,
}

// ps の args は実行ファイルのパスの空白を区切らずに持つので、空白で分けずに最初の ".app/" までを取る。
// 最初に現れる ".app" がいちばん外側（VS Code の拡張ホストは .app の中の .app で動く）
fn app_in_args(args: &str) -> Option<PathBuf> {
    if !args.starts_with('/') {
        return None;
    }
    let end = args.find(".app/")? + ".app".len();
    Some(PathBuf::from(&args[..end]))
}

pub fn outermost_app(rows: &[ProcRow], pid: u32) -> Option<PathBuf> {
    let by_pid: HashMap<u32, &ProcRow> = rows.iter().map(|r| (r.pid, r)).collect();
    let mut current = by_pid.get(&pid)?;
    for _ in 0..MAX_DEPTH {
        if let Some(app) = app_in_args(&current.args) {
            return Some(app);
        }
        if current.ppid == 0 || current.ppid == current.pid {
            return None;
        }
        current = by_pid.get(&current.ppid)?;
    }
    None
}

pub fn url_scheme_from_plist_json(json: &str) -> Option<String> {
    let plist: serde_json::Value = serde_json::from_str(json).ok()?;
    plist["CFBundleURLTypes"]
        .as_array()?
        .iter()
        .filter_map(|t| t["CFBundleURLSchemes"].as_array())
        .flatten()
        .filter_map(|s| s.as_str())
        .find(|s| !matches!(*s, "file" | "http" | "https"))
        .map(String::from)
}

pub fn is_session_id(id: &str) -> bool {
    (8..=64).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub fn jump_target(session: &Session, rows: &[ProcRow], url_scheme: impl Fn(&Path) -> Option<String>) -> JumpTarget {
    let terminal = session.terminal.as_ref();
    if terminal.is_some_and(|t| t.tmux_pane.is_some()) {
        return JumpTarget::None;
    }
    let Some(app) = session.claude_pid.and_then(|pid| outermost_app(rows, pid)) else { return JumpTarget::None };
    let in_editor = terminal.and_then(|t| t.entrypoint.as_deref()) == Some(EDITOR_ENTRYPOINT);
    let folder = (in_editor && !session.cwd.is_empty()).then(|| session.cwd.clone());
    if in_editor && is_session_id(&session.id) {
        if let Some(scheme) = url_scheme(&app) {
            let url = format!("{scheme}://anthropic.claude-code/open?session={}", session.id);
            return JumpTarget::EditorSession { app, folder, url };
        }
    }
    JumpTarget::App { app, folder }
}
```

- [ ] **Step 4: 通ることを確かめ、コミット**

Run: `cargo test -p agents-monitor-core jump::`
Expected: 7 件 PASS

```bash
git add crates/core/src/jump.rs crates/core/src/lib.rs
git commit -m "セッションの移動先（エディターの会話・アプリ・なし）を決める仕組みを足す"
```

---

### Task 2: 移動を実行する（アプリ）

**Files:**
- Modify: `crates/core/src/claude_cli.rs`（`run_command` を切り出す）
- Create: `app/src-tauri/src/jumper.rs`
- Modify: `app/src-tauri/src/commands.rs`, `app/src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `jump::{jump_target, url_scheme_from_plist_json, JumpTarget}`（Task 1）、`ps::run_ps`
- Produces:
  - `claude_cli::run_command(program: &Path, args: &[String], envs: &[(&str, &OsStr)], timeout: Duration) -> Result<String, CliError>`
  - `jumper::jump(app: &AppHandle, session_id: &str) -> bool`（移動できたら true）
  - コマンド `jump_session(id: String) -> bool`

- [ ] **Step 1: `run_command` を切り出す**

`ClaudeCli::run` の中身（子プロセスの起動、出力の読み出し、時間切れでの打ち切り）を `pub fn run_command(program, args, envs, timeout)` に移し、`ClaudeCli::run` は `run_command(&self.program, args, &[("CLAUDE_CONFIG_DIR", config_dir.as_os_str()), ("PATH", OsStr::new(&self.path_env()))], timeout)` を呼ぶだけにする。既存の `claude_cli` のテストがそのまま通ることを確かめる。

Run: `cargo test -p agents-monitor-core claude_cli::`
Expected: PASS（振る舞いは変わらない）

- [ ] **Step 2: `jumper` を書く**（`app/src-tauri/src/jumper.rs`）

```rust
use std::path::Path;
use std::time::Duration;

use agents_monitor_core::claude_cli::run_command;
use agents_monitor_core::jump::{jump_target, url_scheme_from_plist_json, JumpTarget};
use agents_monitor_core::ps::run_ps;
use tauri::{AppHandle, Manager};

use crate::monitor::AppState;

const OPEN_TIMEOUT: Duration = Duration::from_secs(5);

pub fn jump(app: &AppHandle, session_id: &str) -> bool {
    let session = app.state::<AppState>().store.lock().unwrap().sessions().iter().find(|s| s.id == session_id).cloned();
    let Some(session) = session else { return false };
    let Some(rows) = run_ps() else { return false };
    let target = jump_target(&session, &rows, url_scheme);
    if target == JumpTarget::None {
        return false;
    }
    crate::tray::hide_list(app);
    let opened = match &target {
        JumpTarget::EditorSession { app: path, folder, url } => {
            // フォルダーでウィンドウを前に出してから URI を開かないと、最後に使ったウィンドウで会話が開く
            open_app(path, folder.as_deref()) && {
                if !open(&[url.as_str()]) {
                    eprintln!("failed to open the conversation URL; the window is already in front");
                }
                true
            }
        }
        JumpTarget::App { app: path, folder } => open_app(path, folder.as_deref()),
        JumpTarget::None => false,
    };
    if !opened {
        crate::tray::show_list(app);
    }
    opened
}

fn open_app(app: &Path, folder: Option<&str>) -> bool {
    let app = app.to_string_lossy();
    let mut args = vec!["-a", app.as_ref()];
    args.extend(folder);
    open(&args)
}

fn open(args: &[&str]) -> bool {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    match run_command(Path::new("/usr/bin/open"), &args, &[], OPEN_TIMEOUT) {
        Ok(_) => true,
        Err(error) => {
            eprintln!("open failed: {error:?}");
            false
        }
    }
}

fn url_scheme(app: &Path) -> Option<String> {
    let plist = app.join("Contents/Info.plist");
    let args = vec!["-convert".into(), "json".into(), "-o".into(), "-".into(), plist.to_string_lossy().into_owned()];
    let json = run_command(Path::new("/usr/bin/plutil"), &args, &[], OPEN_TIMEOUT).ok()?;
    url_scheme_from_plist_json(&json)
}
```

（`run_command` の `envs` に何も渡さないときは `&[]`）

`app/src-tauri/src/tray.rs` に、一覧を閉じる・開く関数を足す（今の `toggle_list` の中身を分ける）。

```rust
pub fn hide_list(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("list") {
        let _ = window.hide();
        app.state::<AppState>().list_hidden_at_ms.store(paths::now_ms(), Ordering::Relaxed);
    }
}

pub fn show_list(app: &AppHandle) {
    let Some(window) = app.get_webview_window("list") else { return };
    let _ = window.move_window(Position::TrayBottomCenter);
    let _ = window.show();
    let _ = window.set_focus();
}
```

`toggle_list` と `show_settings` は `show_list` を使う形にする。

- [ ] **Step 3: コマンドを足す**（`commands.rs`、`main.rs` の `generate_handler!` と `mod jumper;`）

```rust
// ps と open を呼ぶので、画面を固めないよう別のスレッドで動かす
#[tauri::command]
pub async fn jump_session(app: AppHandle, id: String) -> bool {
    tauri::async_runtime::spawn_blocking(move || crate::jumper::jump(&app, &id)).await.unwrap_or(false)
}
```

- [ ] **Step 4: ビルドと、実装の最初に確かめること**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets`
Expected: PASS、警告なし

（画面からはまだ呼ばないので、確かめは Task 4 の後にまとめる。ここでは `open -a "/Applications/Visual Studio Code.app" <フォルダー>` → URI の 2 段を手で打ち、`open -b` のときと同じように正しいウィンドウで会話が開くことを確かめ、台帳に書く）

- [ ] **Step 5: コミット**

```bash
git add crates/core/src/claude_cli.rs app/src-tauri/src
git commit -m "移動先に応じて open を実行し、失敗したら一覧を開いたままにする"
```

---

### Task 3: 通知を押したら移動する（アプリ）

**Files:**
- Modify: `app/src-tauri/src/notifier.rs`, `app/src-tauri/Cargo.toml`（objc2-user-notifications の features に `"UNNotificationResponse"`）, `app/src-tauri/src/monitor.rs`, `app/src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `jumper::jump`（Task 2）、`tray::show_list`
- Produces: `notifier::show(title: &str, body: &str, session_id: &str)`、`notifier::init(on_change, on_open: impl Fn(String) + Send + Sync + 'static)`、イベント `show-details`（引数: セッション ID）

- [ ] **Step 1: 通知の識別子にセッション ID を入れる**

`show` に `session_id` を足し、識別子を `NSUUID` ではなく `format!("session:{session_id}")` にする（同じセッションの古い通知は置き換わる）。`monitor.rs` の `notifier::show(&notice.title, &notice.body)` を、変化した `change.session_id` を渡す形にする（`Notice` に `session_id: String` を足し、`notice_for` で埋める。`notify.rs` のテストの期待値にも足す）。

- [ ] **Step 2: 押されたときの処理を足す**

`ForegroundPresenter` の delegate に次を足す（`UNNotificationResponse` を import）。押されたときの処理は `init` で受け取ったものを `OnceLock` に入れておく。

```rust
#[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
fn did_receive(
    &self,
    _center: &UNUserNotificationCenter,
    response: &UNNotificationResponse,
    completion_handler: &block2::DynBlock<dyn Fn()>,
) {
    let identifier = response.notification().request().identifier().to_string();
    if let (Some(session_id), Some(on_open)) = (identifier.strip_prefix("session:"), ON_OPEN.get()) {
        on_open(session_id.to_string());
    }
    completion_handler.call(());
}
```

```rust
static ON_OPEN: OnceLock<Box<dyn Fn(String) + Send + Sync>> = OnceLock::new();
```

`init(on_change, on_open)` の最初で `let _ = ON_OPEN.set(Box::new(on_open));`。macOS 以外の `imp` の `init` にも同じ引数を足す。メソッド名や引数の型が合わなければ `objc2-user-notifications-0.3.*/src/generated/UNUserNotificationCenter.rs` の `didReceiveNotificationResponse` を読んで合わせる。

`main.rs`:

```rust
let on_open_handle = app.handle().clone();
notifier::init(
    move || installer::publish(&handle),
    move |session_id| {
        let app = on_open_handle.clone();
        // 通知の処理は macOS のスレッドで呼ばれるので、ps や open は別のスレッドで動かす
        std::thread::spawn(move || {
            if !jumper::jump(&app, &session_id) {
                tray::show_list(&app);
                let _ = app.emit("show-details", session_id);
            }
        });
    },
);
```

- [ ] **Step 3: テストとビルド、実装の最初に確かめること**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets && ./scripts/build-hook-sidecar.sh && (cd app && npm run tauri build -- --bundles app)`
パッケージ版で確かめ、台帳に書く（設計書 6 章）:
- 通知を出し（会話を「完了」にする）、押すと、そのセッションの VS Code の会話へ移動する
- アプリが通知を受け取れない（何も起きない）場合は、作業を止めて報告する

- [ ] **Step 4: コミット**

```bash
git add app/src-tauri crates/core/src/notify.rs Cargo.lock
git commit -m "通知の識別子にセッション ID を入れ、押されたらそのセッションへ移動する"
```

---

### Task 4: 行を押すと移動し、詳細は右側のボタンで開く（画面）

**Files:**
- Modify: `app/src/list.ts`, `app/src/style.css`, `crates/core/locales/{ja,en}.json`

**Interfaces:**
- Consumes: コマンド `jump_session`（Task 2）、イベント `show-details`（Task 3）
- Produces: なし

始める前に Figma の「v0.3 下書き」（`131:609`。ユーザーが微修正済み）を `get_design_context` で読み、右側の 2 段の余白・ボタンの大きさを確かめる。

- [ ] **Step 1: 文言を足す**

`ja.json` に `"details.show": "詳細を開く"`、`"details.hide": "詳細を閉じる"`、`en.json` に `"details.show": "Show details"`、`"details.hide": "Hide details"` を足す（キーの順は両方そろえる）。

Run: `cargo test -p agents-monitor-core messages::`
Expected: PASS（両方の言語にそろっている）

- [ ] **Step 2: 行を組み立て直す**（`list.ts` の `renderRow`）

```ts
const ICON_EXPAND_MORE = `<svg width="16" height="16" viewBox="0 -960 960 960" fill="currentColor" aria-hidden="true"><path d="M480-345 240-585l56-56 184 184 184-184 56 56-240 240Z"/></svg>`;
const ICON_EXPAND_LESS = `<svg width="16" height="16" viewBox="0 -960 960 960" fill="currentColor" aria-hidden="true"><path d="M296-345 240-401l240-240 240 240-56 56-184-184-184 184Z"/></svg>`;

function toggleDetails(id: string): void {
  if (expanded.has(id)) expanded.delete(id);
  else expanded.add(id);
  render();
}

function renderRow(session: SessionView, now: number): HTMLElement {
  const row = el("li", "row");
  if (session.status !== "unknown") row.dataset.id = session.id;
  const dotBox = el("span", "dot-box");
  dotBox.append(el("span", `dot dot-${session.status}`));
  const text = el("div", "text");
  text.append(el("div", "project", session.project), el("div", "meta", metaText(session)));
  const open = expanded.has(session.id);
  if (open) text.append(renderDetails(session));
  const trailing = el("div", "trailing");
  const elapsed = el("span", "elapsed", ELAPSED_STATUSES.has(session.status) ? formatElapsed(now - session.status_since_ms) : "");
  const toggle = el("button", "icon-button details-toggle");
  toggle.type = "button";
  toggle.innerHTML = open ? ICON_EXPAND_LESS : ICON_EXPAND_MORE;
  const label = t(open ? "details.hide" : "details.show");
  toggle.title = label;
  toggle.setAttribute("aria-label", label);
  toggle.addEventListener("click", (event) => {
    event.stopPropagation();
    toggleDetails(session.id);
  });
  trailing.append(elapsed, toggle);
  row.append(dotBox, text, trailing);
  row.addEventListener("click", () => {
    if (dragger.consumeClick()) return;
    void invoke<boolean>("jump_session", { id: session.id }).then((jumped) => {
      if (!jumped && !expanded.has(session.id)) toggleDetails(session.id);
    });
  });
  return row;
}
```

- 詳細のボタンを押してもドラッグが始まらないよう、`drag.ts` の `pointerdown` で `(event.target as HTMLElement).closest(".details-toggle")` なら何もしないようにする
- 通知から詳細を開く: `listen<string>("show-details", (event) => { showPage("agents"); keepPageOnFocus(); if (!expanded.has(event.payload)) toggleDetails(event.payload); })`

- [ ] **Step 3: 見た目を足す**（`style.css`。Figma の値に合わせて調整する）

```css
.trailing { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; flex: none; }
.trailing .elapsed { min-height: 1em; }
.details-toggle { color: var(--fg-secondary); }
```

既存の `.elapsed` の `padding-top` は消す。

- [ ] **Step 4: ビルドと手動確認**

Run: `(cd app && npm run build) && cargo test --workspace && ./scripts/build-hook-sidecar.sh && (cd app && npm run tauri build -- --bundles app)`
パッケージ版で確かめる（設計書 5 章の実機確認）:
- VS Code: 2 つのウィンドウを開いた状態で、それぞれの行を押すと、そのウィンドウの会話が前に出る
- Windsurf（Claude Code の拡張が入っていれば）: 同じく会話へ移動する（設計書 6 章）
- Terminal.app・iTerm2 の `claude`: 押すとアプリが前面に出る
- tmux の中の `claude`: 押すと詳細が開くだけ
- 状態不明の行（Unknown タブ）: アプリが前面に出る
- ∨ で詳細が開き ∧ で閉じ、押しても移動しない。ドラッグを離しても移動しない
- アプリを終了してから行を押すと、詳細が開くだけ
- 英語の環境でも、∨ の説明が英語になる

- [ ] **Step 5: コミット**

```bash
git add app/src crates/core/locales
git commit -m "行を押すとそのセッションの画面へ移動し、詳細は右側のボタンで開く"
```

---

### Task 5: README の案内と仕上げ

**Files:**
- Modify: `README.md`, `README.en.md`

- [ ] **Step 1: README に移動の説明を足す**

`README.md` の「状態の見方」の後:

```markdown
## 画面移動

一覧の行を押すと、そのセッションが動いている画面が前面に出ます。

- VS Code などのエディター: そのプロジェクトのウィンドウで、その会話のタブが開きます。初回だけ VS Code が「'Claude Code for VS Code' 拡張機能がこの URI を開くことを許可しますか?」と確認するので、「今後、この拡張機能についてメッセージを表示しない」にチェックを入れて「開く」を押してください
- ターミナル（Terminal.app・iTerm2 など）: そのアプリが前面に出ます（タブは選びません）
- tmux の中で動くセッションは移動できません。押すと詳細が開きます

詳細（場所・プロフィール）は、行の右側の ∨ で開きます。
```

`README.en.md` にも同じ内容を英語で足す（VS Code の確認画面の文言は英語版の VS Code の表示 "Allow 'Claude Code for VS Code' extension to open this URI?" と "Don't ask again for this extension"）。

- [ ] **Step 2: すべてのテスト**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets && (cd app && npm run build) && ./scripts/integration-test.sh`
Expected: すべて PASS

- [ ] **Step 3: コミット**

```bash
git add README.md README.en.md
git commit -m "README に画面移動と VS Code の初回の確認画面の案内を足す"
```
