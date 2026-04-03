# Spec: Session Persistence & Management

**Status**: 🔶 Partial — save/resume works; list/delete commands missing
**Rust crate**: `piko-session`
**TS source**: `utils/session.ts`, `history.ts`, `commands/session.ts`

---

## Overview

Sessions persist the full conversation history to disk so users can resume interrupted conversations. Each session is stored as a JSON file identified by UUID.

---

## What's Implemented

- [x] `Session` struct: `id` (UUIDv4), `created_at`, `updated_at`, `cwd`, `model`, `messages`, `name`
- [x] `SessionInfo` — metadata-only view (no messages), for listing
- [x] `FilesystemSessionStore` — saves/loads sessions as JSON in `~/.local/share/pikoclaw/sessions/`
- [x] `SessionStore` trait: `save()`, `load(id)`, `load_latest_for_cwd(cwd)`
- [x] Session index file for fast latest-by-cwd lookup
- [x] `continue` subcommand — resume latest session for current working directory
- [x] `resume <session_id>` subcommand — resume a specific session by ID
- [x] Auto-save after each turn
- [x] `updated_at` timestamp updated on each save (`touch()`)
- [x] `display_name()` — returns `name` if set, else truncated first user message

---

## Spec / Technical Details

### Storage Layout

```
~/.local/share/pikoclaw/sessions/
  index.json              ← { cwd → [session_id, updated_at][] }
  {uuid}.json             ← full Session struct
  ...
```

### Session JSON Schema

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": null,
  "created_at": "2025-01-15T10:30:00Z",
  "updated_at": "2025-01-15T11:45:00Z",
  "cwd": "/home/user/myproject",
  "model": "claude-opus-4-6",
  "messages": [
    { "role": "user", "content": "..." },
    { "role": "assistant", "content": [...] }
  ]
}
```

### `continue` vs `resume`

| Command | Behavior |
|---------|----------|
| `pikoclaw continue` (or `-c`) | Load most recent session for current `$PWD` |
| `pikoclaw resume <id>` (or `-r <id>`) | Load specific session by UUID prefix |

---

## Gaps / Todos

- [ ] **`/sessions` slash command** — list all sessions with name, date, cwd in TUI. See [29_session_commands.md](29_session_commands.md).
  - Show: session id (short), display_name, cwd, updated_at, message count.

- [ ] **`/delete` slash command** — delete a session by ID or by selection from list. See [29_session_commands.md](29_session_commands.md).

- [ ] **`/resume` from within TUI** — currently `resume` only works as a CLI subcommand before the TUI starts. Should be callable mid-session to switch to a different session.
  - Implementation: when `/resume <id>` is dispatched from TUI, load the session, replace `ConversationContext`, update display.

- [ ] **Session naming** — `/name <text>` or `/rename <text>` command to set a human-readable name on the current session. Persisted to `session.name`.

- [ ] **Session branching** — TS has `/branch` and `/fork` commands that duplicate a session at a point in history and start a new conversation from there. Not yet in Rust.

- [ ] **Cross-project resume** — TS tracks a global "last session" pointer so you can resume from any directory. Rust only tracks by cwd.

- [ ] **Session backfill / export** — TS can export session history as Markdown or JSON. Not yet in Rust.

- [ ] **Message pruning on resume** — TS trims tool-result/tool-use pairs when resuming to stay under context limit. Rust loads full history without trimming.
