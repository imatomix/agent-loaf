# app/src-tauri/src

Agent Loaf の、Tauri のアプリ（OS とのつなぎ）。判断は `crates/core` に置き、ここでは Tauri・macOS とのやり取りだけをする（ADR 0001）。

| 場所 | 置くもの |
|---|---|
| `main.rs` | `lib.rs` の `run()` を呼ぶだけ（Tauri 2 の標準） |
| `lib.rs` | アプリの組み立て。プラグイン・命令の登録・起動のときの処理 |
| `commands/` | 画面から呼ぶ命令（`#[tauri::command]`）。機能ごとのファイルで分ける。どの画面から呼ぶかに合わせる（session・setup・settings・update・app） |
| `session/` | 監視（monitor）と、macOS の通知（notifier） |
| `setup/` | 設定ディレクトリとプラグインの導入（installer） |
| `process/` | 会話の画面を前に出す（jumper） |
| `update/` | 自動更新（updater） |
| `tray/` | メニューバーのアイコンと一覧の窓 |
| `base/` | 2 つ以上の機能から使うものと、どの機能にも属さない土台。言語の判定（lang）、ロックの扱い（sync） |

## 決まり

- 命令を足したら、`lib.rs` の `generate_handler!` にも足す
- 機能のフォルダーの名前は `crates/core/src` とそろえる（ADR 0010）
