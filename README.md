# Purpose Manager

AIエージェントと人間が共有する「目的（Why）」と判断原則の階層を、ローカルのYAMLファイルで管理するデスクトップアプリです。

- **YAMLが唯一の正本。** アプリを閉じていても、AIハーネス（Claude Code など）はYAMLを直接読めます。
- **Whyだけを扱う。** マイルストーン、タスク、セッション履歴は管理しません。
- **ローカルファースト。** クラウド、ログイン、テレメトリーはありません。

仕様は [docs/specification.md](docs/specification.md)、開発計画は [docs/development-plan.md](docs/development-plan.md) を参照してください。

## インストール

[Releases](https://github.com/januaraid/purpose-manager/releases) から使う環境のファイルをダウンロードします。

| 環境 | ファイル |
|---|---|
| Windows 10/11 | `Purpose.Manager_<版>_x64-setup.exe`（または `.msi`） |
| macOS（Apple シリコン） | `Purpose.Manager_<版>_aarch64.dmg` |

**このビルドはコード署名をしていません。** 初回起動時に OS の警告が出ます。

- **Windows:** 「Windows によって PC が保護されました」と出たら、「詳細情報」→「実行」を選びます。
- **macOS:** `.dmg` からアプリを「アプリケーション」へコピーしたあと、初回だけターミナルで次を実行してから起動します。
  ```sh
  xattr -dr com.apple.quarantine "/Applications/Purpose Manager.app"
  ```
  （または、起動を試みたあと「システム設定」→「プライバシーとセキュリティ」で「このまま開く」を選びます）

会社の PC に入れる場合は、先に社内のソフトウェア導入ルールを確認してください。未署名のアプリは導入が認められないことがあります。

## Workspaceの形式

```text
my-purposes/
  workspace.yaml        # name: 表示名
  purposes/
    acme-corp.yaml      # 目的ID = ファイル名
    acme-jobs.yaml      # parent: acme-corp
```

```yaml
schema_version: 1
id: acme-jobs
parent: acme-corp      # ルートは null
name: Acme Jobs
purpose: 求職者が自分に合った仕事に出会える状態をつくる。
principles:
  - 求職者を第一に考える
status: active         # active / archived
```

架空のサンプルが [fixtures/sample-workspace](fixtures/sample-workspace) にあります。

## Claude Code との連携

目的を選んで「CLAUDE.md 用にコピー」を押し、作業フォルダの `CLAUDE.md` に貼り付けます。Claude Code は `parent` をたどって祖先の目的と判断原則を読みます。アプリが `CLAUDE.md` を書き換えることはありません。

## キーボード操作

| キー | 動作 |
|---|---|
| Ctrl+S | 保存 |
| Ctrl+B | サイドパネルの開閉 |
| Ctrl+1 / Ctrl+2 | 詳細／全体図に切り替える |
| Ctrl+＋ / Ctrl+− / Ctrl+0 | 表示倍率を上げる／下げる／100%に戻す（80〜200%。端末ごとに保存） |

macOSでは Ctrl の代わりに Cmd を使います。

## 既知の制約

- GUIで保存すると、YAMLコメントは失われます（保存前に確認が出ます）。キーは既定の順序に並び替えられます。未知のフィールドは保持されます。
- 配布物は未署名です。会社の端末では、端末管理のルールに従ってください。

## 開発

必要なもの: Node.js 22 以上、Rust（stable）、各OSの [Tauri の前提条件](https://tauri.app/start/prerequisites/)。

```sh
npm install
npm run tauri dev        # アプリを起動
npm test                 # フロントエンドのテスト
npm run check            # 型チェック
cd src-tauri && cargo test -p purpose-core   # ドメインのテスト
```

Workspaceの検証だけをコマンドラインで行うこともできます。

```sh
cd src-tauri
cargo run -p purpose-core --example validate -- ../fixtures/sample-workspace
```

### 構成

| パス | 内容 |
|---|---|
| `src-tauri/crates/purpose-core` | YAMLの読み書き・検証・継承・安全な保存（GUIに依存しない） |
| `src-tauri/src` | Tauri コマンド、ファイル監視、端末ローカル設定 |
| `src/lib` | 画面のロジックとコンポーネント（YAMLは扱わない） |
| `fixtures/` | 架空のサンプルWorkspace |

**実在の組織の目的データ、社内パス、認証情報はコミットしないでください。** 実データはこのリポジトリの外に置いてください（`/workspaces` は誤コミット防止のため ignore しています）。

## ライセンス

[MIT](LICENSE)
