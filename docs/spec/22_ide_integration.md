# Spec: IDE Integration

**Status**: ❌ Todo
**TS source**: `server/`, `hooks/useIDEIntegration.ts`, `hooks/useIdeSelection.ts`

---

## Overview

IDE integration allows PikoClaw to communicate bidirectionally with VS Code, JetBrains, and other editors:
- Editor sends selected code / file context to PikoClaw
- PikoClaw opens diffs inline in the editor
- PikoClaw can navigate the editor to specific files/lines

This is implemented via a local WebSocket/HTTP server that IDEs connect to.

---

## Architecture

```
IDE Plugin ←──WebSocket──→ PikoClaw local server (port 9000)
                           ↑
                    piko-server crate
                    running alongside TUI
```

The IDE plugin (VS Code extension / JetBrains plugin) connects to PikoClaw's local server and exchanges JSON messages.

---

## Protocol Messages

### IDE → PikoClaw

```json
// User selected code in editor
{
  "type": "selection",
  "file": "/path/to/file.rs",
  "start_line": 10,
  "end_line": 25,
  "content": "fn foo() {\n    ...\n}"
}

// User opened a file
{
  "type": "file_opened",
  "file": "/path/to/file.rs"
}

// User @-mentioned the IDE
{
  "type": "at_mention",
  "text": "@IDE selection",
  "selection": { ... }
}
```

### PikoClaw → IDE

```json
// Open a file in editor
{
  "type": "open_file",
  "file": "/path/to/file.rs",
  "line": 42
}

// Show a diff in editor
{
  "type": "show_diff",
  "file": "/path/to/file.rs",
  "original": "...",
  "modified": "..."
}

// Status update
{
  "type": "status",
  "state": "thinking" | "ready" | "running_tool"
}
```

---

## Implementation Plan

### Step 1: Local Server (`piko-server` crate or module)

```rust
// Start a local HTTP/WebSocket server
pub async fn start_ide_server(port: u16, event_tx: mpsc::Sender<IdeEvent>) {
    // axum or warp HTTP server
    // WebSocket upgrade at /ws
    // REST endpoints for IDE to push context
}
```

### Step 2: IdeEvent Type

```rust
pub enum IdeEvent {
    SelectionChanged { file: String, start_line: u32, end_line: u32, content: String },
    FileOpened { file: String },
    AtMention { text: String, selection: Option<Selection> },
    Connected { ide_name: String },
    Disconnected,
}
```

### Step 3: Context Injection

When IDE sends a selection, inject it into the next user message as context:

```
[IDE Context: auth/jwt.rs lines 10-25]
```rust
fn validate_token(token: &str) -> Result<Claims> {
    ...
}
```

### Step 4: `@IDE` Mention in Input

Allow user to type `@IDE` or `@selection` in the input bar to include current IDE selection in message.

### Step 5: Diff Display in IDE

After `FileEditTool` runs, send the diff to the IDE:

```rust
// In PostToolUse hook for file_edit:
if let Some(ide_client) = &context.ide_client {
    ide_client.show_diff(&file_path, &original, &modified).await;
}
```

---

## Configuration

```toml
[ide]
enabled = true
port = 9000    # local server port
auto_inject_selection = true   # inject IDE selection automatically
```

---

## VS Code Extension

Separate repo. Extension:
1. Connects to `ws://localhost:9000/ws` on activation
2. Listens for selection changes, sends to PikoClaw
3. Handles `show_diff` messages: shows diff in VS Code's diff editor
4. Handles `open_file` messages: opens file in editor
5. Shows PikoClaw status in VS Code status bar

---

## Priority

Low priority for initial releases — useful for power users with VS Code/JetBrains but not core functionality. Implement after core features are stable.
