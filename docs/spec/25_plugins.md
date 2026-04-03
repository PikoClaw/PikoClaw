# Spec: Plugin System & Marketplace

**Status**: ❌ Todo
**TS source**: `plugins/`, `services/plugins/`

---

## Overview

The plugin system allows third-party developers to extend PikoClaw with new tools, slash commands, and behaviors — without modifying the core binary. Plugins are distributed via a marketplace and installed with a single command.

---

## TS Plugin Architecture

In TS, plugins are TypeScript modules loaded at runtime via dynamic `import()`. Each plugin exports:
- Additional tools
- Additional slash commands
- Lifecycle hooks (onLoad, onUnload)
- React components for TUI display

Since Rust doesn't support dynamic code loading in the same way, the Rust plugin approach must be different.

---

## Rust Plugin Approaches

### Option A: MCP Server as Plugin (Recommended)

Each "plugin" is actually an MCP server that the user adds to their config. MCP provides:
- New tools (via `tools/list` + `tools/call`)
- No need for a separate plugin loading mechanism

**Advantage**: Already implemented via `piko-mcp`. No new code needed.
**Limitation**: MCP servers can only add tools, not slash commands or UI.

### Option B: Skills as Lightweight Plugins

User-defined skills (`.md` files in `~/.config/pikoclaw/skills/`) already provide:
- New slash commands (each skill becomes a `/skill-name` command)
- Prompt templates for specialized behaviors

**Advantage**: Already implemented via `piko-skills`.
**Limitation**: No new native tools.

### Option C: Plugin Registry TOML

A `plugins.toml` file in config directory that references:
- MCP servers to auto-install (npm package or URL)
- Skill packs to download
- Config snippets to apply

```toml
[[plugins]]
name = "github-tools"
type = "mcp"
package = "@modelcontextprotocol/server-github"
env = { GITHUB_TOKEN = "$GITHUB_TOKEN" }

[[plugins]]
name = "code-review-skills"
type = "skill_pack"
url = "https://example.com/skills/code-review.tar.gz"
```

---

## Marketplace Concept

A central index (static JSON hosted on GitHub Pages or similar) of available plugins:

```json
{
  "plugins": [
    {
      "name": "github-tools",
      "description": "GitHub API integration - manage PRs, issues, code review",
      "type": "mcp",
      "package": "@modelcontextprotocol/server-github",
      "version": "1.0.0",
      "downloads": 12400
    },
    {
      "name": "docker-tools",
      "description": "Docker and container management",
      "type": "mcp",
      "package": "@pikoclaw/mcp-docker",
      "version": "0.2.1"
    }
  ]
}
```

### `/plugin` Slash Commands

```
/plugin list              → list installed plugins
/plugin search <query>    → search marketplace
/plugin install <name>    → install a plugin
/plugin remove <name>     → remove a plugin
/plugin update            → update all plugins
```

---

## Implementation Plan (Minimal)

### Phase 1: MCP-based plugins (uses existing infrastructure)

1. Add `[[plugins]]` table to config
2. On startup: for each plugin, auto-configure as MCP server (install npm package if needed)
3. `/plugin` command reads from marketplace index JSON

### Phase 2: Skill packs

1. Define skill pack format: `.tar.gz` containing `*.md` skill files + `manifest.json`
2. `/plugin install` downloads, verifies, extracts to `~/.config/pikoclaw/plugins/<name>/skills/`
3. Skills from plugin loaded alongside user skills

### Phase 3: Native plugins (future)

If Rust WASM plugins are desired long-term:
- Plugins compiled to WASM
- Loaded via `wasmtime` crate
- Can expose tools via a WASM ABI

This is complex and not recommended for near-term.

---

## Priority

Low. The MCP ecosystem already provides plugin-like extensibility. A formal plugin system can wait until core features are complete.
