# ディレクトリ構成を機能ごとに整理する Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 構成の方針を ADR 0010 にし、`crates/core`・`app/src-tauri`・`app/src` を機能ごとのフォルダーに並べ直す。振る舞いは変えない。

**Architecture:** 3 つの置き場所（ADR 0001）はそのままに、それぞれの中を `session`・`setup`・`process`・`update`・`base`（画面は `features/` と `lib/`）に分ける。ファイルは `git mv` で動かし、`use`・`mod`・パスの書き換えのほかは中身を変えない。`app/src-tauri` は Tauri 2 の標準（`lib.rs` に中身、`main.rs` は呼ぶだけ）にそろえ、命令を `commands/` の下に機能ごとに分ける。

**Tech Stack:** Rust（Cargo workspace、Rust 1.99.1）、Tauri 2、TypeScript（Vite）

**Spec:** `docs/superpowers/specs/2026-10-18-directory-layout-design.md`

## Global Constraints

- 振る舞いを変えない。新しいライブラリを入れない
- 大きなファイル（`store.rs` など）を分けない
- 古い呼び方を残す仕組み（再エクスポート）を作らない
- フォルダーは `<group>/mod.rs` の形でモジュールにする
- ファイルは `git mv` で動かす。動かすコミットでは、`use`・`mod`・パスの書き換えのほかは中身を変えない
- アプリの実行ファイルの名前は `agent-loaf-app` のまま（`Contents/MacOS/agent-loaf-app`）
- 画面から呼ぶ命令の名前（`invoke("get_sessions")` など）は変えない
- コミットのメッセージは日本語。最後に `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
- コメントは理由（WHY）だけを日本語で書く
- コミットごとに `./scripts/check.sh` を通す

## Review Focus

1. 実行ファイルの名前: `lib.rs` を足しても、`target/release/bundle/macos/Agent Loaf.app/Contents/MacOS/agent-loaf-app` のままであること（Task 3 で確かめる）
2. 文言のキーのテスト: `messages_used.rs` が、`app/src/` の下のフォルダーの `.ts` も読むこと（Task 6 で、設定の画面だけが使うキーが拾われることを確かめる）
3. 相対パスの埋め込み: 動かしたファイルの `include_str!`・`include_bytes!` が、新しい深さで正しい場所を指すこと（Task 2・5 で直す。コンパイルが通れば確かめられる）
4. clippy の `module_inception`: `tray/tray.rs`・`update/update.rs` のような、フォルダーと同じ名前のファイルを作らないこと（Task 2・5）
5. 画面の読み込み: `index.html` の `<script>` と、画面の `import`（画像を含む）が新しい場所を指すこと（Task 6 で `tsc` と `tauri build` と実機で確かめる）

---

## File Structure

```
crates/core/src/
  lib.rs                 pub mod base; pub mod process; pub mod session; pub mod setup; pub mod update;
  README.md              （Task 7）
  session/mod.rs         event・hook_input・spool・state・store・view・order・notify・transcript
  setup/mod.rs           config_dirs・claude_cli・codex_plugin・install・install_plan・marketplace・plugin_status・attention
  process/mod.rs         ps・procinfo・jump・git
  update/mod.rs          （今の update.rs の中身をそのまま）
  base/mod.rs            agent・paths・persist・settings・messages・crash_log

app/src-tauri/src/
  main.rs                agent_loaf_app_lib::run() を呼ぶだけ
  lib.rs                 今の main.rs の中身（fn main を pub fn run にする）
  README.md              （Task 7）
  commands/mod.rs        pub mod app; pub mod session; pub mod settings; pub mod setup; pub mod update;
  commands/{session,setup,settings,update,app}.rs
  session/mod.rs         monitor・notifier
  setup/mod.rs           installer
  process/mod.rs         jumper
  update/mod.rs          updater
  tray/mod.rs            （今の tray.rs の中身）と icon（今の tray_icon.rs）
  base/mod.rs            lang・sync

app/src/
  README.md              （Task 7）
  features/list/         list.ts・drag.ts・format.ts
  features/settings/     settings.ts
  lib/                   i18n.ts・icons.ts・types.ts
  assets/  style.css  vite-env.d.ts   （今のまま）
