# agents-monitor v0.2 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 起動するだけで `~/.claude` にプラグインが入り、ほかの設定ディレクトリは設定画面から追加・削除でき、一覧の行をドラッグで並び替えられる v0.2 を作る。

**Architecture:** 判断（CLI の出力の読み取り、実行する手順、並べ方、バッジを出すか）はすべて `agents-monitor-core` の純粋な関数にしてユニットテストで確かめる。`claude` の実行も core の小さなランナーに置き、アプリはそれを裏の 1 スレッドで順に呼ぶだけにする。画面は既存の一覧ウィンドウの設定ページに足し、見た目は Figma の「v0.2 下書き/修正案」に合わせる。

**Tech Stack:** Rust（workspace）、serde / serde_json、dirs、tempfile（テストのみ）、Tauri 2（tray-icon、image-png）、tauri-plugin-autostart / positioner、**tauri-plugin-dialog 2（v0.2 で追加。2026-10-11 に了承済み）**、objc2 / objc2-user-notifications、Vite + 素の TypeScript

**Spec:** `docs/superpowers/specs/2026-10-11-agents-monitor-v0.2-design.md`（元の設計書 `docs/superpowers/specs/2026-10-09-agents-monitor-design.md` も参照する）

## Global Constraints

- 起動時にユーザーへ何かを選ばせない。初回セットアップ画面は作らない
- 自動でプラグインを入れるのは、`config_dirs` が「なし」のときの `~/.claude` だけ。`~/.claude-*` は探さない
- Claude Code の設定ファイル（`~/.claude/settings.json` など）を直接書き換えない。変更は `claude plugin …` だけで行う
- `claude` の呼び出しは裏の 1 スレッドで 1 件ずつ。1 件 60 秒で打ち切る
- 失敗の理由は、エラー出力の最後の空でない行を 200 字まで。保存しない
- 並び順はセッション ID の並び `session_order`。500 件を超えたら、表示していないものを並びの先頭側から消す
- 画面の文字色に状態色（`--needs-action`・`--error`・`--done`・`--running`）を使わない。文字は `--fg` と `--fg-secondary` だけ。色は状態の丸とバッジだけに使う
- アイコンだけのボタンには、`title`（ツールチップ）と `aria-label` に同じ文言を付ける
- 画面に出す任意の文字列（パス、エラー）は `textContent` で入れ、HTML として解釈させない
- 承認済み以外のライブラリを追加しない（追加が必要になったら作業を止めて確認する）
- 対象は macOS のみ。コミットメッセージは日本語。コメントは WHY が非自明なときだけ書く
- v0.2 の範囲外: 画面移動、通知クリック時の移動、署名・公証・自動アップデート

## Review Focus

1. **設定画面からの保存が、裏で更新した項目を古い値で上書きする**（画面は `notify_on` と `launch_at_login` しか知らないのに、設定全体を送り返すと `config_dirs` や `session_order` が消える）→ 画面から保存できるのはその 2 つだけで、ほかの項目は残る（Task 2 でテスト）
2. **フォルダーを選ぶ画面やアラートを開くと、一覧ウィンドウがフォーカスを失って閉じる**→ ダイアログを出している間は閉じず、閉じた後も設定画面が見えている（Task 8 の手動確認）
3. **プラグインの中身に新しいファイルを足したのに、`.app` に同梱し忘れる**→ テストで `plugin/` 以下のすべてのファイルが `tauri.conf.json` の `bundle.resources` にあることを確かめる（Task 7 でテスト）
4. **`claude` の出力が版の違いで変わる・空・壊れている**→ 落ちずに、そのディレクトリを「入っていない」にして理由を「導入状態を読めませんでした」とする（Task 6 でテスト）
5. **ドラッグ中に 1 秒ごとの再描画や `sessions-updated` が来る**→ ドラッグ中の行が消えたり位置が飛んだりしない。離した後に最新の状態で描き直す（Task 4 の実装と手動確認）

---

## ファイル構成

| ファイル | 役割 | 新規/変更 |
|---|---|---|
| `crates/core/src/settings.rs` | アプリの設定。`config_dirs`・`claude_path`・`session_order` と、画面から変えられる `Preferences` | 変更 |
| `crates/core/src/order.rs` | 並べ方（`arrange`）、並びの移動（`move_before`）、上限 | 新規 |
| `crates/core/src/view.rs` | `session_views` が並び順を受け取り、更新した並びも返す | 変更 |
| `crates/core/src/config_dirs.rs` | `config_dirs` の初期値、表示用のパス（`~` に置き換え） | 新規 |
| `crates/core/src/claude_cli.rs` | `claude` の場所を探す、時間制限つきで実行する、失敗の理由を取り出す | 新規 |
| `crates/core/src/plugin_status.rs` | `plugin list --json` と `marketplace list --json` の読み取り、ディレクトリの状態 | 新規 |
| `crates/core/src/install_plan.rs` | 入れる・更新する・外すときの `claude plugin …` の手順 | 新規 |
| `crates/core/src/marketplace.rs` | 同梱マーケットプレイスを `~/.agents-monitor/marketplace` へ写す、同梱プラグインの版 | 新規 |
| `crates/core/src/attention.rs` | 設定アイコンにバッジを付けるかどうか | 新規 |
| `crates/core/src/paths.rs` | `marketplace_dir` を足す | 変更 |
| `crates/core/tests/fixtures/cli/*.json` | `claude` 2.1.296 の実際の出力 | 新規 |
| `crates/core/tests/integration_install.rs` | 本物の `claude` を使う結合テスト（`#[ignore]`） | 新規 |
| `scripts/integration-test.sh` | 結合テストを動かす | 新規 |
| `app/src-tauri/src/installer.rs` | 裏のスレッドでの導入・更新・削除、状態を画面へ送る、アラート | 新規 |
| `app/src-tauri/src/notifier.rs` | 通知の許可状態を調べる、システム設定を開く | 変更 |
| `app/src-tauri/src/monitor.rs` | 並び順つきの一覧、`AppState` に導入の状態を足す | 変更 |
| `app/src-tauri/src/commands.rs` | 新しいコマンド | 変更 |
| `app/src-tauri/src/tray.rs` | ダイアログ中は一覧を閉じない、設定画面で開く | 変更 |
| `app/src-tauri/src/main.rs` | dialog プラグイン、installer の起動 | 変更 |
| `app/src-tauri/tauri.conf.json` | `bundle.resources` に同梱マーケットプレイス | 変更 |
| `app/src-tauri/tests/bundle_resources.rs` | 同梱し忘れを防ぐテスト | 新規 |
| `app/index.html`, `app/src/settings.ts`, `app/src/list.ts`, `app/src/drag.ts`, `app/src/icons.ts`, `app/src/types.ts`, `app/src/style.css` | 画面 | 変更・新規 |
| `docs/superpowers/notes/2026-10-11-v0.2-cli-findings.md` | 実装の最初に確かめたことの記録 | 新規 |
| `README.md` | 導入手順を v0.2 に合わせる | 変更 |

---

### Task 1: 実装の最初に確かめること（設計書 10 章）と、CLI の出力の採取

コードは書かない。結果を記録し、後のタスクのテストデータを採る。API は使わない（`claude -p` は動かさない）。

**Files:**
- Create: `docs/superpowers/notes/2026-10-11-v0.2-cli-findings.md`
- Create: `crates/core/tests/fixtures/cli/plugin-list-empty.json`, `plugin-list-bundled.json`, `plugin-list-other.json`, `plugin-list-disabled.json`, `marketplace-list-empty.json`, `marketplace-list-bundled.json`, `marketplace-list-other.json`

**Interfaces:**
- Produces: 上のテストデータ（Task 6 が読む）と、調べた結果（Task 8 の更新手順に効く）

- [ ] **Step 1: 一時的な設定ディレクトリと、同梱マーケットプレイスの写しを作る**

```bash
WORK=$(mktemp -d)
mkdir -p "$WORK/marketplace" "$WORK/config"
cp -R .claude-plugin plugin "$WORK/marketplace/"
export CLAUDE_CONFIG_DIR="$WORK/config"
claude plugin list --json > crates/core/tests/fixtures/cli/plugin-list-empty.json
claude plugin marketplace list --json > crates/core/tests/fixtures/cli/marketplace-list-empty.json
```

- [ ] **Step 2: 入れて、出力を採る**

```bash
claude plugin marketplace add "$WORK/marketplace"
claude plugin install agents-monitor@agents-monitor
claude plugin list --json > crates/core/tests/fixtures/cli/plugin-list-bundled.json
claude plugin marketplace list --json > crates/core/tests/fixtures/cli/marketplace-list-bundled.json
```

採ったファイルの中の `$WORK` の部分（例 `/var/folders/.../tmp.XXXX`）は、すべて `/Users/tester/.agents-monitor` に置き換えて保存する（テストで同梱先のパスとして使う。マーケットプレイスの `path` は `/Users/tester/.agents-monitor/marketplace` になる）。`installedAt` などの日時はそのまま残してよい。

- [ ] **Step 3: 写しを書き換えたときの反映を確かめる**

`$WORK/marketplace/plugin/.claude-plugin/plugin.json` の `version` を `0.1.1` に変え、`claude plugin list --json` の `version` と `folderVersion` を見る。次に `claude plugin marketplace update agents-monitor` → `claude plugin update agents-monitor@agents-monitor` を実行してもう一度見る。どの時点で `version` が `0.1.1` になったかを記録する。

- [ ] **Step 4: 無効にしたときの出力を採る**

```bash
claude plugin disable agents-monitor@agents-monitor
claude plugin list --json > crates/core/tests/fixtures/cli/plugin-list-disabled.json
claude plugin enable agents-monitor@agents-monitor
```

- [ ] **Step 5: 外したときの痕跡を確かめる**

```bash
claude plugin uninstall agents-monitor@agents-monitor
claude plugin marketplace remove agents-monitor
claude plugin list --json; claude plugin marketplace list --json
grep -rl "agents-monitor" "$WORK/config" || echo "痕跡なし"
```

- [ ] **Step 6: 別の場所から入っているときの出力を採る**

開発者の実際の環境（`~/.claude-personal`、開発中のリポジトリを指す）で読むだけ。書き換えはしない。

```bash
CLAUDE_CONFIG_DIR=~/.claude-personal claude plugin list --json > crates/core/tests/fixtures/cli/plugin-list-other.json
CLAUDE_CONFIG_DIR=~/.claude-personal claude plugin marketplace list --json > crates/core/tests/fixtures/cli/marketplace-list-other.json
```

ホームのパスは `/Users/tester` に置き換えて保存する。

- [ ] **Step 7: 一時的な設定ディレクトリでの変更が、ほかに影響しないことを確かめる**

Step 2〜5 の前後で `claude plugin list --json`（`CLAUDE_CONFIG_DIR` なし、および `~/.claude-personal`）の出力が変わっていないことを確かめる。

- [ ] **Step 8: 記録を書く**

`docs/superpowers/notes/2026-10-11-v0.2-cli-findings.md` に、Step 3・5・7 の結果と、出力の形（項目名）を書く。Step 3 で「写し直すだけで反映される」とわかった場合は、Task 6 の `update_steps` の手順を `[PluginUpdate]` だけにしてよいかを記録に書き、Task 6 の実装でそれに従う。設計書 10 章の残りの項目（通知の許可の画面、ダイアログを開いたときの一覧ウィンドウ）は Task 8・9 の手動確認で記録に追記する。

- [ ] **Step 9: コミット**

```bash
git add docs/superpowers/notes/2026-10-11-v0.2-cli-findings.md crates/core/tests/fixtures/cli
git commit -m "v0.2 の実装の前に claude plugin の動きを確かめ、出力をテストデータとして残す"
```

---

### Task 2: アプリの設定に項目を足し、画面からの保存で消えないようにする

**Files:**
- Modify: `crates/core/src/settings.rs`
- Modify: `crates/core/src/persist.rs`（テストの `Settings` の作り方）
- Modify: `app/src-tauri/src/commands.rs`
- Modify: `app/src/types.ts`, `app/src/settings.ts`

**Interfaces:**
- Produces: `Settings { notify_on, launch_at_login, config_dirs: Option<Vec<String>>, claude_path: Option<String>, session_order: Vec<String> }`、`Preferences { notify_on: Vec<Status>, launch_at_login: bool }`、`Settings::preferences(&self) -> Preferences`、`Settings::apply_preferences(&mut self, Preferences)`。コマンド `get_settings() -> Preferences`、`save_settings(preferences: Preferences)`

- [ ] **Step 1: 失敗するテストを書く**（`crates/core/src/settings.rs` の末尾）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v01_settings_file_loads_with_new_fields_unset() {
        let loaded: Settings = serde_json::from_str(r#"{"notify_on":["done"],"launch_at_login":true}"#).unwrap();
        assert_eq!(loaded.notify_on, vec![Status::Done]);
        assert!(loaded.launch_at_login);
        assert_eq!(loaded.config_dirs, None);
        assert_eq!(loaded.claude_path, None);
        assert!(loaded.session_order.is_empty());
    }

    #[test]
    fn empty_config_dirs_is_distinct_from_unset() {
        let settings = Settings { config_dirs: Some(vec![]), ..Settings::default() };
        let json = serde_json::to_string(&settings).unwrap();
        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.config_dirs, Some(vec![]));
    }

    #[test]
    fn applying_preferences_keeps_other_fields() {
        let mut settings = Settings {
            config_dirs: Some(vec!["/u/.claude".into()]),
            claude_path: Some("/opt/claude".into()),
            session_order: vec!["s1".into()],
            ..Settings::default()
        };
        settings.apply_preferences(Preferences { notify_on: vec![Status::Done], launch_at_login: true });
        assert_eq!(settings.preferences(), Preferences { notify_on: vec![Status::Done], launch_at_login: true });
        assert_eq!(settings.config_dirs, Some(vec!["/u/.claude".into()]));
        assert_eq!(settings.claude_path.as_deref(), Some("/opt/claude"));
        assert_eq!(settings.session_order, vec!["s1".to_string()]);
    }
}
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core settings::`
Expected: コンパイルエラー（`config_dirs`・`Preferences` がない）

- [ ] **Step 3: 実装する**（`crates/core/src/settings.rs` 全体）

```rust
use serde::{Deserialize, Serialize};

