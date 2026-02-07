//! Analytics Commands
//!
//! Tauri commands for usage analytics, cost tracking, and data export.

use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::analytics::{
    AggregationPeriod, DashboardSummary, ExportFormat, ExportRequest, ExportResult,
    ModelPricing, ModelUsage, ProjectUsage, TimeSeriesPoint, UsageFilter, UsageRecord,
    UsageStats,
};
use crate::models::response::CommandResponse;
use crate::services::analytics::{AnalyticsService, CostCalculator, SummaryStatistics, UsageTracker, UsageTrackerBuilder};
use crate::state::AppState;
use crate::utils::error::{AppError, AppResult};

/// Analytics state managed by Tauri
pub struct AnalyticsState {
    service: Arc<RwLock<Option<AnalyticsService>>>,
    tracker: Arc<RwLock<Option<UsageTracker>>>,
    cost_calculator: Arc<CostCalculator>,
}

impl AnalyticsState {
    pub fn new() -> Self {
        Self {
            service: Arc::new(RwLock::new(None)),
            tracker: Arc::new(RwLock::new(None)),
            cost_calculator: Arc::new(CostCalculator::new()),
        }
    }

    /// Initialize the analytics service
    pub async fn initialize(&self, app_state: &AppState) -> AppResult<()> {
        let mut service_lock = self.service.write().await;
        if service_lock.is_some() {
            return Ok(());
        }

        // Get database pool from app state
        let pool = app_state.with_database(|db| Ok(db.pool().clone())).await?;

        let service = AnalyticsService::from_pool(pool)?;
        let service_arc = Arc::new(service);

        // Initialize tracker
        let tracker = UsageTrackerBuilder::new()
            .buffer_size(100)
            .flush_interval_secs(30)
            .enabled(true)
            .cost_calculator(self.cost_calculator.clone())
            .build(service_arc.clone());

        // Store service (we need to unwrap the Arc for storage)
        *service_lock = Some(AnalyticsService::from_pool(
            app_state.with_database(|db| Ok(db.pool().clone())).await?
        )?);

        let mut tracker_lock = self.tracker.write().await;
        *tracker_lock = Some(tracker);

        Ok(())
    }

    /// Get a reference to the service
    pub async fn with_service<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&AnalyticsService) -> AppResult<T>,
    {
        let guard = self.service.read().await;
        match &*guard {
            Some(service) => f(service),
            None => Err(AppError::internal("Analytics service not initialized")),
        }
    }

    /// Get access to the tracker
    pub async fn with_tracker<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&UsageTracker) -> AppResult<T>,
    {
        let guard = self.tracker.read().await;
        match &*guard {
            Some(tracker) => f(tracker),
            None => Err(AppError::internal("Usage tracker not initialized")),
        }
    }

    /// Get the cost calculator
    pub fn cost_calculator(&self) -> Arc<CostCalculator> {
        self.cost_calculator.clone()
    }
}

impl Default for AnalyticsState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Initialization Commands
// ============================================================================

