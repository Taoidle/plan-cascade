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
use std::sync::atomic::{AtomicBool, Ordering};
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
use super::embedding_manager::EmbeddingManager;
use super::embedding_service::EmbeddingService;
use super::hnsw_index::HnswIndex;
use super::index_store::IndexStore;
use crate::models::orchestrator::{
    ExecutionProgress, ExecutionSession, ExecutionSessionSummary, ExecutionStatus,
    StoryExecutionState,
};
use crate::services::agent_composer::registry::ComposerRegistry;
use crate::services::core::compaction::{
    CompactionConfig, CompactionResult, ContextCompactor, LlmSummaryCompactor,
    SlidingWindowCompactor,
};
use crate::services::knowledge::context_provider::{
    KnowledgeContextConfig, KnowledgeContextProvider,
};
use crate::services::llm::{
    AnthropicProvider, DeepSeekProvider, FallbackToolFormatMode, GlmProvider, LlmProvider,
    LlmRequestOptions, LlmResponse, Message, MessageContent, MinimaxProvider, OllamaProvider,
    OpenAIProvider, ProviderConfig, ProviderType, QwenProvider, ToolCallMode, ToolCallReliability,
    ToolDefinition, UsageStats,
};
use crate::services::quality_gates::run_quality_gates as execute_quality_gates;
use crate::services::streaming::UnifiedStreamEvent;
#[allow(deprecated)]
use crate::services::tools::{
    build_memory_section, build_plugin_instructions_section, build_plugin_skills_section,
    build_project_summary, build_skills_section, build_sub_agent_tool_guidance,
    build_system_prompt_with_memories, build_tool_call_instructions, detect_language,
    extract_text_without_tool_calls, format_tool_result, get_basic_tool_definitions_from_registry,
    get_tool_definitions_from_registry, merge_system_prompts, parse_tool_calls, ParsedToolCall,
    SubAgentType, TaskContext, TaskExecutionResult, TaskSpawner, ToolExecutor, MAX_SUB_AGENT_DEPTH,
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
    /// Optional project identifier for knowledge context retrieval.
    /// When set and a KnowledgeContextProvider is configured, relevant
    /// knowledge from the project's collections is injected into the system prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Configuration for pluggable context compaction (ADR-F006).
    /// Controls strategy selection, thresholds, and head/tail preservation counts.
    /// When omitted, defaults are used and the compactor is selected based on provider reliability.
    #[serde(default)]
    pub compaction_config: CompactionConfig,
    /// Optional task type for sub-agent tool guidance differentiation.
    /// Values: "explore", "analyze", "implement", or None for main agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// When set, this sub-agent can spawn further sub-agents at this depth level.
    /// When None, Task tool returns depth-limit error (leaf node behavior).
    /// Root agent uses `Some(0)`, coordinator sub-agent uses `Some(1)`, etc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_agent_depth: Option<u32>,
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
    if let Ok(base) = ensure_plan_cascade_dir() {
        return base.join("analysis-runs");
    }
    dirs::home_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join(".plan-cascade")
        .join("analysis-runs")
}

/// Compute a reasonable token budget for sub-agents based on the model's context window.
///
/// Sub-agents do multiple iterations, each re-sending the full conversation. With compaction
/// enabled, the effective budget can support more iterations. We use `context_window * 8`
/// as the base budget, allowing ~30-40 iterations with compaction.
/// Explore/analyze tasks get a higher multiplier (15x) since they read many files and
/// often exceed lower budgets with 128k-context models.
fn sub_agent_token_budget(context_window: u32, task_type: Option<&str>) -> u32 {
    let multiplier = match task_type {
        Some("explore") | Some("analyze") => 15,
        _ => 8,
    };
    (context_window * multiplier).clamp(20_000, 4_000_000)
}

/// Compute token budget for typed sub-agents with depth-based decay.
///
/// Each nesting level gets 60% of the parent's budget to prevent
/// unbounded token consumption in deep hierarchies.
fn subagent_token_budget_typed(
    context_window: u32,
    subagent_type: crate::services::tools::SubAgentType,
    depth: u32,
) -> u32 {
    use crate::services::tools::SubAgentType;
    let type_multiplier: u32 = match subagent_type {
        SubAgentType::Explore | SubAgentType::Plan => 15,
        SubAgentType::GeneralPurpose => 10,
        SubAgentType::Bash => 4,
    };
    // Each depth level decays to 60% of the parent
    let depth_factor = 0.6_f64.powi(depth as i32);
    let budget = (context_window as f64 * type_multiplier as f64 * depth_factor) as u32;
    budget.clamp(20_000, 4_000_000)
}

