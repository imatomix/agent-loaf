# 0007. 版と配布

- ステータス: 承認
- 日付: 2026-10-12

## コンテキスト

最初の公開の前に、版の付け方と配布の方法を決めた。公開の時点の質を上げたいが、実際に使われるまで見つからない不具合もある。

## 決定

- 公開も 0.x で始める。最初の公開は v0.5.0
- Codex への対応と、実際に使われて出た不具合の修正の後に 1.0.0 にする
- Developer ID で署名し、公証した dmg を GitHub の Releases に出す
- 自動更新は `tauri-plugin-updater` で、起動の約 1 分後に 1 回、その後は 24 時間ごとに確かめる
- 対象は Apple Silicon の macOS 14 以降だけ
- Homebrew には、今は載せない
- 版の番号は 3 か所（ルートの `Cargo.toml` の `[workspace.package]`、`app/package.json`、`plugin/.claude-plugin/plugin.json`）。`tauri.conf.json` には書かず、Tauri が `Cargo.toml` の版を使う

元の設計書: [v0.5 配布](../superpowers/specs/2026-10-12-agent-loaf-v0.5-distribution-design.md)。手順は [docs/releasing.md](../releasing.md)。

## 検討した代替案

- **署名しない、または ad-hoc の署名**: macOS 15 以降でダウンロードしたアプリを開く手間が大きく、通知の許可が更新のたびに消えるおそれがある
- **Intel 用も作る**: 作る手間と確かめる手間に見合う利用者が見込めない
- **Homebrew に載せる**: 公開の直後は、自動更新のある dmg だけで足りる

## 影響

版を上げるときは 3 か所を直す。`every_version_matches_the_app` のテストが、合っているかを確かめる。
