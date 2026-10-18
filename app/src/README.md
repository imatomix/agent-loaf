# app/src

Agent Loaf の画面（TypeScript と Vite）。デザインの元は Figma にある（ADR 0006）。

| 場所 | 置くもの |
|---|---|
| `features/list/` | 一覧の画面（入り口の list.ts、ドラッグ、時間の表し方） |
| `features/settings/` | 設定の画面（設定の画面だけで使うアイコンも） |
| `lib/` | 2 つ以上の画面から使うもの（文言、型） |
| `assets/` | 画像 |
| `style.css` | 見た目 |

## 決まり

- 画面の機能は画面の名前で分ける。2 つ目の利用先が出てから `lib/` に上げる（ADR 0010）
- 文言は `t("キー")` で引く。キーは `crates/core/locales/` にあり、`app/src-tauri/tests/messages_used.rs` が使っているキーがあるかを確かめる
