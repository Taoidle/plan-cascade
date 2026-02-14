//! Orchestrator Service
//!
//! Coordinates LLM provider calls with tool execution in an agentic loop.
//! Supports session-based execution with persistence, cancellation, and progress tracking.

use async_trait::async_trait;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;

use super::analysis_index::{
    build_chunk_plan, build_file_inventory, compute_coverage_report, select_chunks_for_phase,
    AnalysisCoverageReport, AnalysisLimits, AnalysisProfile, ChunkPlan, FileInventory,
    FileInventoryItem, InventoryChunk,
};
use super::analysis_merge::{merge_chunk_summaries, ChunkSummaryRecord};
use super::analysis_scheduler::build_phase_plan;
use super::analysis_store::{
    AnalysisPhaseResultRecord, AnalysisRunHandle, AnalysisRunStore, CoverageMetrics,
    EvidenceRecord, SubAgentResultRecord,
};
use super::embedding_service::EmbeddingService;
use super::index_store::IndexStore;
use crate::models::orchestrator::{
    ExecutionProgress, ExecutionSession, ExecutionSessionSummary, ExecutionStatus,
    StoryExecutionState,
};
use crate::services::llm::{
    AnthropicProvider, DeepSeekProvider, FallbackToolFormatMode, GlmProvider, LlmProvider,
    LlmRequestOptions, LlmResponse, Message, MessageContent, MinimaxProvider, OllamaProvider,
    OpenAIProvider, ProviderConfig, ProviderType, QwenProvider, ToolCallMode, ToolCallReliability,
    ToolDefinition, UsageStats,
};
use crate::services::quality_gates::run_quality_gates as execute_quality_gates;
use crate::services::streaming::UnifiedStreamEvent;
use crate::services::tools::{
    build_system_prompt, build_tool_call_instructions, extract_text_without_tool_calls,
    format_tool_result, get_basic_tool_definitions, get_tool_definitions, merge_system_prompts,
    parse_tool_calls, ParsedToolCall, TaskContext, TaskExecutionResult, TaskSpawner, ToolExecutor,
};
use crate::utils::error::{AppError, AppResult};
use crate::utils::paths::ensure_plan_cascade_dir;

/// Configuration for the orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// LLM provider configuration
    pub provider: ProviderConfig,
    /// System prompt for the LLM
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Maximum iterations before stopping (prevents infinite loops)
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    /// Maximum total tokens to use
    #[serde(default = "default_max_tokens")]
    pub max_total_tokens: u32,
    /// Project root directory
    pub project_root: PathBuf,
    /// Program-data-root directory for analysis artifacts.
    #[serde(default = "default_analysis_artifacts_root")]
    pub analysis_artifacts_root: PathBuf,
    /// Whether to enable streaming
    #[serde(default = "default_streaming")]
    pub streaming: bool,
    /// Whether to enable automatic context compaction when input tokens exceed threshold
    #[serde(default = "default_enable_compaction")]
    pub enable_compaction: bool,
    /// Analysis profile for repository exploration depth.
    #[serde(default)]
    pub analysis_profile: AnalysisProfile,
    /// Detailed limits for chunking and coverage.
    #[serde(default)]
    pub analysis_limits: AnalysisLimits,
    /// Optional analysis session identifier.
    /// Cache reuse is limited to the same analysis session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis_session_id: Option<String>,
}

fn default_max_iterations() -> u32 {
    50
}

fn default_max_tokens() -> u32 {
    1_000_000
}

fn default_streaming() -> bool {
    true
}

fn default_enable_compaction() -> bool {
    true
}

fn default_analysis_artifacts_root() -> PathBuf {
    if let Some(local_data_dir) = dirs::data_local_dir() {
        return local_data_dir.join("plan-cascade").join("analysis-runs");
    }
    if let Ok(base) = ensure_plan_cascade_dir() {
        return base.join("analysis-runs");
    }
    std::env::temp_dir()
        .join("plan-cascade")
        .join("analysis-runs")
}

/// Compute a reasonable token budget for sub-agents based on the model's context window.
///
/// Sub-agents do multiple iterations, each re-sending the full conversation. With compaction
/// enabled, the effective budget can support more iterations. We use `context_window * 3`
/// as the base budget, allowing ~10-15 iterations with compaction.
/// Explore/analyze tasks get a higher multiplier since they read many files.
fn sub_agent_token_budget(context_window: u32, task_type: Option<&str>) -> u32 {
    let multiplier = match task_type {
        Some("explore") | Some("analyze") => 4,
        _ => 3,
    };
    (context_window * multiplier).clamp(20_000, 1_000_000)
}

