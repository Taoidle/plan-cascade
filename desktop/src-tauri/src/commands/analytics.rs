//! Analytics Commands
//!
//! Tauri commands for usage analytics, cost tracking, and data export.

use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::analytics::{
    AggregationPeriod, AnalyticsBreakdownRow, AnalyticsEventDetail, AnalyticsFilter,
    AnalyticsSummary, AnalyticsUsageEvent, DashboardFilterV2, DashboardSummary, ExportJob,
    ExportStreamingJobRequest, PricingRule, RecomputeCostsRequest, RecomputeCostsResult,
    UsageFilter, UsageRecordV2,
};
use crate::models::response::CommandResponse;
use crate::services::analytics::{
    AnalyticsService, CostCalculator, UsageTracker, UsageTrackerBuilder,
};
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
            app_state.with_database(|db| Ok(db.pool().clone())).await?,
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

    /// Get the cost calculator
    pub fn cost_calculator(&self) -> Arc<CostCalculator> {
        self.cost_calculator.clone()
    }

    /// Get a clone of the tracker's channel sender (for injection into orchestrator)
    pub async fn get_tracker_sender(
        &self,
    ) -> Option<tokio::sync::mpsc::Sender<crate::services::analytics::TrackerMessage>> {
        let guard = self.tracker.read().await;
        guard.as_ref().map(|t| t.sender())
    }

    pub async fn get_tracker_components(
        &self,
    ) -> Option<(
        tokio::sync::mpsc::Sender<crate::services::analytics::TrackerMessage>,
        Arc<CostCalculator>,
    )> {
        self.get_tracker_sender()
            .await
            .map(|sender| (sender, self.cost_calculator()))
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

/// List usage records with advanced filters (provider/model/project/session/cost status).
#[tauri::command]
pub async fn list_usage_records_v2(
    analytics_state: State<'_, AnalyticsState>,
    filter: DashboardFilterV2,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<CommandResponse<Vec<UsageRecordV2>>, String> {
    match analytics_state
        .with_service(|s| s.list_usage_records_v2(&filter, limit, offset))
        .await
    {
        Ok(records) => Ok(CommandResponse::ok(records)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn list_usage_events(
    analytics_state: State<'_, AnalyticsState>,
    filter: AnalyticsFilter,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<CommandResponse<Vec<AnalyticsUsageEvent>>, String> {
    match analytics_state
        .with_service(|s| s.list_usage_events(&filter, limit, offset))
        .await
    {
        Ok(events) => Ok(CommandResponse::ok(events)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn count_usage_events(
    analytics_state: State<'_, AnalyticsState>,
    filter: AnalyticsFilter,
) -> Result<CommandResponse<i64>, String> {
    match analytics_state
        .with_service(|s| s.count_usage_events(&filter))
        .await
    {
        Ok(count) => Ok(CommandResponse::ok(count)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn get_analytics_summary(
    analytics_state: State<'_, AnalyticsState>,
    filter: AnalyticsFilter,
    period: Option<AggregationPeriod>,
) -> Result<CommandResponse<AnalyticsSummary>, String> {
    let period = period.unwrap_or(AggregationPeriod::Daily);
    match analytics_state
        .with_service(|s| s.get_analytics_summary(&filter, period))
        .await
    {
        Ok(summary) => Ok(CommandResponse::ok(summary)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn get_usage_breakdown(
    analytics_state: State<'_, AnalyticsState>,
    filter: AnalyticsFilter,
    dimension: String,
) -> Result<CommandResponse<Vec<AnalyticsBreakdownRow>>, String> {
    match analytics_state
        .with_service(|s| s.get_usage_breakdown(&filter, &dimension))
        .await
    {
        Ok(rows) => Ok(CommandResponse::ok(rows)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn get_usage_event_detail(
    analytics_state: State<'_, AnalyticsState>,
    event_id: String,
) -> Result<CommandResponse<Option<AnalyticsEventDetail>>, String> {
    match analytics_state
        .with_service(|s| s.get_usage_event_detail(&event_id))
        .await
    {
        Ok(detail) => Ok(CommandResponse::ok(detail)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Count v2 usage records.
#[tauri::command]
pub async fn count_usage_records_v2(
    analytics_state: State<'_, AnalyticsState>,
    filter: DashboardFilterV2,
) -> Result<CommandResponse<i64>, String> {
    match analytics_state
        .with_service(|s| s.count_usage_records_v2(&filter))
        .await
    {
        Ok(count) => Ok(CommandResponse::ok(count)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get dashboard summary from v2 tables.
#[tauri::command]
pub async fn get_dashboard_summary_v2(
    analytics_state: State<'_, AnalyticsState>,
    filter: DashboardFilterV2,
    period: Option<AggregationPeriod>,
) -> Result<CommandResponse<DashboardSummary>, String> {
    let period = period.unwrap_or(AggregationPeriod::Daily);
    match analytics_state
        .with_service(|s| s.get_dashboard_summary_v2(&filter, period))
        .await
    {
        Ok(summary) => Ok(CommandResponse::ok(summary)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List pricing rules (manual maintenance).
#[tauri::command]
pub async fn list_pricing_rules(
    analytics_state: State<'_, AnalyticsState>,
) -> Result<CommandResponse<Vec<PricingRule>>, String> {
    match analytics_state
        .with_service(|s| s.list_pricing_rules())
        .await
    {
        Ok(rules) => Ok(CommandResponse::ok(rules)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Create or update pricing rule.
#[tauri::command]
pub async fn upsert_pricing_rule(
    analytics_state: State<'_, AnalyticsState>,
    rule: PricingRule,
) -> Result<CommandResponse<PricingRule>, String> {
    match analytics_state
        .with_service(|s| s.upsert_pricing_rule(&rule))
        .await
    {
        Ok(saved) => Ok(CommandResponse::ok(saved)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Delete pricing rule.
#[tauri::command]
pub async fn delete_pricing_rule(
    analytics_state: State<'_, AnalyticsState>,
    rule_id: String,
) -> Result<CommandResponse<bool>, String> {
    match analytics_state
        .with_service(|s| s.delete_pricing_rule(&rule_id))
        .await
    {
        Ok(ok) => Ok(CommandResponse::ok(ok)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Recompute costs by range/filter.
#[tauri::command]
pub async fn recompute_costs(
    analytics_state: State<'_, AnalyticsState>,
    request: RecomputeCostsRequest,
) -> Result<CommandResponse<RecomputeCostsResult>, String> {
    match analytics_state
        .with_service(|s| s.recompute_costs(&request))
        .await
    {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Export v2 usage data to a local file with streaming writes.
#[tauri::command]
pub async fn export_usage_streaming_job(
    analytics_state: State<'_, AnalyticsState>,
    request: ExportStreamingJobRequest,
) -> Result<CommandResponse<ExportJob>, String> {
    match analytics_state
        .with_service(|s| s.export_usage_streaming_job(&request))
        .await
    {
        Ok(job) => Ok(CommandResponse::ok(job)),
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
    match analytics_state
        .with_service(|s| s.delete_usage_records(&filter))
        .await
    {
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