/// Initialize the analytics service
#[tauri::command]
pub async fn init_analytics(
    app_state: State<'_, AppState>,
    analytics_state: State<'_, AnalyticsState>,
) -> Result<CommandResponse<bool>, String> {
    match analytics_state.initialize(&app_state).await {
        Ok(()) => Ok(CommandResponse::ok(true)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ============================================================================
// Usage Tracking Commands
// ============================================================================

/// Track API usage
#[tauri::command]
pub async fn track_usage(
    analytics_state: State<'_, AnalyticsState>,
    provider: String,
    model_name: String,
    input_tokens: i64,
    output_tokens: i64,
    session_id: Option<String>,
    project_id: Option<String>,
) -> Result<CommandResponse<bool>, String> {
    let result = analytics_state.with_tracker(|_tracker| {
        // We need to run this in a blocking context since the tracker is async
        Ok(())
    }).await;

    match result {
        Ok(()) => {
            // Track using the service directly for now
            let record = UsageRecord::new(&model_name, &provider, input_tokens, output_tokens)
                .with_cost(analytics_state.cost_calculator().calculate_cost(&provider, &model_name, input_tokens, output_tokens));

            let record = if let Some(sid) = session_id {
                record.with_session(sid)
            } else {
                record
            };

            let record = if let Some(pid) = project_id {
                record.with_project(pid)
            } else {
                record
            };

            match analytics_state.with_service(|s| s.insert_usage_record(&record).map(|_| ())).await {
                Ok(()) => Ok(CommandResponse::ok(true)),
                Err(e) => Ok(CommandResponse::err(e.to_string())),
            }
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get current session for tracking
#[tauri::command]
pub async fn get_tracking_session(
    _analytics_state: State<'_, AnalyticsState>,
) -> Result<CommandResponse<Option<String>>, String> {
    // Return None for now - session is managed separately
    Ok(CommandResponse::ok(None))
}

/// Set current session for tracking
#[tauri::command]
pub async fn set_tracking_session(
    _analytics_state: State<'_, AnalyticsState>,
    _session_id: Option<String>,
) -> Result<CommandResponse<bool>, String> {
    // Session management would go here
    Ok(CommandResponse::ok(true))
}

// ============================================================================
// Usage Query Commands
// ============================================================================

/// Get usage statistics with optional filtering
#[tauri::command]
pub async fn get_usage_statistics(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<UsageStats>, String> {
    match analytics_state.with_service(|s| s.get_usage_stats(&filter)).await {
        Ok(stats) => Ok(CommandResponse::ok(stats)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List usage records with filtering and pagination
#[tauri::command]
pub async fn list_usage_records(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<CommandResponse<Vec<UsageRecord>>, String> {
    match analytics_state.with_service(|s| s.list_usage_records(&filter, limit, offset)).await {
        Ok(records) => Ok(CommandResponse::ok(records)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get usage record count
#[tauri::command]
pub async fn count_usage_records(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<i64>, String> {
    match analytics_state.with_service(|s| s.count_usage_records(&filter)).await {
        Ok(count) => Ok(CommandResponse::ok(count)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ============================================================================
// Aggregation Commands
// ============================================================================

/// Get usage aggregated by model
#[tauri::command]
pub async fn aggregate_by_model(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<Vec<ModelUsage>>, String> {
    match analytics_state.with_service(|s| s.aggregate_by_model(&filter)).await {
        Ok(data) => Ok(CommandResponse::ok(data)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get usage aggregated by project
#[tauri::command]
pub async fn aggregate_by_project(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<Vec<ProjectUsage>>, String> {
    match analytics_state.with_service(|s| s.aggregate_by_project(&filter)).await {
        Ok(data) => Ok(CommandResponse::ok(data)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get time series data
#[tauri::command]
pub async fn get_time_series(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    period: AggregationPeriod,
) -> Result<CommandResponse<Vec<TimeSeriesPoint>>, String> {
    match analytics_state.with_service(|s| s.get_time_series(&filter, period)).await {
        Ok(data) => Ok(CommandResponse::ok(data)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get dashboard summary with all data
#[tauri::command]
pub async fn get_dashboard_summary(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    period: AggregationPeriod,
) -> Result<CommandResponse<DashboardSummary>, String> {
    match analytics_state.with_service(|s| s.get_dashboard_summary(&filter, period)).await {
        Ok(summary) => Ok(CommandResponse::ok(summary)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get summary statistics with percentiles
#[tauri::command]
pub async fn get_summary_statistics(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<SummaryStatistics>, String> {
    match analytics_state.with_service(|s| s.get_summary_statistics(&filter)).await {
        Ok(stats) => Ok(CommandResponse::ok(stats)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ============================================================================
// Cost Calculation Commands
// ============================================================================

/// Calculate cost for a given usage
#[tauri::command]
pub async fn calculate_usage_cost(
    analytics_state: State<'_, AnalyticsState>,
    provider: String,
    model_name: String,
    input_tokens: i64,
    output_tokens: i64,
) -> Result<CommandResponse<i64>, String> {
    let cost = analytics_state.cost_calculator().calculate_cost(&provider, &model_name, input_tokens, output_tokens);
    Ok(CommandResponse::ok(cost))
}

/// Get pricing for a model
#[tauri::command]
pub async fn get_model_pricing(
    analytics_state: State<'_, AnalyticsState>,
    provider: String,
    model_name: String,
) -> Result<CommandResponse<Option<ModelPricing>>, String> {
    let pricing = analytics_state.cost_calculator().get_pricing(&provider, &model_name);
    Ok(CommandResponse::ok(pricing))
}

/// List all model pricing
#[tauri::command]
pub async fn list_model_pricing(
    analytics_state: State<'_, AnalyticsState>,
) -> Result<CommandResponse<Vec<ModelPricing>>, String> {
    match analytics_state.cost_calculator().get_all_pricing() {
        Ok(pricing) => Ok(CommandResponse::ok(pricing)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Set custom pricing for a model
#[tauri::command]
pub async fn set_custom_pricing(
    analytics_state: State<'_, AnalyticsState>,
    pricing: ModelPricing,
) -> Result<CommandResponse<bool>, String> {
    let pricing_clone = pricing.clone();
    match analytics_state.cost_calculator().set_custom_pricing(pricing) {
        Ok(()) => {
            // Also persist to database
            let result = analytics_state.with_service(|s| {
                let mut p = pricing_clone.clone();
                p.is_custom = true;
                s.upsert_model_pricing(&p)
            }).await;
            match result {
                Ok(()) => Ok(CommandResponse::ok(true)),
                Err(e) => Ok(CommandResponse::err(e.to_string())),
            }
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Remove custom pricing for a model
#[tauri::command]
pub async fn remove_custom_pricing(
    analytics_state: State<'_, AnalyticsState>,
    provider: String,
    model_name: String,
) -> Result<CommandResponse<bool>, String> {
    match analytics_state.cost_calculator().remove_custom_pricing(&provider, &model_name) {
        Ok(removed) => {
            // Also remove from database
            let _ = analytics_state.with_service(|s| {
                s.delete_model_pricing(&model_name, &provider)
            }).await;
            Ok(CommandResponse::ok(removed))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ============================================================================
// Export Commands
// ============================================================================

/// Export usage data
#[tauri::command]
pub async fn export_usage(
    analytics_state: State<'_, AnalyticsState>,
    request: ExportRequest,
) -> Result<CommandResponse<ExportResult>, String> {
    match analytics_state.with_service(|s| s.export_usage(&request)).await {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Export usage data by model
#[tauri::command]
pub async fn export_by_model(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    format: ExportFormat,
) -> Result<CommandResponse<String>, String> {
    match analytics_state.with_service(|s| s.export_by_model(&filter, format)).await {
        Ok(data) => Ok(CommandResponse::ok(data)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Export usage data by project
#[tauri::command]
pub async fn export_by_project(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    format: ExportFormat,
) -> Result<CommandResponse<String>, String> {
    match analytics_state.with_service(|s| s.export_by_project(&filter, format)).await {
        Ok(data) => Ok(CommandResponse::ok(data)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Export time series data
#[tauri::command]
pub async fn export_time_series(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
    period: AggregationPeriod,
    format: ExportFormat,
) -> Result<CommandResponse<String>, String> {
    match analytics_state.with_service(|s| s.export_time_series(&filter, period, format)).await {
        Ok(data) => Ok(CommandResponse::ok(data)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Export pricing data
#[tauri::command]
pub async fn export_pricing(
    analytics_state: State<'_, AnalyticsState>,
) -> Result<CommandResponse<String>, String> {
    match analytics_state.with_service(|s| s.export_pricing()).await {
        Ok(data) => Ok(CommandResponse::ok(data)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ============================================================================
// Management Commands
// ============================================================================

/// Delete usage records matching filter
#[tauri::command]
pub async fn delete_usage_records(
    analytics_state: State<'_, AnalyticsState>,
    filter: UsageFilter,
) -> Result<CommandResponse<i64>, String> {
    match analytics_state.with_service(|s| s.delete_usage_records(&filter)).await {
        Ok(count) => Ok(CommandResponse::ok(count)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Check if analytics service is healthy
#[tauri::command]
pub async fn check_analytics_health(
    analytics_state: State<'_, AnalyticsState>,
) -> Result<CommandResponse<bool>, String> {
    match analytics_state.with_service(|s| Ok(s.is_healthy())).await {
        Ok(healthy) => Ok(CommandResponse::ok(healthy)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}
