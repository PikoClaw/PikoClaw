# Spec: Bridge & Remote Session

**Status**: ❌ Todo (not planned for near term)
**TS source**: `bridge/`, `remote/`

---

## Overview

The bridge system connects PikoClaw to the claude.ai web UI, allowing users to start, monitor, and interact with PikoClaw sessions from their browser. This is a cloud-synchronization feature requiring Anthropic's backend infrastructure.

---

## What the Bridge Does

```
claude.ai web UI ←──────────────── Anthropic Bridge API
                                          ↑
                                   Bridge polling
                                          ↑
                                   PikoClaw (local)
```

1. PikoClaw registers with Anthropic's Bridge API
2. Bridge API gives PikoClaw a session ID + work queue
3. User starts session from claude.ai → work item appears in queue
4. PikoClaw pulls work, executes, streams results back
5. User sees results live in claude.ai

---

## Key Bridge Components in TS

- `bridge/bridgeMain.ts` — main bridge loop (poll → execute → ack)
- `bridge/bridgeApi.ts` — REST client for Anthropic's bridge API
- `bridge/replBridge.ts` — in-process bridge for REPL mode
- `bridge/jwtUtils.ts` — JWT session token encoding
- `bridge/workSecret.ts` — CCR v2 work secret decoding
- `bridge/sessionRunner.ts` — spawn child processes for bridge sessions
- `bridge/bridgeMessaging.ts` — serialization of bridge messages

---

## Why Not Implemented

The bridge requires:
1. Anthropic's private Bridge API (not public)
2. OAuth/JWT authentication with claude.ai
3. A cloud account and session synchronization infrastructure

This is tightly coupled to Anthropic's internal systems and cannot be reimplemented independently. PikoClaw as an independent project cannot replicate this without Anthropic's cooperation.

---

## Alternative: Local Web UI

Instead of claude.ai bridge, we could implement a **local web UI** that PikoClaw serves:

```
Browser (localhost:8080) ←──WebSocket──→ PikoClaw local HTTP server
```

This would allow:
- Browser-based chat UI at `http://localhost:8080`
- Mobile access on same network
- No dependency on claude.ai

### Implementation Sketch

```rust
// New crate: piko-webui
// Axum HTTP server serving:
//   GET /         → static HTML/JS chat UI
//   GET /ws       → WebSocket for real-time streaming
//   POST /message → send a message (REST fallback)
//   GET /sessions → list sessions

pub async fn start_webui_server(port: u16, agent_factory: AgentFactory) {
    // serve embedded static assets
    // WebSocket handler that proxies to agent loop
}
```

This would be a much more valuable feature for PikoClaw users than replicating the claude.ai bridge.

---

## Recommendation

Skip the claude.ai bridge entirely. Consider the local web UI as a future premium feature.
Mark this spec as **out of scope** unless Anthropic opens the Bridge API.
