# Design Spec: Permission Dialogs

**TS source**: `components/permissions/`, `components/PermissionRequest.tsx`

---

## Standard Permission Dialog

Shown when a tool call requires user approval.

```
╭──────────────────────────────────────────────────────────╮
│  PikoClaw wants to run a command                         │
│                                                          │
│  bash                                                    │
│  rm -rf ./node_modules && npm install                    │
│                                                          │
│  [ Yes (y) ]  [ No (n) ]  [ Always (a) ]  [ Never (d) ] │
╰──────────────────────────────────────────────────────────╯
```

### Layout details

- **Border**: rounded, color `theme.permission` (blue)
- **Width**: min 40, max 80 chars (capped to terminal width)
- **Position**: above the input bar, replacing the bottom section
- **Header line**: `theme.inactive` dimmed text
- **Tool name**: `theme.text` bold
- **Input preview**: see section below
- **Button row**: horizontal, separated by 2 spaces

### Button design

```
[ Yes (y) ]   [ No (n) ]   [ Always (a) ]   [ Never (d) ]
```

| Button | Key | Color | Behavior |
|--------|-----|-------|----------|
| Yes | `y` | `theme.permission` (blue) | Allow once |
| No | `n` | `theme.error` (red) | Deny once |
| Always | `a` | `theme.success` (green) | Allow for all future calls this session |
| Never | `d` | `theme.error` dim | Deny all future calls this session |

**Focused button**: inverted colors (white text on colored bg).
Initial focus: `Yes` button.

Navigation: `←/→` or `Tab` to move between buttons, `Enter` to confirm focused.
Direct hotkey: press `y`/`n`/`a`/`d` immediately (no need to navigate to button).

---

## Tool Input Preview

Below the tool name, show the most relevant input field:

### bash tool

```
  bash
  rm -rf ./node_modules && npm install
```

Color rules for bash commands:

| Pattern | Color |
|---------|-------|
| Contains `rm`, `rmdir`, `del`, `unlink` | `theme.error` (red) |
| Contains `sudo`, `su`, `chmod 777` | `theme.warning` (amber) |
| Contains `curl \| bash`, `eval`, `exec` | `theme.warning` (amber) |
| Safe read-only: `ls`, `cat`, `grep`, `git status` | `theme.inactive` (dim) |
| Default | `theme.text` (normal) |

### file_write / file_edit tool

```
  file_edit
  src/auth/jwt.rs
  replacing: "claims.exp < " → "claims.exp <= "
```

Show: `file_path` + brief description of change.

### file_read tool

```
  file_read
  /etc/passwd
```

Color path red if it's a system path (`/etc/`, `/sys/`, `/proc/`, `~/.ssh/`, etc.).

### web_fetch / web_search tool

```
  web_fetch
  https://api.internal.example.com/admin/users
```

### agent tool

```
  agent
  "Analyze the authentication system and suggest improvements"
```

Show truncated prompt (first 80 chars).

---

## Dangerous Operation Emphasis

When the operation is considered dangerous, add a warning header:

```
╭──────────────────────────────────────────────────────────╮
│  ⚠  This command may delete files                        │
│                                                          │
│  bash                                                    │
│  rm -rf ./build ./dist                                   │
│                                                          │
│  [ Yes (y) ]  [ No (n) ]  [ Always (a) ]  [ Never (d) ] │
╰──────────────────────────────────────────────────────────╯
```

- **Warning icon** `⚠`: `theme.warning` (amber)
- **Warning text**: `theme.warning`
- **Border color**: `theme.warning` instead of `theme.permission`

Dangerous patterns:
- `rm -rf` / `del /s /q` — file deletion
- Writing to `/etc/`, `/System/`, `C:\Windows\` — system paths
- `DROP TABLE`, `DELETE FROM` — database destruction
- `git push --force` to main/master
- Commands reading sensitive files: `~/.ssh/id_rsa`, `~/.aws/credentials`

---

## Question Dialog (AskUserQuestion tool)

When the agent calls `AskUserQuestionTool`:

```
╭──────────────────────────────────────────────────────────╮
│  Which approach should I use?                            │
│                                                          │
│  1. Refactor the existing JWT implementation             │
│  2. Replace with a third-party library (jsonwebtoken)    │
│  3. Use Anthropic's built-in auth (if available)         │
│                                                          │
│  Enter number or type your answer: _                     │
╰──────────────────────────────────────────────────────────╯
```

- **Border**: rounded, `theme.permission` (blue)
- **Question**: `theme.text`, first line bold
- **Options**: numbered list, `theme.text`
- **Input line**: standard text input at bottom of dialog
- **Navigation**: type `1`/`2`/`3` to select, or type free text, then Enter

If no options provided (free-form question):

```
╭──────────────────────────────────────────────────────────╮
│  What's your preferred test framework?                   │
│                                                          │
│  > _                                                     │
╰──────────────────────────────────────────────────────────╯
```

---

## Plan Mode Exit Dialog

When agent calls `ExitPlanModeTool`:

```
╭──────────────────────────────────────────────────────────╮
│  Agent wants to exit plan mode and make changes          │
│                                                          │
│  The agent will now be able to run commands and          │
│  modify files.                                           │
│                                                          │
│  [ Allow (y) ]          [ Keep planning (n) ]            │
╰──────────────────────────────────────────────────────────╯
```

- **Border**: `theme.warning` (amber) — signals a mode change
- **Allow**: `theme.success` (green)
- **Keep planning**: `theme.inactive`

---

## Keyboard Shortcuts Summary

| Key | In permission dialog | In question dialog |
|-----|---------------------|-------------------|
| `y` | Allow once | — |
| `n` | Deny once | — |
| `a` | Always allow | — |
| `d` | Always deny | — |
| `←` / `→` | Navigate buttons | — |
| `Tab` | Navigate buttons | — |
| `Enter` | Confirm focused | Submit answer |
| `Esc` | Deny (same as `n`) | Cancel |
| `1`–`9` | — | Select numbered option |