/// Limit evidence verbosity to keep synthesis prompt focused and token-efficient.
const MAX_ANALYSIS_EVIDENCE_LINES: usize = 90;
/// Keep each phase summary short before feeding into synthesis.
const MAX_ANALYSIS_PHASE_SUMMARY_CHARS: usize = 1600;
/// Keep tool outputs bounded when they are fed back into the model during analysis.
const ANALYSIS_TOOL_RESULT_MAX_CHARS: usize = 1200;
const ANALYSIS_TOOL_RESULT_MAX_LINES: usize = 40;
const ANALYSIS_BASELINE_MAX_READ_FILES: usize = 24;
/// Keep phase context compact when feeding one phase into the next.
const MAX_SYNTHESIS_PHASE_CONTEXT_CHARS: usize = 900;
/// Limit chunk-level context in synthesis prompt (details stay in artifacts).
const MAX_SYNTHESIS_CHUNK_CONTEXT_CHARS: usize = 1400;
/// Keep evidence context concise to avoid synthesis overflow.
const MAX_SYNTHESIS_EVIDENCE_LINES: usize = 36;
/// Bound observed-path context passed to synthesis.
const MAX_SYNTHESIS_OBSERVED_PATHS: usize = 90;

// --- Regular (non-analysis) tool result truncation limits ---
// Applied when tool results are injected into the messages vec for the LLM
// during normal execution (outside analysis_phase mode). Frontend ToolResult
// events still receive the full untruncated content.

/// Maximum lines for Read tool output in regular execution context.
const REGULAR_READ_MAX_LINES: usize = 200;
/// Maximum characters for Read tool output in regular execution context.
const REGULAR_READ_MAX_CHARS: usize = 8000;
/// Maximum lines for Grep tool output in regular execution context.
const REGULAR_GREP_MAX_LINES: usize = 100;
/// Maximum characters for Grep tool output in regular execution context.
const REGULAR_GREP_MAX_CHARS: usize = 6000;
/// Maximum lines for LS/Glob tool output in regular execution context.
const REGULAR_LS_MAX_LINES: usize = 150;
/// Maximum characters for LS/Glob tool output in regular execution context.
const REGULAR_LS_MAX_CHARS: usize = 5000;
/// Maximum lines for Bash tool output in regular execution context.
const REGULAR_BASH_MAX_LINES: usize = 150;
/// Maximum characters for Bash tool output in regular execution context.
const REGULAR_BASH_MAX_CHARS: usize = 8000;

