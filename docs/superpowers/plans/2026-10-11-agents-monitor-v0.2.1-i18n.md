# agents-monitor v0.2.1（日英対応）実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 画面・通知・アラートを、macOS の言語に合わせて日本語か英語で表示する。

**Architecture:** 文言は core の `locales/ja.json`・`en.json` にまとめ、core の `messages` が言語・キー・値の差し込みを受け持つ。core は文言を組み立てず、種類（列挙型）と中身を返す。アプリは起動時に言語を決め、通知・アラート・一覧の詳細の文面を作り、画面には `get_messages` で今の言語の表を渡す。

**Tech Stack:** Rust（serde_json、include_str!）、Tauri 2、objc2-foundation（NSBundle）、Vite + TypeScript。新しいライブラリは使わない。

**Spec:** `docs/superpowers/specs/2026-10-11-agents-monitor-v0.2.1-i18n-design.md`

## Global Constraints

- 対応する言語は日本語（`ja`）と英語（`en`）だけ。日本語でなければ英語
- 言語は起動時に 1 回だけ決める。macOS では `NSBundle.mainBundle.preferredLocalizations` の先頭、バンドルでないときは `LANG`
- 文言はすべて `crates/core/locales/{ja,en}.json` に置く。キーは平らな名前（`status.needs_action`）、差し込む値は `{name}`
- 英語の文言は設計書 3 章の表のとおり
- `claude` 自身が出したエラーの文は翻訳しない
- ログ（`eprintln!`）は英語。翻訳しない
- 画面の文字色に状態色を使わない（v0.2 からの約束）。任意の文字列は `textContent` で入れる
- 新しいライブラリを追加しない。コミットメッセージは日本語。コメントは WHY が非自明なときだけ

## Review Focus

1. **v0.2 までの `state.json`（`detail` が日本語の文字列）を読む**→ 落ちずに読め、状態は残り、古い詳細は捨てられる（Task 2 でテスト）
2. **片方の言語にだけあるキー・使っているのに表にないキー**（画面の HTML の `data-i18n` と TypeScript の `t("…")` を含む）→ テストで落ちる（Task 1・5 でテスト）
3. **差し込む値の名前が言語で食い違う**（ja は `{dir}`、en は `{path}` など）→ テストで落ちる（Task 1 でテスト）
4. **画面が対応表を受け取る前に描かれる**→ 空のキーや `undefined` が一瞬出ない。受け取ってから最初の描画をする（Task 5）
5. **通知の文面が言語どおりになる**（要対応の見出しと本文の分け方を含む）→ 英語でも「{project} — Waiting for permission」と本文に分かれる（Task 2 でテスト）

---

## ファイル構成

| ファイル | 役割 | 新規/変更 |
|---|---|---|
| `crates/core/locales/ja.json`, `en.json` | 対応表 | 新規 |
| `crates/core/src/messages.rs` | `Lang`、`Msg`、`text`・`format`、`table` | 新規 |
| `crates/core/src/state.rs` | `DetailKind`、`Session`・`Transition` の詳細を種類と中身に分ける。`Status::label` をなくす | 変更 |
| `crates/core/src/store.rs`, `ps.rs` | 詳細を種類と中身で作る。`launch_origin` を列挙型に | 変更 |
| `crates/core/src/view.rs` | 詳細の文面を言語で作る（`detail_label`） | 変更 |
| `crates/core/src/notify.rs` | 通知の文面を言語で作る | 変更 |
| `crates/core/src/claude_cli.rs`, `plugin_status.rs` | エラーを `CliError` で返す | 変更 |
| `app/src-tauri/src/lang.rs` | 起動時の言語の決定 | 新規 |
| `app/src-tauri/Info.plist` | `CFBundleLocalizations` | 新規 |
| `app/src-tauri/src/*.rs` | 言語を使って通知・アラート・一覧を作る、`get_messages`、ログを英語に | 変更 |
| `app/src-tauri/tests/messages_used.rs` | 画面で使うキーが表にあるか | 新規 |
| `app/src/i18n.ts` | `t()`、`data-i18n` の適用 | 新規 |
| `app/index.html`, `app/src/*.ts` | 文言を `t()`・`data-i18n` に | 変更 |
| `plugin/.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json` | 説明を英語に | 変更 |
| `README.en.md`, `README.md` | 英語の README、相互リンク | 新規・変更 |

---

### Task 1: 対応表と `messages`（core）

**Files:**
- Create: `crates/core/locales/ja.json`, `crates/core/locales/en.json`, `crates/core/src/messages.rs`
- Modify: `crates/core/src/lib.rs`（`pub mod messages;`）

**Interfaces:**
- Produces:
  - `messages::Lang { Ja, En }`（`Copy`、`serde` で `"ja"`/`"en"`）、`Lang::from_tag(tag: &str) -> Lang`
  - `messages::Msg`（列挙型。`Msg::key(&self) -> &'static str`、`Msg::ALL: &[Msg]`）
  - `messages::text(lang: Lang, msg: Msg) -> String`
  - `messages::format(lang: Lang, msg: Msg, args: &[(&str, &str)]) -> String`
  - `messages::table(lang: Lang) -> &'static HashMap<String, String>`（画面に渡す）
  - `messages::table_by_key(lang: Lang, key: &str) -> Option<&'static str>`

- [ ] **Step 1: 対応表を書く**

`crates/core/locales/ja.json`（キーは下の en と同じ順にそろえる）:

