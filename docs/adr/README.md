# ADR（設計の判断の記録）

特定の機能に閉じない、横断的な決定を残す。構成、道具、決まり、配布の方針などが対象。

機能ごとの「何のためにどう作るか」は、設計書（[docs/superpowers/specs/](../superpowers/specs/)）と計画（[docs/superpowers/plans/](../superpowers/plans/)）に置く。

- ADR: なぜこの構成・道具・決まりにしたか
- 設計書と計画: この機能を何のためにどう作るか

## 書き方

1. [0000-template.md](./0000-template.md) を写し、続きの番号を付ける（例: `0009-xxx.md`）
2. ステータス・日付・コンテキスト・決定・検討した代替案・影響を埋める
3. この一覧に足して、コミットする

## 後から変えるとき

本文は書き換えない。新しい ADR で置き換え、古い方のステータスを「置き換え済み（→ 新しい ADR へのリンク）」にする。あのとき何を決めたかを追えるようにするため。

## 一覧

- [0001. リポジトリの構成](./0001-repository-layout.md)
- [0002. 整形・clippy・CI](./0002-format-lint-ci.md)
- [0003. 軽さを仕様の条件にする](./0003-lightweight.md)
- [0004. hook の決まり](./0004-hook-rules.md)
- [0005. Claude Code と Codex への関わり方](./0005-external-tools.md)
- [0006. 画面の決まり](./0006-ui-rules.md)
- [0007. 版と配布](./0007-versioning-and-distribution.md)
- [0008. AI で開発する進め方](./0008-ai-assisted-development.md)
- [0009. 一覧の画面は閉じても捨てずに隠す](./0009-keep-list-window.md)
