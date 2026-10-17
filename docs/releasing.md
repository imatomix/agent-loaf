# 公開の手順

Agent Loaf の新しい版を、署名・公証した dmg として GitHub の Releases に出す手順です。ふだんはタグを push するだけで、GitHub Actions（`.github/workflows/release.yml`）が作って Releases の下書きに上げます。

## 一度だけの準備

1. **Apple Developer Program に登録する**（個人、年 99 ドル）
2. **Developer ID Application の証明書を作る**
   - Xcode → Settings → Accounts → チームを選ぶ → Manage Certificates → ＋ → Developer ID Application
   - Xcode がないときは、キーチェーンアクセス →「証明書アシスタント」→「認証局に証明書を要求」で要求のファイル（`.certSigningRequest`）を作り、developer.apple.com の Certificates → ＋ → Developer ID Application（G2 Sub-CA）に渡して、できた `.cer` を開く
   - 作れるのはチームの Account Holder だけ。証明書の名前（登録した人やチームの名前）は、署名したアプリから誰でも見られる
   - 入ったことを確かめる: `security find-identity -v -p codesigning` に `Developer ID Application: <名前> (<Team ID>)` が出る
3. **証明書を `.p12` に書き出す**
   - キーチェーンアクセス →「自分の証明書」→ Developer ID Application を右クリック → 書き出す（パスワードを付ける）
   - GitHub に入れるために base64 にする: `base64 -i DeveloperID.p12 | pbcopy`
4. **公証のための App 用パスワードを作る**
   - account.apple.com → サインインとセキュリティ → App 用パスワード
5. **Team ID を確かめる**
   - developer.apple.com → Membership details、または証明書の名前の括弧の中
6. **自動更新の署名の鍵を作る**（まだなら）
   - `npx --prefix app tauri signer generate -w ~/.appkeys/tauri/agent-loaf.key`
   - 公開鍵（`~/.appkeys/tauri/agent-loaf.key.pub` の中身）は `app/src-tauri/tauri.conf.json` の `plugins.updater.pubkey` に入っている
