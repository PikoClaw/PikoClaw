# Spec: Configurable Output Styles

**Status**: ❌ Todo
**TS source**: `outputStyles/`, `constants/outputStyles.ts`

---

## Overview

Output styles control how PikoClaw renders its responses — the verbosity, formatting, and visual density of tool results, status messages, and assistant text.

---

## Output Style Modes (from TS)

| Style | Description |
|-------|-------------|
| `default` | Full output, all details shown |
| `minimal` | Compact: no tool headers, brief status |
| `verbose` | Extra detail: raw tool inputs/outputs |
| `json` | Machine-readable JSON stream (for piping) |
| `plain` | No ANSI colors, no special formatting |
| `markdown` | Markdown-formatted output (for docs/pipes) |

---

## What Changes Per Style

### Tool Display

**default**:
```
⚙ bash: git status
  On branch main
  nothing to commit
```

**minimal**:
```
$ git status → On branch main, nothing to commit
```

**verbose**:
```
┌ Tool: bash ──────────────────────────────────────────────┐
│ Input: { "command": "git status", "timeout": 120 }       │
│ Output:                                                    │
│   On branch main                                          │
│   nothing to commit, working tree clean                   │
│ Exit code: 0 | Duration: 0.12s                           │
└───────────────────────────────────────────────────────────┘
```

**json** (for piping):
```json
{"type":"tool_start","name":"bash","input":{"command":"git status"}}
{"type":"tool_result","name":"bash","output":"On branch main\n...","exit_code":0}
{"type":"text","content":"The repo is clean."}
```

### Status Bar

- `minimal`: hide status bar entirely
- `default`: show model + tokens
- `verbose`: show all token categories

---

## Implementation Plan

### Step 1: OutputStyle Enum

```rust
// In piko-config or piko-tui
pub enum OutputStyle {
    Default,
    Minimal,
    Verbose,
    Json,
    Plain,   // no ANSI
    Markdown,
}
```

### Step 2: Style-Aware Rendering

Pass `OutputStyle` to the TUI renderer and `OutputSink`:

```rust
pub trait OutputSink: Send {
    async fn emit(&self, event: AgentEvent);
    fn style(&self) -> OutputStyle;
}
```

In `TuiOutputSink::emit()`, check `self.style()` and render accordingly.

### Step 3: JSON Output Mode

For `--output-format json` (non-interactive / pipe mode):

```rust
pub struct JsonOutputSink {
    writer: BufWriter<io::Stdout>,
}

impl OutputSink for JsonOutputSink {
    async fn emit(&self, event: AgentEvent) {
        let json = match event {
            AgentEvent::Text(t) => json!({"type":"text","content":t}),
            AgentEvent::ToolStart { name, input } => json!({"type":"tool_start","name":name,"input":input}),
            AgentEvent::ToolComplete { name, result } => json!({"type":"tool_result","name":name,"output":result}),
            AgentEvent::Done => json!({"type":"done"}),
            _ => return,
        };
        writeln!(self.writer, "{}", json).ok();
    }
}
```

### Step 4: Config & CLI

```toml
[tui]
output_style = "default"   # default | minimal | verbose
```

CLI: `--output-style minimal` or `--output-format json`

### Step 5: `/output-style` Slash Command

```
/output-style minimal   → switch to minimal
/output-style verbose   → switch to verbose
/output-style           → show current style
```

---

## Priority

Medium. `json` output mode is particularly useful for scripting and integration. `minimal` is good for experienced users. Implement after core features.
