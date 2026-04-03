# Spec: Prompt Caching & Token Tracking

**Status**: ✅ Done
**Rust crate**: `piko-agent`, `piko-api`
**TS source**: `utils/promptCaching.ts`, `bootstrap/state.ts`

---

## Overview

Prompt caching reduces API costs by marking stable portions of the prompt as cacheable. Token usage is tracked cumulatively and displayed in the TUI status bar.

---

## What's Implemented

### Prompt Caching ✅
- [x] `cache_control: { type: "ephemeral" }` injected on:
  - System prompt (last block of system array)
  - Last tool result in message history
  - Last user message
- [x] Cache read tokens tracked separately from regular input tokens
- [x] Cache creation tokens tracked separately

### Token Tracking ✅
- [x] Per-turn token counts from `usage` field in API response
- [x] Cumulative session totals: `input_tokens`, `output_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`
- [x] Displayed in TUI status bar: `in: X | out: Y | cache↑: Z | cache↓: W`
- [x] `TokenUsage` event emitted from agent loop after each turn

---

## Spec / Technical Details

### Cache Control Injection Points

The Anthropic API caches the prefix of the prompt up to the last `cache_control` marker. To maximize cache hits:

```
System: [
  { text: "base system prompt..." },
  { text: "CLAUDE.md content...", cache_control: { type: "ephemeral" } }  ← cache here
]

Messages: [
  { role: "user",      content: "..." },
  { role: "assistant", content: [...] },
  ...
  { role: "user",      content: [
    { type: "tool_result", ... },
    { type: "tool_result", ..., cache_control: { type: "ephemeral" } },  ← cache here
    { type: "text", text: "user's latest message", cache_control: { type: "ephemeral" } }  ← cache here
  ]}
]
```

### Token Display Format (Status Bar)

```
↑12.3k ↓2.1k ⚡3.4k(cached)
```

Where:
- `↑` = input tokens sent this session
- `↓` = output tokens generated
- `⚡` = cache read tokens (cheap, ~10% of input price)
- `cache↑` = cache write tokens (slightly more expensive than input)

---

## Gaps / Todos

- [ ] **Cost calculation** — convert token counts to USD based on model pricing table.
  - Show total session cost in `/cost` command.
  - Model pricing should be a static table in code (updated when Anthropic changes prices).
  - `claude-opus-4-6`: $15/$75 per M tokens (input/output)
  - `claude-sonnet-4-6`: $3/$15 per M tokens
  - `claude-haiku-4-5`: $0.25/$1.25 per M tokens

- [ ] **Cache efficiency display** — show cache hit rate (cache_read / total_input as %) in `/stats` or status bar tooltip.

- [ ] **Smarter cache placement** — currently cache markers placed at fixed positions. TS has more sophisticated logic: if tools list is long and stable, cache after tools too. Could improve cache hit rate for long sessions.

- [ ] **`/compact` token savings display** — after compaction, show how many tokens were freed.
