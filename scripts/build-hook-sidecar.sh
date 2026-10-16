#!/usr/bin/env bash
# Tauri の externalBin は「名前-ターゲットトリプル」のファイルを要求する
set -euo pipefail
cd "$(dirname "$0")/.."
triple="$(rustc -vV | sed -n 's/^host: //p')"
cargo build --release -p agent-loaf-hook
# CARGO_TARGET_DIR や .cargo/config.toml で出力先を変えている人もいるので、cargo に聞く
target_dir="$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
mkdir -p app/src-tauri/binaries
cp "$target_dir/release/agent-loaf-hook" "app/src-tauri/binaries/agent-loaf-hook-$triple"
echo "sidecar: app/src-tauri/binaries/agent-loaf-hook-$triple"