```json
{
  "status.needs_action": "要対応",
  "status.error": "エラー停止",
  "status.done": "完了",
  "status.running": "実行中",
  "status.ended": "終了",
  "status.unknown": "状態不明",
  "detail.new": "新規",
  "detail.permission": "許可待ち",
  "detail.question": "質問待ち",
  "detail.plan_approval": "承認待ち",
  "detail.input": "入力要求",
  "detail.interrupted": "中断",
  "detail.subagent": "サブエージェント実行中",
  "detail.compacting": "圧縮中",
  "detail.background": "バックグラウンド作業あり",
  "detail.not_monitored": "監視対象外",
  "detail.terminal": "ターミナル",
  "elapsed.seconds": "{n}秒",
  "elapsed.minutes": "{n}分",
  "elapsed.hours": "{n}時間",
  "elapsed.days": "{n}日",
  "list.empty": "動いている Claude Code セッションはありません",
  "details.location": "場所: {path}",
  "details.profile": "プロフィール: {name}",
  "details.unknown_value": "不明",
  "details.not_monitored_help": "この会話を開いた後にプラグインが入ったか、使っている設定にプラグインが入っていません。プラグインを入れてから、会話を開き直すと監視されます。",
  "tab.menu": "メニュー",
  "settings.notifications": "通知設定",
  "settings.notifications_off": "通知がオフになっています",
  "settings.open_system_settings": "システム設定を開く",
  "settings.claude_configs": "Claude Code の設定",
  "settings.add_config_dir": "＋ 設定ディレクトリを追加",
  "settings.no_config_dirs": "Claude Code の設定ディレクトリが見つかりませんでした。使用している設定ディレクトリを追加してください。",
  "settings.claude_not_found": "claude コマンドが見つかりません。場所を入力してください。",
  "settings.retry": "もう一度",
  "settings.remove": "削除",
  "settings.launch_at_login": "ログイン時に起動する",
  "settings.quit": "終了する",
  "dialog.choose_config_dir": "Claude Code の設定ディレクトリを選ぶ",
  "notice.title": "{project} — {headline}",
  "error.install_failed": "{dir} にプラグインを入れられませんでした。\n{reason}",
  "error.remove_failed": "{dir} からプラグインを外せませんでした。\n{reason}",
  "error.marketplace_unavailable": "同梱のプラグインを用意できませんでした: {error}",
  "error.claude_not_found": "claude コマンドが見つかりません",
  "error.claude_path_invalid": "そのパスに実行できる claude が見つかりません",
  "error.timeout": "時間切れ（応答がありませんでした）",
  "error.unreadable_status": "導入状態を読めませんでした",
  "error.cli_spawn": "claude を起動できませんでした: {error}",
  "error.cli_wait": "claude の終了を待てませんでした: {error}",
  "error.unknown_reason": "理由がわかりませんでした",
  "error.save_settings": "設定を保存できませんでした: {error}",
  "error.save_order": "並び順を保存できませんでした: {error}",
  "error.autostart": "ログイン時の起動設定を変更できませんでした: {error}"
}
```

`crates/core/locales/en.json`:

```json
{
  "status.needs_action": "Needs action",
  "status.error": "Error",
  "status.done": "Done",
  "status.running": "Running",
  "status.ended": "Ended",
  "status.unknown": "Unknown",
  "detail.new": "New",
  "detail.permission": "Waiting for permission",
  "detail.question": "Waiting for your answer",
  "detail.plan_approval": "Waiting for plan approval",
  "detail.input": "Input requested",
  "detail.interrupted": "Interrupted",
  "detail.subagent": "Running a subagent",
  "detail.compacting": "Compacting",
  "detail.background": "Background work running",
  "detail.not_monitored": "Not monitored",
  "detail.terminal": "Terminal",
  "elapsed.seconds": "{n}s",
  "elapsed.minutes": "{n}m",
  "elapsed.hours": "{n}h",
  "elapsed.days": "{n}d",
  "list.empty": "No Claude Code sessions are running",
  "details.location": "Location: {path}",
  "details.profile": "Profile: {name}",
  "details.unknown_value": "unknown",
  "details.not_monitored_help": "The plugin was installed after this conversation started, or isn't installed for its config. Install it, then reopen the conversation to start monitoring.",
  "tab.menu": "Menu",
  "settings.notifications": "Notifications",
  "settings.notifications_off": "Notifications are turned off",
  "settings.open_system_settings": "Open System Settings",
  "settings.claude_configs": "Claude Code configs",
  "settings.add_config_dir": "＋ Add config directory",
  "settings.no_config_dirs": "No Claude Code config directory was found. Add the one you use.",
  "settings.claude_not_found": "The claude command was not found. Enter its path.",
  "settings.retry": "Retry",
  "settings.remove": "Remove",
  "settings.launch_at_login": "Launch at login",
  "settings.quit": "Quit",
  "dialog.choose_config_dir": "Choose a Claude Code config directory",
  "notice.title": "{project} — {headline}",
  "error.install_failed": "Couldn't install the plugin in {dir}.\n{reason}",
  "error.remove_failed": "Couldn't remove the plugin from {dir}.\n{reason}",
  "error.marketplace_unavailable": "Couldn't prepare the bundled plugin: {error}",
  "error.claude_not_found": "The claude command was not found",
  "error.claude_path_invalid": "No executable claude was found at that path",
  "error.timeout": "Timed out (no response)",
  "error.unreadable_status": "Couldn't read the plugin status",
  "error.cli_spawn": "Couldn't start claude: {error}",
  "error.cli_wait": "Couldn't wait for claude to finish: {error}",
  "error.unknown_reason": "Unknown reason",
  "error.save_settings": "Couldn't save the settings: {error}",
  "error.save_order": "Couldn't save the order: {error}",
  "error.autostart": "Couldn't change the launch-at-login setting: {error}"
}
```

