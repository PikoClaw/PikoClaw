# Design Spec: Input Bar

**TS source**: `components/PromptInput.tsx`, `hooks/useTextInput.ts`, `hooks/fileSuggestions.ts`

---

## Visual Layout

```
╭──────────────────────────────────────────────────────╮
│ ❯ Ask anything...                                   │
╰──────────────────────────────────────────────────────╯
```

- **Border**: full rounded box
- **Prompt symbol**: `❯` followed by space, then input text
- **Cursor**: solid block `█` when input is editable
- **Background**: theme background
- **Suggestion list**: renders below the input box, not inside it

### Multi-line state

```
╭──────────────────────────────────────────────────────╮
│ ❯ First line of input                               │
│   Second line (Shift+Enter was pressed)             │
│   Third line█                                       │
╰──────────────────────────────────────────────────────╯
```

Lines beyond the first have **2 spaces** indent (aligns with text after `> `).

---

## Border Color States

| State | Border color |
|-------|-------------|
| Default (idle / typing) | `theme.prompt_border` |
| Slash-command suggestions visible | `theme.prompt_border` |
| Agent thinking | `theme.subtle` |
| Provider/API-key overlay visible | input remains in place; dialog uses `theme.claude` border |

There is no separate shimmer border state in the current Rust implementation.

---

## Placeholder Text

Shown only when input is completely empty:

| Context | Placeholder |
|---------|------------|
| Default | `Ask anything...` (dimmed, `theme.inactive`) |

---

## Mode Prefixes

Certain characters at the start of input trigger special modes:

| Prefix | Mode | Border color |
|--------|------|-------------|
| `/` | Slash command — dispatched to command system with typeahead menu | `theme.prompt_border` |

---

## Typeahead / Autocomplete Dropdown

Appears **below** the input bar when suggestions are available.

```
  › /connect              [cmd]   Connect a provider and save its API key
    /compact              [cmd]   Summarize conversation history to reduce...
    /cost                 [cmd]   Show the current session cost summary
```

### Dropdown layout

- **Position**: Immediately below the input box, full width
- **Max height**: 5 rows
- **Scroll**: If more items than fit, scrolls (no scrollbar, just clips)

### Item format

```
  [indicator] /command-name   [cmd]   description
```

- **Left indent**: 2 spaces
- **Focused item**: orange text (`theme.claude`) with bold command label
- **Unfocused**: command name in `theme.text`, metadata/description in dim colors
- **Tag**: dimmed ` [cmd] ` badge
- **Description**: single-line truncated summary

### Suggestion types and their indicators

| Type | Indicator | Color |
|------|-----------|-------|
| Slash command | `› ` on focused row, otherwise two spaces | `theme.claude` when focused, `theme.inactive` otherwise |

### Keyboard navigation

| Key | Action |
|-----|--------|
| `↓` | Move focus down |
| `↑` | Move focus up |
| `Tab` | Accept focused suggestion |
| `Esc` | Dismiss dropdown |
| `Enter` | Accept focused suggestion AND submit |

---

## Image Paste (Attachment Chips)

When user pastes an image from clipboard, a text chip is inserted at the cursor:

```
  > Fix this error [Image #1] in the auth module
```

### Chip appearance

- **Format**: `[Image #N]` where N auto-increments from 1
- **Normal state**: standard input text color
- **Cursor-on-chip state**: inverted colors (chip selected for deletion)
- **Deletion**: Backspace at chip start deletes entire chip

### Text paste chips

Large text pastes (> ~5000 chars) become a reference chip:

```
  > Here is the log output [Pasted text #1 +234 lines] what went wrong?
```

- Format: `[Pasted text #N +M lines]` — M = number of newlines
- Zero-line paste: `[Pasted text #N]`
- Truncated paste: `[...Truncated text #N +M lines...]`

---

## Input History Navigation

> **Status: ✅ Implemented in v0.5.0** (`crates/piko-tui/src/history.rs`, `crates/piko-tui/src/app.rs`)

When input is **empty**:
- `↑` — replace input with previous submitted message
- `↓` — navigate forward in history (toward current empty)

Visual: input text replaces, cursor goes to end.

**Implementation notes:**
- `InputHistory` struct tracks `entries: Vec<String>` and `idx: Option<usize>`.
- Whitespace-only submissions are silently ignored.
- `↑` when input has content but navigation is active continues navigating (allows full round-trips).
- Typing while browsing exits navigation mode, keeping the recalled text editable.
- `↑`/`↓` fall back to chat-pane scroll when history is empty or not navigating.

---

## Character/Line Limits

| Limit | Value | Behavior |
|-------|-------|----------|
| Max visible input lines | ~10 | Input scrolls vertically |
| Max paste inline | 800 bytes or 2 lines | Larger becomes reference chip |
| Max input submit | No hard limit | Very large inputs may hit API limits |
