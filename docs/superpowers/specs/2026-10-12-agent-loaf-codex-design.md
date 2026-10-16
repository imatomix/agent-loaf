# Agent Loaf の Codex 対応 設計書

- 作成日: 2026-10-12
- 状態: 実装済み（feat/codex）
- 元の設計書: `2026-10-09-agents-monitor-design.md`、`2026-10-12-agent-loaf-v0.5-distribution-design.md`
- 位置づけ: v0.5.0（最初の公開）に含める。この対応を作ってから、v0.5 の計画の Task 8（署名・公証・公開）を行う

## 1. 目的

Claude Code に加えて、OpenAI の Codex のセッションも、同じ一覧・同じ通知・同じメニューバーのアイコンで見られるようにする。

### 成功の基準

- VS Code の Codex の会話が一覧に出て、実行中・要対応（許可待ち）・完了・完了（中断）が正しく変わる
- 許可を求められたときに通知が届く
- 行を押すと VS Code が前に出て、そのフォルダーを開く
- 起動したときに `~/.codex` があれば、自動でプラグインが入る。hook の許可が要るときは、設定の画面で気づけて、「ターミナルで許可する」から許可できる
- Claude Code の監視の動きは変わらない

### 対象外

- Codex の状態不明の行（3.5 節）
- `CODEX_HOME` で分けた、`~/.codex` 以外の Codex の設定ディレクトリを自動で見つけること（「＋ 設定ディレクトリを追加」で足すことはできる）
- Codex の会話のタブやウィンドウを選んで移動すること
- hook の中身が変わった後の「もう一度許可が必要」の見つけ方（4.4 節）

## 2. 調べて分かったこと（2026-10-12）

試した版: VS Code の拡張 `openai.chatgpt` 26.1007.21434、同梱の Codex 0.162.0-alpha.17.2。「試して見たこと」は、この版での観察で、保証された仕様ではない。

### 2.1 公式の文書に書かれていること（`learn.chatgpt.com/docs/hooks`）

- hook の種類: `SessionStart`（`source`: `startup`・`resume`・`clear`・`compact`）、`SessionEnd`（`reason` は今は `other` だけ）、`UserPromptSubmit`、`PreToolUse`、`PermissionRequest`、`PostToolUse`、`Stop`、`SubagentStart`、`SubagentStop`、`PreCompact`、`PostCompact`、`Interrupt`。`Notification` はない
- 共通の項目: `session_id`、`transcript_path`（null のことがある。形は安定した仕様ではない）、`cwd`、`hook_event_name`、`model`
- `PermissionRequest` の `tool_input.description` は null のことがある
- 置き場所: `~/.codex/hooks.json`、`~/.codex/config.toml`、リポジトリの `.codex/`（そのフォルダーが信頼されているときだけ）、プラグインの `hooks/hooks.json`
- **許可**: 管理されていない hook は、利用者が確かめて許可するまで動かない。許可は hook の定義の指紋で記録され、定義が変わると再び許可が要る。プラグインを入れても、その hook は自動では許可されない。許可は CLI の `/hooks` で行う
- 待ち時間: 秒で指定。既定は 600。`SessionEnd` と `Interrupt` は既定 1、最大 3
- `SessionEnd` が届くとき: 開いている会話をアーカイブ・削除したとき、Codex が正常に終わったとき、**どの画面でも開かれておらず 30 分動きがないとき**。会話を切り替えただけでは、すぐには届かない。サブエージェントには届かない
- hook のコマンドの環境変数: `PLUGIN_ROOT`、`PLUGIN_DATA`、互換のための `CLAUDE_PLUGIN_ROOT`、`CLAUDE_PLUGIN_DATA`

### 2.2 試して見たこと