- [ ] **Step 2: 失敗するテストを書く**（`crates/core/src/messages.rs`）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn placeholders(text: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        let mut rest = text;
        while let Some(start) = rest.find('{') {
            let Some(end) = rest[start..].find('}') else { break };
            found.insert(rest[start + 1..start + end].to_string());
            rest = &rest[start + end + 1..];
        }
        found
    }

    #[test]
    fn both_languages_have_the_same_keys() {
        let ja: BTreeSet<&String> = table(Lang::Ja).keys().collect();
        let en: BTreeSet<&String> = table(Lang::En).keys().collect();
        assert_eq!(ja, en);
    }

    #[test]
    fn every_msg_exists_in_both_languages() {
        for msg in Msg::ALL {
            for lang in [Lang::Ja, Lang::En] {
                assert!(table(lang).contains_key(msg.key()), "{} が {lang:?} にない", msg.key());
            }
        }
    }

    #[test]
    fn placeholders_match_across_languages() {
        for (key, ja) in table(Lang::Ja) {
            assert_eq!(placeholders(ja), placeholders(&table(Lang::En)[key]), "{key}");
        }
    }

    #[test]
    fn formats_values_into_the_text() {
        assert_eq!(text(Lang::En, Msg::StatusDone), "Done");
        assert_eq!(text(Lang::Ja, Msg::StatusDone), "完了");
        assert_eq!(format(Lang::En, Msg::NoticeTitle, &[("project", "api"), ("headline", "Done")]), "api — Done");
        assert_eq!(format(Lang::Ja, Msg::ErrorTimeout, &[]), "時間切れ（応答がありませんでした）");
    }

    #[test]
    fn language_tags_starting_with_ja_are_japanese() {
        assert_eq!(Lang::from_tag("ja"), Lang::Ja);
        assert_eq!(Lang::from_tag("ja-JP"), Lang::Ja);
        assert_eq!(Lang::from_tag("ja_JP.UTF-8"), Lang::Ja);
        assert_eq!(Lang::from_tag("en"), Lang::En);
        assert_eq!(Lang::from_tag("fr-FR"), Lang::En);
        assert_eq!(Lang::from_tag(""), Lang::En);
    }
}
```

- [ ] **Step 3: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core messages::`
Expected: コンパイルエラー（`table`・`Msg` がない）

- [ ] **Step 4: 実装する**（テストの上）

```rust
use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    Ja,
    En,
}

impl Lang {
    pub fn from_tag(tag: &str) -> Lang {
        if tag.to_ascii_lowercase().starts_with("ja") {
            Lang::Ja
        } else {
            Lang::En
        }
    }
}

macro_rules! messages {
    ($($variant:ident => $key:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Msg { $($variant),* }

        impl Msg {
            pub const ALL: &'static [Msg] = &[$(Msg::$variant),*];

            pub fn key(&self) -> &'static str {
                match self { $(Msg::$variant => $key),* }
            }
        }
    };
}

// Rust から引く文言だけをここに並べる。画面だけで使う文言は表にあればよく、ここには書かない
messages! {
    StatusNeedsAction => "status.needs_action",
    StatusError => "status.error",
    StatusDone => "status.done",
    StatusRunning => "status.running",
    StatusEnded => "status.ended",
    StatusUnknown => "status.unknown",
    DetailNew => "detail.new",
    DetailPermission => "detail.permission",
    DetailQuestion => "detail.question",
    DetailPlanApproval => "detail.plan_approval",
    DetailInput => "detail.input",
    DetailInterrupted => "detail.interrupted",
    DetailSubagent => "detail.subagent",
    DetailCompacting => "detail.compacting",
    DetailBackground => "detail.background",
    DetailNotMonitored => "detail.not_monitored",
    DetailTerminal => "detail.terminal",
    DialogChooseConfigDir => "dialog.choose_config_dir",
    NoticeTitle => "notice.title",
    ErrorInstallFailed => "error.install_failed",
    ErrorRemoveFailed => "error.remove_failed",
    ErrorMarketplaceUnavailable => "error.marketplace_unavailable",
    ErrorClaudeNotFound => "error.claude_not_found",
    ErrorClaudePathInvalid => "error.claude_path_invalid",
    ErrorTimeout => "error.timeout",
    ErrorUnreadableStatus => "error.unreadable_status",
    ErrorCliSpawn => "error.cli_spawn",
    ErrorCliWait => "error.cli_wait",
    ErrorUnknownReason => "error.unknown_reason",
    ErrorSaveSettings => "error.save_settings",
    ErrorSaveOrder => "error.save_order",
    ErrorAutostart => "error.autostart",
}

pub fn table(lang: Lang) -> &'static HashMap<String, String> {
    static JA: OnceLock<HashMap<String, String>> = OnceLock::new();
    static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
    let (cell, source) = match lang {
        Lang::Ja => (&JA, include_str!("../locales/ja.json")),
        Lang::En => (&EN, include_str!("../locales/en.json")),
    };
    cell.get_or_init(|| serde_json::from_str(source).expect("埋め込んだ対応表が読めません"))
}

pub fn table_by_key(lang: Lang, key: &str) -> Option<&'static str> {
    table(lang).get(key).map(String::as_str)
}

pub fn text(lang: Lang, msg: Msg) -> String {
    table_by_key(lang, msg.key()).unwrap_or(msg.key()).to_string()
}

pub fn format(lang: Lang, msg: Msg, args: &[(&str, &str)]) -> String {
    args.iter().fold(text(lang, msg), |acc, (name, value)| acc.replace(&format!("{{{name}}}"), value))
}
```

- [ ] **Step 5: 通ることを確かめ、コミット**

Run: `cargo test -p agents-monitor-core messages::`
Expected: 5 件 PASS

```bash
git add crates/core/locales crates/core/src/messages.rs crates/core/src/lib.rs
git commit -m "日本語と英語の対応表と、言語に合わせて文言を引く仕組みを足す"
```

---

### Task 2: 状態の詳細を種類と中身に分け、一覧と通知の文面を言語で作る（core）

**Files:**
- Modify: `crates/core/src/state.rs`, `crates/core/src/store.rs`, `crates/core/src/ps.rs`, `crates/core/src/view.rs`, `crates/core/src/notify.rs`、それぞれのテスト

**Interfaces:**
- Consumes: `messages::{Lang, Msg, text, format}`（Task 1）
- Produces:
  - `state::DetailKind { New, Permission, Question, PlanApproval, Input, Interrupted, Subagent, Compacting, Tool, Error, Background, NotMonitored }`（`serde` は snake_case）
  - `Session { …, detail_kind: Option<DetailKind>, detail_text: Option<String>, … }`（`detail` はなくす）
  - `Transition { status, detail_kind: Option<DetailKind>, detail_text: Option<String> }`
  - `StatusChange { …, detail_kind, detail_text, … }`（`detail` はなくす）
  - `ps::LaunchOrigin { VsCode, Cursor, Terminal }`、`ps::launch_origin(args: &str) -> LaunchOrigin`
  - `view::detail_label(lang: Lang, kind: Option<DetailKind>, text: Option<&str>) -> Option<String>`
  - `view::status_label(lang: Lang, status: Status) -> String`
  - `view::session_views(sessions, order, monitored, lang: Lang)`（引数に言語を足す）
  - `notify::notice_for(change, notify_on, lang: Lang) -> Option<Notice>`

