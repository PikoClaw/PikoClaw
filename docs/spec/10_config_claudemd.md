# Spec: Config, CLAUDE.md & Environment Loading

**Status**: ✅ Done
**Rust crate**: `piko-config`
**TS source**: `utils/config.ts`, `utils/claudemd.ts`

---

## Overview

Configuration is loaded from a TOML file, environment variables, and CLI flags (in that priority order). CLAUDE.md files inject project-specific instructions into the system prompt.

---

## What's Implemented

### Config File ✅
- [x] Config path: `~/.config/pikoclaw/config.toml`
- [x] `load_config()` — reads TOML, falls back to defaults if missing
- [x] `save_config()` — writes updated config back to file
- [x] `PikoConfig` struct with nested sections

### Config Sections ✅

#### `[api]`
```toml
[api]
model = "claude-opus-4-6"
max_tokens = 8192
base_url = "https://api.anthropic.com"  # optional override
api_key = "sk-..."                       # optional (prefer env var)
```

#### `[permissions]`
```toml
[permissions]
default_mode = "ask"   # allow | deny | ask

[[permissions.rules]]
tool_name = "bash"
mode = "ask"

[[permissions.rules]]
tool_name = "bash"
pattern = "rm -rf *"
mode = "deny"
```

#### `[tui]`
```toml
[tui]
theme = "dark"
syntax_highlighting = true
onboarding_done = false
```

#### `[mcp.servers.*]`
```toml
[mcp.servers.filesystem]
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user"]
```

### Environment Variables ✅
- [x] `ANTHROPIC_API_KEY` → `api.api_key`
- [x] `ANTHROPIC_BASE_URL` → `api.base_url`
- [x] `PIKOCLAW_MODEL` → `api.model`
- [x] Env vars override config file values

### CLI Flag Overrides ✅
- [x] `--model <name>` → overrides `api.model`
- [x] `--max-turns <n>` → overrides agent max_turns
- [x] `--system-prompt <text>` → prepended to system prompt
- [x] `--dangerously-skip-permissions` → bypasses permission system

### CLAUDE.md Loading ✅
- [x] `~/.claude/CLAUDE.md` — global user instructions (loaded always)
- [x] `<cwd>/CLAUDE.md` — project-level instructions (loaded when present)
- [x] `<cwd>/.claude/rules/*.md` — additional rule files (all loaded, sorted by filename)
- [x] All CLAUDE.md content appended to system prompt after base system prompt
- [x] Handles missing files gracefully (skip)

---

## CLAUDE.md Loading Order

```
System prompt =
  [base system prompt]
  + [~/.claude/CLAUDE.md content]       (if exists)
  + [<cwd>/CLAUDE.md content]           (if exists)
  + [<cwd>/.claude/rules/*.md content]  (all, sorted, if exist)
```

---

## Gaps / Todos

- [ ] **Layered settings** — TS has 4 config layers:
  1. Managed (cloud/policy, read-only)
  2. Local project (`.claude/settings.local.json` — gitignored)
  3. Project (`.claude/settings.json` — committed)
  4. Global (`~/.claude/settings.json`)

  Rust currently only has one config file. Should add:
  - `<cwd>/.pikoclaw/settings.toml` — project-level config (committed)
  - `<cwd>/.pikoclaw/settings.local.toml` — local overrides (gitignored)

- [ ] **Config schema validation** — on load, validate config values and report clear errors (bad model name, invalid permission mode string, etc.)

- [ ] **`/config` slash command** — interactive config editor in TUI. See [08_slash_commands.md](08_slash_commands.md)

- [ ] **`CLAUDE.md` import directives** — TS supports `@path/to/file.md` inside CLAUDE.md to include other files. Rust ignores these directives.
  - Implementation: scan loaded CLAUDE.md content for `@...` lines, load referenced files recursively.

- [ ] **CLAUDE.md watch** — TS reloads CLAUDE.md if changed mid-session. Rust loads once at startup only.
