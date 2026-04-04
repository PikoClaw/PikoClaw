# Spec: Plan Mode

**Status**: ✅ Done
**TS source**: `tools/EnterPlanModeTool.tsx`, `tools/ExitPlanModeTool.tsx`, `commands/plan.ts`

---

## Overview

Plan mode is a read-only agent mode. When active, the agent can read files and analyze code, but cannot execute bash commands or write/edit files. The user must explicitly approve exiting plan mode before the agent can make changes.

This allows the agent to safely "think through" a problem and present a plan before touching anything.

---

## Behavior

### Entering Plan Mode
- Agent calls `EnterPlanModeTool`
- Context flag `plan_mode: true` set on agent
- Subsequent tool calls that mutate state are blocked with a helpful error
- TUI shows `[PLAN MODE]` indicator in status bar

### Blocked Tools in Plan Mode
The following tools are **blocked** when plan mode is active:
- `bash` (any command — could have side effects)
- `file_write`
- `file_edit`
- `notebook_edit`

The following tools are **allowed** in plan mode:
- `file_read`
- `glob`
- `grep`
- `web_fetch`
- `web_search`
- `ask_user_question`
- `todo_write` (read-only nature, just updates in-memory list)
- `agent` (sub-agents inherit plan mode restrictions)

### Exiting Plan Mode
- Agent calls `ExitPlanModeTool`
- TUI shows a confirmation dialog: `Agent wants to exit plan mode and make changes. Allow? [y/n]`
- User approves → `plan_mode: false`, execution resumes
- User denies → agent continues in plan mode, receives denial as tool result

---

## Implementation Plan

### Step 1: Add Plan Mode to Agent Context

```rust
// In piko-agent/context.rs
pub struct ConversationContext {
    pub messages: Vec<Message>,
    pub system_prompt: String,
    pub plan_mode: bool,   // new field
}
```

### Step 2: EnterPlanModeTool

```rust
// In piko-tools
pub struct EnterPlanModeTool;

impl Tool for EnterPlanModeTool {
    fn name() -> &'static str { "enter_plan_mode" }
    fn description() -> &'static str {
        "Enter plan mode. In plan mode, you can read files and analyze code \
         but cannot execute commands or modify files. Use this to think through \
         a solution before making changes."
    }
    // Input: {} (no params)
    async fn execute(&self, _input: (), context: &ToolContext) -> ToolResult {
        context.set_plan_mode(true);
        ToolResult::success("Entered plan mode. You can now read and analyze without making changes.")
    }
}
```

### Step 3: ExitPlanModeTool

```rust
pub struct ExitPlanModeTool;

impl Tool for ExitPlanModeTool {
    fn name() -> &'static str { "exit_plan_mode" }
    // Input: {} (no params)
    async fn execute(&self, _input: (), context: &ToolContext) -> ToolResult {
        // Send PlanModeExitRequest event to TUI
        // Wait for user approval
        match context.ask_plan_mode_exit().await {
            UserDecision::Allow => {
                context.set_plan_mode(false);
                ToolResult::success("Exited plan mode. You can now make changes.")
            }
            UserDecision::Deny => {
                ToolResult::error("User declined to exit plan mode. Continue planning.")
            }
        }
    }
}
```

### Step 4: Block Mutations in Permission Checker

In `piko-permissions/checker.rs`, add plan mode check:

```rust
pub async fn check(&self, request: PermissionRequest, context: &AgentContext) -> PermissionDecision {
    if context.plan_mode && MUTATING_TOOLS.contains(&request.tool_name) {
        return PermissionDecision::Deny;
        // Tool result will be: "Cannot execute in plan mode. Call exit_plan_mode first."
    }
    // ... normal permission flow
}

const MUTATING_TOOLS: &[&str] = &["bash", "file_write", "file_edit", "notebook_edit"];
```

### Step 5: TUI Integration

- Status bar: show `[PLAN MODE]` badge when `plan_mode: true`
- Plan mode exit dialog (new `AppState::AskingPlanModeExit`):
  ```
  Agent wants to exit plan mode and begin making changes.
  Allow? [y]es / [n]o
  ```
- Style the badge in a distinct color (yellow/amber)

### Step 6: `/plan` Slash Command

```
/plan        → toggle plan mode on/off (convenience shortcut for users)
```

---

## Edge Cases

- Sub-agents spawned in plan mode should also be in plan mode
- If agent tries to call a blocked tool, return error with clear message: `"This tool is blocked in plan mode. Call exit_plan_mode to proceed."`
- User can also manually toggle `/plan` to force-exit plan mode without agent requesting it
- Plan mode state should NOT be persisted across session resume (always starts in non-plan mode)