use crate::state::Status;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub notify_on: Vec<Status>,
    pub launch_at_login: bool,
    // None は「まだ決めていない」。すべて削除した空の一覧と区別し、~/.claude を自動で入れるのは None のときだけにする
    pub config_dirs: Option<Vec<String>>,
    pub claude_path: Option<String>,
    pub session_order: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            notify_on: vec![Status::NeedsAction, Status::Error],
            launch_at_login: false,
            config_dirs: None,
            claude_path: None,
            session_order: Vec::new(),
        }
    }
}

// 画面から変えられるのはこの 2 つだけ。設定全体を送り返させると、裏で更新した config_dirs や session_order を古い値で上書きしてしまう
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    pub notify_on: Vec<Status>,
    pub launch_at_login: bool,
}

impl Settings {
    pub fn preferences(&self) -> Preferences {
        Preferences { notify_on: self.notify_on.clone(), launch_at_login: self.launch_at_login }
    }

    pub fn apply_preferences(&mut self, preferences: Preferences) {
        self.notify_on = preferences.notify_on;
        self.launch_at_login = preferences.launch_at_login;
    }
}
```

`crates/core/src/persist.rs` のテスト `round_trips_settings_and_store` の `Settings { notify_on: vec![Status::Done], launch_at_login: true }` を `Settings { notify_on: vec![Status::Done], launch_at_login: true, ..Settings::default() }` にする。

- [ ] **Step 4: コマンドと画面を `Preferences` に替える**

`app/src-tauri/src/commands.rs` の `get_settings` と `save_settings` を次にする（`use agents_monitor_core::settings::Settings;` は `use agents_monitor_core::settings::Preferences;` に替える）。

```rust
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Preferences {
    state.settings.lock().unwrap().preferences()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, state: State<'_, AppState>, settings: Preferences) -> Result<(), String> {
    let autostart = app.autolaunch();
    let result = if settings.launch_at_login { autostart.enable() } else { autostart.disable() };
    result.map_err(|error| format!("ログイン時の起動設定を変更できませんでした: {error}"))?;
    let mut current = state.settings.lock().unwrap();
    let mut next = current.clone();
    next.apply_preferences(settings);
    save_json(&paths::settings_path(&state.home), &next).map_err(|error| format!("設定を保存できませんでした: {error}"))?;
    *current = next;
    Ok(())
}
```

`app/src/types.ts` の `Settings` を `Preferences` に改名する（中身の 2 項目はそのまま）。`app/src/settings.ts` の `Settings` の参照をすべて `Preferences` にする。

- [ ] **Step 5: テストとビルドを確かめる**

Run: `cargo test --workspace && (cd app && npm run build)`
Expected: すべて PASS、型エラーなし

- [ ] **Step 6: コミット**

```bash
git add crates/core/src/settings.rs crates/core/src/persist.rs app/src-tauri/src/commands.rs app/src/types.ts app/src/settings.ts
git commit -m "アプリの設定に監視の対象・claude の場所・並び順を足し、画面からの保存で消えないようにする"
```

---

### Task 3: 並び順をセッション ID で覚える（core とアプリ）

**Files:**
- Create: `crates/core/src/order.rs`
- Modify: `crates/core/src/lib.rs`（`pub mod order;`）
- Modify: `crates/core/src/view.rs`
- Modify: `app/src-tauri/src/monitor.rs`, `app/src-tauri/src/commands.rs`, `app/src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `Settings.session_order`（Task 2）、`store::UNKNOWN_ID_PREFIX`
- Produces:
  - `order::MAX_ORDER_LEN: usize = 500`
  - `order::arrange<'a>(sessions: &[&'a Session], order: &[String]) -> (Vec<&'a Session>, Vec<String>)`
  - `order::move_before(order: &[String], id: &str, before: Option<&str>) -> Vec<String>`
  - `view::session_views(sessions: &[Session], order: &[String]) -> (Vec<SessionView>, Vec<String>)`
  - アプリ: `monitor::current_views(state: &AppState) -> Vec<SessionView>`、コマンド `move_session(id: String, before_id: Option<String>)`

- [ ] **Step 1: 失敗するテストを書く**（`crates/core/src/order.rs`）

```rust
use std::collections::{HashMap, HashSet};

use crate::state::Session;
use crate::store::UNKNOWN_ID_PREFIX;

pub const MAX_ORDER_LEN: usize = 500;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::test_support::ev;
    use crate::store::SessionStore;

    fn sessions(specs: &[(&str, &str)]) -> Vec<Session> {
        let mut store = SessionStore::default();
        for (i, (id, cwd)) in specs.iter().enumerate() {
            let mut e = ev("SessionStart", id, i as u64 + 1);
            e.cwd = cwd.to_string();
            store.apply(&e);
        }
        store.sessions().to_vec()
    }

    fn ids(arranged: &[&Session]) -> Vec<String> {
        arranged.iter().map(|s| s.id.clone()).collect()
    }

    #[test]
    fn follows_saved_order_and_keeps_hidden_entries() {
        let all = sessions(&[("a", "/w/api"), ("b", "/w/web")]);
        let refs: Vec<&Session> = all.iter().collect();
        let order = vec!["gone".to_string(), "b".into(), "a".into()];
        let (arranged, next) = arrange(&refs, &order);
        assert_eq!(ids(&arranged), ["b", "a"]);
        assert_eq!(next, order, "表示していない gone も後の再開のために残す");
    }

    #[test]
    fn new_session_goes_after_last_session_of_same_cwd() {
        let all = sessions(&[("a", "/w/api"), ("b", "/w/web"), ("c", "/w/api")]);
        let refs: Vec<&Session> = all.iter().collect();
        let (arranged, next) = arrange(&refs, &["a".into(), "b".into()]);
        assert_eq!(ids(&arranged), ["a", "c", "b"]);
        assert_eq!(next, ["a", "c", "b"]);
    }

    #[test]
    fn new_session_of_new_project_goes_last() {
        let all = sessions(&[("a", "/w/api"), ("b", "/w/web")]);
        let refs: Vec<&Session> = all.iter().collect();
        let (arranged, _) = arrange(&refs, &["a".into()]);
        assert_eq!(ids(&arranged), ["a", "b"]);
    }

    #[test]
    fn session_with_empty_cwd_does_not_join_others() {
        let all = sessions(&[("a", ""), ("b", "/w/web"), ("c", "")]);
        let refs: Vec<&Session> = all.iter().collect();
        let (arranged, _) = arrange(&refs, &["a".into(), "b".into()]);
        assert_eq!(ids(&arranged), ["a", "b", "c"]);
    }

    #[test]
    fn unknown_process_rows_stay_last_and_are_not_saved() {
        let mut all = sessions(&[("a", "/w/api")]);
        let mut unknown = all[0].clone();
        unknown.id = format!("{UNKNOWN_ID_PREFIX}42");
        all.insert(0, unknown);
        let refs: Vec<&Session> = all.iter().collect();
        let (arranged, next) = arrange(&refs, &[]);
        assert_eq!(ids(&arranged), ["a".to_string(), format!("{UNKNOWN_ID_PREFIX}42")]);
        assert_eq!(next, ["a"]);
    }

    #[test]
    fn trims_hidden_entries_from_the_front_beyond_the_limit() {
        let all = sessions(&[("shown", "/w/api")]);
        let refs: Vec<&Session> = all.iter().collect();
        let mut order: Vec<String> = (0..MAX_ORDER_LEN).map(|i| format!("old{i}")).collect();
        order.insert(0, "shown".into());
        let (_, next) = arrange(&refs, &order);
        assert_eq!(next.len(), MAX_ORDER_LEN);
        assert_eq!(next[0], "shown", "表示中のものは消さない");
        assert_eq!(next[1], "old1", "表示していないものは先頭側から消す");
    }

    #[test]
    fn moves_before_target_or_to_end() {
        let order: Vec<String> = ["a", "b", "c"].map(String::from).to_vec();
        assert_eq!(move_before(&order, "c", Some("a")), ["c", "a", "b"]);
        assert_eq!(move_before(&order, "a", None), ["b", "c", "a"]);
        assert_eq!(move_before(&order, "x", Some("b")), ["a", "x", "b", "c"], "並びにない ID も入れられる");
        assert_eq!(move_before(&order, "a", Some("a")), order, "自分の前に置いても変わらない");
    }
}
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core order::`
Expected: コンパイルエラー（`arrange`・`move_before` がない）。`crates/core/src/lib.rs` に `pub mod order;` を足してから実行する

- [ ] **Step 3: 実装する**（テストの上に書く）

```rust
pub fn arrange<'a>(sessions: &[&'a Session], order: &[String]) -> (Vec<&'a Session>, Vec<String>) {
    let (unknown, draggable): (Vec<&Session>, Vec<&Session>) =
        sessions.iter().copied().partition(|s| s.id.starts_with(UNKNOWN_ID_PREFIX));
    let by_id: HashMap<&str, &Session> = draggable.iter().map(|s| (s.id.as_str(), *s)).collect();
    let mut next: Vec<String> = order.to_vec();
    for session in &draggable {
        if next.iter().any(|id| id == &session.id) {
            continue;
        }
        let after_same_project = (!session.cwd.is_empty())
            .then(|| next.iter().rposition(|id| by_id.get(id.as_str()).is_some_and(|s| s.cwd == session.cwd)))
            .flatten();
        match after_same_project {
            Some(index) => next.insert(index + 1, session.id.clone()),
            None => next.push(session.id.clone()),
        }
    }
    trim_hidden(&mut next, &by_id.keys().copied().collect());
    let mut arranged: Vec<&Session> = next.iter().filter_map(|id| by_id.get(id.as_str()).copied()).collect();
    arranged.extend(unknown);
    (arranged, next)
}

// いつ加えたかは覚えていないので、古さではなく位置で決める
fn trim_hidden(order: &mut Vec<String>, shown: &HashSet<&str>) {
    let mut excess = order.len().saturating_sub(MAX_ORDER_LEN);
    order.retain(|id| {
        if excess > 0 && !shown.contains(id.as_str()) {
            excess -= 1;
            return false;
        }
        true
    });
}

pub fn move_before(order: &[String], id: &str, before: Option<&str>) -> Vec<String> {
    if before == Some(id) {
        return order.to_vec();
    }
    let mut next: Vec<String> = order.iter().filter(|existing| existing.as_str() != id).cloned().collect();
    let index = before.and_then(|target| next.iter().position(|existing| existing == target)).unwrap_or(next.len());
    next.insert(index, id.to_string());
    next
}
```

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test -p agents-monitor-core order::`
Expected: 7 件 PASS

- [ ] **Step 5: `session_views` を並び順つきにする**

`crates/core/src/view.rs` の `session_views` を次にする（`SessionView` の組み立て部分は今のまま）。

```rust
pub fn session_views(sessions: &[Session], order: &[String]) -> (Vec<SessionView>, Vec<String>) {
    let active: Vec<&Session> = sessions.iter().filter(|s| s.status != Status::Ended).collect();
    // 終了したセッションは遅れて届くイベントを捨てるために内部では残すが、画面には出さない
    let (arranged, next_order) = crate::order::arrange(&active, order);
    let views = arranged
        .into_iter()
        .map(|session| {
            let shares_project =
                !session.cwd.is_empty() && active.iter().filter(|other| other.cwd == session.cwd).count() > 1;
            SessionView {
                id: session.id.clone(),
                status: session.status,
                project: session.project(),
                detail: session.detail.clone(),
                status_since_ms: session.status_since_ms,
                branch: if shares_project { session.git_branch.clone() } else { None },
                cwd: session.cwd.clone(),
                config_dir: session.config_dir.clone(),
                claude_pid: session.claude_pid,
            }
        })
        .collect();
    (views, next_order)
}
```

同じファイルのテストの `session_views(store.sessions())` を `session_views(store.sessions(), &[]).0` にする。

- [ ] **Step 6: アプリで並び順を使い、保存する**

`app/src-tauri/src/monitor.rs` に次を足し、`publish` の `session_views(store.sessions())` を `current_views(&state)` に置き換える（`publish` は `store` のロックを先に外してから呼ぶ。`tray_status` は別に取る）。

```rust
pub fn current_views(state: &AppState) -> Vec<SessionView> {
    let store = state.store.lock().unwrap();
    let mut settings = state.settings.lock().unwrap();
    let (views, next_order) = session_views(store.sessions(), &settings.session_order);
    if next_order != settings.session_order {
        settings.session_order = next_order;
        if let Err(error) = save_json(&paths::settings_path(&state.home), &*settings) {
            eprintln!("並び順の保存に失敗しました: {error}");
        }
    }
    views
}
```

`publish` は次の形にする。

```rust
pub fn publish(app: &AppHandle) {
    let state = app.state::<AppState>();
    let views = current_views(&state);
    let tray_status = state.store.lock().unwrap().tray_status();
    let _ = app.emit("sessions-updated", &views);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        // set_icon だけだとテンプレート指定が外れ、明るいメニューバーで黒く表示されなくなる
        let _ = tray.set_icon_with_as_template(Some(tray_icon(tray_status)), true);
    }
}
```

ロックの順は「store → settings」に統一する。ほかの箇所で settings を持ったまま store を取らないこと。

`app/src-tauri/src/commands.rs` の `get_sessions` を `crate::monitor::current_views(&state)` を返す形にし、次のコマンドを足して `main.rs` の `generate_handler!` に登録する。

```rust
#[tauri::command]
pub fn move_session(app: AppHandle, state: State<'_, AppState>, id: String, before_id: Option<String>) -> Result<(), String> {
    {
        let mut settings = state.settings.lock().unwrap();
        let next = move_before(&settings.session_order, &id, before_id.as_deref());
        let mut updated = settings.clone();
        updated.session_order = next;
        save_json(&paths::settings_path(&state.home), &updated).map_err(|error| format!("並び順を保存できませんでした: {error}"))?;
        *settings = updated;
    }
    crate::monitor::publish(&app);
    Ok(())
}
```

- [ ] **Step 7: テストとビルド**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets`
Expected: すべて PASS、警告なし