- 今の Claude Code の形のマーケットプレイス（`.claude-plugin/marketplace.json`）とプラグインを、`codex plugin marketplace add <フォルダー>` と `codex plugin add agent-loaf@agent-loaf` で、そのまま入れられた。状態は `codex plugin list --json`、`codex plugin marketplace list --json` で読める
- VS Code の拡張の Codex の画面には、hook を許可する画面がなかった（`/hooks` は「ターミナルで codex を起動して」と答えた）
- 拡張に同梱の本体は、そのままではターミナルで起動できず、`--no-daemon` を付けると起動した。起動したときに「Hooks need review」の画面が自分から出て、「Review hooks」「Trust all and continue」「Continue without trusting」を選べた
- 許可は `~/.codex/config.toml` の `[hooks.state."<plugin>@<marketplace>:hooks/hooks.json:<イベント>:0:0"]` に `trusted_hash = "sha256:…"` として記録された。VS Code の Codex も、読み込み直した後にその許可で hook を動かした
- VS Code の Codex は、ウィンドウごとに `codex … app-server` を 1 つ動かし、その中で複数の会話を扱う。新しい会話を始めても前の会話の `SessionEnd` は届かず、前の会話で頼んだシェルのコマンドは、新しい会話を始めた後も動き続け、`PostToolUse`・`Stop` が届いた
- hook の親は `codex`（拡張の中のファイル）→ `Code Helper (Plugin)` → `Visual Studio Code.app`。環境変数に `CODEX_INTERNAL_ORIGINATOR_OVERRIDE=codex_vscode`（内部の情報なので、見分けの補助にだけ使う）。`CODEX_HOME` は、明示しないと付かない
- 届いた中身: `SessionStart`・`UserPromptSubmit`・`PreToolUse`・`PermissionRequest`（理由の文付き）・`PostToolUse`・`Stop`・`Interrupt`（止めたとき。その後に `Stop` は来ない）・`SessionEnd`（ターミナルの Codex を `/quit` で終えたとき）。ツールの名前は `Bash`。`echo hello` は許可を求められず、`curl` は求められた
- ターミナルの `codex` は、`--no-daemon` を付けないと裏の共通のサーバーを使う（表示による）
- 2 つの会話は、別々の `session_id` で届いた

## 3. hook と状態の扱い

### 3.1 入口

- Claude Code と Codex に、同じプラグイン（同じ `hooks/hooks.json`、同じ `agent-loaf-hook`）を入れる
- hook のプログラムは、親をたどって最初に見つかった `claude` か `codex` のプロセスで、呼ばれたエージェントを決める。イベントに `agent`（`claude_code` / `codex`）を足す（前の版のイベントや `state.json` に項目がなければ `claude_code` として読む）
- 設定のディレクトリ: Claude Code は今のまま（`CLAUDE_CONFIG_DIR`、なければ `~/.claude`）。Codex は `CODEX_HOME`、なければ `~/.codex`
- `codex` かどうかの見分け: 実行ファイルの名前が `codex`。`codex-code-mode-host` などのほかの名前は含めない

### 3.2 会話以外を除く

- `codex exec`（と、その別名 `codex e`）、`codex review` から呼ばれた hook は書かない（`claude -p` と同じ扱い）
- VS Code の拡張の `codex … app-server` と、ターミナルの対話の `codex` は会話として扱う

### 3.3 知らせから状態へ

今の状態の決め方（`state.rs`）をそのまま使い、Codex にしかないものを足す。

| 知らせ | 状態 |
|---|---|
| `Interrupt` | 完了（中断） |
| `PostCompact` | 変えない |
| `PermissionRequest` | 要対応（許可待ち）。詳細は `tool_input.description` があればそれ、なければ今と同じくツールとコマンド |

- Codex の会話では、会話の記録の末尾から中断を推測しない（`Interrupt` があり、記録の形は安定した仕様ではないため）
- `hooks.json` に `Interrupt` と `PostCompact` を足す。`SessionEnd` と `Interrupt` の待ち時間を 3 秒にする（Codex の上限）。Claude Code が知らない名前の hook を読み飛ばすことは、実装の最初に確かめる

### 3.4 会話の寿命

- Codex の会話は `session_id` で区別する。pid は、Codex のプロセスが生きているかの確認にだけ使う
- 3 秒ごとの「生きているか」の確認で、`claude` に加えて `codex` のプロセスも生きているものとして数える（今は `claude` だけを数えているので、そのままだと Codex の会話がすぐ終了になる）
- 同じ pid で別の会話が始まったときに前の会話を終了にする処理（`retire_sessions_sharing_pid`）は、Codex の会話には行わない
- Codex の会話が一覧から消えるのは、`SessionEnd` が届いたとき（30 分動きがないときを含む）と、Codex のプロセスがなくなったとき
- ~~そのため、VS Code で別の会話に移った前の会話は、最大 30 分「完了」のまま一覧に残る~~ → 実機では 30 分たっても消えなかった。9 節と 3.4.1 節を参照