```

---

### Task 1: ADR 0010

**Files:**
- Create: `docs/adr/0010-directory-layout.md`
- Modify: `docs/adr/README.md`（一覧に足す）

- [ ] **Step 1: ADR を書く**

`docs/adr/0010-directory-layout.md`:

````markdown
# 0010. ディレクトリ構成

- ステータス: 承認
- 日付: 2026-10-18

## コンテキスト

公開した直後で、手伝う人がまだいない。`crates/core/src` に 28、`app/src-tauri/src` に 11、`app/src` に 9 のファイルが、どれも 1 つのフォルダーに平らに並んでいた。困ってはいなかったが、新しいコードをどこに置くか、今あるコードがどこにあるかの決まりがなかった。並べ直す手間は、人が増えるほど大きくなる。

## 決定

3 つの置き場所（ADR 0001）はそのままに、それぞれの中を機能ごとのフォルダーに分ける。フォルダーの名前は、3 つの置き場所でできるだけそろえる。

```
crates/core/src/      判断のロジック
  session/  setup/  process/  update/  base/
app/src-tauri/src/    OS とのつなぎ（Tauri 2 の標準。中身は lib.rs、main.rs は呼ぶだけ）
  commands/  session/  setup/  process/  update/  tray/  base/
app/src/              画面
  features/list/  features/settings/  lib/
```

- `session`: 会話の状態。`setup`: 設定ディレクトリとプラグインの導入。`process`: プロセスと画面の移動。`update`: 自動更新。`tray`: メニューバーのアイコンと一覧の窓（OS とのつなぎにしかない）
- 2 つ以上の機能から使うものだけを `base`（Rust）・`lib`（画面）に置く。先回りして共通化しない
- 画面から呼ぶ命令（`#[tauri::command]`）は `app/src-tauri/src/commands/` に、機能ごとのファイルで置く
- 画面の機能は画面の名前（`list`・`settings`）で分ける
- 各置き場所の入り口の README に、中のフォルダーの役割と置き場所の決まりを書く
- `src` という名前は道具（Cargo・Vite）の決まりなので変えない

設計書: [ディレクトリ構成を機能ごとに整理する](../superpowers/specs/2026-10-18-directory-layout-design.md)

## 検討した代替案

- **技術の層で分ける**（データの形・判断・読み書き）: 1 つの機能を直すのに、いくつものフォルダーをまたぐ
- **`crates/core` を、さらにいくつかの crate に分ける**: 境目はコンパイルで守られるが、`Cargo.toml` と crate の間の依存の管理が増える。今の規模には重い
- **方針だけを書き、ファイルは触ったときに動かす**: 古い並びと新しい並びが、しばらく混ざる

## 影響

- 新しいコードは、まず機能のフォルダーに置く。2 つ目の利用先が出てから `base`・`lib` に上げる
- `crates/core` を使う側の呼び方は `agent_loaf_core::session::store` のようになる
- 2026-10-18 より前の設計書と計画（`docs/superpowers/specs/`・`plans/`）に書いたファイルのパスは、整理の前のもの
````

- [ ] **Step 2: ADR の一覧に足す**

`docs/adr/README.md` の一覧の最後（`0009` の行の次）に足す:

```markdown
- [0010. ディレクトリ構成](./0010-directory-layout.md)
```

- [ ] **Step 3: コミット**

