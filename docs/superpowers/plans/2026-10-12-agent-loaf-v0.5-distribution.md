# Agent Loaf v0.5（配布と公開）実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** アプリの中の自動更新（知らせて、押したら更新・再起動）を入れ、GitHub Actions で署名・公証済みの dmg と更新の情報を Releases の下書きに上げる仕組みを作り、リポジトリを `imatomix/agent-loaf` として公開して v0.5.0 を出す。

**Architecture:** 更新の状態の移り変わりと確かめる時刻の判断は core の純粋な関数（`update.rs`）に置く。アプリは `tauri-plugin-updater` で確かめ・ダウンロード・入れ替えを行い、監視の 1 秒ごとの処理の中で時刻を見て確かめる。確かめる処理は、作るときに `AGENT_LOAF_RELEASE=1` を渡した公開のビルドでだけ動く。公開は `v*` のタグで動く GitHub Actions（`tauri-action`）が行う。

**Tech Stack:** Rust、Tauri 2、`tauri-plugin-updater`（ユーザーと確認済みの新しいライブラリ）、Vite + TypeScript、GitHub Actions（`tauri-apps/tauri-action`）、Apple の notarytool（Tauri の bundler が使う）

**Spec:** `docs/superpowers/specs/2026-10-12-agent-loaf-v0.5-distribution-design.md`

## Global Constraints

- 足してよいライブラリは `tauri-plugin-updater` だけ。再起動は Tauri 本体の `app.restart()` を使う
- 確かめる頻度: 起動の約 1 分後に 1 回、その後は 24 時間ごと。確かめるのは公開のビルド（作るときに `AGENT_LOAF_RELEASE=1`）だけ
- 更新は知らせて、押したら更新して再起動する。勝手に入れ替えない
- ≡ のバッジは更新では付けない
- Apple Silicon だけ（`aarch64-apple-darwin`）、macOS 14 以降
- 最初の公開の版は v0.5.0
- 画面の文字色に状態色を使わない。任意の文字列は `textContent` で入れる。Figma（`150:1053`）にない見た目を足さない
- コミットメッセージは日本語。末尾に `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`。コメントは WHY が非自明なときだけ（日本語）
- push・リポジトリの公開・Releases の公開は、ユーザーの指示があってから行う
- 秘密鍵・証明書・パスワードはリポジトリに入れない。会話にも貼ってもらわない（Secrets には、ユーザーが自分で入れる）

## Review Focus

1. **Mac をスリープしていて 24 時間を過ぎた**→ 戻った直後の 1 秒ごとの処理で確かめる（Task 1 の `is_due` のテスト。時刻は壁時計で比べる）
2. **オフラインで起動・確かめに失敗した**→ 何も表示せず「なし」のまま、次の回にまた確かめる（Task 1 のテスト）
3. **「アップデートして再起動」を続けて押す・ダウンロード中に一覧を閉じて開く**→ ダウンロードは 1 つだけ、開き直した画面に今の進み具合が出る（Task 2 の二重起動の防止、Task 3 の `get_update`）
4. **ダウンロードが途中で失敗した・署名が合わない**→ 「失敗」と「もう一度」が出て、今の版のまま動き続ける（Task 1 のテスト、Task 8 の実機確認）
5. **開発のビルド（`tauri dev`、手元の `tauri build`）**→ 確かめに行かず、更新のプラグインの設定がなくても起動する（Task 2 の `option_env!` で登録しない、Task 8 で確認）

---

## ファイル構成

| ファイル | 役割 | Task |
|---|---|---|
| `app/src-tauri/src/migration.rs`, `crates/core/src/legacy.rs`（削除）, README | 旧名からの移行をなくす | 0 |
| `crates/core/src/update.rs`（新規） | 更新の状態（`UpdateState`）と移り変わり（`UpdateEvent`）、確かめる時刻（`is_due`） | 1 |
| `crates/core/src/lib.rs` | `pub mod update;` | 1 |
| `app/src-tauri/src/updater.rs`（新規） | 確かめる・ダウンロードする・再起動する。状態を画面に知らせる | 2 |
| `app/src-tauri/Cargo.toml` | `tauri-plugin-updater` | 2 |
| `app/src-tauri/src/main.rs`, `monitor.rs`, `commands.rs` | プラグインの登録（公開のビルドだけ）、1 秒ごとの処理から確かめる、`get_update`・`install_update` | 2 |
| `app/index.html`, `app/src/settings.ts`, `app/src/types.ts`, `app/src/style.css` | 設定の画面の「アップデート」の欄 | 3 |
| `crates/core/locales/{ja,en}.json`, `crates/core/src/messages.rs` | 文言 | 3 |
| `app/src-tauri/tauri.conf.json`, `app/src-tauri/tests/versions.rs`, `window_config.rs` | macOS 14、更新の成果物、版 0.5.0 | 4 |
| `.github/workflows/release.yml`（新規） | タグで作って Releases の下書きに上げる | 5 |
| `docs/releasing.md`（新規）, `README.md`, `README.en.md`, `docs/superpowers/specs/2026-10-09-agents-monitor-design.md` | 文書 | 6 |

---

### Task 0: 旧名からの移行の処理と README の節を消す

**Files:**
- Modify（取り消し）: コミット `6d43275`（`app/src-tauri/src/migration.rs`、`crates/core/src/legacy.rs`、`installer.rs`、`main.rs`、`lib.rs`、`plugin_status.rs`）と `c28d3e6`（README の「agents-monitor から移るとき」）

agents-monitor の名前で使っていたのはユーザーの Mac だけで、その Mac の移行は済んでいる（2026-10-12、ユーザーと決定）。公開の後の利用者には関係がなく、README の節はかえって戸惑わせる。

