# Spec: Git Worktree Tools

**Status**: ❌ Todo
**TS source**: `tools/EnterWorktreeTool.tsx`, `tools/ExitWorktreeTool.tsx`

---

## Overview

Worktree tools let the agent create an isolated git worktree for experimental changes. The agent works in the worktree; the user can review and merge back, or discard entirely. This prevents the main working tree from being polluted by exploratory edits.

---

## What Worktrees Provide

A git worktree is a second checkout of the repo at a different path, on a new branch. Changes in the worktree don't affect the original checkout. When done, the worktree can be:
- Kept and merged back via PR
- Deleted (discards all changes)

---

## Tool Specs

### EnterWorktreeTool

```
Input: {
    description?: string,   // human description of what this worktree is for
    branch?: string         // branch name to create (auto-generated if not set)
}

Output: {
    worktree_path: string,  // absolute path to new worktree
    branch_name: string     // created branch name
}
```

**Behavior**:
1. Check if cwd is inside a git repository (`git rev-parse --show-toplevel`)
2. Generate a branch name if not provided: `pikoclaw/{timestamp}-{slug-of-description}`
3. Run: `git worktree add <worktree_path> -b <branch_name>`
4. Worktree path: `<repo_root>/../<repo_name>-worktrees/<branch_name>` or temp dir
5. Update agent's `cwd` to `worktree_path`
6. Return `{ worktree_path, branch_name }`

### ExitWorktreeTool

```
Input: {
    cleanup: bool    // true = delete worktree (discard changes), false = keep
}

Output: string (confirmation)
```

**Behavior**:
1. If `cleanup: true`:
   - Run `git worktree remove --force <worktree_path>`
   - Delete the branch: `git branch -D <branch_name>`
   - Restore agent's `cwd` to original
2. If `cleanup: false`:
   - Run `git worktree remove <worktree_path>` (keeps branch, just detaches)
   - Restore agent's `cwd` to original
   - Show user: "Worktree changes saved to branch `<branch_name>`"

---

## Implementation Plan

### Step 1: Worktree State in Agent Context

```rust
// In piko-agent/context.rs
pub struct WorktreeState {
    pub worktree_path: PathBuf,
    pub branch_name: String,
    pub original_cwd: PathBuf,
}

pub struct ConversationContext {
    // existing fields...
    pub worktree: Option<WorktreeState>,
}
```

### Step 2: EnterWorktreeTool Implementation

```rust
async fn execute(&self, input: EnterWorktreeInput, context: &ToolContext) -> ToolResult {
    let repo_root = run_git(&["rev-parse", "--show-toplevel"], &context.cwd).await?;
    let branch = input.branch.unwrap_or_else(|| generate_branch_name(&input.description));
    let worktree_path = generate_worktree_path(&repo_root, &branch);

    run_git(&["worktree", "add", &worktree_path, "-b", &branch], &context.cwd).await?;

    context.set_worktree(WorktreeState {
        worktree_path: worktree_path.clone(),
        branch_name: branch.clone(),
        original_cwd: context.cwd.clone(),
    });
    context.set_cwd(worktree_path.clone());

    ToolResult::success_json(json!({
        "worktree_path": worktree_path,
        "branch_name": branch
    }))
}
```

### Step 3: ExitWorktreeTool Implementation

```rust
async fn execute(&self, input: ExitWorktreeInput, context: &ToolContext) -> ToolResult {
    let state = context.worktree.take().ok_or("Not in a worktree")?;
    context.set_cwd(state.original_cwd);

    if input.cleanup {
        run_git(&["worktree", "remove", "--force", &state.worktree_path], &context.cwd).await?;
        run_git(&["branch", "-D", &state.branch_name], &context.cwd).await?;
        ToolResult::success("Worktree deleted and changes discarded.")
    } else {
        run_git(&["worktree", "remove", &state.worktree_path], &context.cwd).await?;
        ToolResult::success(format!(
            "Exited worktree. Changes preserved on branch `{}`.",
            state.branch_name
        ))
    }
}
```

### Step 4: TUI Indicator

Status bar: show `[worktree: branch-name]` when agent is in a worktree.

---

## Edge Cases

- Not a git repo: return helpful error "Not inside a git repository"
- Uncommitted changes in worktree when exiting with `cleanup: false`: warn user but allow
- Worktree path collision: if generated path exists, append a counter
- Agent spawned in worktree should have worktree's `cwd` from the start
- Worktree cleanup on session exit: if session ends with an active worktree, warn user that worktree still exists at path X on branch Y