```bash
git add docs/adr/0010-directory-layout.md docs/adr/README.md
git commit -m "ディレクトリ構成を機能ごとに分ける方針を ADR 0010 に書く

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `crates/core` を機能のフォルダーに動かす

**Files:**
- Move: `crates/core/src/*.rs`（下の表のとおり）
- Create: `crates/core/src/{session,setup,process,base}/mod.rs`
- Modify: `crates/core/src/lib.rs`、`crates/core/src/**/*.rs`（`crate::` の書き換え）、`crates/core/tests/*.rs`、`crates/hook/src/main.rs`、`crates/hook/tests/*.rs`、`app/src-tauri/src/*.rs`、`app/src-tauri/tests/*.rs`

**Interfaces:**
- Produces: `agent_loaf_core::{session,setup,process,base}::<module>`。`agent_loaf_core::update` はそのまま

| 今 | 動かす先 |
|---|---|
| event・hook_input・spool・state・store・view・order・notify・transcript | `session/` |
| config_dirs・claude_cli・codex_plugin・install・install_plan・marketplace・plugin_status・attention | `setup/` |
| ps・procinfo・jump・git | `process/` |
| agent・paths・persist・settings・messages・crash_log | `base/` |
| update | `update/mod.rs`（`update/update.rs` にすると clippy の `module_inception` に引っかかる） |

- [ ] **Step 1: ファイルを動かす**

```bash
cd crates/core/src
mkdir -p session setup process base update
for m in event hook_input spool state store view order notify transcript; do git mv $m.rs session/; done
for m in config_dirs claude_cli codex_plugin install install_plan marketplace plugin_status attention; do git mv $m.rs setup/; done
for m in ps procinfo jump git; do git mv $m.rs process/; done
for m in agent paths persist settings messages crash_log; do git mv $m.rs base/; done
git mv update.rs update/mod.rs
cd -
```

- [ ] **Step 2: `mod.rs` と `lib.rs` を書く**

`crates/core/src/session/mod.rs`:

```rust
pub mod event;
pub mod hook_input;
pub mod notify;
pub mod order;
pub mod spool;
pub mod state;
pub mod store;
pub mod transcript;
pub mod view;
```

`crates/core/src/setup/mod.rs`:

```rust
pub mod attention;
pub mod claude_cli;
pub mod codex_plugin;
pub mod config_dirs;
pub mod install;
pub mod install_plan;
pub mod marketplace;
pub mod plugin_status;
```

`crates/core/src/process/mod.rs`:

```rust
pub mod git;
pub mod jump;
pub mod procinfo;
pub mod ps;
```

`crates/core/src/base/mod.rs`:

```rust
pub mod agent;
pub mod crash_log;
pub mod messages;
pub mod paths;
pub mod persist;
pub mod settings;
```

`crates/core/src/lib.rs` を次だけにする:

```rust
pub mod base;
pub mod process;
pub mod session;
pub mod setup;
pub mod update;
```

- [ ] **Step 3: 呼び方を書き換える**

スクラッチのフォルダーに `rewrite_core_paths.py` を置いて動かす（リポジトリには入れない）:

```python
import re, subprocess
GROUPS = {
    "session": ["event", "hook_input", "spool", "state", "store", "view", "order", "notify", "transcript"],
    "setup": ["config_dirs", "claude_cli", "codex_plugin", "install", "install_plan", "marketplace", "plugin_status", "attention"],
    "process": ["ps", "procinfo", "jump", "git"],
    "base": ["agent", "paths", "persist", "settings", "messages", "crash_log"],
}
files = subprocess.run(["git", "ls-files", "*.rs"], capture_output=True, text=True, check=True).stdout.split()
for path in files:
    text = open(path).read()
    new = text
    for group, mods in GROUPS.items():
        for m in mods:
            new = re.sub(rf"\bagent_loaf_core::{m}\b", f"agent_loaf_core::{group}::{m}", new)
            if path.startswith("crates/core/"):
                new = re.sub(rf"\bcrate::{m}\b", f"crate::{group}::{m}", new)
    if new != text:
        open(path, "w").write(new)
        print(path)
```

```bash
python3 <スクラッチ>/rewrite_core_paths.py
```

続けて、`use crate::{a, b}` や `use agent_loaf_core::{a, b}` の形で書いたものが残っていないかを探し、あれば手で直す:

```bash
git grep -nE "use (crate|agent_loaf_core)::\{" -- '*.rs'
```

Expected: 出てこない（出たら、中のモジュール名を `group::module` に直す）

同じフォルダーに動いた兄弟のモジュールを `super::` で呼んでいる所がないかも見る（テストの `use super::*;` は除く）:

```bash
git grep -n "super::" -- crates/core/src | grep -v "use super::\*;"
```

Expected: 出てこない（出たら、`crate::<group>::<module>` に直す）

- [ ] **Step 4: 深くなったファイルの相対パスを直す**

`crates/core/src/setup/codex_plugin.rs` の `include_str!("../../../plugin/hooks/hooks.json")` を `include_str!("../../../../plugin/hooks/hooks.json")` に、`crates/core/src/base/messages.rs` の `include_str!("../locales/ja.json")`・`("../locales/en.json")` を `include_str!("../../locales/ja.json")`・`("../../locales/en.json")` にする。

`env!("CARGO_MANIFEST_DIR")` から組み立てるパス（`plugin_status.rs` のテストなど）は、crate の場所が変わらないので直さない。

- [ ] **Step 5: 整形して確かめる**

```bash
cargo fmt --all
./scripts/check.sh
```

Expected: 終わりの状態が 0。clippy の警告なし、テストがすべて通る

- [ ] **Step 6: 動かしたことが git に分かるかを確かめる**

```bash
git add -A crates app/src-tauri
git diff --cached -M --stat | tail -3
```

Expected: `crates/core/src/{event.rs => session/event.rs}` のように、名前の変更として出る

- [ ] **Step 7: コミット**

```bash
git commit -m "crates/core を機能ごとのフォルダー（session・setup・process・update・base）に並べ直す

中身は変えず、呼び方を agent_loaf_core::session::store のように書き換えただけ（ADR 0010）。

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `app/src-tauri` の `main.rs` と `lib.rs` を分ける

**Files:**
- Move: `app/src-tauri/src/main.rs` → `app/src-tauri/src/lib.rs`
- Create: `app/src-tauri/src/main.rs`
- Modify: `app/src-tauri/Cargo.toml`

**Interfaces:**
- Produces: `agent_loaf_app_lib::run()`

- [ ] **Step 1: 中身を `lib.rs` に動かす**

```bash
git mv app/src-tauri/src/main.rs app/src-tauri/src/lib.rs
```

`lib.rs` の `fn main() {` を `pub fn run() {` にする。ファイルの先頭に `#![cfg_attr(...)]` があれば、それは `main.rs` に移す（なければ何もしない）。

- [ ] **Step 2: `main.rs` を書く**

```rust
fn main() {
    agent_loaf_app_lib::run();
}
```

- [ ] **Step 3: ライブラリの名前を決める**

`app/src-tauri/Cargo.toml` の `[package]` の後に足す（Tauri 2 の初期の形と同じく、実行ファイルとライブラリの名前を分ける）:

```toml
[lib]
name = "agent_loaf_app_lib"
```

- [ ] **Step 4: 確かめる**

```bash
./scripts/check.sh
(cd app && npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}')
ls "target/release/bundle/macos/Agent Loaf.app/Contents/MacOS/"
```

Expected: check.sh が通る。`ls` に `agent-loaf-app` と `agent-loaf-hook` が出る（実行ファイルの名前が変わっていない）

- [ ] **Step 5: コミット**

```bash
git add app/src-tauri/src/main.rs app/src-tauri/src/lib.rs app/src-tauri/Cargo.toml Cargo.lock
git commit -m "app/src-tauri を Tauri 2 の標準の形にし、中身を lib.rs、main.rs は呼ぶだけにする

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: 画面から呼ぶ命令を `commands/` の下に分ける

**Files:**
- Move: `app/src-tauri/src/commands.rs` → `app/src-tauri/src/commands/setup.rs`（いちばん大きいまとまりを元にして、歴史を残す）
- Create: `app/src-tauri/src/commands/{mod,session,settings,update,app}.rs`
- Modify: `app/src-tauri/src/lib.rs`（`generate_handler!`）

**Interfaces:**
- Produces: `commands::session::{get_sessions, move_session, jump_session, take_pending_details}`、`commands::setup::{get_setup, add_config_dir, retry_config_dir, remove_config_dir, set_claude_path, set_codex_path, open_codex_trust, refresh_setup}`、`commands::settings::{get_settings, save_settings, open_notification_settings, refresh_notification_access}`、`commands::update::{get_update, is_release_build, install_update}`、`commands::app::{get_messages, quit_app}`

- [ ] **Step 1: 元のファイルを動かす**

```bash
mkdir -p app/src-tauri/src/commands
git mv app/src-tauri/src/commands.rs app/src-tauri/src/commands/setup.rs
```

- [ ] **Step 2: 関数を分ける**

`commands/setup.rs` から、次の関数を、中身を変えずにそれぞれのファイルへ切り取って貼る。`set_launch_at_login`（`save_settings` だけが使う）は `settings.rs` に入れる。

| ファイル | 関数 |
|---|---|
| `commands/session.rs` | `get_sessions`・`move_session`・`jump_session`・`take_pending_details` |
| `commands/settings.rs` | `get_settings`・`save_settings`・`set_launch_at_login`・`open_notification_settings`・`refresh_notification_access` |
| `commands/update.rs` | `get_update`・`is_release_build`（コメントごと）・`install_update` |
| `commands/app.rs` | `get_messages`・`quit_app` |
| `commands/setup.rs`（残り） | `get_setup`・`add_config_dir`・`retry_config_dir`・`remove_config_dir`・`set_claude_path`・`set_codex_path`・`open_codex_trust`・`refresh_setup` |

各ファイルの先頭には、そのファイルの関数が使う `use` だけを書く（元の `commands.rs` の `use` から写す）。使わない `use` が残ると clippy で落ちるので、Step 4 で確かめる。

`commands/mod.rs`:

```rust
pub mod app;
pub mod session;
pub mod settings;
pub mod setup;
pub mod update;
```

- [ ] **Step 3: 命令の登録を直す**

`lib.rs` の `tauri::generate_handler![...]` の中の `commands::<名前>` を、上の表のとおり `commands::<ファイル>::<名前>` にする。命令の名前（画面の `invoke("get_sessions")` で使う名前）は関数の名前で決まるので、変わらない。

- [ ] **Step 4: 確かめる**

```bash
cargo fmt --all
./scripts/check.sh
git grep -n "#\[tauri::command\]" -- app/src-tauri/src | wc -l
```

Expected: check.sh が通る。命令は 21

- [ ] **Step 5: コミット**

```bash
git add -A app/src-tauri/src
git commit -m "画面から呼ぶ命令を、commands の下に機能ごとのファイル（session・setup・settings・update・app）で分ける

関数の中身は変えていない（ADR 0010）。

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: `app/src-tauri` を機能のフォルダーに動かす

**Files:**
- Move: 下の表のとおり
- Create: `app/src-tauri/src/{session,setup,process,update,base}/mod.rs`
- Modify: `app/src-tauri/src/lib.rs`（`mod` の宣言と呼び方）、`app/src-tauri/src/**/*.rs`（`crate::` の書き換え）

| 今 | 動かす先 | 呼び方 |
|---|---|---|
| monitor.rs・notifier.rs | `session/` | `crate::session::monitor`・`crate::session::notifier` |
| installer.rs | `setup/` | `crate::setup::installer` |
| jumper.rs | `process/` | `crate::process::jumper` |
| updater.rs | `update/` | `crate::update::updater` |
| tray.rs | `tray/mod.rs` | `crate::tray`（`tray/tray.rs` は clippy の `module_inception` に引っかかる） |
| tray_icon.rs | `tray/icon.rs` | `crate::tray::icon` |
| lang.rs・sync.rs | `base/` | `crate::base::lang`・`crate::base::sync` |

- [ ] **Step 1: ファイルを動かす**

```bash
cd app/src-tauri/src
mkdir -p session setup process update tray base
git mv monitor.rs session/ && git mv notifier.rs session/
git mv installer.rs setup/
git mv jumper.rs process/
git mv updater.rs update/
git mv tray.rs tray/mod.rs && git mv tray_icon.rs tray/icon.rs
git mv lang.rs base/ && git mv sync.rs base/
cd -
```

- [ ] **Step 2: `mod.rs` を書く**

`session/mod.rs`: `pub mod monitor;` と `pub mod notifier;`
`setup/mod.rs`: `pub mod installer;`
`process/mod.rs`: `pub mod jumper;`
`update/mod.rs`: `pub mod updater;`
`base/mod.rs`: `pub mod lang;` と `pub mod sync;`
`tray/mod.rs`（今の tray.rs の中身）の先頭に `pub mod icon;` を足す

`lib.rs` の先頭の `mod ...;` を次にする:

```rust
mod base;
mod commands;
mod process;
mod session;
mod setup;
mod tray;
mod update;
```

- [ ] **Step 3: 呼び方を書き換える**

スクラッチのフォルダーに `rewrite_app_paths.py` を置いて動かす:

```python
import re, subprocess
MOVES = {
    "monitor": "session::monitor", "notifier": "session::notifier",
    "installer": "setup::installer", "jumper": "process::jumper",
    "updater": "update::updater", "tray_icon": "tray::icon",
    "lang": "base::lang", "sync": "base::sync",
}
files = subprocess.run(["git", "ls-files", "app/src-tauri/src/*.rs", "app/src-tauri/src/**/*.rs"], capture_output=True, text=True, check=True).stdout.split()
for path in files:
    text = open(path).read()
    new = text
    for old, to in MOVES.items():
        new = re.sub(rf"\bcrate::{old}\b", f"crate::{to}", new)
    if new != text:
        open(path, "w").write(new)
        print(path)
