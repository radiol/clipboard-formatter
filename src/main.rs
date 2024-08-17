use clipboard::{ClipboardContext, ClipboardProvider};
use env_logger::Builder as EnvLoggerBuilder;
use log::info;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

const DEFAULT_REPLACEMENTS: &str = include_str!("default_replacements.json");
const DEFAULT_EXCLUSIONS: &str = include_str!("default_exclusions.json");
const REPLACEMENTS_FILE_NAME: &str = "replacements.json";
const EXCLUSIONS_FILE_NAME: &str = "exclusions.json";

#[derive(Debug, serde::Deserialize)]
struct Replacement {
    original: String,
    replacement: String,
}

#[derive(Debug, serde::Deserialize)]
struct Exclusions {
    exclude: Vec<char>,
}

fn get_config_dir() -> PathBuf {
    let config_dir = if let Some(config_dir) = env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(config_dir)
    } else {
        let home_dir = env::var_os("HOME").expect("HOME is not set");
        PathBuf::from(home_dir).join(".config")
    };
    config_dir.join("kill-zen-all")
}

fn create_default_config() {
    let config_dir = get_config_dir();
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).expect("Failed to create config directory");
    }
    let replacement_path = config_dir.join(REPLACEMENTS_FILE_NAME);
    if !replacement_path.exists() {
        let default_replacements = DEFAULT_REPLACEMENTS;
        fs::write(&replacement_path, default_replacements)
            .expect("Failed to create default replacements file");
        info!(
            "Created default replacements file: {}",
            replacement_path.to_str().unwrap()
        );
    }
    let exclusion_path = config_dir.join(EXCLUSIONS_FILE_NAME);
    if !exclusion_path.exists() {
        let default_exclusions = DEFAULT_EXCLUSIONS;
        fs::write(&exclusion_path, default_exclusions)
            .expect("Failed to create default exclusions file");
        info!(
            "Created default exclusions file: {}",
            exclusion_path.to_str().unwrap()
        );
    }
}

fn load_json<T>(file_path: &str) -> T
where
    T: serde::de::DeserializeOwned,
{
    let data = fs::read_to_string(file_path).expect("Failed to read file");
    serde_json::from_str(&data).expect("Failed to parse JSON")
}

fn load_replacements(file_path: &str) -> Vec<Replacement> {
    load_json(file_path)
}

fn load_exclusion_list(file_path: &str) -> Vec<char> {
    let exclusions: Exclusions = load_json(file_path);
    exclusions.exclude
}

fn format_text(text: &str, replacements: &[Replacement], exclusion_list: &[char]) -> String {
    let mut formatted_content = text.to_string();
    for replacement in replacements {
        formatted_content =
            formatted_content.replace(&replacement.original, &replacement.replacement);
    }
    let re = Regex::new(r"[！-～]").unwrap();
    formatted_content = re
        .replace_all(&formatted_content, |caps: &regex::Captures| {
            let c = caps[0].chars().next().unwrap();
            if exclusion_list.contains(&c) {
                c.to_string()
            } else {
                let half_width_cahr = (c as u32 - 0xfee0) as u8 as char;
                half_width_cahr.to_string()
            }
        })
        .to_string();
    formatted_content
}

fn main() {
    EnvLoggerBuilder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    create_default_config();

    let replacement_path = get_config_dir().join(REPLACEMENTS_FILE_NAME);
    let exclusion_path = get_config_dir().join(EXCLUSIONS_FILE_NAME);

    let mut replacements = load_replacements(replacement_path.to_str().unwrap());
    let exclusion_list = load_exclusion_list(exclusion_path.to_str().unwrap());
    let (tx, rx) = channel();
    let config = Config::default().with_poll_interval(Duration::from_secs(2));
    let mut watcher: RecommendedWatcher = Watcher::new(tx, config).unwrap();
    watcher
        .watch(&replacement_path, RecursiveMode::NonRecursive)
        .unwrap();
    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();

    loop {
        let clipboard_content = ctx.get_contents().unwrap_or_default();
        let formatted_content = format_text(&clipboard_content, &replacements, &exclusion_list);
        if clipboard_content != formatted_content {
            info!(
                "Replace '{}' to '{}'.",
                clipboard_content, formatted_content
            );
            ctx.set_contents(formatted_content).unwrap();
        }

        if rx.try_recv().is_ok() {
            info!("{} has been modified.", REPLACEMENTS_FILE_NAME);
            info!("Reloading replacements...");
            replacements = load_replacements(replacement_path.to_str().unwrap());
        }
        thread::sleep(Duration::from_secs(1));
    }
}

// Test code
#[cfg(test)]
mod tests {
    use super::*;

    // Test for get_config_dir
    use std::env;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_create_default_config() {
        // 一時ディレクトリを作成
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        // 一時ディレクトリをXDG_CONFIG_HOMEに設定
        env::set_var("XDG_CONFIG_HOME", &temp_path);

        // create_default_config()を呼び出す
        create_default_config();

        // 設定ファイルが正しい場所に作成されたかを確認
        let replacements_path = temp_path.join("kill-zen-all").join("replacements.json");
        let exclusions_path = temp_path.join("kill-zen-all").join("exclusions.json");

        assert!(
            replacements_path.exists(),
            "replacements.json が存在しません"
        );
        assert!(exclusions_path.exists(), "exclusions.json が存在しません");

        // replacements.json の内容を検証
        let replacements_content =
            fs::read_to_string(&replacements_path).expect("Failed to read replacements.json");
        let expected_replacements_content = r#"[
  { "original": "CRLF", "replacement": "。" },
  { "original": "，", "replacement": ", " }
]"#
        .trim(); // テスト用に改行とインデントを除去

