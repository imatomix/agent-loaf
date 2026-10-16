# agents-monitor v0.2.1（日英対応）設計書

- 作成日: 2026-10-11
- 状態: 実装済み
- 元の設計書: `2026-10-09-agents-monitor-design.md`、`2026-10-11-agents-monitor-v0.2-design.md`
- 位置づけ: v0.3（画面移動）の実装の前に入れる。v0.3 の文言はこの仕組みの上で書く

## 1. 目的

画面・通知・アラートを、日本語と英語で表示できるようにする。公開の前に英語で使える状態にし、v0.3 以降で増える文言を最初から日英で書けるようにする。

### 成功の基準

- macOS の言語（または「アプリごとの言語」）が日本語なら日本語、それ以外なら英語で表示される
- 文言の訳し漏れ（片方の言語にしかないキー、使っているのに表にないキー、差し込む値の食い違い）をテストで見つけられる
- README が日本語と英語の両方である

### 対象外

- 日本語・英語以外の言語
- 設定画面での言語の切り替え（macOS の「アプリごとの言語」で変えてもらう）
- 言語を変えたときの即時反映（アプリの再起動で反映する）

## 2. 全体構成

| 置き場所 | 役割 |
|---|---|
| `crates/core/locales/ja.json`、`en.json` | キーと文言の対応表。キーは `status.needs_action` のような平らな名前、差し込む値は `{project}` |
| core `messages` | 言語（`Lang::Ja` / `Lang::En`）、対応表の読み込み（`include_str!`）、キーの列挙型、値の差し込み |
| core `state`・`store`・`notify`・`claude_cli`・`plugin_status` | 文言を組み立てず、種類（列挙型）と中身（ツール名・要約・エラーの種類など）を返す |
| アプリ | 起動時に言語を決め、通知とアラートの文面を作る。コマンド `get_messages` で画面に今の言語の表を渡す |
| 画面 | 起動時に `get_messages` で表を受け取ってから描く。`t("key", { ... })` で文言を引く |

画面から JSON を直接読まないのは、Vite の設定（アプリのフォルダーの外のファイルを読む許可）を増やさないため。

### 2.1 言語の決め方

- macOS では `NSBundle.mainBundle.preferredLocalizations` の先頭を見る。`ja` で始まれば日本語、それ以外は英語。これは macOS の優先言語と「アプリごとの言語」の両方に従う
- `Info.plist` に `CFBundleLocalizations`（`ja`、`en`）を書く
- バンドルとして起動していないとき（`tauri dev`）や macOS 以外では、環境変数 `LANG` が `ja` で始まれば日本語、それ以外は英語
- 言語はアプリの起動時に 1 回だけ決める

### 2.2 状態の詳細を、種類と中身に分ける

v0.2 までは `Session.detail` に「許可待ち · Bash: rm -rf dist」のような出来上がった日本語を入れていた。これを次の形にする。

| 項目 | 内容 |
|---|---|
| `detail_kind` | 詳細の種類（列挙型）: 新規・許可待ち・質問待ち・承認待ち・入力要求・中断・サブエージェント実行中・圧縮中・ツール実行中・エラー |
| `detail_text` | 種類に添える中身。ツール名と要約（`Bash: rm -rf dist`）、エラーの種類など。翻訳しない |

- 表示するときに、`detail_kind` の文言と `detail_text` を `·` でつなぐ（今と同じ見た目）
- `state.json` に v0.2 までの `detail` が残っていても読めるようにし（項目が足りないまま読める）、古い `detail` は捨てる。状態そのものは残る
- 状態の名前（要対応など）も `Status` の文言を core では持たず、キー（`status.needs_action` など）で引く

### 2.3 アラートとエラー

- アプリが出すアラート・エラー（「〜にプラグインを入れられませんでした」「時間切れ」「導入状態を読めませんでした」など）は、キーと差し込む値で表す
- `claude` 自身が出したエラーの文（エラー出力の最後の行）は、翻訳せずにそのまま差し込む

### 2.4 ログとその他

