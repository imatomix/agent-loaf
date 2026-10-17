---
description: 今のブランチの変更から、ひな形に沿った Pull Request の本文を作る
argument-hint: "[比べる先のブランチ。省くと main]"
---

比べる先（`$ARGUMENTS`。空なら `main`）から今のブランチまでの変更を読み、[.github/pull_request_template.md](../../.github/pull_request_template.md) の見出し（変更の内容・関連する文書・確かめたこと）に沿って、Pull Request の本文を Markdown で作る。

## 手順

1. `git log <比べる先>..HEAD` と `git diff <比べる先>...HEAD` で、変更をつかむ
2. 「変更の内容」には、何を・なぜ変えたかを書く。なぜは、コミットのメッセージ・設計書・計画・ADR から読み取り、推測で書かない。読み取れないときは、本文を作る前に聞く
3. 「関連する文書」には、もとにした `docs/superpowers/specs/`・`docs/superpowers/plans/`・`docs/adr/` のパスを書く。なければ見出しごと消す
4. 「確かめたこと」のチェックは、このセッションで実際に行ったものだけに入れる
   - `./scripts/check.sh` は、今のコミットで通したときだけ。まだなら動かしてから入れる
   - 実機の確認は、試した場面の番号を書く。試していなければ空けたままにし、そう伝える
5. 本文の最後に `🤖 Generated with [Claude Code](https://claude.com/claude-code)` を付ける

## 出すもの

本文の Markdown だけを出す。前置きは書かない。Pull Request は、作るように言われたときだけ `gh pr create --body-file` で作る（作る前に hook が `./scripts/check.sh` を動かす）。