        assert_eq!(replacements_content.trim(), expected_replacements_content);

        // exclusions.json の内容を検証
        let exclusions_content =
            fs::read_to_string(&exclusions_path).expect("Failed to read exclusions.json");
        let expected_exclusions_content = r#"{
  "exclude": ["　", "！", "？"]
}"#
        .trim(); // テスト用に改行とインデントを除去

        assert_eq!(exclusions_content.trim(), expected_exclusions_content);

        // 環境変数のクリーンアップ
        env::remove_var("XDG_CONFIG_HOME");
    }
    // Test for load_replacements
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_load_replacements() {
        let test_data = r#"
        [
            {"original": "foo", "replacement": "bar"},
            {"original": "baz", "replacement": "qux"}
        ]
        "#;

        // Create a test file
        let file_path = "test_replacements.json";
        let mut file = File::create(file_path).unwrap();
        file.write_all(test_data.as_bytes()).unwrap();

        let replacements = load_replacements(file_path);
        assert_eq!(replacements.len(), 2);
        assert_eq!(replacements[0].original, "foo");
        assert_eq!(replacements[0].replacement, "bar");
        assert_eq!(replacements[1].original, "baz");
        assert_eq!(replacements[1].replacement, "qux");

        // Remove the test file
        fs::remove_file(file_path).unwrap();
    }

    // Test for load_exclusion_list
    #[test]
    fn test_load_exclusion_list() {
        let test_data = r#"
        {
            "exclude": ["！", "？"]
        }
        "#;

        // Create a test file
        let file_path = "test_exclusions.json";
        let mut file = File::create(file_path).unwrap();
        file.write_all(test_data.as_bytes()).unwrap();

        let exclusions = load_exclusion_list(file_path);
        assert_eq!(exclusions.len(), 2);
        assert_eq!(exclusions[0], '！');
        assert_eq!(exclusions[1], '？');

        // Remove the test file
        fs::remove_file(file_path).unwrap();
    }

    // Test for format_text
    #[test]
    fn test_format_text_with_exclusions() {
        // 置換リスト
        let replacements = vec![
            Replacement {
                original: "foo".to_string(),
                replacement: "bar".to_string(),
            },
            Replacement {
                original: "baz".to_string(),
                replacement: "qux".to_string(),
            },
        ];

        // 除外リスト
        let exclusion_list = vec!['！', '？']; // 例: 全角の「！」「？」を除外

        // テストケース
        let input = "foo baz １２３４！";
        let expected = "bar qux 1234！"; // ！は除外されるので変換されない
        let formatted = format_text(input, &replacements, &exclusion_list);

        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_format_text_without_exclusions() {
        // 置換リスト
        let replacements = vec![
            Replacement {
                original: "foo".to_string(),
                replacement: "bar".to_string(),
            },
            Replacement {
                original: "baz".to_string(),
                replacement: "qux".to_string(),
            },
        ];

        // 除外リストなし
        let exclusion_list = vec![];

        // テストケース
        let input = "foo baz １２３４？";
        let expected = "bar qux 1234?"; // 全ての文字が変換される
        let formatted = format_text(input, &replacements, &exclusion_list);

        assert_eq!(formatted, expected);
    }

    #[test]
    fn test_format_text_with_partial_exclusions() {
        // 置換リスト
        let replacements = vec![
            Replacement {
                original: "foo".to_string(),
                replacement: "bar".to_string(),
            },
            Replacement {
                original: "baz".to_string(),
                replacement: "qux".to_string(),
            },
        ];

        // 部分的な除外リスト
        let exclusion_list = vec!['！']; // 例: 全角の「！」を除外

        // テストケース
        let input = "foo baz １２３４！？";
        let expected = "bar qux 1234！?"; // ！は変換されず、？は変換される
        let formatted = format_text(input, &replacements, &exclusion_list);

        assert_eq!(formatted, expected);
    }

    // Test for kill-zen-all
    use clipboard::{ClipboardContext, ClipboardProvider};

    #[test]
    fn test_clipboard_integration() {
        let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
        let original_text = "foo baz １２３４！";
        ctx.set_contents(original_text.to_string()).unwrap();

        let replacements = vec![
            Replacement {
                original: "foo".to_string(),
                replacement: "bar".to_string(),
            },
            Replacement {
                original: "baz".to_string(),
                replacement: "qux".to_string(),
            },
        ];
        let exclusion_list = vec![];

        let clipboard_content = ctx.get_contents().unwrap();
        let formatted_content = format_text(&clipboard_content, &replacements, &exclusion_list);
        ctx.set_contents(formatted_content.clone()).unwrap();

        assert_eq!(formatted_content, "bar qux 1234!");
        assert_eq!(ctx.get_contents().unwrap(), "bar qux 1234!");
    }
}
