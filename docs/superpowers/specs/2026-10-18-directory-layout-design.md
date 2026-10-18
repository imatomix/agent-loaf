# ディレクトリ構成を機能ごとに整理する

- 日付: 2026-10-18
- 状態: 承認済み（設計の会話で 1〜3 を確かめた）

## 目的

誰が見ても（メンテナー・AI・外から手伝う人）、新しいコードをどこに置くか、今あるコードがどこにあるかが分かるようにする。構成の方針を決めて文書にし、それに沿ってファイルを並べ直す。

公開した直後の今は、手伝う人がまだおらず、開いている Pull Request や、ファイルのパスを指した Issue もない。並べ直す手間と影響がいちばん小さいので、今行う。

お手本は ~/Templates/nextjs-web-template の ADR 0002（`src` を機能ごとに分ける）と `src/features/README.md`。

## 終わったと言える状態

- 方針が ADR 0010「ディレクトリ構成」になっている
- 3 つの置き場所の入り口に README があり、中のフォルダーの役割と置き場所の決まりが書いてある
- ファイルが、この設計のとおりに並んでいる
- アプリの振る舞いは何も変わらない（`./scripts/check.sh` が通り、実機でも確かめた）

## してはいけないこと

- 振る舞いを変えない。新しいライブラリを入れない
- 大きなファイル（`crates/core/src/store.rs` の 1,241 行など）を分けない。中を作り直すのは別の作業にする
- 古い呼び方を残す仕組み（再エクスポート）を作らない。2 通りの呼び方が残ると迷うため

## 全体の方針

- 3 つの置き場所は今のまま変えない（ADR 0001）。判断は `crates/core`、OS とのつなぎは `app/src-tauri`、画面は `app/src`
- それぞれの中を、機能ごとのフォルダーに分ける。フォルダーの名前は、3 つの置き場所でできるだけそろえる（`session`・`setup` など）
- 2 つ以上の機能から使うものだけを `base`（Rust）・`lib`（画面）に置く。先回りして共通化しない
- `src` という名前のフォルダーは 4 つある（`crates/core/src`・`crates/hook/src`・`app/src-tauri/src`・`app/src`）。どれも道具（Cargo・Vite）の決まりなので、名前は変えない。代わりに、入り口の README の 1 行目に「ここは何の `src` か」を書き、`AGENTS.md` と `README.md` に全体の図を載せる

## `crates/core/src/`（判断のロジック）

```
crates/core/src/
  lib.rs
  session/   会話の状態を決める
             event・hook_input・spool・state・store・view・order・notify・transcript
  setup/     設定ディレクトリと、プラグインや hook の導入
             config_dirs・claude_cli・codex_plugin・install・install_plan・marketplace・plugin_status・attention
  process/   プロセスを読む・画面を移動する
             ps・procinfo・jump・git
  update/    自動更新の状態
             update
  base/      2 つ以上の機能から使う土台
             agent・paths・persist・settings・messages・crash_log
```

- `session` は、hook が置いたイベントを読み（`spool`・`event`・`hook_input`）、状態を決め（`state`・`store`）、一覧に出す形と並び（`view`・`order`）、通知を出すか（`notify`）、中断したか（`transcript`）を決める一続きの流れ
- `setup` は、設定の画面の「エージェントの設定」と、赤い丸のバッジ（`attention`）に関わるもの
- `base` の `agent`（Claude Code か Codex か）・`paths`・`messages` は、ほとんどの機能から使う
- フォルダーは `session/mod.rs` の形でモジュールにする。フォルダーの中で完結し、README と並べたときに分かりやすいため
- アプリと hook からの呼び方は `agent_loaf_core::store` から `agent_loaf_core::session::store` のように変わる（約 60 か所）。すべて直す

## `app/src-tauri/src/`（OS とのつなぎ）

