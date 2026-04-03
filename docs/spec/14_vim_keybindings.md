# Spec: Vim Mode & Keybinding Customization

**Status**: ❌ Todo
**TS source**: `vim/`, `keybindings/`

---

## Overview

Two related features:
1. **Vim mode** — modal editing in the input bar (Normal/Insert/Visual modes with standard vim motions)
2. **Keybinding customization** — user-overridable keybindings for all TUI actions via config file

---

## Part 1: Vim Mode

### Modes

```
Insert mode  → default mode on startup
Normal mode  → Esc to enter; navigate/edit with motions
Visual mode  → v in Normal to select text
```

### Normal Mode Commands

| Key | Action |
|-----|--------|
| `h/j/k/l` | Move cursor left/down/up/right |
| `w/W` | Move to next word/WORD start |
| `b/B` | Move to prev word/WORD start |
| `e/E` | Move to end of word/WORD |
| `0/$` | Move to start/end of line |
| `^` | Move to first non-blank |
| `gg/G` | Move to start/end of buffer |
| `i/a` | Enter insert mode at/after cursor |
| `I/A` | Enter insert at start/end of line |
| `o/O` | Open new line below/above |
| `x` | Delete char under cursor |
| `dd` | Delete current line |
| `dw/db/de` | Delete word forward/back/to end |
| `D` | Delete to end of line |
| `cc/cw/C` | Change line/word/to end |
| `yy/yw` | Yank line/word |
| `p/P` | Paste after/before |
| `u` | Undo |
| `Ctrl+r` | Redo |
| `/` | Search in output |
| `n/N` | Next/prev search match |
| `Esc` | Return to Normal from Insert |
| `Enter` | Submit (in Insert mode) |

### Implementation Plan

#### Data Structures

```rust
pub enum VimMode {
    Insert,
    Normal,
    Visual { anchor: usize },
}

pub struct VimState {
    pub mode: VimMode,
    pub cursor: usize,         // byte offset in input buffer
    pub register: String,      // yank/delete register (single unnamed register)
    pub pending_count: Option<u32>,  // numeric prefix (e.g. 3 in "3dw")
    pub pending_operator: Option<char>,  // operator waiting for motion
}
```

#### Integration into `piko-tui`

- In `events.rs`, before passing keyboard event to input handler, check `VimState.mode`
- In Normal mode: handle vim keys, update `VimState`, do NOT pass keys to regular input handler
- In Insert mode: pass keys to existing input handler as normal
- Status bar: show `-- NORMAL --` / `-- INSERT --` / `-- VISUAL --` indicator
- Escape in Insert → Normal; `i/a/etc` in Normal → Insert

#### Config

```toml
[tui]
vim_mode = false   # default off
```

CLI flag: `--vim` / `/vim` slash command to toggle

---

## Part 2: Keybinding Customization

### What Can Be Rebound

All TUI actions should be rebindable:

| Action | Default Key | Description |
|--------|-------------|-------------|
| `submit` | `Enter` | Submit current input |
| `newline` | `Shift+Enter` | Insert newline in input |
| `scroll_up` | `↑` / `PgUp` | Scroll chat up |
| `scroll_down` | `↓` / `PgDn` | Scroll chat down |
| `scroll_top` | `Ctrl+Home` | Scroll to top |
| `scroll_bottom` | `Ctrl+End` | Scroll to bottom |
| `cancel` | `Ctrl+C` | Cancel current operation |
| `exit` | `Ctrl+D` | Exit PikoClaw |
| `clear_input` | `Ctrl+U` | Clear input bar |
| `history_prev` | `↑` (empty input) | Previous input |
| `history_next` | `↓` (empty input) | Next input |

### Config Format

```toml
[keybindings]
submit = "enter"
newline = "shift+enter"
scroll_up = "ctrl+up"
cancel = "ctrl+c"
exit = "ctrl+d"
```

### Key Syntax

```
"enter"
"shift+enter"
"ctrl+c"
"alt+f"
"f5"
"ctrl+shift+k"
```

### Implementation Plan

```rust
// In piko-config
pub struct KeybindingsConfig {
    pub submit: Option<KeyCombo>,
    pub newline: Option<KeyCombo>,
    pub scroll_up: Option<KeyCombo>,
    // ...
}

pub struct KeyCombo {
    pub key: crossterm::event::KeyCode,
    pub modifiers: crossterm::event::KeyModifiers,
}
```

In `events.rs`: before hardcoded key matching, check user bindings first:

```rust
fn match_key(event: KeyEvent, config: &KeybindingsConfig) -> Option<TuiAction> {
    // check custom bindings first
    if matches_combo(event, &config.submit) { return Some(TuiAction::Submit); }
    // ... fall through to defaults
}
```

### Load from File

Also support loading from `~/.config/pikoclaw/keybindings.toml` as separate file (keeps main config cleaner).
