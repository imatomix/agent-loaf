# Agent Loaf のリポジトリの決まりとハーネス

- 状態: 設計中
- 日付: 2026-10-12

## 1. 目的

公開（v0.5.0）の前に、リポジトリの構成・コードの決まり・AI で開発するための設定を整える。

- これまで構成や決まりは書かずに進めてきた。決まりの多くは開発者の AI の個人用のメモにしかなく、ほかの人やほかの AI には伝わらない
- 公開した後に手伝ってくれる人と、その人の AI も、同じ決まりで書けるようにする
- Tauri・Rust の一般的なやり方に合わせる。開発者は Tauri の経験が浅いので、決まりは調べた結果に基づいて決める

## 2. 調べた結果

Tauri の公式の説明と、よく知られた Tauri 製のアプリ（Cap、Yaak、Clash Verge Rev、Spacedrive、Pot）と Tauri 本体を比べた。

| 観点 | 一般的な形 | Agent Loaf の今 |
|---|---|---|
| 全体の構成 | 小さいもの: ルートに画面の `src/` と `src-tauri/`（公式の既定、Pot）。共通の部品を持つもの: `apps/<アプリ>/` と `crates/<部品>/`（Cap、Yaak、Spacedrive） | `app/`（画面と Tauri）、`crates/core`・`crates/hook`、`plugin/` |
| Rust のファイルの並べ方 | 平らに並べるのがふつう（Cap のアプリ本体は約 80 ファイル）。分けるときはフォルダーより crate で分ける（Yaak は 20 以上の crate） | `crates/core/src/` に 29 ファイルが平ら |
| 整形 | どれも `rustfmt.toml` を置く（行の幅 100〜120） | 設定なし |
| CI | どれも整形・clippy・テストを PR ごとに確かめる | 公開用のワークフローだけ |
| AI 用の文書 | 本体を `AGENTS.md` に書き、`CLAUDE.md` は `AGENTS.md` を読むように書くだけ（Cap、Clash Verge Rev、Spacedrive） | なし |
| 手伝う人向けの文書 | `CONTRIBUTING.md`（4 つ） | なし |

AI の設定や開発の進め方は、公開するプロジェクトでは秘密として扱わず公開するのがふつう。会社で社外秘にするのは、中身に社内の事情が入るため。

## 3. 決めたこと

### 3.1 構成は変えない

- `app/`・`crates/`・`plugin/`・`scripts/`・`docs/` の形は、Cap などの「アプリと部品を分ける」形と同じ考え方。アプリが 1 つなので `apps/` ではなく `app/` になっている。名前を変えても、移動の手間に見合う得はない
- ルートの `.claude-plugin/` は、GitHub からプラグインを入れるときのマーケットプレイスの置き場所なので動かせない
- `crates/core/src/` は平らなままにする。ある部分をほかでも使うようになったら、そのとき crate として切り出す

### 3.2 整形

- ルートに `rustfmt.toml` を置く。`edition = "2021"`、`max_width = 120`。ほかはすべて既定のまま
  - 行の幅は 120 にする（Clash Verge Rev と同じ）。今のコードは長い行が多く、100 では差分が 388 か所、120 では 262 か所（約 2,300 行）になる
- 今のコードを `cargo fmt --all` で整形し、「整形だけ」の 1 コミットにする。ほかの変更を混ぜない
- 画面の側（TypeScript）の整形の道具（Prettier など）は、今は入れない。コードが 9 ファイルと少ないため

### 3.3 clippy

- 今あるテストの中の警告（6 件）を直し、`cargo clippy --workspace --all-targets` が警告なしで通るようにする
- CI では警告を失敗として扱う（`-D warnings`）

### 3.4 CI

- `.github/workflows/ci.yml` を足す。`main` への push と、すべての PR で動かす
- macOS（Apple Silicon）で動かす。プロセスを読む部分が macOS 専用のため
- 確かめること（この順）
  1. `cargo fmt --all --check`
  2. hook の同梱用ビルド（`./scripts/build-hook-sidecar.sh`）。Tauri のビルドが同梱する hook の本体を必要とするため
  3. `cargo clippy --workspace --all-targets -- -D warnings`
  4. `cargo test --workspace`
  5. `npm ci` の後に `npx tsc --noEmit`（`app/` の中で）