- ログ（`eprintln!`）は英語にそろえる。翻訳はしない
- プラグインの説明（`plugin/.claude-plugin/plugin.json`、`.claude-plugin/marketplace.json`）は英語にする。Claude Code のプラグイン一覧に出るもので、言語を切り替える仕組みがないため
- README は `README.md`（日本語）と `README.en.md`（英語）にし、冒頭で互いにリンクする

## 3. 英語の文言

画面に出る主な文言（実装時はこれを対応表の英語の値にする。Figma の英語版の下書きもこれで作る）。

| 日本語 | 英語 |
|---|---|
| 要対応 | Needs action |
| エラー停止 | Error |
| 完了 | Done |
| 実行中 | Running |
| 状態不明 | Unknown |
| 終了 | Ended |
| 新規 | New |
| 許可待ち | Waiting for permission |
| 質問待ち | Waiting for your answer |
| 承認待ち | Waiting for plan approval |
| 入力要求 | Input requested |
| 中断 | Interrupted |
| サブエージェント実行中 | Running a subagent |
| 圧縮中 | Compacting |
| バックグラウンド作業あり | Background work running |
| 監視対象外 | Not monitored |
| 3秒・3分・3時間・3日 | 3s・3m・3h・3d |
| 場所: | Location: |
| プロフィール: | Profile: |
| 動いている Claude Code セッションはありません | No Claude Code sessions are running |
| この会話を開いた後にプラグインが入ったか、使っている設定にプラグインが入っていません。プラグインを入れてから、会話を開き直すと監視されます。 | The plugin was installed after this conversation started, or isn't installed for its config. Install it, then reopen the conversation to start monitoring. |
| 通知設定 | Notifications |
| 通知がオフになっています | Notifications are turned off |
| システム設定を開く | Open System Settings |
| Claude Code の設定 | Claude Code configs |
| ＋ 設定ディレクトリを追加 | ＋ Add config directory |
| Claude Code の設定ディレクトリが見つかりませんでした。使用している設定ディレクトリを追加してください。 | No Claude Code config directory was found. Add the one you use. |
| claude コマンドが見つかりません。場所を入力してください。 | The claude command was not found. Enter its path. |
| もう一度 | Retry |
| 削除 | Remove |
| 詳細を開く／詳細を閉じる | Show details / Hide details |
| ログイン時に起動する | Launch at login |
| 終了する | Quit |
| {dir} にプラグインを入れられませんでした。 | Couldn't install the plugin in {dir}. |
| {dir} からプラグインを外せませんでした。 | Couldn't remove the plugin from {dir}. |
| 時間切れ（応答がありませんでした） | Timed out (no response) |
| 導入状態を読めませんでした | Couldn't read the plugin status |
| Claude Code の設定ディレクトリを選ぶ | Choose a Claude Code config directory |

通知の文面は「{project} — {状態の詳細}」（英語も同じ形）。

## 4. テスト

- 日本語と英語の表に、同じキーがそろっていること
- Rust で使うキーは列挙型にし、すべての値が両方の表にあること
- 画面の TypeScript で使うキー（`t("…")` の形）が、すべて表にあること（Rust のテストから `app/src/*.ts` と `app/index.html` を読んで確かめる）
- 差し込む値（`{project}` など）が、同じキーで両方の言語にそろっていること
- 状態の詳細の組み立て（種類 × 中身のあり・なし × 言語）
- v0.2 までの `state.json`（`detail` が文字列）を読んでも落ちないこと
- 言語の決め方（`preferredLocalizations` の先頭、`LANG`）
- 実機確認: macOS の「アプリごとの言語」で agents-monitor を English にし、一覧・設定・通知・アラートが英語になること。日本語に戻すと日本語になること

## 5. 実装の最初に確かめること

- `preferredLocalizations` が `Info.plist` の `CFBundleLocalizations` だけで働くか、`ja.lproj`・`en.lproj` フォルダーが要るか
- Tauri の設定で `Info.plist` に `CFBundleLocalizations` を足す方法（`app/src-tauri/Info.plist` を足すとマージされるか）

## 6. 画面

英語は日本語より長くなりやすく、幅 300px の中で切れることがある。Figma に英語版の下書きを作り、切れ方を確かめる（`v0.2.1 英語版 下書き`）。

## 7. 元の設計書の変更

- 14 章: v0.2.1 の行からこの文書を参照する
