# PikoClaw Feature Spec Index

Porting specs from claude-code (TypeScript) → PikoClaw (Rust).
Each file is a dedicated spec for one feature area with implementation todos.

## Status Legend
- ✅ **Done** — Fully implemented in PikoClaw
- 🔶 **Partial** — Some parts implemented, gaps remain
- ❌ **Todo** — Not yet started

---

## Feature Files

| File | Feature | Status |
|------|---------|--------|
| [01_api_client.md](01_api_client.md) | Anthropic API Client & Streaming | ✅ Done |
| [02_core_agent_loop.md](02_core_agent_loop.md) | Core Agent Loop & Turn Execution | ✅ Done |
| [03_tools_builtin.md](03_tools_builtin.md) | Built-in Tools (Bash, Read, Write, Edit, Glob, Grep, WebFetch, WebSearch) | ✅ Done |
| [04_tools_advanced.md](04_tools_advanced.md) | Advanced Tools (Agent, NotebookEdit, AskUser, TodoWrite) | ✅ Done |
| [05_permissions.md](05_permissions.md) | Permission System | ✅ Done |
| [06_session_persistence.md](06_session_persistence.md) | Session Persistence & Management | 🔶 Partial |
| [07_tui.md](07_tui.md) | Terminal UI (TUI) | ✅ Done |
| [08_slash_commands.md](08_slash_commands.md) | Slash Commands & Skills System | 🔶 Partial |
| [09_mcp.md](09_mcp.md) | MCP (Model Context Protocol) Integration | 🔶 Partial |
| [10_config_claudemd.md](10_config_claudemd.md) | Config, CLAUDE.md & Environment Loading | ✅ Done |
| [11_prompt_caching.md](11_prompt_caching.md) | Prompt Caching & Token Tracking | ✅ Done |
| [12_hooks_system.md](12_hooks_system.md) | Hooks System | ❌ Todo |
| [13_extended_thinking.md](13_extended_thinking.md) | Extended Thinking Support | ❌ Todo |
| [14_vim_keybindings.md](14_vim_keybindings.md) | Vim Mode & Keybinding Customization | ❌ Todo |
| [15_image_input.md](15_image_input.md) | Image & Screenshot Input | ❌ Todo |
| [16_memory_memdir.md](16_memory_memdir.md) | Memory / Memdir System | ❌ Todo |
| [17_plan_mode.md](17_plan_mode.md) | Plan Mode (EnterPlan/ExitPlan tools) | ❌ Todo |
| [18_worktrees.md](18_worktrees.md) | Git Worktree Tools | ❌ Todo |
| [19_task_system.md](19_task_system.md) | Background Task System (TaskCreate/Get/List/etc.) | ❌ Todo |
| [20_auto_compact.md](20_auto_compact.md) | Auto-Compact / Context Summarization | ❌ Todo |
| [21_multi_agent.md](21_multi_agent.md) | Multi-Agent / Coordinator / Swarm | ❌ Todo |
| [22_ide_integration.md](22_ide_integration.md) | IDE Integration (VS Code / JetBrains) | ❌ Todo |
| [23_bridge_remote.md](23_bridge_remote.md) | Bridge & Remote Session (claude.ai) | ❌ Todo |
| [24_voice_input.md](24_voice_input.md) | Voice Input (STT) | ❌ Todo |
| [25_plugins.md](25_plugins.md) | Plugin System & Marketplace | ❌ Todo |
| [26_output_styles.md](26_output_styles.md) | Configurable Output Styles | ❌ Todo |
| [27_cron_scheduler.md](27_cron_scheduler.md) | Cron Scheduler & Remote Triggers | ❌ Todo |
| [28_mcp_resources.md](28_mcp_resources.md) | MCP Resource Reading (List/Read) | ❌ Todo |
| [29_session_commands.md](29_session_commands.md) | Session List & Management Commands | ❌ Todo |
| [30_system_prompt_architecture.md](30_system_prompt_architecture.md) | System Prompt Assembly & Sections | 🔶 Partial |
| [31_cost_tracking.md](31_cost_tracking.md) | Cost Tracking & Budget Enforcement | ❌ Todo |
| [32_settings_layers.md](32_settings_layers.md) | Layered Settings & Config Migrations | ❌ Todo |
| [33_buddy_companion.md](33_buddy_companion.md) | Buddy Companion System (Tamagotchi) | ❌ Todo |
