# crates/core/src

Agent Loaf の判断のロジック（Rust の crate `agent-loaf-core`）。Tauri や OS に頼らない純粋な関数を中心にする（ADR 0001）。

| フォルダー | 置くもの |
|---|---|
| `session/` | 会話の状態を決める。hook のイベントを読み（spool・event・hook_input）、状態を決め（state・store）、一覧に出す形と並び（view・order）、通知（notify）、中断（transcript）を決める |
| `setup/` | 設定ディレクトリと、プラグインや hook の導入。CLI の呼び出し（claude_cli）、導入の手順（install_plan）、状態（plugin_status）、設定の画面のバッジ（attention） |
| `process/` | プロセスを読む（ps・procinfo）・画面を移動する（jump）・git のブランチ（git） |
| `update/` | 自動更新の状態の移り変わり |
| `base/` | 2 つ以上の機能から使う土台。agent・paths・persist・settings・messages・crash_log |

## 決まり

- 新しいコードは、まず機能のフォルダーに置く。2 つ目の利用先が出てから `base/` に上げる（ADR 0010）
- 文言の対応表は `crates/core/locales/` にある
