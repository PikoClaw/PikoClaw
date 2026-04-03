# Spec: MCP (Model Context Protocol) Integration

**Status**: 🔶 Partial — tool bridge done; resource reading missing; auth missing
**Rust crate**: `piko-mcp`
**TS source**: `services/mcp/`

---

## Overview

MCP (Model Context Protocol) allows external programs ("MCP servers") to expose tools and resources to PikoClaw. PikoClaw connects as an MCP client, discovers the server's tools, and bridges them into the agent's tool registry.

---

## What's Implemented

### Transport Layer ✅
- [x] **Stdio transport** — spawn subprocess, communicate via stdin/stdout JSON-RPC
  - Command: `command` + `args` + optional `env` from config
  - Newline-delimited JSON messages
- [x] **SSE transport** — connect to HTTP SSE endpoint
  - POST requests for JSON-RPC calls
  - GET for event stream (server push)

### Protocol ✅
- [x] JSON-RPC 2.0 request/response/notification structs
- [x] `initialize` handshake (client sends capabilities, server responds with server info + capabilities)
- [x] `tools/list` — get list of tools the server exposes
- [x] `tools/call` — invoke a tool by name with JSON arguments
- [x] Error handling for JSON-RPC errors and transport failures

### Tool Bridge ✅
- [x] `McpTool` — wraps an MCP server tool as a `Tool` trait implementation
- [x] Tool definitions (name, description, input schema) converted from MCP format to PikoClaw format
- [x] Tool calls proxied to MCP server, result returned as string
- [x] All MCP server tools auto-registered into agent's `ToolRegistry` on connection
- [x] Multiple MCP servers supported simultaneously (each in own client)

### Config ✅
- [x] `McpConfig` in `config.toml` with `servers` map
- [x] `McpServerConfig`: `transport` (Stdio or SSE), command/args/env or URL
- [x] Servers connected on agent startup

---

## Config Format (Reference)

```toml
[mcp.servers.filesystem]
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]

[mcp.servers.my-api]
transport = "sse"
url = "http://localhost:3000/mcp"
```

---

## Not Yet Implemented

### MCP Resources ❌
MCP servers can expose "resources" (files, data, URLs) in addition to tools. Resources are listed and read via separate protocol calls.
See [28_mcp_resources.md](28_mcp_resources.md)

- `resources/list` → list available resources (uri, name, mimeType)
- `resources/read` → read resource content by URI
- `resources/subscribe` → subscribe to resource changes (optional)
- Bridge to `ListMcpResourcesTool` and `ReadMcpResourceTool`

### MCP Authentication ❌
Some MCP servers require OAuth or API key authentication.
- OAuth flow for MCP servers (browser-based login)
- Bearer token injection into SSE transport headers
- Token refresh handling

### MCP Prompts ❌
MCP servers can also expose "prompts" — pre-defined message templates.
- `prompts/list` → list available prompt templates
- `prompts/get` → retrieve a prompt template with arguments
- Not critical for core use cases

### Dynamic MCP Server Management ❌
- `/mcp add <name> <command>` — add server at runtime without restarting
- `/mcp remove <name>` — disconnect and remove server
- `/mcp list` — show connected servers and their tool counts
- `/mcp status` — show health of each connection

### Server Reconnection ❌
- Automatic reconnection when stdio process crashes
- Backoff retry for SSE connection failures
- Health check ping (optional, MCP extension)

### Tool Schema Validation ❌
- Validate tool call inputs against MCP server's JSON schema before sending
- Return structured error if validation fails (instead of sending invalid request to server)

### Namespacing ❌
- TS prefixes MCP tool names with server name (e.g. `filesystem__read_file`) to avoid collisions
- Rust currently uses bare tool names, which may collide if two servers expose same-named tools
- Implementation: when registering MCP tools, prefix with `{server_name}__{tool_name}`
