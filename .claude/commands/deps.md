---
description: Dependabot の Pull Request を見て、マージしてよいかをまとめる
argument-hint: "[Pull Request の番号。省くと開いているものすべて]"
---

Dependabot の Pull Request（`$ARGUMENTS`。空なら `gh pr list --author app/dependabot` で開いているものすべて）を見て、マージしてよいかを判断する材料をまとめる。マージは、メンテナーの許しが出てから行う。

## 1 つずつ見る

1. `gh pr view` と `gh pr diff --name-only` で、何の版がどこからどこへ上がるかをつかむ
2. 変更点を読む。Pull Request の本文のリリースノートで足りなければ、上流の Releases や CHANGELOG を読む
   - **破壊的な変更**: 大きな版の更新では必ず探す。使っている API や設定（ワークフローの `with:` など）が変わっていないかを、このリポジトリのコードと突き合わせる
   - **セキュリティの修正**: リリースノートと、上流の security advisories（`gh api repos/<上流>/security-advisories`）を見る。Tauri は `tauri-apps/tauri` と `tauri-apps/plugins-workspace`
3. CI が通っているかを見る（`gh pr checks`）

## 気をつけること

- **Tauri**: Rust（`tauri`）と npm（`@tauri-apps/api`・`@tauri-apps/cli`）の版の major.minor をそろえる。Dependabot は別々の Pull Request で出すので、片方だけのものは、もう片方と一緒に入れる。入れる前に、手元で `tauri build`（`--config '{"bundle":{"createUpdaterArtifacts":false}}'`）を通す。ずれていると、ここで止まる
- **tauri-action と公開用のワークフロー**: CI では確かめられない。次の版を出すとき（`/release`）に、下書きのファイルと `latest.json` を確かめる
- **アプリに入る依存**: 振る舞いが変わりうるものは、次の版の前の実機の確認（`/device-check`）で確かめる、と書いておく
- Dependabot alerts（`gh api repos/imatomix/agent-loaf/dependabot/alerts`）に開いているものがあれば、先にそれを扱う

## まとめて見せる

Pull Request ごとに、版の上がり方・破壊的な変更・セキュリティの修正・CI・判断（マージしてよい／後にする／入れない）と理由を、表で見せる。急ぐもの（脆弱性の修正）があれば、先に書く。

## マージする

許しが出たら:

- 1 つずつ入れられるものは、`gh pr merge <番号> --rebase --delete-branch`（`-R` を付けない。付けると手元のブランチが残る）
- 同じファイル（`Cargo.lock` など）がぶつかるものは、手元のブランチに集めて `./scripts/check.sh` と `tauri build` を通し、Pull Request にして入れる。元の Pull Request は、中身が `main` に入ったことを書いて閉じる
