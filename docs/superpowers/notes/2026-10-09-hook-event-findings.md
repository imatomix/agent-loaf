# hook イベントの調査結果（2026-10-09）

Claude Code v2.1.294 を tmux 上で対話モードで動かし、`~/.claude-personal` に入れた agents-monitor プラグインで記録した。記録したイベント列は `crates/core/tests/fixtures/` にある。

## 1. Esc で中断したとき

届いたイベント:

- 応答の生成中に Esc: `UserPromptSubmit` のみ（`Stop` なし）
- 許可ダイアログで Esc、または「No」を選択: `PreToolUse → PermissionRequest → Notification(permission_prompt)` のあと何も届かない
- プランの承認ダイアログで Esc: 何も届かない

会話ログ（`transcript_path`）の末尾:

```
user        "[Request interrupted by user]"            ← 応答中の Esc
user        "[Request interrupted by user for tool use]" ← 許可の拒否
system      turn_duration / last-prompt / ai-title / mode / permission-mode / atis-latch ...
```

中断の目印は**最後の行ではなく**、その後ろに管理用の行（`type` が `user` / `assistant` 以外）が続く。

結論: `Stop` は届かない。中断の検知は「会話ログを末尾から見て、最初に見つかった `type` が `user` か `assistant` の行に `[Request interrupted by user` が含まれるか」で判定する。実行中だけでなく**要対応の状態からも**中断が起きる（許可の拒否）ため、両方を検知の対象にする。

## 2. 許可待ちのイベント順

届いたイベント: `PreToolUse → PermissionRequest → Notification(permission_prompt)`。許可すると `PostToolUse → Stop`。

`AskUserQuestion`（質問）と `ExitPlanMode`（プラン承認）も、同じく `PreToolUse → PermissionRequest(同じツール名) → Notification(permission_prompt)` の順で届く。

結論: 計画の対応表のままでは、質問待ち・プラン承認待ちの表示が `PermissionRequest` と `Notification` で「許可待ち」に上書きされる。次のとおり直す。

- `PermissionRequest` のツールが `AskUserQuestion` なら「質問待ち」、`ExitPlanMode` なら「プラン承認待ち」
- すでに要対応のときに届く `Notification` は表示を変えない

## 3. バックグラウンド作業が残っているとき

- サブエージェント（Agent ツール）がバックグラウンドで動く場合: `PreToolUse(Agent) → SubagentStart → PostToolUse(Agent) → Stop` の順で、本体の `Stop` の時点でサブエージェントは動いている。終わると `SubagentStop → UserPromptSubmit → Stop`（結果を本体に渡すターン）
- Bash の `run_in_background` の場合: 開始の合図はなく、`Stop` だけ。コマンドが終わると `UserPromptSubmit → Stop`

結論: 「バックグラウンド作業あり」と表示できるのはサブエージェントの場合だけ。バックグラウンドの Bash は v0.1 では検知しない。

## 4. サブエージェント

- `SubagentStart` には `agent_id` が付く
- サブエージェント内の `PreToolUse` / `PermissionRequest` / `PostToolUseFailure` にも `agent_id` が付く
- サブエージェントの許可待ちの `Notification(permission_prompt)` には `agent_id` が付かない
- 入力欄の「次の指示の候補」を作る内部処理などで、`SubagentStart` のない `SubagentStop` が `Stop` のたびに届く

結論: 計画の前提どおり。開始のない `SubagentStop` で数が 0 未満にならないこと（`saturating_sub`）が必要。

## 5. VS Code 拡張のセッション

`ps` での見え方（実機）:

```
/Users/<user>/.vscode/extensions/anthropic.claude-code-2.1.289-darwin-arm64/resources/native-binary/claude --output-format stream-json --verbose --input-format stream-json ...
```

実行ファイル名が `claude` なので `is_claude_command` で判定でき、`is_interactive_session` も真になる（テスト `recognizes_real_world_processes`）。VS Code 拡張から起動した Claude の環境では `CLAUDE_CODE_ENTRYPOINT=claude-vscode` が入る（Task 5 の `claude -p` で確認）。

ほかに `claude daemon run`、`claude bg-pty-host`、`claude bg-spare`、`claude agents` の常駐プロセスがあり、セッションではないので除外した。

結論: VS Code 拡張のセッションも監視できる。新しい会話を開いての SessionStart の直接確認は未実施（次の実機確認で見る）。

## 6. hook の所要時間と async

hook 1 回あたり約 0.05 秒（リリースビルド。初回のみ macOS の検査で約 0.46 秒）。対話中に遅れは感じなかった。

結論: 同期のままにする。async にするとイベントの届く順番が前後しうるため。
