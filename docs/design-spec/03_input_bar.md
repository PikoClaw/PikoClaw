# Design Spec: Input Bar

**TS source**: `components/PromptInput.tsx`, `hooks/useTextInput.ts`, `hooks/fileSuggestions.ts`

---

## Visual Layout

```
                                               [fast ⚡]
╰──────────────────────────────────────────────────────╯
  > _
```

- **Border**: bottom only, rounded corners (`╰──╯` style)
- **Top-right label**: optional mode indicator (fast mode `⚡`, plan mode `📋`, bash mode `!`)
- **Prompt symbol**: `>` followed by space, then cursor
- **Background**: transparent (inherits terminal)
- **Top margin**: 1 blank line above the border

### Multi-line state

```
╰──────────────────────────────────────────────────────╯
  > First line of input
    Second line (Shift+Enter was pressed)
    Third line_
```

Lines beyond the first have **2 spaces** indent (aligns with text after `> `).

---

## Border Color States

| State | Border color |
|-------|-------------|
| Default (idle) | `theme.inactive` (gray) |
| Focused / typing | `theme.permission` (blue) |
| Bash mode (`!` prefix) | `theme.bash_border` (hot pink `rgb(255,0,135)`) |
| Plan mode active | `theme.plan_mode` (teal) |
| Agent thinking | `theme.spinner` (blue, shimmer pulse) |

### Shimmer during agent thinking

When the agent is processing, the border pulses between `theme.spinner` and `theme.spinner_shimmer` at 500ms intervals.

---

## Placeholder Text

Shown only when input is completely empty:

| Context | Placeholder |
|---------|------------|
| Default | `Ask anything...` (dimmed, `theme.inactive`) |
| After compact | `Context compacted. Continue...` |
| In plan mode | `Describe your plan...` |
| Agent waiting | `(agent is thinking...)` — not user-editable |

---

## Mode Prefixes

Certain characters at the start of input trigger special modes:

| Prefix | Mode | Border color |
|--------|------|-------------|
| `!` | Bash shortcut — runs as shell command directly | Hot pink |
| `/` | Slash command — dispatched to command system | Blue (default) |
| `@` | File/mention — triggers file path autocomplete | Blue (default) |

---

## Typeahead / Autocomplete Dropdown

Appears **below** the input bar when suggestions are available.

```
╰──────────────────────────────────────────────────────╯
  > src/main@

  ▶ src/main.rs           [rust]   Ctrl+→ to accept
    src/main_test.rs      [rust]
    src/main_helpers.rs   [rust]
```

### Dropdown layout

- **Position**: Immediately below the input border, full width
- **Max height**: `floor(terminal_rows / 2)` — never takes more than half the screen
- **Scroll**: If more items than fit, scrolls (no scrollbar, just clips)

### Item format

```
  [indicator] filename.ext    [tag/lang]   shortcut hint
```

- **Left indent**: 2 spaces
- **Focused item**: inverted colors (white bg, dark text)
- **Unfocused**: normal text
- **Tag**: dimmed, right-aligned
- **Shortcut hint**: `theme.inactive`, far right

### Suggestion types and their indicators

| Type | Indicator | Color |
|------|-----------|-------|
| File path | ` ` (space) | default |
| Directory | `/` suffix | default |
| Slash command | `/` prefix | `theme.suggestion` blue |
| Sub-agent @mention | `@` prefix | agent's team color |
| Recent input | `↑` | `theme.inactive` |

### Keyboard navigation

| Key | Action |
|-----|--------|
| `↓` | Move focus down |
| `↑` | Move focus up |
| `Tab` | Accept focused suggestion |
| `Ctrl+→` | Accept word from focused suggestion |
| `Esc` | Dismiss dropdown |
| `Enter` | Accept focused suggestion AND submit |

---

## @file Mention Syntax

When user types `@` followed by a path fragment, file suggestions appear.

### Display in input text

Accepted file paths appear as inline colored text (not a separate widget):
- Color: `theme.suggestion` (blue)
- No background, no border — just colored text in the input stream

### With line range

`@src/main.rs:10-25` — includes only lines 10–25 of the file in context.

When submitted, the agent receives the file content inline in the user message:

```
[File: src/main.rs lines 10-25]
```rust
fn main() {
    ...
}
```

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
| Max paste inline | ~1024 chars | Larger becomes reference chip |
| Max paste total | ~5000 chars | Beyond this, truncated chip shown |
| Max input submit | No hard limit | Very large inputs may hit API limits |
