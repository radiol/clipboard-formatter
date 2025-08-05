# Clipboard-Formatter

`Clipboard-Formatter`は、クリップボードにコピーされた文字列を監視し、指定されたパターンで自動的に文字列を置換・整形するツールです。このツールは、全角文字を半角に変換するなどの処理を行い、整形されたテキストをクリップボードに再度保存します。

## 特徴

- クリップボードの文字列を監視し、全角文字を半角へ変換。
- カスタマイズ可能な置換ルールと除外リスト。
- 設定ファイルの変更をリアルタイムで検知し、即座に反映。
- **NEW v0.2.0**: 「前回」を含む行の重複削除機能（4行以上のテキストで、上3行中に「前回」を含む行が2つ以上ある場合、2番目に出現する行を削除）

## インストール

### 実行ファイルを[Release](https://github.com/radiol/clipboard-formatter/releases)からダウンロードして実行。

**注意**: このアプリケーションは署名されていないため、macOSではGatekeeperによってブロックされます。下記の方法でbuildしてください。

### cargo build

Rustがインストールされている環境で、以下のコマンドを実行してビルドします。

```bash
git clone git@github.com:radiol/clipboard-formatter.git
cd clipboard-formatter
cargo build --release
```

実行ファイルは`target/release/`ディレクトリに作成されます。

## 使い方

1. アプリケーションを起動すると、クリップボードの内容が監視されます。
2. クリップボードにコピーされたテキストが、設定ファイルで定義されたルールに従って自動的に置換・整形されます。
3. 整形後のテキストは再びクリップボードに保存され、他のアプリケーションに貼り付けることができます。

### 起動

```bash
./target/release/clipboard-formatter
```

Windows向け(exe)はダブルクリックで起動できます。

### 終了

`Ctrl + C`

## 設定ファイル

設定ファイルはアプリケーション初回起動時にデフォルトで生成されます。

### 設定ファイルの位置

設定ファイルの保存場所は、以下の通りです：

- `XDG_CONFIG_HOME`が設定されていれば: `$XDG_CONFIG_HOME/clipboard-formatter/config.toml`
- Linux / macOS: `~/.config/clipboard-formatter/config.toml`
- Windows: `C:\Users\{User}\AppData\Roaming\clipboard-formatter\config.toml`

## 設定の変更方法

1. `config.toml`をエディタで開き、必要な設定を編集します。
2. 保存すると、自動的に変更が検知され、新しい設定が反映されます。

### v0.2.0の新機能設定

「前回」行削除機能を有効にするには、設定ファイルで以下を変更してください：

```toml
[app]
remove_duplicate_previous_lines = true  # デフォルトはfalse
```

**重要**: v0.1.xからv0.2.0へのアップデート時は、既存の設定ファイルを手動で削除してから起動してください。新しい設定項目が追加されているため、既存の設定ファイルでは起動時にエラーが発生する可能性があります。

```bash
# macOS / Linux
rm -f ~/.config/clipboard-formatter/config.toml

# Windows
del "%APPDATA%\clipboard-formatter\config.toml"
```

その後、アプリケーションを起動すると新しい設定ファイルが自動生成されます。

## 開発とテスト

プロジェクトには、Rustの標準的なテストスイートが含まれています。テストを実行するには、以下のコマンドを使用します。

```bash
cargo test
```

特に、クリップボード関連のテストは、ローカル環境で実行されるようになっています。CI環境ではこれらのテストはスキップされます。

## ライセンス

このプロジェクトはMITライセンスの下で公開されています。詳細については`LICENSE`ファイルを参照してください。
