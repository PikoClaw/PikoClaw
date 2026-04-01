# PikoClaw

High-performance AI agent for developers, written in Rust.

## Install

```bash
brew tap PikoClaw/pikoclaw
brew install pikoclaw
```

## Usage

```bash
pikoclaw

pikoclaw --print "explain this codebase"

pikoclaw continue

pikoclaw resume <session-id>

pikoclaw --model sonnet
pikoclaw --model opus
pikoclaw --model haiku

pikoclaw --dangerously-skip-permissions
```

## Configuration

Config file: `~/.config/pikoclaw/config.toml`

```toml
[api]
model = "claude-sonnet-4-5"
max_tokens = 8192

[permissions]
bash = "ask"
file_write = "ask"
file_read = "allow"
web_fetch = "ask"

[[permissions.rules]]
tool = "bash"
pattern = "rm -rf *"
decision = "deny"
```

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

## Built-in Tools

| Tool | Description |
|---|---|
| `Bash` | Run shell commands |
| `Read` | Read files with line numbers |
| `Write` | Write files |
| `Edit` | Exact string replacement in files |
| `Glob` | Find files by pattern |
| `Grep` | Search file contents with regex |
| `WebFetch` | Fetch and extract text from URLs |
| `WebSearch` | Web search via Anthropic beta |
| `NotebookEdit` | Edit Jupyter notebook cells |
| `TodoWrite` | In-session task checklist |
| `AskUserQuestion` | Ask the user multiple-choice questions |
| `Agent` | Spawn isolated sub-agents |

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

## Creating New Release

```
cd PikoClaw
git tag v0.1.0
git push origin v0.1.0
```

The release.yml workflow will:

1. Build binaries for macOS arm64, macOS x86_64, and Linux x86_64
2. Create the GitHub release and upload the binaries
3. Automatically update the SHA256 hashes in homebrew-pikoclaw/Formula/pikoclaw.rb and push it

## Status

### Done

- Cargo workspace with 10 crates
- Anthropic API client with SSE streaming
- Core agent loop with multi-turn tool use
- Tools: Bash, Read, Write, Edit, Glob, Grep, WebFetch, WebSearch
- Sub-agent tool (isolated child agents with own tool access and context)
- NotebookEdit tool (Jupyter `.ipynb` cell replace/insert/delete)
- TodoWrite tool (in-session task checklist with pending/in-progress/completed states)
- AskUserQuestion tool (multi-choice prompts answered inline in TUI)
- Permission system (allow/deny/ask per tool and pattern)
- TUI inline permission dialogs (y/n/always/deny-always per tool call)
- TUI inline question dialogs (numbered option selection)
- Session persistence (save, resume, continue)
- Config file (TOML) and environment variable loading
- Interactive TUI (ratatui) with multi-line input (Shift+Enter) and scroll
- Slash command system with user-defined skills
- `/compact` command (summarizes and clears conversation history)
- `/model <name>` command (switch model mid-session)
- MCP client (stdio + SSE transports, full tool bridge into agent registry)
- Anthropic web search (`web_search_20250305` beta tool)
- CLAUDE.md loading (`~/.claude/CLAUDE.md`, project `CLAUDE.md`, `.claude/rules/*.md`)
- Prompt caching (`cache_control: ephemeral` on system prompt, last tool, last message)
- Token usage tracking and display in TUI status bar (cumulative input/output/cache tokens)

### Todo

- Session list and management commands (`/sessions`, `/delete`)
- Syntax highlighting for code blocks in TUI output
- Image and screenshot input support
- Hooks system (user-defined shell commands triggered on tool events)
- Extended thinking support
- Rate limit display in status bar
- `/resume` command from within the TUI (currently only via CLI)
- MCP resource reading (`ListResources`, `ReadResource`)
- Configurable output styles
- Vim keybinding mode
