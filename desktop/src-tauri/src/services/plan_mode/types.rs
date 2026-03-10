//! Plan Mode Core Types
//!
//! Data structures for the domain-agnostic task decomposition framework.
//! Plan Mode decomposes arbitrary tasks (writing, research, marketing, etc.)
//! into Steps with dependencies and executes them in parallel batches.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// Domain & Phase Types
// ============================================================================

/// Task domain classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskDomain {
    General,
    Writing,
    Research,
    Marketing,
    DataAnalysis,
    ProjectManagement,
    Custom(String),
}

impl std::fmt::Display for TaskDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskDomain::General => write!(f, "General"),
            TaskDomain::Writing => write!(f, "Writing"),
            TaskDomain::Research => write!(f, "Research"),
            TaskDomain::Marketing => write!(f, "Marketing"),
            TaskDomain::DataAnalysis => write!(f, "Data Analysis"),
            TaskDomain::ProjectManagement => write!(f, "Project Management"),
            TaskDomain::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// Plan Mode execution phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanModePhase {
    Idle,
    Analyzing,
    Clarifying,
    Planning,
    ReviewingPlan,
    Executing,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for PlanModePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanModePhase::Idle => write!(f, "Idle"),
            PlanModePhase::Analyzing => write!(f, "Analyzing"),
            PlanModePhase::Clarifying => write!(f, "Clarifying"),
            PlanModePhase::Planning => write!(f, "Planning"),
            PlanModePhase::ReviewingPlan => write!(f, "Reviewing Plan"),
            PlanModePhase::Executing => write!(f, "Executing"),
            PlanModePhase::Completed => write!(f, "Completed"),
            PlanModePhase::Failed => write!(f, "Failed"),
            PlanModePhase::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Plan Mode persona roles (independent from Task Mode's PersonaRole).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanPersonaRole {
    /// Strategic task decomposition, dependency analysis
    Planner,
    /// Requirements clarification, goal understanding
    Analyst,
    /// Task completion, tool usage
    Executor,
    /// Output quality validation
    Reviewer,
}

impl PlanPersonaRole {
    pub fn display_name(&self) -> &'static str {
        match self {
            PlanPersonaRole::Planner => "Planner",
            PlanPersonaRole::Analyst => "Analyst",
            PlanPersonaRole::Executor => "Executor",
            PlanPersonaRole::Reviewer => "Reviewer",
        }
    }

    pub fn id(&self) -> &'static str {
        match self {
            PlanPersonaRole::Planner => "planner",
            PlanPersonaRole::Analyst => "analyst",
            PlanPersonaRole::Executor => "executor",
            PlanPersonaRole::Reviewer => "reviewer",
        }
    }
}

impl std::fmt::Display for PlanPersonaRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// ============================================================================
// Plan Step Types
// ============================================================================

/// A single step in a Plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    /// Unique step identifier (e.g., "step-1")
    pub id: String,
    /// Step title
    pub title: String,
    /// Detailed description of what this step should accomplish
    pub description: String,
    /// Priority level
    pub priority: StepPriority,
    /// Dependencies (step IDs that must complete before this step)
    pub dependencies: Vec<String>,
    /// Structured completion contract for the expected deliverable.
    #[serde(default)]
    pub deliverable: StepDeliverableContract,
    /// Structured runtime evidence requirements.
    #[serde(default)]
    pub evidence_requirements: StepEvidenceRequirements,
    /// Structured quality and semantic expectations.
    #[serde(default)]
    pub quality_requirements: StepQualityRequirements,
    /// Validation profile that selects default validation behavior.
    #[serde(default)]
    pub validation_profile: StepValidationProfile,
    /// Failure policy controlling retries and downstream behavior.
    #[serde(default)]
    pub failure_policy: StepFailurePolicy,
    /// Criteria that determine when this step is complete
    #[serde(default)]
    pub completion_criteria: Vec<String>,
    /// Description of the expected output format/content
    #[serde(default)]
    pub expected_output: String,
    /// Additional domain-specific metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepDeliverableContract {
    #[serde(default)]
    pub deliverable_type: StepDeliverableType,
    #[serde(default)]
    pub format: StepDeliverableFormat,
    #[serde(default)]
    pub required_sections: Vec<String>,
    #[serde(default)]
    pub required_artifacts: Vec<ArtifactRequirement>,
    #[serde(default)]
    pub expected_output_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepDeliverableType {
    Report,
    Markdown,
    Json,
    FilePatch,
    CodeChange,
    ArtifactBundle,
    ResearchSummary,
    AnalysisMemo,
    Custom,
}

