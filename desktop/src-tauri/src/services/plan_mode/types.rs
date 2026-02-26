//! Plan Mode Core Types
//!
//! Data structures for the domain-agnostic task decomposition framework.
//! Plan Mode decomposes arbitrary tasks (writing, research, marketing, etc.)
//! into Steps with dependencies and executes them in parallel batches.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
    /// Criteria that determine when this step is complete
    pub completion_criteria: Vec<String>,
    /// Description of the expected output format/content
    pub expected_output: String,
    /// Additional domain-specific metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
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
}

/// Input types for clarification questions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClarificationInputType {
    Text,
    Textarea,
    SingleSelect(Vec<String>),
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

/// Output produced by executing a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepOutput {
    /// Step ID this output belongs to
    pub step_id: String,
    /// The actual output content
    pub content: String,
    /// Output format (text, markdown, json, etc.)
    pub format: OutputFormat,
    /// Validation results for completion criteria
    pub criteria_met: Vec<CriterionResult>,
    /// Any artifacts produced (file paths, URLs, etc.)
    #[serde(default)]
    pub artifacts: Vec<String>,
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
    /// Failed
    Failed { reason: String },
    /// Cancelled
    Cancelled,
}

impl StepExecutionState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            StepExecutionState::Completed { .. }
                | StepExecutionState::Failed { .. }
                | StepExecutionState::Cancelled
        )
    }
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
    /// Execution progress summary
    pub progress: Option<PlanExecutionProgress>,
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
    /// Overall progress percentage (0-100)
    pub progress_pct: f64,
}

impl PlanModeProgressEvent {
    pub fn batch_started(session_id: &str, current_batch: usize, total_batches: usize, progress_pct: f64) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "batch_started".to_string(),
            current_batch,
            total_batches,
            step_id: None,
            step_status: None,
            error: None,
            progress_pct,
        }
    }

    pub fn step_started(session_id: &str, current_batch: usize, total_batches: usize, step_id: &str, progress_pct: f64) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "step_started".to_string(),
            current_batch,
            total_batches,
            step_id: Some(step_id.to_string()),
            step_status: Some("running".to_string()),
            error: None,
            progress_pct,
        }
    }

    pub fn step_completed(session_id: &str, current_batch: usize, total_batches: usize, step_id: &str, progress_pct: f64) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "step_completed".to_string(),
            current_batch,
            total_batches,
            step_id: Some(step_id.to_string()),
            step_status: Some("completed".to_string()),
            error: None,
            progress_pct,
        }
    }

    pub fn step_failed(session_id: &str, current_batch: usize, total_batches: usize, step_id: &str, error: &str, progress_pct: f64) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "step_failed".to_string(),
            current_batch,
            total_batches,
            step_id: Some(step_id.to_string()),
            step_status: Some("failed".to_string()),
            error: Some(error.to_string()),
            progress_pct,
        }
    }

    pub fn execution_completed(session_id: &str, total_batches: usize, progress_pct: f64) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "execution_completed".to_string(),
            current_batch: total_batches.saturating_sub(1),
            total_batches,
            step_id: None,
            step_status: None,
            error: None,
            progress_pct,
        }
    }

    pub fn execution_cancelled(session_id: &str, current_batch: usize, total_batches: usize) -> Self {
        Self {
            session_id: session_id.to_string(),
            event_type: "execution_cancelled".to_string(),
            current_batch,
            total_batches,
            step_id: None,
            step_status: None,
            error: None,
            progress_pct: 0.0,
        }
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
    /// Total steps
    pub total_steps: usize,
    /// Steps completed
    pub steps_completed: usize,
    /// Steps failed
    pub steps_failed: usize,
    /// Total duration in milliseconds
    pub total_duration_ms: u64,
    /// Per-step output summaries (step_id → truncated content)
    pub step_summaries: HashMap<String, String>,
}

// ============================================================================
// Batch Calculation (reused from task mode pattern)
// ============================================================================

/// Calculate execution batches from steps using topological sort (Kahn's algorithm).
pub fn calculate_plan_batches(steps: &[PlanStep]) -> Vec<PlanBatch> {
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
            // Circular dependency — add all remaining as a single batch
            batch_ids = remaining.keys().map(|&id| id.to_string()).collect();
            for id in &batch_ids {
                remaining.remove(id.as_str());
            }
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
        batches.push(PlanBatch {
            index: batches.len(),
            step_ids: batch_ids,
        });
    }

    batches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_plan_batches_no_deps() {
        let steps = vec![
            PlanStep {
                id: "step-1".to_string(),
                title: "Step 1".to_string(),
                description: "".to_string(),
                priority: StepPriority::High,
                dependencies: vec![],
                completion_criteria: vec![],
                expected_output: "".to_string(),
                metadata: HashMap::new(),
            },
            PlanStep {
                id: "step-2".to_string(),
                title: "Step 2".to_string(),
                description: "".to_string(),
                priority: StepPriority::Medium,
                dependencies: vec![],
                completion_criteria: vec![],
                expected_output: "".to_string(),
                metadata: HashMap::new(),
            },
        ];

        let batches = calculate_plan_batches(&steps);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].step_ids.len(), 2);
    }

    #[test]
    fn test_calculate_plan_batches_with_deps() {
        let steps = vec![
            PlanStep {
                id: "step-1".to_string(),
                title: "Step 1".to_string(),
                description: "".to_string(),
                priority: StepPriority::High,
                dependencies: vec![],
                completion_criteria: vec![],
                expected_output: "".to_string(),
                metadata: HashMap::new(),
            },
            PlanStep {
                id: "step-2".to_string(),
                title: "Step 2".to_string(),
                description: "".to_string(),
                priority: StepPriority::Medium,
                dependencies: vec!["step-1".to_string()],
                completion_criteria: vec![],
                expected_output: "".to_string(),
                metadata: HashMap::new(),
            },
            PlanStep {
                id: "step-3".to_string(),
                title: "Step 3".to_string(),
                description: "".to_string(),
                priority: StepPriority::Low,
                dependencies: vec!["step-1".to_string()],
                completion_criteria: vec![],
                expected_output: "".to_string(),
                metadata: HashMap::new(),
            },
        ];

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
        assert!(StepExecutionState::Failed { reason: "err".to_string() }.is_terminal());
        assert!(StepExecutionState::Cancelled.is_terminal());
    }
}