/// Max iterations for typed sub-agents.
fn subagent_max_iterations(subagent_type: crate::services::tools::SubAgentType) -> u32 {
    use crate::services::tools::SubAgentType;
    match subagent_type {
        SubAgentType::Explore => 100,
        SubAgentType::Plan => 50,
        SubAgentType::GeneralPurpose => 60,
        SubAgentType::Bash => 10,
    }
}

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
    /// Pluggable context compactor (ADR-F006).
    /// Selected at construction time based on provider reliability:
    /// - Reliable (Anthropic, OpenAI) -> LlmSummaryCompactor
    /// - Unreliable/None (Ollama, Qwen, DeepSeek, GLM) -> SlidingWindowCompactor
    compactor: Box<dyn ContextCompactor>,
    cancellation_token: CancellationToken,
    /// Pause flag: when true, the agentic loop sleeps until unpaused or cancelled.
    paused: Arc<AtomicBool>,
    /// Database pool for session persistence
    db_pool: Option<Pool<SqliteConnectionManager>>,
    /// Active sessions (in-memory cache)
    active_sessions: Arc<RwLock<HashMap<String, ExecutionSession>>>,
    /// Persistent analysis artifacts store (run manifests, evidence, reports)
    analysis_store: AnalysisRunStore,
    /// Optional index store for project summary injection into system prompt.
    /// Wrapped in Arc to enable sharing with sub-agents without cloning.
    index_store: Option<Arc<IndexStore>>,
    /// Detected language from the user's first message ("zh", "en", etc.)
    detected_language: Mutex<Option<String>>,
    /// Lifecycle hooks for cross-cutting concerns (memory, skills, etc.)
    hooks: crate::services::orchestrator::hooks::AgenticHooks,
    /// Shared selected skills from skill hooks for system prompt injection.
    /// Populated by `on_session_start` and `on_user_message` hooks.
    selected_skills: Option<Arc<RwLock<Vec<crate::services::skills::model::SkillMatch>>>>,
    /// Shared loaded memories from memory hooks for system prompt injection.
    /// Populated by `on_session_start` hook.
    loaded_memories: Option<Arc<RwLock<Vec<crate::services::memory::store::MemoryEntry>>>>,
    /// Optional knowledge context provider for injecting RAG context into system prompts.
    /// When present and enabled, queries project knowledge collections before each LLM call
    /// and appends relevant context to the system prompt.
    knowledge_context: Option<Arc<KnowledgeContextProvider>>,
    /// Configuration for knowledge context auto-retrieval.
    knowledge_context_config: KnowledgeContextConfig,
    /// Cached knowledge context block, populated at the start of execution.
    /// This avoids re-querying the knowledge base on every LLM call iteration.
    cached_knowledge_block: Mutex<Option<String>>,
    /// Optional composer registry for agent transfer support.
    /// When present, the agentic loop can transfer execution to named agents
    /// via the `TransferHandler` when `apply_actions` returns a `transfer_target`.
    composer_registry: Option<Arc<ComposerRegistry>>,
    /// Optional analytics tracking channel. Each LLM call sends a usage record
    /// through this channel for persistent storage in the analytics database.
    pub(crate) analytics_tx: Option<mpsc::Sender<crate::services::analytics::TrackerMessage>>,
    /// Optional cost calculator, used with analytics_tx to compute per-call costs.
    pub(crate) analytics_cost_calculator: Option<Arc<crate::services::analytics::CostCalculator>>,
    /// Optional permission gate for tool execution approval.
    /// Shared across parent and sub-agents via Arc.
    pub(crate) permission_gate: Option<Arc<super::permission_gate::PermissionGate>>,
    /// Plugin instructions (CLAUDE.md content from enabled plugins), cached at construction.
    plugin_instructions: Option<String>,
    /// Plugin skills (from enabled plugins' skills/), cached at construction.
    plugin_skills: Option<Vec<crate::services::plugins::models::PluginSkill>>,
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
    /// Optional EmbeddingManager for provider-aware semantic search in sub-agents (ADR-F002).
    shared_embedding_manager: Option<Arc<EmbeddingManager>>,
    /// Optional HNSW index for O(log n) approximate nearest neighbor search in sub-agents.
    shared_hnsw_index: Option<Arc<HnswIndex>>,
    /// Detected language from the user's message, propagated to sub-agents.
    detected_language: Option<String>,
    /// Whether the parent provider supports thinking (runtime check result).
    /// When true, sub-agents inherit the parent's enable_thinking setting.
    parent_supports_thinking: bool,

    // Owned snapshots from parent — sub-agents only read, never write back.
    /// Framework best-practice skills detected for the current project.
    skills_snapshot: Vec<crate::services::skills::model::SkillMatch>,
    /// Project memories from previous sessions.
    memories_snapshot: Vec<crate::services::memory::store::MemoryEntry>,
    /// Pre-built knowledge RAG context block (already truncated for sub-agents).
    knowledge_block_snapshot: Option<String>,
    /// Shared analytics tracking channel from the parent orchestrator.
    shared_analytics_tx: Option<mpsc::Sender<crate::services::analytics::TrackerMessage>>,
    /// Shared cost calculator from the parent orchestrator.
    shared_analytics_cost_calculator: Option<Arc<crate::services::analytics::CostCalculator>>,
    /// Shared permission gate from the parent orchestrator.
    shared_permission_gate: Option<Arc<super::permission_gate::PermissionGate>>,
    /// Shared pause flag from the parent orchestrator.
    /// Sub-agents inherit this so that pausing the parent also pauses sub-agents.
    shared_paused: Arc<AtomicBool>,
    /// Plugin instructions snapshot from parent for sub-agent prompt injection.
    plugin_instructions_snapshot: Option<String>,
    /// Plugin skills snapshot from parent for sub-agent prompt injection.
    plugin_skills_snapshot: Option<Vec<crate::services::plugins::models::PluginSkill>>,
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

