# Spec: Built-in Tools

**Status**: ✅ Done
**Rust crate**: `piko-tools`
**TS source**: `tools/` directory

---

## Overview

The 8 core built-in tools that every agent session has access to by default.

---

## What's Implemented

### BashTool ✅
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

### FileReadTool ✅
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

### FileWriteTool ✅
- [x] Write/overwrite a file completely
- [x] Creates parent directories if needed
- [x] Returns confirmation with file path

**TS spec**: `tools/FileWriteTool.tsx`
```
Input: { file_path: string, content: string }
Output: string (success message)
```

### FileEditTool ✅
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

### GlobTool ✅
- [x] Match files by glob pattern (e.g. `**/*.rs`)
- [x] Respects `.gitignore` (uses `ignore` crate)
- [x] Optional `path` to restrict search root
- [x] Returns sorted list of matching paths (by modification time)

**TS spec**: `tools/GlobTool.ts`
```
Input: { pattern: string, path?: string }
Output: string[] (file paths)
```

### GrepTool ✅
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

### WebFetchTool ✅
- [x] HTTP GET a URL
- [x] HTML → plain text conversion (strip tags)
- [x] Configurable `max_length` (default 20000 chars)
- [x] Returns page content as string

**TS spec**: `tools/WebFetchTool.tsx`
```
Input: { url: string, prompt?: string, max_length?: number }
Output: string (page content)
```

### WebSearchTool ✅
- [x] Passed through to Anthropic's native `web_search_20250305` beta tool
- [x] No local HTTP handling — the API executes the search
- [x] Requires `betas: ["web-search-20250305"]` in request

**TS spec**: `tools/WebSearchTool.tsx`
```
Input: { query: string }
Output: handled natively by API
```

---

## Gaps / Todos

- [ ] **PowerShellTool** — Windows-only shell tool (`pwsh -Command`). Low priority for macOS/Linux focus.
- [ ] **BashTool restart** — TS has a concept of "restarting" persistent bash sessions after timeout. Rust currently just spawns a fresh process each call. For now acceptable.
- [ ] **FileReadTool PDF support** — TS reads PDFs via a native library, extracts text per page. Rust currently returns raw bytes or error.
- [ ] **FileReadTool image display** — TS encodes images as base64 for multimodal display. Rust does not support this yet. See [15_image_input.md](15_image_input.md).
- [ ] **GrepTool multiline** — TS supports multiline regex matching across line boundaries. Not yet in Rust.
