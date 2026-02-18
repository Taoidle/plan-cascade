//! Three-Phase Quality Gate Pipeline
//!
//! Provides a `GatePipeline` orchestrator that executes quality gates in three
//! sequential phases:
//! 1. PRE_VALIDATION - Formatting (FormatGate)
//! 2. VALIDATION - TypeCheck, Test, Lint (parallel via tokio::join!)
//! 3. POST_VALIDATION - AI gates (AiVerificationGate, CodeReviewGate)
//!
//! Each phase has a configurable mode (Soft = warning only, Hard = blocking).
//! The pipeline short-circuits on hard-fail in any phase.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::models::quality_gates::GateStatus;
use crate::utils::error::AppResult;

// ============================================================================
// Enums
// ============================================================================

/// Quality gate execution phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatePhase {
    /// Phase 1: Pre-validation (formatting, auto-fixes)
    PreValidation,
    /// Phase 2: Validation (type check, test, lint - parallel)
    Validation,
    /// Phase 3: Post-validation (AI verification, code review)
    PostValidation,
}

impl std::fmt::Display for GatePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GatePhase::PreValidation => write!(f, "pre_validation"),
            GatePhase::Validation => write!(f, "validation"),
            GatePhase::PostValidation => write!(f, "post_validation"),
        }
    }
}

/// Gate mode determining how failures are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateMode {
    /// Warning only - failures are reported but don't block execution
    Soft,
    /// Blocking - failures stop the pipeline
    Hard,
}

impl Default for GateMode {
    fn default() -> Self {
        GateMode::Hard
    }
}

// ============================================================================
// Gate Trait
// ============================================================================

/// Result from a single pipeline gate execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineGateResult {
    /// Gate identifier
    pub gate_id: String,
    /// Gate display name
    pub gate_name: String,
    /// Phase this gate belongs to
    pub phase: GatePhase,
    /// Whether the gate passed
    pub passed: bool,
    /// Status
    pub status: GateStatus,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Output message
    pub message: String,
    /// Detailed findings (if any)
    pub findings: Vec<String>,
}

impl PipelineGateResult {
    /// Create a passed result.
    pub fn passed(gate_id: &str, gate_name: &str, phase: GatePhase, duration_ms: u64) -> Self {
        Self {
            gate_id: gate_id.to_string(),
            gate_name: gate_name.to_string(),
            phase,
            passed: true,
            status: GateStatus::Passed,
            duration_ms,
            message: "Gate passed".to_string(),
            findings: Vec::new(),
        }
    }

    /// Create a failed result.
    pub fn failed(
        gate_id: &str,
        gate_name: &str,
        phase: GatePhase,
        duration_ms: u64,
        message: String,
        findings: Vec<String>,
    ) -> Self {
        Self {
            gate_id: gate_id.to_string(),
            gate_name: gate_name.to_string(),
            phase,
            passed: false,
            status: GateStatus::Failed,
            duration_ms,
            message,
            findings,
        }
    }

    /// Create a skipped result.
    pub fn skipped(gate_id: &str, gate_name: &str, phase: GatePhase, reason: &str) -> Self {
        Self {
            gate_id: gate_id.to_string(),
            gate_name: gate_name.to_string(),
            phase,
            passed: true,
            status: GateStatus::Skipped,
            duration_ms: 0,
            message: reason.to_string(),
            findings: Vec::new(),
        }
    }
}

// ============================================================================
// Phase Result
// ============================================================================

/// Result of executing all gates in a single phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelinePhaseResult {
    /// Which phase was executed
    pub phase: GatePhase,
    /// Mode for this phase
    pub mode: GateMode,
    /// Whether the phase passed overall
    pub passed: bool,
    /// Individual gate results
    pub gate_results: Vec<PipelineGateResult>,
    /// Total duration for this phase
    pub duration_ms: u64,
}

impl PipelinePhaseResult {
    /// Create a new phase result.
    pub fn new(phase: GatePhase, mode: GateMode, gate_results: Vec<PipelineGateResult>) -> Self {
        let duration_ms = gate_results.iter().map(|r| r.duration_ms).sum();
        let passed = gate_results.iter().all(|r| r.passed);
        Self {
            phase,
            mode,
            passed,
            gate_results,
            duration_ms,
        }
    }

