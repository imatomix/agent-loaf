# Agent Loaf に協力する

[English](CONTRIBUTING.en.md)

## 必要なもの

- macOS 14 以降（Apple Silicon）
- Rust（stable）
- Node.js 22

## 始め方

```bash
(cd app && npm ci)
./scripts/build-hook-sidecar.sh   # アプリに同梱する hook を作る
(cd app && npm run tauri dev)
```

## Pull Request の前に

CI と同じ次の 4 つを通してください。`./scripts/check.sh` で、まとめて同じ順に動かせます。

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
(cd app && npx tsc --noEmit)
```

## 決まり

- 決まりは [AGENTS.md](AGENTS.md) に、その理由は [docs/adr/](docs/adr/) にあります
- 画面の見た目や振る舞いを変えたいときは、先に Issue で相談してください（デザインの元はメンテナーの手元にあります）
- コミットのメッセージと Pull Request は日本語で書いています。英語でもかまいません
