# Design Spec: Status Bar

**TS source**: `components/StatusBar.tsx` (or similar), `bootstrap/state.ts`

---

## Layout

```
> · ↑18.9k ↓2.1k · $0.042 · ↑82%                                 pikoclaw [dark]
```

Single line, full terminal width. Left side contains live session state; right side shows app name and active theme.

### Field order (left → right)

| # | Field | Always shown? | Example |
|---|-------|--------------|---------|
| 1 | Current spinner / prompt glyph | Yes | `>` or animated thinking glyph |
| 2 | Token usage | Yes | `↑18.9k ↓2.1k` |
| 3 | Session cost | Yes | `$0.042` |
| 4 | Context usage | Yes | `↑82%` |
| 5 | Rate limit countdown | Only when rate-limited | `⏳2m14s` |
| 6 | Plan mode marker | Only when plan mode on | `[PLAN]` |
| 7 | Brand + theme | Yes | `pikoclaw [dark]` |

---

## Token Display Format

`↑input ↓output`

| Symbol | Meaning | Color |
|--------|---------|-------|
| `↑` | Input tokens sent (cumulative session) | `theme.inactive` |
| `↓` | Output tokens received | `theme.inactive` |

**Number formatting**:
- < 1,000: show exact (`342`)
- ≥ 1,000: show with `k` suffix, 1 decimal (`1.3k`, `18.9k`)
- ≥ 1,000,000: show with `M` suffix (`1.2M`)

## Cost Display Format

| Range | Format | Example |
|-------|--------|---------|
| < $0.001 | `$0.000` style may still be shown if session has started | `$0.000` |
| $0.001–$0.999 | `$X.XXX` | `$0.042` |
| $1.00–$9.99 | `$X.XX` | `$3.24` |
| ≥ $10.00 | `$XX.XX` | `$12.50` |

Color: `theme.inactive`.

---

## Context Usage

The status bar shows a compact context usage percentage based on input tokens:

`↑82%`

Color:

| Usage % | Color |
|---------|-------|
| 0–79% | `theme.claude` |
| 80–89% | `theme.warning` |
| ≥ 90% | `theme.error` |

---

## Plan Mode Indicator

`[PLAN]`

Color: yellow/bold in the current Rust implementation.

---

## Rate Limit Indicator

Only shown when the app has a live retry window after a 429/529-style condition.

Format:

`⏳2m14s`

Color: `theme.warning`

---

## Full Example (various states)

**Normal session:**
```
> · ↑18.9k ↓2.1k · $0.042 · ↑37%                                 pikoclaw [dark]
```

**Rate limited:**
```
> · ↑5.2k ↓0.8k · $0.008 · ↑16% · ⏳2m14s                        pikoclaw [dark]
```

**High context usage:**
```
> · ↑168k ↓12k · $2.84 · ↑90%                                   pikoclaw [dark]
```
(context usage turns red at 90%)

**Plan mode:**
```
> · ↑3.1k ↓0.4k · $0.006 · ↑11% · [PLAN]                        pikoclaw [dark]
```

---

## Rust Implementation

```rust
pub struct StatusBarState {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub context_percent: u8,
    pub theme_name: String,
    pub rate_limit_remaining: Option<Duration>,
    pub plan_mode: bool,
}

impl Widget for StatusBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // build left spans and a right-aligned app/theme label
        // render into single Line
    }
}
```