    /// Whether this phase has a hard failure (failed + hard mode).
    pub fn is_hard_fail(&self) -> bool {
        !self.passed && self.mode == GateMode::Hard
    }
}

// ============================================================================
// Pipeline Config
// ============================================================================

/// Phase configuration within the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhaseGateConfig {
    /// Gate mode for this phase
    pub mode: GateMode,
    /// Gate IDs to run in this phase
    pub gate_ids: Vec<String>,
}

impl Default for PhaseGateConfig {
    fn default() -> Self {
        Self {
            mode: GateMode::Hard,
            gate_ids: Vec::new(),
        }
    }
}

/// Pipeline configuration with per-phase gate lists and modes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineConfig {
    /// Phase configurations
    pub phases: HashMap<String, PhaseGateConfig>,
    /// Project root path
    pub project_path: PathBuf,
}

impl PipelineConfig {
    /// Create a default pipeline config for a project path.
    pub fn new(project_path: PathBuf) -> Self {
        let mut phases = HashMap::new();

        // Phase 1: PRE_VALIDATION - formatting (Soft by default, auto-fixes)
        phases.insert(
            GatePhase::PreValidation.to_string(),
            PhaseGateConfig {
                mode: GateMode::Soft,
                gate_ids: vec!["format".to_string()],
            },
        );

        // Phase 2: VALIDATION - type check, test, lint (Hard by default)
        phases.insert(
            GatePhase::Validation.to_string(),
            PhaseGateConfig {
                mode: GateMode::Hard,
                gate_ids: vec![
                    "typecheck".to_string(),
                    "test".to_string(),
                    "lint".to_string(),
                ],
            },
        );

        // Phase 3: POST_VALIDATION - AI gates (Soft by default)
        phases.insert(
            GatePhase::PostValidation.to_string(),
            PhaseGateConfig {
                mode: GateMode::Soft,
                gate_ids: vec![
                    "ai_verify".to_string(),
                    "code_review".to_string(),
                ],
            },
        );

        Self {
            phases,
            project_path,
        }
    }

    /// Get the config for a specific phase.
    pub fn get_phase_config(&self, phase: GatePhase) -> PhaseGateConfig {
        self.phases
            .get(&phase.to_string())
            .cloned()
            .unwrap_or_default()
    }

    /// Set the mode for a specific phase.
    pub fn set_phase_mode(&mut self, phase: GatePhase, mode: GateMode) {
        let config = self
            .phases
            .entry(phase.to_string())
            .or_insert_with(PhaseGateConfig::default);
        config.mode = mode;
    }
}

// ============================================================================
// Pipeline Result
// ============================================================================

/// Overall pipeline execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineResult {
    /// Whether the overall pipeline passed
    pub passed: bool,
    /// Phase results in execution order
    pub phase_results: Vec<PipelinePhaseResult>,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Whether the pipeline was short-circuited
    pub short_circuited: bool,
    /// Which phase caused the short-circuit (if any)
    pub short_circuit_phase: Option<GatePhase>,
}

impl PipelineResult {
    /// Create a new pipeline result from phase results.
    pub fn new(
        phase_results: Vec<PipelinePhaseResult>,
        short_circuited: bool,
        short_circuit_phase: Option<GatePhase>,
    ) -> Self {
        let total_duration_ms = phase_results.iter().map(|r| r.duration_ms).sum();
        let passed = phase_results.iter().all(|r| r.passed || r.mode == GateMode::Soft);
        Self {
            passed,
            phase_results,
            total_duration_ms,
            short_circuited,
            short_circuit_phase,
        }
    }
}

// ============================================================================
// Gate Pipeline
// ============================================================================

/// Callback type for gate execution. Each gate implementor provides a closure
/// that runs the gate and returns a `PipelineGateResult`.
pub type GateExecutor =
    Box<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = PipelineGateResult> + Send>> + Send + Sync>;