- [ ] **Step 1: 詳細の文面のテストを書く**（`crates/core/src/view.rs` のテストに足す）

```rust
#[test]
fn detail_label_joins_kind_and_text_in_each_language() {
    use crate::messages::Lang;
    use crate::state::DetailKind;
    assert_eq!(detail_label(Lang::Ja, Some(DetailKind::Permission), Some("Bash: rm -rf dist")).as_deref(), Some("許可待ち · Bash: rm -rf dist"));
    assert_eq!(detail_label(Lang::En, Some(DetailKind::Permission), Some("Bash: rm -rf dist")).as_deref(), Some("Waiting for permission · Bash: rm -rf dist"));
    assert_eq!(detail_label(Lang::En, Some(DetailKind::Question), None).as_deref(), Some("Waiting for your answer"));
    assert_eq!(detail_label(Lang::En, Some(DetailKind::Tool), Some("Read: a.rs")).as_deref(), Some("Read: a.rs"), "ツール実行中は中身だけ");
    assert_eq!(detail_label(Lang::En, Some(DetailKind::Error), Some("rate_limit")).as_deref(), Some("rate_limit"), "エラーは種類の名前だけ");
    assert_eq!(detail_label(Lang::Ja, Some(DetailKind::NotMonitored), Some("VS Code")).as_deref(), Some("監視対象外 - VS Code"));
    assert_eq!(detail_label(Lang::En, Some(DetailKind::NotMonitored), None).as_deref(), Some("Not monitored - Terminal"));
    assert_eq!(detail_label(Lang::En, None, None), None);
    assert_eq!(status_label(Lang::En, crate::state::Status::NeedsAction), "Needs action");
}
```

- [ ] **Step 2: 通知の文面のテストを書き換える**（`crates/core/src/notify.rs`）

`change` の補助関数を `fn change(from: Option<Status>, to: Status, kind: Option<DetailKind>, text: Option<&str>) -> StatusChange` に変え、既存の 3 件を次に置き換える。

```rust
#[test]
fn permission_notice_splits_title_and_body() {
    let c = change(Some(Status::Running), Status::NeedsAction, Some(DetailKind::Permission), Some("Bash: rm -rf dist"));
    assert_eq!(notice_for(&c, &DEFAULT, Lang::Ja).unwrap(), Notice { title: "my-api — 許可待ち".into(), body: "Bash: rm -rf dist".into() });
    assert_eq!(notice_for(&c, &DEFAULT, Lang::En).unwrap(), Notice { title: "my-api — Waiting for permission".into(), body: "Bash: rm -rf dist".into() });
}

#[test]
fn question_notice_without_body() {
    let c = change(Some(Status::Running), Status::NeedsAction, Some(DetailKind::Question), None);
    assert_eq!(notice_for(&c, &DEFAULT, Lang::Ja).unwrap(), Notice { title: "my-api — 質問待ち".into(), body: String::new() });
}

#[test]
fn error_notice_uses_status_label() {
    let c = change(Some(Status::Running), Status::Error, Some(DetailKind::Error), Some("rate_limit"));
    assert_eq!(notice_for(&c, &DEFAULT, Lang::En).unwrap(), Notice { title: "my-api — Error".into(), body: "rate_limit".into() });
    assert_eq!(notice_for(&c, &DEFAULT, Lang::Ja).unwrap(), Notice { title: "my-api — エラー停止".into(), body: "rate_limit".into() });
}
```

残りの notify のテスト（同じ状態の繰り返しは通知しない、など）は、呼び出しに `Lang::Ja` を足し、`change` の引数を新しい形に直すだけにする。

- [ ] **Step 3: v0.2 の state.json を読めるテストを書く**（`crates/core/src/persist.rs` のテストに足す）

```rust
#[test]
fn v02_state_with_string_detail_still_loads() {
    let dir = tempfile::tempdir().unwrap();
    let old = r#"{"sessions":[{"id":"s1","status":"needs_action","detail":"許可待ち · Bash: ls","status_since_ms":1,"first_seen_ms":1,"last_event_ms":1,"cwd":"/w/api","config_dir":"/u/.claude","git_branch":null,"claude_pid":7,"terminal":null,"transcript_path":null,"active_subagents":0}]}"#;
    fs::write(dir.path().join("state.json"), old).unwrap();
    let store: SessionStore = load_json(&dir.path().join("state.json"));
    let session = &store.sessions()[0];
    assert_eq!(session.status, Status::NeedsAction);
    assert_eq!(session.detail_kind, None);
    assert_eq!(session.detail_text, None);
}
```

- [ ] **Step 4: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core`
Expected: コンパイルエラー（`DetailKind`・`detail_label` などがない）

- [ ] **Step 5: 実装する**

`crates/core/src/state.rs`:
- `Status::label` を消す
- 次を足す

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailKind {
    New,
    Permission,
    Question,
    PlanApproval,
    Input,
    Interrupted,
    Subagent,
    Compacting,
    Tool,
    Error,
    Background,
    NotMonitored,
}
```

- `Transition` を `{ status, detail_kind: Option<DetailKind>, detail_text: Option<String> }` にし、`transition_for` の各行を次の対応で書き換える（`to` は `|status, kind, text| Some(Transition { status, detail_kind: kind, detail_text: text })`）

| 今の詳細 | 種類 | 中身 |
|---|---|---|
| `"新規"` | `New` | なし |
| `"質問待ち"` | `Question` | なし |
| `"承認待ち"` | `PlanApproval` | なし |
| `join_label("許可待ち", tool_label)` | `Permission` | `tool_label` |
| `"許可待ち"`（`permission_prompt`） | `Permission` | なし |
| `"入力要求"` | `Input` | なし |
| PreToolUse の `tool_label` | `Tool`（`tool_label` が `None` なら種類も `None`） | `tool_label` |
| `"サブエージェント実行中"` | `Subagent` | なし |
| `"圧縮中"` | `Compacting` | なし |
| StopFailure の `error_type` | `Error`（`error_type` が `None` なら種類も `None`） | `error_type` |
| それ以外の `None` | `None` | なし |

