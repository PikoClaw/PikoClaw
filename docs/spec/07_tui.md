# Spec: Terminal User Interface (TUI)

**Status**: ✅ Done (core); 🔶 Missing (vim mode, output styles, history search)
**Rust crate**: `piko-tui`
**TS source**: `ink/`, `components/`, `hooks/`

---

## Overview

The TUI is built with `ratatui` + `crossterm`. It provides an interactive chat interface with a chat pane, input bar, status bar, permission dialogs, and question dialogs.

---

## What's Implemented

### App State Machine ✅
- [x] `AppState` enum: `Running`, `WaitingForAgent`, `AskingPermission`, `AskingQuestion`, `Exiting`
- [x] Event-driven loop: keyboard events + agent events processed in single `select!` loop
- [x] Graceful exit on `Ctrl+C` / `Ctrl+D` (sends cancellation to agent, then exits)

### Chat Pane ✅
- [x] Scrollable message history
- [x] User messages, assistant text, tool start/result display
- [x] Syntax-highlighted code blocks (via `syntect`, 190+ languages, TextMate grammars)
- [x] Markdown-aware rendering (bold, italic, inline code, headers, lists)
- [x] Scroll with `↑/↓` or `PgUp/PgDn`
- [x] Auto-scroll to bottom on new messages

### Input Bar ✅
- [x] Multi-line input (Shift+Enter for newline)
- [x] Enter to submit
- [x] Basic cursor movement (left/right/home/end)
- [x] Backspace/delete
- [x] Paste support (clipboard paste inserts at cursor)

### Status Bar ✅
- [x] Model name display
- [x] Current working directory display
- [x] Token usage: cumulative input / output / cache_creation / cache_read
- [x] Active theme name
- [x] Rate limit warning when rate limit is hit

### Permission Dialog ✅
- [x] Inline dialog appears when `PermissionAsk` event received
- [x] Shows tool name and relevant input preview
- [x] Keybindings: `y`/`n`/`a`/`d`
- [x] Blocks agent until user responds
- [x] Returns decision via channel back to permission checker

### Question Dialog ✅
- [x] Inline numbered option list for `AskUserQuestion` tool
- [x] Type number to select, or free-text
- [x] Returns answer via channel back to tool

### Themes ✅
- [x] 6 built-in themes: `dark`, `light`, `dark-daltonized`, `light-daltonized`, `dark-ansi`, `light-ansi`
- [x] `/theme [name]` command to switch mid-session
- [x] Active theme shown in status bar
- [x] Full-frame background fill (no terminal default bleed)
- [x] Theme persisted to `~/.config/pikoclaw/config.toml`

### First-Run Onboarding ✅
- [x] Full-screen theme picker on first launch
- [x] Live preview of theme with color swatches
- [x] Navigate with arrow keys, confirm with Enter
- [x] Sets `onboarding_done: true` in config after completion

### Welcome Header ✅
- [x] Versioned border
- [x] Pixel-art Clawd logo
- [x] Model name and cwd info
- [x] Tips panel
- [x] Recent activity panel (last N sessions from index)

### Syntax Highlighting ✅
- [x] `syntect` with embedded TextMate grammars
- [x] Detects language from fenced code block (```rust, ```python, etc.)
- [x] Falls back to plain text if language not detected
- [x] Theme-aware colors

---

## Gaps / Todos

- [ ] **Vim keybinding mode** — `/vim` command and `vim_mode: true` config. Full modal editing in input bar.
  See [14_vim_keybindings.md](14_vim_keybindings.md)

- [ ] **Input history navigation** — `↑/↓` in empty input to cycle previous prompts (like shell history).
  - Implementation: maintain `Vec<String>` of past inputs, navigate with arrows when cursor at top/bottom.

- [ ] **History search** — Ctrl+R to search previous messages/inputs.

- [ ] **Image rendering** — Display images inline in TUI using sixel/kitty graphics protocol or reference paths.
  See [15_image_input.md](15_image_input.md)

- [ ] **Configurable output styles** — `--output-style` flag with styles like `minimal`, `verbose`, `json`.
  See [26_output_styles.md](26_output_styles.md)

- [ ] **`/compact` visual feedback** — show summary text in TUI after compaction, not just a silent clear.

- [ ] **Todo list sidebar** — when TodoWrite tool is used, show task checklist in a sidebar or inline panel.

- [ ] **Agent progress indicator** — show spinner/ellipsis while agent is thinking between tool calls.

- [ ] **Tool result collapsing** — long tool outputs (e.g. bash stdout) should be collapsible/expandable in TUI.

- [ ] **Search in output** — Ctrl+F to search through displayed messages.

- [ ] **Copy to clipboard** — select text in output, copy with Ctrl+C (without interrupting session).

- [ ] **Resize handling** — currently TUI may not reflow properly on terminal resize. Needs `crossterm::event::Event::Resize` handling.
