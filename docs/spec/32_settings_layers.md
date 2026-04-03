# Spec: Layered Settings & Migrations

**Status**: ❌ Todo — single flat config file only; no layering, no migrations, no hot reload
**Rust crate**: `piko-config`
**TS source**: `utils/config.ts`, `migrations/`, `services/remoteManagedSettings/`

---

## Overview

Settings are resolved from multiple sources in priority order. Lower-priority defaults are overridden by higher-priority sources. This allows per-project overrides without touching the global config, and gitignored local overrides without polluting committed project config.

---

## Settings Resolution Order (highest wins)

```
1. CLI flags              ← --model, --max-turns, etc. (highest priority)
2. Environment variables  ← ANTHROPIC_API_KEY, PIKOCLAW_MODEL, etc.
3. Local project config   ← <cwd>/.pikoclaw/settings.local.toml  (gitignored)
4. Project config         ← <cwd>/.pikoclaw/settings.toml        (committed)
5. Global user config     ← ~/.config/pikoclaw/config.toml       (current only)
6. Built-in defaults      ← hardcoded in piko-config             (lowest priority)
```

---

## File Locations

| Layer | Path | Gitignored? | Purpose |
|-------|------|-------------|---------|
| Global | `~/.config/pikoclaw/config.toml` | n/a | User-wide preferences |
| Project | `<cwd>/.pikoclaw/settings.toml` | No | Committed project settings |
| Local | `<cwd>/.pikoclaw/settings.local.toml` | Yes | Personal local overrides |

The `.pikoclaw/settings.local.toml` file should be added to `.gitignore` by default when the project config is first created.

---

## What Each Layer Controls

### Global (`~/.config/pikoclaw/config.toml`)
- API key, base URL
- Default model
- Global permission rules
- TUI theme, vim mode
- MCP server connections
- Onboarding state

### Project (`.pikoclaw/settings.toml`)
- Project-specific model override
- Project-specific permission rules
- Project-specific MCP servers
- Custom system prompt additions
- Tool restrictions for this project

### Local (`.pikoclaw/settings.local.toml`)
- API key override (for different billing account on this project)
- Personal permission preferences
- Local MCP server (running on dev machine, not in repo)

---

## Merge Semantics

Scalar values: higher-priority wins outright.

```toml
# global: model = "claude-sonnet-4-6"
# project: model = "claude-opus-4-6"
# resolved: model = "claude-opus-4-6"
```

Lists (permission rules, MCP servers): **concatenated**, with higher-priority entries evaluated first.

```toml
# global rules:   [{tool: bash, mode: ask}]
# project rules:  [{tool: bash, pattern: "rm -rf", mode: deny}]
# resolved rules: [{tool: bash, pattern: "rm -rf", mode: deny}, {tool: bash, mode: ask}]
# (deny-pattern checked first, then general ask)
```

---

## Implementation Plan

### Step 1: Config Loading with Layers

```rust
pub struct ResolvedConfig {
    pub api: ApiConfig,
    pub permissions: PermissionsConfig,
    pub tui: TuiConfig,
    pub mcp: McpConfig,
}

pub async fn load_resolved_config(cwd: &Path) -> ResolvedConfig {
    let global   = load_file(&global_config_path()).await.unwrap_or_default();
    let project  = load_file(&cwd.join(".pikoclaw/settings.toml")).await.unwrap_or_default();
    let local    = load_file(&cwd.join(".pikoclaw/settings.local.toml")).await.unwrap_or_default();

    merge_configs(vec![global, project, local])
    // CLI flags applied on top by caller
}

fn merge_configs(layers: Vec<RawConfig>) -> ResolvedConfig {
    // scalar: last (highest priority) non-None wins
    // lists:  concatenate in order (highest priority first for rule matching)
}
```

### Step 2: `RawConfig` with `Option` Fields

All fields in `RawConfig` are `Option<T>` so absence means "not set at this layer":

```rust
pub struct RawConfig {
    pub api: Option<RawApiConfig>,
    pub permissions: Option<RawPermissionsConfig>,
    pub tui: Option<RawTuiConfig>,
    pub mcp: Option<RawMcpConfig>,
}

pub struct RawApiConfig {
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub max_budget_usd: Option<f64>,
}
```

### Step 3: `.gitignore` Auto-Management

When writing to `.pikoclaw/settings.local.toml` for the first time, ensure `.pikoclaw/settings.local.toml` is in the project's `.gitignore`:

```rust
pub async fn ensure_local_settings_gitignored(cwd: &Path) {
    let gitignore = cwd.join(".gitignore");
    let entry = ".pikoclaw/settings.local.toml";
    // read .gitignore, append entry if not already present
}
```

---

## Settings Migrations

When the config format changes between PikoClaw versions, migrations update the stored config automatically.

### Migration File Format

```rust
pub struct Migration {
    pub version: u32,
    pub description: &'static str,
    pub apply: fn(&mut RawConfig),
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        description: "Rename theme.name → tui.theme",
        apply: |config| {
            if let Some(old_theme) = config.remove("theme.name") {
                config.tui.get_or_insert_default().theme = Some(old_theme);
            }
        },
    },
    // ...
];
```

### Migration Execution

On startup:
1. Read `config_version` from global config (default 0 if missing)
2. Run all migrations where `migration.version > config_version` in order
3. Write migrated config back to disk
4. Update `config_version`

```toml
# Written by migration system:
config_version = 3
```

---

## Hot Reload

Watch config files for changes during session. If changed, reload and apply:

```rust
pub async fn watch_config(config_path: PathBuf, tx: mpsc::Sender<ConfigReloaded>) {
    let mut watcher = notify::recommended_watcher(move |event| {
        if matches!(event, notify::Event::Modify(_)) {
            tx.send(ConfigReloaded).ok();
        }
    })?;
    watcher.watch(&config_path, notify::RecursiveMode::NonRecursive)?;
}
```

On `ConfigReloaded` event in TUI:
- Reload config layers
- Apply new permission rules (new rules take effect immediately)
- Apply new theme if changed
- Do NOT change model mid-turn (apply on next turn start)

---

## Todos

- [ ] Define `RawConfig` with `Option<T>` fields for all layers
- [ ] Implement `load_resolved_config(cwd)` with 3-layer merge
- [ ] Move project settings to `<cwd>/.pikoclaw/settings.toml`
- [ ] Add local settings layer `<cwd>/.pikoclaw/settings.local.toml`
- [ ] Auto-add to `.gitignore` when local settings created
- [ ] Add `config_version` field and migration runner
- [ ] Implement config file watcher for hot reload
- [ ] `/config` slash command: show resolved config with source of each value (global/project/local/env/flag)
