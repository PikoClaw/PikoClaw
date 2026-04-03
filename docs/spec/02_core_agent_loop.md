# Spec: Core Agent Loop & Turn Execution

**Status**: ✅ Done
**Rust crate**: `piko-agent`
**TS source**: `query.ts`, `QueryEngine.ts`, `context.ts`

---

## Overview

The agent loop is the heart of PikoClaw. It drives multi-turn conversations: sends a message to the API, streams the response, executes any tool calls, feeds results back, and repeats until the model stops requesting tools or the turn limit is hit.

---

## What's Implemented

- [x] `run_turn()` — single turn of the agent loop with streaming
- [x] Multi-turn tool execution: tool result messages fed back into next API call
- [x] Turn limit (`max_turns`) with configurable cap
- [x] System prompt injection (from config + CLAUDE.md)
- [x] Cancellation token (`CancellationToken`) — Ctrl+C aborts cleanly
- [x] `AgentEvent` output stream: `Text`, `ToolStart`, `ToolComplete`, `Error`, `TokenUsage`
- [x] `OutputSink` trait for routing events to TUI or stdout
- [x] `ConversationContext` — message history accumulation across turns
- [x] Token usage accumulation (input, output, cache_creation, cache_read)
- [x] Web search beta tool passthrough (native API tool, no local execution)
- [x] `AgentConfig` with: model, max_tokens, max_turns, cwd, system_prompt, bypass_permissions
- [x] `AgentTool` — allows sub-agents to be spawned recursively

---

## Spec / Technical Details

### Turn Flow

```
1. Build request (messages + system + tools)
2. POST /v1/messages with stream:true
3. Stream response:
   a. text deltas → emit AgentEvent::Text
   b. tool_use blocks → collect name + input JSON
4. If stop_reason == "tool_use":
   a. For each tool call:
      - Check permissions
      - Execute tool
      - Collect ToolResult
   b. Append assistant message + tool results to context
   c. Loop back to step 1
5. If stop_reason == "end_turn" or "max_tokens":
   a. Emit AgentEvent::Done
   b. Return
```

### Stop Conditions

| Condition | Behavior |
|-----------|----------|
| `stop_reason: "end_turn"` | Normal completion |
| `stop_reason: "max_tokens"` | Warn user, stop |
| `stop_reason: "tool_use"` | Execute tools, continue |
| `turns >= max_turns` | Stop with error |
| Cancellation token fired | Abort immediately |

### Message Format Sent to API

```
system: [system_prompt] [CLAUDE.md content]
messages: [
  { role: user, content: "..." },
  { role: assistant, content: [text, tool_use, ...] },
  { role: user, content: [tool_result, ...] },
  ...
]
```

### AgentEvent Enum

```rust
pub enum AgentEvent {
    Text(String),
    ThinkingText(String),     // extended thinking blocks (todo)
    ToolStart { name, input },
    ToolComplete { name, result },
    TokenUsage { input, output, cache_creation, cache_read },
    Error(AgentError),
    Done,
}
```

---

## Gaps / Todos

- [ ] **Extended thinking** — detect `thinking` content blocks from stream, emit `ThinkingText` events, display in TUI. See [13_extended_thinking.md](13_extended_thinking.md)
- [ ] **Token budget management** — auto-compact when context approaches limit. See [20_auto_compact.md](20_auto_compact.md)
- [ ] **Tool use concurrency** — TS executes multiple tool calls from a single turn in parallel (where safe). Currently Rust executes sequentially.
- [ ] **`output-format` flag** — `--output-format json` for machine-readable output (print mode)
- [ ] **`--verbose` streaming** — show raw API events in debug mode
