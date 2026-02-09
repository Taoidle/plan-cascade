//! Cost Calculator
//!
//! Handles cost calculation for API usage with support for multiple models and providers.
//! Supports configurable pricing and automatic cost computation.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::models::analytics::ModelPricing;
use crate::utils::error::{AppError, AppResult};

/// Default pricing data for common models (in microdollars per million tokens)
fn get_default_pricing() -> Vec<ModelPricing> {
    vec![
        // Anthropic Claude 3.5 family
        ModelPricing::new(
            "claude-3-5-sonnet-20241022",
            "anthropic",
            3_000_000,
            15_000_000,
        ),
        ModelPricing::new(
            "claude-3-5-sonnet-latest",
            "anthropic",
            3_000_000,
            15_000_000,
        ),
        ModelPricing::new(
            "claude-3-5-haiku-20241022",
            "anthropic",
            1_000_000,
            5_000_000,
        ),
        // Anthropic Claude 3 family
        ModelPricing::new(
            "claude-3-opus-20240229",
            "anthropic",
            15_000_000,
            75_000_000,
        ),
        ModelPricing::new(
            "claude-3-sonnet-20240229",
            "anthropic",
            3_000_000,
            15_000_000,
        ),
        ModelPricing::new("claude-3-haiku-20240307", "anthropic", 250_000, 1_250_000),
        // Anthropic Claude 4 family
        ModelPricing::new(
            "claude-opus-4-20250514",
            "anthropic",
            15_000_000,
            75_000_000,
        ),
        ModelPricing::new(
            "claude-sonnet-4-20250514",
            "anthropic",
            3_000_000,
            15_000_000,
        ),
        // OpenAI GPT-4 family
        ModelPricing::new("gpt-4-turbo", "openai", 10_000_000, 30_000_000),
        ModelPricing::new("gpt-4-turbo-preview", "openai", 10_000_000, 30_000_000),
        ModelPricing::new("gpt-4o", "openai", 5_000_000, 15_000_000),
        ModelPricing::new("gpt-4o-mini", "openai", 150_000, 600_000),
        ModelPricing::new("gpt-4", "openai", 30_000_000, 60_000_000),
        ModelPricing::new("gpt-4-32k", "openai", 60_000_000, 120_000_000),
        // OpenAI GPT-3.5 family
        ModelPricing::new("gpt-3.5-turbo", "openai", 500_000, 1_500_000),
        ModelPricing::new("gpt-3.5-turbo-16k", "openai", 3_000_000, 4_000_000),
        // DeepSeek
        ModelPricing::new("deepseek-chat", "deepseek", 140_000, 280_000),
        ModelPricing::new("deepseek-coder", "deepseek", 140_000, 280_000),
        ModelPricing::new("deepseek-reasoner", "deepseek", 550_000, 2_190_000),
        // Local models (free)
        ModelPricing::new("llama3", "ollama", 0, 0),
        ModelPricing::new("llama3.1", "ollama", 0, 0),
        ModelPricing::new("llama3.2", "ollama", 0, 0),
        ModelPricing::new("codellama", "ollama", 0, 0),
        ModelPricing::new("mistral", "ollama", 0, 0),
        ModelPricing::new("mixtral", "ollama", 0, 0),
        ModelPricing::new("phi3", "ollama", 0, 0),
        ModelPricing::new("qwen2", "ollama", 0, 0),
    ]
}

/// Cost calculator for computing API usage costs
#[derive(Debug)]
pub struct CostCalculator {
    /// Pricing lookup by (provider, model_name)
    pricing: RwLock<HashMap<(String, String), ModelPricing>>,
    /// Custom overrides by (provider, model_name)
    custom_overrides: RwLock<HashMap<(String, String), ModelPricing>>,
}

impl Default for CostCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl CostCalculator {
    /// Create a new cost calculator with default pricing
    pub fn new() -> Self {
        let mut pricing_map = HashMap::new();

        for pricing in get_default_pricing() {
            let key = (pricing.provider.clone(), pricing.model_name.clone());
            pricing_map.insert(key, pricing);
        }

        Self {
            pricing: RwLock::new(pricing_map),
            custom_overrides: RwLock::new(HashMap::new()),
        }
    }

    /// Load pricing from database
    pub fn load_from_pricing_list(&self, pricing_list: Vec<ModelPricing>) -> AppResult<()> {
        let mut pricing = self
            .pricing
            .write()
            .map_err(|_| AppError::internal("Failed to acquire pricing lock"))?;

        let mut custom = self
            .custom_overrides
            .write()
            .map_err(|_| AppError::internal("Failed to acquire custom overrides lock"))?;

        for p in pricing_list {
            let key = (p.provider.clone(), p.model_name.clone());
            if p.is_custom {
                custom.insert(key, p);
            } else {
                pricing.insert(key, p);
            }
        }

        Ok(())
    }

