# Skip Non-Text Clipboard Content Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When non-text content (e.g., images) is in the clipboard, skip processing silently without clearing clipboard state or producing noisy log output.

**Architecture:** Two targeted edits to `src/main.rs`. First, remove the `set_contents("")` side-effect from `ClipboardHandler::new()` so initialization never clears the clipboard. Second, replace the handler-recreation block in `handle_clipboard_processing()` with a silent skip that returns `previous_hash` unchanged.

**Tech Stack:** Rust, `clipboard` crate (`ClipboardContext`, `ClipboardProvider`)

---

### Task 1: Remove initialization side-effect from `ClipboardHandler::new()`

**Files:**
- Modify: `src/main.rs:186-195`

The current `new()` calls `get_contents()`, and if that fails (e.g., image in clipboard), immediately calls `set_contents("")` which clears the clipboard. Remove this check entirely — successful context creation is sufficient.

- [ ] **Step 1: Run existing tests to establish a baseline**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass (note: `test_clipboard_integration` is skipped in CI).

- [ ] **Step 2: Edit `ClipboardHandler::new()` in `src/main.rs`**

Replace the current implementation (lines 186-195):

```rust
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
```

With:

```rust
fn new() -> Result<Self, ClipboardError> {
    let ctx =
        ClipboardContext::new().map_err(|e| ClipboardError::CreateContext(e.to_string()))?;
    Ok(Self { ctx })
}
```

- [ ] **Step 3: Run tests to confirm no regressions**

```bash
cargo test 2>&1 | tail -20
```

Expected: same results as Step 1 — all tests pass.

- [ ] **Step 4: Confirm clippy is clean**

```bash
cargo clippy -- -D warnings 2>&1 | tail -20
```

Expected: no warnings or errors.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "fix: remove clipboard-clearing side-effect from ClipboardHandler::new()"
```

---

### Task 2: Silence error handling in `handle_clipboard_processing()`

**Files:**
- Modify: `src/main.rs:295-324`

The `Err` arm currently logs a `warn!` and tries to recreate the handler (which triggered `new()` above). Replace it with a silent return of `previous_hash`.

- [ ] **Step 1: Edit the `Err` arm in `handle_clipboard_processing()` in `src/main.rs`**

Find the function `handle_clipboard_processing` and replace its `Err` arm:

```rust
// Before
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
```

With:

```rust
// After
Err(_) => previous_hash,
```

The full function after editing:

```rust
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
        Err(_) => previous_hash,
    }
}
```

- [ ] **Step 2: Run tests to confirm no regressions**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 3: Confirm clippy is clean**

```bash
cargo clippy -- -D warnings 2>&1 | tail -20
```

Expected: no warnings or errors. (The `warn` import may now be unused — clippy will catch this if so; remove it from the `use log::{info, warn};` line if needed.)

- [ ] **Step 4: Confirm `warn` import is still used**

The `warn!` macro is still used in `handle_clipboard_processing()` (`warn!("Failed to process clipboard: {e}")`) and in `handle_config_reload()`. No import change needed.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "fix: skip non-text clipboard content silently instead of clearing clipboard"
```

---

### Task 3: Manual verification

- [ ] **Step 1: Build the release binary**

```bash
cargo build --release 2>&1 | tail -10
```

Expected: `Finished release` with no errors.

- [ ] **Step 2: Run the binary**

```bash
./target/release/clipboard-formatter
```

- [ ] **Step 3: Copy an image (e.g., screenshot) to clipboard**

Use Cmd+Shift+4 on macOS or copy any image from a browser. Confirm:
- The clipboard still contains the image after a few seconds (not cleared to empty)
- No error output in the terminal

- [ ] **Step 4: Copy text to clipboard**

Type or copy any text containing full-width characters (e.g., `１２３`). Confirm:
- The text is formatted (converted to `123`)
- Output shows the diff in green/red in the terminal

- [ ] **Step 5: Stop the binary**

Press `Ctrl+C`.
