# リポジトリの決まりとハーネス 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 公開の前に、整形・clippy・CI・版の番号のまとめ・ADR・AGENTS.md・ハーネス（`.claude/`）をそろえ、決まりをリポジトリに残す。

**Architecture:** コードの振る舞いは変えない。整形だけのコミットを最初に入れ、その後は設定と文書を足す。版の番号は Cargo の workspace にまとめ、Tauri は `Cargo.toml` の版を使う。

**Tech Stack:** rustfmt、clippy、GitHub Actions（macOS）、Claude Code の設定（`.claude/settings.json`・agents）

**Spec:** docs/superpowers/specs/2026-10-12-agent-loaf-repo-conventions-design.md

## Global Constraints

- コードの振る舞いを変えない。テストの数（220）と結果は、整形の前後で同じ
- `rustfmt.toml` は `edition = "2021"` と `max_width = 120` だけ
- 整形は「整形だけ」の 1 コミット。ほかの変更を混ぜない
- 新しいライブラリや道具を足さない（Prettier などは入れない）
- 公開用のワークフロー（`.github/workflows/release.yml`）は変えない
- push しない。CI は手元で同じコマンドを同じ順に動かして確かめる
- 文書・コメント・コミットは日本語。コメントは理由（WHY）だけ
- Figma のファイルの場所（URL・キー）は公開するファイルに書かない
- `~/.appkeys/` は読まない

## Review Focus

- 版を `tauri.conf.json` から外した後、ビルドしたアプリの `Info.plist` の `CFBundleShortVersionString` と、設定の画面の版の表示が 0.5.0 のままであること（Task 3 で `plutil` で確かめる）
- 整形の hook は、`.rs` 以外のファイル、リポジトリの外のファイル、空白を含むパス、`jq` がない環境で、作業を止めずに何もしないこと（Task 7 で確かめる）
- CI の順番: Tauri のビルドは同梱する hook の本体を要るので、clippy とテストの前に `build-hook-sidecar.sh` を動かすこと（Task 4 で、本体を消した状態から順に動かして確かめる）
- `AGENTS.md`・`CONTRIBUTING.md` に書いたコマンドが、そのまま動くこと（Task 6 で全部動かす）
- `.claude/settings.json` の禁止が、ふだんの作業（`git diff`、`git log`、テスト）を止めないこと（Task 7 で JSON の形と中身を確かめる）

---

### Task 1: 整形

**Files:**
- Create: `rustfmt.toml`
- Modify: すべての `.rs`（`cargo fmt --all` による）

- [ ] **Step 1: 整形の前のテストの数を控える**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | awk '{s+=$4; f+=$6} END {print "passed",s,"failed",f}'`
Expected: `passed 220 failed 0`

- [ ] **Step 2: `rustfmt.toml` を置く**

```toml
edition = "2021"
max_width = 120
```

- [ ] **Step 3: 整形がまだ通らないことを確かめる**

Run: `cargo fmt --all --check > /dev/null; echo $?`
Expected: `1`

- [ ] **Step 4: 整形する**

Run: `cargo fmt --all && cargo fmt --all --check; echo $?`
Expected: `0`

- [ ] **Step 5: テストの数が変わらないことを確かめる**

Run: Step 1 と同じ
Expected: `passed 220 failed 0`

- [ ] **Step 6: コミット（整形だけ）**

```bash
git add rustfmt.toml $(git diff --name-only)
git commit -m "rustfmt.toml を置き、Rust のコードを整形する（整形だけ）"
```

### Task 2: clippy の警告を直す

**Files:**
- Modify: `crates/core/src/config_dirs.rs:97-103`、`crates/core/src/update.rs:131`、`app/src-tauri/src/updater.rs:142`（行は Task 1 の整形でずれる。`cargo clippy` の出す場所に従う）

- [ ] **Step 1: 警告が 6 件あることを確かめる**

Run: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -cE "^error: (unnecessary use of .clone.|this assertion has a constant value)"`
Expected: `6`

- [ ] **Step 2: 直す**

