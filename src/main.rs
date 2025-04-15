use anyhow::{Context, Result};
use clipboard::{ClipboardContext, ClipboardProvider};
use difference::{Changeset, Difference};
use env_logger::Builder as EnvLoggerBuilder;
use log::info;
use log::warn;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use std::char;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;
use thiserror::Error;

const DEFAULT_CONFIG: &str = include_str!("default_config.toml");
const CONFIG_FILE_NAME: &str = "config.toml";

fn show_self_version() {
    println!("clipboard-formatter v{}", env!("CARGO_PKG_VERSION"));
}

#[derive(Debug, serde::Deserialize)]
struct AppSettings {
    clipboard_poll_interval: u64,
    config_reload_interval: u64,
}

type Replacements = HashMap<String, String>;

type Exclusions = HashMap<String, Vec<char>>;

#[derive(Debug, serde::Deserialize)]
struct AppConfig {
    app: AppSettings,
    replacements: Replacements,
    exclusions: Exclusions,
}

#[derive(Debug, Error)]
enum ClipboardError {
    #[error("Failed to create clipboard provider: {0}")]
    CreateContext(String),
    #[error("Failed to set clipboard contents: {0}")]
    SetContents(String),
    #[error("Failed to get clipboard contents: {0}")]
    GetContents(String),
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

fn get_config_dir() -> Result<PathBuf> {
    let config_dir = if let Some(config_dir) = env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(config_dir)
    } else {
        dirs::config_dir().context("Failed to get config directory")?
    };
    Ok(config_dir.join("clipboard-formatter"))
}

fn create_default_config() -> Result<()> {
    let config_dir = get_config_dir()?;
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
    }
    let config_path = config_dir.join(CONFIG_FILE_NAME);
    if !config_path.exists() {
        fs::write(&config_path, DEFAULT_CONFIG).context("Failed to create default config")?;
        info!("Created default config: {}", config_path.display());
    }
    Ok(())
}

fn load_config(file_path: &str) -> Result<AppConfig> {
    let text = fs::read_to_string(file_path)?;
    toml::from_str(&text).context("Failed to parse config.toml")
}

fn format_text(text: &str, replacements: &Replacements, exclusion_list: &[char]) -> Result<String> {
    let mut formatted_content = text.to_string();
    for (original, replacement) in replacements.iter() {
        formatted_content = formatted_content.replace(original, replacement);
    }
    let re = Regex::new(r"[！-～]").context("Failed to create regex pattern")?;
    formatted_content = re
        .replace_all(&formatted_content, |caps: &regex::Captures| {
            let c = caps[0].chars().next().unwrap_or_default();
            if exclusion_list.contains(&c) {
                c.to_string()
            } else {
                let half_width_char = (c as u32 - 0xfee0) as u8 as char;
                half_width_char.to_string()
            }
        })
        .to_string();
    Ok(formatted_content)
}

fn create_clipboard_context() -> Result<ClipboardContext, ClipboardError> {
    let mut ctx =
        ClipboardContext::new().map_err(|e| ClipboardError::CreateContext(e.to_string()))?;
    if ctx.get_contents().is_err() && ctx.set_contents("".to_string()).is_err() {
        return Err(ClipboardError::CreateContext(
            "Failed to set empty contents".to_string(),
        ));
    };
    Ok(ctx)
}

fn set_clipboard_contents(
    ctx: &mut ClipboardContext,
    content: String,
) -> Result<(), ClipboardError> {
    ctx.set_contents(content)
        .map_err(|e| ClipboardError::SetContents(e.to_string()))?;
    Ok(())
}

fn get_clipboard_contents(ctx: &mut ClipboardContext) -> Result<String, ClipboardError> {
    ctx.get_contents()
        .map_err(|e| ClipboardError::GetContents(e.to_string()))
}

fn highlight_diff(original: &str, formatted: &str) -> String {
    let changeset = Changeset::new(original, formatted, "");
    let mut highlighted = String::new();
    for change in changeset.diffs {
        match change {
            Difference::Same(s) => highlighted.push_str(&s),
            Difference::Add(s) => highlighted.push_str(&format!("\x1b[32m{}\x1b[0m", s)),
            Difference::Rem(s) => highlighted.push_str(&format!("\x1b[31m{}\x1b[0m", s)),
        }
    }
    highlighted
}

