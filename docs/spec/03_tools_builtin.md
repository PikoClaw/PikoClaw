# Spec: Built-in Tools

**Status**: ✅ Done
**Rust crate**: `piko-tools`
**TS source**: `tools/` directory

---

## Overview

The built-in tools that every agent session has access to by default, organized into five groups: File & Shell, Web, Tasks & Scheduling, Agent & Session, and Git Worktrees.

---

## What's Implemented

### File & Shell

#### BashTool ✅
- [x] Execute arbitrary shell commands via `/bin/bash -c`
- [x] Configurable timeout (default 120s, max 600s)
- [x] Returns stdout, stderr, exit code as structured output
- [x] Working directory from `AgentConfig.cwd`
- [x] Timeout kill (SIGKILL after timeout)

**TS spec**: `tools/BashTool.tsx`
```
Input: { command: string, timeout?: number, description?: string }
Output: { stdout, stderr, exit_code, timed_out }
```

#### PowerShellTool ✅
- [x] Execute PowerShell commands (`pwsh -Command` on macOS/Linux, `powershell.exe` on Windows)
- [x] Windows-native shell support
- [x] Returns stdout, stderr, exit code

```
Input: { command: string, timeout?: number }
Output: { stdout, stderr, exit_code }
```

#### FileReadTool ✅
- [x] Read file contents with line numbers (`cat -n` style)
- [x] `offset` — skip first N lines
- [x] `limit` — return at most N lines
- [x] Returns content as string with `line_num\tcontent` format
- [x] Error if file not found

**TS spec**: `tools/FileReadTool.tsx`
```
Input: { file_path: string, offset?: number, limit?: number }
Output: string (numbered lines)
```

#### FileWriteTool ✅
- [x] Write/overwrite a file completely
- [x] Creates parent directories if needed
- [x] Returns confirmation with file path

**TS spec**: `tools/FileWriteTool.tsx`
```
Input: { file_path: string, content: string }
Output: string (success message)
```

#### FileEditTool ✅
- [x] Exact string replacement (old_string → new_string)
- [x] `replace_all: bool` flag for global replace
- [x] Fails if `old_string` not found in file
- [x] Fails if `old_string` appears multiple times and `replace_all` is false
- [x] Returns diff-style output of changed region

**TS spec**: `tools/FileEditTool.tsx`
```
Input: { file_path: string, old_string: string, new_string: string, replace_all?: bool }
Output: string (confirmation with line ranges changed)
```

#### ApplyPatchTool ✅
- [x] Apply unified diff patches to files
- [x] Supports standard `diff -u` patch format

```
Input: { patch: string }
Output: string (success message or error)
```

#### BatchEditTool ✅
- [x] Apply multiple file edits atomically in a single call
- [x] Each edit is an `old_string` → `new_string` replacement on a target file
- [x] All edits applied or none (atomic semantics)

```
Input: { edits: Array<{ file_path, old_string, new_string, replace_all? }> }
Output: string (confirmation)
```

#### GlobTool ✅
- [x] Match files by glob pattern (e.g. `**/*.rs`)
- [x] Respects `.gitignore` (uses `ignore` crate)
- [x] Optional `path` to restrict search root
- [x] Returns sorted list of matching paths (by modification time)

**TS spec**: `tools/GlobTool.ts`
```
Input: { pattern: string, path?: string }
Output: string[] (file paths)
```

#### GrepTool ✅
- [x] Regex search across files
- [x] Optional `path` (directory or file)
- [x] Optional `glob` filter (e.g. `*.ts`)
- [x] Optional `type` filter (e.g. `rust`, `js`)
- [x] Case-insensitive flag (`-i`)
- [x] Context lines (`-A`, `-B`, `-C`)
- [x] Output modes: `files_with_matches` (default), `content`, `count`
- [x] `head_limit` — truncate results to N lines/files
- [x] `offset` — skip first N entries
- [x] Respects `.gitignore`

**TS spec**: `tools/GrepTool.ts`
```
Input: { pattern, path?, glob?, type?, -i?, -A?, -B?, -C?, output_mode?, head_limit?, offset? }
Output: string (matching lines or file paths)
```

#### NotebookEditTool ✅
- [x] Edit Jupyter notebook cells (`.ipynb`)
- [x] Insert, replace, or delete cells by index
- [x] Supports code and markdown cell types

```
Input: { notebook_path: string, cell_index: number, new_source: string, cell_type?: string }
Output: string (confirmation)
```

---

### Web