`join_label` は使わなくなるので消す。

- `Session` の `detail: Option<String>` を `detail_kind: Option<DetailKind>` と `detail_text: Option<String>` に替える（`Option` なので、v0.2 の `state.json` にこの 2 項目がなくても `None` で読める。古い `detail` は serde が読み飛ばす）

`crates/core/src/ps.rs`:

```rust
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
```

テスト `tells_launch_origin_from_executable_path` は `LaunchOrigin::VsCode` などと比べる形にする。

`crates/core/src/store.rs`:
- `StatusChange` の `detail` を `detail_kind`・`detail_text` に替え、`apply` でそれを埋める
- 中断: `session.detail_kind = Some(DetailKind::Interrupted); session.detail_text = None;`
- `enter`: バックグラウンドのときは `(Some(DetailKind::Background), None)`、それ以外は `next` の 2 項目
- `mark_ended`: 2 項目とも `None`
- `unknown_process_session`: `detail_kind: Some(DetailKind::NotMonitored), detail_text: launch_origin(&row.args).editor_name().map(String::from)`
- テストの `status_of` は `(Status, Option<DetailKind>, Option<String>)` を返す形にし、`Some("中断".into())` などの比較を `Some(DetailKind::Interrupted), None` に、「監視対象外 - VS Code」を `Some(DetailKind::NotMonitored), Some("VS Code")` に直す

`crates/core/src/view.rs`:

```rust
use crate::messages::{text, Lang, Msg};
use crate::state::DetailKind;

pub fn status_label(lang: Lang, status: Status) -> String {
    let msg = match status {
        Status::NeedsAction => Msg::StatusNeedsAction,
        Status::Error => Msg::StatusError,
        Status::Done => Msg::StatusDone,
        Status::Running => Msg::StatusRunning,
        Status::Ended => Msg::StatusEnded,
        Status::Unknown => Msg::StatusUnknown,
    };
    text(lang, msg)
}

pub fn detail_label(lang: Lang, kind: Option<DetailKind>, detail_text: Option<&str>) -> Option<String> {
    let head = match kind? {
        DetailKind::Tool | DetailKind::Error => return detail_text.map(String::from),
        DetailKind::NotMonitored => {
            let origin = detail_text.map(String::from).unwrap_or_else(|| text(lang, Msg::DetailTerminal));
            return Some(format!("{} - {origin}", text(lang, Msg::DetailNotMonitored)));
        }
        DetailKind::New => Msg::DetailNew,
        DetailKind::Permission => Msg::DetailPermission,
        DetailKind::Question => Msg::DetailQuestion,
        DetailKind::PlanApproval => Msg::DetailPlanApproval,
        DetailKind::Input => Msg::DetailInput,
        DetailKind::Interrupted => Msg::DetailInterrupted,
        DetailKind::Subagent => Msg::DetailSubagent,
        DetailKind::Compacting => Msg::DetailCompacting,
        DetailKind::Background => Msg::DetailBackground,
    };
    let head = text(lang, head);
    Some(match detail_text {
        Some(rest) => format!("{head} · {rest}"),
        None => head,
    })
}
```

`session_views` に `lang: Lang` を足し、`SessionView.detail` を `detail_label(lang, session.detail_kind, session.detail_text.as_deref())` で埋める。既存のテストの呼び出しには `Lang::Ja` を足す。

`crates/core/src/notify.rs`:

```rust
pub fn notice_for(change: &StatusChange, notify_on: &[Status], lang: Lang) -> Option<Notice> {
    if change.from == Some(change.to) || !notify_on.contains(&change.to) {
        return None;
    }
    let (headline, body) = match (change.to, change.detail_kind) {
        // 要対応は「何を待っているか」が見出しで、ツールの中身は本文に回す
        (Status::NeedsAction, Some(kind)) if kind != DetailKind::Tool => (
            detail_label(lang, Some(kind), None).unwrap_or_default(),
            change.detail_text.clone().unwrap_or_default(),
        ),
        (status, kind) => (
            status_label(lang, status),
            detail_label(lang, kind, change.detail_text.as_deref()).unwrap_or_default(),
        ),
    };
    let title = messages::format(lang, Msg::NoticeTitle, &[("project", &change.project), ("headline", &headline)]);
    Some(Notice { title, body })
}
```

- [ ] **Step 6: 通ることを確かめる**

Run: `cargo test -p agents-monitor-core`
Expected: すべて PASS（`replay` の結合テストも含む。`fixtures_replay.rs` が詳細の文字列を比べていれば、種類で比べる形に直す）

- [ ] **Step 7: アプリ側の呼び出しを仮に合わせる**

`app/src-tauri` のうち、`session_views` と `notice_for` の呼び出しに `Lang::Ja` を渡してコンパイルを通す（Task 4 で起動時に決めた言語に替える）。`cargo clippy --workspace --all-targets` で警告なし。

- [ ] **Step 8: コミット**

```bash
git add crates app/src-tauri/src
git commit -m "状態の詳細を種類と中身に分け、一覧と通知の文面を言語に合わせて作る"
```

---

### Task 3: `claude` の実行と導入状態のエラーを種類で返す（core）

**Files:**
- Modify: `crates/core/src/claude_cli.rs`, `crates/core/src/plugin_status.rs`, `crates/core/tests/integration_install.rs`

**Interfaces:**
- Consumes: `messages`（Task 1）
- Produces:
  - `claude_cli::CliError { Failed(String), Timeout, Spawn(String), Wait(String), Unreadable }`、`CliError::message(&self, lang: Lang) -> String`
  - `ClaudeCli::run(...) -> Result<String, CliError>`
  - `claude_cli::failure_reason(stderr: &str, stdout: &str) -> Option<String>`（理由がなければ `None`）
  - `plugin_status::parse_inspection(...) -> Result<Inspection, CliError>`（読めなければ `CliError::Unreadable`）

