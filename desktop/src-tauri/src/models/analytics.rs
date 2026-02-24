//! Analytics Models
//!
//! Data structures for usage analytics and cost tracking.

use serde::{Deserialize, Serialize};

/// A single usage record tracking API usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// Unique record identifier
    pub id: i64,
    /// Associated session ID (optional)
    pub session_id: Option<String>,
    /// Associated project ID (optional)
    pub project_id: Option<String>,
    /// Model name (e.g., "claude-3-5-sonnet-20241022")
    pub model_name: String,
    /// Provider name (e.g., "anthropic", "openai")
    pub provider: String,
    /// Number of input tokens
    pub input_tokens: i64,
    /// Number of output tokens
    pub output_tokens: i64,
    /// Number of thinking/reasoning tokens
    #[serde(default)]
    pub thinking_tokens: i64,
    /// Number of cache read tokens
    #[serde(default)]
    pub cache_read_tokens: i64,
    /// Number of cache creation tokens
    #[serde(default)]
    pub cache_creation_tokens: i64,
    /// Calculated cost in microdollars (1 USD = 1,000,000 microdollars)
    pub cost_microdollars: i64,
    /// Unix timestamp of the record
    pub timestamp: i64,
    /// Optional metadata as JSON
    pub metadata: Option<String>,
}

impl UsageRecord {
    /// Create a new usage record
    pub fn new(
        model_name: impl Into<String>,
        provider: impl Into<String>,
        input_tokens: i64,
        output_tokens: i64,
    ) -> Self {
        Self {
            id: 0,
            session_id: None,
            project_id: None,
            model_name: model_name.into(),
            provider: provider.into(),
            input_tokens,
            output_tokens,
            thinking_tokens: 0,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_microdollars: 0,
            timestamp: chrono::Utc::now().timestamp(),
            metadata: None,
        }
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set project ID
    pub fn with_project(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = Some(project_id.into());
        self
    }

    /// Set cost in microdollars
    pub fn with_cost(mut self, cost_microdollars: i64) -> Self {
        self.cost_microdollars = cost_microdollars;
        self
    }

    /// Set extended token counts (thinking, cache read, cache creation)
    pub fn with_extended_tokens(
        mut self,
        thinking: i64,
        cache_read: i64,
        cache_creation: i64,
    ) -> Self {
        self.thinking_tokens = thinking;
        self.cache_read_tokens = cache_read;
        self.cache_creation_tokens = cache_creation;
        self
    }

    /// Get cost in dollars as f64
    pub fn cost_dollars(&self) -> f64 {
        self.cost_microdollars as f64 / 1_000_000.0
    }

    /// Get total tokens
    pub fn total_tokens(&self) -> i64 {
        self.input_tokens + self.output_tokens
    }
}

/// Model pricing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Unique pricing identifier
    pub id: i64,
    /// Model name pattern (can include wildcards)
    pub model_name: String,
    /// Provider name
    pub provider: String,
    /// Input token price per million tokens in microdollars
    pub input_price_per_million: i64,
    /// Output token price per million tokens in microdollars
    pub output_price_per_million: i64,
    /// Whether this is a custom/override pricing
    pub is_custom: bool,
    /// When the pricing was last updated (Unix timestamp)
    pub updated_at: i64,
}

impl ModelPricing {
    /// Create new pricing for a model
    pub fn new(
        model_name: impl Into<String>,
        provider: impl Into<String>,
        input_price_per_million: i64,
        output_price_per_million: i64,
    ) -> Self {
        Self {
            id: 0,
            model_name: model_name.into(),
            provider: provider.into(),
            input_price_per_million,
            output_price_per_million,
            is_custom: false,
            updated_at: chrono::Utc::now().timestamp(),
        }
    }

    /// Calculate cost for given token counts
    pub fn calculate_cost(&self, input_tokens: i64, output_tokens: i64) -> i64 {
        let input_cost = (input_tokens * self.input_price_per_million) / 1_000_000;
        let output_cost = (output_tokens * self.output_price_per_million) / 1_000_000;
        input_cost + output_cost
    }
}

