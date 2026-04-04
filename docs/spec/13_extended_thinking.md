# Spec: Extended Thinking Support

**Status**: ✅ Done
**TS source**: `constants/betas.ts` (`interleaved-thinking-2025-05-14`), `components/messages/AssistantThinkingMessage.tsx`

---

## Overview

Extended thinking allows the model to emit internal "thinking" blocks before its final response. These blocks contain the model's step-by-step reasoning and are displayed separately in the TUI.

---

## API Behavior

### Request

Extended thinking is enabled by including the `thinking` parameter in the request:

```json
{
  "model": "claude-opus-4-6",
  "max_tokens": 16000,
  "thinking": {
    "type": "enabled",
    "budget_tokens": 10000
  },
  "messages": [...],
  "betas": ["interleaved-thinking-2025-05-14"]
}
```

`budget_tokens`: how many tokens the model can spend on thinking (must be < `max_tokens`).

### Response Stream

Thinking blocks appear as `content_block_start` events with `type: "thinking"`:

```
event: content_block_start
data: { "index": 0, "content_block": { "type": "thinking", "thinking": "" } }

event: content_block_delta
data: { "index": 0, "delta": { "type": "thinking_delta", "thinking": "Let me analyze..." } }

event: content_block_stop
data: { "index": 0 }

event: content_block_start
data: { "index": 1, "content_block": { "type": "text", "text": "" } }
...
```

---

## Implementation Plan

### Step 1: API Layer (`piko-api`)

Add `thinking` field to `MessagesRequest`:

```rust
pub struct ThinkingConfig {
    pub r#type: String,      // "enabled"
    pub budget_tokens: u32,
}

pub struct MessagesRequest {
    // existing fields...
    pub thinking: Option<ThinkingConfig>,
}
```

Add `thinking` to `StreamEvent` / content block handling:

```rust
pub enum ContentBlockType {
    Text,
    ToolUse,
    Thinking,   // new
}

pub enum ContentDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    ThinkingDelta { thinking: String },  // new
}
```

### Step 2: Agent Loop (`piko-agent`)

Emit `AgentEvent::ThinkingText(String)` when thinking deltas arrive:

```rust
AgentEvent::ThinkingText(chunk)  // streaming thinking text
AgentEvent::ThinkingComplete     // thinking block finished
```

### Step 3: Config

```toml
[api]
extended_thinking = true
thinking_budget_tokens = 10000
```

CLI flag: `--thinking` / `--thinking-budget <n>`

### Step 4: TUI Display (`piko-tui`)

- Render thinking blocks in a collapsed/expandable section above the assistant's response
- Style differently from regular text (dimmed, italic, or with a border)
- Header: `▶ Thinking (1234 tokens)` — expandable
- On expand: show full thinking text with scroll

```
┌─ Thinking ───────────────────────────────────────────┐
│ Let me analyze the code structure first...            │
│ The function on line 42 has a potential null...       │
└───────────────────────────────────────────────────────┘

The issue is in the `process_data` function...
```

### Step 5: Message Storage

Thinking blocks should be stored in session history so they can be reviewed on resume.
- Add `ContentBlock::Thinking { thinking: String }` variant to `piko-types`
- Include in session JSON serialization

---

## Notes

- Thinking blocks are **not sent back** in subsequent API calls — the model only sees the `text` and `tool_use` blocks in history
- Thinking blocks don't count toward regular output tokens in the same way — they're billed separately
- `budget_tokens` should be tunable; 10000 is a reasonable default for complex tasks
- Some models support thinking, some don't — check model compatibility before enabling