#### 3.4.1 見たあとの行を隠す（実機の確認の後に追加）

- VS Code の Codex 拡張は、新しいチャットを始めても前のチャットをつかんだままで、ウィンドウを閉じるまで「どの画面でも開かれていない」にならない。そのため 30 分の `SessionEnd` は届かない。VS Code の Claude Code も、新しいチャットごとに `claude` のプロセスを起動し、前のプロセスは残り続ける
- 類似のアプリ（claude-status-bar、OpenAgentPeek、cc-status-bar、claude-watch、ccmonitor、c9s）は、30 分〜24 時間何も起きなければ状態を問わず消すか、手で消すボタンを付けていた。見逃した通知の行が勝手に消えないよう、Agent Loaf では「見た」ことを条件にする
- 「完了」（「中断」を含む。裏でサブエージェントが動いている完了は除く）の行は、一覧を開いている間に印を付け、見た時刻と最後の動きの遅い方から 1 時間たったら隠す。状態が変わったら印を消し、隠すのもやめる
- `SessionStart` が届いたら、同じ親のプロセス（VS Code では同じウィンドウの拡張のプロセス）の「新規」の行を隠す。hook は会話のプロセスの親の pid を `terminal.host_pid` で送る
- 隠した行は内部では残す。プロセスが生きている間に状態不明の行を作らないためと、また使われたら同じ行として戻すため。Claude Code と Codex で同じに扱う

### 3.5 状態不明の行

- Codex では出さない。プロセス 1 つに会話がいくつあるか分からないため
- hook が届かないいちばん多い理由（許可していない）は、4.4 節の知らせで気づけるようにする
- 3 秒ごとのプロセスの確認で、`codex` のプロセスを状態不明の行にしない

### 3.6 移動

- 今の方法（親をたどって `.app` を見つけ、`open -a <アプリ> <始めたフォルダー>`）を、`codex` の親でも使う。VS Code の Codex なら、VS Code を前に出し、そのフォルダーを開く（ウィンドウや会話を選ぶ保証はない）
- 移動の前の「pid がまだ `claude` か」の確認を、「まだ `claude` か `codex` か」にする
- エディターかどうか（フォルダーを開くか）は、Claude Code では `CLAUDE_CODE_ENTRYPOINT`、Codex では親に `.app` の中の拡張ホストがあることで決める
- ターミナルの Codex が裏のサーバーで動いているときは移動先が見つからず、詳細を開くだけになる見込み。実装の最初に確かめ、そうなら README の既知の制約に書く

## 4. 導入と設定の画面

### 4.1 設定のディレクトリ

- 「Claude Code の設定」の欄を「エージェントの設定」にし、Claude Code と Codex の設定ディレクトリを同じ一覧に並べる（Figma の `156:1613`）
- 種類は、そのフォルダーに `config.toml` があれば Codex、なければ Claude Code。保存する設定の形（`config_dirs`）は変えない
- 起動したときの自動の追加
  - `~/.claude`: 今のまま（設定のディレクトリをまだ決めていないときだけ）
  - `~/.codex`: あれば、まだ一度も自動で足していなければ足して、プラグインを入れる。足したことを設定（`codex_dir_offered: bool`、既定は false）に覚え、利用者が外した後はもう足さない。前の版の設定にこの項目がないときは false として読むので、この Mac でも次の起動で 1 回だけ足される

### 4.2 Codex のプラグインを入れる・更新する・外す

| 操作 | コマンド（`CODEX_HOME` にその設定ディレクトリを渡す） |
|---|---|
| 状態を読む | `codex plugin list --json`、`codex plugin marketplace list --json` |
| 入れる | `codex plugin marketplace add <~/.agent-loaf/marketplace>`、`codex plugin add agent-loaf@agent-loaf` |
| 更新する | プラグインの版が上がったとき。手順（入れ直し、または `codex plugin marketplace upgrade`）は実装の最初に確かめる |
| 外す | `codex plugin remove agent-loaf@agent-loaf`、`codex plugin marketplace remove agent-loaf` |

