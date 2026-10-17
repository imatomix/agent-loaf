#!/bin/sh
# コードの変更は Pull Request で main に入れる（AGENTS.md）。main に直接 push するのは、文書だけの変更に限る。
# マージの前に CI を通すため。通らないときは理由を出して 2 で終わり、Claude Code にそのコマンドを止めさせる。

# 文字列の中に書いただけのコマンド名に反応しないよう、区切った各コマンドの先頭だけを見る
push=$(jq -r '.tool_input.command // empty' | tr ';&|' '\n\n\n' | sed 's/^[[:space:]]*//' | grep -E '^git push( |$)' | head -1)
[ -z "$push" ] && exit 0

cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0
branch=$(git symbolic-ref --quiet --short HEAD 2>/dev/null)
targets_main=no
case " $push " in
  *" main "*|*":main "*|*" HEAD:main "*|*"refs/heads/main"*) targets_main=yes ;;
esac
# 送る先を書かない push は、今のブランチを送る
positional=$(printf '%s' "$push" | awk '{n=0; for (i=3; i<=NF; i++) if ($i !~ /^-/) n++; print n}')
if [ "$targets_main" = no ] && [ "$branch" = main ] && [ "$positional" -lt 2 ]; then
  targets_main=yes
fi
[ "$targets_main" = yes ] || exit 0

code=$(git diff --name-only origin/main...main 2>/dev/null | grep -vE '^docs/|\.md$' | head -5)
[ -z "$code" ] && exit 0

echo "main に、文書ではない変更を直接 push しようとしています（例: $(echo $code)）。ブランチを push し、Pull Request で入れてください（AGENTS.md）。" >&2
exit 2