```

`lib.rs` は `mod monitor;` で直接読んでいたので、`monitor::install_bundled_hook` のように `crate::` なしで書いた呼び方が残る。`lib.rs` の中の `monitor::`・`tray::`・`updater::`・`installer::`・`notifier::`・`lang::`・`jumper::` などを、上の表の呼び方に手で直す。

- [ ] **Step 4: 深くなったファイルの相対パスを直す**

`tray/icon.rs` の `include_bytes!("../icons/tray/...")`（4 か所）を `include_bytes!("../../icons/tray/...")` にする。

- [ ] **Step 5: 確かめる**

```bash
cargo fmt --all
./scripts/check.sh
```

Expected: 通る

- [ ] **Step 6: コミット**

```bash
git add -A app/src-tauri/src
git commit -m "app/src-tauri を機能ごとのフォルダー（session・setup・process・update・tray・base）に並べ直す

中身は変えず、mod と呼び方を書き換えただけ（ADR 0010）。

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: `app/src` を `features/` と `lib/` に動かす

**Files:**
- Move: 下の表のとおり
- Modify: 動かしたファイルの `import`、`app/index.html`、`app/src-tauri/tests/messages_used.rs`

| 今 | 動かす先 |
|---|---|
| list.ts・drag.ts・format.ts | `features/list/` |
| settings.ts | `features/settings/` |
| i18n.ts・icons.ts・types.ts | `lib/` |

