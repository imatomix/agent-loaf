#!/bin/sh
# Pull Request を作る前に、CI と同じ確認（scripts/check.sh）を通す。通らなければ理由を出して 2 で終わり、Claude Code に止めさせる。
# Claude Code の if の条件だけでは、引用符の込み入ったコマンドで関係なく動くことがあったので、ここでもコマンドの先頭を確かめる

segments=$(jq -r '.tool_input.command // empty' | tr ';&|' '\n\n\n' | sed 's/^[[:space:]]*//')
printf '%s\n' "$segments" | grep -qE '^gh pr create( |$)' || exit 0

log="${TMPDIR:-/tmp}/agent-loaf-check.log"
"$CLAUDE_PROJECT_DIR/scripts/check.sh" >"$log" 2>&1 && exit 0
echo "Pull Request の前の確認（整形・clippy・テスト・型チェック）が通りませんでした。直してから作ってください。全体は $log にあります。" >&2
tail -20 "$log" >&2
exit 2