impl Default for StepDeliverableType {
    fn default() -> Self {
        Self::Custom
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepDeliverableFormat {
    Markdown,
    Json,
    Text,
    Code,
    Mixed,
}

impl Default for StepDeliverableFormat {
    fn default() -> Self {
        Self::Text
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRequirement {
    #[serde(default)]
    pub artifact_type: String,
    #[serde(default)]
    pub path_hint: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepEvidenceRequirements {
    #[serde(default)]
    pub min_files_read: usize,
    #[serde(default)]
    pub required_paths: Vec<String>,
    #[serde(default)]
    pub required_tools: Vec<String>,
    #[serde(default)]
    pub required_searches: Vec<String>,
    #[serde(default)]
    pub required_artifact_types: Vec<String>,
    #[serde(default)]
    pub dependency_evidence_mode: DependencyEvidenceMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DependencyEvidenceMode {
    None,
    Optional,
    Required,
}

impl Default for DependencyEvidenceMode {
    fn default() -> Self {
        Self::Optional
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepQualityRequirements {
    #[serde(default)]
    pub must_cover_topics: Vec<String>,
    #[serde(default)]
    pub must_reference_evidence: bool,
    #[serde(default)]
    pub must_include_reasoning_links: bool,
    #[serde(default)]
    pub must_pass_checks: Vec<ValidationCheck>,
    #[serde(default)]
    pub semantic_expectations: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationCheck {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub severity: ValidationSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepValidationProfile {
    Report,
    Analysis,
    Research,
    CodeChange,
    Documentation,
    Mixed,
}

impl Default for StepValidationProfile {
    fn default() -> Self {
        Self::Mixed
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepFailurePolicy {
    #[serde(default)]
    pub severity: FailureSeverity,
    #[serde(default = "default_step_failure_retries")]
    pub max_auto_retries: usize,
    #[serde(default)]
    pub allow_downstream_on_soft_fail: bool,
}

fn default_step_failure_retries() -> usize {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureSeverity {
    Hard,
    Soft,
    Review,
}

impl Default for FailureSeverity {
    fn default() -> Self {
        Self::Hard
    }
}

/// Step priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepPriority {
    High,
    Medium,
    Low,
}

impl std::fmt::Display for StepPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepPriority::High => write!(f, "High"),
            StepPriority::Medium => write!(f, "Medium"),
            StepPriority::Low => write!(f, "Low"),
        }
    }
}

/// A batch of steps that can be executed in parallel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanBatch {
    /// Batch index (0-based)
    pub index: usize,
    /// Step IDs in this batch
    pub step_ids: Vec<String>,
}

// ============================================================================
// Plan Type
// ============================================================================

/// A complete Plan with steps and execution batches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// Plan title
    pub title: String,
    /// Overall plan description
    pub description: String,
    /// Detected task domain
    pub domain: TaskDomain,
    /// Adapter used for this plan
    pub adapter_name: String,
    /// Steps to execute
    pub steps: Vec<PlanStep>,
    /// Execution batches (calculated from step dependencies)
    pub batches: Vec<PlanBatch>,
    /// Execution-level settings configurable during plan editing.
    #[serde(default)]
    pub execution_config: PlanExecutionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecutionConfig {
    /// Maximum number of parallel step executions.
    pub max_parallel: usize,
    /// Max orchestrator iterations for a single step execution.
    #[serde(default = "default_max_step_iterations")]
    pub max_step_iterations: u32,
    /// Automatic retry behavior for failed or incomplete steps.
    #[serde(default)]
    pub retry: PlanRetryPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRetryPolicy {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Number of retries after the first attempt.
    #[serde(default = "default_retry_max_attempts")]
    pub max_attempts: usize,
    #[serde(default = "default_retry_backoff_ms")]
    pub backoff_ms: u64,
    #[serde(default = "default_true")]
    pub fail_batch_on_exhausted: bool,
}

fn default_retry_max_attempts() -> usize {
    2
}

fn default_retry_backoff_ms() -> u64 {
    800
}

fn default_max_step_iterations() -> u32 {
    36
}

impl Default for PlanRetryPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            max_attempts: default_retry_max_attempts(),
            backoff_ms: default_retry_backoff_ms(),
            fail_batch_on_exhausted: true,
        }
    }
}

impl PlanRetryPolicy {
    pub fn normalized(&self) -> Self {
        Self {
            enabled: self.enabled,
            max_attempts: self.max_attempts.min(5),
            backoff_ms: self.backoff_ms.clamp(100, 5_000),
            fail_batch_on_exhausted: self.fail_batch_on_exhausted,
        }
    }
}

impl Default for PlanExecutionConfig {
    fn default() -> Self {
        Self {
            max_parallel: 4,
            max_step_iterations: default_max_step_iterations(),
            retry: PlanRetryPolicy::default(),
        }
    }
}

impl PlanExecutionConfig {
    pub fn normalized_max_parallel(&self) -> usize {
        self.max_parallel.clamp(1, 8)
    }

    pub fn normalized_retry_policy(&self) -> PlanRetryPolicy {
        self.retry.normalized()
    }

    pub fn normalized_max_step_iterations(&self) -> u32 {
        self.max_step_iterations.clamp(12, 96)
    }
}

// ============================================================================
// Analysis & Output Types
// ============================================================================

/// Result of the analysis phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanAnalysis {
    /// Detected task domain
    pub domain: TaskDomain,
    /// Estimated complexity (1-10)
    pub complexity: u8,
    /// Estimated number of steps
    pub estimated_steps: usize,
    /// Whether the task needs clarification before planning
    pub needs_clarification: bool,
    /// Reasoning for the analysis
    pub reasoning: String,
    /// Selected adapter name
    pub adapter_name: String,
    /// Suggested high-level approach
    pub suggested_approach: String,
}

/// A clarification question for the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarificationQuestion {
    /// Unique question ID
    pub question_id: String,
    /// The question text
    pub question: String,
    /// Hint or example answer
    pub hint: Option<String>,
    /// Input type for the question
    pub input_type: ClarificationInputType,
    /// Whether the user can provide a custom answer for select types
    #[serde(default = "default_true")]
    pub allow_custom: bool,
}

/// Input types for clarification questions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClarificationInputType {
    Text,
    Textarea,
    SingleSelect(Vec<String>),
    MultiSelect(Vec<String>),
    Boolean,
}

/// User's answer to a clarification question.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClarificationAnswer {
    /// Question ID
    pub question_id: String,
    /// The answer text
    pub answer: String,
    /// Whether the question was skipped
    pub skipped: bool,
    /// Original question text (for context in subsequent LLM calls)
    #[serde(default)]
    pub question_text: String,
}

/// Result of validating a single completion criterion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CriterionResult {
    /// The criterion text
    pub criterion: String,
    /// Whether it was met
    pub met: bool,
    /// Explanation of why it was met or not
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Hard,
    Soft,
    Review,
}

impl Default for ValidationSeverity {
    fn default() -> Self {
        Self::Soft
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepValidationStatus {
    Passed,
    SoftFailed,
    NeedsReview,
    HardFailed,
}

impl Default for StepValidationStatus {
    fn default() -> Self {
        Self::Passed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepOutcomeStatus {
    Completed,
    SoftFailed,
    NeedsReview,
    HardFailed,
}

impl Default for StepOutcomeStatus {
    fn default() -> Self {
        Self::Completed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepReviewReason {
    ReviewRequired,
    LowSemanticConfidence,
    AmbiguousEvidence,
    SemanticGap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepFailureBucket {
    MissingEvidence,
    DeliverableIncomplete,
    SemanticGap,
    ReviewRequired,
    ExecutionError,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationEvidenceRef {
    #[serde(default)]
    pub reference_type: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationCheckResult {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub severity: ValidationSeverity,
    #[serde(default)]
    pub explanation: String,
    #[serde(default)]
    pub evidence_refs: Vec<ValidationEvidenceRef>,
    #[serde(default)]
    pub missing_items: Vec<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StepValidationResult {
    #[serde(default)]
    pub status: StepValidationStatus,
    #[serde(default)]
    pub outcome_status: StepOutcomeStatus,
    #[serde(default)]
    pub failure_bucket: Option<StepFailureBucket>,
    #[serde(default)]
    pub checks: Vec<ValidationCheckResult>,
    #[serde(default)]
    pub unmet_checks: Vec<ValidationCheckResult>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub retry_guidance: Vec<String>,
    #[serde(default)]
    pub review_reason: Option<StepReviewReason>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepToolCallEvidence {
    #[serde(default)]
    pub tool_name: String,
    #[serde(default)]
    pub args_summary: String,
    #[serde(default)]
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepFileReadEvidence {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub read_count: usize,
    #[serde(default)]
    pub bytes: u64,
    #[serde(default)]
    pub matched_required_path: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepArtifactEvidence {
    #[serde(default)]
    pub artifact_type: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepRuntimeStats {
    #[serde(default)]
    pub iterations: u32,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub attempt_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepEvidenceBundle {
    #[serde(default)]
    pub tool_calls: Vec<StepToolCallEvidence>,
    #[serde(default)]
    pub files_read: Vec<StepFileReadEvidence>,
    #[serde(default)]
    pub files_written: Vec<String>,
    #[serde(default)]
    pub search_queries: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<StepArtifactEvidence>,
    #[serde(default)]
    pub dependency_inputs: Vec<String>,
    #[serde(default)]
    pub runtime_stats: StepRuntimeStats,
    #[serde(default)]
    pub coverage_markers: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepEvidenceSummary {
    #[serde(default)]
    pub files_read_count: usize,
    #[serde(default)]
    pub files_written_count: usize,
    #[serde(default)]
    pub tool_call_count: usize,
    #[serde(default)]
    pub search_query_count: usize,
    #[serde(default)]
    pub artifact_count: usize,
    #[serde(default)]
    pub dependency_input_count: usize,
    #[serde(default)]
    pub coverage_markers: Vec<String>,
}

/// Output produced by executing a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepOutput {
    /// Step ID this output belongs to
    pub step_id: String,
    /// The actual output content
    pub content: String,
    /// A concise user-facing summary of the output
    #[serde(default)]
    pub summary: String,
    /// Full output content before any display truncation
    #[serde(default)]
    pub full_content: String,
    /// Output format (text, markdown, json, etc.)
    pub format: OutputFormat,
    /// Validation results for completion criteria
    #[serde(default)]
    pub criteria_met: Vec<CriterionResult>,
    /// Any artifacts produced (file paths, URLs, etc.)
    #[serde(default)]
    pub artifacts: Vec<String>,
    /// Whether the output shown in `content` is truncated from `full_content`
    #[serde(default)]
    pub truncated: bool,
    /// Character length of the original full output
    #[serde(default)]
    pub original_length: usize,
    /// Character length of the shown output
    #[serde(default)]
    pub shown_length: usize,
    /// Output quality state from quality gate.
    #[serde(default)]
    pub quality_state: StepOutputQualityState,
    /// Reason when quality gate marks output incomplete.
    #[serde(default)]
    pub incomplete_reason: Option<String>,
    /// Number of attempts used to produce this output.
    #[serde(default = "default_step_attempt_count")]
    pub attempt_count: usize,
    /// Tool/evidence markers extracted from execution context.
    #[serde(default)]
    pub tool_evidence: Vec<String>,
    /// Iteration count used by orchestrator for this output.
    #[serde(default)]
    pub iterations: u32,
    /// Stop reason from orchestrator/runtime if available.
    #[serde(default)]
    pub stop_reason: Option<String>,
    /// Structured error code for diagnostics.
    #[serde(default)]
    pub error_code: Option<String>,
    /// Evidence bundle captured from runtime execution.
    #[serde(default)]
    pub evidence_bundle: StepEvidenceBundle,
    /// Condensed evidence summary for UI display.
    #[serde(default)]
    pub evidence_summary: StepEvidenceSummary,
    /// Structured validation result.
    #[serde(default)]
    pub validation_result: StepValidationResult,
    /// Final outcome status for this step output.
    #[serde(default)]
    pub outcome_status: StepOutcomeStatus,
    /// Optional review reason when manual review is recommended.
    #[serde(default)]
    pub review_reason: Option<StepReviewReason>,
}

fn default_step_attempt_count() -> usize {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepOutputQualityState {
    Complete,
    Incomplete,
}

impl Default for StepOutputQualityState {
    fn default() -> Self {
        Self::Complete
    }
}

/// Output format types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    Text,
    Markdown,
    Json,
    Html,
    Code,
}

// ============================================================================
// Step Execution State
// ============================================================================

/// Execution state of a single step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepExecutionState {
    /// Waiting to be executed
    Pending,
    /// Currently running
    Running,
    /// Completed successfully
    Completed { duration_ms: u64 },
    /// Completed but with non-blocking validation issues
    SoftFailed { reason: String, duration_ms: u64 },
    /// Requires human review
    NeedsReview { reason: String, duration_ms: u64 },
    /// Failed hard and blocks downstream execution
    HardFailed { reason: String },
    /// Cancelled
    Cancelled,
}

impl StepExecutionState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StepExecutionState::Completed { .. }
                | StepExecutionState::SoftFailed { .. }
                | StepExecutionState::NeedsReview { .. }
                | StepExecutionState::HardFailed { .. }
                | StepExecutionState::Cancelled
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanPhaseAgentKind {
    Llm,
    Cli,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanPhaseAgentRef {
    Llm { provider: String, model: String },
    Cli { agent_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPlanPhaseAgent {
    pub phase_id: String,
    #[serde(default)]
    pub agent_ref: Option<String>,
    pub agent_kind: PlanPhaseAgentKind,
    pub source: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub execution_backend_unavailable: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPlanPhaseAgents {
    #[serde(default)]
    pub strategy: Option<ResolvedPlanPhaseAgent>,
    #[serde(default)]
    pub clarification: Option<ResolvedPlanPhaseAgent>,
    #[serde(default)]
    pub generation: Option<ResolvedPlanPhaseAgent>,
    #[serde(default)]
    pub execution: Option<ResolvedPlanPhaseAgent>,
    #[serde(default)]
    pub retry: Option<ResolvedPlanPhaseAgent>,
}

// ============================================================================
// Session
// ============================================================================

/// Plan Mode session tracking all state across phases.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanModeSession {
    /// Unique session identifier
    pub session_id: String,
    /// Kernel root session ID for cross-mode handoff lookups
    #[serde(default)]
    pub kernel_session_id: Option<String>,
    /// Preferred locale for user-visible summaries
    #[serde(default)]
    pub locale: Option<String>,
    /// User's task description
    pub description: String,
    /// Current phase
    pub phase: PlanModePhase,
    /// Analysis result
    pub analysis: Option<PlanAnalysis>,
    /// Clarification Q&A history
    #[serde(default)]
    pub clarifications: Vec<ClarificationAnswer>,
    /// Current pending clarification question (if in Clarifying phase)
    #[serde(default)]
    pub current_question: Option<ClarificationQuestion>,
    /// Generated plan
    pub plan: Option<Plan>,
    /// Step outputs keyed by step ID
    #[serde(default)]
    pub step_outputs: HashMap<String, StepOutput>,
    /// Step execution states keyed by step ID
    #[serde(default)]
    pub step_states: HashMap<String, StepExecutionState>,
    /// Step attempt counters keyed by step ID
    #[serde(default)]
    pub step_attempts: HashMap<String, usize>,
    /// Execution progress summary
    pub progress: Option<PlanExecutionProgress>,
    /// Persisted execution launch metadata used for background resume.
    #[serde(default)]
    pub execution_resume_payload: Option<Value>,
    /// Resolved phase-agent snapshots for all plan lifecycle phases.
    #[serde(default)]
    pub resolved_phase_agents: ResolvedPlanPhaseAgents,
    /// Execution agent snapshot frozen when execution starts.
    #[serde(default)]
    pub execution_agent_snapshot: Option<ResolvedPlanPhaseAgent>,
    /// Retry agent snapshot frozen for the latest retry invocation.
    #[serde(default)]
    pub retry_agent_snapshot: Option<ResolvedPlanPhaseAgent>,
    /// Session creation timestamp
    pub created_at: String,
}

/// Progress update for plan execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecutionProgress {
    /// Current batch index (0-based)
    pub current_batch: usize,
    /// Total number of batches
    pub total_batches: usize,
    /// Number of steps completed
    pub steps_completed: usize,
    /// Number of steps failed
    pub steps_failed: usize,
    /// Total steps
    pub total_steps: usize,
    /// Overall progress percentage (0-100)
    pub progress_pct: f64,
}

// ============================================================================
// Event Types
// ============================================================================

/// Event channel name for plan mode progress events.
pub const PLAN_MODE_EVENT_CHANNEL: &str = "plan-mode-progress";

/// Progress event payload emitted to the frontend during plan execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanModeProgressEvent {
    /// Session ID
    pub session_id: String,
    /// Event type
    pub event_type: String,
    /// Current batch index (0-based)
    pub current_batch: usize,
    /// Total number of batches
    pub total_batches: usize,
    /// Step ID (if event relates to a specific step)
    pub step_id: Option<String>,
    /// Step status
    pub step_status: Option<String>,
    /// Error message (if any)
    pub error: Option<String>,
    /// Current attempt number when relevant.
    #[serde(default)]
    pub attempt_count: Option<usize>,
    /// Structured error code when relevant.
    #[serde(default)]
    pub error_code: Option<String>,
    /// Step output payload when a step completes successfully
    pub step_output: Option<StepOutput>,
    /// Terminal report payload when execution reaches a terminal state
    pub terminal_report: Option<PlanExecutionReport>,
    /// Overall progress percentage (0-100)
    pub progress_pct: f64,
    /// Stable run identifier for observability.
    #[serde(default)]
    pub run_id: String,
    /// Monotonic sequence within the run.
    #[serde(default)]
    pub event_seq: u64,
    /// Event producer source.
    #[serde(default)]
    pub source: String,
    /// Optional drop reason when event carries degraded data.
    #[serde(default)]
    pub drop_reason: Option<String>,
}

impl PlanModeProgressEvent {
    fn with_metadata(mut self, run_id: &str, event_seq: u64, source: &str) -> Self {
        self.run_id = run_id.to_string();
        self.event_seq = event_seq;
        self.source = source.to_string();
        self
    }

    pub fn batch_started(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "batch_started".to_string(),
            current_batch,
            total_batches,
            step_id: None,
            step_status: None,
            error: None,
            attempt_count: None,
            error_code: None,
            step_output: None,
            terminal_report: None,
            progress_pct,
            run_id: String::new(),
            event_seq: 0,
            source: String::new(),
            drop_reason: None,
        }
    }

    pub fn step_started(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        step_id: &str,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "step_started".to_string(),
            current_batch,
            total_batches,
            step_id: Some(step_id.to_string()),
            step_status: Some("running".to_string()),
            error: None,
            attempt_count: None,
            error_code: None,
            step_output: None,
            terminal_report: None,
            progress_pct,
            run_id: String::new(),
            event_seq: 0,
            source: String::new(),
            drop_reason: None,
        }
    }

    pub fn step_completed(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        step_id: &str,
        step_output: StepOutput,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "step_completed".to_string(),
            current_batch,
            total_batches,
            step_id: Some(step_id.to_string()),
            step_status: Some("completed".to_string()),
            error: None,
            attempt_count: None,
            error_code: None,
            step_output: Some(step_output),
            terminal_report: None,
            progress_pct,
            run_id: String::new(),
            event_seq: 0,
            source: String::new(),
            drop_reason: None,
        }
    }

    pub fn step_failed(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        step_id: &str,
        error: &str,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "step_failed".to_string(),
            current_batch,
            total_batches,
            step_id: Some(step_id.to_string()),
            step_status: Some("failed".to_string()),
            error: Some(error.to_string()),
            attempt_count: None,
            error_code: None,
            step_output: None,
            terminal_report: None,
            progress_pct,
            run_id: String::new(),
            event_seq: 0,
            source: String::new(),
            drop_reason: None,
        }
    }

    pub fn step_failed_with_output(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        step_id: &str,
        error: &str,
        step_output: StepOutput,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "step_failed".to_string(),
            current_batch,
            total_batches,
            step_id: Some(step_id.to_string()),
            step_status: Some("failed".to_string()),
            error: Some(error.to_string()),
            attempt_count: None,
            error_code: None,
            step_output: Some(step_output),
            terminal_report: None,
            progress_pct,
            run_id: String::new(),
            event_seq: 0,
            source: String::new(),
            drop_reason: None,
        }
    }

    pub fn step_retrying(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        step_id: &str,
        error: &str,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "step_retrying".to_string(),
            current_batch,
            total_batches,
            step_id: Some(step_id.to_string()),
            step_status: Some("retrying".to_string()),
            error: Some(error.to_string()),
            attempt_count: None,
            error_code: None,
            step_output: None,
            terminal_report: None,
            progress_pct,
            run_id: String::new(),
            event_seq: 0,
            source: String::new(),
            drop_reason: None,
        }
    }

    pub fn batch_blocked(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        error: &str,
        progress_pct: f64,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "batch_blocked".to_string(),
            current_batch,
            total_batches,
            step_id: None,
            step_status: Some("blocked".to_string()),
            error: Some(error.to_string()),
            attempt_count: None,
            error_code: None,
            step_output: None,
            terminal_report: None,
            progress_pct,
            run_id: String::new(),
            event_seq: 0,
            source: String::new(),
            drop_reason: None,
        }
    }

    pub fn execution_completed(
        session_id: &str,
        total_batches: usize,
        progress_pct: f64,
        terminal_report: PlanExecutionReport,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "execution_completed".to_string(),
            current_batch: total_batches.saturating_sub(1),
            total_batches,
            step_id: None,
            step_status: None,
            error: None,
            attempt_count: None,
            error_code: None,
            step_output: None,
            terminal_report: Some(terminal_report),
            progress_pct,
            run_id: String::new(),
            event_seq: 0,
            source: String::new(),
            drop_reason: None,
        }
    }

    pub fn execution_cancelled(
        session_id: &str,
        current_batch: usize,
        total_batches: usize,
        terminal_report: PlanExecutionReport,
    ) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "execution_cancelled".to_string(),
            current_batch,
            total_batches,
            step_id: None,
            step_status: None,
            error: None,
            attempt_count: None,
            error_code: None,
            step_output: None,
            terminal_report: Some(terminal_report),
            progress_pct: 0.0,
            run_id: String::new(),
            event_seq: 0,
            source: String::new(),
            drop_reason: None,
        }
    }

    pub fn with_observability(self, run_id: &str, event_seq: u64, source: &str) -> Self {
        self.with_metadata(run_id, event_seq, source)
    }

    pub fn with_attempt_metadata(
        mut self,
        attempt_count: Option<usize>,
        error_code: Option<impl Into<String>>,
    ) -> Self {
        self.attempt_count = attempt_count;
        self.error_code = error_code.map(Into::into);
        self
    }
}

// ============================================================================
// Execution Report
// ============================================================================

/// Final execution report for a completed plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExecutionReport {
    /// Session ID
    pub session_id: String,
    /// Plan title
    pub plan_title: String,
    /// Overall success
    pub success: bool,
    /// Terminal state of this run
    pub terminal_state: String,
    /// Structured terminal verdict
    #[serde(default)]
    pub terminal_status: PlanTerminalStatus,
    /// Total steps
    pub total_steps: usize,
    /// Steps completed
    pub steps_completed: usize,
    /// Steps failed
    pub steps_failed: usize,
    /// Steps completed with warnings
    #[serde(default)]
    pub steps_soft_failed: usize,
    /// Steps pending review
    #[serde(default)]
    pub steps_needs_review: usize,
    /// Steps cancelled
    #[serde(default)]
    pub steps_cancelled: usize,
    /// Steps that reached any terminal state (completed/failed/cancelled)
    #[serde(default)]
    pub steps_attempted: usize,
    /// Number of failed steps observed before cancellation happened.
    #[serde(default)]
    pub steps_failed_before_cancel: usize,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Per-step output summaries (step_id → truncated content)
    pub step_summaries: HashMap<String, String>,
    /// Per-step failure reason map (step_id → reason)
    pub failure_reasons: HashMap<String, String>,
    /// Who initiated cancellation if terminal_state is cancelled
    pub cancelled_by: Option<String>,
    /// Stable run identifier.
    #[serde(default)]
    pub run_id: String,
    /// Final synthesized conclusion markdown.
    #[serde(default)]
    pub final_conclusion_markdown: String,
    /// High-level highlights.
    #[serde(default)]
    pub highlights: Vec<String>,
    /// Suggested next actions.
    #[serde(default)]
    pub next_actions: Vec<String>,
    /// Retry metrics for this run.
    #[serde(default)]
    pub retry_stats: PlanRetryStats,
    /// Terminal verdict explanation trace.
    #[serde(default)]
    pub terminal_verdict_trace: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanTerminalStatus {
    Completed,
    CompletedWithWarnings,
    NeedsReview,
    Failed,
    Cancelled,
}

impl Default for PlanTerminalStatus {
    fn default() -> Self {
        Self::Completed
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRetryStats {
    pub total_retries: usize,
    pub steps_retried: usize,
    pub exhausted_failures: usize,
}

fn default_true() -> bool {
    true
}

// ============================================================================
// Batch Calculation (reused from task mode pattern)
// ============================================================================

/// Calculate execution batches from steps using topological sort (Kahn's algorithm).
pub fn calculate_plan_batches(steps: &[PlanStep]) -> Vec<PlanBatch> {
    calculate_plan_batches_with_parallel(steps, 4)
}

/// Calculate execution batches from steps using topological sort and max parallel cap per batch.
pub fn calculate_plan_batches_with_parallel(
    steps: &[PlanStep],
    max_parallel: usize,
) -> Vec<PlanBatch> {
    let chunk_size = max_parallel.clamp(1, 8);
    let step_ids: HashMap<&str, usize> = steps
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();

    // Build in-degree map
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

    for step in steps {
        in_degree.entry(step.id.as_str()).or_insert(0);
        for dep in &step.dependencies {
            if step_ids.contains_key(dep.as_str()) {
                *in_degree.entry(step.id.as_str()).or_insert(0) += 1;
                dependents
                    .entry(dep.as_str())
                    .or_default()
                    .push(step.id.as_str());
            }
        }
    }

    let mut batches = Vec::new();
    let mut remaining: HashMap<&str, usize> = in_degree.clone();

    while !remaining.is_empty() {
        // Collect all steps with zero in-degree
        let mut batch_ids: Vec<String> = remaining
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id.to_string())
            .collect();

        if batch_ids.is_empty() {
            // Circular dependency — add all remaining as chunks
            let mut leftovers: Vec<String> = remaining.keys().map(|&id| id.to_string()).collect();
            leftovers.sort();
            for id in &leftovers {
                remaining.remove(id.as_str());
            }
            for chunk in leftovers.chunks(chunk_size) {
                batches.push(PlanBatch {
                    index: batches.len(),
                    step_ids: chunk.to_vec(),
                });
            }
            continue;
        } else {
            // Remove batch from remaining and update in-degrees
            for id in &batch_ids {
                remaining.remove(id.as_str());
                if let Some(deps) = dependents.get(id.as_str()) {
                    for dep_id in deps {
                        if let Some(deg) = remaining.get_mut(dep_id) {
                            *deg = deg.saturating_sub(1);
                        }
                    }
                }
            }
        }

        batch_ids.sort();
        for chunk in batch_ids.chunks(chunk_size) {
            batches.push(PlanBatch {
                index: batches.len(),
                step_ids: chunk.to_vec(),
            });
        }
    }

    batches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_step(id: &str, title: &str, priority: StepPriority) -> PlanStep {
        PlanStep {
            id: id.to_string(),
            title: title.to_string(),
            description: String::new(),
            priority,
            dependencies: vec![],
            deliverable: StepDeliverableContract::default(),
            evidence_requirements: StepEvidenceRequirements::default(),
            quality_requirements: StepQualityRequirements::default(),
            validation_profile: StepValidationProfile::default(),
            failure_policy: StepFailurePolicy::default(),
            completion_criteria: vec![],
            expected_output: String::new(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_calculate_plan_batches_no_deps() {
        let steps = vec![
            sample_step("step-1", "Step 1", StepPriority::High),
            sample_step("step-2", "Step 2", StepPriority::Medium),
        ];

        let batches = calculate_plan_batches(&steps);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].step_ids.len(), 2);
    }

    #[test]
    fn test_calculate_plan_batches_with_deps() {
        let mut step_1 = sample_step("step-1", "Step 1", StepPriority::High);
        let mut step_2 = sample_step("step-2", "Step 2", StepPriority::Medium);
        step_2.dependencies = vec!["step-1".to_string()];
        let mut step_3 = sample_step("step-3", "Step 3", StepPriority::Low);
        step_3.dependencies = vec!["step-1".to_string()];

        let steps = vec![step_1, step_2, step_3];

        let batches = calculate_plan_batches(&steps);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].step_ids, vec!["step-1"]);
        assert!(batches[1].step_ids.contains(&"step-2".to_string()));
        assert!(batches[1].step_ids.contains(&"step-3".to_string()));
    }

    #[test]
    fn test_step_execution_state_terminal() {
        assert!(!StepExecutionState::Pending.is_terminal());
        assert!(!StepExecutionState::Running.is_terminal());
        assert!(StepExecutionState::Completed { duration_ms: 100 }.is_terminal());
        assert!(StepExecutionState::HardFailed {
            reason: "err".to_string()
        }
        .is_terminal());
        assert!(StepExecutionState::Cancelled.is_terminal());
    }

    #[test]
    fn test_plan_execution_config_normalizes_max_step_iterations() {
        let mut cfg = PlanExecutionConfig::default();
        cfg.max_step_iterations = 8;
        assert_eq!(cfg.normalized_max_step_iterations(), 12);
        cfg.max_step_iterations = 128;
        assert_eq!(cfg.normalized_max_step_iterations(), 96);
        cfg.max_step_iterations = 48;
        assert_eq!(cfg.normalized_max_step_iterations(), 48);
    }
}