`config_dirs.rs` のテスト（4 か所）: `Some(&[claude.clone()])` を `Some(std::slice::from_ref(&claude))` に、`Some(&[codex_s.clone()])` を `Some(std::slice::from_ref(&codex_s))` にする。

`update.rs` の `failed_check_is_retried_soon`: 定数どうしの比べは、コンパイルのときに確かめる。

```rust
        const { assert!(RETRY_AFTER_FAILURE_MS < CHECK_INTERVAL_MS) };
```

`updater.rs` の `development_builds_never_check`: `ENABLED` は作るときの環境変数で決まる定数なので、コンパイルのときではなく実行のときに確かめたい（`AGENT_LOAF_RELEASE=1` で作ったテストは失敗してほしい）。

```rust
    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn development_builds_never_check() {
```

- [ ] **Step 3: 警告がなくなったことを確かめる**

Run: `cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --workspace 2>&1 | grep -E "^test result" | awk '{s+=$4; f+=$6} END {print "passed",s,"failed",f}'`
Expected: `Finished ...` の行（error なし）と `passed 220 failed 0`

- [ ] **Step 4: コミット**

```bash
git add crates/core/src/config_dirs.rs crates/core/src/update.rs app/src-tauri/src/updater.rs
git commit -m "clippy の警告（テストの中の 6 件）を直す"
```

### Task 3: 版の番号をまとめる

**Files:**
- Modify: `Cargo.toml`、`crates/core/Cargo.toml`、`crates/hook/Cargo.toml`、`app/src-tauri/Cargo.toml`、`app/src-tauri/tauri.conf.json`、`app/src-tauri/tests/versions.rs`、`docs/releasing.md`

- [ ] **Step 1: テストを書き直す（先に失敗させる）**

`app/src-tauri/tests/versions.rs` の `cargo_version` と `every_version_matches_the_app` を、次に置き換える（`json_version` と、テストの上のコメントは残す）。

```rust
fn inherits_workspace_version(path: &Path) -> bool {
    fs::read_to_string(path).unwrap().lines().any(|line| line.trim() == "version.workspace = true")
}

// プラグインの版が変わらないと、入れてあるプラグインは更新されず、hooks.json の変更が利用者に届かない
#[test]
fn every_version_matches_the_app() {
    let app = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = app.join("../..");
    let expected = env!("CARGO_PKG_VERSION");
    for file in ["app/package.json", "plugin/.claude-plugin/plugin.json"] {
        assert_eq!(json_version(&repo.join(file)), expected, "{file} の版をアプリ（{expected}）に合わせること");
    }
    for file in ["crates/core/Cargo.toml", "crates/hook/Cargo.toml", "app/src-tauri/Cargo.toml"] {
        assert!(inherits_workspace_version(&repo.join(file)), "{file} は version.workspace = true でルートの版を使うこと");
    }
    // Tauri は tauri.conf.json に版がないと Cargo.toml の版を使う。書くと 2 か所になる
    let conf: serde_json::Value = serde_json::from_str(&fs::read_to_string(app.join("tauri.conf.json")).unwrap()).unwrap();
    assert!(conf.get("version").is_none(), "tauri.conf.json に version を書かないこと");
}
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cargo test -p agent-loaf-app --test versions 2>&1 | grep -E "panicked|version.workspace|FAILED"`
Expected: FAIL（`crates/core/Cargo.toml は version.workspace = true ...`）

- [ ] **Step 3: 版をまとめる**

ルートの `Cargo.toml` に足す:

```toml
[workspace.package]
version = "0.5.0"
```

3 つの crate の `Cargo.toml` の `version = "0.5.0"` を `version.workspace = true` にする。`app/src-tauri/tauri.conf.json` の `"version": "0.5.0",` の行を消す。

