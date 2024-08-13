use clipboard::{ClipboardContext, ClipboardProvider};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use regex::Regex;
use std::fs;
use std::path::Path;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

#[derive(Debug, serde::Deserialize)]
struct Replacement {
    original: String,
    replacement: String,
}

fn load_replacements(file_path: &str) -> Vec<Replacement> {
    let data = fs::read_to_string(file_path).expect("Failed to read file");
    serde_json::from_str(&data).expect("Failed to parse JSON")
}

fn format_text(text: &str, replacements: &[Replacement]) -> String {
    let mut formatted_content = text.to_string();
    for replacement in replacements {
        formatted_content =
            formatted_content.replace(&replacement.original, &replacement.replacement);
    }
    let re = Regex::new(r"[！-～]").unwrap();
    formatted_content = re
        .replace_all(&formatted_content, |caps: &regex::Captures| {
            let c = caps[0].chars().next().unwrap();
            let half_width_cahr = (c as u32 - 0xfee0) as u8 as char;
            half_width_cahr.to_string()
        })
        .to_string();
    formatted_content
}
fn main() {
    let mut replacements = load_replacements("replacements.json");
    let (tx, rx) = channel();
    let config = Config::default().with_poll_interval(Duration::from_secs(2));
    let mut watcher: RecommendedWatcher = Watcher::new(tx, config).unwrap();
    watcher
        .watch(Path::new("replacements.json"), RecursiveMode::NonRecursive)
        .unwrap();
    let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();

    loop {
        let clipboard_content = ctx.get_contents().unwrap_or_default();
        let formatted_content = format_text(&clipboard_content, &replacements);
        if clipboard_content != formatted_content {
            println!("Replace {} to {}", clipboard_content, formatted_content);
            ctx.set_contents(formatted_content).unwrap();
        }

        if rx.try_recv().is_ok() {
            println!("replacements.json has been modified.");
            println!("Reloading replacements...");
            replacements = load_replacements("replacements.json");
        }

        thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    // Test for format_text
    #[test]
    fn test_format_text() {
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

        let input = "foo baz １２３４！";
        let expected = "bar qux 1234!";
        let formatted = format_text(input, &replacements);

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

        let clipboard_content = ctx.get_contents().unwrap();
        let formatted_content = format_text(&clipboard_content, &replacements);
        ctx.set_contents(formatted_content.clone()).unwrap();

        assert_eq!(formatted_content, "bar qux 1234!");
        assert_eq!(ctx.get_contents().unwrap(), "bar qux 1234!");
    }
}
