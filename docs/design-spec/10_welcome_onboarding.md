# Design Spec: Welcome Screen & Onboarding

**TS source**: `components/WelcomeV2.tsx`, `piko-tui/src/onboarding.rs`

---

## Welcome Header

Shown at the top of every new session (not on resume). Width: **58 chars**.

```
╭────────────────────────────────────────────────────────╮
│                                                        │
│         ██████                                         │
│      ███      ████████                                 │
│    ██████████████████████                              │
│       ████████████                                     │
│                         PikoClaw 0.3.9                 │
│                         claude-opus-4-6                │
│                         ~/projects/myapp               │
│                                                        │
│  ┌─ Tips ──────────────────┐  ┌─ Recent ─────────────┐│
│  │ /help  list commands    │  │ auth-refactor   2h ago││
│  │ /compact  free tokens   │  │ bug-hunt        1d ago││
│  │ Shift+Enter  new line   │  │ quick-question  3d ago││
│  └─────────────────────────┘  └──────────────────────┘│
│                                                        │
╰────────────────────────────────────────────────────────╯
```

### Logo

ASCII pixel-art Clawd. Uses Unicode block characters (`█ ▓ ░`).

```
     ██████
  ███      ████████
██████████████████████
   ████████████
```

Color: `theme.claude` (orange `rgb(215,119,87)`). Dark shadow/bg for contrast on light theme.

### Version + info block (right side)

```
PikoClaw 0.3.9         ← bold, theme.claude
claude-opus-4-6        ← theme.inactive
~/projects/myapp       ← theme.inactive
```

### Tips panel

Fixed set of 3 contextual tips, rotated each session (or fixed until dismissed):

```
/help         list commands
/compact      free up context
Shift+Enter   new line
@file         attach a file
/theme        change theme
Ctrl+C        cancel
```

Color: command `theme.suggestion` (blue), description `theme.inactive`.

### Recent sessions panel

Shows last 3 sessions from the session index:

```
auth-refactor    2h ago
bug-hunt         1d ago
quick-question   3d ago
```

Session name: `theme.text`. Age: `theme.inactive`.
If no prior sessions: show `No recent sessions` in `theme.inactive`.

---

## First-Run Theme Picker

On the very first launch (`onboarding_done = false` in config), a full-screen theme selection replaces the normal TUI.

```
┌──────────────────────────────────────────────────────────────┐
│                                                              │
│              Welcome to PikoClaw                             │
│                                                              │
│    Choose a color theme:                                     │
│                                                              │
│    ▶ dark              ████ ░░░░ ████                        │
│      light             ░░░░ ████ ░░░░                        │
│      dark-daltonized   ████ ░░░░ ████  (colorblind-safe)     │
│      light-daltonized  ░░░░ ████ ░░░░  (colorblind-safe)     │
│      dark-ansi         ████ ░░░░ ████  (16-color terminals)  │
│      light-ansi        ░░░░ ████ ░░░░  (16-color terminals)  │
│                                                              │
│    ↑↓ to select · Enter to confirm                          │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

### Color swatches

Each theme entry has a 4-character swatch showing its key colors:

```
Theme entry format:
  [selected marker]  [name]      [swatch]  [note]
  ▶                  dark        ████ ░░░░ ████
```

Swatch composition (left to right):
1. `████` — background color (2 blocks) + text color (2 blocks)
2. ` ` space
3. `░░░░` — `theme.inactive` color (4 blocks)
4. ` ` space
5. `████` — `theme.success` color (2) + `theme.error` color (2)

### Navigation

| Key | Action |
|-----|--------|
| `↑` / `↓` | Move selection |
| `Enter` | Confirm selection |
| No Esc | Forced — must pick a theme to continue |

### After selection

1. Save `theme` to `~/.config/pikoclaw/config.toml`
2. Set `onboarding_done = true`
3. Transition to normal TUI immediately (no restart needed)
4. Show brief toast: `✓ Theme set to dark`

---

## Live Preview

As the user navigates themes in the picker, the **swatch area and surrounding UI recolor in real time** to preview the selected theme. The picker itself rerenders with the selected theme applied.

This means the picker background, border, and text all change as you arrow through options — giving a true live preview.

---

## Session Resume Header

On `pikoclaw continue` or `pikoclaw resume <id>`, a smaller header is shown instead:

```
╭─ Resuming session ─────────────────────────────────────────╮
│  auth-refactor  ·  claude-opus-4-6  ·  ~/projects/myapp   │
│  Started 2 hours ago  ·  12 messages  ·  18.9k tokens used │
╰────────────────────────────────────────────────────────────╯
```

- **Border**: `theme.suggestion` (blue)
- **Session name**: bold `theme.text`
- **Meta info**: `theme.inactive`
- Dismisses after the first user message (replaced by normal chat flow)

---

## API Key Prompt (First Run, No Key)

If launched without an API key:

```
╭──────────────────────────────────────────────────────────╮
│  Welcome to PikoClaw                                     │
│                                                          │
│  An Anthropic API key is required.                       │
│                                                          │
│  Get one at: https://console.anthropic.com               │
│                                                          │
│  Enter API key: sk-ant-_
│                                                          │
│  Or set: export ANTHROPIC_API_KEY=sk-ant-...             │
╰──────────────────────────────────────────────────────────╯
```

- Input field masks characters after first 8 (`sk-ant-••••••••••••`)
- On valid key: saved to `~/.config/pikoclaw/config.toml`
- On invalid key (401 from API): show error and re-prompt