- [ ] **Step 1: テストを書き換える**（`claude_cli.rs`）

- `failure_reports_last_stderr_line`: `assert_eq!(error, CliError::Failed("blocked by policy".into()));`
- `times_out_and_kills_the_process`: `assert_eq!(error, CliError::Timeout);`
- `reason_is_trimmed_to_200_chars_and_falls_back_to_stdout`: 最後を `assert_eq!(failure_reason("", ""), None);` にし、上の 2 つは `.unwrap()` を付ける
- 足す:

```rust
#[test]
fn error_messages_follow_the_language() {
    use crate::messages::Lang;
    assert_eq!(CliError::Timeout.message(Lang::En), "Timed out (no response)");
    assert_eq!(CliError::Failed("blocked".into()).message(Lang::Ja), "blocked", "claude 自身の文は訳さない");
    assert_eq!(CliError::Failed(String::new()).message(Lang::En), "Unknown reason");
    assert_eq!(CliError::Unreadable.message(Lang::Ja), "導入状態を読めませんでした");
}
```

`plugin_status.rs` の `broken_or_unexpected_output_is_an_error_not_a_panic` は `CliError::Unreadable` と比べる形にする。

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core claude_cli:: plugin_status::` は 1 つずつ（`cargo test -p agents-monitor-core claude_cli` と `… plugin_status`）
Expected: コンパイルエラー（`CliError` がない）

- [ ] **Step 3: 実装する**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    Failed(String),
    Timeout,
    Spawn(String),
    Wait(String),
    Unreadable,
}

impl CliError {
    pub fn message(&self, lang: Lang) -> String {
        match self {
            CliError::Failed(reason) if !reason.is_empty() => reason.clone(),
            CliError::Failed(_) => messages::text(lang, Msg::ErrorUnknownReason),
            CliError::Timeout => messages::text(lang, Msg::ErrorTimeout),
            CliError::Spawn(error) => messages::format(lang, Msg::ErrorCliSpawn, &[("error", error)]),
            CliError::Wait(error) => messages::format(lang, Msg::ErrorCliWait, &[("error", error)]),
            CliError::Unreadable => messages::text(lang, Msg::ErrorUnreadableStatus),
        }
    }
}
```

`failure_reason` は `Option<String>` を返し、`run` の失敗は `CliError::Failed(failure_reason(&stderr, &stdout).unwrap_or_default())` にする。起動の失敗は `CliError::Spawn(error.to_string())`、待てなかったら `CliError::Wait(error.to_string())`、時間切れは `CliError::Timeout`。`plugin_status::UNREADABLE` を消し、`map_err(|_| CliError::Unreadable)` にする。

`crates/core/tests/integration_install.rs` は `.unwrap()` のままで型が合う（`CliError` は `Debug`）。

- [ ] **Step 4: 通ることを確かめ、アプリの呼び出しを合わせる**

Run: `cargo test -p agents-monitor-core`
Expected: PASS

`app/src-tauri/src/installer.rs` の `apply`・`run_plan`・`inspect` を `Result<_, CliError>` に合わせる。`marketplace_error` は `Option<String>`（`io::Error` の文字列）のまま持ち、`apply` が返すエラーを次の列挙型にまとめる。

```rust
enum InstallError {
    Cli(CliError),
    ClaudeNotFound,
    MarketplaceUnavailable(String),
}

impl InstallError {
    fn message(&self, lang: Lang) -> String {
        match self {
            InstallError::Cli(error) => error.message(lang),
            InstallError::ClaudeNotFound => messages::text(lang, Msg::ErrorClaudeNotFound),
            InstallError::MarketplaceUnavailable(error) => messages::format(lang, Msg::ErrorMarketplaceUnavailable, &[("error", error)]),
        }
    }
}
```

アラートの文面は `messages::format(lang, Msg::ErrorInstallFailed, &[("dir", &display(path)), ("reason", &error.message(lang))])`（削除は `ErrorRemoveFailed`）。言語は Task 4 で `AppState` に持たせるので、ここでは `Lang::Ja` を仮に使う。

Run: `cargo clippy --workspace --all-targets && ./scripts/integration-test.sh`
Expected: 警告なし、結合テスト PASS

- [ ] **Step 5: コミット**

```bash
git add crates app/src-tauri/src
git commit -m "claude の実行と導入状態のエラーを種類で返し、表示するときに言語に合わせて文面にする"
```

---

### Task 4: 起動時に言語を決め、アプリの文面と画面への受け渡しに使う（アプリ）

**Files:**
- Create: `app/src-tauri/src/lang.rs`, `app/src-tauri/Info.plist`
- Modify: `app/src-tauri/src/main.rs`, `monitor.rs`, `installer.rs`, `commands.rs`, `app/src-tauri/Cargo.toml`（objc2-foundation の features に `NSArray`）

**Interfaces:**
- Consumes: `messages`（Task 1）、Task 2・3 の言語つきの関数
- Produces: `lang::detect() -> Lang`、`AppState.lang: Lang`、コマンド `get_messages() -> HashMap<String, String>`（今の言語の表）

- [ ] **Step 1: 言語の決め方を書く**（`app/src-tauri/src/lang.rs`）

```rust
use agents_monitor_core::messages::Lang;

pub fn detect() -> Lang {
    preferred_localization().or_else(|| std::env::var("LANG").ok()).map(|tag| Lang::from_tag(&tag)).unwrap_or(Lang::En)
}

// macOS の優先言語と「アプリごとの言語」の両方を、アプリが宣言した言語（Info.plist）の中から選んだ結果
#[cfg(target_os = "macos")]
fn preferred_localization() -> Option<String> {
    use objc2_foundation::NSBundle;
    let bundle = NSBundle::mainBundle();
    bundle.bundleIdentifier()?;
    bundle.preferredLocalizations().firstObject().map(|tag| tag.to_string())
}

#[cfg(not(target_os = "macos"))]
fn preferred_localization() -> Option<String> {
    None
}
```

`objc2-foundation` の features に `"NSArray"` を足す（`preferredLocalizations` が `NSArray<NSString>` を返すため）。メソッド名が合わなければ `~/.cargo/registry/src/*/objc2-foundation-0.3.*/src/generated/NSBundle.rs` を読んで合わせる。