- [ ] **Step 1: 2 つのコミットを取り消す**

```bash
git revert --no-edit c28d3e6 6d43275
```

ぶつかったら（`main.rs` の起動の順番や README の既知の制約は、その後の v0.4 の修正で変わっている）、その後の修正を残し、移行に関わる行だけを消す形で直してから `git revert --continue`。

- [ ] **Step 2: 確かめる**

Run: `cargo test --workspace && cd app && npx tsc --noEmit && cd .. && ! grep -rn "legacy\|migration\|agents-monitor から\|Moving from agents-monitor" crates/*/src app/src-tauri/src README.md README.en.md`
Expected: PASS・型エラーなし・grep は何も出さない

---

### Task 1: 更新の状態と確かめる時刻（core）

**Files:**
- Create: `crates/core/src/update.rs`
- Modify: `crates/core/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub enum UpdateState { Idle, Available { version: String }, Downloading { version: String, percent: u8 }, Failed { version: String } }`（`Serialize`、`#[serde(tag = "kind", rename_all = "snake_case")]`、`Clone`、`PartialEq`、`Debug`、`Default` は `Idle`）
  - `pub enum UpdateEvent { Found(String), NotFound, CheckFailed, Started, Progress { downloaded: u64, total: Option<u64> }, InstallFailed }`
  - `impl UpdateState { pub fn next(&self, event: &UpdateEvent) -> UpdateState }`
  - `pub const FIRST_CHECK_DELAY_MS: u64 = 60_000;`、`pub const CHECK_INTERVAL_MS: u64 = 24 * 60 * 60 * 1000;`
  - `pub fn is_due(now_ms: u64, started_ms: u64, last_check_ms: Option<u64>) -> bool`

- [ ] **Step 1: 失敗するテストを書く**

`crates/core/src/update.rs` を作る（テストだけ先に）。

```rust
use serde::Serialize;

#[cfg(test)]
mod tests {
    use super::*;

    fn available() -> UpdateState {
        UpdateState::Available { version: "0.5.1".into() }
    }

    #[test]
    fn found_version_becomes_available_and_quiet_failures_change_nothing() {
        assert_eq!(UpdateState::Idle.next(&UpdateEvent::Found("0.5.1".into())), available());
        assert_eq!(UpdateState::Idle.next(&UpdateEvent::NotFound), UpdateState::Idle);
        // オフラインなどで確かめられなかったときは、何も表示しない
        assert_eq!(UpdateState::Idle.next(&UpdateEvent::CheckFailed), UpdateState::Idle);
        assert_eq!(available().next(&UpdateEvent::CheckFailed), available(), "前に見つけた版の知らせは消さない");
    }

    #[test]
    fn download_reports_percent_and_failure_keeps_the_version() {
        let started = available().next(&UpdateEvent::Started);
        assert_eq!(started, UpdateState::Downloading { version: "0.5.1".into(), percent: 0 });
        let half = started.next(&UpdateEvent::Progress { downloaded: 50, total: Some(200) });
        assert_eq!(half, UpdateState::Downloading { version: "0.5.1".into(), percent: 25 });
        assert_eq!(half.next(&UpdateEvent::Progress { downloaded: 10, total: None }), half, "大きさが分からなければ % を変えない");
        assert_eq!(half.next(&UpdateEvent::Progress { downloaded: 999, total: Some(200) }), UpdateState::Downloading { version: "0.5.1".into(), percent: 100 });
        let failed = half.next(&UpdateEvent::InstallFailed);
        assert_eq!(failed, UpdateState::Failed { version: "0.5.1".into() });
        assert_eq!(failed.next(&UpdateEvent::Started), UpdateState::Downloading { version: "0.5.1".into(), percent: 0 }, "もう一度");
    }

    #[test]
    fn downloading_ignores_new_checks() {
        let downloading = UpdateState::Downloading { version: "0.5.1".into(), percent: 40 };
        assert_eq!(downloading.next(&UpdateEvent::Found("0.5.2".into())), downloading);
        assert_eq!(downloading.next(&UpdateEvent::NotFound), downloading);
        assert_eq!(UpdateState::Idle.next(&UpdateEvent::Started), UpdateState::Idle, "知らせていない版はダウンロードしない");
    }

    #[test]
    fn checks_a_minute_after_start_then_daily() {
        let start = 1_000_000;
        assert!(!is_due(start + FIRST_CHECK_DELAY_MS - 1, start, None));
        assert!(is_due(start + FIRST_CHECK_DELAY_MS, start, None));
        let last = start + FIRST_CHECK_DELAY_MS;
        assert!(!is_due(last + CHECK_INTERVAL_MS - 1, start, Some(last)));
        assert!(is_due(last + CHECK_INTERVAL_MS, start, Some(last)));
        // スリープで何日も止まっていても、戻ったときに 1 回だけ確かめればよい
        assert!(is_due(last + 3 * CHECK_INTERVAL_MS, start, Some(last)));
        // 時計を戻したときに、ずっと確かめなくならない
        assert!(is_due(last - CHECK_INTERVAL_MS, start, Some(last)));
    }

    #[test]
    fn serializes_for_the_page() {
        let json = serde_json::to_value(UpdateState::Downloading { version: "0.5.1".into(), percent: 40 }).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "downloading", "version": "0.5.1", "percent": 40 }));
        assert_eq!(serde_json::to_value(UpdateState::Idle).unwrap(), serde_json::json!({ "kind": "idle" }));
    }
}
```