    /// Get pricing for a specific model
    pub fn get_pricing(&self, provider: &str, model_name: &str) -> Option<ModelPricing> {
        let key = (provider.to_string(), model_name.to_string());

        // Check custom overrides first
        if let Ok(custom) = self.custom_overrides.read() {
            if let Some(pricing) = custom.get(&key) {
                return Some(pricing.clone());
            }
        }

        // Fall back to default pricing
        if let Ok(pricing) = self.pricing.read() {
            if let Some(p) = pricing.get(&key) {
                return Some(p.clone());
            }

            // Try to match by prefix for versioned models
            for ((prov, model), p) in pricing.iter() {
                if prov == provider && model_name.starts_with(model) {
                    return Some(p.clone());
                }
            }
        }

        None
    }

    /// Calculate cost for given token counts
    /// Returns cost in microdollars (1 USD = 1,000,000 microdollars)
    pub fn calculate_cost(
        &self,
        provider: &str,
        model_name: &str,
        input_tokens: i64,
        output_tokens: i64,
    ) -> i64 {
        if let Some(pricing) = self.get_pricing(provider, model_name) {
            pricing.calculate_cost(input_tokens, output_tokens)
        } else {
            // Unknown model - estimate using average pricing
            // Default: $5/M input, $15/M output
            let input_cost = (input_tokens * 5_000_000) / 1_000_000;
            let output_cost = (output_tokens * 15_000_000) / 1_000_000;
            input_cost + output_cost
        }
    }

    /// Set custom pricing override for a model
    pub fn set_custom_pricing(&self, pricing: ModelPricing) -> AppResult<()> {
        let mut custom = self
            .custom_overrides
            .write()
            .map_err(|_| AppError::internal("Failed to acquire custom overrides lock"))?;

        let key = (pricing.provider.clone(), pricing.model_name.clone());
        custom.insert(
            key,
            ModelPricing {
                is_custom: true,
                ..pricing
            },
        );

        Ok(())
    }

    /// Remove custom pricing override for a model
    pub fn remove_custom_pricing(&self, provider: &str, model_name: &str) -> AppResult<bool> {
        let mut custom = self
            .custom_overrides
            .write()
            .map_err(|_| AppError::internal("Failed to acquire custom overrides lock"))?;

        let key = (provider.to_string(), model_name.to_string());
        Ok(custom.remove(&key).is_some())
    }

    /// Get all pricing (default + custom)
    pub fn get_all_pricing(&self) -> AppResult<Vec<ModelPricing>> {
        let pricing = self
            .pricing
            .read()
            .map_err(|_| AppError::internal("Failed to acquire pricing lock"))?;
        let custom = self
            .custom_overrides
            .read()
            .map_err(|_| AppError::internal("Failed to acquire custom overrides lock"))?;

        let mut result: Vec<ModelPricing> = pricing.values().cloned().collect();

        // Add custom overrides (they will override defaults when displayed)
        for (key, p) in custom.iter() {
            // Remove default if custom exists
            result.retain(|r| (r.provider.clone(), r.model_name.clone()) != *key);
            result.push(p.clone());
        }

        result.sort_by(|a, b| {
            a.provider
                .cmp(&b.provider)
                .then_with(|| a.model_name.cmp(&b.model_name))
        });

        Ok(result)
    }

    /// Batch calculate costs for multiple requests
    pub fn batch_calculate_cost(&self, requests: &[(String, String, i64, i64)]) -> Vec<i64> {
        requests
            .iter()
            .map(|(provider, model, input, output)| {
                self.calculate_cost(provider, model, *input, *output)
            })
            .collect()
    }

    /// Get estimated monthly cost based on daily usage
    pub fn estimate_monthly_cost(&self, daily_cost_microdollars: i64) -> i64 {
        daily_cost_microdollars * 30
    }

    /// Format cost in dollars for display
    pub fn format_cost_dollars(microdollars: i64) -> String {
        let dollars = microdollars as f64 / 1_000_000.0;
        format!("${:.4}", dollars)
    }

