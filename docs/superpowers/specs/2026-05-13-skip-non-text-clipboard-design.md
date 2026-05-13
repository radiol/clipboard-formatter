# Design: Skip Non-Text Clipboard Content

**Date:** 2026-05-13
**Status:** Approved

## Problem

When non-text content (e.g., images) is copied to the clipboard, `get_contents()` returns an error. The current code handles this error by attempting to recreate `ClipboardHandler`, which calls `ClipboardHandler::new()`. Inside `new()`, if `get_contents()` fails, `set_contents("")` is called to initialize the clipboard — this clears the user's clipboard content. Additionally, every polling cycle logs a `warn!` message, producing noisy output.

## Goal

When the clipboard contains non-text content, skip processing silently without modifying clipboard state or producing log output.

## Scope

Two changes in `src/main.rs`:

1. Remove the initialization side-effect in `ClipboardHandler::new()`
2. Simplify error handling in `handle_clipboard_processing()`

## Design

### Change 1: `ClipboardHandler::new()` — Remove initialization logic

**Before:**
```rust
fn new() -> Result<Self, ClipboardError> {
    let mut ctx = ClipboardContext::new().map_err(|e| ClipboardError::CreateContext(e.to_string()))?;
    if ctx.get_contents().is_err() && ctx.set_contents("".to_string()).is_err() {
        return Err(ClipboardError::CreateContext(
            "Failed to set empty contents".to_string(),
        ));
    }
    Ok(Self { ctx })
}
```

**After:**
```rust
fn new() -> Result<Self, ClipboardError> {
    let ctx = ClipboardContext::new().map_err(|e| ClipboardError::CreateContext(e.to_string()))?;
    Ok(Self { ctx })
}
```

**Rationale:** Successful context creation is sufficient. The current clipboard content (text or image) is irrelevant at initialization time. The `set_contents("")` call was the direct cause of clipboard clearing.

### Change 2: `handle_clipboard_processing()` — Silent skip on error

**Before:**
```rust
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

**After:**
```rust
Err(_) => {
    previous_hash
}
```

**Rationale:** Errors from `get_contents()` when non-text is in the clipboard are expected and recurring (once per poll interval). Logging would produce continuous noise. Handler recreation is unnecessary since the context itself is not broken — only the content type is unsupported. Returning `previous_hash` unchanged means the next cycle will re-evaluate the clipboard, picking up text content when it is copied again.

## Error Handling

- Context creation errors (`ClipboardContext::new()` failure) still propagate as `ClipboardError::CreateContext` and are handled upstream.
- `get_contents()` errors (non-text content) are silently swallowed per cycle.
- `set_contents()` errors (during formatting write-back) are still logged via `process_clipboard()`.

## Testing

- Existing tests are unaffected. No test directly exercises `ClipboardHandler::new()` with a real clipboard context.
- No new unit tests are added; mocking the `clipboard` crate's context would add complexity without meaningful coverage gain.
- Manual verification: copy an image, confirm clipboard is not cleared; copy text after, confirm formatting works normally.