- `cargo test` が画面のビルドの成果物（`app/dist`）を必要とするかは、実装の最初に確かめる。必要なら、その前に `npm run build` を足す
- 公開用のワークフロー（`release.yml`）は変えない

### 3.5 版の番号を上げる場所を減らす

- 今は 6 か所（`tauri.conf.json`、3 つの `Cargo.toml`、`app/package.json`、`plugin.json`）
- ルートの `Cargo.toml` に `[workspace.package] version` を置き、3 つの crate は `version.workspace = true` で受け継ぐ
- `tauri.conf.json` から `version` を外す。Tauri は、外すと `Cargo.toml` の版を使う（`tauri-utils` の設定の説明で確かめた）
- これで 3 か所（ルートの `Cargo.toml`、`app/package.json`、`plugin.json`）になる。`every_version_matches_the_app` のテストと `docs/releasing.md` をこれに合わせる
- アプリの中の版（設定の画面の版の表示、自動更新の比べる元）が変わらないことを、ビルドしたアプリで確かめる

### 3.6 文書

#### `AGENTS.md`（ルールの本体、公開）

Codex は `AGENTS.md` を、Claude Code は `CLAUDE.md` を読む。本体を `AGENTS.md` に書き、二重に持たない。

- 概要: アプリが何か。`crates/core`・`crates/hook`・`app/src-tauri`・`app/src`・`plugin/`・`.claude-plugin/` の役割。データの置き場所 `~/.agent-loaf`
- コマンド: テスト、clippy、整形、型チェック、hook の同梱用ビルド、アプリのビルド
- 決まり（理由は ADR へのリンクで示す）
  - 常駐アプリなので、軽さを仕様の条件にする。定期処理・外部コマンド・描き直しは負荷を見積もる
  - hook は何も出力せず常に 0 で終わる。プロセスは自分の親だけをたどる。`SessionEnd` と `Interrupt` の待ち時間は 3 秒以下
  - Claude Code と Codex の設定ファイルは直接書き換えず、それぞれの CLI を通す
  - 文字に状態色を使わない。色はバッジなど文字以外で
  - 画面のデザインの元は Figma にある（メンテナーだけが見られる）。画面を変えるときは先に Figma を見て、描かれていない振る舞いは推測で足さずに確かめる。メンテナー以外が画面を変えたいときは、先に Issue で相談してもらう
  - アプリのアイコンは正方形のまま。角丸は macOS が付ける
  - 版の番号は 3 か所で上げる（`docs/releasing.md`）
  - コメントは理由（WHY）だけを日本語で書く。コミットと PR も日本語
  - 新しいライブラリは入れる前に確認する
  - 変更の後は、テスト・clippy・型チェックを通してから完了とする
  - 振る舞いや構成を決める前に、類似のアプリや一般的なやり方を調べる。自分たちの考えだけで決めない
  - 外のツール（Claude Code・Codex・Tauri など）の振る舞いは、公式の文書と実機で確かめる。確かめた版と日付を書く（版が上がると変わるため）
  - 画面やプロセスに関わる変更は、テストに加えて実機での確認を完了の条件にする
- 仕様と設計の文書の置き場所: 横断的な決定は `docs/adr/`、機能ごとの設計と計画は `docs/superpowers/specs/`・`docs/superpowers/plans/`

#### `CLAUDE.md`（公開）

- `@AGENTS.md` の 1 行だけ

#### `docs/adr/`（公開）

- `README.md`（ADR と `docs/superpowers/` の使い分け、書き方）と `0000-template.md`
- 最初に書く ADR（これまでの設計書で決めたことをまとめ、元の設計書へリンクする）
  - 0001 リポジトリの構成（3.1）
  - 0002 整形・clippy・CI（3.2〜3.4）
  - 0003 軽さを仕様の条件にする
  - 0004 hook の決まり
  - 0005 Claude Code と Codex の設定ファイルは CLI を通して変える
  - 0006 画面の決まり（文字に状態色を使わない、Figma にない振る舞いを足さない、アイコンは正方形）。Figma から書き出した素材の出どころと作り方も書く
    - 一覧のエージェントのアイコン（`app/src/assets/claude-code.svg`・`codex.svg`）: ノード 157:2477・157:2480 から書き出し、背景の四角を外したもの
    - メニューバーのアイコン（`app/src-tauri/icons/tray/`）: MenuBarIcon から書き出した SVG を `scripts/make-tray-icons.sh` で 36px の PNG にしたもの
    - アプリのアイコン（`app/src-tauri/icons/`）: Figma のアプリのアイコンから作ったもの（作り方は実装のときに確かめて書く）
    - ファイルの場所（URL）は書かない。Figma を公開しないため
  - 0007 版と配布（公開も 0.x、Codex 対応と実際に使われて出た不具合の修正の後に 1.0.0、Developer ID の署名と公証、Apple Silicon だけ、Homebrew には載せない）
  - 0008 AI で開発する進め方（superpowers の brainstorming → 設計書 → 計画 → 実装 → 最後に別の AI が見直す。ADR と設計書の使い分け。決める前に類似のアプリや一般的なやり方を調べる。外のツールの振る舞いは公式の文書と実機で確かめ、版と日付を書く。画面やプロセスに関わる変更は実機での確認までを完了とする）