- [ ] **Step 1: 文言のキーのテストに、下のフォルダーのファイルを読む確かめを足す**

`app/src-tauri/tests/messages_used.rs` の `assert!(used.len() > 10, ...)` の次に足す:

```rust
    // 設定の画面だけが使うキー。画面のファイルを下のフォルダーまで読めているかを確かめる
    assert!(
        used.iter().any(|key| key == "settings.codex_trust_open"),
        "app/src の下のフォルダーの .ts を読めていない"
    );
```

- [ ] **Step 2: ファイルを動かして、テストが落ちることを確かめる**

```bash
cd app/src
mkdir -p features/list features/settings lib
git mv list.ts drag.ts format.ts features/list/
git mv settings.ts features/settings/
git mv i18n.ts icons.ts types.ts lib/
cd -
cargo test -q -p agent-loaf-app --test messages_used
```

Expected: FAIL（`app/src の下のフォルダーの .ts を読めていない`）

- [ ] **Step 3: テストを下のフォルダーまで読むように直す**

`messages_used.rs` の `for entry in fs::read_dir(app.join("src")).unwrap() { ... }` を、下のフォルダーまでたどる形にする:

```rust
    let mut dirs = vec![app.join("src")];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|e| e == "ts") {
                used.extend(keys_in(&fs::read_to_string(&path).unwrap(), "t(\"", '"'));
            }
        }
    }
```

