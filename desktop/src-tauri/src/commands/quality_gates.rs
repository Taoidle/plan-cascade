//! Quality Gates Commands
//!
//! Tauri commands for project type detection and quality gate execution.

use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::models::quality_gates::{
    CustomGateConfig, GatesSummary, ProjectDetectionResult, ProjectType, QualityGate,
    StoredGateResult,
};
use crate::models::response::CommandResponse;
use crate::services::quality_gates::{
    detect_project_type, get_default_gates, GateCache, QualityGateRunner, QualityGatesStore,
    ValidatorRegistry,
};
use crate::state::AppState;
use crate::utils::error::{AppError, AppResult};

/// Quality gates state managed by Tauri
pub struct QualityGatesState {
    store: Arc<RwLock<Option<QualityGatesStore>>>,
    registry: Arc<ValidatorRegistry>,
    cache: Arc<RwLock<Option<Arc<GateCache>>>>,
}

impl QualityGatesState {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(None)),
            registry: Arc::new(ValidatorRegistry::new()),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialize the quality gates store and cache
    pub async fn initialize(&self, app_state: &AppState) -> AppResult<()> {
        let mut store_lock = self.store.write().await;
        if store_lock.is_some() {
            return Ok(());
        }

        // Get database pool from app state
        let pool = app_state.with_database(|db| Ok(db.pool().clone())).await?;
        let store = QualityGatesStore::new(pool.clone())?;
        *store_lock = Some(store);

        // Initialize the gate cache using the same database pool
        let mut cache_lock = self.cache.write().await;
        let gate_cache = GateCache::new(pool)?;
        *cache_lock = Some(Arc::new(gate_cache));

        Ok(())
    }

    /// Get access to the store
    pub async fn with_store<F, T>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&QualityGatesStore) -> AppResult<T>,
    {
        let guard = self.store.read().await;
        match &*guard {
            Some(store) => f(store),
            None => Err(AppError::internal("Quality gates store not initialized")),
        }
    }

    /// Get the validator registry
    pub fn registry(&self) -> &ValidatorRegistry {
        &self.registry
    }

    /// Get access to the gate cache as an Arc (if initialized)
    pub async fn get_cache(&self) -> Option<Arc<GateCache>> {
        let guard = self.cache.read().await;
        guard.clone()
    }
}

impl Default for QualityGatesState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Initialization Commands
// ============================================================================