/// The main GatePipeline orchestrator.
///
/// Executes gates in three sequential phases:
/// 1. PRE_VALIDATION (sequential)
/// 2. VALIDATION (parallel via tokio::join!)
/// 3. POST_VALIDATION (parallel)
///
/// Short-circuits on hard-fail in any phase.
pub struct GatePipeline {
    /// Pipeline configuration
    config: PipelineConfig,
    /// Registered gate executors by gate_id
    gates: HashMap<String, GateExecutor>,
}

impl GatePipeline {
    /// Create a new pipeline with the given configuration.
    pub fn new(config: PipelineConfig) -> Self {
        Self {
            config,
            gates: HashMap::new(),
        }
    }

    /// Register a gate executor.
    pub fn register_gate(&mut self, gate_id: &str, executor: GateExecutor) {
        self.gates.insert(gate_id.to_string(), executor);
    }

    /// Execute the full three-phase pipeline.
    pub async fn execute(&self) -> AppResult<PipelineResult> {
        let phases = [
            GatePhase::PreValidation,
            GatePhase::Validation,
            GatePhase::PostValidation,
        ];

        let mut phase_results = Vec::new();
        let mut short_circuited = false;
        let mut short_circuit_phase = None;

        for phase in &phases {
            let phase_config = self.config.get_phase_config(*phase);
            let result = self.execute_phase(*phase, &phase_config).await;

            let is_hard_fail = result.is_hard_fail();
            phase_results.push(result);

            if is_hard_fail {
                short_circuited = true;
                short_circuit_phase = Some(*phase);
                break;
            }
        }

        Ok(PipelineResult::new(
            phase_results,
            short_circuited,
            short_circuit_phase,
        ))
    }

    /// Execute a single phase.
    async fn execute_phase(
        &self,
        phase: GatePhase,
        phase_config: &PhaseGateConfig,
    ) -> PipelinePhaseResult {
        let mut gate_results = Vec::new();

        // Collect gates to execute
        let gate_ids: Vec<String> = phase_config
            .gate_ids
            .iter()
            .filter(|id| self.gates.contains_key(*id))
            .cloned()
            .collect();

        if gate_ids.is_empty() {
            return PipelinePhaseResult::new(phase, phase_config.mode, Vec::new());
        }

        match phase {
            GatePhase::PreValidation => {
                // Sequential execution for pre-validation (formatting)
                for gate_id in &gate_ids {
                    if let Some(executor) = self.gates.get(gate_id) {
                        let result = executor().await;
                        gate_results.push(result);
                    }
                }
            }
            GatePhase::Validation | GatePhase::PostValidation => {
                // Parallel execution for validation and post-validation
                let mut futures = Vec::new();
                for gate_id in &gate_ids {
                    if let Some(executor) = self.gates.get(gate_id) {
                        futures.push(executor());
                    }
                }

                let results = futures_util::future::join_all(futures).await;
                gate_results.extend(results);
            }
        }

        PipelinePhaseResult::new(phase, phase_config.mode, gate_results)
    }

