---
description: 実機で確かめる場面（docs/device-checks.md）を 1 つずつ案内し、結果を記録に書く
argument-hint: "[組み合わせ（CT・CV・XV・XT）と場面の番号。例: CV 1-9。省くと聞く]"
---

[docs/device-checks.md](../../docs/device-checks.md) の場面を、メンテナーと一緒に実機で確かめ、結果を「記録」の表に足す。対象は `$ARGUMENTS`（空なら、どの組み合わせと場面を通すかを最初に聞く）。

画面を操作するのはメンテナー。こちらは案内と、結果の記録をする。結果を推測で書かない。

## 始める前に

1. docs/device-checks.md を読み、場面の表と、「記録」の最後の数行を見る
2. 版を調べて見せる
   - Agent Loaf: 動いているアプリ（`ps -axo command | grep agent-loaf-app`）の `Contents/Info.plist` の `CFBundleShortVersionString`。手元で作ったものなら、作った元のコミット（`git rev-parse --short HEAD`）も
   - Claude Code: ターミナルは `claude --version`。VS Code の拡張は `~/.vscode/extensions/anthropic.claude-code-*` の版（古い版のフォルダーが残るので、いちばん新しいもの）
   - Codex: ターミナルは使う `codex` の `--version`。VS Code の拡張は `~/.vscode/extensions/openai.chatgpt-*` の版と、同梱の codex-cli の版
   - macOS: `sw_vers -productVersion`
3. 通知の設定で、要対応・エラー停止・完了がすべてオンになっているかを確かめてもらう

## 場面ごとに

1. 場面の番号・操作・期待することを、そのまま見せる。手順が分かりにくい場面（5 の止め方、8 の閉じ方、9 の新しいチャットの始め方など）は、その組み合わせでの具体的な操作を書き添える
2. 結果を聞く。外れたときは、何が起きたか（状態の表示、通知の有無、かかった時間）を聞く
3. 外れた原因を調べるときは、`~/.agent-loaf/events` と `~/.agent-loaf/state.json` を読む。直すのは、確かめが終わってから、別の変更として行う
4. 時間のかかる場面（10 は 1 時間、11 は数時間のスリープ）は、始めた時刻を記録して、後でまた確かめる

Auto モードの Claude Code では許可を求められないことがある。場面 2 は、選択肢を出して質問させて（AskUserQuestion）確かめてよい。そのときは記録に書く。

## 記録する

1. 「記録」の表の最後に、組み合わせごとに 1 行を足す。日付・版（Agent Loaf・Claude Code・Codex・macOS）・組み合わせ・結果。結果は「1〜8・12 合格。9 は対象外。」のように書き、外れた場面と、確かめ方を変えた場面は、何が起きたかを書く
2. 足した行を見せて、合っているかを確かめてもらう
3. 記録だけの変更は `main` に直接 push してよい（AGENTS.md）。push する前に聞く
