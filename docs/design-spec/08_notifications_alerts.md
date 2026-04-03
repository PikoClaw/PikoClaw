# Design Spec: Notifications & Alerts

**TS source**: `hooks/notifs/`, `components/BannerMessages.tsx`

---

## Toast Notifications

Transient messages that appear at the top of the chat pane, auto-dismiss after 5 seconds.

```
╭─ ℹ  Session saved  ──────────────────────────────────────╮
╰──────────────────────────────────────────────────────────╯
```

### Visual

- **Position**: Top of chat pane area (below welcome header if present)
- **Height**: 1–2 lines
- **Width**: Full chat pane width
- **Border**: thin, rounded top only (`╭──╯`)
- **Auto-dismiss**: 5 seconds (`FOOTER_TEMPORARY_STATUS_TIMEOUT = 5000ms`)
- **Dismiss**: Press `Esc` to dismiss immediately

### Types and colors

| Type | Icon | Color |
|------|------|-------|
| Info | `ℹ` | `theme.suggestion` (blue) |
| Success | `✓` | `theme.success` (green) |
| Warning | `⚠` | `theme.warning` (amber) |
| Error | `✗` | `theme.error` (red) |

### Examples

```
╭─ ✓  Session renamed to "auth-refactor"  ─────────────────╮

╭─ ⚠  Context at 82% — consider running /compact  ─────────╮

╭─ ✗  API error: model overloaded, retrying...  ───────────╮

╭─ ✨  Ferris reached level 4!  ────────────────────────────╮
```

---

## Persistent Banners

Always-visible messages for ongoing states. Shown between the chat pane and input bar.

### Rate limit banner

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  ⚡ Rate limited (5-hour window). Waiting 2m 34s...
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

- **Color**: `theme.warning` (amber)
- **Separator**: `━` heavy horizontal line
- **Countdown**: updates every second

### Context nearly full banner

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  ⚠  Context window 90% full. Run /compact to continue.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

- **Color**: `theme.error` (red) at ≥ 90%
- **Color**: `theme.warning` (amber) at 80–89%

### Update available banner

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  📦  PikoClaw 0.4.0 available.  Run: pikoclaw update
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

- **Color**: `theme.suggestion` (blue)
- **Dismissible**: Press `Esc` or `d` to hide for 24 hours

---

## Error Overlays (Modal)

For unrecoverable or blocking errors, show a full modal.

```
╭──────────────────────────────────────────────────────────╮
│  ✗  Authentication Error                                 │
│                                                          │
│  Your API key is invalid or expired.                     │
│  Set a valid key with:                                   │
│                                                          │
│    export ANTHROPIC_API_KEY=sk-ant-...                   │
│                                                          │
│  Or run: pikoclaw login                                  │
│                                                          │
│              [ Dismiss (Esc) ]                           │
╰──────────────────────────────────────────────────────────╯
```

- **Position**: Centered, 60–80% terminal width
- **Background**: `theme.user_message_bg` (slightly tinted)
- **Border**: rounded, `theme.error` (red)
- **Header**: `theme.error`, bold, with `✗` icon
- **Body**: `theme.text`
- **Code snippet**: `theme.suggestion` (blue) background
- **Button**: centered at bottom

### Error overlay types

| Error | Border | Header |
|-------|--------|--------|
| Auth failure (401) | red | `Authentication Error` |
| Rate limit (429) | amber | `Rate Limit Reached` |
| Context overflow | amber | `Context Window Full` |
| Network error | red | `Connection Error` |
| Invalid config | amber | `Configuration Error` |

---

## Inline Status Messages

Not a separate widget — rendered inline in the chat pane as system messages.

```
⚡ Auto-compacting context (85% full)...
✓ Context compacted. 43,390 tokens freed.
```

```
⏳ Waiting for rate limit... 1m 47s remaining
✓ Rate limit cleared. Resuming...
```

```
⚠ Tool bash timed out after 120s. Continuing...
```

- **Position**: Inline between messages, full width
- **No border, no background**
- Colors per message type (see Toast table above)

---

## Level-Up Notification (Buddy)

A special celebratory toast shown when the companion levels up:

```
╭─ ✨  Ferris reached level 5! Hat unlocked: Crown  ────────╮
╰──────────────────────────────────────────────────────────╯
```

- **Border color**: `theme.claude` (orange)
- **Icon**: `✨`
- **Auto-dismiss**: 8 seconds (slightly longer than normal toasts)
- **Companion name**: bold

---

## Notification Queue

If multiple notifications arrive simultaneously, they stack (max 3 visible):

```
╭─ ✓  File saved  ────────────────────────────────────────╮
╭─ ⚠  Context at 80%  ───────────────────────────────────╮
╭─ ✓  Session auto-saved  ───────────────────────────────╮
```

Oldest at top, newest at bottom. Each dismisses independently.

---

## Rust Implementation Notes

```rust
pub struct Notification {
    pub kind: NotificationKind,
    pub message: String,
    pub created_at: Instant,
    pub duration_ms: u64,    // default 5000
    pub dismissible: bool,
}

pub enum NotificationKind {
    Info, Success, Warning, Error, LevelUp,
}

pub struct NotificationQueue {
    pub items: VecDeque<Notification>,
    pub max_visible: usize,  // 3
}

impl NotificationQueue {
    pub fn push(&mut self, n: Notification) {
        if self.items.len() >= self.max_visible {
            self.items.pop_front();  // drop oldest
        }
        self.items.push_back(n);
    }

    pub fn tick(&mut self, now: Instant) {
        self.items.retain(|n| {
            now.duration_since(n.created_at).as_millis() < n.duration_ms as u128
        });
    }
}
```
