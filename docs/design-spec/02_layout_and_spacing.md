# Design Spec: Layout & Spacing

**TS source**: `ink/`, `components/App.tsx`

---

## Overall TUI Layout

```
┌─────────────────────────────────────────────────────────────┐
│  Welcome header (first launch only)                         │
│                                                             │
│  Chat pane (scrollable)                                     │
│    [user message]                                           │
│    [assistant message]                                      │
│    [tool use block]                                         │
│    [tool result]                                            │
│    ...                                                      │
│                                                             │
│  Status bar  model · cwd · tokens · cost                    │
│  Input bar                                                  │
│  > _                                                        │
│  Slash suggestions / dialogs                                │
└─────────────────────────────────────────────────────────────┘
```

With companion (when buddy enabled):

```
┌──────────────────────────────────────┐  ╔════════╗
│  Chat pane                           │  ║ {E}{E} ║
│                                      │  ║ (  )   ║
│                                      │  ╚════════╝
├──────────────────────────────────────┤   Ferris
│  Input bar                           │   Lv.3 ★★
│  > _                                 │
├──────────────────────────────────────┤
│  Status bar                          │
└──────────────────────────────────────┘
```

---

## Flex Layout Model

PikoClaw uses ratatui's constraint-based layout, which maps to the Ink/Yoga flex model used in TS:

| Ink concept | ratatui equivalent |
|-------------|-------------------|
| `flexDirection: column` | `Layout::vertical()` |
| `flexDirection: row` | `Layout::horizontal()` |
| `flexGrow: 1` | `Constraint::Min(0)` or `Constraint::Fill(1)` |
| `width: 100%` | `Constraint::Percentage(100)` |
| fixed width | `Constraint::Length(n)` |
| `alignItems: flex-start` | default (top/left aligned) |

---

## Spacing Rules

### Between message groups
- **1 blank line** between each user↔assistant exchange
- No blank line between a tool-use block and its result (they belong together)
- **1 blank line** after a tool result before the next assistant text

### Footer stack
- Status bar is rendered above the input area
- Input bar is rendered below the status bar
- Slash suggestion list is rendered below the input bar
- Provider/API-key dialogs overlay the footer stack without changing message-pane layout

### Inside message blocks
- User message: **1 space** right padding inside its background box
- Tool use header: **no indent**
- Tool output: **2 spaces** left indent
- Diff hunks: **no indent** (full width)

### Boxes / containers
- Default padding: `1 space left/right`, `0 top/bottom`
- Exception: permission dialogs and info cards: `1 space all sides`

---

## Border Styles

```rust
// ratatui border styles used:
Borders::NONE
Borders::ALL         // full box
Borders::BOTTOM      // underline only (historical design; current Rust input uses full rounded box)
Borders::TOP         // overline only
```

Border character sets:
- **Rounded**: `╭─╮ │ ╰─╯` — used for dialogs, info cards, permission prompts
- **Plain**: `┌─┐ │ └─┘` — used for tool output boxes
- **Double**: `╔═╗ ║ ╚═╝` — used for companion panel (optional)
- **Thick/Bold**: `┏━┓ ┃ ┗━┛` — used for emphasis/warnings

---

## Text Wrapping

| Mode | Behavior | When to use |
|------|----------|-------------|
| `Wrap` | Wrap at word boundary | Default for all message content |
| `TruncateEnd` | Cut with `…` | Status bar fields, long paths |
| `NoWrap` | No wrapping | Code blocks (horizontal scroll) |

Max display characters for a single message: **10,000** (truncate head + tail beyond this for performance).

---

## Column Width Constraints

| Element | Width |
|---------|-------|
| Welcome header | 58 chars fixed |
| Companion panel | 14 chars wide |
| Permission dialog | min 40, max 80 chars |
| Suggestion dropdown | full input width |
| Diff block | full terminal width |
| Status bar | full terminal width |
| Info card (`/buddy`) | 41 chars |

---

## Z-order (Overlay Layers)

Rendered bottom-to-top:

```
1. Chat pane background
2. Chat messages
3. Input bar
4. Status bar
5. Suggestion dropdown
6. Permission dialog (modal, overlaps everything)
7. Toast notifications (top layer)
```

In ratatui: use `Frame::render_widget` in order, with modals rendered last into a `Rect` that overlaps the main content.

---

## Resize Handling

On `Event::Resize(w, h)`:
1. Recalculate all layout `Rect` values
2. Re-render entire frame
3. Input bar text re-wraps to new width
4. Chat pane scroll position preserved (messages don't shift, only viewport changes)
5. If terminal becomes too small (< 40 wide or < 10 tall): show warning `Terminal too small`