- [ ] **Step 2: Info.plist で言語を宣言する**（`app/src-tauri/Info.plist`。Tauri はビルド時にこのファイルを生成する Info.plist にマージする）

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleLocalizations</key>
  <array>
    <string>en</string>
    <string>ja</string>
  </array>
</dict>
</plist>
```

- [ ] **Step 3: 言語を `AppState` に持たせ、すべての文面に使う**

- `AppState` に `pub lang: Lang` を足し、`load_state` で `lang::detect()` を入れる。`main.rs` に `mod lang;`
- Task 2・3 で仮に `Lang::Ja` を渡した箇所を、`state.lang` に替える（`session_views`・`notice_for`・installer のアラート・`InstallError::message`）
- `commands.rs` の利用者に見えるエラー（`error.autostart`・`error.save_settings`・`error.save_order`・`error.claude_path_invalid`）と、フォルダーを選ぶ画面の題（`dialog.choose_config_dir`）を `messages` で引く
- 次のコマンドを足して `generate_handler!` に登録する

```rust
#[tauri::command]
pub fn get_messages(state: State<'_, AppState>) -> std::collections::HashMap<String, String> {
    messages::table(state.lang).clone()
}
```

- [ ] **Step 4: ログを英語にする**

`app/src-tauri/src` と `crates` の `eprintln!` と `expect(...)` の文を英語にする（翻訳しない）。対応:

| 今 | 英語 |
|---|---|
| ホームディレクトリが見つかりません | home directory not found |
| {} を作成できませんでした: {error} | failed to create {}: {error} |
| agents-monitor-hook の配置に失敗しました: {error} | failed to install agents-monitor-hook: {error} |
| 読めない、または上限超過のイベントファイルを {} 件捨てました | dropped {} unreadable or excess event files |
| 状態の保存に失敗しました: {error} | failed to save state: {error} |
| 並び順の保存に失敗しました: {error} | failed to save session order: {error} |
| 設定の保存に失敗しました: {error} | failed to save settings: {error} |
| アプリのバンドルとして起動していないため、通知は送りません | not running as an app bundle; notifications are disabled |
| 通知の許可を求められませんでした: {} | failed to request notification permission: {} |
| 通知が許可されていません | notifications are not allowed |
| 通知を送れませんでした: {} | failed to send notification: {} |
| システム設定を開けませんでした: {error} | failed to open System Settings: {error} |
| 埋め込んだメニューバーのアイコンが読めません | embedded menu bar icon is unreadable |
| 埋め込んだ対応表が読めません | embedded message table is unreadable |

ログ以外のコメント（`//`）は日本語のままでよい（コードの説明でユーザーには出ない）。

- [ ] **Step 5: テストとビルド、実装の最初に確かめること**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets && ./scripts/build-hook-sidecar.sh && (cd app && npm run tauri build -- --bundles app)`
確かめること（設計書 5 章）:
- `plutil -p target/release/bundle/macos/agents-monitor.app/Contents/Info.plist | grep -A3 CFBundleLocalizations` で `en`・`ja` が入っている
- パッケージ版を起動し、日本語の環境で通知とアラートが日本語になる。`defaults write io.github.imatomix.agentsmonitor AppleLanguages '("en")'` で英語にして起動し直し、英語になる（確かめたら `defaults delete io.github.imatomix.agentsmonitor AppleLanguages`）
- 英語にならなければ `ja.lproj`・`en.lproj` の空のフォルダーを `bundle.resources` で同梱して試し、結果を台帳に書く

- [ ] **Step 6: コミット**

```bash
git add app/src-tauri crates Cargo.lock
git commit -m "起動時に macOS の言語を調べ、通知・アラート・一覧の文面と画面に渡す対応表に使う。ログは英語にする"
```

---

### Task 5: 画面の文言を対応表から引く（画面）

**Files:**
- Create: `app/src/i18n.ts`, `app/src-tauri/tests/messages_used.rs`
- Modify: `app/index.html`, `app/src/list.ts`, `app/src/settings.ts`, `app/src/format.ts`, `app/src/types.ts`

**Interfaces:**
- Consumes: コマンド `get_messages`（Task 4）
- Produces: `i18n.ts` の `loadMessages(): Promise<void>`、`t(key: string, args?: Record<string, string | number>): string`、`applyStaticTexts(root: ParentNode): void`

- [ ] **Step 1: 画面で使うキーが表にあることを確かめるテストを書く**（`app/src-tauri/tests/messages_used.rs`）

```rust
use std::fs;
use std::path::Path;

use agents_monitor_core::messages::{table, Lang};

fn keys_in(text: &str, open: &str, close: char) -> Vec<String> {
    text.match_indices(open).filter_map(|(i, _)| {
        let rest = &text[i + open.len()..];
        rest.find(close).map(|end| rest[..end].to_string())
    }).collect()
}

#[test]
fn every_key_used_by_the_page_exists() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut used = Vec::new();
    let html = fs::read_to_string(app.join("index.html")).unwrap();
    used.extend(keys_in(&html, "data-i18n=\"", '"'));
    used.extend(keys_in(&html, "data-i18n-title=\"", '"'));
    for entry in fs::read_dir(app.join("src")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "ts") {
            used.extend(keys_in(&fs::read_to_string(&path).unwrap(), "t(\"", '"'));
        }
    }
    assert!(used.len() > 10, "キーを拾えていない: {used:?}");
    for key in used {
        assert!(table(Lang::Ja).contains_key(&key), "{key} が対応表にない");
    }
}
```

`app/src-tauri/Cargo.toml` に `agents-monitor-core` はすでにあるので追加はいらない。

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-app --test messages_used`
Expected: FAIL（「キーを拾えていない」。画面はまだ `t()` を使っていない）

- [ ] **Step 3: `i18n.ts` を書く**