`crates/core/src/lib.rs` に `pub mod update;` を足す（並び順は今のファイルに合わせる）。

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agent-loaf-core --lib update`
Expected: コンパイルエラー（`UpdateState` などがない）

- [ ] **Step 3: 実装する**

`crates/core/src/update.rs` の `use` の後に足す。

```rust
pub const FIRST_CHECK_DELAY_MS: u64 = 60_000;
pub const CHECK_INTERVAL_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UpdateState {
    #[default]
    Idle,
    Available { version: String },
    Downloading { version: String, percent: u8 },
    Failed { version: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateEvent {
    Found(String),
    NotFound,
    CheckFailed,
    Started,
    Progress { downloaded: u64, total: Option<u64> },
    InstallFailed,
}

impl UpdateState {
    fn version(&self) -> Option<&str> {
        match self {
            UpdateState::Idle => None,
            UpdateState::Available { version } | UpdateState::Downloading { version, .. } | UpdateState::Failed { version } => Some(version),
        }
    }

    pub fn next(&self, event: &UpdateEvent) -> UpdateState {
        let downloading = matches!(self, UpdateState::Downloading { .. });
        match event {
            // ダウンロードの途中に届いた確認の結果で、進み具合の表示を消さない
            _ if downloading && !matches!(event, UpdateEvent::Progress { .. } | UpdateEvent::InstallFailed) => self.clone(),
            UpdateEvent::Found(version) => UpdateState::Available { version: version.clone() },
            UpdateEvent::NotFound => UpdateState::Idle,
            UpdateEvent::CheckFailed => self.clone(),
            UpdateEvent::Started => match self.version() {
                Some(version) => UpdateState::Downloading { version: version.to_string(), percent: 0 },
                None => self.clone(),
            },
            UpdateEvent::Progress { downloaded, total } => match (self, total) {
                (UpdateState::Downloading { version, .. }, Some(total)) if *total > 0 => {
                    let percent = (downloaded.saturating_mul(100) / total).min(100) as u8;
                    UpdateState::Downloading { version: version.clone(), percent }
                }
                _ => self.clone(),
            },
            UpdateEvent::InstallFailed => match self.version() {
                Some(version) => UpdateState::Failed { version: version.to_string() },
                None => self.clone(),
            },
        }
    }
}

// 時刻は壁時計で比べる。スリープ中に止まる時計で数えると、何日も眠っていた Mac が戻っても確かめない
pub fn is_due(now_ms: u64, started_ms: u64, last_check_ms: Option<u64>) -> bool {
    match last_check_ms {
        None => now_ms >= started_ms + FIRST_CHECK_DELAY_MS,
        Some(last) => now_ms < last || now_ms - last >= CHECK_INTERVAL_MS,
    }
}
```

- [ ] **Step 4: 通ることを確かめる**

Run: `cargo test -p agent-loaf-core --lib update`
Expected: 5 件とも PASS。続けて `cargo test --workspace` も PASS

- [ ] **Step 5: コミット**

```bash
git add crates/core/src/update.rs crates/core/src/lib.rs
git commit -m "更新の状態の移り変わりと、確かめる時刻の判断を core に足す

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: アプリから確かめ、ダウンロードし、再起動する

**Files:**
- Create: `app/src-tauri/src/updater.rs`
- Modify: `app/src-tauri/Cargo.toml`, `app/src-tauri/src/main.rs`, `app/src-tauri/src/monitor.rs`, `app/src-tauri/src/commands.rs`

**Interfaces:**
- Consumes: Task 1 の `UpdateState`、`UpdateEvent`、`is_due`
- Produces:
  - `updater::ENABLED: bool`（`option_env!("AGENT_LOAF_RELEASE").is_some()`）
  - `updater::UpdaterState`（`Default`。`app.manage` する）
  - `updater::tick(app: &AppHandle, now_ms: u64)`（監視の 1 秒ごとの処理から呼ぶ）
  - `updater::current(app: &AppHandle) -> UpdateState`
  - `updater::install(app: AppHandle)`
  - イベント `"update-updated"`（中身は `UpdateState`）
  - コマンド `get_update() -> UpdateState`、`install_update()`

- [ ] **Step 1: ライブラリを足す**

`app/src-tauri/Cargo.toml` の `[dependencies]` に足す（ユーザーと確認済み）。

```toml
tauri-plugin-updater = "2"
```

Run: `cargo build -p agent-loaf-app`
Expected: ビルドが通る（まだ使っていないので警告は出てよい）

- [ ] **Step 2: 失敗するテストを書く**

公開のビルドかどうかの判断だけは、アプリの中でテストできる。`app/src-tauri/src/updater.rs` を作る（テストだけ先に）。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_builds_never_check() {
        // cargo test は AGENT_LOAF_RELEASE を付けずに作るので、確かめる処理は止まっている
        assert!(!ENABLED);
    }
}
```

`main.rs` に `mod updater;` を足す。

Run: `cargo test -p agent-loaf-app --bin agent-loaf-app updater`
Expected: コンパイルエラー（`ENABLED` がない）

- [ ] **Step 3: 実装する**

`app/src-tauri/src/updater.rs` のテストの前に足す。

```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use agent_loaf_core::paths;
use agent_loaf_core::update::{is_due, UpdateEvent, UpdateState};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::sync::LockExt;

// 作るときに AGENT_LOAF_RELEASE を渡した公開のビルドだけが確かめる。開発のビルドは更新の設定（鍵）がなくても動かす
pub const ENABLED: bool = option_env!("AGENT_LOAF_RELEASE").is_some();

pub struct UpdaterState {
    started_ms: u64,
    last_check_ms: Mutex<Option<u64>>,
    busy: AtomicBool,
    state: Mutex<UpdateState>,
    found: Mutex<Option<Update>>,
}

impl Default for UpdaterState {
    fn default() -> Self {
        UpdaterState { started_ms: paths::now_ms(), last_check_ms: Mutex::new(None), busy: AtomicBool::new(false), state: Mutex::default(), found: Mutex::new(None) }
    }
}

pub fn current(app: &AppHandle) -> UpdateState {
    app.state::<UpdaterState>().state.locked().clone()
}

fn apply(app: &AppHandle, event: UpdateEvent) {
    let next = {
        let mut state = app.state::<UpdaterState>().state.locked();
        let next = state.next(&event);
        if next == *state {
            return;
        }
        *state = next.clone();
        next
    };
    let _ = app.emit("update-updated", next);
}

// 監視の 1 秒ごとの処理から呼ぶ。確かめる時刻でなければ何もしないので、ふだんの負荷は時刻の比較だけ
pub fn tick(app: &AppHandle, now_ms: u64) {
    if !ENABLED {
        return;
    }
    let updater = app.state::<UpdaterState>();
    if !is_due(now_ms, updater.started_ms, *updater.last_check_ms.locked()) {
        return;
    }
    *updater.last_check_ms.locked() = Some(now_ms);
    if updater.busy.swap(true, Ordering::Relaxed) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let event = match check(&app).await {
            Ok(Some(update)) => {
                let version = update.version.clone();
                *app.state::<UpdaterState>().found.locked() = Some(update);
                UpdateEvent::Found(version)
            }
            Ok(None) => UpdateEvent::NotFound,
            Err(error) => {
                eprintln!("update check failed: {error}");
                UpdateEvent::CheckFailed
            }
        };
        app.state::<UpdaterState>().busy.store(false, Ordering::Relaxed);
        apply(&app, event);
    });
}

