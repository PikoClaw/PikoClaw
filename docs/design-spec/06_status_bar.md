# Design Spec: Status Bar

**TS source**: `components/StatusBar.tsx` (or similar), `bootstrap/state.ts`

---

## Layout

```
claude-opus-4-6  ·  ~/projects/myapp  ·  ↑18.9k ↓2.1k ⚡3.4k  $0.042  ·  dark
```

Single line, full terminal width. Fields separated by ` · ` (space + bullet operator `∙` U+2219 + space).

### Field order (left → right)

| # | Field | Always shown? | Example |
|---|-------|--------------|---------|
| 1 | Model name | Yes | `claude-opus-4-6` |
| 2 | Current working directory | Yes | `~/projects/myapp` |
| 3 | Token usage | Yes | `↑18.9k ↓2.1k ⚡3.4k` |
| 4 | Session cost | Yes (when > $0.001) | `$0.042` |
| 5 | Active theme | Yes | `dark` |
| 6 | Rate limit bar | Only when rate-limited | `5h: ████░ 78%` |
| 7 | Vim mode | Only when vim mode on | `[NORMAL]` or `[INSERT]` |
| 8 | Plan mode | Only when plan mode on | `[PLAN]` |

---

## Token Display Format

```
↑18.9k ↓2.1k ⚡3.4k
```

| Symbol | Meaning | Color |
|--------|---------|-------|
| `↑` | Input tokens sent (cumulative session) | `theme.text` |
| `↓` | Output tokens received | `theme.text` |
| `⚡` | Cache-read tokens (cheap, 10% of input cost) | `theme.suggestion` (blue) |

**Number formatting**:
- < 1,000: show exact (`342`)
- ≥ 1,000: show with `k` suffix, 1 decimal (`1.3k`, `18.9k`)
- ≥ 1,000,000: show with `M` suffix (`1.2M`)

**Context usage color coding** (applied to the whole token section):

| Usage % | Color |
|---------|-------|
| 0–49% | `theme.text` (normal) |
| 50–79% | `theme.warning` (amber) |
| 80–89% | `theme.warning` bold |
| ≥ 90% | `theme.error` (red) |

When context is near-full, a compact bar may appear:

```
↑18.9k ↓2.1k ⚡3.4k  [████████░░ 82%]
```

---

## Cost Display Format

```
$0.042
```

| Range | Format | Example |
|-------|--------|---------|
| < $0.001 | hidden | — |
| $0.001–$0.999 | `$X.XXX` | `$0.042` |
| $1.00–$9.99 | `$X.XX` | `$3.24` |
| ≥ $10.00 | `$XX.XX` | `$12.50` |

Color: `theme.inactive` normally. `theme.warning` if > $5, `theme.error` if > $20.

---

## Rate Limit Bar

Only shown when the user has hit a usage tier limit.

```
5h: ████████░░  78%
```

- `5h` / `7d` — the window being limited (5-hour or 7-day)
- Filled bar: `theme.rate_limit_fill` (`rgb(87,105,247)` blue)
- Empty bar: `theme.rate_limit_empty` (`rgb(39,47,111)` dark blue)
- Percentage: exact number
- Bar width: 10 characters (`█` × filled + `░` × remaining)

---

## Model Name Display

Show the model identifier, cleaned up for display:

| API model ID | Display |
|-------------|---------|
| `claude-opus-4-6` | `claude-opus-4-6` |
| `claude-sonnet-4-6` | `claude-sonnet-4-6` |
| `claude-haiku-4-5-20251001` | `claude-haiku-4-5` |

Strip trailing date suffixes for display. Color: `theme.text`.

---

## CWD Display

- Full absolute path, but replace `$HOME` with `~`
- If path is very long (> 30 chars) and terminal is narrow: abbreviate middle dirs with `…`

```
~/projects/myapp/src/components   →   ~/…/src/components  (if needed)
```

---

## Vim Mode Indicator

When vim mode is enabled, show current mode after the theme field:

| Mode | Display | Color |
|------|---------|-------|
| Insert | `[INSERT]` | `theme.suggestion` (blue) |
| Normal | `[NORMAL]` | `theme.warning` (amber) |
| Visual | `[VISUAL]` | `theme.claude` (orange) |

---

## Plan Mode Indicator

```
[PLAN MODE]
```

Color: `theme.plan_mode` (teal `rgb(0,102,102)`). Shown as a badge.

---

## Full Example (various states)

**Normal session:**
```
claude-opus-4-6  ·  ~/projects/myapp  ·  ↑18.9k ↓2.1k ⚡3.4k  $0.042  ·  dark
```

**Rate limited:**
```
claude-sonnet-4-6  ·  ~/myapp  ·  ↑5.2k ↓0.8k  $0.008  ·  dark  ·  5h: ████████░░ 78%
```

**High context usage:**
```
claude-opus-4-6  ·  ~/myapp  ·  ↑168k ↓12k ⚡42k  $2.84  [████████░░ 90%]  ·  dark
```
(token section turns red at 90%)

**Vim normal mode + plan mode:**
```
claude-opus-4-6  ·  ~/myapp  ·  ↑3.1k ↓0.4k  $0.006  ·  dark  ·  [PLAN MODE]  [NORMAL]
```

---

## Rust Implementation

```rust
pub struct StatusBarState {
    pub model: String,
    pub cwd: PathBuf,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cost_usd: f64,
    pub context_limit: u64,
    pub theme_name: String,
    pub rate_limit: Option<RateLimitState>,
    pub vim_mode: Option<VimMode>,
    pub plan_mode: bool,
}

pub struct RateLimitState {
    pub window: String,   // "5h" or "7d"
    pub used_pct: u8,     // 0–100
}

impl Widget for StatusBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // build fields as Span vec, join with " · " separators
        // apply color coding for context usage
        // render into single Line
    }
}
```