```ts
import { invoke } from "@tauri-apps/api/core";

let messages: Record<string, string> = {};

export async function loadMessages(): Promise<void> {
  messages = await invoke<Record<string, string>>("get_messages");
  document.documentElement.lang = messages["status.done"] === "完了" ? "ja" : "en";
}

export function t(key: string, args: Record<string, string | number> = {}): string {
  const template = messages[key] ?? key;
  return Object.entries(args).reduce((text, [name, value]) => text.replaceAll(`{${name}}`, String(value)), template);
}

// HTML に書いた固定の文言を差し替える。文言は textContent で入れ、HTML として解釈させない
export function applyStaticTexts(root: ParentNode): void {
  for (const node of root.querySelectorAll<HTMLElement>("[data-i18n]")) node.textContent = t(node.dataset.i18n!);
  for (const node of root.querySelectorAll<HTMLElement>("[data-i18n-title]")) {
    const label = t(node.dataset.i18nTitle!);
    node.title = label;
    node.setAttribute("aria-label", label);
  }
}
```

- [ ] **Step 4: 文言を置き換える**

`app/index.html`（日本語の文字を `data-i18n` に。中身は空でよいが、読み込み前の一瞬に備えて日本語のまま残してもよい）:

| 箇所 | 属性 |
|---|---|
| メニューのボタン `aria-label="メニュー"` | `data-i18n-title="tab.menu"`（`aria-label` は消す） |
| 「通知設定」の見出し | `data-i18n="settings.notifications"` |
| 「通知がオフになっています」の span | `data-i18n="settings.notifications_off"` |
| 歯車のボタン | `data-i18n-title="settings.open_system_settings"`（`title`・`aria-label` は消す） |
| 「Claude Code の設定」の見出し | `data-i18n="settings.claude_configs"` |
| 見つからないときの p | `data-i18n="settings.no_config_dirs"` |
| claude が見つからないときの p | `data-i18n="settings.claude_not_found"` |
| 「＋ 設定ディレクトリを追加」 | `data-i18n="settings.add_config_dir"` |
| 「ログイン時に起動する」 | 文字の部分を `<span data-i18n="settings.launch_at_login"></span>` で包む |
| 「終了する」 | 文字の部分を `<span data-i18n="settings.quit"></span>` で包む |

`app/src/types.ts`: `STATUS_LABEL` を消す。

`app/src/format.ts`:

```ts
import { t } from "./i18n";

export function formatElapsed(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  if (seconds < 60) return t("elapsed.seconds", { n: seconds });
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return t("elapsed.minutes", { n: minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t("elapsed.hours", { n: hours });
  return t("elapsed.days", { n: Math.floor(hours / 24) });
}
```

`app/src/list.ts`:
- `STATUS_LABEL[session.status]` を `` t(`status.${session.status}`) `` にする（キーが動的なので Step 1 のテストでは拾えないが、`status.*` は Rust の `Msg` で両言語にあることを確かめている）
- `場所: ${…}` を `t("details.location", { path: session.cwd || t("details.unknown_value") })`、プロフィールを `t("details.profile", { name: profileName(session.config_dir) || t("details.unknown_value") })`、状態不明の説明を `t("details.not_monitored_help")`、空のときを `t("list.empty")` にする
- 最初の描画の前に表を受け取る: ファイルの最後の初期化（`setupSettingsMenu()` 以降、`listen`・`invoke("get_sessions")`・`setInterval`）を `async function start()` にまとめ、先頭で `await loadMessages(); applyStaticTexts(document);` を呼んでから進める。`void start();` で始める

`app/src/settings.ts`:
- `STATUS_LABEL[status]` を `` t(`status.${status}`) ``
- `iconButton(..., "もう一度", ...)` を `t("settings.retry")`、`"削除"` を `t("settings.remove")`

- [ ] **Step 5: 通ることを確かめる**

Run: `cargo test -p agents-monitor-app --test messages_used && (cd app && npm run build) && grep -rn "[ぁ-んァ-ン一-龥]" app/src/*.ts | grep -v "^\s*//" | grep -v "// "`
Expected: テスト PASS、ビルド成功、`grep` はコメント以外の日本語を出さない

- [ ] **Step 6: コミット**

```bash
git add app/index.html app/src app/src-tauri/tests/messages_used.rs
git commit -m "画面の文言を対応表から引き、最初の描画の前に今の言語の表を受け取る"
```

---

### Task 6: プラグインの説明、README の英語版、仕上げの確認

**Files:**
- Modify: `plugin/.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json`, `README.md`
- Create: `README.en.md`

- [ ] **Step 1: プラグインの説明を英語にする**

両方の `description` を `"Hooks that report Claude Code session status to the agents-monitor menu bar app"` にする。プラグインの版（`plugin.json` の `version`）は上げない（hooks の中身は変わらないため）。

Run: `cargo test -p agents-monitor-hook --test plugin_manifest`
Expected: PASS

- [ ] **Step 2: README の英語版を書く**

`README.en.md` に、今の `README.md` と同じ節（概要、状態の見方、導入、既知の制約、保存するデータ、開発、ライセンス）を英語で書く。状態の名前は設計書 3 章の英語に合わせる。両方の冒頭に言語のリンクを置く。

`README.md` の 1 行目の下: `[English](README.en.md)`
`README.en.md` の 1 行目の下: `[日本語](README.md)`

- [ ] **Step 3: すべてのテストと手動確認**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets && (cd app && npm run build) && ./scripts/integration-test.sh`
Expected: すべて PASS

パッケージ版で（設計書 4 章の実機確認）:
- 日本語の環境で、一覧・設定・通知・アラート（存在しないフォルダーを追加して失敗させる、など）が日本語
- `defaults write io.github.imatomix.agentsmonitor AppleLanguages '("en")'` で起動し直すと、すべて英語。経過時間が `3m` の形
- v0.2 のときの `state.json` が残った状態で起動しても落ちない（Task 2 のテストで確かめているが、実物でも見る）
- 確かめ終えたら `defaults delete io.github.imatomix.agentsmonitor AppleLanguages`

- [ ] **Step 4: コミット**

```bash
git add plugin/.claude-plugin/plugin.json .claude-plugin/marketplace.json README.md README.en.md
git commit -m "プラグインの説明を英語にし、英語の README を足す"
```