- 同梱のマーケットプレイス以外から入れた `agent-loaf` には手を出さない（今の Claude Code と同じ）
- 時間の制限と失敗の扱いは、今の `claude` のコマンドと同じ（60 秒、失敗したら理由をアラートで出す）

### 4.3 Codex の本体を探す

- 探す場所の順: 設定に保存した場所（実行できなければ探し直す）→ `~/.local/bin/codex`、`/opt/homebrew/bin/codex`、`/usr/local/bin/codex`、`~/.npm-global/bin/codex` → VS Code・Cursor・Windsurf の拡張の中の `openai.chatgpt-*/bin/macos-aarch64/codex`（いちばん新しい版のフォルダー）
- 見つからなければ、`claude` のときと同じく、その行の下に「codex コマンドが見つかりません。場所を入力してください。」と入力欄を出す

### 4.4 hook の許可

- `<Codex の設定ディレクトリ>/config.toml` を**読むだけ**で確かめる。`[hooks.state]` の中に、`agent-loaf@agent-loaf:hooks/hooks.json:<イベント>:…` の項目が、同梱の `hooks.json` の hook の数だけそろっていれば「許可済み」とする（書き換えない）
- 指紋（`trusted_hash`）の食い違いは見ない。Codex の計算を真似ることになり壊れやすいため。hook の中身を変えた版を出すときは、README とリリースノートで「もう一度許可してください」と知らせる
- 許可されていなければ、その行の下に「Codex の hook の許可が必要です。ターミナルで Codex を開き、Agent Loaf の hook を許可してください。」と「ターミナルで許可する」を出し、≡ に赤い丸を付ける（Figma の `156:1613`）
- 「ターミナルで許可する」を押すと、専用の空のフォルダー（`~/.agent-loaf/codex-trust`）で Codex の本体を `--no-daemon` で起動する `.command` のファイルを作り、`open` で開く（Terminal.app が開く。AppleScript は使わないので、オートメーションの許可は要らない）。`CODEX_HOME` はその設定ディレクトリにする
  - 空のフォルダーで開くのは、リポジトリの hook（`.codex/hooks.json`）を一緒に許可しないため
  - 案内の文は「Trust all」ではなく「Agent Loaf の hook を許可」とする（ほかのプラグインの確かめていない hook を一緒に許可しないため）
  - 空のフォルダーを信頼するか聞かれたときの動き、「Review hooks」の画面で 1 つずつ許可する手順は、実装の最初に確かめ、必要なら案内の文を直す
- 確かめ直すのは、設定の画面を開いたとき（今の Claude Code と同じ）

## 5. 一覧の行

- 補助の文字の前に、16px のエージェントのアイコンを置く。文字との間は 4px。文字は今まで通り（Figma の `156:1265`、案 A）
- アイコン: Claude Code は `claude-code 1`（`157:2477`）、Codex は `codex 1`（`157:2480`）。Figma から SVG で書き出して同梱する
- 状態不明の行（Claude Code だけ）にも Claude Code のアイコンを付ける
- 2 つのアイコンは各社の製品のロゴに見えるので、公開の前に Anthropic と OpenAI のブランドの決まりに合っているかを確かめる。合わなければ独自のアイコンに替える

## 6. 文書

- README（日本語・英語）
  - 冒頭と「必要な環境」: Claude Code と Codex の両方に対応する
  - 「導入」: Codex の hook の許可の手順
  - 「既知の制約」: ~~Codex で別の会話に移った前の会話が最大 30 分「完了」のまま残る~~（3.4.1 節の動きに置き換えた）、Codex では状態不明の行を出さない、ターミナルの Codex は移動できないことがある（3.6 節の確認の結果による）、hook の中身を変えた版の後はもう一度許可が要る
  - 「消し方」: Codex のプラグインとマーケットプレイスの外し方
- v0.5 の設計書: 1 章の「対象外」から Codex を外し、この文書を参照する

## 7. テスト