/// Build a pluggable compactor based on provider reliability (ADR-F006).
///
/// - **Reliable** providers (Anthropic, OpenAI) get `LlmSummaryCompactor` whose
///   `SummarizeFn` closure captures the provider `Arc` and calls it for summarization.
/// - **Unreliable / None** providers (Ollama, Qwen, DeepSeek, GLM) get
///   `SlidingWindowCompactor` which is deterministic and makes no LLM calls.
fn build_compactor(provider: &Arc<dyn LlmProvider>) -> Box<dyn ContextCompactor> {
    match provider.tool_call_reliability() {
        ToolCallReliability::Reliable => {
            let provider_clone = Arc::clone(provider);
            Box::new(LlmSummaryCompactor::new(move |messages_to_summarize| {
                let provider = Arc::clone(&provider_clone);
                Box::pin(async move {
                    // Build a compaction prompt from the messages to summarize
                    let mut conversation_snippets: Vec<String> = Vec::new();
                    let mut tool_names: std::collections::HashSet<String> =
                        std::collections::HashSet::new();
                    let mut file_paths: Vec<String> = Vec::new();

                    for msg in &messages_to_summarize {
                        for content in &msg.content {
                            match content {
                                MessageContent::Text { text } => {
                                    let snippet = if text.len() > 500 {
                                        format!("{}...", &text[..500])
                                    } else {
                                        text.clone()
                                    };
                                    conversation_snippets.push(snippet);
                                }
                                MessageContent::ToolUse { name, .. } => {
                                    tool_names.insert(name.clone());
                                }
                                MessageContent::ToolResult { content, .. } => {
                                    for line in content.lines().take(5) {
                                        let trimmed = line.trim();
                                        if (trimmed.contains('/') || trimmed.contains('\\'))
                                            && trimmed.len() < 200
                                        {
                                            let path = trimmed
                                                .split_whitespace()
                                                .next()
                                                .unwrap_or(trimmed);
                                            if !file_paths.contains(&path.to_string()) {
                                                file_paths.push(path.to_string());
                                            }
                                        }
                                    }
                                    let snippet = if content.len() > 500 {
                                        format!("{}...", &content[..500])
                                    } else {
                                        content.clone()
                                    };
                                    conversation_snippets.push(snippet);
                                }
                                _ => {}
                            }
                        }
                    }

                    let snippets_summary = conversation_snippets
                        .iter()
                        .take(20)
                        .map(|s| format!("- {}", s))
                        .collect::<Vec<_>>()
                        .join("\n");

                    let tool_names_str = if tool_names.is_empty() {
                        "none".to_string()
                    } else {
                        let mut sorted: Vec<String> = tool_names.into_iter().collect();
                        sorted.sort();
                        sorted.join(", ")
                    };

                    let files_str = if file_paths.is_empty() {
                        "none".to_string()
                    } else {
                        file_paths
                            .iter()
                            .take(20)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    };

                    let compaction_prompt = format!(
                        "Summarize the following conversation history concisely in under 800 words. \
                         Focus on: what was asked, what tools were used, what was discovered, and what decisions were made.\n\n\
                         Tools used: {}\n\
                         Files touched: {}\n\n\
                         Conversation excerpts:\n{}\n\n\
                         Provide a clear, structured summary that preserves the key context needed to continue the task.",
                        tool_names_str, files_str, snippets_summary,
                    );

                    let summary_messages = vec![Message::user(compaction_prompt)];
                    let response = provider
                        .send_message(
                            summary_messages,
                            None,
                            Vec::new(),
                            LlmRequestOptions::default(),
                        )
                        .await
                        .map_err(|e| {
                            plan_cascade_core::error::CoreError::internal(format!(
                                "LLM summarization failed: {}",
                                e
                            ))
                        })?;

                    Ok(response.content.unwrap_or_else(|| {
                        "Previous conversation context was compacted.".to_string()
                    }))
                })
            }))
        }
        ToolCallReliability::Unreliable | ToolCallReliability::None => {
            Box::new(SlidingWindowCompactor::new())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AnalysisPhaseStatus {
    Passed,
    Partial,
    Failed,
}

#[path = "service_helpers/mod.rs"]
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
