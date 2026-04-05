# Spec: Auto-Compact / Context Summarization

**Status**: 🔶 Partial — manual `/compact` slash command exists; auto-trigger missing
**TS source**: `services/compact/`, `commands/compact.ts`

---

## Overview

When the conversation history grows large and approaches the context window limit, the compaction system summarizes the conversation and replaces the full history with a compact summary. This keeps token usage manageable for long sessions.

The `/compact` slash command triggers this manually. Auto-compact triggers it automatically when usage crosses a threshold.

---

## Current State

- `/compact` command exists and clears history with a brief summary
- No token budget tracking or auto-trigger implemented yet

---

## Implementation Plan

### Step 1: Token Budget Tracking

Track the total conversation size in tokens at all times:

```rust
// In ConversationContext
pub struct ConversationContext {
    pub messages: Vec<Message>,
    pub system_prompt: String,
    pub token_estimate: u32,  // running estimate
}

impl ConversationContext {
    pub fn estimated_tokens(&self) -> u32 {
        // rough estimate: 4 chars ≈ 1 token
        // or use tiktoken-rs for more accuracy
        self.token_estimate
    }

    pub fn needs_compaction(&self, model_limit: u32) -> bool {
        self.estimated_tokens() > (model_limit as f32 * 0.85) as u32
    }
}
```

Model context limits:
- `claude-opus-4-6`: 200,000 tokens
- `claude-sonnet-4-6`: 200,000 tokens
- `claude-haiku-4-5`: 200,000 tokens

Compaction threshold: 85% of limit (to leave room for next response).

### Step 2: Compaction Logic

```rust
pub async fn compact_conversation(
    context: &mut ConversationContext,
    api_client: &AnthropicClient,
    model: &str,
) -> Result<String> {
    // Build compaction prompt
    let summary_prompt = format!(
        "Summarize this conversation concisely, preserving:\n\
         - All important facts, decisions, and code changes made\n\
         - Current task state and what was accomplished\n\
         - Any errors encountered and how they were resolved\n\
         - File paths and key code snippets\n\n\
         Conversation:\n{conversation}",
        conversation = format_conversation_for_summary(&context.messages)
    );

    // Call API with a small model (haiku) for summary
    let summary = call_summary_api(api_client, "claude-haiku-4-5", &summary_prompt).await?;

    // Replace conversation with summary as a single user message
    context.messages = vec![
        Message::user(format!(
            "[Conversation summary - previous context compacted]\n\n{summary}"
        ))
    ];

    Ok(summary)
}
```

### Step 3: Auto-Compact in Agent Loop

```rust
// In piko-agent/agent_loop.rs, at top of each turn:
if context.needs_compaction(model_context_limit) {
    // notify TUI
    output.emit(AgentEvent::Compacting).await;

    match compact_conversation(&mut context, &api_client, &model).await {
        Ok(summary) => {
            output.emit(AgentEvent::CompactionDone { tokens_freed: freed }).await;
        }
        Err(e) => {
            // log error but continue — don't break the session
            output.emit(AgentEvent::Text(format!("Warning: auto-compact failed: {e}"))).await;
        }
    }
}
```

### Step 4: Manual `/compact` Command

Existing `/compact` command triggers the same `compact_conversation()` function on demand, regardless of token usage.

Enhanced output: show before/after token counts:
```
Compacting conversation...
Summary: "We analyzed the auth module and fixed the token expiry bug.
          Modified auth/jwt.rs lines 45-67..."
Context reduced from 45,230 → 1,840 tokens.
```

### Step 5: TUI Feedback

New `AgentEvent` variants:
```rust
AgentEvent::Compacting,
AgentEvent::CompactionDone { summary_preview: String, tokens_freed: u32 },
```

TUI shows:
```
⚡ Auto-compacting context (85% full)...
✓ Context compacted. Freed 43,390 tokens.
```

---

## Micro-Compact (Partial Compaction)

When only the older half of the conversation is summarized — keeping recent messages intact for context continuity. Used when the conversation is large but still has room.

```rust
pub async fn micro_compact(
    context: &mut ConversationContext,
    api_client: &AnthropicClient,
) -> Result<()> {
    let midpoint = context.messages.len() / 2;
    let old_messages = &context.messages[..midpoint];
    let recent_messages = &context.messages[midpoint..];

    // Summarize only the older half
    let summary = summarize_messages(old_messages, api_client).await?;

    // Replace with: [summary message] + [recent messages]
    context.messages = vec![
        Message::user(format!("[Earlier context summarized]\n\n{summary}"))
    ];
    context.messages.extend_from_slice(recent_messages);
    Ok(())
}
```

Micro-compact triggers at 70% context usage; full compact triggers at 85%.

| Usage | Action |
|-------|--------|
| < 70% | No action |
| 70–85% | Micro-compact (old half summarized) |
| > 85% | Full compact (entire history summarized) |

## Edge Cases

- Compaction API call itself costs tokens — use cheap model (haiku) for summaries
- If compaction call fails: continue with full context, warn user
- Compaction during tool execution: wait until between turns (at top of loop)
- Very short conversations: don't compact if < 1000 tokens (not worth it)
- Multiple rapid turns filling context: compact once, then continue — don't compact on every turn
- System prompt + CLAUDE.md alone fills most context (pathological case): warn user, can't compact further
- Token estimate uses `actual_input_tokens` from last API response (most accurate); fall back to char-count estimate for new messages not yet sent