- [ ] **Step 8: コミット**

```bash
git add crates/core/src/order.rs crates/core/src/lib.rs crates/core/src/view.rs app/src-tauri/src/monitor.rs app/src-tauri/src/commands.rs app/src-tauri/src/main.rs
git commit -m "一覧の並び順をセッション ID で覚え、新しいセッションは同じプロジェクトの後ろに並べる"
```

---

### Task 4: 一覧の行をドラッグで並び替える（画面）

**Files:**
- Create: `app/src/drag.ts`
- Modify: `app/src/list.ts`, `app/src/style.css`

**Interfaces:**
- Consumes: コマンド `move_session(id, beforeId)`（Task 3。Tauri は引数名を camelCase で受ける: `{ id, beforeId }`）
- Produces: `setupDrag(listEl: HTMLElement, onDrop: (id: string, beforeId: string | null) => void): { isDragging(): boolean; consumeClick(): boolean }`

- [ ] **Step 1: ドラッグの処理を書く**（`app/src/drag.ts`）

```ts
// 行を押すと詳細を開く操作と区別するため、この距離を動かしてからドラッグとみなす
const DRAG_THRESHOLD_PX = 4;

interface DragState {
  id: string;
  row: HTMLElement;
  pointerId: number;
  startY: number;
  started: boolean;
  beforeId: string | null;
}

export function setupDrag(
  listEl: HTMLElement,
  onDrop: (id: string, beforeId: string | null) => void,
): { isDragging(): boolean; consumeClick(): boolean } {
  const indicator = document.createElement("div");
  indicator.className = "drop-indicator";
  let drag: DragState | null = null;
  let suppressClick = false;

  const rows = () => [...listEl.querySelectorAll<HTMLElement>(".row[data-id]")];

  function targetFor(clientY: number, dragged: HTMLElement): { beforeId: string | null; top: number } {
    const others = rows().filter((row) => row !== dragged);
    for (const row of others) {
      const box = row.getBoundingClientRect();
      if (clientY < box.top + box.height / 2) return { beforeId: row.dataset.id!, top: row.offsetTop };
    }
    const last = others.at(-1);
    return { beforeId: null, top: last ? last.offsetTop + last.offsetHeight : 0 };
  }

  listEl.addEventListener("pointerdown", (event) => {
    const row = (event.target as HTMLElement).closest<HTMLElement>(".row[data-id]");
    if (!row || event.button !== 0) return;
    drag = { id: row.dataset.id!, row, pointerId: event.pointerId, startY: event.clientY, started: false, beforeId: null };
  });

  listEl.addEventListener("pointermove", (event) => {
    if (!drag || event.pointerId !== drag.pointerId) return;
    const dy = event.clientY - drag.startY;
    if (!drag.started) {
      if (Math.abs(dy) < DRAG_THRESHOLD_PX) return;
      drag.started = true;
      listEl.setPointerCapture(drag.pointerId);
      drag.row.classList.add("dragging");
      listEl.append(indicator);
    }
    drag.row.style.transform = `translateY(${dy}px)`;
    const target = targetFor(event.clientY, drag.row);
    drag.beforeId = target.beforeId;
    indicator.style.top = `${target.top}px`;
  });

  function finish(drop: boolean): void {
    if (!drag) return;
    if (drag.started) {
      suppressClick = true;
      drag.row.classList.remove("dragging");
      drag.row.style.transform = "";
      indicator.remove();
      if (drop) onDrop(drag.id, drag.beforeId);
    }
    drag = null;
  }
  listEl.addEventListener("pointerup", () => finish(true));
  listEl.addEventListener("pointercancel", () => finish(false));

  return {
    isDragging: () => drag?.started ?? false,
    consumeClick: () => {
      const suppressed = suppressClick;
      suppressClick = false;
      return suppressed;
    },
  };
}
```

- [ ] **Step 2: 一覧に組み込む**（`app/src/list.ts`）

- `renderRow` で、`session.status !== "unknown"` の行に `row.dataset.id = session.id;` を付ける（状態不明の行は動かせない）
- 行の `click` の最初で `if (dragger.consumeClick()) return;` とする（ドラッグを離した直後のクリックで詳細が開かないように）
- `render()` の最初で `if (dragger.isDragging()) return;` とする（1 秒ごとの再描画や `sessions-updated` で、ドラッグ中の行を作り直さない）
- ファイルの下の方（`setupSettingsMenu()` の前）に次を足す

```ts
const dragger = setupDrag(listEl, (id, beforeId) => {
  const moving = sessions.find((session) => session.id === id);
  if (moving) {
    const rest = sessions.filter((session) => session.id !== id);
    const index = beforeId === null ? rest.length : rest.findIndex((session) => session.id === beforeId);
    rest.splice(index < 0 ? rest.length : index, 0, moving);
    sessions = rest;
  }
  render();
  void invoke("move_session", { id, beforeId });
});
```

`dragger` は `renderRow` と `render` から参照されるので、`let` の変数宣言の近く（`let page` の下）に `let dragger: ReturnType<typeof setupDrag>;` を置き、上の代入を `dragger = setupDrag(...)` にする。`import { setupDrag } from "./drag";` を足す。

- [ ] **Step 3: 見た目を足す**（`app/src/style.css` の `.row` の近く）

```css
.sessions { position: relative; }
.row[data-id] { touch-action: none; }
.row.dragging { position: relative; z-index: 1; background: var(--bg-hover); box-shadow: 0 6px 16px rgb(0 0 0 / 0.45); }
.drop-indicator { position: absolute; left: 0; right: 0; height: 2px; margin-top: -1px; background: var(--fg-secondary); pointer-events: none; }
```

落とす位置の線は、文字ではないので Figma どおり色を使ってよいが、状態の色と混同しないよう `--fg-secondary` にする（Figma の修正案の線の色を確かめ、違えば合わせる）。

- [ ] **Step 4: ビルドと手動確認**

Run: `(cd app && npm run build)` → `npm run tauri dev`（`app/` で）
確かめること:
- 行を 4px 未満動かして離すと詳細が開く。4px 以上動かすとドラッグになり、離しても詳細は開かない
- 落とす位置に線が出て、離すとその位置に並ぶ。アプリを再起動しても順が保たれる
- ドラッグ中に 1 秒以上止めても、行が消えたり位置が飛んだりしない
- Unknown タブの行はドラッグできない

- [ ] **Step 5: コミット**

```bash
git add app/src/drag.ts app/src/list.ts app/src/style.css
git commit -m "一覧の行をドラッグで並び替えられるようにする"
```

---

### Task 5: `claude` の場所を探し、時間制限つきで実行する（core）

**Files:**
- Create: `crates/core/src/claude_cli.rs`
- Modify: `crates/core/src/lib.rs`（`pub mod claude_cli;`）

**Interfaces:**
- Produces:
  - `claude_cli::CLI_TIMEOUT: Duration`（60 秒）
  - `claude_cli::find_claude(configured: Option<&str>, home: &Path, is_executable: impl Fn(&Path) -> bool) -> Option<PathBuf>`
  - `claude_cli::is_executable(path: &Path) -> bool`
  - `claude_cli::ClaudeCli::new(program: PathBuf) -> ClaudeCli`、`ClaudeCli::run(&self, config_dir: &Path, args: &[String], timeout: Duration) -> Result<String, String>`（成功なら標準出力、失敗なら理由）
  - `claude_cli::failure_reason(stderr: &str, stdout: &str) -> String`

- [ ] **Step 1: 失敗するテストを書く**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    #[test]
    fn configured_path_wins_then_known_locations_in_order() {
        let home = Path::new("/Users/u");
        let exists = |p: &Path| p == Path::new("/opt/homebrew/bin/claude") || p == Path::new("/Users/u/.local/bin/claude");
        assert_eq!(find_claude(None, home, exists), Some(PathBuf::from("/Users/u/.local/bin/claude")));
        assert_eq!(find_claude(Some("/opt/homebrew/bin/claude"), home, exists), Some(PathBuf::from("/opt/homebrew/bin/claude")));
        assert_eq!(find_claude(Some("/nowhere/claude"), home, |_| false), None);
    }

    #[test]
    fn passes_config_dir_and_returns_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "claude", r#"echo "$CLAUDE_CONFIG_DIR $1""#);
        let out = ClaudeCli::new(program).run(Path::new("/cfg/a"), &["plugin".into()], CLI_TIMEOUT).unwrap();
        assert_eq!(out.trim(), "/cfg/a plugin");
    }

    #[test]
    fn program_directory_is_on_path() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "claude", r#"echo "$PATH""#);
        let out = ClaudeCli::new(program).run(Path::new("/cfg"), &[], CLI_TIMEOUT).unwrap();
        assert!(out.split(':').next().unwrap().trim() == dir.path().to_str().unwrap());
    }

    #[test]
    fn failure_reports_last_stderr_line() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "claude", "echo progress; echo 'first' >&2; echo 'blocked by policy' >&2; exit 1");
        let error = ClaudeCli::new(program).run(Path::new("/cfg"), &[], CLI_TIMEOUT).unwrap_err();
        assert_eq!(error, "blocked by policy");
    }

    #[test]
    fn times_out_and_kills_the_process() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "claude", "sleep 5");
        let started = std::time::Instant::now();
        let error = ClaudeCli::new(program).run(Path::new("/cfg"), &[], Duration::from_millis(200)).unwrap_err();
        assert_eq!(error, "時間切れ（応答がありませんでした）");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn reason_is_trimmed_to_200_chars_and_falls_back_to_stdout() {
        let long = "あ".repeat(300);
        assert_eq!(failure_reason(&format!("{long}\n\n"), "").chars().count(), 200);
        assert_eq!(failure_reason("", "only stdout\n"), "only stdout");
        assert_eq!(failure_reason("", ""), "理由がわかりませんでした");
    }
}
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core claude_cli::`
Expected: コンパイルエラー

- [ ] **Step 3: 実装する**

```rust
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const CLI_TIMEOUT: Duration = Duration::from_secs(60);
const REASON_MAX_CHARS: usize = 200;
const POLL_INTERVAL: Duration = Duration::from_millis(50);
// GUI アプリにはターミナルの PATH が引き継がれない。npm 版の claude は node を探すので、よくある場所も足す
const EXTRA_PATH: [&str; 6] = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"];

pub fn find_claude(configured: Option<&str>, home: &Path, is_executable: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    if let Some(path) = configured {
        let path = PathBuf::from(path);
        return is_executable(&path).then_some(path);
    }
    [
        home.join(".local/bin/claude"),
        PathBuf::from("/opt/homebrew/bin/claude"),
        PathBuf::from("/usr/local/bin/claude"),
        home.join(".npm-global/bin/claude"),
        home.join(".claude/local/claude"),
    ]
    .into_iter()
    .find(|path| is_executable(path))
}

pub fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

pub fn failure_reason(stderr: &str, stdout: &str) -> String {
    let last_line = |text: &str| text.lines().rev().map(str::trim).find(|line| !line.is_empty()).map(str::to_string);
    let reason = last_line(stderr).or_else(|| last_line(stdout)).unwrap_or_else(|| "理由がわかりませんでした".into());
    reason.chars().take(REASON_MAX_CHARS).collect()
}

#[derive(Debug, Clone)]
pub struct ClaudeCli {
    program: PathBuf,
}

impl ClaudeCli {
    pub fn new(program: PathBuf) -> Self {
        ClaudeCli { program }
    }

    pub fn run(&self, config_dir: &Path, args: &[String], timeout: Duration) -> Result<String, String> {
        let mut child = Command::new(&self.program)
            .args(args)
            .env("CLAUDE_CONFIG_DIR", config_dir)
            .env("PATH", self.path_env())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("claude を起動できませんでした: {error}"))?;
        // 出力を読みながら待たないと、出力が多いときにパイプが詰まって終わらなくなる
        let stdout = drain(child.stdout.take());
        let stderr = drain(child.stderr.take());
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("時間切れ（応答がありませんでした）".into());
                }
                Ok(None) => thread::sleep(POLL_INTERVAL),
                Err(error) => return Err(format!("claude の終了を待てませんでした: {error}")),
            }
        };
        let (stdout, stderr) = (stdout.join().unwrap_or_default(), stderr.join().unwrap_or_default());
        if status.success() {
            Ok(stdout)
        } else {
            Err(failure_reason(&stderr, &stdout))
        }
    }

    fn path_env(&self) -> String {
        let mut parts: Vec<String> = self.program.parent().map(|dir| dir.to_string_lossy().into_owned()).into_iter().collect();
        parts.extend(EXTRA_PATH.iter().map(|dir| dir.to_string()));
        parts.join(":")
    }
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut text = String::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_string(&mut text);
        }
        text
    })
}
```

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test -p agents-monitor-core claude_cli::`
Expected: 6 件 PASS

