---
description: 新しい版を公開する（docs/releasing.md の「公開のたびに」）。タグを push する前に必ず止まる
argument-hint: "<版。例: 0.5.1>"
disable-model-invocation: true
---

版 `$ARGUMENTS` を公開する。手順の元は [docs/releasing.md](../../docs/releasing.md) の「公開のたびに」。この文書と食い違うときは docs/releasing.md に従い、食い違いを伝える。

**止まるところ**: 次の 3 か所では、必ず止まってメンテナーの返事を待つ。返事がないまま先に進まない。

- A. 実機の確認が済んでいるか（手順 1）
- B. タグを push してよいか（手順 4 の前）
- C. 説明文と、公開してよいか（手順 8 の前）

## 1. 始める前に確かめる

- `$ARGUMENTS` が `x.y.z` の形で、今の版（ルートの `Cargo.toml` の `[workspace.package]`）より大きい。違えば止める
- `main` にいて、まだコミットしていない変更がなく、`origin/main` と同じ
- `gh auth status` で、アクティブなアカウントが imatomix
- 同じ名前のタグが、手元にも `origin` にもない
- [docs/device-checks.md](../../docs/device-checks.md) の記録の最後の数行と、前の版のタグからの `git log --oneline` を見せて、**止まる（A）**。この版の中身で実機の確認を通したかを聞く。通していなければ、ここで終える

## 2. 版の番号を上げる

1. `release/v$ARGUMENTS` のブランチを切る
2. 3 か所の版を `$ARGUMENTS` にする: ルートの `Cargo.toml` の `[workspace.package]`、`app/package.json`、`plugin/.claude-plugin/plugin.json`
3. `cargo build --workspace && (cd app && npm install)` で、`Cargo.lock` と `app/package-lock.json` を更新する
4. `./scripts/check.sh` を通す（`every_version_matches_the_app` が 3 か所をそろえて確かめる）
5. 「版を $ARGUMENTS にする」とコミットする

## 3. main に入れる

ブランチを push し、`/pr-description` の決まりで本文を作って Pull Request にする（AGENTS.md）。CI が通ったら `gh pr merge --rebase --delete-branch` でマージし（`-R` を付けない）、手元の `main` を `git pull --ff-only` で合わせる。

## 4. タグを push する

前の版のタグから今までのコミットの一覧を見せて、**止まる（B）**。許しが出たら、`git tag v$ARGUMENTS && git push origin v$ARGUMENTS`。

## 5. 公開用のワークフローを見届ける

`gh run watch` で release を見届ける。落ちたら `gh run view --log-failed` で理由を読み、docs/releasing.md の「うまくいかないとき」に沿って直し方を相談する。自分の判断でタグを付け直さない。

## 6. 下書きを確かめる

1. 下書きに、`Agent.Loaf_<版>_aarch64.dmg`、`Agent.Loaf_<版>_aarch64.app.tar.gz` と `.sig`、`latest.json` がある（`<版>` は $ARGUMENTS。GitHub は名前の空白を `.` に変える）
2. `latest.json` の `version` が `$ARGUMENTS` で、`darwin-aarch64` に URL と署名がある
3. 下書きのファイルをスクラッチのフォルダーに落とし、dmg の中のアプリについて確かめる
   - `codesign -dv`: Developer ID で署名され、`flags` に `runtime` がある。`Contents/MacOS/` の hook も同じ
   - `spctl -a -vv -t exec`: `source=Notarized Developer ID`
   - `xcrun stapler validate`: 通る

## 7. メンテナーに開いてもらう

ブラウザで下書きの dmg を落とし、アプリを入れ替えて開いてもらう。警告なしに開けて、一覧に会話が出ることを確かめてもらう。

前の版からの自動更新は、ここではまだ試せない（`latest.json` は公開済みの最新の版を指す）。手順 9 の後に行う。

## 8. 説明文を書いて公開する

前の版のタグからのコミットと変更から、日本語と英語の説明文を作り、スクラッチのファイルに書く。振る舞いについて書くことは、コードで確かめてから書く。説明文を見せて、**止まる（C）**。許しが出たら:

```bash
gh release edit v$ARGUMENTS --notes-file <説明文のファイル> --draft=false --latest
```

## 9. 公開を確かめる

ログインしていない状態（`curl`）で、`https://github.com/imatomix/agent-loaf/releases/latest/download/latest.json` の `version` が `$ARGUMENTS` になり、dmg が取れることを確かめる。

前の版が入っている Mac があれば、自動更新を試してもらう。前の版の設定の画面で「アップデートして再起動」を押し、この版で起動し直すかを見る。前の版は 24 時間ごとにしか確かめないので、すぐ出ないときは前の版を起動し直す（起動の約 1 分後に確かめる）。

## 終わったら

行ったこと、確かめたこと、確かめられなかったこと（自動更新を試していない、など）を書いて伝える。
