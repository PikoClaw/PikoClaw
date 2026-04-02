# Status

## Done

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
- Multiple UI themes (dark, light, dark-daltonized, light-daltonized, dark-ansi, light-ansi)
- `/theme [name]` command (cycle or set theme mid-session; active theme shown in status bar)
- First-run onboarding (full-screen theme picker with live preview and colour swatches)
- Theme persisted to config file (`~/.config/pikoclaw/config.toml`) after onboarding
- Syntax highlighting for code blocks in TUI output (syntect / TextMate grammars, 190+ languages)
- Welcome header on launch (versioned border, Clawd pixel-art, model/cwd info, tips and recent activity panels)
- Fixed `/theme` slash command not dispatching as built-in
- Rate limit display in status bar
- Fixed dark theme background (full-frame bg fill so terminal default doesn't bleed through)

## Todo

- Session list and management commands (`/sessions`, `/delete`)
- Image and screenshot input support
- Hooks system (user-defined shell commands triggered on tool events)
- Extended thinking support
- `/resume` command from within the TUI (currently only via CLI)
- MCP resource reading (`ListResources`, `ReadResource`)
- Configurable output styles
- Vim keybinding mode