#[derive(Debug, Clone, Copy)]
struct EffectiveAnalysisTargets {
    coverage_ratio: f64,
    test_coverage_ratio: f64,
    sampled_read_ratio: f64,
    max_total_read_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnalyzeCacheEntry {
    key: String,
    #[serde(default)]
    query_signature: String,
    mode: String,
    project_root: String,
    response: String,
    created_at: i64,
    updated_at: i64,
    access_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AnalyzeCacheFile {
    version: u32,
    entries: Vec<AnalyzeCacheEntry>,
}

const ANALYZE_CACHE_MAX_ENTRIES: usize = 96;
const ANALYZE_CACHE_TTL_SECS: i64 = 60 * 60 * 6;

fn clamp_ratio(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn profile_adjusted_ratio(profile: AnalysisProfile, deep_ratio: f64) -> f64 {
    match profile {
        AnalysisProfile::DeepCoverage => deep_ratio,
        AnalysisProfile::Balanced => (deep_ratio - 0.15).max(0.30),
        AnalysisProfile::Fast => (deep_ratio - 0.35).max(0.15),
    }
}

fn desired_sampled_read_ratio(profile: AnalysisProfile, total_files: usize) -> f64 {
    let deep_ratio = match total_files {
        0..=300 => 0.95,
        301..=1_000 => 0.92,
        1_001..=2_500 => 0.86,
        2_501..=5_000 => 0.78,
        5_001..=10_000 => 0.68,
        _ => 0.55,
    };
    clamp_ratio(profile_adjusted_ratio(profile, deep_ratio))
}

fn desired_observed_coverage_ratio(profile: AnalysisProfile, total_files: usize) -> f64 {
    let deep_ratio = match total_files {
        0..=1_000 => 0.98,
        1_001..=5_000 => 0.95,
        5_001..=10_000 => 0.90,
        _ => 0.85,
    };
    clamp_ratio(profile_adjusted_ratio(profile, deep_ratio))
}

fn desired_test_coverage_ratio(profile: AnalysisProfile, total_test_files: usize) -> f64 {
    let deep_ratio = match total_test_files {
        0..=20 => 0.92,
        21..=80 => 0.86,
        81..=250 => 0.78,
        251..=1_000 => 0.66,
        _ => 0.52,
    };
    clamp_ratio(profile_adjusted_ratio(profile, deep_ratio))
}

fn compute_effective_analysis_targets(
    limits: &AnalysisLimits,
    profile: AnalysisProfile,
    inventory: &FileInventory,
) -> EffectiveAnalysisTargets {
    if inventory.total_files == 0 {
        return EffectiveAnalysisTargets {
            coverage_ratio: 1.0,
            test_coverage_ratio: 1.0,
            sampled_read_ratio: 1.0,
            max_total_read_files: 0,
        };
    }

    let desired_sampled = desired_sampled_read_ratio(profile, inventory.total_files);
    let desired_coverage = desired_observed_coverage_ratio(profile, inventory.total_files);
    let desired_test = desired_test_coverage_ratio(profile, inventory.total_test_files);

    let dynamic_read_budget = ((inventory.total_files as f64) * desired_sampled).ceil() as usize;
    let min_budget = limits
        .max_total_read_files
        .max(((inventory.total_files as f64) * 0.20).ceil() as usize)
        .max(1);
    let max_total_read_files = dynamic_read_budget
        .max(min_budget)
        .min(inventory.total_files.max(1));

    let achievable_sampled = (max_total_read_files as f64 / inventory.total_files as f64).min(1.0);
    let sampled_target = desired_sampled
        .min((achievable_sampled - 0.01).max(0.10))
        .max(0.0);

    let coverage_target = desired_coverage.max(limits.target_coverage_ratio.min(0.70));
    let test_target = desired_test
        .min((sampled_target + 0.12).min(1.0))
        .max(limits.target_test_coverage_ratio.min(0.35));

    EffectiveAnalysisTargets {
        coverage_ratio: clamp_ratio(coverage_target),
        test_coverage_ratio: clamp_ratio(test_target),
        sampled_read_ratio: clamp_ratio(sampled_target),
        max_total_read_files,
    }
}

/// Result of an orchestration execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Final response from the LLM
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    /// Total usage across all iterations
    pub usage: UsageStats,
    /// Number of iterations performed
    pub iterations: u32,
    /// Whether execution completed successfully
    pub success: bool,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Session-based execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionExecutionResult {
    /// Session ID
    pub session_id: String,
    /// Overall success
    pub success: bool,
    /// Number of completed stories
    pub completed_stories: usize,
    /// Number of failed stories
    pub failed_stories: usize,
    /// Total stories
    pub total_stories: usize,
    /// Total usage
    pub usage: UsageStats,
    /// Error message if session failed
    pub error: Option<String>,
    /// Quality gates summary (if run)
    pub quality_gates_passed: Option<bool>,
}

/// Orchestrator service for standalone LLM execution
pub struct OrchestratorService {
    config: OrchestratorConfig,
    provider: Arc<dyn LlmProvider>,
    tool_executor: ToolExecutor,
    cancellation_token: CancellationToken,
    /// Database pool for session persistence
    db_pool: Option<Pool<SqliteConnectionManager>>,
    /// Active sessions (in-memory cache)
    active_sessions: Arc<RwLock<HashMap<String, ExecutionSession>>>,
    /// Persistent analysis artifacts store (run manifests, evidence, reports)
    analysis_store: AnalysisRunStore,
    /// Optional index store for project summary injection into system prompt
    index_store: Option<IndexStore>,
}

/// Task spawner that creates sub-agent OrchestratorService instances
struct OrchestratorTaskSpawner {
    provider_config: ProviderConfig,
    project_root: PathBuf,
    context_window: u32,
    /// Shared file-read deduplication cache from the parent ToolExecutor.
    /// Sub-agents created by this spawner reuse the parent's cache so reads
    /// are not duplicated across the parent/child boundary.
    shared_read_cache:
        Arc<Mutex<HashMap<(PathBuf, usize, usize), crate::services::tools::ReadCacheEntry>>>,
    /// Optional index store for CodebaseSearch in sub-agents.
    shared_index_store: Option<Arc<IndexStore>>,
    /// Optional embedding service for semantic search in sub-agents.
    shared_embedding_service: Option<Arc<EmbeddingService>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisPhase {
    StructureDiscovery,
    ArchitectureTrace,
    ConsistencyCheck,
}

impl AnalysisPhase {
    fn id(self) -> &'static str {
        match self {
            AnalysisPhase::StructureDiscovery => "structure_discovery",
            AnalysisPhase::ArchitectureTrace => "architecture_trace",
            AnalysisPhase::ConsistencyCheck => "consistency_check",
        }
    }

    fn title(self) -> &'static str {
        match self {
            AnalysisPhase::StructureDiscovery => "Structure Discovery",
            AnalysisPhase::ArchitectureTrace => "Architecture Trace",
            AnalysisPhase::ConsistencyCheck => "Consistency Check",
        }
    }

    fn objective(self) -> &'static str {
        match self {
            AnalysisPhase::StructureDiscovery => {
                "Enumerate real project structure and verify manifests/entrypoints."
            }
            AnalysisPhase::ArchitectureTrace => {
                "Trace major modules, data flow, and integration boundaries using concrete files."
            }
            AnalysisPhase::ConsistencyCheck => {
                "Verify claims against file reads and grep results; explicitly mark unknowns."
            }
        }
    }

    fn task_type(self) -> &'static str {
        match self {
            AnalysisPhase::StructureDiscovery => "explore",
            AnalysisPhase::ArchitectureTrace => "analyze",
            AnalysisPhase::ConsistencyCheck => "analyze",
        }
    }

