#!/usr/bin/env bash
# Pull Request の前に通すこと。CI（.github/workflows/ci.yml）と同じ順で動かす
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
./scripts/build-hook-sidecar.sh
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
(cd app && npx tsc --noEmit)
