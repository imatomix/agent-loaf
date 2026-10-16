#!/usr/bin/env bash
# Figma の MenuBarIcon から書き出した SVG を、メニューバー用の 36×36 PNG にする（rsvg-convert が必要）
set -euo pipefail

dir="$(cd "$(dirname "$0")/.." && pwd)/app/src-tauri/icons/tray"
for name in default error play required; do
  # Figma の 0.85 はメニューバーでの見え方を再現するためのもの。テンプレート画像の濃さは OS が決めるので外す
  plain=$(mktemp)
  sed -e 's/ fill-opacity="[^"]*"//g' "$dir/$name.svg" > "$plain"
  rsvg-convert -w 36 -h 36 "$plain" -o "$dir/$name.png"
  rm "$plain"
done
echo "$dir に PNG を書き出しました"
