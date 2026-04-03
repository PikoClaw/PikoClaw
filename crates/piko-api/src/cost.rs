use serde::{Deserialize, Serialize};

/// Prices in USD per million tokens.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ModelPricing {
    pub input_per_m: f64,
    pub output_per_m: f64,
    pub cache_write_per_m: f64,
    pub cache_read_per_m: f64,
}

pub const OPUS_PRICING: ModelPricing = ModelPricing {
    input_per_m: 15.00,
    output_per_m: 75.00,
    cache_write_per_m: 18.75,
    cache_read_per_m: 1.50,
};

pub const SONNET_PRICING: ModelPricing = ModelPricing {
    input_per_m: 3.00,
    output_per_m: 15.00,
    cache_write_per_m: 3.75,
    cache_read_per_m: 0.30,
};

pub const HAIKU_PRICING: ModelPricing = ModelPricing {
    input_per_m: 0.25,
    output_per_m: 1.25,
    cache_write_per_m: 0.30,
    cache_read_per_m: 0.03,
};

/// Look up pricing by model identifier string.
pub fn get_pricing(model: &str) -> ModelPricing {
    if model.contains("opus") {
        OPUS_PRICING
    } else if model.contains("haiku") {
        HAIKU_PRICING
    } else {
        SONNET_PRICING
    }
}

/// Calculate the USD cost for a single API response.
pub fn calculate_cost(usage: &crate::response::Usage, pricing: &ModelPricing) -> f64 {
    (usage.input_tokens as f64 / 1_000_000.0) * pricing.input_per_m
        + (usage.output_tokens as f64 / 1_000_000.0) * pricing.output_per_m
        + (usage.cache_creation_input_tokens as f64 / 1_000_000.0) * pricing.cache_write_per_m
        + (usage.cache_read_input_tokens as f64 / 1_000_000.0) * pricing.cache_read_per_m
}

/// Convenience: compute cost directly from raw token counts.
pub fn calculate_cost_raw(
    input_tokens: u32,
    output_tokens: u32,
    cache_creation_tokens: u32,
    cache_read_tokens: u32,
    pricing: &ModelPricing,
) -> f64 {
    (input_tokens as f64 / 1_000_000.0) * pricing.input_per_m
        + (output_tokens as f64 / 1_000_000.0) * pricing.output_per_m
        + (cache_creation_tokens as f64 / 1_000_000.0) * pricing.cache_write_per_m
        + (cache_read_tokens as f64 / 1_000_000.0) * pricing.cache_read_per_m
}

/// Accumulated session cost.
#[derive(Debug, Clone, Default)]
pub struct CostTracker {
    pub total_cost_usd: f64,
    pub turns: Vec<TurnCost>,
}

#[derive(Debug, Clone)]
pub struct TurnCost {
    pub turn_index: usize,
    pub cost_usd: f64,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub cache_creation_tokens: u32,
    pub cache_read_tokens: u32,
}

impl CostTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_turn(
        &mut self,
        pricing: &ModelPricing,
        input_tokens: u32,
        output_tokens: u32,
        cache_creation_tokens: u32,
        cache_read_tokens: u32,
    ) {
        let cost = calculate_cost_raw(
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
            pricing,
        );
        let turn = TurnCost {
            turn_index: self.turns.len(),
            cost_usd: cost,
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
        };
        self.total_cost_usd += cost;
        self.turns.push(turn);
    }
}

/// Format a cost for display. For sub-$1 values shows 3 decimals (e.g. $0.023),
/// for >=$1 shows 2 decimals (e.g. $1.43).
pub fn format_cost(cost: f64) -> String {
    if cost >= 1.0 {
        format!("${:.2}", cost)
    } else {
        format!("${:.3}", cost)
    }
}

/// Budget enforcement result.
#[derive(Debug, Clone, PartialEq)]
pub enum BudgetStatus {
    Ok,
    Exceeded { limit: f64, actual: f64 },
}

impl BudgetStatus {
    pub fn check(total: f64, limit: Option<f64>) -> Self {
        match limit {
            Some(max) if total >= max => BudgetStatus::Exceeded {
                limit: max,
                actual: total,
            },
            _ => BudgetStatus::Ok,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::response::Usage;

    #[test]
    fn test_get_pricing_opus() {
        let p = get_pricing("claude-opus-4-6");
        assert!((p.input_per_m - 15.00).abs() < f64::EPSILON);
        assert!((p.output_per_m - 75.00).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_pricing_sonnet() {
        let p = get_pricing("claude-sonnet-4-6");
        assert!((p.input_per_m - 3.00).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_pricing_haiku() {
        let p = get_pricing("claude-haiku-4-5");
        assert!((p.input_per_m - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_pricing_default_is_sonnet() {
        let p = get_pricing("unknown-model");
        assert!((p.input_per_m - 3.00).abs() < f64::EPSILON);
    }

    #[test]
    fn test_calculate_cost_from_usage() {
        let pricing = SONNET_PRICING;
        let usage = Usage {
            input_tokens: 10_000,
            output_tokens: 1_000,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        let cost = calculate_cost(&usage, &pricing);
        // 10k input at $3/M = $0.03, 1k output at $15/M = $0.015
        assert!((cost - 0.045).abs() < 0.0001);
    }

    #[test]
    fn test_calculate_cost_with_caching() {
        let pricing = SONNET_PRICING;
        let usage = Usage {
            input_tokens: 5_000,
            output_tokens: 2_000,
            cache_creation_input_tokens: 3_000,
            cache_read_input_tokens: 2_000,
        };
        let cost = calculate_cost(&usage, &pricing);
        let expected = (5_000.0 / 1_000_000.0) * 3.0
            + (2_000.0 / 1_000_000.0) * 15.0
            + (3_000.0 / 1_000_000.0) * 3.75
            + (2_000.0 / 1_000_000.0) * 0.30;
        assert!((cost - expected).abs() < 0.0001);
    }

    #[test]
    fn test_cost_tracker_accumulation() {
        let mut tracker = CostTracker::new();
        let pricing = SONNET_PRICING;

        tracker.record_turn(&pricing, 10_000, 2_000, 0, 0);
        tracker.record_turn(&pricing, 10_000, 2_000, 0, 0);

        // Each turn: 10k * $3/M + 2k * $15/M = $0.03 + $0.03 = $0.06
        assert!((tracker.total_cost_usd - 0.12).abs() < 0.0001);
        assert_eq!(tracker.turns.len(), 2);
    }

    #[test]
    fn test_format_cost() {
        assert_eq!(format_cost(0.023), "$0.023");
        assert_eq!(format_cost(0.999), "$0.999");
        assert_eq!(format_cost(1.0), "$1.00");
        assert_eq!(format_cost(12.34), "$12.34");
    }

    #[test]
    fn test_budget_status_check() {
        assert!(BudgetStatus::check(0.05, Some(5.0)) == BudgetStatus::Ok);
        assert!(BudgetStatus::check(5.0, Some(5.0)) == BudgetStatus::Exceeded { limit: 5.0, actual: 5.0 });
        assert!(BudgetStatus::check(0.05, None) == BudgetStatus::Ok);
    }
}
