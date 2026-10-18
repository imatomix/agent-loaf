# 0010. ディレクトリ構成

- ステータス: 承認
- 日付: 2026-10-18

## コンテキスト

公開した直後で、手伝う人がまだいない。`crates/core/src` に 28、`app/src-tauri/src` に 11、`app/src` に 9 のファイルが、どれも 1 つのフォルダーに平らに並んでいた。困ってはいなかったが、新しいコードをどこに置くか、今あるコードがどこにあるかの決まりがなかった。並べ直す手間は、人が増えるほど大きくなる。

## 決定

3 つの置き場所（ADR 0001）はそのままに、それぞれの中を機能ごとのフォルダーに分ける。フォルダーの名前は、3 つの置き場所でできるだけそろえる。

```
crates/core/src/      判断のロジック
  session/  setup/  process/  update/  base/
app/src-tauri/src/    OS とのつなぎ（Tauri 2 の標準。中身は lib.rs、main.rs は呼ぶだけ）
  commands/  session/  setup/  process/  update/  tray/  base/
app/src/              画面
  features/list/  features/settings/  lib/
```

- `session`: 会話の状態。`setup`: 設定ディレクトリとプラグインの導入。`process`: プロセスと画面の移動。`update`: 自動更新。`tray`: メニューバーのアイコンと一覧の窓（OS とのつなぎにしかない）
- 2 つ以上の機能から使うもの、または、どの機能にも属さないアプリ全体の土台（言語の判定、落ちたときの記録など）だけを`base`（Rust）・`lib`（画面）に置く。先回りして共通化しない
- 画面から呼ぶ命令（`#[tauri::command]`）は `app/src-tauri/src/commands/` に、機能ごとのファイルで置く（どの画面から呼ぶかに合わせる）
- 画面の機能は画面の名前（`list`・`settings`）で分ける
- 各置き場所の入り口の README に、中のフォルダーの役割と置き場所の決まりを書く
- `src` という名前は道具（Cargo・Vite）の決まりなので変えない

設計書: [ディレクトリ構成を機能ごとに整理する](../superpowers/specs/2026-10-18-directory-layout-design.md)

## 検討した代替案

- **技術の層で分ける**（データの形・判断・読み書き）: 1 つの機能を直すのに、いくつものフォルダーをまたぐ
- **`crates/core` を、さらにいくつかの crate に分ける**: 境目はコンパイルで守られるが、`Cargo.toml` と crate の間の依存の管理が増える。今の規模には重い
- **方針だけを書き、ファイルは触ったときに動かす**: 古い並びと新しい並びが、しばらく混ざる

## 影響

- ADR 0001 の「`crates/core/src/` は平らに並べる」を置き換える
- 新しいコードは、まず機能のフォルダーに置く。2 つ目の利用先が出てから `base`・`lib` に上げる
- `crates/core` を使う側の呼び方は `agent_loaf_core::session::store` のようになる
- 2026-10-18 より前の設計書と計画（`docs/superpowers/specs/`・`plans/`）に書いたファイルのパスは、整理の前のもの
