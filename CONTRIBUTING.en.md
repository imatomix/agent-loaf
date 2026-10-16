# Contributing to Agent Loaf

[日本語](CONTRIBUTING.md)

## Requirements

- macOS 14 or later (Apple Silicon)
- Rust (stable)
- Node.js 22

## Getting started

```bash
(cd app && npm ci)
./scripts/build-hook-sidecar.sh   # builds the hook bundled with the app
(cd app && npm run tauri dev)
```

## Before opening a pull request

Make sure the same four checks as CI pass. `./scripts/check.sh` runs them all in the same order:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
(cd app && npx tsc --noEmit)
```

## Rules

- The rules are in [AGENTS.md](AGENTS.md) (in Japanese), and the reasons behind them are in [docs/adr/](docs/adr/)
- If you want to change how the UI looks or behaves, please open an issue first (the design source is kept by the maintainer)
- Commit messages and pull requests are written in Japanese, but English is fine too
