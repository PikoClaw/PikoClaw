# Spec: Image & Screenshot Input

**Status**: 🔶 Partial — clipboard image paste and direct image path references implemented; screenshot capture and full @file autocomplete still missing
**TS source**: `hooks/useClipboardImageHint.ts`, `utils/attachments.ts`

---

## Overview

Allows users to include images in their messages — either by pasting from clipboard, referencing a file path, or taking a screenshot. Images are base64-encoded and sent as `image` content blocks in the API request.

---

## API Format

Images are sent as content blocks in user messages:

```json
{
  "role": "user",
  "content": [
    {
      "type": "image",
      "source": {
        "type": "base64",
        "media_type": "image/png",
        "data": "<base64-encoded-image>"
      }
    },
    {
      "type": "text",
      "text": "What's in this screenshot?"
    }
  ]
}
```

Supported media types: `image/jpeg`, `image/png`, `image/gif`, `image/webp`

Max image size: 5MB per image, max 20 images per request.

---

## Input Methods

### 1. File Path Reference
User types `@/path/to/image.png` or pastes a file path ending in an image extension.

```
User: @/tmp/screenshot.png what's wrong with this error?
```

### 2. Clipboard Paste
User copies an image to clipboard (e.g. screenshot), then pastes in the input bar.
- macOS: detect image data in clipboard via `pbpaste` / `osascript`
- Linux: detect via `xclip` or `wl-paste`

### 3. Screenshot Capture (Optional / future)
A `/screenshot` command or keyboard shortcut that takes a screenshot of the current screen and attaches it.

---

## Implementation Plan

### Step 1: `piko-types` — Add Image Content Block

```rust
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id, name, input },
    ToolResult { tool_use_id, content, is_error },
    Image { source: ImageSource },   // new
}

pub enum ImageSource {
    Base64 { media_type: String, data: String },
    Url { url: String },  // supported by API but less common
}
```

### Step 2: Image Loading Utilities

```rust
// Load image file from disk → base64
pub fn load_image_as_base64(path: &Path) -> Result<(String, String), Error>
// returns (media_type, base64_data)
// detects media_type from file extension or magic bytes

// Read image from clipboard
pub async fn read_clipboard_image() -> Option<(String, String)>
// macOS: osascript to export clipboard image to temp file
// Linux: xclip -selection clipboard -t image/png -o
```

### Step 3: Input Bar — `@` File Attachment

In TUI input parsing:
- Detect `@<filepath>` token in input text
- If path has image extension (`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`): load as image attachment
- Strip `@<filepath>` from text content
- Add as `ContentBlock::Image` alongside text in user message

### Step 4: Input Bar — Clipboard Paste Hint

When user pastes and clipboard contains image data:
- Show hint: `📎 Image in clipboard. Press Ctrl+V to attach, Esc to dismiss`
- On confirm: encode clipboard image, attach to next message

### Step 5: TUI Display

For messages containing image blocks, show placeholder in chat:
```
[Image: screenshot.png (1920x1080, 245KB)]
```
Use sixel or kitty graphics protocol if terminal supports it for inline preview.
Otherwise show text placeholder.

---

## Supported Image Extensions

```
.png .jpg .jpeg .gif .webp .bmp .tiff .tif
```

Only PNG, JPEG, GIF, WEBP can be sent to API — convert BMP/TIFF if needed.

---

## Edge Cases

- Image too large (>5MB): resize or compress before sending, or show error
- Non-image file with image extension: detect via magic bytes (PNG header `\x89PNG`, JPEG `\xFF\xD8`, etc.)
- Multiple images in one message: all included as sequential image blocks
- Images in session history: store base64 data in session JSON (may make session files large)
  - Optimization: store image files separately in `~/.local/share/pikoclaw/images/{session_id}/`, reference by path in session JSON