```bash
cargo test -q -p agent-loaf-app --test messages_used
```

Expected: PASS

- [ ] **Step 4: 画面の `import` と `index.html` を直す**

- `features/list/list.ts`: `./drag`・`./format` はそのまま。`./settings` → `../settings/settings`、`./i18n` → `../../lib/i18n`、`./types` → `../../lib/types`、`./assets/claude-code.svg` → `../../assets/claude-code.svg`、`./assets/codex.svg` → `../../assets/codex.svg`
- `features/list/format.ts`: `./i18n` → `../../lib/i18n`
- `features/settings/settings.ts`: `./icons` → `../../lib/icons`、`./i18n` → `../../lib/i18n`、`./types` → `../../lib/types`
- `app/index.html`: `<script type="module" src="/src/list.ts">` → `src="/src/features/list/list.ts"`（`/src/style.css` と `/src/assets/loaf.svg` はそのまま）

- [ ] **Step 5: 確かめる**

```bash
(cd app && npx tsc --noEmit)
./scripts/check.sh
(cd app && npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}')
```

Expected: どれも通る

- [ ] **Step 6: コミット**

```bash
git add -A app/src app/index.html app/src-tauri/tests/messages_used.rs
git commit -m "画面のコードを features（list・settings）と lib に並べ直し、文言のテストが下のフォルダーまで読むようにする

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: README と構成の説明

**Files:**
- Create: `crates/core/src/README.md`、`app/src-tauri/src/README.md`、`app/src/README.md`
- Modify: `AGENTS.md`（「構成」）、`README.md`・`README.en.md`（「開発」）、`CONTRIBUTING.md`・`CONTRIBUTING.en.md`（構成に触れている所があれば）

- [ ] **Step 1: 3 つの README を書く**

`crates/core/src/README.md`:

```markdown
# crates/core/src