/// Initialize the quality gates service
#[tauri::command]
pub async fn init_quality_gates(
    app_state: State<'_, AppState>,
    quality_state: State<'_, QualityGatesState>,
) -> Result<CommandResponse<bool>, String> {
    match quality_state.initialize(&app_state).await {
        Ok(()) => Ok(CommandResponse::ok(true)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ============================================================================
// Detection Commands
// ============================================================================

/// Detect the project type for a given path
#[tauri::command]
pub async fn detect_project_type_cmd(
    project_path: String,
) -> Result<CommandResponse<ProjectDetectionResult>, String> {
    match detect_project_type(&project_path) {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get available quality gates for a project type
#[tauri::command]
pub async fn get_available_gates(
    project_type: ProjectType,
    quality_state: State<'_, QualityGatesState>,
) -> Result<CommandResponse<Vec<QualityGate>>, String> {
    let gates = quality_state
        .registry()
        .get_for_project_type(project_type)
        .into_iter()
        .cloned()
        .collect();
    Ok(CommandResponse::ok(gates))
}

/// Get all registered quality gates
#[tauri::command]
pub async fn list_all_gates(
    quality_state: State<'_, QualityGatesState>,
) -> Result<CommandResponse<Vec<QualityGate>>, String> {
    let gates = quality_state
        .registry()
        .all()
        .into_iter()
        .cloned()
        .collect();
    Ok(CommandResponse::ok(gates))
}

// ============================================================================
// Execution Commands
// ============================================================================

/// Run all quality gates for a project
#[tauri::command]
pub async fn run_quality_gates(
    app_state: State<'_, AppState>,
    project_path: String,
    session_id: Option<String>,
) -> Result<CommandResponse<GatesSummary>, String> {
    // Get database pool
    let pool_result = app_state.with_database(|db| Ok(db.pool().clone())).await;

    let mut runner = QualityGateRunner::new(&project_path);

    if let Ok(pool) = pool_result {
        runner = runner.with_database(pool);
    }

    if let Some(sid) = session_id {
        runner = runner.with_session(sid);
    }

    match runner.run_all().await {
        Ok(summary) => Ok(CommandResponse::ok(summary)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Run specific quality gates by ID
#[tauri::command]
pub async fn run_specific_gates(
    app_state: State<'_, AppState>,
    project_path: String,
    gate_ids: Vec<String>,
    session_id: Option<String>,
) -> Result<CommandResponse<GatesSummary>, String> {
    // Get database pool
    let pool_result = app_state.with_database(|db| Ok(db.pool().clone())).await;

    let mut runner = QualityGateRunner::new(&project_path);

    if let Ok(pool) = pool_result {
        runner = runner.with_database(pool);
    }

    if let Some(sid) = session_id {
        runner = runner.with_session(sid);
    }

    match runner.run_specific(&gate_ids).await {
        Ok(summary) => Ok(CommandResponse::ok(summary)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Run custom quality gates from configuration
#[tauri::command]
pub async fn run_custom_gates(
    app_state: State<'_, AppState>,
    project_path: String,
    custom_gates: Vec<CustomGateConfig>,
    session_id: Option<String>,
) -> Result<CommandResponse<GatesSummary>, String> {
    // Get database pool
    let pool_result = app_state.with_database(|db| Ok(db.pool().clone())).await;

    let mut runner = QualityGateRunner::new(&project_path);

    if let Ok(pool) = pool_result {
        runner = runner.with_database(pool);
    }

    if let Some(sid) = session_id {
        runner = runner.with_session(sid);
    }

    match runner.run_custom(custom_gates).await {
        Ok(summary) => Ok(CommandResponse::ok(summary)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ============================================================================
// Results Commands
// ============================================================================

/// Get stored gate results for a project
#[tauri::command]
pub async fn get_gate_results(
    quality_state: State<'_, QualityGatesState>,
    project_path: String,
    limit: Option<i64>,
) -> Result<CommandResponse<Vec<StoredGateResult>>, String> {
    match quality_state
        .with_store(|store| store.get_results_for_project(&project_path, limit))
        .await
    {
        Ok(results) => Ok(CommandResponse::ok(results)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get stored gate results for a session
#[tauri::command]
pub async fn get_session_gate_results(
    quality_state: State<'_, QualityGatesState>,
    session_id: String,
) -> Result<CommandResponse<Vec<StoredGateResult>>, String> {
    match quality_state
        .with_store(|store| store.get_results_for_session(&session_id))
        .await
    {
        Ok(results) => Ok(CommandResponse::ok(results)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get a single gate result by ID
#[tauri::command]
pub async fn get_gate_result(
    quality_state: State<'_, QualityGatesState>,
    result_id: i64,
) -> Result<CommandResponse<Option<StoredGateResult>>, String> {
    match quality_state
        .with_store(|store| store.get_result(result_id))
        .await
    {
        Ok(result) => Ok(CommandResponse::ok(result)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Clean up old gate results
#[tauri::command]
pub async fn cleanup_gate_results(
    quality_state: State<'_, QualityGatesState>,
    days_old: i64,
) -> Result<CommandResponse<i64>, String> {
    match quality_state
        .with_store(|store| store.cleanup_old_results(days_old))
        .await
    {
        Ok(count) => Ok(CommandResponse::ok(count)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ============================================================================
// Utility Commands
// ============================================================================

/// Get default gates for a project type
#[tauri::command]
pub async fn get_default_gates_for_type(
    project_type: ProjectType,
) -> Result<CommandResponse<Vec<QualityGate>>, String> {
    let gates = get_default_gates(project_type);
    Ok(CommandResponse::ok(gates))
}

/// Check if quality gates service is healthy
#[tauri::command]
pub async fn check_quality_gates_health(
    quality_state: State<'_, QualityGatesState>,
) -> Result<CommandResponse<bool>, String> {
    let result = quality_state.with_store(|_| Ok(true)).await;

    match result {
        Ok(healthy) => Ok(CommandResponse::ok(healthy)),
        Err(_) => Ok(CommandResponse::ok(false)),
    }
}
