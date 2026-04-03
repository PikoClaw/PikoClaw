# Spec: Hooks System

**Status**: ❌ Todo
**TS source**: `hooks/` (hook execution), `utils/hooks.ts`, spec: `claurst/spec/07_hooks.md`

---

## Overview

Hooks let users run custom shell commands in response to agent events. For example: play a sound when the agent finishes, run a linter after every file edit, or post a Slack message when a bash command fails.

---

## Hook Types

### PreToolUse
Runs **before** a tool is executed. Can block the tool call.

```
Trigger:  agent is about to call a tool
Input:    tool_name, tool_input (JSON)
Output:   exit code 0 = proceed, non-zero = block tool (output shown as error)
```

### PostToolUse
Runs **after** a tool completes.

```
Trigger:  tool has finished executing
Input:    tool_name, tool_input (JSON), tool_result (JSON)
Output:   exit code ignored (fire-and-forget)
          stdout injected back into conversation as a system message (optional)
```

### Stop
Runs when the agent finishes its turn (reaches `end_turn`).

```
Trigger:  agent turn complete
Input:    session_id, turn_count, last_message_preview
Output:   exit code ignored
          stdout injected as next user message if non-empty (allows chaining)
```

### Notification
Runs when a notification event is emitted (e.g. permission needed, agent waiting for input).

```
Trigger:  notification event
Input:    event_type, message
Output:   exit code ignored
```

---

## Config Format

Hooks defined in `~/.config/pikoclaw/config.toml`:

```toml
[[hooks.pre_tool_use]]
tool_name = "bash"           # optional: only run for specific tool
command = "/usr/local/bin/my-pre-bash-hook"

[[hooks.post_tool_use]]
tool_name = "file_edit"
command = "prettier --write $PIKOCLAW_TOOL_INPUT_FILE_PATH"

[[hooks.stop]]
command = "say 'Claude is done'"   # macOS text-to-speech

[[hooks.notification]]
command = "terminal-notifier -message \"$PIKOCLAW_NOTIFICATION_MESSAGE\""
```

---

## Environment Variables Passed to Hook

| Variable | Value |
|----------|-------|
| `PIKOCLAW_TOOL_NAME` | Name of the tool being called |
| `PIKOCLAW_TOOL_INPUT` | JSON string of tool input |
| `PIKOCLAW_TOOL_RESULT` | JSON string of tool result (PostToolUse only) |
| `PIKOCLAW_SESSION_ID` | Current session UUID |
| `PIKOCLAW_CWD` | Current working directory |
| `PIKOCLAW_NOTIFICATION_MESSAGE` | Notification text (Notification hooks only) |

---

## Implementation Plan

### Step 1: Data Structures

```rust
// In piko-config
pub struct HookConfig {
    pub pre_tool_use: Vec<HookEntry>,
    pub post_tool_use: Vec<HookEntry>,
    pub stop: Vec<HookEntry>,
    pub notification: Vec<HookEntry>,
}

pub struct HookEntry {
    pub tool_name: Option<String>,  // None = match all tools
    pub command: String,            // shell command to run
}
```

### Step 2: Hook Runner

```rust
// New crate: piko-hooks (or add to piko-agent)
pub async fn run_hook(entry: &HookEntry, env: HashMap<String, String>) -> HookResult {
    // spawn: /bin/sh -c {command}
    // set environment variables
    // capture stdout + exit code
    // timeout: 30s (configurable)
}

pub struct HookResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}
```

### Step 3: Integration Points

In `piko-agent/agent_loop.rs`:

```rust
// Before tool execution:
if let Some(hooks) = config.hooks.pre_tool_use.matching(tool_name) {
    let result = run_hook(hook, env).await;
    if result.exit_code != 0 {
        // block tool, return error result
        return ToolResult::error(result.stdout);
    }
}

// After tool execution:
for hook in config.hooks.post_tool_use.matching(tool_name) {
    let result = run_hook(hook, env).await;
    if !result.stdout.is_empty() {
        // inject stdout as system message into conversation
    }
}

// After turn end_turn:
for hook in config.hooks.stop {
    let result = run_hook(hook, env).await;
    if !result.stdout.is_empty() {
        // inject as next user message (enables agent chaining)
    }
}
```

### Step 4: TUI Feedback
- Show hook execution in TUI with dim styling: `⚙ Running hook: say 'done'`
- Show hook errors inline if pre_tool_use blocks

---

## Edge Cases

- Hook command must not block indefinitely — enforce 30s timeout
- PreToolUse blocking: return tool result with `is_error: true` and hook's stdout as content
- Concurrent hooks: if multiple hooks match, run sequentially (not parallel) to preserve ordering
- Hook failures in PostToolUse and Stop: log but don't propagate error to agent
- Circular hooks: hook that triggers a tool that triggers a hook — detect via `PIKOCLAW_HOOK_DEPTH` env var, refuse to run if depth > 3