Agent Loaf の判断のロジック（Rust の crate `agent-loaf-core`）。Tauri や OS に頼らない純粋な関数を中心にする（ADR 0001）。

| フォルダー | 置くもの |
|---|---|
| `session/` | 会話の状態を決める。hook のイベントを読み（spool・event・hook_input）、状態を決め（state・store）、一覧に出す形と並び（view・order）、通知（notify）、中断（transcript）を決める |
| `setup/` | 設定ディレクトリと、プラグインや hook の導入。CLI の呼び出し（claude_cli）、導入の手順（install_plan）、状態（plugin_status）、設定の画面のバッジ（attention） |
| `process/` | プロセスを読む（ps・procinfo）・画面を移動する（jump）・git のブランチ（git） |
| `update/` | 自動更新の状態の移り変わり |
| `base/` | 2 つ以上の機能から使う土台。agent・paths・persist・settings・messages・crash_log |

## 決まり

- 新しいコードは、まず機能のフォルダーに置く。2 つ目の利用先が出てから `base/` に上げる（ADR 0010）
- 文言の対応表は `crates/core/locales/` にある
```

`app/src-tauri/src/README.md`:

```markdown
# app/src-tauri/src

Agent Loaf の、Tauri のアプリ（OS とのつなぎ）。判断は `crates/core` に置き、ここでは Tauri・macOS とのやり取りだけをする（ADR 0001）。

| 場所 | 置くもの |
|---|---|
| `main.rs` | `lib.rs` の `run()` を呼ぶだけ（Tauri 2 の標準） |
| `lib.rs` | アプリの組み立て。プラグイン・命令の登録・起動のときの処理 |
| `commands/` | 画面から呼ぶ命令（`#[tauri::command]`）。画面のどこから呼ぶかで分ける（session・setup・settings・update・app） |
| `session/` | 監視（monitor）と、macOS の通知（notifier） |
| `setup/` | 設定ディレクトリとプラグインの導入（installer） |
| `process/` | 会話の画面を前に出す（jumper） |
| `update/` | 自動更新（updater） |
| `tray/` | メニューバーのアイコンと一覧の窓 |
| `base/` | 土台。言語の判定（lang）、ロックの扱い（sync） |

## 決まり

- 命令を足したら、`lib.rs` の `generate_handler!` にも足す
- 機能のフォルダーの名前は `crates/core/src` とそろえる（ADR 0010）
```

`app/src/README.md`:

```markdown
# app/src

Agent Loaf の画面（TypeScript と Vite）。デザインの元は Figma にある（ADR 0006）。

| 場所 | 置くもの |
|---|---|
| `features/list/` | 一覧の画面（入り口の list.ts、ドラッグ、時間の表し方） |
| `features/settings/` | 設定の画面 |
| `lib/` | 2 つ以上の画面から使うもの（文言、アイコン、型） |
| `assets/` | 画像 |
| `style.css` | 見た目 |