7. **リポジトリを非公開で作る**（下の「最初の公開」の 1〜7。Secrets はリポジトリを作った後でないと入れられない）
8. **GitHub の Secrets に入れる**（`gh secret set <名前>`。値は会話やファイルに残さず、端末で入れる）

   | 名前 | 中身 |
   |---|---|
   | `APPLE_CERTIFICATE` | `.p12` を base64 にしたもの |
   | `APPLE_CERTIFICATE_PASSWORD` | `.p12` に付けたパスワード |
   | `APPLE_SIGNING_IDENTITY` | `Developer ID Application: <名前> (<Team ID>)` |
   | `APPLE_ID` | Apple ID のメールアドレス |
   | `APPLE_PASSWORD` | App 用パスワード |
   | `APPLE_TEAM_ID` | Team ID |
   | `TAURI_SIGNING_PRIVATE_KEY` | `~/.appkeys/tauri/agent-loaf.key` の中身 |
   | `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 自動更新の鍵のパスワード |

## 最初の公開（一度だけ）

公開より前の手元の履歴は公開しない。今の中身から新しい最初のコミットを作り、リポジトリは先に非公開で作る。GitHub の上で CI と公開用のワークフローを動かし、下書きのアプリを確かめてから、公開に切り替える。

### 非公開のリポジトリを作る

1. [実機で確かめる場面](device-checks.md)を通し、公開の前に直すものが、すべて `main` に入っていることを確かめる
2. 手元の履歴を、リポジトリの外の別のフォルダーに写して残す（push の先は持たせない）

   ```bash
   git clone --no-local . ../agent-loaf-history
   git -C ../agent-loaf-history remote remove origin
   ```

3. 今の `main` の中身から、履歴のない新しいブランチを作り、`main` と入れ替える

   ```bash
   git checkout --orphan public
   git commit -m "Agent Loaf を公開する"
   git branch -D main
   git branch -m public main
   ```

4. ほかの古いブランチを手元から消す（`git branch` に `main` だけが残る）
5. `gh auth status` で、公開に使うアカウントでログインしていることを確かめる。`gh repo create --push` と `git push` は gh の設定に従って SSH か HTTPS で送るので、その通り道も同じアカウントになっていることを確かめる
   - SSH のとき: `ssh -T git@github.com` が公開に使うアカウントの名前を返す。別のアカウントの鍵なら、`gh config set git_protocol https` で HTTPS に切り替える
6. 非公開で作って、`main` だけを push する: `gh repo create imatomix/agent-loaf --private --source . --push`
7. CI（`.github/workflows/ci.yml`）が GitHub の上で通ることを `gh run watch` で見届ける。落ちたら直して push する

### 下書きのアプリを作って確かめる

8. 「一度だけの準備」の 8 のとおり、Secrets を入れる
9. タグを push する: `git tag v0.5.0 && git push origin v0.5.0`。`gh run watch` で公開用のワークフローを見届ける
10. Releases の下書きに、dmg、`Agent.Loaf_aarch64.app.tar.gz` と `.sig`、`latest.json` がそろっていることを確かめる（GitHub はファイルの名前の空白を `.` に変える）
11. 下書きの dmg をダウンロードして開き、警告なしに開けること、アプリが動くことを確かめる
12. 実機の確認の後に変えたところがあれば、見直し役（`.claude/agents/code-reviewer.md`）に見てもらう。直したときは、「うまくいかないとき」の「タグを付け直す」で作り直す

### 公開に切り替える

最後までリポジトリは非公開のままにし、外から見えるようになる操作を一番後にする。

13. 下書きに変更点を書き、公開済みにする: `gh release edit v0.5.0 --notes-file <説明文のファイル> --draft=false --latest`。リポジトリが非公開の間は、見えるのはメンテナーだけ。説明文の見え方をここで確かめる
14. リポジトリを公開にする: `gh repo edit imatomix/agent-loaf --visibility public --accept-visibility-change-consequences`
15. `main` に、CI が通らないと Pull Request をマージできない決まりを付ける。無料のアカウントでは非公開のリポジトリに付けられないので、公開にした後に行う

    ```bash
    gh api -X PUT repos/imatomix/agent-loaf/branches/main/protection --input - <<'JSON'
    {"required_status_checks":{"strict":true,"contexts":["check"]},"enforce_admins":false,"required_pull_request_reviews":null,"restrictions":null}
    JSON
    ```

    `check` は CI のジョブの名前。`enforce_admins` を `false` にして、メンテナーが手元でマージした `main` を push できるようにしておく
16. Settings → Advanced Security で、Dependabot alerts と Dependabot security updates を有効にする。`.github/dependabot.yml` は版の更新を知らせるだけで、脆弱性の知らせはこれを有効にしないと届かない。gh の通常の権限では変えられないので、ブラウザで行う
17. Secret scanning と Push protection を有効にする。鍵やパスワードがコミットに入ったときに知らせ、それを含む push を GitHub の側で止める。公開のリポジトリなら無料

    ```bash
    gh api -X PATCH repos/imatomix/agent-loaf --input - <<'JSON'
    {"security_and_analysis":{"secret_scanning":{"status":"enabled"},"secret_scanning_push_protection":{"status":"enabled"}}}
    JSON
    ```

18. ログインしていない状態で、自動更新の情報と dmg が取れることを確かめる: `curl -sL https://github.com/imatomix/agent-loaf/releases/latest/download/latest.json`

Dependabot は、`.github/dependabot.yml` を push した直後に一度動き、`main` の依存が変わるたびに調べ直す。最初の公開の前に届いた Pull Request は、公開した版には入れず、次の版のために `main` に入れる。

非公開の間は、GitHub Actions に無料のアカウントの月ごとの枠がある（macOS は何倍かに数えられる）。CI と公開用のワークフローを数回動かす程度なら収まるが、何度も作り直すときは、GitHub の Settings → Billing で残りを見る。公開に切り替えた後は、使い放題になる。

古い経緯を調べるときは、`../agent-loaf-history` で `git log` や `git show` を使う。

## 控え（必ず）

次の 4 つは、GitHub の Secrets のほかに、パスワード管理のアプリなどにも控えておく。

- 自動更新の秘密鍵（`~/.appkeys/tauri/agent-loaf.key`）とそのパスワード
- Developer ID の `.p12` とそのパスワード

**自動更新の秘密鍵をなくすと、すでに配ったアプリに更新を届けられなくなる。** 利用者に dmg から手で入れ直してもらうしかなくなる。

## 手元での確かめ（最初の 1 回）

GitHub Actions に載せる前に、この Mac で署名から公証までを通して、証明書と設定が正しいことを確かめる。公証にはコマンドラインツールの `notarytool` を使う（Xcode は要らない）。自動更新の成果物は作らないので、自動更新の鍵は要らない。

