# Spec: Advanced Tools

**Status**: ✅ Done (core set + plan mode); ❌ Missing (task system, worktrees, cron)
**Rust crate**: `piko-tools`, `piko-agent`
**TS source**: `tools/AgentTool.tsx`, `tools/NotebookEditTool.tsx`, `tools/AskUserQuestionTool.tsx`, `tools/TodoWriteTool.tsx`

---

## Overview

Tools beyond the basic file/shell set: agent spawning, notebook editing, user interaction, and task management.

---

## What's Implemented

### AgentTool ✅
- [x] Spawns isolated child agent with own tool registry and conversation context
- [x] Child agent inherits parent's config (model, permissions, cwd)
- [x] Child runs full agent loop independently
- [x] Returns child agent's final response as tool result
- [x] `bypass_permissions` flag — child bypasses permission checks when parent has approved

**TS spec**: `tools/AgentTool.tsx`
```
Input: {
  prompt: string,
  description?: string,
  tools?: string[],       // restrict which tools child can use
  subagent_type?: string  // named agent type (general-purpose, Explore, Plan, etc.)
}
Output: string (child agent's final response)
```

### NotebookEditTool ✅
- [x] Edit Jupyter `.ipynb` files (JSON format)
- [x] Operations: `replace_cell`, `insert_cell`, `delete_cell`
- [x] Cell types: `code`, `markdown`, `raw`
- [x] Preserves notebook metadata and output cells

**TS spec**: `tools/NotebookEditTool.tsx`
```
Input: {
  notebook_path: string,
  operation: "replace_cell" | "insert_cell" | "delete_cell",
  cell_index: number,
  cell_type?: "code" | "markdown" | "raw",
  source?: string
}
Output: string (confirmation)
```

### AskUserQuestionTool ✅
- [x] Pauses agent loop, presents question to user in TUI
- [x] Numbered options list (multi-choice)
- [x] Free-text fallback (user types answer)
- [x] Returns user's chosen option or typed text as tool result

**TS spec**: `tools/AskUserQuestionTool.tsx`
```
Input: {
  question: string,
  options?: string[]
}
Output: string (user's answer)
```

### TodoWriteTool ✅
- [x] In-session task checklist (not persisted across sessions)
- [x] States: `pending`, `in_progress`, `completed`
- [x] Replaces the full todo list on each call (not incremental)
- [x] Displayed in TUI status or sidebar

**TS spec**: `tools/TodoWriteTool.tsx`
```
Input: {
  todos: Array<{ id: string, content: string, status: "pending" | "in_progress" | "completed", priority: "high" | "medium" | "low" }>
}
Output: string (confirmation)
```

---

## Not Yet Implemented

### EnterPlanModeTool / ExitPlanModeTool ❌
Read-only "plan" mode: agent may only read files and call tools that don't modify state. No bash writes, no file writes. User must explicitly approve exiting plan mode.
See: [17_plan_mode.md](17_plan_mode.md)

```
EnterPlanMode: Input: {} → Output: string (confirmation, sets plan_mode=true on context)
ExitPlanMode:  Input: {} → Output: string (asks user approval in TUI)
```

### TaskCreateTool / TaskGetTool / TaskListTool / TaskUpdateTool / TaskOutputTool / TaskStopTool ❌
Background task system (V2). Tasks run as independent agents in background threads/processes.
See: [19_task_system.md](19_task_system.md)

```
TaskCreate: { prompt, description?, subagent_type? } → { task_id }
TaskGet:    { task_id }  → TaskInfo { id, status, created_at }
TaskList:   {}           → TaskInfo[]
TaskUpdate: { task_id, status } → confirmation
TaskOutput: { task_id }  → string (streamed output so far)
TaskStop:   { task_id }  → confirmation
```

### EnterWorktreeTool / ExitWorktreeTool ❌
Creates a temporary git worktree for isolated changes. Agent works in the worktree; changes can be merged back or discarded.
See: [18_worktrees.md](18_worktrees.md)

```
EnterWorktree: { description?, branch? } → { worktree_path, branch_name }
ExitWorktree:  { cleanup: bool }         → confirmation (optionally deletes worktree)
```

### ListMcpResourcesTool / ReadMcpResourceTool ❌
Exposes MCP server resources (files, data) as tools.
See: [28_mcp_resources.md](28_mcp_resources.md)

```
ListMcpResources: { server_name? } → resource list
ReadMcpResource:  { server_name, uri } → resource content
```

### SkillTool ❌
Invokes user-defined skills (markdown prompt templates) from within agent context. Different from slash commands — this is callable by the agent itself.

```
Input: { skill: string, args?: string }
Output: string (result of skill execution)
```

### CronCreateTool / CronDeleteTool / CronListTool ❌
Schedule recurring agent tasks.
See: [27_cron_scheduler.md](27_cron_scheduler.md)

### RemoteTriggerTool ❌
Trigger remote scheduled agents.
See: [27_cron_scheduler.md](27_cron_scheduler.md)
