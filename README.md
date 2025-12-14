# license-scout

Rust製のライセンス可視化CLIです。`frontend`/`backend`など複数ディレクトリを一括で走査し、`requirements.txt`と`package-lock.json`から依存を収集。`--fetch-licenses`を付けるだけでPyPI/npm Registryからライセンスと公式URLを取得し、色付きテーブル＋JSONで一覧化します。

## これでできること

- 解析対象を複数指定し、`venv`や`node_modules`などを自動で除外
- 依存名／バージョン／ライセンス／ソースファイル／公式URLをテーブル表示
- JSONをファイルへ保存、または標準出力へ出力
- ネットワーク取得結果をローカルキャッシュし、次回以降は高速化

## セットアップ

```sh
# 開発中はそのまま実行
cargo run -- [OPTIONS]

# バイナリとして使う場合
cargo install --path .
export PATH="$HOME/.cargo/bin:$PATH"
license-scout --help
```

## 使い方

```sh
# プロジェクト全体を解析
license-scout --path ~/dev/yourproject

# backendとfrontendを個別指定
license-scout --path ~/dev/yourproject/backend --path ~/dev/yourproject/frontend

# ライセンス情報を補完し、JSONファイルに保存
license-scout --path ~/dev/yourproject \
      --fetch-licenses \
      --json-output ~/dev/yourproject/licenses.json

# JSONを標準出力にも出したい場合
license-scout --path ~/dev/yourproject --fetch-licenses --print-json

# reactを含む依存だけを検索して表示
license-scout --path ~/dev/yourproject --search react

# テーブルからSource列を隠す
license-scout --path ~/dev/yourproject --hide-source
```

## 主なオプション

| オプション | 説明 |
| --- | --- |
| `-p, --path <PATH>` | 解析対象ディレクトリ。複数指定可（省略時はカレントディレクトリ） |
| `--fetch-licenses` | PyPI/npm Registryにアクセスし、不明なライセンス・公式URLを補完 |
| `--json-output <FILE>` | JSONを書き出すファイルパス |
| `--print-json` | JSONを標準出力にも表示 |
| `--search <QUERY>` | 指定文字列を含む依存のみ表示（名前・マネージャ・ライセンス・バージョン・URL・ソースが対象） |
| `--hide-source` | テーブル出力からSource列を非表示にする |

## 出力例

```
┌────────┬───────────┬─────────┬────────────┬─────────────────────┬───────────────────────────────┐
│ Manager│ Name      │ Version │ License    │ Homepage            │ Source                        │
├────────┼───────────┼─────────┼────────────┼─────────────────────┼───────────────────────────────┤
│ npm    │ react     │ 18.2.0  │ MIT        │ https://react.dev   │ frontend/package-lock.json    │
│ pip    │ requests  │ 2.32.0  │ Apache-2.0 │ https://requests.dev│ backend/requirements.txt      │
└────────┴───────────┴─────────┴────────────┴─────────────────────┴───────────────────────────────┘
```

JSONには以下のような`DependencyRecord`構造体の配列が出力されます。

```json
[
  {
    "manager": "npm",
    "name": "react",
    "version": "18.2.0",
    "license": "MIT",
    "homepage": "https://react.dev",
    "source": "frontend/package-lock.json"
  }
]
```

## 注意事項

- `--fetch-licenses`使用時はネットワークアクセスが発生します。オフライン環境ではキャッシュが無い場合に失敗します。
- 現状は`requirements.txt`と`package-lock.json`のみ対応です（`poetry.lock`/`yarn.lock`などは未対応）。

## 開発コマンド

```sh
cargo fmt
cargo test
```

改善案や要望があればお気軽にどうぞ！
