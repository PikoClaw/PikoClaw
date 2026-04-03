# Spec: Slash Commands & Skills System

**Status**: 🔶 Partial — core built-ins done; many commands missing
**Rust crate**: `piko-skills`
**TS source**: `commands/`, `skills/`

---

## Overview

Slash commands are user-facing shortcuts dispatched from the input bar. Built-in commands are hard-coded in `piko-skills`. User-defined skills are loaded from `~/.config/pikoclaw/skills/`.

---

## What's Implemented

### Built-in Slash Commands ✅

| Command | Action |
|---------|--------|
| `/help` | Show available commands in TUI |
| `/clear` | Clear conversation history (start fresh, keep session) |
| `/model <name>` | Switch active model mid-session |
| `/compact` | Summarize conversation and replace with summary to reduce tokens |
| `/theme [name]` | Cycle themes or set by name |

### User-Defined Skills ✅
- [x] Load from `~/.config/pikoclaw/skills/*.md`
- [x] Skill file format: YAML frontmatter (`name`, `description`, `args`) + body as prompt template
- [x] Argument substitution: `{{arg_name}}` replaced with user-provided values
- [x] `SkillRegistry` — register + lookup by name
- [x] `SkillDispatcher` — invoke skill, return `DispatchResult`
- [x] Skills appear in `/help` output

---

## Not Yet Implemented

### Session Commands ❌
- `/sessions` — list all saved sessions (name, date, cwd). See [29_session_commands.md](29_session_commands.md)
- `/delete <id>` — delete a session
- `/resume <id>` — resume session from within TUI. See [06_session_persistence.md](06_session_persistence.md)
- `/rename <name>` — rename current session

### Configuration Commands ❌
- `/config` or `/settings` — interactive settings editor in TUI
  - Show current config values
  - Allow editing model, max_tokens, permission defaults, theme
  - Write changes to `~/.config/pikoclaw/config.toml`

### Permission Commands ❌
- `/permissions` — show current permission rules
  - List all rules from config
  - List session-accumulated "always allow/deny"
  - Allow adding/removing rules inline

### Git Commands ❌
- `/commit` — stage all changes and commit with AI-generated commit message
  - Implementation: run `git diff --staged`, send to agent with "write a commit message" prompt, run `git commit -m "..."`
- `/commit-push-pr` — commit + push + create GitHub PR

### Context Management ❌
- `/add-dir <path>` — add additional working directory to context (multi-root projects)
- `/context` — show context window usage stats (tokens used, % full)

### Memory Commands ❌
- `/memory` — view/edit memory files from within TUI. See [16_memory_memdir.md](16_memory_memdir.md)

### MCP Commands ❌
- `/mcp` — list connected MCP servers, their tools and status
- `/mcp add <name> <command>` — add an MCP server dynamically

### Utility Commands ❌
- `/cost` — show cumulative token cost in USD with per-token breakdown. See [31_cost_tracking.md](31_cost_tracking.md)
- `/version` — show PikoClaw version, build date, and current model
- `/status` — show connection status: API key valid, model, MCP server connection states
- `/doctor` — run diagnostics: API key check, config file validity, MCP server connectivity, CLAUDE.md found/not-found
- `/context` — show context window usage: tokens used, % of model limit, estimated remaining turns
- `/export` — export current session to Markdown or JSON file (output path printed)
- `/add-dir <path>` — add an additional working directory to file search context (multi-root projects)
- `/diff` — show git diff of all changes made this session (runs `git diff HEAD`)

### Plan Mode Commands ❌
- `/plan` — toggle plan mode (read-only agent). See [17_plan_mode.md](17_plan_mode.md)

### Vim Mode Commands ❌
- `/vim` — toggle vim keybinding mode in input bar. See [14_vim_keybindings.md](14_vim_keybindings.md)

---

## Skill File Format (Reference)

```markdown
---
name: review
description: Review code changes for issues
args:
  - name: focus
    description: What to focus on (security, performance, style)
    required: false
---

Please review the following code changes.
{{#if focus}}Focus specifically on: {{focus}}.{{/if}}

Look at the recent changes with `git diff HEAD~1` and provide feedback.
```

---

## Dispatcher Implementation Notes

When user types `/command args` in input bar:

1. Parse: extract command name and remainder as args string
2. Lookup in `SkillRegistry` by name
3. If built-in: call handler directly (returns `DispatchResult::Handled` or `DispatchResult::SendToAgent(prompt)`)
4. If user skill: substitute args into template, send resulting prompt to agent loop
5. If not found: show error "Unknown command: /foo"

`DispatchResult` enum:
```rust
pub enum DispatchResult {
    Handled,                    // command handled internally (e.g. /theme)
    SendToAgent(String),        // turn the skill into a user message for the agent
    Error(String),              // show error in TUI
}
```
