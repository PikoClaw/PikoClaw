# Design Spec: Keyboard Help & Shortcut Display

**TS source**: `components/HelpV2/`, `keybindings/`

---

## `/help` Output (in-TUI)

The `/help` command renders an inline help panel in the chat pane (not a separate modal).

```
╭─ PikoClaw Help ──────────────────────────────────────────╮
│                                                          │
│  BUILT-IN COMMANDS                                       │
│  ─────────────────────────────────────────────────────   │
│  /help              Show this help                       │
│  /clear             Clear conversation history           │
│  /compact           Summarize to free context            │
│  /model <name>      Switch model                         │
│  /theme [name]      Change color theme                   │
│  /sessions          List saved sessions                  │
│  /resume <id>       Resume a session                     │
│  /cost              Show session cost                    │
│  /context           Show context usage                   │
│  /permissions       View permission rules                │
│  /mcp               List MCP servers                     │
│  /memory            View memory files                    │
│  /plan              Toggle plan mode                     │
│  /vim               Toggle vim keybindings               │
│  /buddy             Show companion info                  │
│                                                          │
│  YOUR SKILLS                                             │
│  ─────────────────────────────────────────────────────   │
│  /review <focus>    Review code changes                  │
│  /commit            Generate commit message              │
│                                                          │
│  KEYBOARD SHORTCUTS                                       │
│  ─────────────────────────────────────────────────────   │
│  Enter              Submit message                       │
│  Shift+Enter        New line in input                    │
│  Ctrl+C             Cancel current operation             │
│  Ctrl+D             Exit PikoClaw                        │
│  ↑/↓               Scroll chat / navigate history        │
│  PgUp/PgDn         Scroll chat (larger jumps)            │
│  Ctrl+Home/End     Jump to top / bottom of chat          │
│  Tab                Navigate suggestions                 │
│  Esc                Dismiss dialog / dropdown            │
│                                                          │
╰──────────────────────────────────────────────────────────╯
```

### Layout rules

- **Width**: Full chat pane width
- **Border**: rounded, `theme.inactive`
- **Section headers**: `theme.text` bold + `─────` separator line in `theme.inactive`
- **Command name**: `theme.suggestion` (blue), left-aligned, width 20 chars
- **Description**: `theme.inactive`, remainder of line
- **Separator**: `─` in `theme.inactive` color
- **Column alignment**: commands padded to consistent width with spaces

---

## Shortcut Key Display Format

Throughout the TUI, keyboard shortcuts are shown consistently:

### Single key

```
Enter  ·  Esc  ·  Tab  ·  Space
```

### Modified key

```
Shift+Enter    Ctrl+C    Ctrl+D    Alt+F
```

Format: `Modifier+Key` with capital first letter, `+` separator, no spaces.

### Multiple alternatives

```
Ctrl+C or Ctrl+D
```

Shown in `theme.inactive` color. The key names themselves may be in `theme.suggestion` (blue) when highlighted.

### In-context hints (input bar top-right)

Small hints shown in the top-right of the input border:

```
╰──────────────────────────────────  Shift+Enter new line ╯
```

Format: `Key action` — key in `theme.suggestion`, action in `theme.inactive`.
Multiple hints separated by ` · `.

---

## Key Names Reference

Use these exact names consistently across help text:

| Key | Display name |
|-----|-------------|
| ↵ | `Enter` |
| ⇧↵ | `Shift+Enter` |
| ⌃C | `Ctrl+C` |
| ⌃D | `Ctrl+D` |
| ⌃U | `Ctrl+U` |
| ⌃R | `Ctrl+R` |
| ⌃Home | `Ctrl+Home` |
| ⌃End | `Ctrl+End` |
| ⇥ | `Tab` |
| ⇧⇥ | `Shift+Tab` |
| ⎋ | `Esc` |
| ↑↓←→ | `↑` `↓` `←` `→` |
| ⇞⇟ | `PgUp` `PgDn` |
| F1–F12 | `F1`–`F12` |

---

## Searchable Command List

When `/help commands` or `/help <query>` is called, filter commands:

```
╭─ Help: "commit" ─────────────────────────────────────────╮
│                                                          │
│  /commit              Generate AI commit message         │
│  /commit-push-pr      Commit, push, and create PR        │
│                                                          │
╰──────────────────────────────────────────────────────────╯
```

Matching: case-insensitive substring match on both command name and description.

---

## Keybindings Reference (`/keybindings`)

Shows current keybinding configuration:

```
╭─ Keybindings ────────────────────────────────────────────╮
│                                                          │
│  submit          Enter                                   │
│  newline         Shift+Enter                             │
│  cancel          Ctrl+C                                  │
│  exit            Ctrl+D                                  │
│  scroll_up       ↑ / PgUp                                │
│  scroll_down     ↓ / PgDn                                │
│  clear_input     Ctrl+U                                  │
│  history_prev    ↑ (empty input)                         │
│  history_next    ↓ (empty input)                         │
│                                                          │
│  * customized    + default                               │
│                                                          │
╰──────────────────────────────────────────────────────────╯
```

- Action name: `theme.text` left-aligned, 18 chars
- Binding: `theme.suggestion` (blue)
- `*` marker: indicates user-overridden binding (vs default)

---

## Context-sensitive Hints

Small inline hints shown at relevant moments, not in a panel:

```
  Press Tab to accept suggestion · Esc to dismiss
```

```
  ↑↓ to navigate · Enter to select · Esc to cancel
```

```
  y/n/a/d to respond to permission request
```

Format: plain `theme.inactive` text, shown for 5 seconds then hidden automatically.