    fn max_iterations(self) -> u32 {
        match self {
            AnalysisPhase::StructureDiscovery => 6,
            AnalysisPhase::ArchitectureTrace => 5,
            AnalysisPhase::ConsistencyCheck => 4,
        }
    }

    fn layers(self) -> &'static [&'static str] {
        match self {
            AnalysisPhase::StructureDiscovery => &[
                "Layer 1 (Inventory): identify actual root directories and manifests.",
                "Layer 2 (Entrypoints): verify language/runtime entrypoints from discovered files only.",
                "Layer 3 (Test surface): verify test directories/framework entrypoints and representative test files.",
            ],
            AnalysisPhase::ArchitectureTrace => &[
                "Layer 1 (Module map): map major components and boundaries with concrete files.",
                "Layer 2 (Flow trace): verify integration/data-flow edges across components.",
                "Layer 3 (Quality trace): map testing/quality boundaries (unit/integration/frontend tests) with concrete files.",
            ],
            AnalysisPhase::ConsistencyCheck => &[
                "Layer 1 (Claim audit): re-open cited files and mark VERIFIED/UNVERIFIED/CONTRADICTED.",
                "Layer 2 (Test audit): verify testing and quality claims against concrete test files.",
            ],
        }
    }

    fn min_workers_before_early_exit(self) -> usize {
        match self {
            AnalysisPhase::StructureDiscovery => 2,
            AnalysisPhase::ArchitectureTrace => 2,
            AnalysisPhase::ConsistencyCheck => 2,
        }
    }
}

#[derive(Debug, Clone)]
struct AnalysisToolQuota {
    min_total_calls: usize,
    min_read_calls: usize,
    min_search_calls: usize,
    required_tools: Vec<&'static str>,
}

#[derive(Debug, Clone)]
struct AnalysisPhasePolicy {
    max_attempts: u32,
    force_tool_mode_attempts: u32,
    temperature_override: f32,
    quota: AnalysisToolQuota,
}

impl AnalysisPhasePolicy {
    fn for_phase(phase: AnalysisPhase) -> Self {
        match phase {
            AnalysisPhase::StructureDiscovery => Self {
                max_attempts: 2,
                force_tool_mode_attempts: 1,
                temperature_override: 0.0,
                quota: AnalysisToolQuota {
                    min_total_calls: 4,
                    min_read_calls: 1,
                    min_search_calls: 1,
                    required_tools: vec!["Cwd", "LS", "Read"],
                },
            },
            AnalysisPhase::ArchitectureTrace => Self {
                max_attempts: 1,
                force_tool_mode_attempts: 1,
                temperature_override: 0.0,
                quota: AnalysisToolQuota {
                    min_total_calls: 6,
                    min_read_calls: 3,
                    min_search_calls: 1,
                    required_tools: vec!["Read", "Grep"],
                },
            },
            AnalysisPhase::ConsistencyCheck => Self {
                max_attempts: 1,
                force_tool_mode_attempts: 1,
                temperature_override: 0.0,
                quota: AnalysisToolQuota {
                    min_total_calls: 6,
                    min_read_calls: 3,
                    min_search_calls: 1,
                    required_tools: vec!["Read", "Grep"],
                },
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisPhaseStatus {
    Passed,
    Partial,
    Failed,
}

#[path = "service_helpers.rs"]
mod service_helpers;
pub(crate) use service_helpers::text_describes_pending_action;

/// Information about the current provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub model: String,
    pub supports_thinking: bool,
    pub supports_tools: bool,
}