fn main() -> Result<()> {
    show_self_version();
    EnvLoggerBuilder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    create_default_config()?;

    let app_config_path = get_config_dir()?.join(CONFIG_FILE_NAME);
    let mut app_config = load_config(
        app_config_path
            .to_str()
            .context("Failed to convert path to string")?,
    )
    .context("Failed to load config")?;

    let (tx, rx) = channel();
    let notify_config = Config::default()
        .with_poll_interval(Duration::from_millis(app_config.app.config_reload_interval));
    let mut watcher: RecommendedWatcher =
        Watcher::new(tx.clone(), notify_config).context("Failed to initialize file watcher")?;
    watcher
        .watch(&app_config_path, RecursiveMode::NonRecursive)
        .context("Failed to watch config file")?;
    let mut ctx: ClipboardContext =
        create_clipboard_context().context("Failed to create context")?;

    let mut previous_clipboard_hash = 0u64;

    loop {
        // Get clipboard content with better error handling
        match get_clipboard_contents(&mut ctx) {
            Ok(clipboard_content) => {
                let current_clipboard_hash = calculate_hash(&clipboard_content);
                if current_clipboard_hash != previous_clipboard_hash {
                    if let Ok(formatted_content) = format_text(
                        &clipboard_content,
                        &app_config.replacements,
                        app_config.exclusions.get("exclusions").unwrap_or(&vec![]),
                    ) {
                        if clipboard_content != formatted_content {
                            info!(
                                "Formatted\n{}",
                                highlight_diff(&clipboard_content, &formatted_content)
                            );
                            // Don't crash if setting clipboard fails
                            if let Err(e) = set_clipboard_contents(&mut ctx, formatted_content) {
                                warn!("Failed to set clipboard contents: {}", e);
                            }
                        }
                    } else {
                        warn!("Failed to format clipboard text");
                    }
                    previous_clipboard_hash = current_clipboard_hash;
                }
            }
            Err(e) => {
                warn!("Failed to get clipboard contents: {}", e);
                // If clipboard access fails, recreate the clipboard context
                match create_clipboard_context() {
                    Ok(new_ctx) => {
                        ctx = new_ctx;
                        info!("Successfully recreated clipboard context");
                    }
                    Err(e) => {
                        warn!("Failed to recreate clipboard context: {}", e);
                        // Wait a bit longer before retrying when clipboard is inaccessible
                        thread::sleep(Duration::from_millis(
                            app_config.app.clipboard_poll_interval,
                        ));
                    }
                }
            }
        }

        // Check for file changes
        match rx.try_recv() {
            Ok(events) => {
                for event in events.iter() {
                    if event.paths.contains(&app_config_path) {
                        match load_config(app_config_path.to_str().unwrap()) {
                            Ok(new_config) => {
                                app_config = new_config;
                                info!("Reloaded config.toml");
                            }
                            Err(e) => warn!("Failed to reload config.toml: {}", e),
                        }
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // No events, continue normally
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                warn!("File watcher disconnected, attempting to reconnect");
                if let Ok(new_watcher) = Watcher::new(tx.clone(), notify_config) {
                    watcher = new_watcher;
                    watcher.watch(&app_config_path, RecursiveMode::NonRecursive)?;
                } else {
                    warn!("Failed to recreate file watcher");
                }
            }
        }

        // Sleep to prevent high CPU usage
        thread::sleep(Duration::from_millis(
            app_config.app.clipboard_poll_interval,
        ));
    }
}

// Test code
#[cfg(test)]
mod tests {
    use super::*;

    // Test for get_config_dir
    use std::env;
    use tempfile::tempdir;

    #[test]
    fn test_create_default_config() {
        // 一時ディレクトリを作成
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        // 一時ディレクトリをXDG_CONFIG_HOMEに設定
        env::set_var("XDG_CONFIG_HOME", &temp_path);

        // create_default_config()を呼び出す
        create_default_config().unwrap();

        // 設定ファイルが正しい場所に作成されたかを確認
        let config_path = temp_path.join("clipboard-formatter").join("config.toml");

        assert!(config_path.exists(), "config.tomlが存在しません");

        // 環境変数のクリーンアップ
        env::remove_var("XDG_CONFIG_HOME");
    }

    // Test for format_text
    #[test]
    fn test_format_text_with_replacements_exclusions() {
        // 置換リスト
        let replacements = HashMap::from([
            ("foo".to_string(), "bar".to_string()),
            ("baz".to_string(), "qux".to_string()),
        ]);

        // 除外リスト
        let exclusion_list = vec!['！', '？']; // 例: 全角の「！」「？」を除外

        // テストケース
        let input = "foo baz １２３４！？";
        let expected = "bar qux 1234！？"; // ！？は除外されるので変換されない
        let formatted = format_text(input, &replacements, &exclusion_list).unwrap();

        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_format_text_with_replacements_without_exclusions() {
        // 置換リスト
        let replacements = HashMap::from([
            ("foo".to_string(), "bar".to_string()),
            ("baz".to_string(), "qux".to_string()),
        ]);

        // 除外リストなし
        let exclusion_list = vec![];

        // テストケース
        let input = "foo baz １２３４！？";
        let expected = "bar qux 1234!?"; // 全ての文字が変換される
        let formatted = format_text(input, &replacements, &exclusion_list).unwrap();

        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_format_text_without_replacements_with_exclusions() {
        // 置換リストなし
        let replacements = HashMap::new();

        // 除外リスト
        let exclusion_list = vec!['！', '？']; // 例: 全角の「！」「？」を除外

        // テストケース
        let input = "foo baz １２３４！？";
        let expected = "foo baz 1234！？"; // ！？は除外されるので変換されない
        let formatted = format_text(input, &replacements, &exclusion_list).unwrap();

        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_format_text_without_replacements_exclusions() {
        // 置換リストなし
        let replacements = HashMap::new();

        // 除外リストなし
        let exclusion_list = vec![];

        // テストケース
        let input = "foo baz １２３４！？";
        let expected = "foo baz 1234!?"; // 全ての文字が変換される
        let formatted = format_text(input, &replacements, &exclusion_list).unwrap();

        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_format_text_with_partial_exclusions() {
        // 置換リスト

        let replacements = HashMap::from([
            ("foo".to_string(), "bar".to_string()),
            ("baz".to_string(), "qux".to_string()),
        ]);

        // 部分的な除外リスト
        let exclusion_list = vec!['！']; // 例: 全角の「！」を除外

        // テストケース
        let input = "foo baz １２３４！？";
        let expected = "bar qux 1234！?"; // ！は変換されず、？は変換される
        let formatted = format_text(input, &replacements, &exclusion_list).unwrap();

        assert_eq!(formatted, expected);
    }

    // Test for highlight_diff
    #[test]
    fn test_diff_no_changes() {
        let original = "This is a test.";
        let formatted = "This is a test.";
        let result = highlight_diff(original, formatted);
        // 差分がない場合はそのままの文字列が返るはず
        assert_eq!(result, "This is a test.");
    }

    #[test]
    fn test_diff_with_addition() {
        let original = "This is a test";
        let formatted = "This is a test!";
        let result = highlight_diff(original, formatted);
        // 追加された「!」が緑色（ANSIエスケープシーケンスで囲まれている）で表示される
        let expected = "This is a test\x1b[32m!\x1b[0m";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_diff_with_removal() {
        let original = "This is a test!";
        let formatted = "This is a test";
        let result = highlight_diff(original, formatted);
        // 削除された「!」が赤色で表示される
        let expected = "This is a test\x1b[31m!\x1b[0m";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_diff_with_complex_changes() {
        let original = "A string";
        let formatted = "B string";
        let result = highlight_diff(original, formatted);
        // 変更された削除された'A'が赤色、追加された'B'が緑色で表示される
        let expected = "\x1b[31mA\x1b[0m\x1b[32mB\x1b[0m string";
        assert_eq!(result, expected);
    }

    // Test for clipboard-formatter
    use clipboard::{ClipboardContext, ClipboardProvider};

    #[test]
    fn test_clipboard_integration() {
        if std::env::var("CI").is_ok() {
            // CI環境ではクリップボードを操作できないのでスキップ
            // このテストはローカル環境でのみ実行される
            return;
        }
        let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
        let original_text = "foo baz １２３４！";
        ctx.set_contents(original_text.to_string()).unwrap();

        let replacements = HashMap::from([
            ("foo".to_string(), "bar".to_string()),
            ("baz".to_string(), "qux".to_string()),
        ]);
        let exclusion_list = vec![];

        let clipboard_content = ctx.get_contents().unwrap();
        let formatted_content =
            format_text(&clipboard_content, &replacements, &exclusion_list).unwrap();
        ctx.set_contents(formatted_content.clone()).unwrap();

        assert_eq!(formatted_content, "bar qux 1234!");
        assert_eq!(ctx.get_contents().unwrap(), "bar qux 1234!");
    }
}
