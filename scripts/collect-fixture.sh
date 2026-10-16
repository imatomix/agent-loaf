#!/usr/bin/env bash
# 使い方: scripts/collect-fixture.sh <期待する最終状態>__<シナリオ名>
set -euo pipefail
cd "$(dirname "$0")/.."
name="$1"
src="$HOME/.agent-loaf/events"
dest="crates/core/tests/fixtures/$name.jsonl"
mkdir -p "$(dirname "$dest")"
: > "$dest"
for f in $(ls "$src"/*.json | sort); do
  # 個人のパスをリポジトリに残さないため、ホームディレクトリを置き換える
  sed -e "s#$HOME#/Users/test#g" -e "s/$(basename "$HOME")/test/g" "$f" >> "$dest"
  echo >> "$dest"
  rm "$f"
done
echo "$(wc -l < "$dest") events -> $dest"
