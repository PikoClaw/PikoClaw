# Design Spec: Symbols & Glyphs

**TS source**: `constants/figures.ts`

Complete reference of all Unicode symbols used in the PikoClaw TUI.

---

## Status & Result Indicators

| Symbol | Unicode | Name | Used For |
|--------|---------|------|---------|
| `✓` | U+2713 | Check mark | Tool success, confirmed action |
| `✗` | U+2717 | Ballot X | Tool error, denied action |
| `⚠` | U+26A0 | Warning sign | Warning messages, dangerous ops |
| `ℹ` | U+2139 | Info source | Informational messages |
| `●` | U+25CF | Black circle | Status dot, filled |
| `⏺` | U+23FA | Black circle (macOS) | Status dot (macOS variant) |
| `◉` | U+25C9 | Bullseye | Max effort indicator |
| `◐` | U+25D0 | Half circle | Medium effort |
| `○` | U+25CB | Hollow circle | Low effort |

---

## Spinner Frames

### Braille spinner (tool execution)

```
⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏
```

Unicode: U+280B U+2819 U+2839 U+2838 U+283C U+2834 U+2826 U+2827 U+2807 U+280F

### Connection spinner

```
·|·  ·/·  ·─·  ·\·  →  ·✔·  or  ·✗·
```

---

## Separator & Structural

| Symbol | Unicode | Used For |
|--------|---------|---------|
| `·` | U+00B7 | Middle dot (list separator) |
| `∙` | U+2219 | Bullet operator (status bar separator) |
| `▎` | U+258E | Left one-quarter block (blockquote bar) |
| `━` | U+2501 | Heavy horizontal (banner separator) |
| `─` | U+2500 | Light horizontal (box drawing) |
| `│` | U+2502 | Light vertical (box drawing) |
| `╎` | U+254E | Light double dash vertical (thinking block border) |

---

## Arrows & Direction

| Symbol | Unicode | Used For |
|--------|---------|---------|
| `↑` | U+2191 | Input tokens (status bar) |
| `↓` | U+2193 | Output tokens (status bar) |
| `⚡` | U+26A1 | Cache read tokens (status bar) |
| `→` | U+2192 | Injected message indicator |
| `←` | U+2190 | Inbound channel message |
| `↻` | U+21BB | Refresh / resource update |
| `▶` | U+25B6 | Play, selected item indicator, thinking block header |
| `▶` | U+25B6 | Right-pointing solid triangle |

---

## Diff Symbols

| Symbol | Unicode | Used For |
|--------|---------|---------|
| `+` | U+002B | Added line prefix |
| `-` | U+002D | Removed line prefix |
| `@` | U+0040 | Hunk header `@@` |

---

## Mode & Feature Indicators

| Symbol | Unicode | Used For |
|--------|---------|---------|
| `⚑` | U+2691 | Issue flag banner |
| `⧈` | U+29C8 | Sandbox violation (square in circle) |
| `⑂` | U+2442 | Fork directive |
| `※` | U+203B | Away-summary recap marker |
| `◇` | U+25C7 | Running (open diamond) |
| `◆` | U+25C6 | Completed (filled diamond) |
| `★` | U+2605 | Rarity star (companion system) |
| `✨` | U+2728 | Sparkle (level-up, shiny) |
| `💤` | U+1F4A4 | Sleeping (idle companion) |
| `🎉` | U+1F389 | Party (major success) |

---

## Block Drawing (Boxes)

### Rounded box (dialogs, user messages)
```
╭─────╮
│     │
╰─────╯
```
`╭` U+256D · `─` U+2500 · `╮` U+256E · `│` U+2502 · `╰` U+2570 · `╯` U+256F

### Plain box (tool output)
```
┌─────┐
│     │
└─────┘
```
`┌` U+250C · `┐` U+2510 · `└` U+2514 · `┘` U+2518

### Double box (companion panel)
```
╔═════╗
║     ║
╚═════╝
```
`╔` U+2554 · `═` U+2550 · `╗` U+2557 · `║` U+2551 · `╚` U+255A · `╝` U+255D

### Heavy/bold box (warnings)
```
┏━━━━━┓
┃     ┃
┗━━━━━┛
```
`┏` U+250F · `━` U+2501 · `┓` U+2513 · `┃` U+2503 · `┗` U+2517 · `┛` U+251B

---

## Progress Bar Characters

| Symbol | Unicode | Used For |
|--------|---------|---------|
| `█` | U+2588 | Full block (filled progress) |
| `▓` | U+2593 | Dark shade (medium fill) |
| `░` | U+2591 | Light shade (empty progress) |

---

## Logo Characters (Clawd)

The Clawd logo uses block characters for pixel-art rendering:

| Symbol | Unicode | Shade |
|--------|---------|-------|
| `█` | U+2588 | Full / darkest |
| `▓` | U+2593 | Dark shade |
| `▒` | U+2592 | Medium shade |
| `░` | U+2591 | Light shade / lightest |

---

## Ratatui Rendering Notes

In ratatui, symbols are just string slices. Ensure:

1. Terminal font supports these Unicode code points (most modern terminals do)
2. Width: all symbols above are **1 cell wide** except emoji (which are 2 cells wide)
3. Emoji (`✨` `🎉` `💤` `📦`) are 2 cells wide — account for this in layout calculations
4. Fallback: if emoji aren't rendering correctly, use ASCII alternatives:
   - `✨` → `**`
   - `🎉` → `\o/`
   - `📦` → `[pkg]`

### Cell width detection

```rust
use unicode_width::UnicodeWidthChar;

pub fn cell_width(c: char) -> usize {
    c.width().unwrap_or(1)
}

// For strings:
pub fn display_width(s: &str) -> usize {
    s.chars().map(cell_width).sum()
}
```

Add `unicode-width = "0.1"` to `piko-tui` dependencies.