/// Aggregated usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageStats {
    /// Total input tokens
    pub total_input_tokens: i64,
    /// Total output tokens
    pub total_output_tokens: i64,
    /// Total cost in microdollars
    pub total_cost_microdollars: i64,
    /// Number of requests
    pub request_count: i64,
    /// Average tokens per request
    pub avg_tokens_per_request: f64,
    /// Average cost per request in microdollars
    pub avg_cost_per_request: f64,
}

impl UsageStats {
    /// Get total cost in dollars
    pub fn total_cost_dollars(&self) -> f64 {
        self.total_cost_microdollars as f64 / 1_000_000.0
    }

    /// Get total tokens
    pub fn total_tokens(&self) -> i64 {
        self.total_input_tokens + self.total_output_tokens
    }
}

/// Usage aggregated by model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    /// Model name
    pub model_name: String,
    /// Provider name
    pub provider: String,
    /// Usage statistics for this model
    pub stats: UsageStats,
}

/// Usage aggregated by project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectUsage {
    /// Project ID
    pub project_id: String,
    /// Project name (if available)
    pub project_name: Option<String>,
    /// Usage statistics for this project
    pub stats: UsageStats,
}

/// Time-series data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    /// Timestamp (Unix epoch)
    pub timestamp: i64,
    /// Formatted timestamp for display
    pub timestamp_formatted: String,
    /// Usage statistics for this time period
    pub stats: UsageStats,
}

/// Aggregation period for time-series data
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AggregationPeriod {
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

impl AggregationPeriod {
    /// Get SQL date format string for SQLite strftime
    pub fn sql_format(&self) -> &'static str {
        match self {
            AggregationPeriod::Hourly => "%Y-%m-%d %H:00:00",
            AggregationPeriod::Daily => "%Y-%m-%d",
            AggregationPeriod::Weekly => "%Y-%W",
            AggregationPeriod::Monthly => "%Y-%m",
        }
    }
}

/// Filter criteria for usage queries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UsageFilter {
    /// Start timestamp (Unix epoch, inclusive)
    pub start_timestamp: Option<i64>,
    /// End timestamp (Unix epoch, exclusive)
    pub end_timestamp: Option<i64>,
    /// Filter by model name
    pub model_name: Option<String>,
    /// Filter by provider
    pub provider: Option<String>,
    /// Filter by session ID
    pub session_id: Option<String>,
    /// Filter by project ID
    pub project_id: Option<String>,
}

impl UsageFilter {
    /// Create a filter for the last N days
    pub fn last_days(days: i64) -> Self {
        let now = chrono::Utc::now().timestamp();
        let start = now - (days * 24 * 60 * 60);
        Self {
            start_timestamp: Some(start),
            end_timestamp: Some(now),
            ..Default::default()
        }
    }

    /// Set model filter
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model_name = Some(model.into());
        self
    }

    /// Set provider filter
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Set project filter
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project_id = Some(project.into());
        self
    }
}

/// Dashboard summary data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    /// Current period statistics
    pub current_period: UsageStats,
    /// Previous period statistics (for comparison)
    pub previous_period: UsageStats,
    /// Percentage change in cost
    pub cost_change_percent: f64,
    /// Percentage change in tokens
    pub tokens_change_percent: f64,
    /// Percentage change in requests
    pub requests_change_percent: f64,
    /// Usage breakdown by model
    pub by_model: Vec<ModelUsage>,
    /// Usage breakdown by project
    pub by_project: Vec<ProjectUsage>,
    /// Time series data for charts
    pub time_series: Vec<TimeSeriesPoint>,
}

impl DashboardSummary {
    /// Calculate percentage change between two values
    pub fn calculate_change(current: f64, previous: f64) -> f64 {
        if previous == 0.0 {
            if current > 0.0 {
                100.0
            } else {
                0.0
            }
        } else {
            ((current - previous) / previous) * 100.0
        }
    }
}

/// Export format for usage data
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Csv,
    Json,
}