Tauri 2 の標準の形（[Project Structure](https://v2.tauri.app/start/project-structure/)、2026-10-18 に確かめた）にそろえ、`crates/core` と同じ名前の機能のフォルダーに分ける。

```
app/src-tauri/src/
  main.rs      lib.rs の run() を呼ぶだけ
  lib.rs       アプリの組み立て（プラグイン・命令の登録・起動のときの処理）。今の main.rs の中身
  commands/    画面から呼ぶ命令（#[tauri::command]）を、機能ごとに
    session.rs   get_sessions・move_session・jump_session・take_pending_details
    setup.rs     get_setup・add_config_dir・retry_config_dir・remove_config_dir・set_claude_path・set_codex_path・open_codex_trust・refresh_setup
    settings.rs  get_settings・save_settings・open_notification_settings・refresh_notification_access
    update.rs    get_update・is_release_build・install_update
    app.rs       get_messages・quit_app
  session/     監視と通知：monitor・notifier
  setup/       導入：installer
  process/     画面の移動：jumper
  update/      自動更新：updater
  tray/        メニューバーのアイコンと一覧の窓：tray・tray_icon
  base/        土台：lang・sync
```

- Tauri 2 では、中身を `lib.rs` に書き、`main.rs` は呼ぶだけにする（スマートフォン向けに作るとき、アプリをライブラリとして作るため）。今は `main.rs` に中身があり、`lib.rs` がない。スマートフォン向けに作る予定はないが、標準の形にそろえる
- 命令を `commands/` の下に機能ごとのファイルで置くのは、公式の決まりではなく、よく見かける慣習。今は `commands.rs` 1 つに 21 の命令が入っている
- `tray` は `crates/core` にない名前。メニューバーのアイコンと窓は、OS とのつなぎにしかないため

## `app/src/`（画面）

テンプレートと同じく、`features/` と `lib/` に分ける。

```
app/src/
  features/
    list/       一覧の画面：list・drag・format
    settings/   設定の画面：settings
  lib/          2 つ以上の画面から使うもの：i18n・icons・types
  assets/       画像（今のまま）
  style.css     見た目（今のまま）
  vite-env.d.ts （今のまま）
```

- 画面の機能は、`session`・`setup` ではなく画面の名前で分ける。設定の画面には、エージェントの設定のほかに、通知・アップデート・ログイン時の起動も入っているため
- `format` は一覧でしか使っていないので `list` に置く
- `index.html` が読み込むファイルの場所を `/src/features/list/list.ts` に直す
- テスト `app/src-tauri/tests/messages_used.rs` は、画面が使う文言のキーを `app/src/` の直下の `.ts` だけから拾っている。並べ直すと、下のフォルダーのファイルを読まずに通ってしまうので、下のフォルダーまで読むように直す

## README と文書

- README を 3 つの置き場所の入り口に 1 つずつ置く：`crates/core/src/README.md`・`app/src-tauri/src/README.md`・`app/src/README.md`。1 行目に「ここは何の `src` か」、続けて中のフォルダーの役割と置き場所の決まりを書く
- `AGENTS.md` の「構成」の表を新しい形に直し、リポジトリの全体の図を載せる。`README.md`（開発の章）にも全体の図を載せる
- 今の決まりを書いた文書（ADR・`AGENTS.md`・README・CONTRIBUTING・`docs/device-checks.md`・`.claude` のコマンドと見直し役）のファイルのパスを新しい場所に直す
- 過去の設計書と計画（`docs/superpowers/specs/`・`plans/`）は、そのときの記録なので直さない。ADR 0010 に「古い文書のパスは整理の前のもの」と書く

## 進め方

ブランチ `refactor/directory-layout` で作業し、1 つの Pull Request にまとめる。コミットは次の順に分ける。

1. ADR 0010（方針）
2. `crates/core` を動かす（呼び方の書き換えを含む）
3. `app/src-tauri`：`main.rs` と `lib.rs` を分ける
4. `app/src-tauri`：命令を `commands/` の下に分ける
5. `app/src-tauri`：機能のフォルダーに動かす
6. `app/src` を動かし、`messages_used.rs` を下のフォルダーまで読むように直す
7. README（3 つ）と、`AGENTS.md`・`README.md`・CONTRIBUTING などの構成の説明を直す

- ファイルは `git mv` で動かす。動かすコミットでは、`use`・`mod`・パスの書き換えのほかは中身を変えない。`git log --follow` で動かす前の歴史も追えるようにするため
- 命令を分けるときも、関数の中身はそのままで、ファイルを分けるだけにする

## 確かめ方

- コミットごとに `./scripts/check.sh`（整形・clippy・テスト・型チェック）を通す
- 最後に手元で `tauri build` を通す
- 実機で手短に確かめる：場面 1・4（状態が変わる）、6（画面の移動）、12（起動し直し）、14（ドラッグ）、設定の画面（エージェントの設定・アップデートの欄の「dev」）
- 振る舞いは変わらないので、この整理のためだけに新しい版は出さない。次の版に一緒に入る
