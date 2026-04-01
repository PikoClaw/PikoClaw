# PikoClaw

High-performance AI agent for developers, written in Rust.

## Install

```bash
brew tap PikoClaw/pikoclaw
brew install pikoclaw
```

## Usage

```bash
# Start interactive session
pikoclaw

# One-shot prompt (headless)
pikoclaw --print "explain this codebase"

# Continue last session
pikoclaw continue

# Resume a specific session
pikoclaw resume <session-id>

# Use a specific model
pikoclaw --model sonnet
pikoclaw --model opus
pikoclaw --model haiku

# Bypass permission prompts
pikoclaw --dangerously-skip-permissions
```

## Configuration

Config file: `~/.config/pikoclaw/config.toml`

```toml
[api]
model = "claude-sonnet-4-5"
max_tokens = 8192

[permissions]
bash = "ask"        # allow | deny | ask
file_write = "ask"
file_read = "allow"
web_fetch = "ask"

[[permissions.rules]]
tool = "bash"
pattern = "rm -rf *"
decision = "deny"
```

Set your API key via environment variable:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

## Built-in Tools

| Tool | Description |
|---|---|
| `Bash` | Run shell commands |
| `Read` | Read files with line numbers |
| `Write` | Write files (creates directories as needed) |
| `Edit` | Exact string replacement in files |
| `Glob` | Find files by pattern (respects .gitignore) |
| `Grep` | Search file contents with regex |
| `WebFetch` | Fetch and extract text from URLs |

## Slash Commands

| Command | Description |
|---|---|
| `/help` | List available commands |
| `/clear` | Clear conversation history |
| `/model <name>` | Switch model mid-session |
| `/compact` | Summarize history to reduce token usage |
| `/exit` | Exit |

Custom skills can be added as Markdown files in `~/.config/pikoclaw/skills/`.

## Building

```bash
cargo build --release
```

Requires Rust 1.80+.

## Architecture

Cargo workspace with 10 crates:

```
crates/
  piko-types        # Core domain types
  piko-config       # Config file and env var loading
  piko-api          # Anthropic API client with SSE streaming
  piko-tools        # Tool trait and built-in tool implementations
  piko-permissions  # Permission policy engine
  piko-session      # Session persistence and resume
  piko-agent        # Core agent loop and orchestration
  piko-mcp          # Model Context Protocol client
  piko-tui          # ratatui interactive terminal UI
  piko-skills       # Slash command registry and dispatcher
```

## Status

### Done

- Cargo workspace with 10 crates
- Anthropic API client with SSE streaming
- Core agent loop with multi-turn tool use
- Tools: Bash, Read, Write, Edit, Glob, Grep, WebFetch, WebSearch
- Sub-agent tool (spawn isolated child agents with their own tool access)
- Permission system (allow/deny/ask per tool and pattern)
- TUI inline permission dialogs (y/n/always/deny-always per tool call)
- Session persistence (save, resume, continue)
- Config file (TOML) and environment variable loading
- Interactive TUI (ratatui) with multi-line input (Shift+Enter) and scroll
- Slash command system with user-defined skills
- `/compact` command (summarizes conversation history)
- MCP client (stdio + SSE transports, full tool bridge into agent registry)
- Anthropic web search (web_search_20250305 beta tool)

### Next

- Session list and management commands (`/sessions`, `/delete`)
- Syntax highlighting for code blocks in TUI output
- Notebook (Jupyter) tool support
- Image/screenshot input support
- Token usage display in status bar
- Configurable system prompt from CLAUDE.md files