## 決まり

- 画面の機能は画面の名前で分ける。2 つ目の利用先が出てから `lib/` に上げる（ADR 0010）
- 文言は `t("キー")` で引く。キーは `crates/core/locales/` にあり、`app/src-tauri/tests/messages_used.rs` が使っているキーがあるかを確かめる
```

- [ ] **Step 2: `AGENTS.md` の「構成」に全体の図を足す**

`AGENTS.md` の「構成」の表の前に足す:

````markdown
```
agents-monitor/
  app/                 画面とアプリ（Tauri の 1 つのアプリ）
    src/               画面（features/・lib/）
    src-tauri/src/     OS とのつなぎ（commands/・session/・setup/・process/・update/・tray/・base/）
  crates/
    core/src/          判断のロジック（session/・setup/・process/・update/・base/）
    hook/src/          hook の本体
  plugin/  .claude-plugin/  scripts/  docs/
```

`src` という名前のフォルダーがいくつもあるのは、道具（Cargo・Vite）の決まり。それぞれの入り口の README に、中の決まりがある（ADR 0010）。
````

表の `app/src/`・`app/src-tauri/`・`crates/core/` の行の「役割」の列の終わりに、「（中は ADR 0010、入り口の README）」を足す。

- [ ] **Step 3: `README.md`・`README.en.md` の「開発」に図を足す**

「開発」の章の始めのほうに、Step 2 と同じ図を足す。英語の版は、図の説明を英語にする（`app/` Screens and the app (one Tauri app)、`src/` Screens、`src-tauri/src/` OS integration、`crates/core/src/` Decision logic、`crates/hook/src/` The hook）。

- [ ] **Step 4: CONTRIBUTING を確かめる**

```bash
grep -n "src/\|crates/" CONTRIBUTING.md CONTRIBUTING.en.md
```

構成やファイルの場所に触れている所があれば、新しい場所に直す。なければ何もしない。

- [ ] **Step 5: 今の文書に、古いパスが残っていないかを確かめる**

```bash
git grep -nE "app/src/(list|settings|drag|format|i18n|icons|types)\.ts|app/src-tauri/src/(commands|installer|jumper|lang|monitor|notifier|sync|tray|tray_icon|updater)\.rs|crates/core/src/[a-z_]+\.rs" -- AGENTS.md README.md README.en.md CONTRIBUTING.md CONTRIBUTING.en.md docs/adr docs/device-checks.md docs/releasing.md .claude
```

Expected: 出てこない（ADR 0006 などで出たら、新しい場所に直す）

- [ ] **Step 6: コミット**

```bash
git add crates/core/src/README.md app/src-tauri/src/README.md app/src/README.md AGENTS.md README.md README.en.md CONTRIBUTING.md CONTRIBUTING.en.md docs
git commit -m "3 つの置き場所の入り口に README を置き、AGENTS.md と README に全体の図を載せる

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: 仕上げの確かめと Pull Request

- [ ] **Step 1: 手元でアプリを作り、起動し直す**

```bash
./scripts/build-hook-sidecar.sh
(cd app && npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}')
pkill -f "MacOS/agent-loaf-app"
open "target/release/bundle/macos/Agent Loaf.app"
ps -axo command | grep "MacOS/agent-loaf-app" | grep -v grep
```

Expected: 手元のアプリだけが 1 つ動く

- [ ] **Step 2: 実機で手短に確かめる（メンテナーに頼む）**

CV で、場面 1・4（状態が変わる）、6（画面の移動）、12（起動し直し）、14（ドラッグ）と、設定の画面（エージェントの設定の行、アップデートの欄の「0.5.3 dev」）。結果を `docs/device-checks.md` の記録に 1 行足す。

- [ ] **Step 3: 記録をコミットし、Pull Request にする**

```bash
git add docs/device-checks.md
git commit -m "実機の確認の記録に、ディレクトリ構成の整理の後に確かめた結果を足す

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
git push -u origin refactor/directory-layout
```

`/pr-description` の決まりで本文を作り、`gh pr create` する。CI が通ったら、メンテナーに聞いてから `gh pr merge --rebase --delete-branch` でマージする。
