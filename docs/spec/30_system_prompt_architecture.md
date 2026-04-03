# Spec: System Prompt Architecture

**Status**: 🔶 Partial — base prompt exists; multi-section structure and dynamic tool injection missing
**Rust crate**: `piko-agent`, `piko-config`
**TS source**: `constants/prompts.ts`, `constants/systemPromptSections.ts`, `context.ts`

---

## Overview

The system prompt is not a single static string — it's assembled dynamically from multiple sections at the start of each session. Sections include base instructions, tool descriptions, CLAUDE.md content, memory, and dynamic context.

---

## System Prompt Assembly Order

```
[1] Base instructions        ← hardcoded, describes agent behavior
[2] Tool guidance            ← hints about how to use specific tools
[3] Cwd context              ← current working directory info
[4] CLAUDE.md (global)       ← ~/.claude/CLAUDE.md
[5] CLAUDE.md (project)      ← <cwd>/CLAUDE.md
[6] Rules files              ← <cwd>/.claude/rules/*.md (sorted)
[7] Memory                   ← relevant memory files from memdir
[8] cache_control marker     ← ephemeral cache set on last block
```

---

## Section Specifications

### [1] Base Instructions

Core behavioral guidelines. In TS this is a large multi-section string in `constants/prompts.ts`. Key points it covers:

- Role: you are an interactive CLI coding assistant
- Tool use guidelines: prefer precise edits over rewrites, verify before acting
- Safety: don't delete files without confirmation, don't run destructive commands without approval
- Communication style: concise, no unnecessary preamble
- Code quality: follow existing conventions, don't add unsolicited improvements
- Security: never introduce vulnerabilities

This should be a static `const SYSTEM_PROMPT_BASE: &str` in `piko-agent`.

### [2] Tool Guidance (dynamic)

Per-tool hints injected based on which tools are active. Example hints:

```
# Tool Guidelines

## bash
- Always provide a `description` parameter explaining what the command does
- For long-running commands, set `timeout` appropriately
- Prefer non-interactive commands; use `-y` flags where available

## file_edit
- Always read the file first to understand existing code before editing
- Make the smallest change needed; don't rewrite unrelated sections

## glob / grep
- Use grep with specific patterns before reading whole files
- Use glob to discover file structure before searching content
```

These are assembled from a `HashMap<&str, &str>` keyed on tool name, including only tools that are registered for the current session.

### [3] Working Directory Context

```
# Working Directory
Current directory: /home/user/myproject
```

Injected so the model knows where it's operating. Derived from `AgentConfig.cwd`.

### [4–6] CLAUDE.md Content

See [10_config_claudemd.md](10_config_claudemd.md) for loading logic. Content is appended verbatim after a `# Project Instructions` header.

### [7] Memory

See [16_memory_memdir.md](16_memory_memdir.md). Relevant memory entries appended after a `# Memory` header.

### [8] Cache Control Marker

The last block of the system array gets `cache_control: { type: "ephemeral" }`. This is the most important cache boundary — everything up to this point is cached after the first request.

---

## Rust Implementation

### Current State

Currently `piko-agent` builds the system prompt as a flat string. The CLAUDE.md content is appended but the structure is ad-hoc.

### Target Structure

```rust
pub struct SystemPromptBuilder {
    sections: Vec<SystemSection>,
}

pub struct SystemSection {
    pub header: Option<String>,
    pub content: String,
}

impl SystemPromptBuilder {
    pub fn new() -> Self { ... }
    pub fn add_base_instructions(&mut self) { ... }
    pub fn add_tool_guidance(&mut self, tool_names: &[&str]) { ... }
    pub fn add_cwd_context(&mut self, cwd: &Path) { ... }
    pub fn add_claudemd(&mut self, content: &str, label: &str) { ... }
    pub fn add_memory(&mut self, entries: &[MemoryEntry]) { ... }
    pub fn build_api_blocks(&self) -> Vec<SystemBlock> {
        // Returns Vec of {type: "text", text: ...} blocks
        // Last block gets cache_control: ephemeral
    }
}
```

### API Format

The Anthropic API accepts `system` as either a string or an array of content blocks:

```json
"system": [
  { "type": "text", "text": "base instructions..." },
  { "type": "text", "text": "# Project Instructions\n..." },
  { "type": "text", "text": "# Memory\n...", "cache_control": { "type": "ephemeral" } }
]
```

Using the array form lets us put `cache_control` on the last block specifically, maximizing cache hits.

---

## Todos

- [ ] Extract `SYSTEM_PROMPT_BASE` constant into a dedicated file (`piko-agent/src/prompt.rs`)
- [ ] Implement `SystemPromptBuilder` with structured sections
- [ ] Add per-tool guidance hints
- [ ] Switch `MessagesRequest.system` from `Option<String>` to `Option<SystemContent>` where `SystemContent = String | Vec<SystemBlock>`
- [ ] Ensure cache_control is placed on the last system block (not hardcoded position)
- [ ] Add `--system-prompt-dump` CLI flag for debugging (prints assembled system prompt and exits)
