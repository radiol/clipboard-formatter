# Kill-Zen-All

`Kill-Zen-All`は、クリップボードにコピーされた文字列を監視し、指定されたパターンで自動的に文字列を置換・整形するツールです。このツールは、全角文字を半角に変換するなどの処理を行い、整形されたテキストをクリップボードに再度保存します。

## 特徴

- クリップボードの文字列をリアルタイムで監視し(1秒ごと)、自動で整形。
- カスタマイズ可能な置換ルールと除外リスト。
- 設定ファイルの変更をリアルタイムで検知し、即座に反映。

## インストール

### 実行ファイルを[Release](https://github.com/radiol/kill-zen-all/releases)からダウンロードして実行。

**注意**: このアプリケーションは署名されていないため、macOSではGatekeeperによってブロックされます。下記の方法でbuildしてください。

### cargo build

Rustがインストールされている環境で、以下のコマンドを実行してビルドします。

```bash
git clone https://github.com/yourusername/kill-zen-all.git
cd kill-zen-all
cargo build --release
```

実行ファイルは`target/release/`ディレクトリに作成されます。

## 使い方

1. アプリケーションを起動すると、クリップボードの内容が監視されます。
2. クリップボードにコピーされたテキストが、設定ファイルで定義されたルールに従って自動的に置換・整形されます。
3. 整形後のテキストは再びクリップボードに保存され、他のアプリケーションに貼り付けることができます。

### 起動

```bash
./target/release/kill-zen-all
```

Windows向け(exe)はダブルクリックで起動できます。

### 終了

`Ctrl + C`

## 設定ファイル

`kill-zen-all`は、以下の2つのJSON設定ファイルを使用します。これらの設定ファイルはアプリケーション初回起動時にデフォルトで生成されます。

### 設定ファイルの位置

設定ファイルの保存場所は、以下の通りです：

- `XDG_CONFIG_HOME`が設定されていれば: `$XDG_CONFIG_HOME/kill-zen-all/`
- Linux: `~/.config/kill-zen-all/`
- MacOS: `/Users/{User}/Library/Application Support/kill-zen-all/`
- Windows: `C:\Users\{User}\AppData\Roaming\kill-zen-all\`

### replacements.json

`replacements.json`は、置換する文字列のペアを定義します。以下はデフォルトの設定例です。

```json
[
  { "original": "，", "replacement": ", " },
  { "original": "．", "replacement": ". " },
  { "original": "CRLF", "replacement": "。" },
  { "original": "頚", "replacement": "頸" }
]
```

このファイルには、`original`（置換前の文字列）と`replacement`（置換後の文字列）のペアを指定します。新しいペアを追加する場合、このファイルに新しいJSONオブジェクトを追加してください。

### exclusions.json

`exclusions.json`は、全角から半角に変換する際に除外する文字を定義します。除外対象は全角で指定します。以下はデフォルトの設定例です。

```json
{
  "exclude": ["　", "！", "？", "〜", "～"]
}
```

このファイルには、`exclude`キーに続いて除外する文字のリストを定義します。指定された文字は、全角から半角に変換されません。

## 設定の変更方法

1. `replacements.json`または`exclusions.json`をエディタで開き、必要な設定を編集します。
2. 保存すると、自動的に変更が検知され、新しい設定が即座に反映されます。

## 開発とテスト

プロジェクトには、Rustの標準的なテストスイートが含まれています。テストを実行するには、以下のコマンドを使用します。

```bash
cargo test
```

特に、クリップボード関連のテストは、ローカル環境で実行されるようになっています。CI環境ではこれらのテストはスキップされます。

## ライセンス

このプロジェクトはMITライセンスの下で公開されています。詳細については`LICENSE`ファイルを参照してください。
