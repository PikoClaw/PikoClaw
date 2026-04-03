# Design Spec: Message Rendering

**TS source**: `components/messages/`, `components/FileEditToolDiff.tsx`

---

## Message Flow Layout

Messages are rendered top-to-bottom in the chat pane. Each exchange is:

```
[user message block]
                                               ← 1 blank line
[assistant text]
[tool use block 1]
[tool result 1]
[tool use block 2]
[tool result 2]
[assistant text continuation]
                                               ← 1 blank line
[user message block]
...
```

---

## User Message

```
╭──────────────────────────────────────────────────────────╮
│ Fix the authentication bug in src/auth/jwt.rs            │
╰──────────────────────────────────────────────────────────╯
```

- **Background**: `theme.user_message_bg` — `rgb(240,240,240)` light / `rgb(40,40,40)` dark
- **Border**: rounded, all sides, `theme.inactive` color
- **Padding**: 1 space left/right
- **Text color**: `theme.text`
- **Max displayed chars**: 10,000 (truncate with `[... N chars omitted ...]` if longer)
- **Right padding**: 1 space after text (inside border)

### With image attachment

```
╭──────────────────────────────────────────────────────────╮
│ What's wrong with this screenshot? [Image #1]            │
╰──────────────────────────────────────────────────────────╯
```

Image chips shown inline as `[Image #1]` in default text color. Not expandable in TUI (image data is sent to API but not rendered inline).

---

## Assistant Text

No background, no border. Renders directly in the chat pane.

```
The issue is in the `validate_token` function on line 42.
The expiry check uses `<` instead of `<=`, which means tokens
expire one second early.
```

- **Background**: transparent
- **Text color**: `theme.text`
- **Margin-top**: 0 (spacing from prior block handled by blank line)
- **Wraps**: at terminal width

### Inline code

Backtick-wrapped code: `` `validate_token` `` — rendered with `theme.suggestion` blue color.

### Code blocks

`````
```rust
fn validate_token(token: &str) -> Result<Claims> {
    let claims = decode(token)?;
    if claims.exp <= Utc::now().timestamp() {  // ← fixed
        return Err(Error::Expired);
    }
    Ok(claims)
}
```
`````

- **Syntax highlighting**: via `syntect`, language detected from fence label
- **Background**: slightly darker than surrounding text (subtle, not boxed)
- **Padding**: 1 space left/right
- **No border**
- **No line numbers** (unless explicitly in content)
- **Falls back** to plain monospace if language not recognized

---

## Tool Use Block

### While running (in-progress)

```
⠙ bash · git diff src/auth/jwt.rs                         2.3s
```

- **Spinner**: braille pattern `⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏` rotating at ~100ms
- **Spinner color**: `theme.spinner` + shimmer pulse between `theme.spinner` and `theme.spinner_shimmer`
- **Tool name**: `theme.suggestion` (blue), bold
- **·** separator: `theme.inactive`
- **Input preview**: `theme.inactive` (dimmed) — truncated to ~60 chars
- **Elapsed time**: right-aligned, `theme.inactive`

### Completed — success

```
✓ bash · git diff src/auth/jwt.rs                         1.2s
```

- **✓**: `theme.success` (green)
- Rest: `theme.inactive` (dimmed, since it's done)

### Completed — error

```
✗ bash · npm test                                         5.0s
  Error: 3 tests failed
  FAIL src/auth.test.ts
    ✕ validates token expiry (42ms)
```

- **✗**: `theme.error` (red)
- **Error text**: `theme.error`, indented 2 spaces
- Error output shown inline (not collapsed)

### Tool input display (expandable preview)

For tools with structured inputs, show the key field:

| Tool | Preview field |
|------|--------------|
| `bash` | `command` value |
| `file_read` | `file_path` |
| `file_write` | `file_path` |
| `file_edit` | `file_path` |
| `glob` | `pattern` |
| `grep` | `pattern` in `path` |
| `web_fetch` | URL |
| `web_search` | `query` |
| `agent` | `description` or first 60 chars of `prompt` |

---

## File Edit Diff Block

After a `file_edit` tool call succeeds, the diff is shown inline:

```
  ✓ file_edit · src/auth/jwt.rs

  @@ -40,7 +40,7 @@
   fn validate_token(token: &str) -> Result<Claims> {
       let claims = decode(token)?;
  -    if claims.exp < Utc::now().timestamp() {
  +    if claims.exp <= Utc::now().timestamp() {
           return Err(Error::Expired);
       }
```

### Diff line colors

| Line type | Prefix | Background | Text color |
|-----------|--------|------------|------------|
| Added | `+` | `theme.diff_added` | `theme.diff_added_word` |
| Removed | `-` | `theme.diff_removed` | `theme.diff_removed_word` |
| Context | ` ` | transparent | `theme.inactive` |
| Hunk header `@@` | `@` | transparent | `theme.suggestion` (blue) |
| File header | `---`/`+++` | transparent | `theme.inactive` |

### Word-level highlighting

Within added/removed lines, the specific changed words get a darker shade:

- Added word: `theme.diff_added_word` background (darker green)
- Removed word: `theme.diff_removed_word` background (darker red)

### Gutter

Line numbers shown in `theme.inactive` to the left of the diff sigil. The gutter is **non-selectable** (excluded from text copy).

### Collapsed large diffs

If diff > 50 lines: show first 20 and last 5 lines with a collapse indicator:

```
  ... 28 lines hidden (press space to expand) ...
```

---

## Thinking Block (Extended Thinking)

```
▶ Thinking  (1,234 tokens)
  ╎ Let me analyze the JWT structure...
  ╎ The issue is likely in the expiry comparison...
  ╎ I should check both the encode and decode paths...
```

- **Header**: `▶ Thinking` + token count — `theme.inactive`, collapsible
- **Body**: indented 2 spaces, with `╎` left border in `theme.inactive`
- **Collapsed by default** when > 5 lines
- **Expand**: press Space on header row
- **Color**: all `theme.inactive` / `theme.subtle` — clearly subordinate to main response

---

## Sub-agent Message

When a sub-agent is running and streaming output:

```
┌ agent: Exploring codebase... ─────────────────────────────┐
│ ⠙ grep · "authenticate" in src/                           │
│ Found 12 matches across 4 files                           │
└───────────────────────────────────────────────────────────┘
```

- **Border**: plain (`┌─┐ └─┘`), color = agent's team color (or `theme.suggestion` for solo)
- **Header**: agent description, `theme.inactive`
- **Content**: same tool use rendering as main agent

---

## Error / System Messages

```
⚠ Rate limit reached. Waiting 45s before retrying...
```

- **Icon**: `⚠` in `theme.warning`
- **Text**: `theme.warning`
- **No background, no border**

```
✗ API error: Request too large (tokens: 205,000 / limit: 200,000)
```

- **Icon**: `✗` in `theme.error`
- **Text**: `theme.error`

---

## Compact Summary Message

After `/compact` runs, a summary message is inserted:

```
╭──────────────────────────────────────────────────────────╮
│ [Conversation compacted]                                 │
│ We analyzed the auth module. Fixed JWT expiry bug in     │
│ src/auth/jwt.rs. Tests now passing.                      │
│                          45,230 → 1,840 tokens freed     │
╰──────────────────────────────────────────────────────────╯
```

- **Border**: rounded, `theme.inactive`
- **Header text**: `theme.inactive`, italic-ish
- **Summary text**: `theme.text`
- **Token count**: right-aligned, `theme.success` (green)