async fn check(app: &AppHandle) -> tauri_plugin_updater::Result<Option<Update>> {
    app.updater()?.check().await
}

pub fn install(app: AppHandle) {
    let updater = app.state::<UpdaterState>();
    // 続けて押されても、ダウンロードは 1 つだけにする
    if updater.busy.swap(true, Ordering::Relaxed) {
        return;
    }
    let Some(update) = updater.found.locked().clone() else {
        updater.busy.store(false, Ordering::Relaxed);
        return;
    };
    apply(&app, UpdateEvent::Started);
    tauri::async_runtime::spawn(async move {
        let mut downloaded: u64 = 0;
        let progress_app = app.clone();
        let result = update
            .download_and_install(
                move |chunk, total| {
                    downloaded += chunk as u64;
                    apply(&progress_app, UpdateEvent::Progress { downloaded, total });
                },
                || {},
            )
            .await;
        match result {
            // 入れ替えが済んだら、新しい版で起動し直す。hook とプラグインは起動したときの処理で新しくなる
            Ok(()) => app.restart(),
            Err(error) => {
                eprintln!("update install failed: {error}");
                app.state::<UpdaterState>().busy.store(false, Ordering::Relaxed);
                apply(&app, UpdateEvent::InstallFailed);
            }
        }
    });
}
```

`Update` が `Clone` でない場合は、`found` から `take()` で取り出し、失敗したときに `found` へ戻す形にする（`let Some(update) = updater.found.locked().take()`、失敗の枝で `*app.state::<UpdaterState>().found.locked() = Some(update)` — そのときは `download_and_install` を `&update` で呼ぶ）。どちらの形かは Step 1 で入った版の `Update` の定義を見て決め、決めたことを報告に書く。

`main.rs`:
- `.manage(updater::UpdaterState::default())` を、ほかの `.manage` の並びに足す
- プラグインは公開のビルドでだけ登録する。`tauri::Builder::default()` を変数に受けてから登録する。

```rust
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None));
    // 更新のプラグインは、設定に公開鍵がないと起動できない。開発のビルドでは登録しない
    if updater::ENABLED {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }
    builder
        .manage(monitor::load_state(home))
        // （以下、今の .manage・.invoke_handler・.setup・.build・.run のまま）
```

- `invoke_handler` に `commands::get_update` と `commands::install_update` を足す

`monitor.rs` の `spawn` のループの中、`run_tick` の後に足す。

```rust
            crate::updater::tick(&app, paths::now_ms());
```

`commands.rs` に足す。

```rust
#[tauri::command]
pub fn get_update(app: AppHandle) -> agent_loaf_core::update::UpdateState {
    crate::updater::current(&app)
}

#[tauri::command]
pub fn install_update(app: AppHandle) {
    crate::updater::install(app);
}
```

- [ ] **Step 4: 確かめる**

Run: `cargo test --workspace && cargo build -p agent-loaf-app`
Expected: PASS（`development_builds_never_check` も）・ビルドが通る・警告なし

Run: `AGENT_LOAF_RELEASE=1 cargo build -p agent-loaf-app`
Expected: ビルドが通る（公開のビルドの経路もコンパイルできる。起動は Task 4 で公開鍵を入れてから）

- [ ] **Step 5: コミット**

```bash
git add app/src-tauri/Cargo.toml Cargo.lock app/src-tauri/src
git commit -m "公開のビルドで 1 日に 1 回新しい版を確かめ、押したらダウンロードして再起動する

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: 設定の画面の「アップデート」の欄

**Files:**
- Modify: `app/index.html`, `app/src/settings.ts`, `app/src/types.ts`, `app/src/style.css`
- Modify: `crates/core/locales/ja.json`, `crates/core/locales/en.json`

**Interfaces:**
- Consumes: Task 2 のイベント `"update-updated"`、コマンド `get_update`・`install_update`、`UpdateState` の JSON（`{ kind: "idle" }`、`{ kind: "available", version }`、`{ kind: "downloading", version, percent }`、`{ kind: "failed", version }`）
- Produces: `settings.ts` の `setupUpdateSection(): void`（`list.ts` の `start()` から呼ぶ）

