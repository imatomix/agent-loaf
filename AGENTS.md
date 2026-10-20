# Agent Loaf の開発の手引き

この文書は、このリポジトリで作業する AI と人の両方が読む。決まりの理由は [docs/adr/](docs/adr/) にある。

## 概要

Claude Code と Codex の会話（セッション）の状態を、macOS のメニューバーに一覧で出すアプリ。Tauri 2 と Rust で作っている。hook がイベントを `~/.agent-loaf/events` に置き、アプリがそれを読んで状態を決める。アプリのデータも `~/.agent-loaf` に置く。

## 構成

```
agent-loaf/
  app/                 画面とアプリ（Tauri の 1 つのアプリ）
    src/               画面（features/・lib/）
    src-tauri/src/     OS とのつなぎ（commands/・session/・setup/・process/・update/・tray/・base/）
  crates/
    core/src/          判断のロジック（session/・setup/・process/・update/・base/）
    hook/src/          hook の本体
  plugin/  .claude-plugin/  scripts/  docs/
```

`src` という名前のフォルダーがいくつもあるのは、道具（Cargo・Vite）の決まり。それぞれの入り口の README に、中の決まりがある（ADR 0010）。

| 場所 | 役割 |
|---|---|
| `app/src/` | 画面（TypeScript と Vite）。中は ADR 0010 と入り口の README |
| `app/src-tauri/` | Tauri のアプリ。OS とのつなぎ。中は ADR 0010 と `src/README.md` |
| `crates/core/` | 判断と状態。純粋な関数を中心にする。中は ADR 0010 と `src/README.md` |
| `crates/hook/` | Claude Code と Codex から呼ばれる hook の本体 |
| `plugin/` | Claude Code と Codex に入れるプラグイン |
| `.claude-plugin/` | プラグインのマーケットプレイス（ルートから動かせない） |
| `scripts/` | ビルドと開発の補助 |
| `docs/` | ADR、設計書と計画、公開の手順 |

迷ったら、判断は `crates/core`、Tauri や OS とのつなぎは `app/src-tauri` に置く（ADR 0001）。

## コマンド

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
(cd app && npx tsc --noEmit)
./scripts/check.sh   # 上の整形の確認・clippy・テスト・型チェックを、CI と同じ順にまとめて動かす
./scripts/build-hook-sidecar.sh   # アプリのビルドとテストの前に一度。アプリに同梱する hook を作る
(cd app && npm run tauri build -- --config '{"bundle":{"createUpdaterArtifacts":false}}')   # 手元でアプリを作る。自動更新の署名の鍵がなくても作れる
```

## 決まり

- 常に動くアプリなので、軽さを仕様の条件にする。定期的な処理・外部コマンドの起動・画面の描き直しを足すときは、負荷を見積もる（ADR 0003）
- hook は何も出力せず、常に 0 で終わる。プロセスは自分の親だけをたどる。`SessionEnd` と `Interrupt` の待ち時間は 3 秒以下（ADR 0004）
- Claude Code と Codex の設定ファイルは直接書き換えず、それぞれの CLI を通す。アプリの操作で会話を勝手に作らない（ADR 0005）
- 文字に状態色を使わない。色はバッジなど文字以外で使う（ADR 0006）
- 画面のデザインの元は Figma にある（メンテナーだけが見られる）。画面を変えるときは先に Figma を見て、描かれていない振る舞いは推測で足さずに確かめる。メンテナー以外が画面を変えたいときは、先に Issue で相談してもらう（ADR 0006）
- アプリのアイコンは正方形のまま。角丸は macOS が付ける（ADR 0006）
- 版の番号は 3 か所で上げる（ADR 0007、[docs/releasing.md](docs/releasing.md)）
- コメントは理由（WHY）だけを日本語で書く。コミットのメッセージと Pull Request も日本語
- 変更はブランチで行い、Pull Request で `main` に入れる。CI が通ってから `gh pr merge --rebase --delete-branch` でマージする（履歴を一直線に保ち、コミットも分けたまま残す）。実機の確認の記録など、文書だけの小さな変更は `main` に直接 push してよい
- 新しいライブラリは、入れる前にメンテナーに確認する
- 変更の後は、整形・clippy・テスト・型チェックを通してから完了とする（`./scripts/check.sh`、ADR 0002）。Claude Code では、`gh pr create` の前に hook がこれを動かし、通らなければ止める
- 同じ原因の失敗を 2〜3 回直そうとして解消しないときは、繰り返さずに、何が・なぜ失敗しているかを報告して判断を仰ぐ
- 振る舞いや構成を決める前に、類似のアプリや一般的なやり方を調べる。自分たちの考えだけで決めない（ADR 0008）
- 外のツール（Claude Code・Codex・Tauri など）の振る舞いは、公式の文書と実機で確かめる。確かめた版と日付を書く（ADR 0008）
- 画面やプロセスに関わる変更は、テストに加えて実機での確認を完了の条件にする。確かめる場面は [docs/device-checks.md](docs/device-checks.md)（ADR 0008）

## 文書の置き場所

- 横断的な決定: [docs/adr/](docs/adr/)
- 機能ごとの設計と計画: [docs/superpowers/specs/](docs/superpowers/specs/)・[docs/superpowers/plans/](docs/superpowers/plans/)
- 公開の手順: [docs/releasing.md](docs/releasing.md)。Claude Code では `/release <版>` がこの「公開のたびに」を行う
- Pull Request の本文: Claude Code の `/pr-description` が、ひな形に沿って作る
- 実機で確かめる場面: [docs/device-checks.md](docs/device-checks.md)。Claude Code の `/device-check` が案内と記録をする
- Dependabot の Pull Request: Claude Code の `/deps` が、変更点・セキュリティの修正・Tauri の版のそろい方を見てまとめる
