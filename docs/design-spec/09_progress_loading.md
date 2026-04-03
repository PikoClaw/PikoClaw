# Design Spec: Progress & Loading States

**TS source**: `components/Spinner.tsx`, `constants/spinnerVerbs.ts`, `constants/figures.ts`

---

## Spinner Frames

### Standard spinner (braille dots)

Used for agent thinking and tool execution.

```
Frames (cycle at ~100ms):
⠋  ⠙  ⠹  ⠸  ⠼  ⠴  ⠦  ⠧  ⠇  ⠏
```

Color: `theme.spinner` (`rgb(87,105,247)` blue), shimmer pulses to `theme.spinner_shimmer` (`rgb(117,135,255)`) every 500ms.

### Bridge/connection spinner

Used for MCP server connections or IDE connection status.

```
Frames: ·|·  ·/·  ·─·  ·\·
Ready:  ·✔·
Failed: ·✗·
```

---

## Tool Execution States

### In progress

```
⠙ bash · git diff src/auth/jwt.rs                        2.3s
```

Layout:
- `spinner` — braille frame, `theme.spinner` with shimmer
- ` ` — space
- `tool_name` — `theme.text` bold
- ` · ` — `theme.inactive`
- `input_preview` — `theme.inactive`, max 60 chars, truncated with `…`
- right-aligned elapsed time — `theme.inactive`

### Elapsed time format

| Elapsed | Format |
|---------|--------|
| < 1s | `0.Xs` (e.g. `0.3s`) |
| 1–59s | `Xs` (e.g. `2s`, `45s`) |
| 1–59m | `Xm Ys` (e.g. `1m 23s`) |
| ≥ 1h | `Xh Ym` (e.g. `1h 02m`) |

---

## Agent Thinking States

While the agent is streaming its response (no tool use yet):

```
⠙ Thinking...
```

With effort level indicator (when `--effort` flag is set):

```
◉ Thinking...     ← Opus, max effort
●  Thinking...    ← High effort
◐  Thinking...    ← Medium effort
○  Thinking...    ← Low effort
```

Effort symbols:
- `○` U+25CB — low (hollow circle)
- `◐` U+25D0 — medium (half circle)
- `●` U+25CF — high (filled circle)
- `◉` U+25C9 — max (bullseye, Opus only)

---

## Spinner Verbs

The spinner label cycles through verbs to feel dynamic. Randomly selected from:

```
Thinking · Pondering · Analyzing · Processing · Reasoning
Calculating · Considering · Contemplating · Evaluating
Examining · Exploring · Figuring out · Investigating
Planning · Reflecting · Reviewing · Studying · Working on it
```

Verb changes every ~3 seconds (not every tick). Seeded by session ID for consistency.

---

## Streaming Text

As assistant text arrives from the API, it renders character-by-character.

No explicit animation — text simply appears as it streams in. The cursor (`▋` or `_`) is shown at the current write position:

```
The issue is in the `validate_token` functi▋
```

Cursor character: `▋` (U+258B left half block) or simple `_` underscore.
Cursor color: `theme.text` — blinks at ~500ms if terminal supports it.

---

## Progress Bars

Used for file operations, compaction, and context usage display.

### Filled bar characters

```
Full block:   █   U+2588
Light shade:  ░   U+2591
```

### Usage bar (10 chars wide)

```
[████████░░]  78%
```

```
Width 10: filled = floor(pct * 10 / 100), empty = 10 - filled
Example 78%: 7 filled + 3 empty → ████████░░  (note: ████████ = 7, ░░░ = 3... adjust)
```

Wait — correct formula:
```
filled = round(pct / 10)  → 78% → 8 chars filled, 2 empty
████████░░
```

Colors:
- Filled: `theme.rate_limit_fill` (blue) for rate limits, `theme.success` (green) for progress
- Empty: `theme.rate_limit_empty` (dark blue) for rate limits, `theme.inactive` (gray) for progress

---

## Compact / Summarization Progress

```
⚡ Summarizing conversation...
```

Then on complete:

```
✓ Context compacted: 45,230 → 1,840 tokens (freed 43,390)
```

- `⚡` `theme.suggestion` (blue)
- `✓` `theme.success` (green)
- Token numbers: bold
- `freed N` part: `theme.success` (green)

---

## MCP Connection Progress

On startup, when connecting to MCP servers:

```
⠙ Connecting to MCP servers...
  ✓  filesystem   (3 tools)
  ✓  github       (12 tools)
  ✗  my-server    connection refused
```

- MCP server list shown below the spinner
- `✓` `theme.success` per connected server
- `✗` `theme.error` per failed server
- Tool count: `theme.inactive`
- Transitions to normal TUI once all connections settled (success or failure)

---

## Welcome Screen Progress

On first launch, before the TUI is shown:

```
Starting PikoClaw...
  Loading config
  Connecting to API
  Loading skills (3 found)
  Ready!
```

Simple text output, not ratatui — written directly to stdout before the TUI takes over. Each step appears as it completes. `Ready!` in `theme.success` green.

---

## Rust Notes

```rust
// Spinner tick management
pub struct SpinnerState {
    pub frame: usize,
    pub last_tick: Instant,
    pub shimmer: bool,
    pub verb: &'static str,
    pub verb_last_change: Instant,
}

impl SpinnerState {
    pub fn tick(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_tick).as_millis() >= 100 {
            self.frame = (self.frame + 1) % SPINNER_FRAMES.len();
            self.last_tick = now;
        }
        if now.duration_since(self.verb_last_change).as_secs() >= 3 {
            self.verb = pick_random_verb();
            self.verb_last_change = now;
        }
        self.shimmer = (now.elapsed().as_millis() / 500) % 2 == 0;
    }

    pub fn current_frame(&self) -> &'static str {
        SPINNER_FRAMES[self.frame]
    }

    pub fn current_color(&self, theme: &Theme) -> Color {
        if self.shimmer { theme.spinner_shimmer } else { theme.spinner }
    }
}

const SPINNER_FRAMES: &[&str] = &[
    "⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"
];
```