- [ ] **Step 5: コミット**

```bash
git add crates/core/src/claude_cli.rs crates/core/src/lib.rs
git commit -m "claude コマンドを探し、時間制限つきで実行する仕組みを足す"
```

---

### Task 6: 導入の状態の判定と、実行する手順（core）

**Files:**
- Create: `crates/core/src/plugin_status.rs`, `crates/core/src/install_plan.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 のテストデータ
- Produces:
  - `plugin_status::{PLUGIN_ID, MARKETPLACE_NAME}`
  - `plugin_status::PluginPresence { Absent, Enabled { version: String }, Disabled }`
  - `plugin_status::MarketplacePresence { Absent, Bundled, Other }`
  - `plugin_status::Inspection { plugin: PluginPresence, marketplace: MarketplacePresence }`
  - `plugin_status::parse_inspection(plugin_list_json: &str, marketplace_list_json: &str, bundled_dir: &Path) -> Result<Inspection, String>`
  - `plugin_status::DirState { Installed, NotInstalled, Missing }`（`serde` で `"installed"` などの snake_case）、`Inspection::state(&self) -> DirState`
  - `install_plan::CliStep { MarketplaceAdd, MarketplaceUpdate, MarketplaceRemove, Install, Update, Uninstall, Enable }`、`CliStep::args(&self, bundled_dir: &Path) -> Vec<String>`
  - `install_plan::{install_steps(&Inspection) -> Vec<CliStep>, update_steps(&Inspection, bundled_version: &str) -> Vec<CliStep>, remove_steps(&Inspection) -> Vec<CliStep>}`
  - `install_plan::INSPECT_PLUGINS: [&str; 3]`、`install_plan::INSPECT_MARKETPLACES: [&str; 4]`

- [ ] **Step 1: 失敗するテストを書く**（`crates/core/src/plugin_status.rs`）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = "/Users/tester/.agents-monitor/marketplace";

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/fixtures/cli/{name}.json", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn inspect(plugins: &str, marketplaces: &str) -> Inspection {
        parse_inspection(&fixture(plugins), &fixture(marketplaces), Path::new(BUNDLED)).unwrap()
    }

    #[test]
    fn nothing_installed() {
        let i = inspect("plugin-list-empty", "marketplace-list-empty");
        assert_eq!(i, Inspection { plugin: PluginPresence::Absent, marketplace: MarketplacePresence::Absent });
        assert_eq!(i.state(), DirState::NotInstalled);
    }

    #[test]
    fn installed_from_bundled_marketplace() {
        let i = inspect("plugin-list-bundled", "marketplace-list-bundled");
        assert_eq!(i.plugin, PluginPresence::Enabled { version: "0.1.0".into() });
        assert_eq!(i.marketplace, MarketplacePresence::Bundled);
        assert_eq!(i.state(), DirState::Installed);
    }

    #[test]
    fn installed_from_elsewhere_counts_as_installed() {
        let i = inspect("plugin-list-other", "marketplace-list-other");
        assert_eq!(i.marketplace, MarketplacePresence::Other);
        assert_eq!(i.state(), DirState::Installed);
    }

    #[test]
    fn disabled_counts_as_not_installed() {
        let i = inspect("plugin-list-disabled", "marketplace-list-bundled");
        assert_eq!(i.plugin, PluginPresence::Disabled);
        assert_eq!(i.state(), DirState::NotInstalled);
    }

    #[test]
    fn trailing_slash_on_bundled_path_still_matches() {
        let i = parse_inspection("[]", &fixture("marketplace-list-bundled"), Path::new(&format!("{BUNDLED}/"))).unwrap();
        assert_eq!(i.marketplace, MarketplacePresence::Bundled);
    }

    #[test]
    fn broken_or_unexpected_output_is_an_error_not_a_panic() {
        for (plugins, marketplaces) in [("", "[]"), ("[]", "{oops"), ("{\"a\":1}", "[]"), ("[]", "null")] {
            assert_eq!(
                parse_inspection(plugins, marketplaces, Path::new(BUNDLED)).unwrap_err(),
                "導入状態を読めませんでした"
            );
        }
    }

    #[test]
    fn unknown_fields_and_other_plugins_are_ignored() {
        let plugins = r#"[{"id":"x@y","enabled":true,"version":"9"},{"id":"agents-monitor@agents-monitor","enabled":true,"version":"0.2.0","new":1}]"#;
        let i = parse_inspection(plugins, "[]", Path::new(BUNDLED)).unwrap();
        assert_eq!(i.plugin, PluginPresence::Enabled { version: "0.2.0".into() });
    }
}
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core plugin_status::`
Expected: コンパイルエラー

- [ ] **Step 3: 実装する**（`crates/core/src/plugin_status.rs` のテストの上）

```rust
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const PLUGIN_ID: &str = "agents-monitor@agents-monitor";
pub const MARKETPLACE_NAME: &str = "agents-monitor";
const UNREADABLE: &str = "導入状態を読めませんでした";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginPresence {
    Absent,
    Enabled { version: String },
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketplacePresence {
    Absent,
    Bundled,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    pub plugin: PluginPresence,
    pub marketplace: MarketplacePresence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DirState {
    Installed,
    NotInstalled,
    Missing,
}

impl Inspection {
    pub fn state(&self) -> DirState {
        match self.plugin {
            PluginPresence::Enabled { .. } => DirState::Installed,
            PluginPresence::Absent | PluginPresence::Disabled => DirState::NotInstalled,
        }
    }
}

#[derive(Deserialize)]
struct PluginEntry {
    id: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    version: String,
}

#[derive(Deserialize)]
struct MarketplaceEntry {
    name: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    path: Option<String>,
}

pub fn parse_inspection(plugin_list_json: &str, marketplace_list_json: &str, bundled_dir: &Path) -> Result<Inspection, String> {
    let plugins: Vec<PluginEntry> = serde_json::from_str(plugin_list_json).map_err(|_| UNREADABLE.to_string())?;
    let marketplaces: Vec<MarketplaceEntry> = serde_json::from_str(marketplace_list_json).map_err(|_| UNREADABLE.to_string())?;
    let plugin = match plugins.into_iter().find(|p| p.id == PLUGIN_ID) {
        None => PluginPresence::Absent,
        Some(p) if p.enabled => PluginPresence::Enabled { version: p.version },
        Some(_) => PluginPresence::Disabled,
    };
    let bundled = normalized(&bundled_dir.to_string_lossy());
    let marketplace = match marketplaces.into_iter().find(|m| m.name == MARKETPLACE_NAME) {
        None => MarketplacePresence::Absent,
        Some(m) if m.source == "directory" && m.path.as_deref().map(normalized) == Some(bundled.clone()) => MarketplacePresence::Bundled,
        Some(_) => MarketplacePresence::Other,
    };
    Ok(Inspection { plugin, marketplace })
}

fn normalized(path: &str) -> String {
    path.trim_end_matches('/').to_string()
}
```

Task 1 で採った実際の出力の項目名が `id`・`enabled`・`version`・`name`・`source`・`path` と違っていたら、テストデータに合わせて `PluginEntry`・`MarketplaceEntry` を直す（2026-10-10 に確かめた出力ではこの名前だった）。

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test -p agents-monitor-core plugin_status::`
Expected: 7 件 PASS

- [ ] **Step 5: 手順の失敗するテストを書く**（`crates/core/src/install_plan.rs`）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_status::{MarketplacePresence as M, PluginPresence as P};

    fn i(plugin: P, marketplace: M) -> Inspection {
        Inspection { plugin, marketplace }
    }
    fn enabled(v: &str) -> P {
        P::Enabled { version: v.into() }
    }

    #[test]
    fn install_registers_marketplace_only_when_missing() {
        assert_eq!(install_steps(&i(P::Absent, M::Absent)), [CliStep::MarketplaceAdd, CliStep::Install]);
        assert_eq!(install_steps(&i(P::Absent, M::Bundled)), [CliStep::Install]);
        assert_eq!(install_steps(&i(P::Absent, M::Other)), [CliStep::Install], "別の場所のマーケットプレイスから入れる");
        assert_eq!(install_steps(&i(P::Disabled, M::Other)), [CliStep::Enable]);
        assert!(install_steps(&i(enabled("0.1.0"), M::Bundled)).is_empty());
    }

    #[test]
    fn update_only_bundled_installs_with_a_different_version() {
        assert_eq!(update_steps(&i(enabled("0.1.0"), M::Bundled), "0.2.0"), [CliStep::MarketplaceUpdate, CliStep::Update]);
        assert!(update_steps(&i(enabled("0.2.0"), M::Bundled), "0.2.0").is_empty());
        assert!(update_steps(&i(enabled("0.1.0"), M::Other), "0.2.0").is_empty(), "別の場所のものはその元で更新される");
        assert!(update_steps(&i(P::Absent, M::Bundled), "0.2.0").is_empty());
    }

    #[test]
    fn remove_uninstalls_and_drops_only_our_marketplace() {
        assert_eq!(remove_steps(&i(enabled("0.1.0"), M::Bundled)), [CliStep::Uninstall, CliStep::MarketplaceRemove]);
        assert_eq!(remove_steps(&i(enabled("0.1.0"), M::Other)), [CliStep::Uninstall]);
        assert_eq!(remove_steps(&i(P::Disabled, M::Bundled)), [CliStep::Uninstall, CliStep::MarketplaceRemove]);
        assert_eq!(remove_steps(&i(P::Absent, M::Bundled)), [CliStep::MarketplaceRemove]);
        assert!(remove_steps(&i(P::Absent, M::Other)).is_empty());
    }

    #[test]
    fn steps_map_to_claude_plugin_arguments() {
        let dir = Path::new("/h/.agents-monitor/marketplace");
        assert_eq!(CliStep::MarketplaceAdd.args(dir), ["plugin", "marketplace", "add", "/h/.agents-monitor/marketplace"]);
        assert_eq!(CliStep::Install.args(dir), ["plugin", "install", "agents-monitor@agents-monitor"]);
        assert_eq!(CliStep::MarketplaceRemove.args(dir), ["plugin", "marketplace", "remove", "agents-monitor"]);
        assert_eq!(CliStep::Enable.args(dir), ["plugin", "enable", "agents-monitor@agents-monitor"]);
    }
}
```

- [ ] **Step 6: 実装する**（テストの上）

```rust
use std::path::Path;

use crate::plugin_status::{Inspection, MarketplacePresence, PluginPresence, MARKETPLACE_NAME, PLUGIN_ID};

pub const INSPECT_PLUGINS: [&str; 3] = ["plugin", "list", "--json"];
pub const INSPECT_MARKETPLACES: [&str; 4] = ["plugin", "marketplace", "list", "--json"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliStep {
    MarketplaceAdd,
    MarketplaceUpdate,
    MarketplaceRemove,
    Install,
    Update,
    Uninstall,
    Enable,
}

impl CliStep {
    pub fn args(&self, bundled_dir: &Path) -> Vec<String> {
        let words: Vec<&str> = match self {
            CliStep::MarketplaceAdd => return vec!["plugin".into(), "marketplace".into(), "add".into(), bundled_dir.to_string_lossy().into_owned()],
            CliStep::MarketplaceUpdate => vec!["plugin", "marketplace", "update", MARKETPLACE_NAME],
            CliStep::MarketplaceRemove => vec!["plugin", "marketplace", "remove", MARKETPLACE_NAME],
            CliStep::Install => vec!["plugin", "install", PLUGIN_ID],
            CliStep::Update => vec!["plugin", "update", PLUGIN_ID],
            CliStep::Uninstall => vec!["plugin", "uninstall", PLUGIN_ID],
            CliStep::Enable => vec!["plugin", "enable", PLUGIN_ID],
        };
        words.into_iter().map(String::from).collect()
    }
}

pub fn install_steps(inspection: &Inspection) -> Vec<CliStep> {
    match (&inspection.plugin, inspection.marketplace) {
        (PluginPresence::Enabled { .. }, _) => vec![],
        (PluginPresence::Disabled, _) => vec![CliStep::Enable],
        (PluginPresence::Absent, MarketplacePresence::Absent) => vec![CliStep::MarketplaceAdd, CliStep::Install],
        (PluginPresence::Absent, _) => vec![CliStep::Install],
    }
}

pub fn update_steps(inspection: &Inspection, bundled_version: &str) -> Vec<CliStep> {
    match (&inspection.plugin, inspection.marketplace) {
        (PluginPresence::Enabled { version }, MarketplacePresence::Bundled) if version != bundled_version => {
            vec![CliStep::MarketplaceUpdate, CliStep::Update]
        }
        _ => vec![],
    }
}

pub fn remove_steps(inspection: &Inspection) -> Vec<CliStep> {
    let mut steps = Vec::new();
    if inspection.plugin != PluginPresence::Absent {
        steps.push(CliStep::Uninstall);
    }
    if inspection.marketplace == MarketplacePresence::Bundled {
        steps.push(CliStep::MarketplaceRemove);
    }
    steps
}
```