- **core のユニットテスト**
  - エージェントの見分け（親が `claude` か `codex` か、`codex-code-mode-host` は含めない）
  - `codex exec`・`codex e`・`codex review` を除き、`app-server` と対話の `codex` を会話とすること
  - 前の版のイベント・`state.json`（`agent` がない）を Claude Code として読むこと
  - Codex の会話では、同じ pid の別の会話を終了にしないこと。Claude Code では今まで通り終了にすること
  - `Interrupt` で完了（中断）、`PostCompact` で変えない、`PermissionRequest` の理由の文
  - Codex の会話では会話の記録から中断を推測しないこと
  - 3 秒ごとの確認で、`codex` のプロセスを状態不明の行にしないこと
  - 設定ディレクトリの種類の決め方（`config.toml` の有無）
  - `[hooks.state]` の読み取りで許可を判断すること（そろっている・足りない・ない・壊れた TOML）
  - `~/.codex` を 1 回だけ自動で足すこと（外した後は足さない、前の版の設定）
  - `codex plugin list --json`・`codex plugin marketplace list --json` の読み取りと、入れる・更新する・外す手順の決め方
  - Codex の本体の探し方（保存した場所、よくある場所、拡張の中のいちばん新しい版）
  - 移動先の決め方（`codex` の親の VS Code）
- **hook のテスト**: `codex` の親から呼ばれたら `agent` が `codex`、設定のディレクトリが `CODEX_HOME` か `~/.codex` になること
- **実機での確認（ユーザーと一緒に）**
  - この Mac で `~/.codex` が自動で入り、許可の案内と ≡ の赤い丸が出ること
  - 「ターミナルで許可する」で許可して、VS Code の Codex の会話が一覧に出ること（アイコン付き）
  - 許可を求められたときに要対応になり、通知が届くこと
  - 止めたときに完了（中断）になること
  - 行を押すと VS Code が前に出ること
  - 別の会話に移って 30 分たつと、前の会話が消えること
  - Claude Code の会話の監視が変わらないこと

## 8. 実装の最初に確かめること

- Claude Code が、`hooks.json` の知らない名前の hook（`Interrupt`・`PostCompact`）を読み飛ばすこと
- Codex のプラグインの版が上がったときの更新の手順
- 専用の空のフォルダーで `codex --no-daemon` を開いたときの動き（フォルダーの信頼の質問、「Review hooks」の画面）
- ターミナルの `codex` の hook の親（裏のサーバーか、ターミナルか）

結果に応じて 3.3・3.6・4.2・4.4 節を直す。

## 9. 実機で確かめた結果（2026-10-12）

- `~/.codex` が起動したときに自動で足され、プラグインが入り、許可の案内と ≡ の赤い丸が出た。「ターミナルで許可する」で Terminal.app に Codex が開き、許可した後に案内と赤い丸が消えた。空のフォルダーを信頼する設定は増えなかった
- VS Code の Codex の会話が、アイコン付きで一覧に出た。要対応と通知、完了（中断）、VS Code への移動、同じウィンドウの 2 つ目の会話でも 1 つ目が残ることを確かめた
- ターミナルの Codex（拡張に同梱の本体を `--no-daemon` で起動）で、行を押すと Terminal.app が前に出た。`--no-daemon` を付けない起動は、同梱の本体では起動しなかったので確かめていない
- 確かめる中で見つけて直したもの
  - 一覧を開いている間の確認で、`codex` のプロセスの起動のコマンドを読んでおらず、Codex の会話がすぐ終了になっていた
  - 親をたどるときに root の `login` で止まり、ターミナルの Claude Code・Codex へ移動できていなかった（v0.4 の前の性能の改善で入っていた不具合）
  - ターミナルで拡張に同梱の `codex` を動かすと、エディターと取り違えてフォルダーを開こうとしていた。親に「… Helper (Plugin)」がいるかで決めるようにした
- 別の会話に移って 30 分以上たっても、前の会話は消えなかった。Codex の記録では、閉じられていたのはタイトル作りなどの内部の短いスレッドだけで、一覧の会話は一度も閉じられていなかった。VS Code の Claude Code でも、新しいチャットごとのプロセスが残り続けていた。3.4.1 節の動きを足した
- 3.4.1 節の動きを実機で確かめた: VS Code の Claude Code で新しいチャットを続けて始めると、何も送っていない「新規」の行が 1 つだけになった。一覧で見た「完了」の行は 1 時間後に消え、消えた会話に送ると一覧に戻った
