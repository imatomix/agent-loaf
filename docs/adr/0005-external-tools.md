# 0005. Claude Code と Codex への関わり方

- ステータス: 承認
- 日付: 2026-10-11（Codex は 2026-10-12）

## コンテキスト

Agent Loaf は、利用者の Claude Code と Codex にプラグインを入れ、会話の画面へ移動する。利用者の設定や会話を壊すと、信頼を失う。

## 決定

- 設定ファイル（`~/.claude/settings.json`、`~/.codex/config.toml` など）は直接書き換えない。プラグインの導入・更新・削除は `claude plugin`・`codex plugin` の CLI を通す
- 読むだけなら読んでよい（Codex の hook の許可の状態など）
- アプリの操作で会話（セッション）を勝手に作らない。VS Code の会話の URI は、別のウィンドウに空の会話を作ることがあるので使わない。移動はウィンドウを前に出すだけ
- Codex の hook の許可は、利用者がターミナルの Codex で行う。Agent Loaf は許可の画面をターミナルで開くだけ

元の設計書: [v0.2](../superpowers/specs/2026-10-11-agents-monitor-v0.2-design.md)、[v0.3](../superpowers/specs/2026-10-11-agents-monitor-v0.3-design.md)、[Codex 対応](../superpowers/specs/2026-10-12-agent-loaf-codex-design.md)

## 検討した代替案

- **設定ファイルに hook を直接書く**: 利用者の設定を壊すおそれがあり、Claude Code や Codex の形が変わると壊れる
- **会話の URI で会話まで開く**: 空の会話が作られることがある。利用者は、勝手に会話が作られることを最も嫌う

## 影響

外のツールに何かするときは、まず公式の CLI があるかを調べる。ないときは、読むだけにする。