    /// Get the pipeline configuration.
    pub fn config(&self) -> &PipelineConfig {
        &self.config
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Instant;

    fn make_passing_gate(gate_id: &str, gate_name: &str, phase: GatePhase) -> (String, GateExecutor) {
        let id = gate_id.to_string();
        let name = gate_name.to_string();
        (
            id.clone(),
            Box::new(move || {
                let id = id.clone();
                let name = name.clone();
                Box::pin(async move {
                    PipelineGateResult::passed(&id, &name, phase, 10)
                })
            }),
        )
    }

    fn make_failing_gate(gate_id: &str, gate_name: &str, phase: GatePhase) -> (String, GateExecutor) {
        let id = gate_id.to_string();
        let name = gate_name.to_string();
        (
            id.clone(),
            Box::new(move || {
                let id = id.clone();
                let name = name.clone();
                Box::pin(async move {
                    PipelineGateResult::failed(
                        &id,
                        &name,
                        phase,
                        5,
                        "Gate failed".to_string(),
                        vec!["Finding 1".to_string()],
                    )
                })
            }),
        )
    }

    #[tokio::test]
    async fn test_pipeline_all_pass() {
        let config = PipelineConfig::new(PathBuf::from("/test"));
        let mut pipeline = GatePipeline::new(config);

        let (id, exec) = make_passing_gate("format", "Format", GatePhase::PreValidation);
        pipeline.register_gate(&id, exec);
        let (id, exec) = make_passing_gate("typecheck", "TypeCheck", GatePhase::Validation);
        pipeline.register_gate(&id, exec);
        let (id, exec) = make_passing_gate("test", "Test", GatePhase::Validation);
        pipeline.register_gate(&id, exec);
        let (id, exec) = make_passing_gate("lint", "Lint", GatePhase::Validation);
        pipeline.register_gate(&id, exec);
        let (id, exec) = make_passing_gate("ai_verify", "AI Verify", GatePhase::PostValidation);
        pipeline.register_gate(&id, exec);
        let (id, exec) = make_passing_gate("code_review", "Code Review", GatePhase::PostValidation);
        pipeline.register_gate(&id, exec);

        let result = pipeline.execute().await.unwrap();
        assert!(result.passed);
        assert!(!result.short_circuited);
        assert!(result.short_circuit_phase.is_none());
        assert_eq!(result.phase_results.len(), 3);
    }

    #[tokio::test]
    async fn test_pipeline_short_circuits_on_hard_fail() {
        let mut config = PipelineConfig::new(PathBuf::from("/test"));
        // Make validation phase hard
        config.set_phase_mode(GatePhase::Validation, GateMode::Hard);

        let mut pipeline = GatePipeline::new(config);

        let (id, exec) = make_passing_gate("format", "Format", GatePhase::PreValidation);
        pipeline.register_gate(&id, exec);
        let (id, exec) = make_failing_gate("typecheck", "TypeCheck", GatePhase::Validation);
        pipeline.register_gate(&id, exec);
        let (id, exec) = make_passing_gate("test", "Test", GatePhase::Validation);
        pipeline.register_gate(&id, exec);
        let (id, exec) = make_passing_gate("lint", "Lint", GatePhase::Validation);
        pipeline.register_gate(&id, exec);
        let (id, exec) = make_passing_gate("ai_verify", "AI Verify", GatePhase::PostValidation);
        pipeline.register_gate(&id, exec);

        let result = pipeline.execute().await.unwrap();
        assert!(!result.passed);
        assert!(result.short_circuited);
        assert_eq!(result.short_circuit_phase, Some(GatePhase::Validation));
        // Should have executed pre-validation and validation, but not post-validation
        assert_eq!(result.phase_results.len(), 2);
    }

    #[tokio::test]
    async fn test_soft_mode_does_not_short_circuit() {
        let mut config = PipelineConfig::new(PathBuf::from("/test"));
        config.set_phase_mode(GatePhase::Validation, GateMode::Soft);

        let mut pipeline = GatePipeline::new(config);

        let (id, exec) = make_passing_gate("format", "Format", GatePhase::PreValidation);
        pipeline.register_gate(&id, exec);
        let (id, exec) = make_failing_gate("typecheck", "TypeCheck", GatePhase::Validation);
        pipeline.register_gate(&id, exec);
        let (id, exec) = make_passing_gate("ai_verify", "AI Verify", GatePhase::PostValidation);
        pipeline.register_gate(&id, exec);

        let result = pipeline.execute().await.unwrap();
        // Soft mode failure should not block - all 3 phases execute
        assert!(!result.short_circuited);
        assert_eq!(result.phase_results.len(), 3);
        // Validation failed but it's soft, so pipeline passes
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_pipeline_execution_order() {
        let config = PipelineConfig::new(PathBuf::from("/test"));
        let mut pipeline = GatePipeline::new(config);

        let order = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        // Pre-validation gate
        {
            let order = order.clone();
            pipeline.register_gate(
                "format",
                Box::new(move || {
                    let order = order.clone();
                    Box::pin(async move {
                        order.lock().await.push("format");
                        PipelineGateResult::passed("format", "Format", GatePhase::PreValidation, 1)
                    })
                }),
            );
        }

        // Validation gate
        {
            let order = order.clone();
            pipeline.register_gate(
                "typecheck",
                Box::new(move || {
                    let order = order.clone();
                    Box::pin(async move {
                        order.lock().await.push("typecheck");
                        PipelineGateResult::passed("typecheck", "TypeCheck", GatePhase::Validation, 1)
                    })
                }),
            );
        }

        // Post-validation gate
        {
            let order = order.clone();
            pipeline.register_gate(
                "ai_verify",
                Box::new(move || {
                    let order = order.clone();
                    Box::pin(async move {
                        order.lock().await.push("ai_verify");
                        PipelineGateResult::passed("ai_verify", "AI Verify", GatePhase::PostValidation, 1)
                    })
                }),
            );
        }

        let _result = pipeline.execute().await.unwrap();
        let exec_order = order.lock().await;
        // Pre-validation should come first, then validation, then post-validation
        assert_eq!(exec_order[0], "format");
        assert!(exec_order.contains(&"typecheck"));
        assert!(exec_order.contains(&"ai_verify"));
    }

    #[tokio::test]
    async fn test_empty_pipeline() {
        let config = PipelineConfig::new(PathBuf::from("/test"));
        let pipeline = GatePipeline::new(config);
        // No gates registered

        let result = pipeline.execute().await.unwrap();
        assert!(result.passed);
        assert!(!result.short_circuited);
        assert_eq!(result.phase_results.len(), 3);
    }

    #[test]
    fn test_pipeline_config_default() {
        let config = PipelineConfig::new(PathBuf::from("/test"));

        let pre = config.get_phase_config(GatePhase::PreValidation);
        assert_eq!(pre.mode, GateMode::Soft);
        assert!(pre.gate_ids.contains(&"format".to_string()));

        let val = config.get_phase_config(GatePhase::Validation);
        assert_eq!(val.mode, GateMode::Hard);
        assert!(val.gate_ids.contains(&"typecheck".to_string()));
        assert!(val.gate_ids.contains(&"test".to_string()));
        assert!(val.gate_ids.contains(&"lint".to_string()));

        let post = config.get_phase_config(GatePhase::PostValidation);
        assert_eq!(post.mode, GateMode::Soft);
        assert!(post.gate_ids.contains(&"ai_verify".to_string()));
        assert!(post.gate_ids.contains(&"code_review".to_string()));
    }

    #[test]
    fn test_gate_phase_serialization() {
        let json = serde_json::to_string(&GatePhase::PreValidation).unwrap();
        assert_eq!(json, "\"pre_validation\"");
        let json = serde_json::to_string(&GatePhase::Validation).unwrap();
        assert_eq!(json, "\"validation\"");
        let json = serde_json::to_string(&GatePhase::PostValidation).unwrap();
        assert_eq!(json, "\"post_validation\"");
    }

    #[test]
    fn test_gate_mode_serialization() {
        let json = serde_json::to_string(&GateMode::Soft).unwrap();
        assert_eq!(json, "\"soft\"");
        let json = serde_json::to_string(&GateMode::Hard).unwrap();
        assert_eq!(json, "\"hard\"");
    }

    #[test]
    fn test_pipeline_gate_result_serialization() {
        let result = PipelineGateResult::passed("format", "Format", GatePhase::PreValidation, 42);
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"gateId\""));
        assert!(json.contains("\"gateName\""));
        assert!(json.contains("\"durationMs\""));
    }

    #[test]
    fn test_phase_result_hard_fail() {
        let gate_results = vec![
            PipelineGateResult::passed("test1", "Test1", GatePhase::Validation, 10),
            PipelineGateResult::failed("test2", "Test2", GatePhase::Validation, 5, "err".to_string(), vec![]),
        ];
        let result = PipelinePhaseResult::new(GatePhase::Validation, GateMode::Hard, gate_results);
        assert!(!result.passed);
        assert!(result.is_hard_fail());
    }

    #[test]
    fn test_phase_result_soft_fail() {
        let gate_results = vec![
            PipelineGateResult::failed("test", "Test", GatePhase::PreValidation, 5, "err".to_string(), vec![]),
        ];
        let result = PipelinePhaseResult::new(GatePhase::PreValidation, GateMode::Soft, gate_results);
        assert!(!result.passed);
        assert!(!result.is_hard_fail()); // Soft mode, so not a hard fail
    }
}