Task 1 の Step 3 で「写し直すだけで反映される」と記録された場合は、`update_steps` を `vec![CliStep::Update]` にしてテストもそれに合わせる。

- [ ] **Step 7: 通ることを確かめ、コミット**

Run: `cargo test -p agents-monitor-core plugin_status:: install_plan::`
Expected: すべて PASS

```bash
git add crates/core/src/plugin_status.rs crates/core/src/install_plan.rs crates/core/src/lib.rs
git commit -m "claude plugin の出力から導入の状態を判定し、入れる・更新する・外す手順を決める"
```

---

### Task 7: 同梱マーケットプレイスを写す（core と bundle 設定）

**Files:**
- Create: `crates/core/src/marketplace.rs`
- Modify: `crates/core/src/paths.rs`, `crates/core/src/lib.rs`
- Modify: `app/src-tauri/tauri.conf.json`
- Create: `app/src-tauri/tests/bundle_resources.rs`

**Interfaces:**
- Produces:
  - `paths::marketplace_dir(home: &Path) -> PathBuf`（`home/marketplace`）
  - `marketplace::sync_marketplace(source: &Path, dest: &Path) -> io::Result<bool>`（変わったファイルがあれば true）
  - `marketplace::bundled_plugin_version(dest: &Path) -> Option<String>`
  - `.app` の `Resources/marketplace/` に `.claude-plugin/marketplace.json` と `plugin/` 一式

- [ ] **Step 1: 失敗するテストを書く**（`crates/core/src/marketplace.rs`）

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    #[test]
    fn copies_tree_and_reports_changes_only_when_content_differs() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        write(&src.path().join(".claude-plugin/marketplace.json"), "{}");
        write(&src.path().join("plugin/.claude-plugin/plugin.json"), r#"{"version":"0.2.0"}"#);
        write(&src.path().join("plugin/hooks/hooks.json"), "{}");
        let out = dest.path().join("marketplace");
        assert!(sync_marketplace(src.path(), &out).unwrap());
        assert_eq!(fs::read_to_string(out.join("plugin/hooks/hooks.json")).unwrap(), "{}");
        assert!(!sync_marketplace(src.path(), &out).unwrap(), "同じ中身なら写し直さない");
        write(&src.path().join("plugin/hooks/hooks.json"), r#"{"a":1}"#);
        assert!(sync_marketplace(src.path(), &out).unwrap());
        assert_eq!(bundled_plugin_version(&out).as_deref(), Some("0.2.0"));
    }

    #[test]
    fn removes_files_that_no_longer_exist_in_the_bundle() {
        let src = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();
        write(&src.path().join("plugin/a.json"), "{}");
        let out = dest.path().join("marketplace");
        write(&out.join("plugin/stale.json"), "{}");
        assert!(sync_marketplace(src.path(), &out).unwrap());
        assert!(!out.join("plugin/stale.json").exists());
    }

    #[test]
    fn missing_source_is_an_error() {
        let dest = tempfile::tempdir().unwrap();
        assert!(sync_marketplace(Path::new("/nonexistent/src"), &dest.path().join("m")).is_err());
    }
}
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core marketplace::`
Expected: コンパイルエラー

- [ ] **Step 3: 実装する**

```rust
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn sync_marketplace(source: &Path, dest: &Path) -> io::Result<bool> {
    let wanted = files_under(source)?;
    let mut changed = false;
    for relative in &wanted {
        let bytes = fs::read(source.join(relative))?;
        let target = dest.join(relative);
        if fs::read(&target).ok().as_deref() == Some(bytes.as_slice()) {
            continue;
        }
        fs::create_dir_all(target.parent().unwrap_or(dest))?;
        fs::write(&target, bytes)?;
        changed = true;
    }
    if dest.exists() {
        let keep: HashSet<&PathBuf> = wanted.iter().collect();
        for relative in files_under(dest)? {
            if !keep.contains(&relative) {
                fs::remove_file(dest.join(&relative))?;
                changed = true;
            }
        }
    }
    Ok(changed)
}

pub fn bundled_plugin_version(dest: &Path) -> Option<String> {
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(dest.join("plugin/.claude-plugin/plugin.json")).ok()?).ok()?;
    manifest["version"].as_str().map(String::from)
}

