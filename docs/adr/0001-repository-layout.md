# 0001. リポジトリの構成

- ステータス: 一部置き換え済み（`crates/core/src/` を平らに並べる決まり → [0010](./0010-directory-layout.md)）
- 日付: 2026-10-12

## コンテキスト

構成を決めずに進めてきた。公開の前に、Tauri の公式の説明と、よく知られた Tauri 製のアプリ（Cap、Yaak、Clash Verge Rev、Spacedrive、Pot）と比べた（[設計書](../superpowers/specs/2026-10-12-agent-loaf-repo-conventions-design.md)の 2 節）。

- 小さいアプリは、ルートに画面の `src/` と `src-tauri/` を置く（公式の既定）
- 共通の部品を持つアプリは、`apps/<アプリ>/` と `crates/<部品>/` に分ける
- Rust のファイルは平らに並べるのがふつうで、分けるときはフォルダーより crate で分ける

## 決定

| 場所 | 役割 |
|---|---|
| `app/src/` | 画面（TypeScript と Vite） |
| `app/src-tauri/` | Tauri のアプリ。OS とのつなぎ（メニューバー、通知、プロセスの起動、画面との受け渡し） |
| `crates/core/` | 判断と状態。純粋な関数を中心にし、テストしやすくする |
| `crates/hook/` | Claude Code と Codex から呼ばれる hook の本体 |
| `plugin/` | Claude Code と Codex に入れるプラグイン（`hooks.json` など） |
| `.claude-plugin/` | GitHub からプラグインを入れるときのマーケットプレイス。ルートにないと読まれないので動かせない |
| `scripts/` | ビルドと開発の補助 |
| `docs/` | ADR、設計書と計画、公開の手順 |

`crates/core/src/` は平らに並べる。ほかでも使う部分が出たら、そのとき crate として切り出す。

アプリ名は 2026-10-12 に agents-monitor から Agent Loaf に変えた。それより前の設計書・計画・コミットは旧名のまま残す（その時点の記録なので書き換えない）。

## 検討した代替案

- **ルートに `src/` と `src-tauri/`**: 公式の既定だが、共通の部品の crate（`core`・`hook`）を持つ形に合わない
- **`apps/desktop/`**: Cap などと同じ名前になるが、アプリは 1 つなので、名前を変える得が移動の手間に見合わない
- **`crates/core` をフォルダーに分ける**: 類似のアプリでも平らに並べるのがふつうで、今の 29 ファイルでは困っていない

## 影響

新しいファイルは役割に合う場所に置く。迷ったら、判断は `crates/core`、Tauri や OS とのつなぎは `app/src-tauri`。