    /// Format cost in a human-readable way
    pub fn format_cost_human(microdollars: i64) -> String {
        let dollars = microdollars as f64 / 1_000_000.0;
        if dollars < 0.01 {
            format!("${:.6}", dollars)
        } else if dollars < 1.0 {
            format!("${:.4}", dollars)
        } else if dollars < 100.0 {
            format!("${:.2}", dollars)
        } else {
            format!("${:.0}", dollars)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculator_creation() {
        let calc = CostCalculator::new();

        // Should have default pricing loaded
        let pricing = calc.get_pricing("anthropic", "claude-3-5-sonnet-20241022");
        assert!(pricing.is_some());

        let p = pricing.unwrap();
        assert_eq!(p.input_price_per_million, 3_000_000);
        assert_eq!(p.output_price_per_million, 15_000_000);
    }

    #[test]
    fn test_cost_calculation_claude() {
        let calc = CostCalculator::new();

        // Claude 3.5 Sonnet: $3/M input, $15/M output
        // 1000 input tokens, 500 output tokens
        let cost = calc.calculate_cost("anthropic", "claude-3-5-sonnet-20241022", 1000, 500);

        // Expected: (1000 * 3_000_000 / 1_000_000) + (500 * 15_000_000 / 1_000_000)
        // = 3000 + 7500 = 10500 microdollars = $0.0105
        assert_eq!(cost, 10500);
    }

    #[test]
    fn test_cost_calculation_gpt4() {
        let calc = CostCalculator::new();

        // GPT-4o: $5/M input, $15/M output
        let cost = calc.calculate_cost("openai", "gpt-4o", 10_000, 5_000);

        // Expected: (10000 * 5_000_000 / 1_000_000) + (5000 * 15_000_000 / 1_000_000)
        // = 50000 + 75000 = 125000 microdollars = $0.125
        assert_eq!(cost, 125000);
    }

    #[test]
    fn test_cost_calculation_free_model() {
        let calc = CostCalculator::new();

        // Ollama models are free
        let cost = calc.calculate_cost("ollama", "llama3", 100_000, 50_000);
        assert_eq!(cost, 0);
    }

    #[test]
    fn test_custom_pricing_override() {
        let calc = CostCalculator::new();

        // Set custom pricing
        let custom = ModelPricing::new("custom-model", "custom-provider", 1_000_000, 2_000_000);
        calc.set_custom_pricing(custom).unwrap();

        // Should use custom pricing
        let pricing = calc.get_pricing("custom-provider", "custom-model");
        assert!(pricing.is_some());
        assert!(pricing.unwrap().is_custom);

        let cost = calc.calculate_cost("custom-provider", "custom-model", 1000, 500);
        // (1000 * 1_000_000 / 1_000_000) + (500 * 2_000_000 / 1_000_000) = 1000 + 1000 = 2000
        assert_eq!(cost, 2000);
    }

    #[test]
    fn test_remove_custom_pricing() {
        let calc = CostCalculator::new();

        let custom = ModelPricing::new("test-model", "test-provider", 1_000_000, 2_000_000);
        calc.set_custom_pricing(custom).unwrap();

        assert!(calc.get_pricing("test-provider", "test-model").is_some());

        let removed = calc
            .remove_custom_pricing("test-provider", "test-model")
            .unwrap();
        assert!(removed);

        // Should no longer exist (not in defaults)
        assert!(calc.get_pricing("test-provider", "test-model").is_none());
    }

    #[test]
    fn test_batch_calculate() {
        let calc = CostCalculator::new();

        let requests = vec![
            (
                "anthropic".to_string(),
                "claude-3-5-sonnet-20241022".to_string(),
                1000_i64,
                500_i64,
            ),
            (
                "openai".to_string(),
                "gpt-4o".to_string(),
                2000_i64,
                1000_i64,
            ),
        ];

        let costs = calc.batch_calculate_cost(&requests);
        assert_eq!(costs.len(), 2);
        assert!(costs[0] > 0);
        assert!(costs[1] > 0);
    }

    #[test]
    fn test_format_cost_dollars() {
        assert_eq!(CostCalculator::format_cost_dollars(1_500_000), "$1.5000");
        assert_eq!(CostCalculator::format_cost_dollars(100), "$0.0001");
        assert_eq!(CostCalculator::format_cost_dollars(0), "$0.0000");
    }

    #[test]
    fn test_format_cost_human() {
        // Very small amounts
        assert!(CostCalculator::format_cost_human(10).starts_with("$0.00001"));
        // Small amounts
        assert!(CostCalculator::format_cost_human(50_000).starts_with("$0.05"));
        // Medium amounts
        assert!(CostCalculator::format_cost_human(1_500_000).starts_with("$1.5"));
        // Large amounts
        assert!(CostCalculator::format_cost_human(150_000_000).starts_with("$150"));
    }

    #[test]
    fn test_estimate_monthly_cost() {
        let calc = CostCalculator::new();

        // $1/day = $30/month
        let monthly = calc.estimate_monthly_cost(1_000_000);
        assert_eq!(monthly, 30_000_000);
    }

    #[test]
    fn test_unknown_model_fallback() {
        let calc = CostCalculator::new();

        // Unknown model should use default estimate
        let cost = calc.calculate_cost("unknown", "unknown-model", 1000, 500);

        // Default: $5/M input, $15/M output
        // (1000 * 5_000_000 / 1_000_000) + (500 * 15_000_000 / 1_000_000) = 5000 + 7500 = 12500
        assert_eq!(cost, 12500);
    }

    #[test]
    fn test_get_all_pricing() {
        let calc = CostCalculator::new();

        let all = calc.get_all_pricing().unwrap();
        assert!(!all.is_empty());

        // Should contain Claude models
        assert!(all.iter().any(|p| p.model_name.contains("claude")));
        // Should contain GPT models
        assert!(all.iter().any(|p| p.model_name.contains("gpt")));
    }
}