- [ ] **Step 4: テストが通ることを確かめる**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | awk '{s+=$4; f+=$6} END {print "passed",s,"failed",f}'; git diff --stat Cargo.lock`
Expected: `passed 220 failed 0`。`Cargo.lock` は変わらない（版の値が同じため）

- [ ] **Step 5: ビルドしたアプリの版を確かめる**

Run:
```bash
./scripts/build-hook-sidecar.sh > /dev/null && (cd app && npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}' > /dev/null 2>&1); plutil -extract CFBundleShortVersionString raw "target/release/bundle/macos/Agent Loaf.app/Contents/Info.plist"
```
Expected: `0.5.0`

- [ ] **Step 6: `docs/releasing.md` を直す**

「公開のたびに」の 1 を次にする。

```markdown
1. 版の番号を上げる（3 か所。`cargo test --workspace` の `every_version_matches_the_app` が確かめる）
   - ルートの `Cargo.toml` の `[workspace.package]`、`app/package.json`、`plugin/.claude-plugin/plugin.json`
   - 3 つの crate は `version.workspace = true` でルートの版を使う。`tauri.conf.json` には版を書かない（Tauri が `Cargo.toml` の版を使う）
   - `cargo build --workspace && (cd app && npm install)` で `Cargo.lock` と `app/package-lock.json` も更新する
```

- [ ] **Step 7: コミット**

```bash
git add Cargo.toml crates/core/Cargo.toml crates/hook/Cargo.toml app/src-tauri/Cargo.toml app/src-tauri/tauri.conf.json app/src-tauri/tests/versions.rs docs/releasing.md
git commit -m "版の番号を Cargo の workspace にまとめ、上げる場所を 6 か所から 3 か所にする"
```

- [ ] **Step 8: 実機の確認（ユーザー）**

Step 5 で作ったアプリを起動し直し、設定の画面の版の表示が 0.5.0 であることを見てもらう。

### Task 4: CI

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: ワークフローを書く**

```yaml
name: ci

# Secrets を使わないので、ほかの人の Pull Request でも動かしてよい
on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

jobs:
  check:
    # プロセスを読む部分が macOS 専用のため
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: npm
          cache-dependency-path: app/package-lock.json
      - name: 整形
        run: cargo fmt --all --check
      # Tauri のビルドは同梱する hook の本体を要るので、clippy とテストより前に作る
      - name: 同梱する hook を作る
        run: ./scripts/build-hook-sidecar.sh
      - name: clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      - name: テスト
        run: cargo test --workspace
      - run: npm ci
        working-directory: app
      - name: 型チェック
        run: npx tsc --noEmit
        working-directory: app
```

- [ ] **Step 2: 同梱する hook の本体がない状態から、同じ順に手元で動かす**

Run:
```bash
rm -rf app/src-tauri/binaries && cargo fmt --all --check && ./scripts/build-hook-sidecar.sh && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --workspace 2>&1 | grep -E "^test result" | awk '{s+=$4; f+=$6} END {print "passed",s,"failed",f}' && (cd app && npm ci > /dev/null && npx tsc --noEmit && echo tsc-ok)
```
Expected: clippy の `Finished` の行、`passed 220 failed 0`、`tsc-ok`

- [ ] **Step 3: YAML の形を確かめる**

Run: `ruby -ryaml -e 'puts YAML.load_file(".github/workflows/ci.yml")["jobs"]["check"]["steps"].size'`
Expected: `10`

- [ ] **Step 4: コミット**

```bash
git add .github/workflows/ci.yml
git commit -m "push と Pull Request のたびに整形・clippy・テスト・型チェックを動かす CI を足す"
```

### Task 5: ADR

**Files:**
- Create: `docs/adr/README.md`、`docs/adr/0000-template.md`、`docs/adr/0001-*.md`〜`docs/adr/0008-*.md`

各 ADR はテンプレートの形（ステータス・日付・コンテキスト・決定・検討した代替案・影響）で書く。ステータスはすべて「承認」、日付は決めた日。元の設計書があるものはリンクする。過去の設計書の旧名（agents-monitor）は、リンクの中ではそのまま使う。

- [ ] **Step 1: `README.md` と `0000-template.md`**

`0000-template.md`:

```markdown
# NNNN. タイトル

- ステータス: 提案中 / 承認 / 却下 / 置き換え済み（→ 置き換えた ADR へのリンク）
- 日付: YYYY-MM-DD

