# Spec: Memory / Memdir System

**Status**: ❌ Todo
**TS source**: `memdir/`, `services/SessionMemory/`, `services/autoDream/`, `utils/memory.ts`

---

## Overview

The memory system gives PikoClaw persistent long-term memory across sessions. Memory is stored as Markdown files in a directory (`memdir`), loaded into the system prompt when relevant, and periodically consolidated ("dreaming") to stay concise.

---

## Components

### 1. Memory Directory (memdir)

```
~/.config/pikoclaw/memory/       ← global memory (all projects)
<cwd>/.pikoclaw/memory/          ← project-specific memory
```

Each memory file is a Markdown document with YAML frontmatter:

```markdown
---
name: user_preferences
description: User's coding style preferences
type: user        # user | feedback | project | reference
created: 2025-01-15
updated: 2025-01-20
---

The user prefers snake_case for Rust variables.
They like concise explanations without preamble.
Always use early returns over nested if blocks.
```

### 2. Context Injection

On each turn, relevant memory files are loaded into the system prompt:

```
System prompt:
  [base instructions]
  [CLAUDE.md content]

  # Memory
  [relevant memory file 1 content]
  [relevant memory file 2 content]
```

**Relevance scoring**: simple keyword/embedding match between current conversation context and memory file descriptions.

### 3. Memory CRUD

The agent can write memory via a dedicated tool or via the `/memory` command.

### 4. Auto-Consolidation ("Dreaming")

When session is idle (no activity for N minutes), run a background consolidation pass:
- Review all memory files
- Merge duplicates
- Remove stale/contradicted entries
- Summarize verbose entries

---

## Implementation Plan

### Step 1: Memory File I/O

```rust
// In piko-config or new piko-memory crate
pub struct MemoryEntry {
    pub name: String,
    pub description: String,
    pub memory_type: MemoryType,  // User, Feedback, Project, Reference
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub body: String,
}

pub enum MemoryType { User, Feedback, Project, Reference }

pub fn load_memory_dir(path: &Path) -> Vec<MemoryEntry>
pub fn save_memory_entry(path: &Path, entry: &MemoryEntry) -> Result<()>
pub fn delete_memory_entry(path: &Path, name: &str) -> Result<()>
```

### Step 2: Context Loading

```rust
// In piko-agent, when building system prompt:
pub fn load_relevant_memories(
    memory_dir: &Path,
    conversation_context: &str,
    max_tokens: usize,
) -> Vec<MemoryEntry>
```

Simple relevance: load all entries, sort by `updated` desc, take top N that fit in `max_tokens`.
Future: TF-IDF or embedding similarity against `description` field.

### Step 3: Memory Write Tool

```rust
// In piko-tools: new MemoryWriteTool
Input: {
    action: "write" | "delete",
    path: "user" | "project" | String,  // path or category
    name: String,        // file name (slug)
    description: String, // used for relevance matching
    memory_type: "user" | "feedback" | "project" | "reference",
    body: String,        // markdown content
}
Output: String (confirmation)
```

### Step 4: `/memory` Slash Command

```
/memory list             → show all memory files with names and descriptions
/memory show <name>      → display full content of a memory file
/memory delete <name>    → delete a memory file
/memory edit <name>      → open memory file in $EDITOR
```

### Step 5: MEMORY.md Index

Maintain a `MEMORY.md` index file in the memory directory:
```markdown
# Memory Index

- [user_preferences.md](user_preferences.md) — User's coding style preferences
- [project_context.md](project_context.md) — Current project goals and constraints
```

Loaded first on every session (small, fast); individual files loaded as needed.

---

## Auto-Consolidation ("Dreaming")

Optional background task. After session is idle for 10+ minutes:

1. Load all memory files
2. Build a consolidation prompt:
   ```
   Review these memory entries and:
   - Merge entries that cover the same topic
   - Remove entries that are contradicted by newer entries
   - Shorten verbose entries
   - Keep all unique facts

   Return the cleaned memory files in the same format.
   ```
3. Send to API (low-budget, haiku model)
4. Write back updated files

This runs at most once per session, in a background Tokio task.

---

## Edge Cases

- Memory files should be < 10KB each (enforce limit)
- Total memory injected into system prompt: max 4000 tokens
- If no memory directory exists: skip silently (memory is opt-in by usage)
- Concurrent writes: use file locking (`.lock` file) to avoid corruption