見た目は Figma の `150:1053`（「Claude Code の設定」と「ログイン時に起動する」の間の欄。版の文の下に間を空けて「アップデートして再起動」。失敗のときは「もう一度」に replay のアイコン）。

- [ ] **Step 1: 文言を足す（失敗するテストから）**

`index.html` の「Claude Code の設定」の `menu-section` の後に足す。

```html
        <div id="update-section" class="menu-section" hidden>
          <h2 class="menu-heading" data-i18n="settings.update"></h2>
          <p id="update-note" class="menu-note"></p>
          <button id="update-action" class="menu-item centered" type="button" hidden></button>
        </div>
```

Run: `cargo test -p agent-loaf-app --test messages_used`
Expected: FAIL（`settings.update が対応表にない`）

- [ ] **Step 2: 対応表に足す**

`crates/core/locales/ja.json`（`settings.quit` の後など、settings の並びの中）:

```json
  "settings.update": "アップデート",
  "update.available": "新しいバージョン {new} があります（今は {current}）",
  "update.install": "アップデートして再起動",
  "update.downloading": "ダウンロードしています… {percent}%",
  "update.failed": "アップデートできませんでした。ネットワークにつながっているか確かめてください。",
```

`crates/core/locales/en.json`:

```json
  "settings.update": "Update",
  "update.available": "Version {new} is available (you have {current})",
  "update.install": "Update and restart",
  "update.downloading": "Downloading… {percent}%",
  "update.failed": "Couldn't update. Check your network connection.",
```

「もう一度」は今ある `settings.retry` を使う。日英のキーがそろっているかのテストがあるので、両方に足す。

Run: `cargo test --workspace`
Expected: PASS

- [ ] **Step 3: 画面を作る**

`app/src/types.ts` に足す。

```ts
export type UpdateState =
  | { kind: "idle" }
  | { kind: "available"; version: string }
  | { kind: "downloading"; version: string; percent: number }
  | { kind: "failed"; version: string };
```

`app/src/style.css` の `[hidden]` の行（`.badge[hidden], .menu-item[hidden], …`）に `.menu-section[hidden]` を足す。

```css
.badge[hidden], .menu-item[hidden], .menu-note[hidden], .claude-path[hidden], .menu-section[hidden] { display: none; }
```

`app/src/settings.ts` に足す（`ICON_REPLAY` は `./icons` から import 済み。`getVersion` は `@tauri-apps/api/app` から import する。新しいライブラリではなく、今使っている `@tauri-apps/api` の中のもの）。

```ts
const updateSectionEl = document.getElementById("update-section")!;
const updateNoteEl = document.getElementById("update-note")!;
const updateActionEl = document.getElementById("update-action") as HTMLButtonElement;
let currentVersion = "";

function renderUpdate(update: UpdateState): void {
  updateSectionEl.hidden = update.kind === "idle";
  updateActionEl.hidden = update.kind === "downloading" || update.kind === "idle";
  switch (update.kind) {
    case "idle":
      return;
    case "available":
      updateNoteEl.textContent = t("update.available", { new: update.version, current: currentVersion });
      updateActionEl.textContent = t("update.install");
      return;
    case "downloading":
      updateNoteEl.textContent = t("update.downloading", { percent: update.percent });
      return;
    case "failed": {
      updateNoteEl.textContent = t("update.failed");
      // アイコンは固定の文字列なので innerHTML、文言は textContent で入れる
      updateActionEl.innerHTML = ICON_REPLAY;
      updateActionEl.append(t("settings.retry"));
      return;
    }
  }
}

export function setupUpdateSection(): void {
  updateActionEl.addEventListener("click", () => void invoke("install_update"));
  void listen<UpdateState>("update-updated", (event) => renderUpdate(event.payload));
  // 一覧は開くたびに作り直すので、開いたときの状態を受け取る
  void getVersion().then((version) => {
    currentVersion = version;
    return invoke<UpdateState>("get_update").then(renderUpdate);
  });
}
```

`import type { … } from "./types"` に `UpdateState` を足す。`app/src/list.ts` の `start()` の `setupClaudeSection({…})` の後で `setupUpdateSection();` を呼ぶ（import にも足す）。

アイコンと文字の間は、今の「＋ 設定ディレクトリを追加」や設定の行のアイコンと同じ間にする（`style.css` に `.menu-item.centered` の `gap` がなければ、Figma の `150:1053` の「もう一度」の間を `get_design_context` で読んで、その値を足す）。

- [ ] **Step 4: 確かめる**

Run: `cd app && npx tsc --noEmit && cd .. && cargo test --workspace`
Expected: 型エラーなし・PASS

画面の見た目は、開発のビルドでは確かめる処理が動かないので、Task 8 の試しの版（v0.5.1）で、ユーザーに「新しい版がある」「ダウンロード中」の表示を見てもらう。「失敗」は Task 8 でオフラインにして「アップデートして再起動」を押して確かめる。ここでは `npm run tauri dev` で設定の画面を開き、「アップデート」の欄が**出ていない**こと（「なし」のとき）と、ほかの欄が崩れていないことだけを確かめる。

- [ ] **Step 5: コミット**