#### `CONTRIBUTING.md`（公開）

- README に合わせて日本語（`CONTRIBUTING.md`）と英語（`CONTRIBUTING.en.md`）を置く
- 中身: 開発の環境、ビルドとテストのコマンド、PR の前に通すこと（CI と同じ）、決まりは `AGENTS.md` を見ること

### 3.7 ハーネス（`.claude/`、公開）

#### `.claude/settings.json`

- 許可: `cargo test`・`cargo build`・`cargo clippy`・`cargo fmt`、`npx tsc --noEmit`、`npm run build`、`./scripts/build-hook-sidecar.sh`、`git status`・`git diff`・`git log`
- 確認する: `git push`
- 禁止: `~/.appkeys/` を読むこと（自動更新の秘密鍵の置き場所）。`git push --force`・`git push -f`・`git reset --hard`・`git clean -f`・`git branch -D`・`git rebase -i`・`rm -rf`
- `enabledPlugins`: superpowers
- hook: ファイルを書いた・直した後（`PostToolUse` の `Write|Edit`）、それが `.rs` なら `rustfmt` をかける。失敗しても作業を止めない

#### `.claude/agents/code-reviewer.md`

- このプロジェクト用の見直し役。`AGENTS.md` の決まりを前提に、軽さ・hook の決まり・設定ファイルの扱い・テストの追随・余計な複雑さを見る。テスト・clippy・型チェックも動かす

### 3.8 公開しないもの

- `CLAUDE.local.md` と `.claude/settings.local.json` を `.gitignore` に足す
- `CLAUDE.local.md` には、個人の事情に関わる手順を書く（中身は公開しない）
- 開発者の AI の個人用のメモのうち、`AGENTS.md` と ADR に移したもの（軽さ、文字に状態色を使わない、Figma、アイコンは正方形、アプリ名）は消す。同じことを 2 か所に持たないため
- ルートの `icon.ai`（アイコンの元のファイル、コミットしていない）は、開発者が手でリポジトリの外に移す

## 4. 進める順番

1. 整形（`rustfmt.toml` と、整形だけのコミット）
2. clippy の警告を直す
3. 版の番号をまとめる（テストと `docs/releasing.md` も）
4. CI を足す
5. ADR
6. `AGENTS.md`・`CLAUDE.md`・`CONTRIBUTING.md`
7. `.claude/`（設定、整形の hook、見直し役）と `.gitignore`
8. 公開しないもの（`CLAUDE.local.md`、個人用のメモの整理）

整形を最初にするのは、後の変更の差分に整形の差分を混ぜないため。ハーネスを後にするのは、そこに書く決まりが前の段階で決まるため。

## 5. 確かめ方

- `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`npx tsc --noEmit` がすべて通る
- 整形のコミットの前後で、テストの数と結果が変わらない
- 版をまとめた後、ビルドしたアプリの設定の画面に同じ版が出る
- CI は、push できるようになってから（公開の時に）GitHub で動くことを確かめる。それまでは、ワークフローに書いたコマンドを手元で同じ順に動かして確かめる
- `.claude/settings.json` の整形の hook は、Claude Code で `.rs` を直したときに整形されることを確かめる

## 6. 対象外

- 画面の側の整形と lint の道具
- ディレクトリの名前の変更や、`crates/core` のフォルダー分け、crate の切り出し
- 公開用のワークフローの変更
- コマンド（`.claude/commands/`）。決まった手順で繰り返す作業がまだないため