fn files_under(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![PathBuf::new()];
    while let Some(relative) = pending.pop() {
        for entry in fs::read_dir(root.join(&relative))? {
            let entry = entry?;
            let path = relative.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                pending.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}
```

`crates/core/src/paths.rs` に足す。

```rust
pub fn marketplace_dir(home: &Path) -> PathBuf {
    home.join("marketplace")
}
```

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test -p agents-monitor-core marketplace::`
Expected: 3 件 PASS

- [ ] **Step 5: `.app` に同梱する**

`app/src-tauri/tauri.conf.json` の `bundle` に足す（隠しディレクトリを確実に含めるため、ファイルを 1 つずつ書く）。

```json
"resources": {
  "../../.claude-plugin/marketplace.json": "marketplace/.claude-plugin/marketplace.json",
  "../../plugin/.claude-plugin/plugin.json": "marketplace/plugin/.claude-plugin/plugin.json",
  "../../plugin/hooks/hooks.json": "marketplace/plugin/hooks/hooks.json"
}
```

- [ ] **Step 6: 同梱し忘れを防ぐテストを書く**（`app/src-tauri/tests/bundle_resources.rs`）

```rust
use std::fs;
use std::path::{Path, PathBuf};

fn files_under(root: &Path, relative: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root.join(relative)).unwrap() {
        let entry = entry.unwrap();
        let path = relative.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            files_under(root, &path, out);
        } else {
            out.push(path);
        }
    }
}

#[test]
fn every_plugin_file_is_bundled_into_marketplace() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest_dir.join("../..");
    let conf: serde_json::Value = serde_json::from_str(&fs::read_to_string(manifest_dir.join("tauri.conf.json")).unwrap()).unwrap();
    let resources = conf["bundle"]["resources"].as_object().expect("bundle.resources がない");
    let mut files = vec![PathBuf::from(".claude-plugin/marketplace.json")];
    files_under(&repo, Path::new("plugin"), &mut files);
    for file in files {
        let source = format!("../../{}", file.display());
        let dest = format!("marketplace/{}", file.display());
        assert_eq!(resources.get(&source).and_then(|v| v.as_str()), Some(dest.as_str()), "{source} を bundle.resources に足すこと");
    }
}
```

Run: `cargo test -p agents-monitor-app --test bundle_resources`
Expected: PASS（試しに `tauri.conf.json` から 1 行消すと FAIL することを確かめてから戻す）

- [ ] **Step 7: 同梱されることを確かめ、コミット**

Run: `./scripts/build-hook-sidecar.sh && (cd app && npm run tauri build -- --bundles app) && find target/release/bundle/macos/agents-monitor.app/Contents/Resources/marketplace -type f`
Expected: 3 つのファイルが出る

```bash
git add crates/core/src/marketplace.rs crates/core/src/paths.rs crates/core/src/lib.rs app/src-tauri/tauri.conf.json app/src-tauri/tests/bundle_resources.rs
git commit -m "プラグイン一式をアプリに同梱し、~/.agents-monitor/marketplace へ写す仕組みを足す"
```

---

### Task 8: 導入・更新・削除を裏で実行し、画面へ状態を送る（アプリ）

**Files:**
- Create: `crates/core/src/config_dirs.rs`, `crates/core/src/attention.rs`
- Modify: `crates/core/src/lib.rs`
- Create: `app/src-tauri/src/installer.rs`
- Modify: `app/src-tauri/Cargo.toml`（`tauri-plugin-dialog = "2"`）
- Modify: `app/src-tauri/src/main.rs`, `app/src-tauri/src/monitor.rs`, `app/src-tauri/src/commands.rs`, `app/src-tauri/src/tray.rs`

**Interfaces:**
- Consumes: Task 2・5・6・7 のすべて
- Produces:
  - `config_dirs::initial_config_dirs(current: Option<&[String]>, default_dir: &Path) -> Option<Vec<String>>`（決め直すときだけ `Some`）
  - `config_dirs::display_path(path: &str, home: &Path) -> String`
  - `attention::needs_attention(states: &[DirState], claude_found: bool, notifications_allowed: bool) -> bool`
  - アプリ: `installer::Job { Startup, Add(String), Retry(String), Remove(String), Refresh }`、`installer::spawn(app: AppHandle) -> Sender<Job>`、`installer::SetupView { dirs: Vec<ConfigDirView>, claude_found: bool, notifications_allowed: bool, needs_attention: bool }`、`installer::ConfigDirView { path: String, display: String, state: DirState, busy: bool }`
  - イベント `setup-updated`（`SetupView`）、`open-settings`（引数なし）
  - コマンド `get_setup() -> SetupView`、`add_config_dir()`、`retry_config_dir(path: String)`、`remove_config_dir(path: String)`、`set_claude_path(path: String) -> Result<(), String>`

- [ ] **Step 1: core の 2 つの小さな関数のテストを書く**

`crates/core/src/config_dirs.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decides_only_while_unset_and_default_exists() {
        let home = tempfile::tempdir().unwrap();
        let claude = home.path().join(".claude");
        assert_eq!(initial_config_dirs(None, &claude), None, "~/.claude がなければ決めずに次の起動でまた探す");
        std::fs::create_dir(&claude).unwrap();
        assert_eq!(initial_config_dirs(None, &claude), Some(vec![claude.to_string_lossy().into_owned()]));
        assert_eq!(initial_config_dirs(Some(&[]), &claude), None, "すべて削除した後は入れ直さない");
        assert_eq!(initial_config_dirs(Some(&["/x".into()]), &claude), None);
    }

    #[test]
    fn shows_home_as_tilde() {
        let home = Path::new("/Users/u");
        assert_eq!(display_path("/Users/u/.claude", home), "~/.claude");
        assert_eq!(display_path("/Users/u", home), "~");
        assert_eq!(display_path("/Users/uu/.claude", home), "/Users/uu/.claude");
        assert_eq!(display_path("/opt/claude", home), "/opt/claude");
    }
}
```

`crates/core/src/attention.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use DirState::*;

    #[test]
    fn badge_when_something_needs_the_user() {
        assert!(!needs_attention(&[Installed], true, true));
        assert!(needs_attention(&[Installed], true, false), "通知がオフ");
        assert!(needs_attention(&[Installed], false, true), "claude が見つからない");
        assert!(needs_attention(&[], true, true), "入っているディレクトリが 1 つもない");
        assert!(needs_attention(&[Installed, NotInstalled], true, true), "入っていないディレクトリがある");
        assert!(needs_attention(&[Installed, Missing], true, true), "フォルダーがない");
    }
}
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agents-monitor-core config_dirs:: attention::`
Expected: コンパイルエラー

- [ ] **Step 3: 実装する**

`crates/core/src/config_dirs.rs`（テストの上）:

```rust
use std::path::Path;

pub fn initial_config_dirs(current: Option<&[String]>, default_dir: &Path) -> Option<Vec<String>> {
    if current.is_some() || !default_dir.is_dir() {
        return None;
    }
    Some(vec![default_dir.to_string_lossy().into_owned()])
}

pub fn display_path(path: &str, home: &Path) -> String {
    let home = home.to_string_lossy();
    match path.strip_prefix(home.as_ref()) {
        Some("") => "~".into(),
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}
```

`crates/core/src/attention.rs`（テストの上）:

```rust
use crate::plugin_status::DirState;

pub fn needs_attention(states: &[DirState], claude_found: bool, notifications_allowed: bool) -> bool {
    !notifications_allowed || !claude_found || states.is_empty() || states.iter().any(|s| *s != DirState::Installed)
}
```

`lib.rs` に `pub mod config_dirs;` と `pub mod attention;` を足す。

Run: `cargo test -p agents-monitor-core config_dirs:: attention::`
Expected: 3 件 PASS

- [ ] **Step 4: dialog プラグインを入れる**

`app/src-tauri/Cargo.toml` の `[dependencies]` に `tauri-plugin-dialog = "2"` を足し、`main.rs` の Builder に `.plugin(tauri_plugin_dialog::init())` を足す。ダイアログは Rust 側からだけ使うので、`capabilities/default.json` は変えない。

- [ ] **Step 5: 一覧ウィンドウをダイアログ中に閉じないようにする**（`app/src-tauri/src/monitor.rs` と `tray.rs`）

`AppState` に `pub suppress_hide: AtomicBool` を足し（`load_state` で `AtomicBool::new(false)`）、`tray.rs` の `Focused(false)` の処理を次にする。

```rust
if let WindowEvent::Focused(false) = event {
    let state = handle.state::<AppState>();
    // フォルダーを選ぶ画面やアラートを開くとフォーカスが移るが、そのときに閉じると設定画面が消えてしまう
    if state.suppress_hide.load(Ordering::Relaxed) {
        return;
    }
    let _ = window.hide();
    state.list_hidden_at_ms.store(paths::now_ms(), Ordering::Relaxed);
}
```

`tray.rs` に、一覧を開いて設定ページを出す関数を足す。

```rust
pub fn show_settings(app: &AppHandle) {
    let Some(window) = app.get_webview_window("list") else { return };
    let _ = window.move_window(Position::TrayBottomCenter);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit("open-settings", ());
}
```

- [ ] **Step 6: installer を書く**（`app/src-tauri/src/installer.rs`）

```rust
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Mutex;
use std::thread;

use agents_monitor_core::attention::needs_attention;
use agents_monitor_core::claude_cli::{find_claude, is_executable, ClaudeCli, CLI_TIMEOUT};
use agents_monitor_core::config_dirs::{display_path, initial_config_dirs};
use agents_monitor_core::install_plan::{install_steps, remove_steps, update_steps, CliStep, INSPECT_MARKETPLACES, INSPECT_PLUGINS};
use agents_monitor_core::marketplace::{bundled_plugin_version, sync_marketplace};
use agents_monitor_core::paths;
use agents_monitor_core::persist::save_json;
use agents_monitor_core::plugin_status::{parse_inspection, DirState, Inspection};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::monitor::AppState;
use crate::notifier;

pub enum Job {
    Startup,
    Add(String),
    Retry(String),
    Remove(String),
    Refresh,
}

#[derive(Default)]
pub struct InstallerState {
    pub sender: Mutex<Option<Sender<Job>>>,
    inspections: Mutex<HashMap<String, Result<Inspection, String>>>,
    busy: Mutex<HashSet<String>>,
    claude: Mutex<Option<PathBuf>>,
    marketplace_error: Mutex<Option<String>>,
}

#[derive(Clone, Serialize)]
pub struct ConfigDirView {
    pub path: String,
    pub display: String,
    pub state: DirState,
    pub busy: bool,
}

#[derive(Clone, Serialize)]
pub struct SetupView {
    pub dirs: Vec<ConfigDirView>,
    pub claude_found: bool,
    pub notifications_allowed: bool,
    pub needs_attention: bool,
}

pub fn spawn(app: AppHandle) {
    let (sender, receiver) = mpsc::channel();
    *app.state::<InstallerState>().sender.lock().unwrap() = Some(sender.clone());
    thread::spawn(move || run(app, receiver));
    let _ = sender.send(Job::Startup);
}

pub fn send(app: &AppHandle, job: Job) {
    if let Some(sender) = app.state::<InstallerState>().sender.lock().unwrap().as_ref() {
        let _ = sender.send(job);
    }
}

fn run(app: AppHandle, receiver: Receiver<Job>) {
    for job in receiver {
        match job {
            Job::Startup => startup(&app),
            Job::Add(path) => add(&app, path),
            Job::Retry(path) => user_action(&app, &path, |i| install_steps(i)),
            Job::Remove(path) => remove(&app, &path),
            Job::Refresh => refresh_all(&app),
        }
        publish(&app);
    }
}

fn startup(app: &AppHandle) {
    let state = app.state::<AppState>();
    let installer = app.state::<InstallerState>();
    let source = app.path().resource_dir().map(|dir| dir.join("marketplace"));
    let result = source
        .map_err(|error| error.to_string())
        .and_then(|source| sync_marketplace(&source, &paths::marketplace_dir(&state.home)).map_err(|error| error.to_string()));
    *installer.marketplace_error.lock().unwrap() = result.err().map(|error| format!("同梱のプラグインを用意できませんでした: {error}"));
    resolve_claude(app);

    let home = dirs::home_dir().unwrap_or_default();
    let auto_dir = {
        let mut settings = state.settings.lock().unwrap();
        let decided = initial_config_dirs(settings.config_dirs.as_deref(), &home.join(".claude"));
        if let Some(dirs) = decided.clone() {
            settings.config_dirs = Some(dirs);
            if let Err(error) = save_json(&paths::settings_path(&state.home), &*settings) {
                eprintln!("設定の保存に失敗しました: {error}");
            }
        }
        decided.and_then(|dirs| dirs.into_iter().next())
    };
    refresh_all(app);
    if let Some(dir) = auto_dir {
        let _ = apply(app, &dir, |i| install_steps(i));
    }
    let bundled_version = bundled_plugin_version(&paths::marketplace_dir(&state.home));
    if let Some(version) = bundled_version {
        for dir in config_dirs(app) {
            let _ = apply(app, &dir, |i| update_steps(i, &version));
        }
    }
    if config_dirs(app).is_empty() {
        crate::tray::show_settings(app);
    }
}

fn add(app: &AppHandle, path: String) {
    let state = app.state::<AppState>();
    {
        let mut settings = state.settings.lock().unwrap();
        let mut dirs = settings.config_dirs.clone().unwrap_or_default();
        if dirs.contains(&path) {
            return;
        }
        dirs.push(path.clone());
        settings.config_dirs = Some(dirs);
        if let Err(error) = save_json(&paths::settings_path(&state.home), &*settings) {
            alert(app, &format!("設定を保存できませんでした: {error}"));
            return;
        }
    }
    user_action(app, &path, |i| install_steps(i));
}

fn remove(app: &AppHandle, path: &str) {
    if Path::new(path).is_dir() {
        if let Err(reason) = apply(app, path, |i| remove_steps(i)) {
            alert(app, &format!("{} からプラグインを外せませんでした。\n{reason}", display(path)));
            return;
        }
    }
    let state = app.state::<AppState>();
    let mut settings = state.settings.lock().unwrap();
    let dirs: Vec<String> = settings.config_dirs.clone().unwrap_or_default().into_iter().filter(|d| d != path).collect();
    settings.config_dirs = Some(dirs);
    if let Err(error) = save_json(&paths::settings_path(&state.home), &*settings) {
        eprintln!("設定の保存に失敗しました: {error}");
    }
    app.state::<InstallerState>().inspections.lock().unwrap().remove(path);
}

fn user_action(app: &AppHandle, path: &str, steps: impl Fn(&Inspection) -> Vec<CliStep>) {
    if let Err(reason) = apply(app, path, steps) {
        alert(app, &format!("{} にプラグインを入れられませんでした。\n{reason}", display(path)));
    }
}

// 状態を読み直してから手順を決め、1 つずつ実行する。終わったらもう一度読み直して画面に反映する
fn apply(app: &AppHandle, path: &str, steps: impl Fn(&Inspection) -> Vec<CliStep>) -> Result<(), String> {
    let installer = app.state::<InstallerState>();
    if let Some(error) = installer.marketplace_error.lock().unwrap().clone() {
        return Err(error);
    }
    let cli = cli(app).ok_or_else(|| "claude コマンドが見つかりません".to_string())?;
    let inspection = inspect(app, &cli, path)?;
    let plan = steps(&inspection);
    if plan.is_empty() {
        return Ok(());
    }
    installer.busy.lock().unwrap().insert(path.to_string());
    publish(app);
    let bundled = paths::marketplace_dir(&app.state::<AppState>().home);
    let result = plan.iter().try_for_each(|step| cli.run(Path::new(path), &step.args(&bundled), CLI_TIMEOUT).map(|_| ()));
    installer.busy.lock().unwrap().remove(path);
    let _ = inspect(app, &cli, path);
    result
}

fn inspect(app: &AppHandle, cli: &ClaudeCli, path: &str) -> Result<Inspection, String> {
    let bundled = paths::marketplace_dir(&app.state::<AppState>().home);
    let to_args = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    let result = cli
        .run(Path::new(path), &to_args(&INSPECT_PLUGINS), CLI_TIMEOUT)
        .and_then(|plugins| {
            let marketplaces = cli.run(Path::new(path), &to_args(&INSPECT_MARKETPLACES), CLI_TIMEOUT)?;
            parse_inspection(&plugins, &marketplaces, &bundled)
        });
    app.state::<InstallerState>().inspections.lock().unwrap().insert(path.to_string(), result.clone());
    result
}

fn refresh_all(app: &AppHandle) {
    resolve_claude(app);
    let Some(cli) = cli(app) else { return };
    for dir in config_dirs(app) {
        if Path::new(&dir).is_dir() {
            let _ = inspect(app, &cli, &dir);
        }
    }
}

fn resolve_claude(app: &AppHandle) {
    let configured = app.state::<AppState>().settings.lock().unwrap().claude_path.clone();
    let home = dirs::home_dir().unwrap_or_default();
    *app.state::<InstallerState>().claude.lock().unwrap() = find_claude(configured.as_deref(), &home, is_executable);
}

fn cli(app: &AppHandle) -> Option<ClaudeCli> {
    app.state::<InstallerState>().claude.lock().unwrap().clone().map(ClaudeCli::new)
}

fn config_dirs(app: &AppHandle) -> Vec<String> {
    app.state::<AppState>().settings.lock().unwrap().config_dirs.clone().unwrap_or_default()
}

fn display(path: &str) -> String {
    display_path(path, &dirs::home_dir().unwrap_or_default())
}

pub fn view(app: &AppHandle) -> SetupView {
    let installer = app.state::<InstallerState>();
    let inspections = installer.inspections.lock().unwrap();
    let busy = installer.busy.lock().unwrap();
    let dirs: Vec<ConfigDirView> = config_dirs(app)
        .into_iter()
        .map(|path| {
            let state = if !Path::new(&path).is_dir() {
                DirState::Missing
            } else {
                match inspections.get(&path) {
                    Some(Ok(inspection)) => inspection.state(),
                    _ => DirState::NotInstalled,
                }
            };
            ConfigDirView { display: display(&path), busy: busy.contains(&path), path, state }
        })
        .collect();
    let claude_found = installer.claude.lock().unwrap().is_some();
    let notifications_allowed = notifier::notifications_allowed();
    let states: Vec<DirState> = dirs.iter().map(|d| d.state).collect();
    SetupView { needs_attention: needs_attention(&states, claude_found, notifications_allowed), dirs, claude_found, notifications_allowed }
}

pub fn publish(app: &AppHandle) {
    let _ = app.emit("setup-updated", view(app));
}

pub fn alert(app: &AppHandle, message: &str) {
    let state = app.state::<AppState>();
    state.suppress_hide.store(true, Ordering::Relaxed);
    let handle = app.clone();
    app.dialog()
        .message(message)
        .title("agents-monitor")
        .kind(MessageDialogKind::Error)
        .show(move |_| handle.state::<AppState>().suppress_hide.store(false, Ordering::Relaxed));
}
```

`dirs` クレートは core の依存にあるが、アプリの `Cargo.toml` にはないので `dirs = "6"`（core と同じ版。既存のライブラリなので追加の了承は不要）を足す。

- [ ] **Step 7: コマンドを足す**（`app/src-tauri/src/commands.rs`、`main.rs` の `generate_handler!` に登録）

```rust
#[tauri::command]
pub fn get_setup(app: AppHandle) -> installer::SetupView {
    installer::view(&app)
}

#[tauri::command]
pub fn add_config_dir(app: AppHandle) {
    let state = app.state::<AppState>();
    state.suppress_hide.store(true, Ordering::Relaxed);
    let mut dialog = app.dialog().file().set_title("Claude Code の設定ディレクトリを選ぶ");
    if let Some(window) = app.get_webview_window("list") {
        dialog = dialog.set_parent(&window);
    }
    let handle = app.clone();
    dialog.pick_folder(move |folder| {
        handle.state::<AppState>().suppress_hide.store(false, Ordering::Relaxed);
        if let Some(path) = folder.and_then(|f| f.into_path().ok()) {
            installer::send(&handle, installer::Job::Add(path.to_string_lossy().into_owned()));
        }
    });
}

#[tauri::command]
pub fn retry_config_dir(app: AppHandle, path: String) {
    installer::send(&app, installer::Job::Retry(path));
}

#[tauri::command]
pub fn remove_config_dir(app: AppHandle, path: String) {
    installer::send(&app, installer::Job::Remove(path));
}

#[tauri::command]
pub fn set_claude_path(app: AppHandle, state: State<'_, AppState>, path: String) -> Result<(), String> {
    let trimmed = path.trim();
    if !is_executable(Path::new(trimmed)) {
        return Err("そのパスに実行できる claude が見つかりません".into());
    }
    {
        let mut settings = state.settings.lock().unwrap();
        settings.claude_path = Some(trimmed.to_string());
        save_json(&paths::settings_path(&state.home), &*settings).map_err(|error| format!("設定を保存できませんでした: {error}"))?;
    }
    installer::send(&app, installer::Job::Refresh);
    Ok(())
}

#[tauri::command]
pub fn refresh_setup(app: AppHandle) {
    installer::send(&app, installer::Job::Refresh);
}
```

`main.rs` で `.manage(installer::InstallerState::default())` を足し、`setup` の最後で `installer::spawn(app.handle().clone());` を呼ぶ。

- [ ] **Step 8: テストとビルド**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets`
Expected: PASS、警告なし。`tauri_plugin_dialog` の API 名（`set_parent`・`pick_folder`・`into_path`・`message(...).kind(...).show(...)`）がコンパイルで合わなければ、`~/.cargo/registry/src/*/tauri-plugin-dialog-2.*/src/lib.rs` を読んで合わせる

- [ ] **Step 9: 手動確認（設計書 10 章の残り）**

`npm run tauri dev`（`app/` で）で次を確かめ、`docs/superpowers/notes/2026-10-11-v0.2-cli-findings.md` に追記する。
- 「追加」でフォルダーを選ぶ画面が一覧ウィンドウに付いて開き、選んだ後も一覧ウィンドウが開いたまま
- 選ぶのをやめても一覧ウィンドウが開いたまま
- アラートを閉じた後も一覧ウィンドウが開いたまま

- [ ] **Step 10: コミット**

```bash
git add crates/core/src/config_dirs.rs crates/core/src/attention.rs crates/core/src/lib.rs app/src-tauri docs/superpowers/notes/2026-10-11-v0.2-cli-findings.md Cargo.lock
git commit -m "起動時に ~/.claude へプラグインを入れ、設定ディレクトリの追加・もう一度・削除を裏で実行する"
```

---

### Task 9: 通知の許可状態を調べ、システム設定を開く（アプリ）

**Files:**
- Modify: `app/src-tauri/src/notifier.rs`, `app/src-tauri/Cargo.toml`, `app/src-tauri/src/commands.rs`, `app/src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `installer::publish`（Task 8）
- Produces: `notifier::notifications_allowed() -> bool`、`notifier::refresh_authorization(on_done: impl Fn() + Send + 'static)`、コマンド `open_notification_settings()`、`refresh_notification_access()`

- [ ] **Step 1: 許可状態を持つ**（`notifier.rs` の `imp`）

`Cargo.toml` の `objc2-user-notifications` の features に `"UNNotificationSettings"` を足す。`imp` に次を足す。

```rust
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use objc2_user_notifications::{UNAuthorizationStatus, UNNotificationSettings};

static ALLOWED: AtomicBool = AtomicBool::new(false);

pub fn notifications_allowed() -> bool {
    ALLOWED.load(Ordering::Relaxed)
}

pub fn refresh_authorization(on_done: impl Fn() + Send + 'static) {
    let Some(center) = notification_center() else { return };
    let block = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
        let status = unsafe { settings.as_ref() }.authorizationStatus();
        let allowed = [UNAuthorizationStatus::Authorized, UNAuthorizationStatus::Provisional, UNAuthorizationStatus::Ephemeral].contains(&status);
        ALLOWED.store(allowed, Ordering::Relaxed);
        on_done();
    });
    center.getNotificationSettingsWithCompletionHandler(&block);
}
```

`init` は、許可の答えが出たら許可状態を読み直して画面に知らせるよう、引数を受け取る形に変える（`on_answer` の中身だけを差し替える）。

```rust
pub fn init(on_change: impl Fn() + Send + Sync + 'static) {
    // ……（delegate の設定は今のまま）……
    let on_change = std::sync::Arc::new(on_change);
    let on_answer = RcBlock::new(move |granted: Bool, error: *mut NSError| {
        if let Some(error) = unsafe { error.as_ref() } {
            eprintln!("通知の許可を求められませんでした: {}", error.localizedDescription());
        } else if !granted.as_bool() {
            eprintln!("通知が許可されていません");
        }
        let on_change = on_change.clone();
        refresh_authorization(move || on_change());
    });
    center.requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &on_answer,
    );
}
```

バンドルとして起動していないとき（`notification_center()` が `None`）は、今と同じく何もせずに戻る。

macOS 以外の `imp` には `pub fn init(_on_change: impl Fn() + Send + Sync + 'static) {}`、`pub fn notifications_allowed() -> bool { true }` と `pub fn refresh_authorization(on_done: impl Fn() + Send + 'static) { on_done() }` を置き、`pub use imp::{init, show, notifications_allowed, refresh_authorization};` にする。`UNAuthorizationStatus` が `PartialEq` を実装していなければ、`status.0` の整数で比べる。

- [ ] **Step 2: システム設定を開くコマンドと、許可状態を読み直すコマンド**（`commands.rs`）

```rust
#[tauri::command]
pub fn open_notification_settings(app: AppHandle) {
    let url = format!("x-apple.systempreferences:com.apple.Notifications-Settings.extension?id={}", app.config().identifier);
    if let Err(error) = std::process::Command::new("open").arg(url).spawn() {
        eprintln!("システム設定を開けませんでした: {error}");
    }
}

#[tauri::command]
pub fn refresh_notification_access(app: AppHandle) {
    let handle = app.clone();
    notifier::refresh_authorization(move || installer::publish(&handle));
}
```

`main.rs` の `notifier::init()` を、許可の答えが出たら `installer::publish` するように呼ぶ（`let handle = app.handle().clone(); notifier::init(move || installer::publish(&handle));`）。

- [ ] **Step 3: ビルドと手動確認**

Run: `cargo clippy --workspace --all-targets && ./scripts/build-hook-sidecar.sh && (cd app && npm run tauri build -- --bundles app)`
パッケージ版で確かめ、`docs/superpowers/notes/2026-10-11-v0.2-cli-findings.md` に追記する。
- システム設定で agents-monitor の通知をオフにして一覧を開き直すと、`get_setup` の `notifications_allowed` が false になる（Task 10 の画面ではバッジと案内が出る）
- `open_notification_settings` で、システム設定の agents-monitor の通知の画面が開く
- 設計書 10 章: 新しいユーザーのつもりで、システム設定から agents-monitor を消せない場合は `tccutil reset All io.github.imatomix.agentsmonitor` を試してから起動し、許可の画面が出るかを記録する

- [ ] **Step 4: コミット**

```bash
git add app/src-tauri/src/notifier.rs app/src-tauri/Cargo.toml app/src-tauri/src/commands.rs app/src-tauri/src/main.rs docs/superpowers/notes/2026-10-11-v0.2-cli-findings.md Cargo.lock
git commit -m "通知の許可状態を調べ、システム設定の通知の画面を開けるようにする"
```

---

### Task 10: 設定画面の「Claude Code の設定」と、設定アイコンのバッジ（画面）

見た目は Figma（ファイルのキーは非公開）の「v0.2 下書き/修正案」（`119:752`）と、更新されたコンポーネント（`AgentMenu` の `Page=Page3`、`Icons` の `settings`・`delete`・`replay`、`TabBar` のバッジ）に合わせる。始める前に `get_design_context` で `119:753`・`119:777` と `Page=Page3`（`76:2843`）を読み、余白・文字の大きさ・並びを確かめる。

**Files:**
- Create: `app/src/icons.ts`
- Modify: `app/index.html`, `app/src/settings.ts`, `app/src/list.ts`, `app/src/types.ts`, `app/src/style.css`

**Interfaces:**
- Consumes: `get_setup`・`add_config_dir`・`retry_config_dir`・`remove_config_dir`・`set_claude_path`・`refresh_setup`・`open_notification_settings`・`refresh_notification_access`、イベント `setup-updated`・`open-settings`（Task 8・9）
- Produces: なし（最後の画面）

- [ ] **Step 1: 型を足す**（`app/src/types.ts`）

```ts
export type DirState = "installed" | "not_installed" | "missing";

export interface ConfigDirView {
  path: string;
  display: string;
  state: DirState;
  busy: boolean;
}

export interface SetupView {
  dirs: ConfigDirView[];
  claude_found: boolean;
  notifications_allowed: boolean;
  needs_attention: boolean;
}
```

- [ ] **Step 2: アイコンを置く**（`app/src/icons.ts`。Figma の `Icons` から書き出したパス。マスクは外し、色は `currentColor`）

```ts
const svg = (path: string) =>
  `<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="${path}"/></svg>`;

export const ICON_SETTINGS = svg(
  "M9.25001 22L8.85001 18.8C8.63335 18.7167 8.42918 18.6167 8.23751 18.5C8.04585 18.3833 7.85835 18.2583 7.67501 18.125L4.70001 19.375L1.95001 14.625L4.52501 12.675C4.50835 12.5583 4.50001 12.4458 4.50001 12.3375V11.6625C4.50001 11.5542 4.50835 11.4417 4.52501 11.325L1.95001 9.375L4.70001 4.625L7.67501 5.875C7.85835 5.74167 8.05001 5.61667 8.25001 5.5C8.45001 5.38333 8.65001 5.28333 8.85001 5.2L9.25001 2H14.75L15.15 5.2C15.3667 5.28333 15.5708 5.38333 15.7625 5.5C15.9542 5.61667 16.1417 5.74167 16.325 5.875L19.3 4.625L22.05 9.375L19.475 11.325C19.4917 11.4417 19.5 11.5542 19.5 11.6625V12.3375C19.5 12.4458 19.4833 12.5583 19.45 12.675L22.025 14.625L19.275 19.375L16.325 18.125C16.1417 18.2583 15.95 18.3833 15.75 18.5C15.55 18.6167 15.35 18.7167 15.15 18.8L14.75 22H9.25001ZM11 20H12.975L13.325 17.35C13.8417 17.2167 14.3208 17.0208 14.7625 16.7625C15.2042 16.5042 15.6083 16.1917 15.975 15.825L18.45 16.85L19.425 15.15L17.275 13.525C17.3583 13.2917 17.4167 13.0458 17.45 12.7875C17.4833 12.5292 17.5 12.2667 17.5 12C17.5 11.7333 17.4833 11.4708 17.45 11.2125C17.4167 10.9542 17.3583 10.7083 17.275 10.475L19.425 8.85L18.45 7.15L15.975 8.2C15.6083 7.81667 15.2042 7.49583 14.7625 7.2375C14.3208 6.97917 13.8417 6.78333 13.325 6.65L13 4H11.025L10.675 6.65C10.1583 6.78333 9.67918 6.97917 9.23751 7.2375C8.79585 7.49583 8.39168 7.80833 8.02501 8.175L5.55001 7.15L4.57501 8.85L6.72501 10.45C6.64168 10.7 6.58335 10.95 6.55001 11.2C6.51668 11.45 6.50001 11.7167 6.50001 12C6.50001 12.2667 6.51668 12.525 6.55001 12.775C6.58335 13.025 6.64168 13.275 6.72501 13.525L4.57501 15.15L5.55001 16.85L8.02501 15.8C8.39168 16.1833 8.79585 16.5042 9.23751 16.7625C9.67918 17.0208 10.1583 17.2167 10.675 17.35L11 20ZM12.05 15.5C13.0167 15.5 13.8417 15.1583 14.525 14.475C15.2083 13.7917 15.55 12.9667 15.55 12C15.55 11.0333 15.2083 10.2083 14.525 9.525C13.8417 8.84167 13.0167 8.5 12.05 8.5C11.0667 8.5 10.2375 8.84167 9.56251 9.525C8.88751 10.2083 8.55001 11.0333 8.55001 12C8.55001 12.9667 8.88751 13.7917 9.56251 14.475C10.2375 15.1583 11.0667 15.5 12.05 15.5Z",
);

export const ICON_DELETE = svg(
  "M7 21C6.45 21 5.97917 20.8042 5.5875 20.4125C5.19583 20.0208 5 19.55 5 19V6H4V4H9V3H15V4H20V6H19V19C19 19.55 18.8042 20.0208 18.4125 20.4125C18.0208 20.8042 17.55 21 17 21H7ZM17 6H7V19H17V6ZM9 17H11V8H9V17ZM13 17H15V8H13V17Z",
);

export const ICON_REPLAY = svg(
  "M8.4875 21.2875C7.39583 20.8125 6.44583 20.1708 5.6375 19.3625C4.82917 18.5542 4.1875 17.6042 3.7125 16.5125C3.2375 15.4208 3 14.25 3 13H5C5 14.95 5.67917 16.6042 7.0375 17.9625C8.39583 19.3208 10.05 20 12 20C13.95 20 15.6042 19.3208 16.9625 17.9625C18.3208 16.6042 19 14.95 19 13C19 11.05 18.3208 9.39583 16.9625 8.0375C15.6042 6.67917 13.95 6 12 6H11.85L13.4 7.55L12 9L8 5L12 1L13.4 2.45L11.85 4H12C13.25 4 14.4208 4.2375 15.5125 4.7125C16.6042 5.1875 17.5542 5.82917 18.3625 6.6375C19.1708 7.44583 19.8125 8.39583 20.2875 9.4875C20.7625 10.5792 21 11.75 21 13C21 14.25 20.7625 15.4208 20.2875 16.5125C19.8125 17.6042 19.1708 18.5542 18.3625 19.3625C17.5542 20.1708 16.6042 20.8125 15.5125 21.2875C14.4208 21.7625 13.25 22 12 22C10.75 22 9.57917 21.7625 8.4875 21.2875Z",
);
```

アイコンは固定の文字列なので `innerHTML` で入れてよい（任意の文字列は入れない）。

- [ ] **Step 3: HTML を足す**（`app/index.html`）

- タブの設定ボタン（`data-page="menu"`）の中、SVG の後ろに `<span id="settings-badge" class="badge" hidden></span>` を足す
- 「通知設定」の見出しのすぐ下（`#notify-options` の前）に次を足す

```html
<div id="notify-off" class="menu-item" hidden>
  <span class="grow">通知がオフになっています</span>
  <button id="open-notification-settings" class="icon-button" type="button" title="システム設定を開く" aria-label="システム設定を開く"></button>
</div>
```

- 通知設定の `menu-section` と、ログイン時に起動する の `menu-section` の間に次を足す

```html
<div class="menu-section">
  <h2 class="menu-heading">Claude Code の設定</h2>
  <p id="config-dirs-empty" class="menu-note" hidden>Claude Code の設定ディレクトリが見つかりませんでした。使用している設定ディレクトリを追加してください。</p>
  <ul id="config-dirs" class="config-dirs"></ul>
  <form id="claude-path-form" class="claude-path" hidden>
    <p class="menu-note">claude コマンドが見つかりません。場所を入力してください。</p>
    <input id="claude-path" class="text-input" type="text" placeholder="/Users/you/.local/bin/claude" />
  </form>
  <button id="add-config-dir" class="menu-item centered" type="button">＋ 設定ディレクトリを追加</button>
  <p id="setup-error" class="menu-note" hidden></p>
</div>
```

- [ ] **Step 4: 描画する**（`app/src/settings.ts` に足す）

```ts
import { listen } from "@tauri-apps/api/event";
import { ICON_DELETE, ICON_REPLAY, ICON_SETTINGS } from "./icons";
import type { ConfigDirView, SetupView } from "./types";

const dirsEl = document.getElementById("config-dirs")!;
const dirsEmptyEl = document.getElementById("config-dirs-empty")!;
const notifyOffEl = document.getElementById("notify-off")!;
const badgeEl = document.getElementById("settings-badge")!;
const claudeForm = document.getElementById("claude-path-form") as HTMLFormElement;
const claudeInput = document.getElementById("claude-path") as HTMLInputElement;
const setupErrorEl = document.getElementById("setup-error")!;

function iconButton(icon: string, label: string, disabled: boolean, onClick: () => void): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "icon-button";
  button.title = label;
  button.setAttribute("aria-label", label);
  button.innerHTML = icon;
  button.disabled = disabled;
  button.addEventListener("click", onClick);
  return button;
}

function renderDir(dir: ConfigDirView): HTMLElement {
  const item = document.createElement("li");
  item.className = "menu-item";
  const name = document.createElement("span");
  name.className = "grow path";
  name.textContent = dir.display;
  name.title = dir.path;
  item.append(name);
  if (dir.state === "not_installed") {
    item.append(iconButton(ICON_REPLAY, "もう一度", dir.busy, () => void invoke("retry_config_dir", { path: dir.path })));
  }
  item.append(iconButton(ICON_DELETE, "削除", dir.busy, () => void invoke("remove_config_dir", { path: dir.path })));
  return item;
}

function renderSetup(setup: SetupView): void {
  badgeEl.hidden = !setup.needs_attention;
  notifyOffEl.hidden = setup.notifications_allowed;
  dirsEmptyEl.hidden = setup.dirs.length > 0;
  dirsEl.replaceChildren(...setup.dirs.map(renderDir));
  claudeForm.hidden = setup.claude_found;
}

export function setupClaudeSection(): void {
  document.getElementById("open-notification-settings")!.innerHTML = ICON_SETTINGS;
  document.getElementById("open-notification-settings")!.addEventListener("click", () => void invoke("open_notification_settings"));
  document.getElementById("add-config-dir")!.addEventListener("click", () => void invoke("add_config_dir"));
  claudeForm.addEventListener("submit", (event) => {
    event.preventDefault();
    invoke("set_claude_path", { path: claudeInput.value })
      .then(() => {
        setupErrorEl.hidden = true;
      })
      .catch((error) => {
        setupErrorEl.hidden = false;
        setupErrorEl.textContent = String(error);
      });
  });
  void listen<SetupView>("setup-updated", (event) => renderSetup(event.payload));
  void invoke<SetupView>("get_setup").then(renderSetup);
}
```

- [ ] **Step 5: 一覧に組み込む**（`app/src/list.ts`）

- `setupSettingsMenu();` の後に `setupClaudeSection();` を呼ぶ（`import { setupClaudeSection, setupSettingsMenu } from "./settings";`）
- 起動時に `~/.claude` が見つからないときに設定ページで開く: `void listen("open-settings", () => showPage("menu"));`
- 起動時に `~/.claude` が見つからないときは設定ページのまま開きたいが、今はフォーカスを得るたびに `agents` に戻している。`open-settings` を受けたら、次の 1 回のフォーカスでは戻さないようにする。`onFocusChanged` と `open-settings` の処理を次にする（上の `listen("open-settings", ...)` はこれに置き換える）

```ts
let keepPageOnNextFocus = false;

void listen("open-settings", () => {
  keepPageOnNextFocus = true;
  showPage("menu");
});

void getCurrentWindow().onFocusChanged(({ payload: focused }) => {
  if (!focused) return;
  if (keepPageOnNextFocus) keepPageOnNextFocus = false;
  else showPage("agents");
  void invoke("refresh_notification_access");
  void invoke("refresh_setup");
});
```

- [ ] **Step 6: 見た目を足す**（`app/src/style.css`。色は前景色だけ。バッジだけ `--error`）

```css
.icon-tab { position: relative; }
.badge { position: absolute; top: 0; right: 0; width: 8px; height: 8px; border-radius: 50%; background: var(--error); }
.badge[hidden], .menu-item[hidden], .menu-note[hidden], .claude-path[hidden] { display: none; }
.grow { flex: 1; min-width: 0; }
.path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.config-dirs { list-style: none; margin: 0; padding: 0; }
.icon-button { display: flex; align-items: center; justify-content: center; padding: 4px; border-radius: 8px; color: var(--fg); }
.icon-button:hover:not(:disabled) { background: var(--bg-hover); }
.icon-button:disabled { opacity: 0.4; }
.menu-item.centered { justify-content: center; }
.menu-note { margin: 0; padding: 4px 8px; font-size: var(--font-sm); color: var(--fg-secondary); }
.claude-path { display: flex; flex-direction: column; gap: 4px; }
.text-input { margin: 0 8px; padding: 4px 8px; border: 1px solid var(--border-primary); border-radius: 8px; background: var(--bg); color: var(--fg); font: inherit; }
```

既存の `.error-text`（`color: var(--error)`）は文字に状態色を使っているので、`color: var(--fg-secondary)` に変える。

- [ ] **Step 7: ビルドと手動確認**

Run: `(cd app && npm run build) && ./scripts/build-hook-sidecar.sh && (cd app && npm run tauri build -- --bundles app)` → アプリを立ち上げ直す
Figma の「v0.2 下書き/修正案」と並べて確かめること:
- 通知がオンのとき「通知がオフになっています」の行が出ない。オフにすると出て、歯車でシステム設定が開く。歯車に乗せると「システム設定を開く」と出る
- 開発者の環境（`~/.claude` と `~/.claude-personal`、どちらも開発中のフォルダーから入っている）で、`~/.claude` が削除アイコンだけで並ぶ
- `~/.claude-personal` を追加すると一覧に足され、もう一度アイコンが出ない（別の場所から入っているので「入っている」）
- `claude plugin disable` した設定ディレクトリでは、もう一度アイコンが出て、押すと有効に戻る
- 実行中はその行のアイコンが押せない
- 問題がないときはバッジが出ず、通知をオフにするとバッジが出る
- 文字色に状態色が使われていない

- [ ] **Step 8: コミット**

```bash
git add app/index.html app/src
git commit -m "設定画面に Claude Code の設定と通知オフの案内を足し、手を打つべきときは設定アイコンにバッジを付ける"
```

---

### Task 11: 本物の `claude` を使う結合テスト

**Files:**
- Create: `crates/core/tests/integration_install.rs`
- Create: `scripts/integration-test.sh`

**Interfaces:**
- Consumes: `ClaudeCli`・`find_claude`・`is_executable`（Task 5）、`parse_inspection`・`DirState`（Task 6）、`install_steps`・`remove_steps`・`INSPECT_*`（Task 6）、`sync_marketplace`（Task 7）

- [ ] **Step 1: テストを書く**（`crates/core/tests/integration_install.rs`）

```rust
use std::fs;
use std::path::Path;
use std::time::Duration;

use agents_monitor_core::claude_cli::{find_claude, is_executable, ClaudeCli, CLI_TIMEOUT};
use agents_monitor_core::install_plan::{install_steps, remove_steps, INSPECT_MARKETPLACES, INSPECT_PLUGINS};
use agents_monitor_core::marketplace::sync_marketplace;
use agents_monitor_core::plugin_status::{parse_inspection, DirState, Inspection};

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| w.to_string()).collect()
}

fn inspect(cli: &ClaudeCli, config: &Path, bundled: &Path) -> Inspection {
    let plugins = cli.run(config, &words(&INSPECT_PLUGINS), CLI_TIMEOUT).unwrap();
    let marketplaces = cli.run(config, &words(&INSPECT_MARKETPLACES), CLI_TIMEOUT).unwrap();
    parse_inspection(&plugins, &marketplaces, bundled).unwrap()
}

#[test]
#[ignore = "本物の claude を使う。scripts/integration-test.sh から動かす"]
fn installs_receives_events_and_removes() {
    let home = dirs::home_dir().unwrap();
    let cli = ClaudeCli::new(find_claude(None, &home, is_executable).expect("claude が見つからない"));
    let work = tempfile::tempdir().unwrap();
    let config = work.path().join("config");
    let monitor_home = work.path().join("monitor");
    let bundled = monitor_home.join("marketplace");
    fs::create_dir_all(&config).unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    fs::create_dir_all(bundled.join(".claude-plugin")).unwrap();
    fs::copy(repo.join(".claude-plugin/marketplace.json"), bundled.join(".claude-plugin/marketplace.json")).unwrap();
    sync_marketplace(&repo.join("plugin"), &bundled.join("plugin")).unwrap();

    let before = inspect(&cli, &config, &bundled);
    assert_eq!(before.state(), DirState::NotInstalled);
    for step in install_steps(&before) {
        cli.run(&config, &step.args(&bundled), CLI_TIMEOUT).unwrap();
    }
    let installed = inspect(&cli, &config, &bundled);
    assert_eq!(installed.state(), DirState::Installed);

    if std::env::var_os("ANTHROPIC_API_KEY").is_some() || std::env::var_os("CLAUDE_CODE_OAUTH_TOKEN").is_some() {
        std::env::set_var("AGENTS_MONITOR_HOME", &monitor_home);
        cli.run(&config, &words(&["-p", "「はい」とだけ答えて"]), Duration::from_secs(120)).unwrap();
        let events = fs::read_dir(monitor_home.join("events")).map(|d| d.count()).unwrap_or(0);
        assert!(events > 0, "イベントファイルが届いていない（~/.agents-monitor/bin/agents-monitor-hook があるか確かめる）");
    } else {
        eprintln!("ANTHROPIC_API_KEY も CLAUDE_CODE_OAUTH_TOKEN もないので、claude -p でのイベントの確認は飛ばします");
    }

    for step in remove_steps(&installed) {
        cli.run(&config, &step.args(&bundled), CLI_TIMEOUT).unwrap();
    }
    assert_eq!(inspect(&cli, &config, &bundled).state(), DirState::NotInstalled);
}
```

`ClaudeCli::run` は子プロセスに自分の環境変数を引き継ぐので、`AGENTS_MONITOR_HOME` を `set_var` で設定すれば hook に届く（hook は `$HOME/.agents-monitor/bin` にある実物を使い、書き出し先だけ一時ディレクトリになる）。core の `[dev-dependencies]` に `dirs` は通常の依存としてすでにあるので追加はいらない。

- [ ] **Step 2: スクリプトを書く**（`scripts/integration-test.sh`、実行権限を付ける）

```bash
#!/usr/bin/env bash
# 本物の claude を使い、一時的な設定ディレクトリでプラグインを入れて外す。
# ANTHROPIC_API_KEY か CLAUDE_CODE_OAUTH_TOKEN があれば claude -p を 1 回動かしてイベントが届くことも確かめる
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/dev-install-hook.sh
cargo test -p agents-monitor-core --test integration_install -- --ignored --nocapture
```

- [ ] **Step 3: 動かす**

Run: `./scripts/integration-test.sh`
Expected: PASS（API キーがなければ「飛ばします」と出て、導入と削除だけ確かめて PASS）

- [ ] **Step 4: コミット**

```bash
git add crates/core/tests/integration_install.rs scripts/integration-test.sh
git commit -m "本物の claude でプラグインの導入と削除、イベントの到着を確かめる結合テストを足す"
```

---

### Task 12: README と仕上げの確認

**Files:**
- Modify: `README.md`

- [ ] **Step 1: README の「導入（v0.1）」を書き換える**

```markdown
## 導入

1. アプリを起動する。`~/.claude` があれば、プラグインが自動で入ります
2. 動いている Claude Code の会話を開き直す（`claude --resume` や、VS Code の会話の履歴から開く）。開き直すまでは Unknown タブに「状態不明」として出ます

`~/.claude` 以外の設定ディレクトリ（`CLAUDE_CONFIG_DIR` で分けている場合）は、一覧の ≡ →「Claude Code の設定」→「＋ 設定ディレクトリを追加」で選んでください。外したいときは、その行の削除のアイコンを押します。

設定アイコンに赤い丸が付いているときは、通知がオフになっている、プラグインが入っていない設定ディレクトリがある、などの確認が必要な状態です。
```

「開発」の節に `./scripts/integration-test.sh    # 本物の claude を使う結合テスト` を足す。並び替えについて「一覧の行はドラッグで並び替えられます。会話を開き直しても元の位置に戻ります」を「状態の見方」の後に足す。

- [ ] **Step 2: すべてのテストと手動確認**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets && (cd app && npm run build) && ./scripts/integration-test.sh`
Expected: すべて PASS

設計書 9 章の手動確認をパッケージ版で行う。
- 新しいユーザーのつもりで（`~/.agents-monitor/settings.json` の `config_dirs` を消し、`~/.claude` から agents-monitor を外してから）起動し、`~/.claude` に入ること
- `HOME=$(mktemp -d) open target/release/bundle/macos/agents-monitor.app` のように `~/.claude` がない状態で起動し、設定画面が開いて「見つかりませんでした」の案内が出ること（`open` は環境変数を渡さないので、`Contents/MacOS/agents-monitor-app` を直接起動する）
- 追加・削除・もう一度、失敗したときのアラート（組織の管理設定がなければ、`claude` の場所を壊れたスクリプトにして試す）、`~/.claude` を削除した後の再起動で入れ直されないこと
- 通知がオフのときのバッジと案内
- ドラッグでの並び替えと、再起動・会話の再開の後も順が保たれること
- `plugin/.claude-plugin/plugin.json` の版を一時的に上げてアプリを作り直し、同梱版から入れた設定ディレクトリで版が上がること（確かめたら戻す）

- [ ] **Step 3: コミット**

```bash
git add README.md
git commit -m "README の導入手順を v0.2 に合わせる"
```