```bash
git add app/index.html app/src crates/core/locales
git commit -m "設定の画面に、新しい版があるときだけアップデートの欄を出す

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: 公開のための設定と版

**Files:**
- Modify: `app/src-tauri/tauri.conf.json`
- Modify: `app/src-tauri/tests/versions.rs` か `window_config.rs`（設定のテスト）
- Modify: 版の番号のあるファイル（`app/src-tauri/tauri.conf.json`、3 つの `Cargo.toml`、`app/package.json`、`plugin/.claude-plugin/plugin.json`）、`Cargo.lock`、`app/package-lock.json`

**前提:** 自動更新の公開鍵が要る。ユーザーに次を頼み、出力された**公開鍵**（`.pub` の中身）だけを受け取る。秘密鍵とパスワードは会話に貼ってもらわない。

```bash
npx --prefix app tauri signer generate -w ~/.tauri/agent-loaf.key
```

（`~/.tauri/agent-loaf.key` が秘密鍵、`~/.tauri/agent-loaf.key.pub` が公開鍵。秘密鍵とパスワードはパスワード管理のアプリにも控えてもらう。）

公開鍵がまだなければ、Step 1〜3 のうち `pubkey` 以外を先に入れてコミットし、`pubkey` は受け取ってから足す。

- [ ] **Step 1: 失敗するテストを書く**

`app/src-tauri/tests/window_config.rs` に足す（同じファイルの `conf` の読み方に合わせる）。

```rust
// 公開のビルドが更新を確かめに行く先と、入れ替える前に中身を確かめる鍵。どちらかがないと公開のビルドが起動できない
#[test]
fn release_settings_are_present() {
    let conf: serde_json::Value = serde_json::from_str(&fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json")).unwrap()).unwrap();
    assert_eq!(conf["bundle"]["macOS"]["minimumSystemVersion"], "14.0");
    assert_eq!(conf["bundle"]["createUpdaterArtifacts"], true);
    let updater = &conf["plugins"]["updater"];
    assert_eq!(updater["endpoints"][0], "https://github.com/imatomix/agent-loaf/releases/latest/download/latest.json");
    assert!(updater["pubkey"].as_str().is_some_and(|key| key.len() > 40), "自動更新の公開鍵を入れること");
}
```

Run: `cargo test -p agent-loaf-app --test window_config`
Expected: FAIL

- [ ] **Step 2: 設定を入れる**

`tauri.conf.json`:
- `bundle.macOS` を `{ "signingIdentity": "-", "minimumSystemVersion": "14.0" }` にする（`signingIdentity` の `"-"` は開発のビルドのために残す。公開のビルドは `APPLE_SIGNING_IDENTITY` で上書きする。上書きできることは Task 8 の手元での確かめで確かめる）
- `bundle` に `"createUpdaterArtifacts": true` を足す
- 一番外に足す。

```json
  "plugins": {
    "updater": {
      "pubkey": "<ユーザーから受け取った公開鍵の 1 行>",
      "endpoints": ["https://github.com/imatomix/agent-loaf/releases/latest/download/latest.json"]
    }
  }
```

（`<…>` は、受け取った公開鍵に置き換えてからコミットする。置き換える前にコミットしない。）

- [ ] **Step 3: 版を 0.5.0 にする**

7 か所の `0.4.0` を `0.5.0` にする（`tauri.conf.json`、`app/src-tauri/Cargo.toml`、`crates/core/Cargo.toml`、`crates/hook/Cargo.toml`、`app/package.json`、`plugin/.claude-plugin/plugin.json`）。

```bash
cargo build --workspace && (cd app && npm install)
```

Run: `cargo test --workspace`
Expected: PASS（`every_version_matches_the_app` と `release_settings_are_present` も）

`createUpdaterArtifacts` を入れると、手元の `npm run tauri build` が署名の秘密鍵（`TAURI_SIGNING_PRIVATE_KEY`）を求めて止まる。その場合は、手元の開発のビルドでは `--config '{"bundle":{"createUpdaterArtifacts":false}}'` を付けて作る手順を README の「開発」に書く（Task 6）。止まるかどうかを確かめて、報告に書く。

- [ ] **Step 4: コミット**

```bash
git add app/src-tauri/tauri.conf.json app/src-tauri/tests app/src-tauri/Cargo.toml crates/core/Cargo.toml crates/hook/Cargo.toml app/package.json app/package-lock.json Cargo.lock plugin/.claude-plugin/plugin.json
git commit -m "公開のための設定（macOS 14 以降、更新の成果物と確かめる先、公開鍵）を入れ、版を 0.5.0 にする

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: タグで作って Releases の下書きに上げるワークフロー

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: ワークフローを書く**

```yaml
name: release

# v で始まるタグを push したときだけ動く。ほかの人の Pull Request からは動かないので、Secrets は読まれない
on:
  push:
    tags: ["v*"]

permissions:
  contents: write

jobs:
  release:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-apple-darwin
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: app/package-lock.json
      - run: npm ci
        working-directory: app
      - name: 同梱する hook を作る
        run: ./scripts/build-hook-sidecar.sh
      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          AGENT_LOAF_RELEASE: "1"
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
        with:
          projectPath: app
          tagName: ${{ github.ref_name }}
          releaseName: Agent Loaf ${{ github.ref_name }}
          releaseBody: "変更点は後で書き足す（公開する前に、この下書きを編集する）"
          releaseDraft: true
          args: --target aarch64-apple-darwin
```

`macos-14` は Apple Silicon の実行環境。`build-hook-sidecar.sh` は `rustc -vV` の host（`aarch64-apple-darwin`）の名前で hook を置くので、`--target aarch64-apple-darwin` と名前が合う。

- [ ] **Step 2: 書き方を確かめる**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml')); print('ok')" 2>/dev/null || ruby -ryaml -e "YAML.load_file('.github/workflows/release.yml'); puts 'ok'"`
Expected: `ok`

実際に動くかは、Task 8 でタグを push して確かめる（それまでは動かせない）。

- [ ] **Step 3: コミット**

```bash
git add .github/workflows/release.yml
git commit -m "タグを push したら、署名・公証した dmg と更新の情報を Releases の下書きに上げる

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: 文書

**Files:**
- Create: `docs/releasing.md`
- Modify: `README.md`, `README.en.md`, `docs/superpowers/specs/2026-10-09-agents-monitor-design.md`, `docs/superpowers/specs/2026-10-12-agent-loaf-v0.5-distribution-design.md`

- [ ] **Step 1: `docs/releasing.md` を書く（日本語）**

節と中身:
1. **一度だけの準備**: 設計書 2.1 の 1〜6 を、具体的な手順で書く。
   - Developer ID Application の証明書の作り方（Xcode → Settings → Accounts → Manage Certificates、または developer.apple.com）
   - `.p12` の書き出し（キーチェーンアクセス）と base64 化（`base64 -i cert.p12 | pbcopy`）
   - App 用パスワード（account.apple.com → サインインとセキュリティ）
   - Team ID の確かめ方
   - 自動更新の鍵（`npx --prefix app tauri signer generate -w ~/.tauri/agent-loaf.key`）
   - Secrets の名前の一覧（Task 5 の `env` の 8 つ）
2. **控え**: 秘密鍵・パスワード・`.p12` を、パスワード管理のアプリにも控える。自動更新の秘密鍵をなくすと、配ったアプリに更新を届けられなくなること
3. **手元での確かめ**（最初の 1 回）:

```bash
./scripts/build-hook-sidecar.sh
cd app
AGENT_LOAF_RELEASE=1 \
APPLE_SIGNING_IDENTITY="Developer ID Application: <名前> (<Team ID>)" \
APPLE_ID=<Apple ID> APPLE_PASSWORD=<App 用パスワード> APPLE_TEAM_ID=<Team ID> \
TAURI_SIGNING_PRIVATE_KEY="$(cat ~/.tauri/agent-loaf.key)" TAURI_SIGNING_PRIVATE_KEY_PASSWORD=<鍵のパスワード> \
npm run tauri build -- --target aarch64-apple-darwin
```

   確かめ方: `spctl -a -vv "../target/aarch64-apple-darwin/release/bundle/macos/Agent Loaf.app"` が `source=Notarized Developer ID` を出すこと、`codesign -dv --verbose=2` の `Authority` が Developer ID であること
4. **公開のたびに**: 版を上げる（7 か所。テストが確かめる）→ コミット → `git tag v0.5.0 && git push origin v0.5.0` → Actions が終わったら Releases の下書きを開き、変更点を書いて、dmg を手元で開いて確かめてから「Publish release」
5. **うまくいかないとき**: Actions のログの見方、タグの付け直し（`git tag -d v0.5.0 && git push origin :refs/tags/v0.5.0`、下書きを消してから付け直す）

- [ ] **Step 2: README（日本語・英語）を直す**

1. 「必要な環境」を「Apple Silicon の Mac、macOS 14 以降（確かめたのは macOS 26）」と `claude` コマンドにする
2. 「入手」を次にする: 「[Releases](https://github.com/imatomix/agent-loaf/releases/latest) から `Agent Loaf_<版>_aarch64.dmg` をダウンロードして開き、Agent Loaf を『アプリケーション』に入れてください。」ソースからの作り方は「開発」に移す
3. 「設定」に足す: 「アップデート: 新しい版があるときだけ出ます。『アップデートして再起動』で入れ替えて起動し直します」
4. 「保存するデータ」の後に「通信」の節: 「1 日に 1 回、GitHub に新しい版を確かめに行きます（`latest.json` を読むだけ）。ほかには通信しません。」
5. 「開発」: ソースからの作り方（今の「入手」の中身）。Task 4 の Step 3 で手元のビルドに `--config` が要ると分かった場合は、それも書く
6. 英語版も同じ節・同じ中身にする

Run: `diff <(grep '^#' README.md | wc -l) <(grep '^#' README.en.md | wc -l)`
Expected: 差がない

- [ ] **Step 3: 設計書を直す**

- 元の設計書の 13 章: 「配布は `.dmg` と Homebrew Cask」を「配布は GitHub の Releases の `.dmg`（Homebrew は対象外。`2026-10-12-agent-loaf-v0.5-distribution-design.md`）」に、自動更新の行を「知らせて、押したら更新して再起動する」にする。Apple Silicon だけ・macOS 14 以降を足す
- 14 章: v0.5（配布と公開）の行を足す
- v0.5 の設計書の状態を、すべての Task が終わったら「実装済み」にする（Task 8 の最後）

- [ ] **Step 4: コミット**

```bash
git add docs/releasing.md README.md README.en.md docs/superpowers/specs
git commit -m "公開の手順（docs/releasing.md）を書き、README を Releases からの入手と自動更新に合わせる

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: 公開する前の点検

**Files:** なし（点検だけ。見つかったものはユーザーと相談してから直す）

- [ ] **Step 1: 今のファイルを点検する**

```bash
git grep -nIiE '(BEGIN [A-Z ]*PRIVATE KEY|ghp_[A-Za-z0-9]{20,}|gho_[A-Za-z0-9]{20,}|sk-ant-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16}|password\s*[:=]\s*["'"'"'][^"'"'"']+)' -- . ':!app/package-lock.json'
git grep -nIE '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[a-z]{2,}' -- . ':!app/package-lock.json' ':!Cargo.lock'
git grep -nI '/Users/imatomix' -- .
```

Expected: 1 つ目は何も出ない。2 つ目は `noreply@anthropic.com`（コミットの共著の表記）と、テストの例のアドレスだけ。3 つ目は、テストの fixtures や設計書の例としての家のパスだけ（`/Users/imatomix` が公開されて困るかをユーザーに確かめる）

- [ ] **Step 2: 履歴を点検する**

```bash
git log -p --all | grep -nIiE '(BEGIN [A-Z ]*PRIVATE KEY|ghp_[A-Za-z0-9]{20,}|gho_[A-Za-z0-9]{20,}|sk-ant-[A-Za-z0-9-]{10,}|AKIA[0-9A-Z]{16})' | head
git log --all --format='%an <%ae> | %cn <%ce>' | sort | uniq -c
git log --all --name-only --format='' | sort -u | grep -viE '\.(rs|ts|json|md|toml|css|html|sh|mjs|svg|png|icns|ico|jsonl|yml|lock)$'
```

Expected: 1 つ目は何も出ない。2 つ目は、すべて `Keita Imatomi <3133649+imatomix@users.noreply.github.com>`。3 つ目は、`.gitignore` や `LICENSE` など、入っていて当たり前のものだけ（`icon.ai` や `.p12` などがないこと）

- [ ] **Step 3: 結果をユーザーに報告する**

見つかったもの（とくに Step 1 の 3 つ目の `/Users/imatomix`）を一覧にして、公開してよいかを確かめる。直すときは、そのファイルだけを直してコミットする（履歴の書き換えは、ユーザーが望んだときだけ）。

---

### Task 8: 登録の後に、公開して確かめる（ユーザーと一緒に）

**前提:** ユーザーの Apple Developer Program の登録が済んでいること。Task 1〜7 が済んでいること。

- [ ] **Step 1: 一度だけの準備**

`docs/releasing.md` の「一度だけの準備」を、ユーザーと一緒に進める。証明書がキーチェーンに入ったことは `security find-identity -v -p codesigning` で確かめる（`Developer ID Application:` の行が出ること）。

- [ ] **Step 2: 手元で署名と公証を通す**

`docs/releasing.md` の「手元での確かめ」の通りに作る（値はユーザーに打ってもらう）。

- `codesign -dv --verbose=2` の `Authority` が Developer ID であること（`APPLE_SIGNING_IDENTITY` が設定の `"-"` を上書きできたこと）
- 同梱の hook（`Contents/MacOS/agent-loaf-hook`）も同じ署名であること（`codesign -dv` で確かめる）
- `spctl -a -vv` が `source=Notarized Developer ID` を出すこと
- dmg を作り直したもの（`target/aarch64-apple-darwin/release/bundle/dmg/*.dmg`）に quarantine の印を付けて開き（`xattr -w com.apple.quarantine "0081;$(printf %x $(date +%s));Safari;" <dmg>`）、警告なしに開けること
- 動いているアプリを終了してから、このアプリを「アプリケーション」に入れて起動し、通知の許可を尋ねる画面が出ること（出なければ、システム設定の通知の欄から Agent Loaf を消してから試す）

うまくいかなければ、ログの理由を見て、設定・ワークフロー・`docs/releasing.md` を直してコミットする。

- [ ] **Step 3: Secrets とリポジトリの公開（ユーザーの指示を待ってから）**

1. ユーザーが `gh auth login` で `imatomix` に切り替える（`gh auth status` で確かめる）
2. ユーザーの指示を受けてから: `gh repo create imatomix/agent-loaf --public --source . --description "<説明>" --push`（説明文はユーザーと決める）。トピック: `gh repo edit imatomix/agent-loaf --add-topic claude-code,macos,menu-bar,tauri`
3. ユーザーが Secrets を入れる（`gh secret set <名前>`。値は会話に貼らずに、ユーザーの端末で入れてもらう）。`gh secret list` で 8 つそろったことを確かめる

- [ ] **Step 4: v0.5.0 を出す（ユーザーの指示を待ってから）**

1. `git tag v0.5.0 && git push origin v0.5.0`
2. `gh run watch` で Actions を見届ける。失敗したら `docs/releasing.md` の「うまくいかないとき」の通りに直す
3. Releases の下書きに `Agent Loaf_0.5.0_aarch64.dmg`、`Agent Loaf.app.tar.gz`、その `.sig`、`latest.json` があること
4. dmg をダウンロードして、Step 2 と同じく警告なしに開けること
5. ユーザーが変更点を書いて「Publish release」を押す
6. `curl -sL https://github.com/imatomix/agent-loaf/releases/latest/download/latest.json` が 0.5.0 を返すこと

- [ ] **Step 5: 自動更新を確かめる（試しの版）**

1. Releases の v0.5.0 の dmg から入れた Agent Loaf を動かしておく
2. 版を 0.5.1 にした試しのコミットをブランチに作り（`main` には入れない）、`v0.5.1` のタグを push し、下書きを公開する
3. 確かめる時刻を待たないよう、Agent Loaf を終了して起動し直し、約 1 分待つ
4. ユーザーに確かめてもらう: 設定の画面に「新しいバージョン 0.5.1 があります（今は 0.5.0）」が出る → 「アップデートして再起動」→「ダウンロードしています… n%」→ 再起動して 0.5.1 になる → 通知の許可が残っている（新しいセッションの通知が届く）
5. 失敗の表示: もう一度 0.5.0 を入れ直し、知らせが出たら Wi-Fi を切ってから「アップデートして再起動」を押し、「アップデートできませんでした…」と「もう一度」が出て、今の版のまま動くこと
6. オフラインで起動しても、何も表示されず動き続けること
7. 後片付け: v0.5.1 の Release とタグを消す（`gh release delete v0.5.1 --cleanup-tag`）。試しのブランチを消す。利用者の手元は、ユーザーが v0.5.0 を入れ直す

- [ ] **Step 6: 仕上げ**

- v0.5 の設計書の状態を「実装済み」にしてコミットし、push する（ユーザーの指示を受けてから）
- 最も能力の高いモデルの新しいエージェントに、`main` からこのブランチまでの差分全体を見直してもらう（設計書と計画を渡す）