```bash
./scripts/build-hook-sidecar.sh
cd app
read -s 'APPLE_PASSWORD?App 用パスワード: '   # 貼り付けて Enter。画面とコマンドの履歴に残さないため
export APPLE_PASSWORD
APPLE_SIGNING_IDENTITY="Developer ID Application: <名前> (<Team ID>)" APPLE_ID=<Apple ID> APPLE_TEAM_ID=<Team ID> npm run tauri build -- --target aarch64-apple-darwin --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

- 署名のときに「codesign がキーチェーンに含まれるキーへアクセスしようとしています」と出る。「ログイン」のキーチェーンのパスワード（ふつうは Mac のログインのパスワード）を入れる。「常に許可」にすると、次から聞かれない
- 公証は Apple のサーバーで行うので、数分かかる
- `tauri.conf.json` の `signingIdentity` は `"-"`（手元の開発用）のままでよい。環境変数の `APPLE_SIGNING_IDENTITY` が上書きする

確かめること（`cd ..` でリポジトリのルートに戻ってから）:

- `codesign -dv --verbose=2 "target/aarch64-apple-darwin/release/bundle/macos/Agent Loaf.app"` の `Authority` が `Developer ID Application: …`、`flags` に `runtime`
- 同梱の hook も同じ署名: `codesign -dv "target/aarch64-apple-darwin/release/bundle/macos/Agent Loaf.app/Contents/MacOS/agent-loaf-hook"`
- `spctl -a -vv "target/aarch64-apple-darwin/release/bundle/macos/Agent Loaf.app"` が `source=Notarized Developer ID`
- `xcrun stapler validate "target/aarch64-apple-darwin/release/bundle/macos/Agent Loaf.app"` が通る
- dmg に quarantine の印を付けて開き、警告が出ない: `xattr -w com.apple.quarantine "0081;$(printf %x $(date +%s));Safari;" <dmg>` してから開く。出るのは、初めて開くときの確認（「Apple によるチェックで悪質なソフトウェアは検出されませんでした」）だけ

dmg 自体は署名だけで、公証はされない（Tauri が公証するのは中のアプリ）。中のアプリが公証済みなので、dmg を開くときに警告は出ない（2026-10-14、macOS 26.5.1 で確かめた）。

## 公開のたびに

Claude Code では、`/release <版>`（`.claude/commands/release.md`）が下の手順を行う。実機の確認・タグの push・公開の前では、止まって確かめる。

先に、[実機で確かめる場面](device-checks.md)をすべて通し、結果を記録する。

1. 版の番号を上げる（3 か所。`cargo test --workspace` の `every_version_matches_the_app` が確かめる）
   - ルートの `Cargo.toml` の `[workspace.package]`、`app/package.json`、`plugin/.claude-plugin/plugin.json`
   - 3 つの crate は `version.workspace = true` でルートの版を使う。`tauri.conf.json` には版を書かない（Tauri が `Cargo.toml` の版を使う）
   - `cargo build --workspace && (cd app && npm install)` で `Cargo.lock` と `app/package-lock.json` も更新する
2. コミットして `main` に入れ、push する
3. タグを push する: `git tag v0.5.0 && git push origin v0.5.0`
4. `gh run watch` で GitHub Actions を見届ける
5. Releases の下書きを開き、次がそろっていることを確かめる
   - `Agent.Loaf_<版>_aarch64.dmg`
   - `Agent.Loaf_<版>_aarch64.app.tar.gz` と `Agent.Loaf_<版>_aarch64.app.tar.gz.sig`（tauri-action 1 から版が入る）
   - `latest.json`
6. dmg をダウンロードして開き、警告なしに開けることを確かめる
7. 変更点を書いて「Publish release」を押す
8. 自動更新の情報が新しくなったことを確かめる: `curl -sL https://github.com/imatomix/agent-loaf/releases/latest/download/latest.json`

## うまくいかないとき

- **Actions が失敗した**: `gh run view --log-failed` で理由を見る。署名や公証の失敗は、まず「手元での確かめ」で同じことが起きるかを見る
- **タグを付け直す**: Releases の下書きを消してから、`git tag -d v0.5.0 && git push origin :refs/tags/v0.5.0`、直してからもう一度付ける
- **公開した版に問題があった**: その Release を下書きに戻すか消すと、`latest.json` は 1 つ前の公開の版を指すので、まだ更新していない人には届かない。直した版を次の番号で出す
