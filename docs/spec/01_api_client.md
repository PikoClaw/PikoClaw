# Spec: Anthropic API Client & Streaming

**Status**: ✅ Done
**Rust crate**: `piko-api`
**TS source**: `services/api/claude.ts`, `services/api/client.ts`, `services/api/errors.ts`

---

## Overview

The API client handles all communication with the Anthropic Messages API, including SSE streaming, error classification, and retry logic.

---

## What's Implemented

- [x] `AnthropicClient` struct with configurable base URL and API key
- [x] `messages_stream()` — POST `/v1/messages` with `stream: true`, returns `EventStream`
- [x] SSE line parsing (`parse_sse_line`) for `event:` / `data:` interleaving
- [x] `StreamEvent` enum: `MessageStart`, `ContentBlockStart`, `ContentBlockDelta`, `ContentBlockStop`, `MessageDelta`, `MessageStop`, `Ping`, `Error`
- [x] `MessagesRequest` — full request body: model, max_tokens, messages, system, tools, stream, betas
- [x] Request headers: `anthropic-version: 2023-06-01`, `x-api-key`, `content-type`
- [x] Rate limit handling: 429 → extract `retry-after`, back off
- [x] Overload handling: 529 → treat same as rate limit
- [x] `ApiError` enum: `RateLimit`, `Overload`, `Auth`, `BadRequest`, `Server`, `Network`
- [x] Beta header injection for experimental features (e.g. `web-search-20250305`)
- [x] `MessagesResponse` and `StopReason` types

---

## Spec / Technical Details

### Request Structure

```json
{
  "model": "claude-opus-4-6",
  "max_tokens": 8192,
  "stream": true,
  "system": "...",
  "messages": [...],
  "tools": [...],
  "betas": ["web-search-20250305"]
}
```

### SSE Event Flow

```
event: message_start      → capture usage (input_tokens, cache_creation, cache_read)
event: content_block_start → type: text | tool_use
event: content_block_delta → text_delta.text | input_json_delta.partial_json
event: content_block_stop
event: message_delta       → stop_reason, output_tokens
event: message_stop
```

### Error HTTP Codes

| Code | Meaning | Action |
|------|---------|--------|
| 401 | Auth failure | Surface to user immediately |
| 429 | Rate limited | Read `retry-after` header, wait |
| 529 | Overloaded | Same as 429 |
| 500/503 | Server error | Retry with backoff (max 3x) |

### Beta Headers (currently used)

- `web-search-20250305` — enables native web search tool
- `interleaved-thinking-2025-05-14` — extended thinking (not yet wired up in agent)
- `prompt-caching-2024-07-31` — cache_control blocks (implicit, no header needed in newer API)

---

## Nothing To Do

This crate is complete. Future work would only be adding new beta headers as Anthropic releases them.