/// Export request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRequest {
    /// Filter criteria
    pub filter: UsageFilter,
    /// Export format
    pub format: ExportFormat,
    /// Include summary row/object
    pub include_summary: bool,
}

/// Export result with data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    /// Exported data as string (CSV or JSON)
    pub data: String,
    /// Number of records exported
    pub record_count: i64,
    /// Summary statistics if requested
    pub summary: Option<UsageStats>,
    /// Suggested filename
    pub suggested_filename: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_record_creation() {
        let record = UsageRecord::new("claude-3-5-sonnet", "anthropic", 1000, 500)
            .with_session("sess-123")
            .with_project("proj-456")
            .with_cost(5000);

        assert_eq!(record.model_name, "claude-3-5-sonnet");
        assert_eq!(record.provider, "anthropic");
        assert_eq!(record.input_tokens, 1000);
        assert_eq!(record.output_tokens, 500);
        assert_eq!(record.total_tokens(), 1500);
        assert_eq!(record.cost_microdollars, 5000);
        assert_eq!(record.session_id, Some("sess-123".to_string()));
        assert_eq!(record.project_id, Some("proj-456".to_string()));
    }

    #[test]
    fn test_model_pricing_calculation() {
        // Claude 3.5 Sonnet pricing: $3/M input, $15/M output
        let pricing = ModelPricing::new(
            "claude-3-5-sonnet",
            "anthropic",
            3_000_000,  // $3 in microdollars per million
            15_000_000, // $15 in microdollars per million
        );

        // 1000 input tokens, 500 output tokens
        let cost = pricing.calculate_cost(1000, 500);
        // Expected: (1000 * 3_000_000 / 1_000_000) + (500 * 15_000_000 / 1_000_000)
        // = 3000 + 7500 = 10500 microdollars = $0.0105
        assert_eq!(cost, 10500);
    }

    #[test]
    fn test_usage_stats_cost_dollars() {
        let stats = UsageStats {
            total_input_tokens: 10000,
            total_output_tokens: 5000,
            total_cost_microdollars: 1_500_000, // $1.50
            request_count: 10,
            avg_tokens_per_request: 1500.0,
            avg_cost_per_request: 150_000.0,
        };

        assert_eq!(stats.total_cost_dollars(), 1.5);
        assert_eq!(stats.total_tokens(), 15000);
    }

    #[test]
    fn test_usage_filter_last_days() {
        let filter = UsageFilter::last_days(7);
        assert!(filter.start_timestamp.is_some());
        assert!(filter.end_timestamp.is_some());

        let start = filter.start_timestamp.unwrap();
        let end = filter.end_timestamp.unwrap();
        let duration = end - start;

        // Should be approximately 7 days in seconds
        assert!((duration - 7 * 24 * 60 * 60).abs() < 60); // Within 1 minute tolerance
    }

    #[test]
    fn test_dashboard_summary_change_calculation() {
        assert_eq!(DashboardSummary::calculate_change(110.0, 100.0), 10.0);
        assert_eq!(DashboardSummary::calculate_change(90.0, 100.0), -10.0);
        assert_eq!(DashboardSummary::calculate_change(100.0, 0.0), 100.0);
        assert_eq!(DashboardSummary::calculate_change(0.0, 0.0), 0.0);
    }

    #[test]
    fn test_aggregation_period_sql_format() {
        assert_eq!(AggregationPeriod::Hourly.sql_format(), "%Y-%m-%d %H:00:00");
        assert_eq!(AggregationPeriod::Daily.sql_format(), "%Y-%m-%d");
        assert_eq!(AggregationPeriod::Weekly.sql_format(), "%Y-%W");
        assert_eq!(AggregationPeriod::Monthly.sql_format(), "%Y-%m");
    }

    #[test]
    fn test_usage_record_serialization() {
        let record = UsageRecord::new("gpt-4", "openai", 500, 250);
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("\"model_name\":\"gpt-4\""));
        assert!(json.contains("\"provider\":\"openai\""));

        let deserialized: UsageRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model_name, "gpt-4");
        assert_eq!(deserialized.input_tokens, 500);
    }
}
