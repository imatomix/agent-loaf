# 0004. hook の決まり

- ステータス: 承認
- 日付: 2026-10-09（Codex の決まりは 2026-10-12）

## コンテキスト

hook は Claude Code と Codex の動作の途中で、ツールを使うたびに動く。出力や失敗、遅れは、会話の邪魔になる。Claude Code では、`Stop` の hook が 0 以外で終わると止まれなくなり、`UserPromptSubmit` が失敗すると入力が消える。

## 決定

- 何も出力せず、パニックも含めて常に 0 で終わる
- 全プロセスではなく、自分の親だけをたどる
- 会話でない実行（`claude -p`、Agent SDK、`codex exec`・`codex review`）では何もしない
- イベントは `~/.agent-loaf/events` に置くだけにし、判断はアプリが行う
- 置き場が 10,000 件を超えたら置かない（アプリが止まっている間にたまり続けないため）
- `SessionEnd` と `Interrupt` の待ち時間は 3 秒以下（Codex の上限）

元の設計書: [v0.1](../superpowers/specs/2026-10-09-agents-monitor-design.md)、[Codex 対応](../superpowers/specs/2026-10-12-agent-loaf-codex-design.md)

## 検討した代替案

- **hook がアプリに直接知らせる（ソケットなど）**: アプリが止まっていると hook が待たされる。ファイルに置くだけなら、アプリがなくても速く終わる

## 影響

hook を変えたら、出力がないこと、0 で終わること、待ち時間の上限をテストで確かめる（`crates/hook/tests/`）。
