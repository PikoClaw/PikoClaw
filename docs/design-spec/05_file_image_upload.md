# Design Spec: File & Image Upload / Attachment

**TS source**: `utils/attachments.ts`, `hooks/useClipboardImageHint.ts`, `components/PromptInput.tsx`

---

## Overview

PikoClaw supports three attachment methods:
1. **Image paste** — copy an image, paste into input bar
2. **Text paste** — paste large text blocks, stored as reference chips
3. **@file mention** — type `@path/to/file` to attach file content

---

## Image Paste Flow

### User action
1. User copies image to clipboard (e.g. screenshots with Cmd+Shift+4, or copies from browser)
2. User presses Cmd+V / Ctrl+V in the input bar

### System behavior
1. Detect clipboard contains image data (not text)
2. Auto-increment image ID counter (starts at 1 per session)
3. Insert chip `[Image #N]` at cursor position
4. Store image data: `{ id, type: "image", content: base64_data, media_type, dimensions? }`
5. If cursor is mid-word (next char is non-space), insert a space after the chip

### Chip appearance in input

```
  > Check this screenshot [Image #1] for any UI issues
```

**Chip states**:

| State | Appearance |
|-------|-----------|
| Normal | `[Image #1]` in default text color |
| Cursor at chip start `[` | inverted (white bg, dark text) — "selected" |
| After deletion | chip and its data both removed |

**Deletion**: Pressing `Backspace` when cursor is immediately after `]` deletes the chip.
Pressing `Backspace` when cursor is at `[` (inverted state) also deletes chip.

### API representation

When submitted, image chips are converted to API content blocks:

```json
{
  "role": "user",
  "content": [
    { "type": "text", "text": "Check this screenshot " },
    {
      "type": "image",
      "source": {
        "type": "base64",
        "media_type": "image/png",
        "data": "iVBORw0KGgoAAAA..."
      }
    },
    { "type": "text", "text": " for any UI issues" }
  ]
}
```

### Supported image types

`image/png` · `image/jpeg` · `image/gif` · `image/webp`

BMP/TIFF: convert to PNG before sending.

### Image size limits

| Limit | Value |
|-------|-------|
| Max per image | 5 MB |
| Max images per message | 20 |
| Max pixels (long edge) | 8192px |

Images exceeding pixel limit: **resize down** before encoding (preserve aspect ratio).
Images exceeding file size after resize: show error `Image too large (max 5MB)`.

### Clipboard hint

On paste when clipboard has image but user hasn't confirmed:

```
  📎 Image in clipboard — press Ctrl+V to attach
```

Shown as a dim hint above the input bar for 3 seconds. Disappears after timeout or after user types.

---

## Large Text Paste Flow

When pasted text exceeds **~1024 characters**:

1. Store full text in paste store (keyed by content hash)
2. Insert reference chip: `[Pasted text #N +M lines]`
   - N = auto-incrementing ID
   - M = number of newlines in the pasted content
3. On submit: inline the full text at the chip's position

### Chip formats

| Content | Chip |
|---------|------|
| Short paste (≤1024 chars) | Inlined directly, no chip |
| Long paste | `[Pasted text #1 +49 lines]` |
| Zero newlines | `[Pasted text #1]` |
| Truncated (>5000 chars) | `[...Truncated text #1 +234 lines...]` |

---

## @file Mention

User types `@` in the input to reference a file.

### Autocomplete trigger

Typing `@` followed by any character triggers the file path suggestion dropdown:

```
  > Look at @src/auth

  ▶ src/auth/jwt.rs
    src/auth/middleware.rs
    src/auth/mod.rs
    src/auth/tests.rs
```

File suggestions are filtered by the typed fragment. Respects `.gitignore`.

### Accepted @mention appearance

After the user selects a file from the dropdown:

```
  > Look at src/auth/jwt.rs
```

The `@` is consumed and replaced with the resolved path in `theme.suggestion` (blue) color.

### With line range

User can append `:line` or `:start-end`:

```
@src/auth/jwt.rs:42
@src/auth/jwt.rs:40-55
```

### What gets sent to API

The file content is fetched and inlined into the user message before submission:

```
Look at [File: src/auth/jwt.rs]
```rust
// lines 40-55
fn validate_token(token: &str) -> Result<Claims> {
    ...
}
```

File attachments are **not** stored as separate content blocks — they're inlined as text in the user message.

### File type handling

| Extension | Behavior |
|-----------|----------|
| Text files (`.rs`, `.ts`, `.md`, etc.) | Inlined as fenced code block |
| Image files (`.png`, `.jpg`, etc.) | Converted to image content block (base64) |
| Binary files (`.exe`, `.zip`, etc.) | Rejected with error: `Binary files cannot be attached` |
| Large files (>100KB) | Warning shown, user confirms |

### Binary file detection

Check by extension first (see `constants/files.ts`), then by magic bytes if extension is ambiguous.

Known binary extensions: `.png .jpg .jpeg .gif .webp .bmp .tiff .pdf .zip .tar .gz .exe .dll .so .dylib .wasm .sqlite .db` (and ~100 more).

---

## In-session Image Storage

Images pasted during a session are stored in memory (`HashMap<u32, PastedContent>`). They are:

- **Not persisted** to the session JSON file by default (would make session files huge)
- **Option**: write image files to `~/.local/share/pikoclaw/images/{session_id}/{id}.png` and store path reference in session JSON for resume

On session resume, if image files exist at the expected paths, they are reloaded. If not, the chip `[Image #1]` is shown as a broken reference in the message history.

---

## Rust Implementation Notes

### Clipboard image reading

```rust
// macOS
pub async fn read_clipboard_image() -> Option<(Vec<u8>, String)> {
    // Run: osascript -e 'set img to (read (POSIX path of (path to temporary items folder)) ...)'
    // Or use: pbpaste -Prefer png (doesn't work for images, use osascript)
    // Practical: use `image` crate + arboard crate for clipboard access
}

// Linux (X11)
// arboard::Clipboard::new()?.get_image()

// Cross-platform: `arboard` crate handles macOS + Linux + Windows
```

### @file suggestion source

```rust
// Reuse GlobTool internals: walk cwd with ignore crate, filter by fragment
pub fn file_suggestions(cwd: &Path, fragment: &str) -> Vec<PathBuf> {
    // walk files respecting .gitignore
    // filter paths where any component starts with fragment
    // sort by modification time (most recent first)
    // cap at 20 results
}
```
