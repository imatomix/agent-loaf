# 0002. 整形・clippy・CI

- ステータス: 承認
- 日付: 2026-10-12

## コンテキスト

公開すると、ほかの人の変更も受け取る。調べた Tauri 製のアプリはどれも、`rustfmt.toml` を置き、整形・clippy・テストを Pull Request ごとに確かめていた。Agent Loaf には整形の設定も、変更を確かめる CI もなかった。

## 決定

- ルートの `rustfmt.toml` は `edition = "2021"` と `max_width = 120` だけ。ほかは既定のまま
- clippy は警告を失敗として扱う（`-D warnings`）
- `.github/workflows/ci.yml` で、`main` への push とすべての Pull Request のたびに、整形・clippy・テスト・型チェックを macOS（Apple Silicon）で動かす。プロセスを読む部分が macOS 専用のため
- 画面の側（TypeScript）の整形の道具は入れない

## 検討した代替案

- **行の幅 100**: Tauri 本体と同じだが、今のコードでは差分が大きい（100 で 388 か所、120 で 262 か所）
- **Prettier などを入れる**: 画面のコードは 9 ファイルと少なく、新しい道具を足すほどではない

## 影響

- 変更の後は `cargo fmt --all`、clippy、テスト、型チェックを通してから完了とする
- Claude Code では、`.rs` を書くと整形の hook が動く（`.claude/settings.json`）
- 公開用のワークフロー（`release.yml`）とは分けてある。CI は Secrets を使わない