#### WebFetchTool ✅
- [x] HTTP GET a URL
- [x] HTML → plain text conversion (strip tags)
- [x] Configurable `max_length` (default 20000 chars)
- [x] Returns page content as string

**TS spec**: `tools/WebFetchTool.tsx`
```
Input: { url: string, prompt?: string, max_length?: number }
Output: string (page content)
```

#### WebSearchTool ✅
- [x] Passed through to Anthropic's native `web_search_20250305` beta tool
- [x] No local HTTP handling — the API executes the search
- [x] Requires `betas: ["web-search-20250305"]` in request

**TS spec**: `tools/WebSearchTool.tsx`
```
Input: { query: string }
Output: handled natively by API
```

---

### Tasks & Scheduling

#### TaskCreate ✅
- [x] Spawn a background task (sub-agent or shell job)
- [x] Returns a task ID for subsequent polling

```
Input: { description: string, prompt: string }
Output: { task_id: string }
```

#### TaskGet ✅
- [x] Get task details and current status by ID

```
Input: { task_id: string }
Output: { task_id, status, progress?, result? }
```

#### TaskUpdate ✅
- [x] Update a task's status or progress message

```
Input: { task_id: string, status?: string, progress?: string }
Output: string (confirmation)
```

#### TaskList ✅
- [x] List all tasks in the current session with their statuses

```
Input: {}
Output: Array<{ task_id, description, status }>
```

#### TaskStop ✅
- [x] Stop a running task by ID

```
Input: { task_id: string }
Output: string (confirmation)
```

#### TaskOutput ✅
- [x] Retrieve stdout/logs from a background task

```
Input: { task_id: string }
Output: string (task output / logs)
```

#### CronCreate ✅
- [x] Schedule a recurring cron task with a cron expression

```
Input: { expression: string, prompt: string, description?: string }
Output: { cron_id: string }
```

#### CronDelete ✅
- [x] Cancel a scheduled cron task by ID

```
Input: { cron_id: string }
Output: string (confirmation)
```

#### CronList ✅
- [x] List all scheduled cron tasks and their next run times

```
Input: {}
Output: Array<{ cron_id, expression, description, next_run }>
```

#### SleepTool ✅
- [x] Pause execution for a specified duration

```
Input: { duration_ms: number }
Output: string (confirmation after wait)
```

---

### Agent & Session

#### AskUserQuestion ✅
- [x] Ask the user a free-text or multiple-choice question
- [x] Blocks until the user responds

```
Input: { question: string, options?: string[] }
Output: string (user's answer)
```

#### TodoWrite ✅
- [x] In-session task checklist visible to the user
- [x] Create, update, and complete checklist items

```
Input: { todos: Array<{ content: string, status: string, priority: string, id: string }> }
Output: string (confirmation)
```

#### Brief ✅
- [x] Send a formatted status message to the user without blocking

```
Input: { message: string }
Output: string (confirmation)
```

#### StructuredOutput ✅
- [x] Return structured JSON output for SDK or non-interactive sessions

```
Input: { output: object }
Output: JSON string
```

#### SendMessage ✅
- [x] Send a message to another named agent in the session

```
Input: { to: string, message: string }
Output: string (delivery confirmation)
```

#### RemoteTrigger ✅
- [x] Dispatch a named event to another session

```
Input: { session_id: string, event: string, payload?: object }
Output: string (confirmation)
```

#### ToolSearch ✅
- [x] Search available tools by name or keyword
- [x] Supports `select:<name>` exact lookup and keyword ranking

```
Input: { query: string, max_results?: number }
Output: tool schemas matching the query
```

---

### Git Worktrees

#### EnterWorktree ✅
- [x] Create a new git worktree and switch the session into it
- [x] Supports optional branch name

```
Input: { path: string, branch?: string }
Output: string (worktree path confirmation)
```

#### ExitWorktree ✅
- [x] Exit the current worktree session and restore the original working directory

```
Input: {}
Output: string (confirmation)
```

---

## Gaps / Todos

- [ ] **FileReadTool PDF support** — TS reads PDFs via a native library, extracts text per page. Rust currently returns raw bytes or error.
- [ ] **FileReadTool image display** — TS encodes images as base64 for multimodal display. Rust does not support this yet. See [15_image_input.md](15_image_input.md).
- [ ] **GrepTool multiline** — TS supports multiline regex matching across line boundaries. Not yet in Rust.
- [ ] **BashTool restart** — TS has a concept of "restarting" persistent bash sessions after timeout. Rust currently just spawns a fresh process each call. For now acceptable.
