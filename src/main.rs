use anyhow::{Context, Result};
use clipboard::{ClipboardContext, ClipboardProvider};
use difference::{Changeset, Difference};
use env_logger::Builder as EnvLoggerBuilder;
use log::{info, warn};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
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

struct ConfigManager {
    config_path: PathBuf,
    config: AppConfig,
}

impl ConfigManager {
    fn new() -> Result<Self> {
        let config_path = Self::get_config_path_static()?;
        Self::create_default_config(&config_path)?;
        let config = Self::load_config(&config_path)?;
        Ok(Self {
            config_path,
            config,
        })
    }

    fn get_config_path_static() -> Result<PathBuf> {
        let config_dir = if let Some(config_dir) = env::var_os("XDG_CONFIG_HOME") {
            PathBuf::from(config_dir)
        } else {
            dirs::config_dir().context("Failed to get config directory")?
        };
        Ok(config_dir
            .join("clipboard-formatter")
            .join(CONFIG_FILE_NAME))
    }

    fn create_default_config(config_path: &Path) -> Result<()> {
        let config_dir = config_path.parent().unwrap();
        if !config_dir.exists() {
            fs::create_dir_all(config_dir).context("Failed to create config directory")?;
        }
        if !config_path.exists() {
            fs::write(config_path, DEFAULT_CONFIG).context("Failed to create default config")?;
            info!("Created default config: {}", config_path.display());
        }
        Ok(())
    }

    fn load_config(config_path: &Path) -> Result<AppConfig> {
        let text = fs::read_to_string(config_path)?;
        toml::from_str(&text).context("Failed to parse config.toml")
    }

    fn reload_config(&mut self) -> Result<()> {
        match Self::load_config(&self.config_path) {
            Ok(new_config) => {
                self.config = new_config;
                info!("Reloaded config.toml");
                Ok(())
            }
            Err(e) => {
                warn!("Failed to reload config.toml: {e}");
                Err(e)
            }
        }
    }

    fn get_config(&self) -> &AppConfig {
        &self.config
    }

    fn get_config_path(&self) -> &PathBuf {
        &self.config_path
    }
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

struct ClipboardHandler {
    ctx: ClipboardContext,
}

impl ClipboardHandler {
    fn new() -> Result<Self, ClipboardError> {
        let mut ctx =
            ClipboardContext::new().map_err(|e| ClipboardError::CreateContext(e.to_string()))?;
        if ctx.get_contents().is_err() && ctx.set_contents("".to_string()).is_err() {
            return Err(ClipboardError::CreateContext(
                "Failed to set empty contents".to_string(),
            ));
        }
        Ok(Self { ctx })
    }

    fn set_contents(&mut self, content: String) -> Result<(), ClipboardError> {
        self.ctx
            .set_contents(content)
            .map_err(|e| ClipboardError::SetContents(e.to_string()))
    }

    fn get_contents(&mut self) -> Result<String, ClipboardError> {
        self.ctx
            .get_contents()
            .map_err(|e| ClipboardError::GetContents(e.to_string()))
    }

    fn process_clipboard(&mut self, config: &AppConfig) -> Result<(), ClipboardError> {
        let clipboard_content = self.get_contents()?;
        let formatted_content = format_text(
            &clipboard_content,
            &config.replacements,
            config.exclusions.get("exclusions").unwrap_or(&vec![]),
        )
        .map_err(|e| ClipboardError::GetContents(e.to_string()))?;

        if clipboard_content != formatted_content {
            info!(
                "Formatted\n{}",
                highlight_diff(&clipboard_content, &formatted_content)
            );
            self.set_contents(formatted_content)?;
        }
        Ok(())
    }
}

fn highlight_diff(original: &str, formatted: &str) -> String {
    let changeset = Changeset::new(original, formatted, "");
    let mut highlighted = String::new();
    for change in changeset.diffs {
        match change {
            Difference::Same(s) => highlighted.push_str(&s),
            Difference::Add(s) => highlighted.push_str(&format!("\x1b[32m{s}\x1b[0m")),
            Difference::Rem(s) => highlighted.push_str(&format!("\x1b[31m{s}\x1b[0m")),
        }
    }
    highlighted
}

fn main() -> Result<()> {
    show_self_version();
    EnvLoggerBuilder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let mut config_manager = ConfigManager::new()?;
    let mut clipboard_handler =
        ClipboardHandler::new().context("Failed to create clipboard handler")?;
    let (tx, rx) = channel();
    let _watcher = setup_file_watcher(
        config_manager.get_config_path(),
        config_manager.get_config(),
        tx,
    )?;

    let mut previous_clipboard_hash = 0u64;

    loop {
        previous_clipboard_hash = handle_clipboard_processing(
            &mut clipboard_handler,
            config_manager.get_config(),
            previous_clipboard_hash,
        );

        handle_config_reload(&mut config_manager, &rx);

        thread::sleep(Duration::from_millis(
            config_manager.get_config().app.clipboard_poll_interval,
        ));
    }
}

fn setup_file_watcher(
    config_path: &Path,
    config: &AppConfig,
    tx: std::sync::mpsc::Sender<notify::Result<notify::Event>>,
) -> Result<RecommendedWatcher> {
    let notify_config = Config::default()
        .with_poll_interval(Duration::from_millis(config.app.config_reload_interval));
    let mut watcher: RecommendedWatcher =
        Watcher::new(tx, notify_config).context("Failed to initialize file watcher")?;
    watcher
        .watch(config_path, RecursiveMode::NonRecursive)
        .context("Failed to watch config file")?;
    Ok(watcher)
}

fn handle_clipboard_processing(
    clipboard_handler: &mut ClipboardHandler,
    config: &AppConfig,
    previous_hash: u64,
) -> u64 {
    match clipboard_handler.get_contents() {
        Ok(clipboard_content) => {
            let current_hash = calculate_hash(&clipboard_content);
            if current_hash != previous_hash {
                if let Err(e) = clipboard_handler.process_clipboard(config) {
                    warn!("Failed to process clipboard: {e}");
                }
            }
            current_hash
        }
        Err(e) => {
            warn!("Failed to get clipboard contents: {e}");
            match ClipboardHandler::new() {
                Ok(new_handler) => {
                    *clipboard_handler = new_handler;
                    info!("Successfully recreated clipboard handler");
                }
                Err(e) => {
                    warn!("Failed to recreate clipboard handler: {e}");
                }
            }
            previous_hash
        }
    }
}

fn handle_config_reload(
    config_manager: &mut ConfigManager,
    rx: &std::sync::mpsc::Receiver<notify::Result<notify::Event>>,
) {
    match rx.try_recv() {
        Ok(events) => {
            for event in events.iter() {
                if event.paths.contains(config_manager.get_config_path()) {
                    let _ = config_manager.reload_config();
                }
            }
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            // No events, continue normally
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
            warn!("File watcher disconnected");
        }
    }
}

// Test code
#[cfg(test)]
mod tests {
    use super::*;

    use std::env;
    use tempfile::tempdir;

    #[test]
    fn test_create_default_config() {
        // 一時ディレクトリを作成
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        // 一時ディレクトリをXDG_CONFIG_HOMEに設定
        env::set_var("XDG_CONFIG_HOME", &temp_path);

        // ConfigManagerを作成
        let _config_manager = ConfigManager::new().unwrap();

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
