# Spec: Permission System

**Status**: ✅ Done (core); 🔶 Missing (classifier auto-approval, pattern matching depth)
**Rust crate**: `piko-permissions`
**TS source**: `utils/permissions.ts`, `components/permissions/`

---

## Overview

Every tool call goes through the permission system before execution. The system decides whether to allow, deny, or ask the user — based on config rules and per-session memory.

---

## What's Implemented

- [x] `PermissionChecker` trait: `async fn check(request: PermissionRequest) -> PermissionDecision`
- [x] `PermissionRequest` struct: `tool_name`, `description`, `input` (JSON)
- [x] `PermissionDecision` enum: `Allow`, `AllowAlways`, `Deny`, `DenyAlways`
- [x] `DefaultPermissionChecker` — reads config rules, falls back to `Ask`
- [x] `PermissionPolicy` — evaluates `PermissionRule` list from config
- [x] `PermissionRule` struct: `tool_name`, `pattern` (glob), `mode` (Allow/Deny/Ask)
- [x] `PermissionMode` enum in config: `Allow`, `Deny`, `Ask`
- [x] TUI permission dialogs: `y` (allow once), `n` (deny), `a` (always allow), `d` (always deny)
- [x] `TuiPermissionChecker` — sends `PermissionAsk` events to TUI, waits for user response
- [x] `--dangerously-skip-permissions` flag — bypasses all checks
- [x] Per-session "always allow" accumulation (remembers `AllowAlways` decisions for session lifetime)

---

## Permission Rule Evaluation Logic

```
1. Load rules from config (in order)
2. For each rule: check tool_name matches (exact or glob)
3. If rule has pattern: check input JSON contains matching substring/glob
4. First matching rule wins
5. If no rule matches: use global default_mode (default: Ask)
```

### Config Example (TOML)

```toml
[permissions]
default_mode = "ask"

[[permissions.rules]]
tool_name = "bash"
mode = "ask"

[[permissions.rules]]
tool_name = "bash"
pattern = "rm -rf"
mode = "deny"

[[permissions.rules]]
tool_name = "file_read"
mode = "allow"
```

---

## TUI Permission Dialog

When `Ask` decision is reached, TUI shows:

```
Tool: bash
Command: git status

Allow? [y]es / [n]o / [a]lways / [d]eny always
```

- `y` → `Allow` (once)
- `n` → `Deny` (once, tool returns error result)
- `a` → `AllowAlways` (remembered for session, not persisted)
- `d` → `DenyAlways` (remembered for session)

---

## Gaps / Todos

- [ ] **Persist "always allow" to config** — TS writes `AllowAlways` decisions back to `~/.claude/settings.json` so they persist across sessions. Rust only keeps them in memory for the current session.
  - Implementation: after `AllowAlways` decision, append new `PermissionRule` to config file.

- [ ] **Pattern matching on command arguments** — current Rust implementation matches `pattern` as a simple substring on the JSON-serialized input. TS uses glob patterns with structured extraction per tool type (e.g. extracts `command` field from bash input specifically).
  - Implementation: per-tool pattern extractors that pull the relevant field before matching.

- [ ] **Path-based file rules** — allow/deny specific file paths or directories for file_read/file_write/file_edit:
  ```toml
  [[permissions.rules]]
  tool_name = "file_write"
  pattern = "/etc/*"
  mode = "deny"

  [[permissions.rules]]
  tool_name = "file_read"
  pattern = "~/.ssh/*"
  mode = "deny"
  ```
  Pattern is matched against the `file_path` field of the tool input (not full JSON).

- [ ] **MCP tool permissions** — MCP tools (namespaced as `{server}__{tool}`) should be individually permissionable:
  ```toml
  [[permissions.rules]]
  tool_name = "filesystem__write_file"
  mode = "ask"
  ```

- [ ] **Per-tool override in rule** — rule can set `default_mode` for a tool without a pattern (current behavior), but also needs to fully override just for specific patterns while still asking for the rest.

- [ ] **`/permissions` slash command** — show and edit permission rules from within TUI. See [08_slash_commands.md](08_slash_commands.md).
  - Display: show all config rules + session-accumulated always-allow/deny
  - Add rule: `/permissions allow bash "git *"` — add allow rule for bash with pattern
  - Remove rule: `/permissions remove <index>`

- [ ] **Sandbox mode** — TS has a `--sandbox` flag that restricts bash to a Docker container or similar. Not yet in Rust.

- [ ] **Classifier auto-approval** (`TRANSCRIPT_CLASSIFIER` flag in TS) — ML-based automatic approval for low-risk tool calls in AFK/auto mode. Not a near-term priority.
