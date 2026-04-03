# Spec: Cost Tracking & Budget Enforcement

**Status**: ❌ Todo — token counts tracked; USD cost calculation and display missing
**Rust crate**: `piko-agent`, `piko-tui`, `piko-skills`
**TS source**: `cost-tracker.ts`, `costHook.ts`, `commands/cost.ts`

---

## Overview

Cost tracking converts raw token counts into USD using a per-model pricing table, accumulates cost across the session, displays it in the TUI, and optionally enforces a budget ceiling.

---

## Model Pricing Table

Prices in USD per million tokens (as of early 2025 — update when Anthropic changes pricing):

| Model | Input $/M | Output $/M | Cache Write $/M | Cache Read $/M |
|-------|-----------|------------|-----------------|----------------|
| `claude-opus-4-6` | $15.00 | $75.00 | $18.75 | $1.50 |
| `claude-sonnet-4-6` | $3.00 | $15.00 | $3.75 | $0.30 |
| `claude-haiku-4-5` | $0.25 | $1.25 | $0.30 | $0.03 |

Cache write is 1.25× input price. Cache read is 0.1× input price.

---

## Cost Calculation

```rust
pub struct ModelPricing {
    pub input_per_m: f64,
    pub output_per_m: f64,
    pub cache_write_per_m: f64,
    pub cache_read_per_m: f64,
}

pub fn calculate_cost(usage: &TokenUsage, pricing: &ModelPricing) -> f64 {
    (usage.input_tokens as f64 / 1_000_000.0) * pricing.input_per_m
    + (usage.output_tokens as f64 / 1_000_000.0) * pricing.output_per_m
    + (usage.cache_creation_tokens as f64 / 1_000_000.0) * pricing.cache_write_per_m
    + (usage.cache_read_tokens as f64 / 1_000_000.0) * pricing.cache_read_per_m
}

pub fn get_pricing(model: &str) -> ModelPricing {
    match model {
        m if m.contains("opus") => OPUS_PRICING,
        m if m.contains("sonnet") => SONNET_PRICING,
        m if m.contains("haiku") => HAIKU_PRICING,
        _ => SONNET_PRICING,  // safe default
    }
}
```

---

## Session Cost Accumulator

```rust
pub struct CostTracker {
    pub total_cost_usd: f64,
    pub total_usage: TokenUsage,
    pub model: String,
    pub turn_costs: Vec<TurnCost>,  // per-turn breakdown
}

pub struct TurnCost {
    pub turn_index: usize,
    pub cost_usd: f64,
    pub usage: TokenUsage,
}

impl CostTracker {
    pub fn record_turn(&mut self, usage: TokenUsage) {
        let pricing = get_pricing(&self.model);
        let cost = calculate_cost(&usage, &pricing);
        self.total_cost_usd += cost;
        self.total_usage += usage;
        self.turn_costs.push(TurnCost { ... });
    }
}
```

---

## TUI Display

### Status Bar

Show cost alongside token counts:

```
↑12.3k ↓2.1k ⚡3.4k  $0.023
```

Format: `$X.XXX` — 3 decimal places (most sessions cost fractions of a cent to a few cents).

For costs ≥ $1.00: show `$X.XX`. For costs ≥ $10.00: show `$XX.XX`.

### `/cost` Slash Command

```
Session Cost Summary
────────────────────────────────────
Model:          claude-opus-4-6
Turns:          12

Token Usage:
  Input:        45,230  →  $0.679
  Output:        8,910  →  $0.668
  Cache write:   3,200  →  $0.060
  Cache read:   18,400  →  $0.028
                         ─────────
  Total:                   $1.435

Savings from cache: $0.248 (compared to no caching)
────────────────────────────────────
```

---

## Budget Enforcement

### CLI Flag

```
pikoclaw --max-budget-usd 5.00
```

### Behavior

When accumulated cost reaches `max_budget_usd`:
1. Finish the current turn (don't cut mid-response)
2. Show warning: `⚠ Budget limit reached ($5.00). Session stopped.`
3. Save session, exit cleanly

### Config

```toml
[api]
max_budget_usd = 10.0   # optional; no limit if unset
```

### Implementation

In agent loop, after each turn:
```rust
if let Some(max) = config.api.max_budget_usd {
    if cost_tracker.total_cost_usd >= max {
        output.emit(AgentEvent::BudgetExceeded { limit: max, actual: cost_tracker.total_cost_usd }).await;
        return Ok(StopReason::BudgetExceeded);
    }
}
```

---

## Todos

- [ ] Add `ModelPricing` table to `piko-agent` or `piko-config`
- [ ] Implement `CostTracker` struct with per-turn accumulation
- [ ] Wire `CostTracker` into agent loop after each `TokenUsage` event
- [ ] Pass cost to TUI via `AgentEvent::TokenUsage { ..., cost_usd: f64 }`
- [ ] Show cost in status bar alongside token counts
- [ ] Implement `/cost` slash command with detailed breakdown
- [ ] Add `--max-budget-usd` CLI flag and `max_budget_usd` config option
- [ ] Budget enforcement check in agent loop
- [ ] Update pricing table when Anthropic announces changes
