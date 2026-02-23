//! Data Aggregation
//!
//! Provides aggregation queries for analytics by model, project, and time period.
//! Supports flexible filtering and grouping options.

use crate::models::analytics::{
    AggregationPeriod, DashboardSummary, ModelUsage, ProjectUsage, TimeSeriesPoint, UsageFilter,
    UsageStats,
};
use crate::utils::error::{AppError, AppResult};

use super::service::AnalyticsService;

impl AnalyticsService {
    // ========================================================================
    // Aggregation by Model
    // ========================================================================

    /// Get usage aggregated by model
    pub fn aggregate_by_model(&self, filter: &UsageFilter) -> AppResult<Vec<ModelUsage>> {
        let conn = self.get_connection()?;

        let mut sql = String::from(
            "SELECT model_name, provider,
                    SUM(input_tokens) as total_input,
                    SUM(output_tokens) as total_output,
                    SUM(cost_microdollars) as total_cost,
                    COUNT(*) as request_count
             FROM usage_records WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        Self::append_filter_clauses(&mut sql, &mut params_vec, filter);

        sql.push_str(" GROUP BY model_name, provider ORDER BY total_cost DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let results = stmt
            .query_map(params_refs.as_slice(), |row| {
                let model_name: String = row.get(0)?;
                let provider: String = row.get(1)?;
                let total_input: i64 = row.get(2)?;
                let total_output: i64 = row.get(3)?;
                let total_cost: i64 = row.get(4)?;
                let request_count: i64 = row.get(5)?;

                let avg_tokens = if request_count > 0 {
                    (total_input + total_output) as f64 / request_count as f64
                } else {
                    0.0
                };
                let avg_cost = if request_count > 0 {
                    total_cost as f64 / request_count as f64
                } else {
                    0.0
                };

                Ok(ModelUsage {
                    model_name,
                    provider,
                    stats: UsageStats {
                        total_input_tokens: total_input,
                        total_output_tokens: total_output,
                        total_cost_microdollars: total_cost,
                        request_count,
                        avg_tokens_per_request: avg_tokens,
                        avg_cost_per_request: avg_cost,
                    },
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    // ========================================================================
    // Aggregation by Project
    // ========================================================================

    /// Get usage aggregated by project
    pub fn aggregate_by_project(&self, filter: &UsageFilter) -> AppResult<Vec<ProjectUsage>> {
        let conn = self.get_connection()?;

        let mut sql = String::from(
            "SELECT project_id,
                    SUM(input_tokens) as total_input,
                    SUM(output_tokens) as total_output,
                    SUM(cost_microdollars) as total_cost,
                    COUNT(*) as request_count
             FROM usage_records WHERE project_id IS NOT NULL",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        Self::append_filter_clauses(&mut sql, &mut params_vec, filter);

        sql.push_str(" GROUP BY project_id ORDER BY total_cost DESC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let results = stmt
            .query_map(params_refs.as_slice(), |row| {
                let project_id: String = row.get(0)?;
                let total_input: i64 = row.get(1)?;
                let total_output: i64 = row.get(2)?;
                let total_cost: i64 = row.get(3)?;
                let request_count: i64 = row.get(4)?;

                let avg_tokens = if request_count > 0 {
                    (total_input + total_output) as f64 / request_count as f64
                } else {
                    0.0
                };
                let avg_cost = if request_count > 0 {
                    total_cost as f64 / request_count as f64
                } else {
                    0.0
                };

                Ok(ProjectUsage {
                    project_id,
                    project_name: None, // Would need to join with projects table
                    stats: UsageStats {
                        total_input_tokens: total_input,
                        total_output_tokens: total_output,
                        total_cost_microdollars: total_cost,
                        request_count,
                        avg_tokens_per_request: avg_tokens,
                        avg_cost_per_request: avg_cost,
                    },
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    // ========================================================================
    // Time Series Aggregation
    // ========================================================================

    /// Get time series data with specified aggregation period
    pub fn get_time_series(
        &self,
        filter: &UsageFilter,
        period: AggregationPeriod,
    ) -> AppResult<Vec<TimeSeriesPoint>> {
        let conn = self.get_connection()?;

        let date_format = period.sql_format();

        let mut sql = format!(
            "SELECT strftime('{}', datetime(timestamp, 'unixepoch')) as period,
                    MIN(timestamp) as period_start,
                    SUM(input_tokens) as total_input,
                    SUM(output_tokens) as total_output,
                    SUM(cost_microdollars) as total_cost,
                    COUNT(*) as request_count
             FROM usage_records WHERE 1=1",
            date_format
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        Self::append_filter_clauses(&mut sql, &mut params_vec, filter);

        sql.push_str(" GROUP BY period ORDER BY period_start ASC");

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let results = stmt
            .query_map(params_refs.as_slice(), |row| {
                let period_formatted: String = row.get(0)?;
                let timestamp: i64 = row.get(1)?;
                let total_input: i64 = row.get(2)?;
                let total_output: i64 = row.get(3)?;
                let total_cost: i64 = row.get(4)?;
                let request_count: i64 = row.get(5)?;

                let avg_tokens = if request_count > 0 {
                    (total_input + total_output) as f64 / request_count as f64
                } else {
                    0.0
                };
                let avg_cost = if request_count > 0 {
                    total_cost as f64 / request_count as f64
                } else {
                    0.0
                };

                Ok(TimeSeriesPoint {
                    timestamp,
                    timestamp_formatted: period_formatted,
                    stats: UsageStats {
                        total_input_tokens: total_input,
                        total_output_tokens: total_output,
                        total_cost_microdollars: total_cost,
                        request_count,
                        avg_tokens_per_request: avg_tokens,
                        avg_cost_per_request: avg_cost,
                    },
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(results)
    }

    // ========================================================================
    // Dashboard Summary
    // ========================================================================

    /// Get complete dashboard summary with comparisons and breakdowns
    pub fn get_dashboard_summary(
        &self,
        filter: &UsageFilter,
        period: AggregationPeriod,
    ) -> AppResult<DashboardSummary> {
        // Get current period stats
        let current_stats = self.get_usage_stats(filter)?;

        // Calculate previous period filter
        let previous_filter = Self::calculate_previous_period_filter(filter)?;
        let previous_stats = self.get_usage_stats(&previous_filter)?;

        // Calculate percentage changes
        let cost_change = DashboardSummary::calculate_change(
            current_stats.total_cost_microdollars as f64,
            previous_stats.total_cost_microdollars as f64,
        );
        let tokens_change = DashboardSummary::calculate_change(
            current_stats.total_tokens() as f64,
            previous_stats.total_tokens() as f64,
        );
        let requests_change = DashboardSummary::calculate_change(
            current_stats.request_count as f64,
            previous_stats.request_count as f64,
        );

        // Get breakdowns
        let by_model = self.aggregate_by_model(filter)?;
        let by_project = self.aggregate_by_project(filter)?;
        let time_series = self.get_time_series(filter, period)?;

        Ok(DashboardSummary {
            current_period: current_stats,
            previous_period: previous_stats,
            cost_change_percent: cost_change,
            tokens_change_percent: tokens_change,
            requests_change_percent: requests_change,
            by_model,
            by_project,
            time_series,
        })
    }

    /// Calculate the filter for the previous equivalent period
    fn calculate_previous_period_filter(filter: &UsageFilter) -> AppResult<UsageFilter> {
        let mut prev = filter.clone();

        if let (Some(start), Some(end)) = (filter.start_timestamp, filter.end_timestamp) {
            let duration = end - start;
            prev.start_timestamp = Some(start - duration);
            prev.end_timestamp = Some(start);
        }

        Ok(prev)
    }

    // ========================================================================
    // Summary Statistics
    // ========================================================================

    /// Get min, max, and percentile statistics
    pub fn get_summary_statistics(&self, filter: &UsageFilter) -> AppResult<SummaryStatistics> {
        let conn = self.get_connection()?;

        let mut sql = String::from(
            "SELECT
                MIN(cost_microdollars) as min_cost,
                MAX(cost_microdollars) as max_cost,
                MIN(input_tokens + output_tokens) as min_tokens,
                MAX(input_tokens + output_tokens) as max_tokens,
                COUNT(*) as total_count
             FROM usage_records WHERE 1=1",
        );
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        Self::append_filter_clauses(&mut sql, &mut params_vec, filter);

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let (min_cost, max_cost, min_tokens, max_tokens, total_count) =
            conn.query_row(&sql, params_refs.as_slice(), |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    row.get::<_, i64>(4)?,
                ))
            })?;

        // Get percentiles (p50, p90, p95, p99) for cost
        let percentiles = if total_count > 0 {
            self.calculate_percentiles(filter, "cost_microdollars")?
        } else {
            Percentiles::default()
        };

        Ok(SummaryStatistics {
            min_cost_microdollars: min_cost,
            max_cost_microdollars: max_cost,
            min_tokens: min_tokens,
            max_tokens: max_tokens,
            total_records: total_count,
            cost_percentiles: percentiles,
        })
    }

    /// Calculate percentiles for a given field
    fn calculate_percentiles(&self, filter: &UsageFilter, field: &str) -> AppResult<Percentiles> {
        let conn = self.get_connection()?;

        // Get count first
        let count = self.count_usage_records(filter)?;
        if count == 0 {
            return Ok(Percentiles::default());
        }

        let mut sql = format!("SELECT {} FROM usage_records WHERE 1=1", field);
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        Self::append_filter_clauses(&mut sql, &mut params_vec, filter);
        sql.push_str(&format!(" ORDER BY {}", field));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let values: Vec<i64> = stmt
            .query_map(params_refs.as_slice(), |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        if values.is_empty() {
            return Ok(Percentiles::default());
        }

        let p50_idx = (values.len() as f64 * 0.50) as usize;
        let p90_idx = (values.len() as f64 * 0.90) as usize;
        let p95_idx = (values.len() as f64 * 0.95) as usize;
        let p99_idx = (values.len() as f64 * 0.99) as usize;

        Ok(Percentiles {
            p50: values
                .get(p50_idx.min(values.len() - 1))
                .copied()
                .unwrap_or(0),
            p90: values
                .get(p90_idx.min(values.len() - 1))
                .copied()
                .unwrap_or(0),
            p95: values
                .get(p95_idx.min(values.len() - 1))
                .copied()
                .unwrap_or(0),
            p99: values
                .get(p99_idx.min(values.len() - 1))
                .copied()
                .unwrap_or(0),
        })
    }

    // ========================================================================
    // Top N Queries
    // ========================================================================

    /// Get top N most expensive requests
    pub fn get_top_expensive_requests(
        &self,
        filter: &UsageFilter,
        limit: i64,
    ) -> AppResult<Vec<crate::models::analytics::UsageRecord>> {
        let modified_filter = filter.clone();
        self.list_usage_records(&modified_filter, Some(limit), None)
    }

    /// Get top N models by cost
    pub fn get_top_models_by_cost(
        &self,
        filter: &UsageFilter,
        limit: usize,
    ) -> AppResult<Vec<ModelUsage>> {
        let all = self.aggregate_by_model(filter)?;
        Ok(all.into_iter().take(limit).collect())
    }

    /// Get top N projects by cost
    pub fn get_top_projects_by_cost(
        &self,
        filter: &UsageFilter,
        limit: usize,
    ) -> AppResult<Vec<ProjectUsage>> {
        let all = self.aggregate_by_project(filter)?;
        Ok(all.into_iter().take(limit).collect())
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Append filter clauses to SQL query
    fn append_filter_clauses(
        sql: &mut String,
        params: &mut Vec<Box<dyn rusqlite::ToSql>>,
        filter: &UsageFilter,
    ) {
        if let Some(ref start) = filter.start_timestamp {
            sql.push_str(" AND timestamp >= ?");
            params.push(Box::new(*start));
        }
        if let Some(ref end) = filter.end_timestamp {
            sql.push_str(" AND timestamp < ?");
            params.push(Box::new(*end));
        }
        if let Some(ref model) = filter.model_name {
            sql.push_str(" AND model_name = ?");
            params.push(Box::new(model.clone()));
        }
        if let Some(ref provider) = filter.provider {
            sql.push_str(" AND provider = ?");
            params.push(Box::new(provider.clone()));
        }
        if let Some(ref session) = filter.session_id {
            sql.push_str(" AND session_id = ?");
            params.push(Box::new(session.clone()));
        }
        if let Some(ref project) = filter.project_id {
            sql.push_str(" AND project_id = ?");
            params.push(Box::new(project.clone()));
        }
    }
}

/// Summary statistics with percentiles
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SummaryStatistics {
    pub min_cost_microdollars: i64,
    pub max_cost_microdollars: i64,
    pub min_tokens: i64,
    pub max_tokens: i64,
    pub total_records: i64,
    pub cost_percentiles: Percentiles,
}

/// Percentile values
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Percentiles {
    pub p50: i64,
    pub p90: i64,
    pub p95: i64,
    pub p99: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::analytics::UsageRecord;
    use r2d2::Pool;
    use r2d2_sqlite::SqliteConnectionManager;

    fn create_test_service() -> AppResult<AnalyticsService> {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder()
            .max_size(5)
            .connection_timeout(std::time::Duration::from_secs(30))
            .build(manager)
            .map_err(|e| AppError::database(e.to_string()))?;
        AnalyticsService::from_pool(pool)
    }

    fn seed_test_data(service: &AnalyticsService) {
        let now = chrono::Utc::now().timestamp();
        let day_ago = now - 86400;
        let two_days_ago = now - 172800;

        let records = vec![
            UsageRecord {
                id: 0,
                session_id: Some("s1".to_string()),
                project_id: Some("p1".to_string()),
                model_name: "claude-3-5-sonnet".to_string(),
                provider: "anthropic".to_string(),
                input_tokens: 1000,
                output_tokens: 500,
                thinking_tokens: 0,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                cost_microdollars: 100,
                timestamp: now,
                metadata: None,
            },
            UsageRecord {
                id: 0,
                session_id: Some("s1".to_string()),
                project_id: Some("p1".to_string()),
                model_name: "claude-3-5-sonnet".to_string(),
                provider: "anthropic".to_string(),
                input_tokens: 2000,
                output_tokens: 1000,
                thinking_tokens: 0,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                cost_microdollars: 200,
                timestamp: day_ago,
                metadata: None,
            },
            UsageRecord {
                id: 0,
                session_id: Some("s2".to_string()),
                project_id: Some("p2".to_string()),
                model_name: "gpt-4o".to_string(),
                provider: "openai".to_string(),
                input_tokens: 500,
                output_tokens: 250,
                thinking_tokens: 0,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                cost_microdollars: 50,
                timestamp: now,
                metadata: None,
            },
            UsageRecord {
                id: 0,
                session_id: Some("s2".to_string()),
                project_id: Some("p2".to_string()),
                model_name: "gpt-4o".to_string(),
                provider: "openai".to_string(),
                input_tokens: 1500,
                output_tokens: 750,
                thinking_tokens: 0,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
                cost_microdollars: 150,
                timestamp: two_days_ago,
                metadata: None,
            },
        ];

        service.insert_usage_records_batch(&records).unwrap();
    }

    #[test]
    fn test_aggregate_by_model() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let results = service.aggregate_by_model(&UsageFilter::default()).unwrap();
        assert_eq!(results.len(), 2);

        // Should be sorted by cost descending
        let claude = results
            .iter()
            .find(|m| m.model_name.contains("claude"))
            .unwrap();
        assert_eq!(claude.stats.request_count, 2);
        assert_eq!(claude.stats.total_cost_microdollars, 300);
    }

    #[test]
    fn test_aggregate_by_project() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let results = service
            .aggregate_by_project(&UsageFilter::default())
            .unwrap();
        assert_eq!(results.len(), 2);

        let p1 = results.iter().find(|p| p.project_id == "p1").unwrap();
        assert_eq!(p1.stats.request_count, 2);
    }

    #[test]
    fn test_time_series_daily() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let results = service
            .get_time_series(&UsageFilter::default(), AggregationPeriod::Daily)
            .unwrap();
        assert!(!results.is_empty());

        // Should have data points for different days
        for point in &results {
            assert!(!point.timestamp_formatted.is_empty());
            assert!(point.stats.request_count > 0);
        }
    }

    #[test]
    fn test_dashboard_summary() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let summary = service
            .get_dashboard_summary(&UsageFilter::default(), AggregationPeriod::Daily)
            .unwrap();

        assert_eq!(summary.current_period.request_count, 4);
        assert!(!summary.by_model.is_empty());
        assert!(!summary.by_project.is_empty());
    }

    #[test]
    fn test_aggregate_with_filter() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let filter = UsageFilter::default().with_provider("anthropic");
        let results = service.aggregate_by_model(&filter).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].provider, "anthropic");
    }

    #[test]
    fn test_summary_statistics() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let stats = service
            .get_summary_statistics(&UsageFilter::default())
            .unwrap();

        assert_eq!(stats.total_records, 4);
        assert!(stats.min_cost_microdollars <= stats.max_cost_microdollars);
    }

    #[test]
    fn test_top_models() {
        let service = create_test_service().unwrap();
        seed_test_data(&service);

        let top = service
            .get_top_models_by_cost(&UsageFilter::default(), 1)
            .unwrap();
        assert_eq!(top.len(), 1);

        // Claude should be most expensive
        assert!(top[0].model_name.contains("claude"));
    }
}