## コンテキスト

なぜこの決定が必要になったか。前提、制約、関係する人の要望。

## 決定

何を決めたか。

## 検討した代替案

ほかにどんな選択肢があり、なぜ選ばなかったか。

## 影響

この決定で何が変わるか。トレードオフと、今後の運用で要ること。
```

`README.md` に書くこと:
- ADR は、特定の機能に閉じない横断的な決定（構成、道具、決まり、配布の方針）の記録。機能ごとの「何のためにどう作るか」は `docs/superpowers/specs/`（設計書）と `docs/superpowers/plans/`（計画）に置く
- 書き方: `0000-template.md` を写し、続きの番号を付け、埋めてコミットする
- 後から変えるとき: 本文を書き換えず、新しい ADR で置き換え、古い方のステータスを「置き換え済み（→ 新しい ADR）」にする
- 一覧（0001〜0008 へのリンク）

- [ ] **Step 2: 0001〜0004**

- `0001-repository-layout.md`「リポジトリの構成」（2026-10-12）
  - コンテキスト: 構成を決めずに進めてきた。公開の前に一般的な形と比べた（設計書 2026-10-12-agent-loaf-repo-conventions-design.md の 2 節の表）
  - 決定: `app/`（画面と Tauri）、`crates/core`（判断と状態。純粋な関数を中心にする）、`crates/hook`（hook の本体）、`plugin/`（Claude Code と Codex に入れるプラグイン）、`.claude-plugin/`（GitHub から入れるときのマーケットプレイス。ルートから動かせない）、`scripts/`、`docs/`。`crates/core/src/` は平らに並べ、ほかでも使う部分が出たら crate として切り出す
  - 代替案: ルートに `src/` と `src-tauri/`（公式の既定。共通の部品の crate を持つ形に合わない）、`apps/desktop/`（アプリが 1 つなので名前を変える得がない）、`crates/core` をフォルダーに分ける（類似のアプリでも平らに並べるのがふつう）
  - 影響: 新しいファイルは役割に合う場所に置く。迷ったら判断は `crates/core`、Tauri や OS とのつなぎは `app/src-tauri`。アプリ名は 2026-10-12 に agents-monitor から Agent Loaf に変えた。それより前の設計書・計画・コミットは旧名のまま残す（その時点の記録なので書き換えない）
- `0002-format-lint-ci.md`「整形・clippy・CI」（2026-10-12）
  - 決定: `rustfmt.toml`（edition 2021、行の幅 120）、clippy は警告を失敗にする、`.github/workflows/ci.yml` で push と PR ごとに整形・clippy・テスト・型チェックを macOS で動かす。画面の側の整形の道具は入れない
  - 代替案: 行の幅 100（Tauri 本体と同じだが、今のコードでは差分が大きい）、Prettier（コードが少なく、新しい道具を足すほどではない）
  - 影響: 変更の後は `cargo fmt --all`、clippy、テスト、型チェックを通す。Claude Code では `.rs` を直すと整形の hook が動く
- `0003-lightweight.md`「軽さを仕様の条件にする」（2026-10-11）
  - コンテキスト: 常に動くメニューバーのアプリ。2026-10-11 に測ると、何もしていないのに CPU が約 3% だった（1 秒ごとの画面の描き直し、3 秒ごとの `ps`）
  - 決定: 定期的な処理・外部コマンドの起動・画面の描き直しを足すときは、負荷を見積もり、できれば測る。何もしていないときは CPU 0.1% 前後を目安にする。重くなる案は、軽い代わりの案と並べて示す
  - 対策の例: 一覧を閉じている間は画面を描き直さない、`ps`・`lsof` をやめて libc の `proc_*`・`sysctl` で読む、hook は自分の親だけをたどる、`claude` を起動するプラグインの状態の確認は設定の画面を開いたときだけ、一覧を閉じている間は知っている pid だけを確かめる。結果、何もしていないときの CPU は約 0.2%
  - 代替案: kqueue・FSEvents で待つ（さらに下がるが、まだ行っていない）
- `0004-hook-rules.md`「hook の決まり」（2026-10-09〜2026-10-12）
  - コンテキスト: hook は Claude Code と Codex の動作の途中で動く。失敗や遅れは、会話の邪魔になる
  - 決定: 何も出力せず、パニックも含めて常に 0 で終わる。ツールを使うたびに動くので、全プロセスではなく自分の親だけをたどる。会話かどうか（`claude -p`、Agent SDK、`codex exec`・`review`）を見て、会話でなければ何もしない。イベントは `~/.agent-loaf/events` に置くだけにし、判断はアプリが行う。置き場が 10,000 件を超えたら置かない。`SessionEnd` と `Interrupt` の待ち時間は 3 秒以下（Codex の上限）
  - 元の設計書: 2026-10-09-agents-monitor-design.md、2026-10-12-agent-loaf-codex-design.md

- [ ] **Step 3: 0005〜0008**

- `0005-external-tools.md`「Claude Code と Codex への関わり方」（2026-10-11〜2026-10-12）
  - 決定: 設定ファイル（`~/.claude/settings.json`、`~/.codex/config.toml` など）は直接書き換えない。プラグインの導入・更新・削除は `claude plugin`・`codex plugin` の CLI を通す。読むだけなら読んでよい（Codex の hook の許可の状態など）。アプリの操作で会話（セッション）を勝手に作らない（VS Code の会話の URI は、別のウィンドウに空の会話を作るので使わない。移動はウィンドウを前に出すだけ）。Codex の hook の許可は利用者がターミナルの Codex で行う
  - 代替案: 設定ファイルに hook を直接書く（利用者の設定を壊すおそれがある）、会話の URI で会話まで開く（空の会話が作られる）
  - 元の設計書: v0.2、v0.3、Codex の設計書
- `0006-ui-rules.md`「画面の決まり」（2026-10-10〜2026-10-12）
  - 決定: 文字に状態色を使わず、前景色（primary・secondary）だけにする。色は状態の丸やバッジなど文字以外で使う。画面のデザインの元は Figma にある（メンテナーだけが見られる）。描かれていない振る舞いは推測で足さずに確かめる。メンテナー以外が画面を変えたいときは、先に Issue で相談してもらう。アプリのアイコンは角まで塗った正方形のまま（macOS 26 が角丸に収めて表示する。実機で比べて決めた）
  - 素材の出どころ（Figma のファイルの場所は書かない）
    - 一覧のエージェントのアイコン（`app/src/assets/claude-code.svg`・`codex.svg`）: ノード 157:2477・157:2480 から書き出し、背景の四角を外したもの
    - メニューバーのアイコン（`app/src-tauri/icons/tray/`）: MenuBarIcon から書き出した SVG を `scripts/make-tray-icons.sh` で 36px の PNG にしたもの
    - アプリのアイコン（`app/src-tauri/icons/`）: AppIcon（139:1982）を 1024px の正方形のまま `app-icon-source.png` に書き出し、`npx --prefix app tauri icon app/src-tauri/icons/app-icon-source.png -o app/src-tauri/icons` で作ったもの（書く前に `npx --prefix app tauri icon --help` で `-o` を確かめる）
- `0007-versioning-and-distribution.md`「版と配布」（2026-10-12）
  - 決定: 公開も 0.x で始める（最初は v0.5.0）。Codex への対応と、実際に使われて出た不具合の修正の後に 1.0.0。Developer ID で署名し公証した dmg を GitHub の Releases に出す。自動更新は `tauri-plugin-updater` で 1 日 1 回確かめる。Apple Silicon と macOS 14 以降だけ。Homebrew には載せない。版の番号は 3 か所（`docs/releasing.md`）
  - 元の設計書: 2026-10-12-agent-loaf-v0.5-distribution-design.md
- `0008-ai-assisted-development.md`「AI で開発する進め方」（2026-10-12）
  - 決定:
    - 大きな変更は superpowers の brainstorming → 設計書 → 計画 → 実装 → 最後に別の AI が全体を見直す。小さな変更は、短い設計を会話で決めてから直す
    - 横断的な決定は ADR、機能ごとの設計と計画は `docs/superpowers/`
    - 振る舞いや構成を決める前に、類似のアプリや一般的なやり方を調べる。自分たちの考えだけで決めない（例: 終わった会話を一覧から消す決まりは、類似のアプリ 6 つの作りを比べてから決めた）
    - 外のツール（Claude Code・Codex・Tauri など）の振る舞いは、公式の文書と実機で確かめ、確かめた版と日付を書く（版が上がると変わるため）
    - 画面やプロセスに関わる変更は、テストに加えて実機での確認を完了の条件にする（テストが通っても実機で見つかった不具合が何度もあった）
    - ルールの本体は `AGENTS.md`。Claude Code は `CLAUDE.md` から読み込む
  - 代替案: ルールを個人の設定にだけ置く（ほかの人と、その人の AI に伝わらない）

- [ ] **Step 4: リンクが切れていないことを確かめる**

Run:
```bash
for f in docs/adr/*.md; do grep -oE '\]\([^)#]+' "$f" | sed 's/](//' | grep -v '^http' | while read -r l; do [ -e "docs/adr/$l" ] || echo "切れている: $f -> $l"; done; done; echo done
```
Expected: `done` だけ

- [ ] **Step 5: コミット**

```bash
git add docs/adr
git commit -m "横断的な決定を ADR に残す（構成、整形と CI、軽さ、hook、外のツール、画面、版と配布、AI で開発する進め方）"
```

### Task 6: AGENTS.md・CLAUDE.md・CONTRIBUTING

**Files:**
- Create: `AGENTS.md`、`CLAUDE.md`、`CONTRIBUTING.md`、`CONTRIBUTING.en.md`

- [ ] **Step 1: `AGENTS.md`**

見出しと中身:
- `# Agent Loaf の開発の手引き`（この文書は AI と人の両方が読む）
- `## 概要`: Claude Code と Codex の会話の状態をメニューバーに出す macOS のアプリ。Tauri 2 と Rust。データは `~/.agent-loaf`
- `## 構成`: `app/src`（画面。TypeScript と Vite）、`app/src-tauri`（Tauri。OS とのつなぎ）、`crates/core`（判断と状態）、`crates/hook`（hook の本体）、`plugin/`（Claude Code と Codex に入れるプラグイン）、`.claude-plugin/`（マーケットプレイス）、`scripts/`、`docs/`。詳しくは ADR 0001
- `## コマンド`:
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo fmt --all`
  - `(cd app && npx tsc --noEmit)`
  - `./scripts/build-hook-sidecar.sh`（アプリのビルドとテストの前に一度。同梱する hook の本体を作る）
  - `(cd app && npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}')`（手元でアプリを作る。自動更新の署名の鍵がなくても作れる）
- `## 決まり`（各項目の後ろに ADR の番号）: 設計書 3.6 節の決まりをすべて。Global Constraints の「Figma の場所を書かない」に従い、Figma は「メンテナーだけが見られる」とだけ書く
- `## 文書の置き場所`: ADR は `docs/adr/`、機能ごとの設計と計画は `docs/superpowers/specs/`・`docs/superpowers/plans/`、公開の手順は `docs/releasing.md`

- [ ] **Step 2: `CLAUDE.md`**

```markdown
@AGENTS.md
```

- [ ] **Step 3: `CONTRIBUTING.md` と `CONTRIBUTING.en.md`**

中身（日本語と英語で同じ）:
- 必要なもの: macOS 14 以降（Apple Silicon）、Rust（stable）、Node.js 22
- 始め方: `(cd app && npm ci)`、`./scripts/build-hook-sidecar.sh`、`(cd app && npm run tauri dev)`
- Pull Request の前に通すこと: CI と同じ 4 つ（整形、clippy、テスト、型チェック）
- 決まり: `AGENTS.md` を読む。画面の見た目や振る舞いを変えたいときは、先に Issue で相談する
- 英語の版の先頭に、日本語の版へのリンク。日本語の版の先頭に、英語の版へのリンク（README と同じ形）

- [ ] **Step 4: 書いたコマンドを全部動かす**

Run:
```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -1 && cargo test --workspace 2>&1 | grep -E "^test result" | awk '{s+=$4; f+=$6} END {print "passed",s,"failed",f}' && (cd app && npx tsc --noEmit && echo tsc-ok) && ./scripts/build-hook-sidecar.sh | tail -1 && (cd app && npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}' > /dev/null 2>&1 && echo build-ok)
```
Expected: `Finished`、`passed 220 failed 0`、`tsc-ok`、`sidecar: ...`、`build-ok`

`npm run tauri dev` は起動したら止める（`timeout 60` の中で動かし、`Running` か `localhost:1420` が出ることを見る）。

- [ ] **Step 5: 公開してはいけないものが入っていないことを確かめる**

Run: `CLAUDE.local.md` に書いた Figma のキーと、個人の gh のアカウント名で、このブランチで変えたファイルをすべて探す（探す文字列そのものは公開するファイルに書かない）。`git diff --name-only main..HEAD | xargs grep -lE "<キー>|<アカウント名>|figma.com"; echo done`
Expected: `done` だけ

- [ ] **Step 6: コミット**

```bash
git add AGENTS.md CLAUDE.md CONTRIBUTING.md CONTRIBUTING.en.md
git commit -m "開発の手引き（AGENTS.md）と、手伝う人向けの CONTRIBUTING を足す。CLAUDE.md は AGENTS.md を読み込む"
```

### Task 7: ハーネス（`.claude/`）と `.gitignore`

**Files:**
- Create: `.claude/settings.json`、`.claude/agents/code-reviewer.md`
- Modify: `.gitignore`

- [ ] **Step 1: `.claude/settings.json`**

```json
{
  "permissions": {
    "allow": [
      "Bash(cargo test)",
      "Bash(cargo test *)",
      "Bash(cargo build)",
      "Bash(cargo build *)",
      "Bash(cargo clippy *)",
      "Bash(cargo fmt)",
      "Bash(cargo fmt *)",
      "Bash(npx tsc --noEmit)",
      "Bash(npm run build)",
      "Bash(./scripts/build-hook-sidecar.sh)",
      "Bash(git status)",
      "Bash(git diff)",
      "Bash(git diff *)",
      "Bash(git log)",
      "Bash(git log *)"
    ],
    "ask": [
      "Bash(git push *)"
    ],
    "deny": [
      "Read(~/.appkeys/**)",
      "Bash(git push --force*)",
      "Bash(git push -f*)",
      "Bash(git reset --hard*)",
      "Bash(git clean -f*)",
      "Bash(git branch -D*)",
      "Bash(git rebase -i*)",
      "Bash(rm -rf *)"
    ]
  },
  "enabledPlugins": {
    "superpowers@claude-plugins-official": true
  },
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [
          {
            "type": "command",
            "command": "f=$(jq -r '.tool_input.file_path // empty' 2>/dev/null); case \"$f\" in *.rs) rustfmt \"$f\" >/dev/null 2>&1 ;; esac; exit 0"
          }
        ]
      }
    ]
  }
}
```

hook のコマンドは、整形できなくても作業を止めないよう、常に 0 で終わる。`rustfmt` はファイルの場所から上へ `rustfmt.toml` を探すので、行の幅と edition は `rustfmt.toml` に従う。

- [ ] **Step 2: hook のコマンドを確かめる**

Run（コマンドの文字列を JSON から取り出して、いろいろな入力で動かす）:
```bash
cmd=$(jq -r '.hooks.PostToolUse[0].hooks[0].command' .claude/settings.json)
S=$(mktemp -d); printf 'fn main(){let x=1;}\n' > "$S/a b.rs"; cp "$S/a b.rs" crates/core/src/zz_fmt_probe.rs
for input in '{"tool_input":{"file_path":"'"$PWD"'/crates/core/src/zz_fmt_probe.rs"}}' '{"tool_input":{"file_path":"'"$S"'/a b.rs"}}' '{"tool_input":{"file_path":"README.md"}}' '{}' 'not json'; do echo "$input" | sh -c "$cmd"; echo "exit=$?"; done
cat crates/core/src/zz_fmt_probe.rs; rm crates/core/src/zz_fmt_probe.rs; rm -r "$S"
echo '{"tool_input":{"file_path":"x.rs"}}' | PATH=/nonexistent /bin/sh -c "$cmd"; echo "exit-without-jq=$?"
```
Expected: どれも `exit=0`。`zz_fmt_probe.rs` は整形されている（`let x = 1;` が別の行）。`exit-without-jq=0`

- [ ] **Step 3: `.claude/agents/code-reviewer.md`**

```markdown
---
name: code-reviewer
description: Agent Loaf の変更を見直す。コミットの前や実装の後に使う。
tools: Read, Grep, Glob, Bash
---

あなたはこのリポジトリの見直し役です。[AGENTS.md](../../AGENTS.md) の決まりと [docs/adr/](../../docs/adr/) を前提に見直してください。

## 手順

1. `git diff`（まだコミットしていない変更）か、指定された範囲の差分を読む
2. 次を確かめる
   - **軽さ**: 定期的な処理・外部コマンドの起動・画面の描き直しが増えていないか。増えるなら負荷の見積もりがあるか（ADR 0003）
   - **hook**: 何か出力していないか、0 以外で終わる道がないか、全プロセスを読んでいないか（ADR 0004）
   - **外のツール**: Claude Code・Codex の設定ファイルを直接書き換えていないか。会話を勝手に作る操作がないか（ADR 0005）
   - **画面**: 文字に状態色を使っていないか。Figma にない振る舞いを足していないか（ADR 0006）
   - **テスト**: 振る舞いが変わる変更に、テストが追いついているか
   - **余計な複雑さ**: 頼まれた範囲を超えた抽象化や作り替えが混ざっていないか
3. `cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`(cd app && npx tsc --noEmit)` を動かし、失敗があれば書く

## 書き方

問題を重い順に箇条書きで書く。それぞれにファイルと行、なぜ問題か、どう直すかを短く書く。問題がなければそう書く。褒めるだけの言葉や、好みの違いだけの指摘は書かない。
```

- [ ] **Step 4: `.gitignore`**

「エディタ・作業メモ」の節に足す:

```
CLAUDE.local.md
.claude/settings.local.json
.claude/worktrees/
```

- [ ] **Step 5: JSON の形と、無視の設定を確かめる**

Run: `jq -e '.permissions.deny | index("Read(~/.appkeys/**)")' .claude/settings.json && touch CLAUDE.local.md && git check-ignore CLAUDE.local.md .claude/settings.local.json; git status --short`
Expected: 数字、`CLAUDE.local.md`、`.claude/settings.local.json`。`git status` に `CLAUDE.local.md` が出ない

- [ ] **Step 6: コミット**

```bash
git add .claude/settings.json .claude/agents/code-reviewer.md .gitignore
git commit -m "Claude Code の設定（権限、superpowers、.rs を直したら整形する hook）と見直し役を足し、個人の設定を git の対象外にする"
```

- [ ] **Step 7: 実機の確認（ユーザー）**

新しい Claude Code の会話をこのリポジトリで開き、`.rs` を 1 つ直してもらい（または私が直し）、整形されることと、`/permissions` に設定が出ることを確かめる。

### Task 8: 公開しないもの（コミットしない）

**Files:**
- Create: `CLAUDE.local.md`（git の対象外）
- Modify: 開発者の AI の個人用のメモ（リポジトリの外）

- [ ] **Step 1: `CLAUDE.local.md`**

開発者の個人の事情に関わる手順を書く（中身は公開しないので、ここには書かない）。

- [ ] **Step 2: 個人用のメモを整理する**

`AGENTS.md` と ADR に移したメモ（軽さ、文字に状態色を使わない、Figma、アイコンは正方形、アプリ名）を、個人用のメモから消す。同じことを 2 か所に持たないため。

- [ ] **Step 3: コミットされないことを確かめる**

Run: `git status --short`
Expected: `CLAUDE.local.md` が出ない
