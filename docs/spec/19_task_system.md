# Spec: Background Task System

**Status**: ❌ Todo
**TS source**: `tools/TaskCreateTool.tsx`, `tools/TaskGetTool.tsx`, etc., `tasks/`

---

## Overview

The task system allows the agent to spawn **background agents** that run independently and concurrently. The main agent can check on their status, read their output, and stop them. Tasks enable parallelism — the agent can start multiple long-running operations and coordinate results.

---

## Task Lifecycle

```
TaskCreate → Task starts running (async Tokio task)
           → main agent continues
           → later: TaskGet / TaskOutput to check status
           → TaskUpdate to set status (mark completed/failed)
           → TaskStop to cancel if needed
```

## Tool Specs

### TaskCreate

```
Input: {
    prompt: string,           // what the background agent should do
    description?: string,     // human label for TUI display
    subagent_type?: string,   // "general-purpose" | "Explore" | etc.
    tools?: string[],         // restrict available tools
}
Output: {
    task_id: string           // UUID for tracking
}
```

### TaskGet

```
Input: { task_id: string }
Output: {
    task_id: string,
    status: "running" | "completed" | "failed" | "stopped",
    description: string,
    created_at: string,
    output_preview: string    // first N chars of output
}
```

### TaskList

```
Input: {}
Output: TaskInfo[]   // all tasks for this session
```

### TaskOutput

```
Input: { task_id: string }
Output: string    // full accumulated output so far (or final output if done)
```

### TaskUpdate

```
Input: {
    task_id: string,
    status: "completed" | "failed"   // only these two are user-settable
}
Output: string (confirmation)
```

### TaskStop

```
Input: { task_id: string }
Output: string (confirmation)
```

---

## Implementation Plan

### Step 1: Task Registry

```rust
// New module: piko-agent/tasks.rs
use std::collections::HashMap;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

pub struct TaskHandle {
    pub id: String,
    pub description: String,
    pub status: TaskStatus,
    pub created_at: DateTime<Utc>,
    pub output_buffer: Arc<RwLock<String>>,   // accumulated output
    pub cancel_token: CancellationToken,
}

pub enum TaskStatus {
    Running,
    Completed,
    Failed(String),
    Stopped,
}

pub struct TaskRegistry {
    tasks: Arc<RwLock<HashMap<String, TaskHandle>>>,
}

impl TaskRegistry {
    pub async fn create_task(&self, prompt: String, agent_config: AgentConfig) -> String {
        let id = Uuid::new_v4().to_string();
        let output_buffer = Arc::new(RwLock::new(String::new()));
        let cancel_token = CancellationToken::new();

        // spawn background Tokio task
        let buffer_clone = output_buffer.clone();
        let cancel_clone = cancel_token.clone();
        let id_clone = id.clone();
        let registry_clone = self.clone();

        tokio::spawn(async move {
            // run agent loop, write output to buffer
            // update status to Completed/Failed when done
        });

        id
    }
}
```

### Step 2: Tool Implementations

```rust
pub struct TaskCreateTool {
    registry: Arc<TaskRegistry>,
    base_config: AgentConfig,
}

impl Tool for TaskCreateTool {
    async fn execute(&self, input: TaskCreateInput, _ctx: &ToolContext) -> ToolResult {
        let task_id = self.registry.create_task(
            input.prompt,
            self.base_config.clone(),
        ).await;
        ToolResult::success_json(json!({ "task_id": task_id }))
    }
}
```

### Step 3: Output Streaming

Background agents write to their `output_buffer` as they run. `TaskOutput` reads the current buffer contents. This allows the main agent to poll for partial results.

### Step 4: TUI Integration

Show active tasks in a sidebar or inline:
```
● Task: "Analyze authentication code" [running, 45s]
✓ Task: "Search for test files"      [completed]
✗ Task: "Run type checks"            [failed]
```

Tasks panel toggle: `Ctrl+T` or auto-shown when tasks exist.

### Step 5: Task Inheritance

Background tasks inherit from parent:
- Same model
- Same permission settings (or reduced set via `tools` parameter)
- Independent `ConversationContext` (fresh conversation)
- Same `TaskRegistry` reference (can spawn sub-tasks)

---

## Concurrency Notes

- Tasks run as Tokio tasks on the same async runtime — not separate OS threads
- Max concurrent tasks: configurable, default 5
- If max reached: `TaskCreate` blocks until a slot opens (or returns error)
- Each task has its own cancellation token
- When main session is cancelled: all tasks cancelled too (via parent `CancellationToken`)

---

## Edge Cases

- Task tries to interact with user (AskUserQuestion): should either be blocked (with error: "Background tasks cannot ask user questions") or queue to main TUI
- Task output is unbounded in size: enforce max buffer size (default 1MB), truncate oldest output
- Task ID collision: use UUID v4, collision probability negligible
- Resume session: tasks from previous session are not resumed (tasks are ephemeral, not persisted)
