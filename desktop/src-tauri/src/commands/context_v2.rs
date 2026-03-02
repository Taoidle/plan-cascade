//! Context V2 Commands
//!
//! Production-oriented context assembly pipeline with structured envelope,
//! trace persistence, policy controls, and reusable context artifacts.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use tauri::State;

use crate::commands::knowledge::KnowledgeState;
use crate::models::response::CommandResponse;
use crate::services::context::assembly::{
    apply_budget_and_compaction as apply_budget_and_compaction_core,
    build_budget as build_budget_core, build_fallback_compaction as build_fallback_compaction_core,
    infer_injected_source_kinds as infer_injected_source_kinds_core, AssemblyBlock,
    AssemblyCompactionPolicy, AssemblyFallbackCompaction, AssemblySource,
};
use crate::services::context::events::TraceEventType;
use crate::services::knowledge::context_provider::{
    KnowledgeContextConfig, KnowledgeContextProvider,
};
use crate::services::memory::retrieval::{
    search_memories_v2_async, MemorySearchIntent, MemorySearchRequestV2,
};
use crate::services::memory::store::{
    build_session_project_path, MemoryCategory, MemoryEntry, GLOBAL_PROJECT_PATH,
};
use crate::services::skills::model::InjectionPhase;
use crate::services::task_mode::context_provider::{
    ensure_knowledge_initialized_public, query_selected_context, select_skills_for_task_filtered,
    ContextSourceConfig, KnowledgeSourceConfig, MemorySourceConfig, SkillsSourceConfig,
};
use crate::services::tools::system_prompt::build_memory_section;
use crate::state::AppState;

const DEFAULT_INPUT_TOKEN_BUDGET: usize = 24_000;
const DEFAULT_RESERVED_OUTPUT_TOKENS: usize = 3_000;
const DEFAULT_SOFT_COMPACTION_RATIO: f32 = 0.85;
const DEFAULT_HARD_COMPACTION_RATIO: f32 = 0.95;
const CONTEXT_POLICY_KEY: &str = "context_policy_v2";
const CONTEXT_ROLLOUT_KEY: &str = "context_rollout_v2";
const CONTEXT_RUNBOOK_PATH: &str = "docs/Context-V2-Incident-Runbook.md";
const DEFAULT_CONTEXT_WINDOW_HOURS: u32 = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSourceKind {
    History,
    Memory,
    Knowledge,
    Rules,
    Skills,
    Manual,
}

impl ContextSourceKind {
    fn as_str(&self) -> &'static str {
        match self {
            ContextSourceKind::History => "history",
            ContextSourceKind::Memory => "memory",
            ContextSourceKind::Knowledge => "knowledge",
            ContextSourceKind::Rules => "rules",
            ContextSourceKind::Skills => "skills",
            ContextSourceKind::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRequestMeta {
    pub turn_id: String,
    pub session_id: Option<String>,
    pub mode: String,
    pub query: String,
    pub intent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudget {
    pub input_token_budget: usize,
    pub reserved_output_tokens: usize,
    pub hard_limit: usize,
    pub used_input_tokens: usize,
    pub over_budget: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSourceRef {
    pub id: String,
    pub kind: ContextSourceKind,
    pub label: String,
    pub token_cost: usize,
    pub included: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBlock {
    pub source_id: String,
    pub title: String,
    pub content: String,
    pub token_cost: usize,
    pub priority: i32,
    pub reason: String,
    pub anchor: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionPolicy {
    #[serde(default = "default_soft_ratio")]
    pub soft_threshold_ratio: f32,
    #[serde(default = "default_hard_ratio")]
    pub hard_threshold_ratio: f32,
    #[serde(default = "default_true")]
    pub preserve_anchors: bool,
}

fn default_soft_ratio() -> f32 {
    DEFAULT_SOFT_COMPACTION_RATIO
}

fn default_hard_ratio() -> f32 {
    DEFAULT_HARD_COMPACTION_RATIO
}

fn default_true() -> bool {
    true
}

impl Default for CompactionPolicy {
    fn default() -> Self {
        Self {
            soft_threshold_ratio: DEFAULT_SOFT_COMPACTION_RATIO,
            hard_threshold_ratio: DEFAULT_HARD_COMPACTION_RATIO,
            preserve_anchors: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionAction {
    pub stage: String,
    pub action: String,
    pub source_id: String,
    pub before_tokens: usize,
    pub after_tokens: usize,
    pub reason: String,
}

fn default_quality_basis() -> serde_json::Value {
    json!({})
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionReport {
    pub triggered: bool,
    pub trigger_reason: String,
    pub strategy: String,
    pub before_tokens: usize,
    pub after_tokens: usize,
    pub compaction_tokens: u32,
    pub net_saving: i64,
    pub quality_score: f32,
    #[serde(default)]
    pub compaction_actions: Vec<CompactionAction>,
    #[serde(default = "default_quality_basis")]
    pub quality_basis: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextTraceEvent {
    pub trace_id: String,
    pub event_type: String,
    pub source_kind: Option<String>,
    pub source_id: Option<String>,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextTrace {
    pub trace_id: String,
    pub events: Vec<ContextTraceEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEnvelope {
    pub request_meta: ContextRequestMeta,
    pub budget: ContextBudget,
    pub sources: Vec<ContextSourceRef>,
    pub blocks: Vec<ContextBlock>,
    pub compaction: CompactionReport,
    pub trace_id: String,
    pub assembled_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAssemblyResponse {
    pub request_meta: ContextRequestMeta,
    pub assembled_prompt: String,
    pub trace_id: String,
    pub budget: ContextBudget,
    pub sources: Vec<ContextSourceRef>,
    pub blocks: Vec<ContextBlock>,
    pub compaction: CompactionReport,
    pub injected_source_kinds: Vec<String>,
    pub fallback_used: bool,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConversationTurn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualContextBlock {
    pub id: Option<String>,
    pub title: Option<String>,
    pub content: String,
    pub priority: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContextFaultInjection {
    #[serde(default)]
    pub memory_timeout: bool,
    #[serde(default)]
    pub knowledge_timeout: bool,
    #[serde(default)]
    pub ranker_unavailable: bool,
    #[serde(default)]
    pub compaction_quality_fail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareTurnContextV2Request {
    pub project_path: String,
    pub query: String,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub conversation_history: Vec<ContextConversationTurn>,
    #[serde(default)]
    pub context_sources: Option<ContextSourceConfig>,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub manual_blocks: Vec<ManualContextBlock>,
    #[serde(default)]
    pub input_token_budget: Option<usize>,
    #[serde(default)]
    pub reserved_output_tokens: Option<usize>,
    #[serde(default)]
    pub hard_limit: Option<usize>,
    #[serde(default)]
    pub compaction_policy: Option<CompactionPolicy>,
    #[serde(default)]
    pub fault_injection: Option<ContextFaultInjection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPolicy {
    #[serde(default = "default_true")]
    pub context_v2_pipeline: bool,
    #[serde(default = "default_true")]
    pub memory_v2_ranker: bool,
    #[serde(default)]
    pub context_inspector_ui: bool,
    #[serde(default)]
    pub pinned_sources: Vec<String>,
    #[serde(default)]
    pub excluded_sources: Vec<String>,
    #[serde(default = "default_soft_ratio")]
    pub soft_threshold_ratio: f32,
    #[serde(default = "default_hard_ratio")]
    pub hard_threshold_ratio: f32,
}

impl Default for ContextPolicy {
    fn default() -> Self {
        Self {
            context_v2_pipeline: true,
            memory_v2_ranker: true,
            context_inspector_ui: false,
            pinned_sources: Vec::new(),
            excluded_sources: Vec::new(),
            soft_threshold_ratio: DEFAULT_SOFT_COMPACTION_RATIO,
            hard_threshold_ratio: DEFAULT_HARD_COMPACTION_RATIO,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAck {
    pub key: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveContextArtifactInput {
    pub name: String,
    pub project_path: String,
    #[serde(default)]
    pub session_id: Option<String>,
    pub envelope: ContextEnvelope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextArtifactMeta {
    pub id: String,
    pub name: String,
    pub project_path: String,
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub artifact_id: String,
    pub session_id: Option<String>,
    pub applied: bool,
    pub envelope: ContextEnvelope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRolloutConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_rollout_percentage")]
    pub rollout_percentage: u8,
    #[serde(default = "default_ab_mode")]
    pub ab_mode: String,
    #[serde(default)]
    pub experiment_key: Option<String>,
    #[serde(default)]
    pub chaos_enabled: bool,
    #[serde(default)]
    pub chaos_probability: f32,
}

fn default_rollout_percentage() -> u8 {
    100
}

fn default_ab_mode() -> String {
    "off".to_string()
}

impl Default for ContextRolloutConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rollout_percentage: 100,
            ab_mode: "off".to_string(),
            experiment_key: None,
            chaos_enabled: false,
            chaos_probability: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRolloutAssignment {
    pub variant: String,
    pub bucket: u8,
    pub in_rollout: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextOpsDashboardRequest {
    pub project_path: String,
    #[serde(default)]
    pub window_hours: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextOpsAlert {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub value: f32,
    pub threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextOpsVariantStat {
    pub variant: String,
    pub traces: usize,
    pub degraded_rate: f32,
    pub avg_latency_ms: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChaosRunMeta {
    pub run_id: String,
    pub project_path: String,
    pub session_id: Option<String>,
    pub created_at: String,
    pub iterations: u32,
    pub fallback_success_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextOpsDashboard {
    pub project_path: String,
    pub window_start: String,
    pub window_end: String,
    pub window_hours: u32,
    pub total_traces: usize,
    pub assembled_traces: usize,
    pub availability: f32,
    pub degraded_traces: usize,
    pub degraded_rate: f32,
    pub source_failure_traces: usize,
    pub prepare_context_p50_ms: f32,
    pub prepare_context_p95_ms: f32,
    pub total_compaction_saving_tokens: i64,
    pub avg_compaction_saving_tokens: f32,
    pub ab_variants: Vec<ContextOpsVariantStat>,
    pub alerts: Vec<ContextOpsAlert>,
    pub policy: ContextPolicy,
    pub rollout: ContextRolloutConfig,
    pub recent_chaos_runs: Vec<ContextChaosRunMeta>,
    pub runbook_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChaosProbeRequest {
    pub project_path: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub iterations: Option<u32>,
    #[serde(default)]
    pub failure_probability: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChaosScenarioResult {
    pub scenario: String,
    pub injected: bool,
    pub fallback_ok: bool,
    pub warning_emitted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextChaosProbeReport {
    pub run_id: String,
    pub project_path: String,
    pub session_id: Option<String>,
    pub created_at: String,
    pub iterations: u32,
    pub failure_probability: f32,
    pub injected_faults: u32,
    pub fallback_success_rate: f32,
    pub scenarios: Vec<ContextChaosScenarioResult>,
    pub recommendation: String,
}

fn estimate_tokens_rough(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    (text.chars().count() + 3) / 4
}

fn parse_memory_intent(intent: Option<&str>) -> MemorySearchIntent {
    match intent
        .unwrap_or("default")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "bugfix" => MemorySearchIntent::Bugfix,
        "refactor" => MemorySearchIntent::Refactor,
        "qa" => MemorySearchIntent::Qa,
        "docs" => MemorySearchIntent::Docs,
        _ => MemorySearchIntent::Default,
    }
}

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn clamp_rollout_config(mut config: ContextRolloutConfig) -> ContextRolloutConfig {
    config.rollout_percentage = config.rollout_percentage.min(100);
    config.chaos_probability = config.chaos_probability.clamp(0.0, 1.0);
    let mode = config.ab_mode.trim().to_ascii_lowercase();
    config.ab_mode = match mode.as_str() {
        "off" | "shadow" | "split" => mode,
        _ => "off".to_string(),
    };
    config
}

fn stable_bucket(input: &str) -> u8 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    (hasher.finish() % 100) as u8
}

fn assign_rollout_variant(
    config: &ContextRolloutConfig,
    session_id: Option<&str>,
    turn_id: &str,
    trace_id: &str,
) -> ContextRolloutAssignment {
    let experiment = config
        .experiment_key
        .clone()
        .unwrap_or_else(|| "default".to_string());
    let seed = format!(
        "{}:{}:{}:{}",
        experiment,
        session_id.unwrap_or(""),
        turn_id,
        trace_id
    );
    let bucket = stable_bucket(&seed);
    let in_rollout = config.enabled && bucket < config.rollout_percentage;

    let variant = if !config.enabled {
        "disabled".to_string()
    } else if !in_rollout {
        "control".to_string()
    } else {
        match config.ab_mode.as_str() {
            "split" => {
                if bucket % 2 == 0 {
                    "v2".to_string()
                } else {
                    "v1_control".to_string()
                }
            }
            "shadow" => "shadow".to_string(),
            _ => "v2".to_string(),
        }
    };

    ContextRolloutAssignment {
        variant,
        bucket,
        in_rollout,
    }
}

fn source_selector_matches(kind: &ContextSourceKind, source_id: &str, selector: &str) -> bool {
    let normalized = selector.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    let kind_text = kind.as_str();
    let source_id_lower = source_id.to_ascii_lowercase();
    if normalized == kind_text || normalized == source_id_lower {
        return true;
    }

    if let Some(rest) = normalized.strip_prefix("kind:") {
        return rest == kind_text;
    }
    if let Some(rest) = normalized.strip_prefix("id:") {
        return rest == source_id_lower;
    }
    if let Some(rest) = normalized.strip_prefix("exclude:") {
        return rest == kind_text || rest == source_id_lower;
    }

    false
}

fn is_source_excluded(policy: &ContextPolicy, kind: &ContextSourceKind, source_id: &str) -> bool {
    policy
        .excluded_sources
        .iter()
        .any(|selector| source_selector_matches(kind, source_id, selector))
}

fn is_source_pinned(policy: &ContextPolicy, kind: &ContextSourceKind, source_id: &str) -> bool {
    policy
        .pinned_sources
        .iter()
        .any(|selector| source_selector_matches(kind, source_id, selector))
}

fn make_trace_event(
    trace_id: &str,
    event_type: TraceEventType,
    source_kind: Option<&str>,
    source_id: Option<&str>,
    message: impl Into<String>,
    metadata: Option<serde_json::Value>,
) -> ContextTraceEvent {
    ContextTraceEvent {
        trace_id: trace_id.to_string(),
        event_type: event_type.as_str().to_string(),
        source_kind: source_kind.map(|s| s.to_string()),
        source_id: source_id.map(|s| s.to_string()),
        message: message.into(),
        metadata,
        created_at: now_string(),
    }
}

async fn persist_trace_events(
    app_state: &AppState,
    session_id: Option<&str>,
    turn_id: Option<&str>,
    events: &[ContextTraceEvent],
) {
    let sid = session_id.map(|s| s.to_string());
    let tid = turn_id.map(|s| s.to_string());
    let payload = events.to_vec();

    let _ = app_state
        .with_database(move |db| {
            let conn = db.get_connection()?;
            for ev in payload {
                let metadata_json = ev
                    .metadata
                    .as_ref()
                    .map(|m| serde_json::to_string(m))
                    .transpose()?
                    .unwrap_or_else(|| "{}".to_string());

                conn.execute(
                    "INSERT INTO context_trace_events (trace_id, session_id, turn_id, event_type, source_kind, source_id, message, metadata, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    rusqlite::params![
                        ev.trace_id,
                        sid,
                        tid,
                        ev.event_type,
                        ev.source_kind,
                        ev.source_id,
                        ev.message,
                        metadata_json,
                        ev.created_at,
                    ],
                )?;
            }
            Ok(())
        })
        .await;
}

async fn load_context_policy(app_state: &AppState) -> ContextPolicy {
    match app_state
        .with_database(|db| db.get_setting(CONTEXT_POLICY_KEY))
        .await
    {
        Ok(Some(raw)) => serde_json::from_str::<ContextPolicy>(&raw).unwrap_or_default(),
        Ok(None) | Err(_) => ContextPolicy::default(),
    }
}

async fn load_context_rollout(app_state: &AppState) -> ContextRolloutConfig {
    match app_state
        .with_database(|db| db.get_setting(CONTEXT_ROLLOUT_KEY))
        .await
    {
        Ok(Some(raw)) => serde_json::from_str::<ContextRolloutConfig>(&raw)
            .map(clamp_rollout_config)
            .unwrap_or_default(),
        Ok(None) | Err(_) => ContextRolloutConfig::default(),
    }
}

fn format_history_block(history: &[ContextConversationTurn]) -> Option<String> {
    if history.is_empty() {
        return None;
    }

    let rendered = history
        .iter()
        .enumerate()
        .map(|(idx, turn)| {
            let role = turn.role.trim().to_ascii_uppercase();
            format!("{}. [{}]\n{}", idx + 1, role, turn.content)
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    Some(format!("Conversation History\n{}", rendered))
}

fn push_block(
    policy: &ContextPolicy,
    sources: &mut Vec<ContextSourceRef>,
    blocks: &mut Vec<ContextBlock>,
    id: impl Into<String>,
    kind: ContextSourceKind,
    label: impl Into<String>,
    content: impl Into<String>,
    priority: i32,
    reason: impl Into<String>,
    anchor: bool,
) -> bool {
    let source_id = id.into();
    if is_source_excluded(policy, &kind, &source_id) {
        return false;
    }

    let reason_text = reason.into();
    let pinned = is_source_pinned(policy, &kind, &source_id);
    let label_text = label.into();
    let content_text = content.into();
    let token_cost = estimate_tokens_rough(&content_text);
    let source_reason = if pinned {
        "pinned_by_policy".to_string()
    } else {
        reason_text.clone()
    };
    let block_reason = if pinned {
        "pinned_by_policy".to_string()
    } else {
        "selected".to_string()
    };

    sources.push(ContextSourceRef {
        id: source_id.clone(),
        kind,
        label: label_text.clone(),
        token_cost,
        included: true,
        reason: source_reason,
    });

    blocks.push(ContextBlock {
        source_id,
        title: label_text,
        content: content_text,
        token_cost,
        priority: if pinned { priority.max(120) } else { priority },
        reason: block_reason,
        anchor: anchor || pinned,
    });

    true
}

fn build_prompt(query: &str, blocks: &[ContextBlock]) -> String {
    let mut parts = Vec::new();
    parts.push("[Context Envelope v2]".to_string());
    parts.push(format!("User Query:\n{}", query));

    for block in blocks {
        parts.push(format!("\n### {}\n{}", block.title, block.content));
    }

    parts.join("\n\n")
}

fn map_fallback_compaction(core: AssemblyFallbackCompaction) -> CompactionReport {
    CompactionReport {
        triggered: false,
        trigger_reason: core.trigger_reason,
        strategy: core.strategy,
        before_tokens: core.before_tokens,
        after_tokens: core.after_tokens,
        compaction_tokens: core.compaction_tokens,
        net_saving: core.net_saving,
        quality_score: core.quality_score,
        compaction_actions: Vec::new(),
        quality_basis: core.quality_basis,
    }
}

fn map_envelope_to_assembly_response(
    envelope: ContextEnvelope,
    injected_source_kinds: Vec<String>,
) -> ContextAssemblyResponse {
    ContextAssemblyResponse {
        request_meta: envelope.request_meta,
        assembled_prompt: envelope.assembled_prompt,
        trace_id: envelope.trace_id,
        budget: envelope.budget,
        sources: envelope.sources,
        blocks: envelope.blocks,
        compaction: envelope.compaction,
        injected_source_kinds,
        fallback_used: false,
        fallback_reason: None,
    }
}

fn build_legacy_fallback_response(
    request: &PrepareTurnContextV2Request,
    fallback_prompt: String,
    trigger_reason: &str,
    fallback_reason: Option<String>,
    injected_source_kinds: Vec<String>,
) -> ContextAssemblyResponse {
    let fallback_tokens = estimate_tokens_rough(&fallback_prompt);
    let fallback_budget = build_budget_core(
        request.input_token_budget,
        request.reserved_output_tokens,
        request.hard_limit,
        DEFAULT_INPUT_TOKEN_BUDGET,
        DEFAULT_RESERVED_OUTPUT_TOKENS,
        fallback_tokens,
    );

    ContextAssemblyResponse {
        request_meta: ContextRequestMeta {
            turn_id: request
                .turn_id
                .clone()
                .unwrap_or_else(|| format!("fallback-{}", uuid::Uuid::new_v4())),
            session_id: request.session_id.clone(),
            mode: request
                .mode
                .clone()
                .unwrap_or_else(|| "standalone".to_string()),
            query: request.query.clone(),
            intent: request.intent.clone(),
        },
        assembled_prompt: fallback_prompt,
        trace_id: format!("fallback-{}", uuid::Uuid::new_v4()),
        budget: ContextBudget {
            input_token_budget: fallback_budget.input_token_budget,
            reserved_output_tokens: fallback_budget.reserved_output_tokens,
            hard_limit: fallback_budget.hard_limit,
            used_input_tokens: fallback_budget.used_input_tokens,
            over_budget: fallback_budget.over_budget,
        },
        sources: Vec::new(),
        blocks: Vec::new(),
        compaction: map_fallback_compaction(build_fallback_compaction_core(
            trigger_reason,
            "legacy_with_selected_sources",
            fallback_tokens,
        )),
        injected_source_kinds,
        fallback_used: true,
        fallback_reason,
    }
}

fn infer_injected_source_kinds(request: &PrepareTurnContextV2Request) -> Vec<String> {
    let config = request.context_sources.as_ref();
    infer_injected_source_kinds_core(
        !request.conversation_history.is_empty(),
        config
            .and_then(|c| c.memory.as_ref())
            .map(|m| m.enabled)
            .unwrap_or(false),
        config
            .and_then(|c| c.knowledge.as_ref())
            .map(|k| k.enabled)
            .unwrap_or(false),
        config
            .and_then(|c| c.skills.as_ref())
            .map(|s| s.enabled)
            .unwrap_or(false),
    )
}

async fn build_legacy_fallback_prompt(
    request: &PrepareTurnContextV2Request,
    app_state: &AppState,
    knowledge_state: &KnowledgeState,
) -> String {
    let mut sections: Vec<String> = Vec::new();
    if let Some(history_block) = format_history_block(&request.conversation_history) {
        sections.push(history_block);
    }

    if let Some(config) = request.context_sources.as_ref() {
        let enriched = query_selected_context(
            config,
            knowledge_state,
            app_state,
            &request.project_path,
            &request.query,
            InjectionPhase::Always,
        )
        .await;
        if !enriched.knowledge_block.is_empty() {
            sections.push(enriched.knowledge_block);
        }
        if !enriched.memory_block.is_empty() {
            sections.push(enriched.memory_block);
        }
        if !enriched.skills_block.is_empty() {
            sections.push(enriched.skills_block);
        }
    }

    if sections.is_empty() {
        return request.query.clone();
    }

    let mut parts = Vec::new();
    parts.push(
        "Continue the same conversation. Keep consistency with previous context.".to_string(),
    );
    parts.push("Selected context sources were preserved via compatibility fallback.".to_string());
    parts.push(format!("User Query:\n{}", request.query));
    for section in sections {
        parts.push(section);
    }
    parts.join("\n\n")
}

fn apply_budget_and_compaction(
    blocks: Vec<ContextBlock>,
    sources: &mut [ContextSourceRef],
    input_budget: usize,
    policy: &CompactionPolicy,
) -> (Vec<ContextBlock>, CompactionReport) {
    let assembly_policy = AssemblyCompactionPolicy {
        soft_threshold_ratio: policy.soft_threshold_ratio,
        hard_threshold_ratio: policy.hard_threshold_ratio,
        preserve_anchors: policy.preserve_anchors,
    };

    let assembly_blocks = blocks
        .into_iter()
        .map(|block| AssemblyBlock {
            source_id: block.source_id,
            title: block.title,
            content: block.content,
            token_cost: block.token_cost,
            priority: block.priority,
            reason: block.reason,
            anchor: block.anchor,
        })
        .collect::<Vec<_>>();

    let assembly_sources = sources
        .iter()
        .map(|source| AssemblySource {
            id: source.id.clone(),
            token_cost: source.token_cost,
            included: source.included,
            reason: source.reason.clone(),
        })
        .collect::<Vec<_>>();

    let assembly_result = apply_budget_and_compaction_core(
        assembly_blocks,
        assembly_sources,
        input_budget,
        &assembly_policy,
    );

    let source_map = assembly_result
        .sources
        .iter()
        .map(|source| {
            (
                source.id.clone(),
                (source.token_cost, source.included, source.reason.clone()),
            )
        })
        .collect::<HashMap<_, _>>();

    for source in sources.iter_mut() {
        if let Some(updated) = source_map.get(&source.id) {
            source.token_cost = updated.0;
            source.included = updated.1;
            source.reason = updated.2.clone();
        }
    }

    let retained = assembly_result
        .blocks
        .into_iter()
        .map(|block| ContextBlock {
            source_id: block.source_id,
            title: block.title,
            content: block.content,
            token_cost: block.token_cost,
            priority: block.priority,
            reason: block.reason,
            anchor: block.anchor,
        })
        .collect::<Vec<_>>();

    let report = CompactionReport {
        triggered: assembly_result.triggered,
        trigger_reason: assembly_result.trigger_reason,
        strategy: assembly_result.strategy,
        before_tokens: assembly_result.before_tokens,
        after_tokens: assembly_result.after_tokens,
        compaction_tokens: assembly_result.compaction_tokens,
        net_saving: assembly_result.net_saving,
        quality_score: assembly_result.quality_score,
        compaction_actions: assembly_result
            .compaction_actions
            .into_iter()
            .map(|action| CompactionAction {
                stage: action.stage,
                action: action.action,
                source_id: action.source_id,
                before_tokens: action.before_tokens,
                after_tokens: action.after_tokens,
                reason: action.reason,
            })
            .collect(),
        quality_basis: assembly_result.quality_basis,
    };

    (retained, report)
}

async fn prepare_turn_context_v2_internal(
    request: PrepareTurnContextV2Request,
    app_state: State<'_, AppState>,
    knowledge_state: State<'_, KnowledgeState>,
) -> Result<CommandResponse<ContextEnvelope>, String> {
    let project_path = request.project_path.trim().to_string();
    if project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required"));
    }
    if request.query.trim().is_empty() {
        return Ok(CommandResponse::err("query is required"));
    }

    let trace_id = uuid::Uuid::new_v4().to_string();
    let turn_id = request
        .turn_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mode = request
        .mode
        .clone()
        .unwrap_or_else(|| "standalone".to_string());

    let input_budget = request
        .input_token_budget
        .unwrap_or(DEFAULT_INPUT_TOKEN_BUDGET)
        .max(256);
    let reserved_output_tokens = request
        .reserved_output_tokens
        .unwrap_or(DEFAULT_RESERVED_OUTPUT_TOKENS)
        .max(128);
    let hard_limit = request
        .hard_limit
        .unwrap_or(input_budget + reserved_output_tokens)
        .max(input_budget + reserved_output_tokens);
    let context_policy = load_context_policy(&app_state).await;
    let rollout_config = load_context_rollout(&app_state).await;
    let rollout_assignment = assign_rollout_variant(
        &rollout_config,
        request.session_id.as_deref(),
        &turn_id,
        &trace_id,
    );
    let compaction_policy = request.compaction_policy.unwrap_or(CompactionPolicy {
        soft_threshold_ratio: context_policy.soft_threshold_ratio,
        hard_threshold_ratio: context_policy.hard_threshold_ratio,
        preserve_anchors: true,
    });
    let fault_injection = request.fault_injection.clone().unwrap_or_default();

    let mut trace_events = Vec::new();
    trace_events.push(make_trace_event(
        &trace_id,
        TraceEventType::CollectStart,
        None,
        None,
        "context collection started",
        Some(json!({
            "mode": mode,
            "project_path": project_path,
            "query_len": request.query.len(),
            "policy_context_v2_pipeline": context_policy.context_v2_pipeline,
            "policy_excluded_count": context_policy.excluded_sources.len(),
            "policy_pinned_count": context_policy.pinned_sources.len(),
            "fault_injection": {
                "memory_timeout": fault_injection.memory_timeout,
                "knowledge_timeout": fault_injection.knowledge_timeout,
                "ranker_unavailable": fault_injection.ranker_unavailable,
                "compaction_quality_fail": fault_injection.compaction_quality_fail,
            },
        })),
    ));
    trace_events.push(make_trace_event(
        &trace_id,
        TraceEventType::RolloutAssignment,
        None,
        None,
        "rollout/ab assignment evaluated",
        Some(json!({
            "variant": rollout_assignment.variant,
            "bucket": rollout_assignment.bucket,
            "in_rollout": rollout_assignment.in_rollout,
            "rollout_enabled": rollout_config.enabled,
            "rollout_percentage": rollout_config.rollout_percentage,
            "ab_mode": rollout_config.ab_mode,
        })),
    ));
    if !context_policy.context_v2_pipeline {
        trace_events.push(make_trace_event(
            &trace_id,
            TraceEventType::PolicyNotice,
            None,
            None,
            "context_v2_pipeline feature flag disabled; explicit call continues",
            None,
        ));
    }
    if rollout_config.chaos_enabled && rollout_config.chaos_probability > 0.0 {
        trace_events.push(make_trace_event(
            &trace_id,
            TraceEventType::ChaosConfig,
            None,
            None,
            "chaos configuration active for observability",
            Some(json!({
                "chaos_probability": rollout_config.chaos_probability,
            })),
        ));
    }

    let mut sources = Vec::new();
    let mut blocks = Vec::new();

    // 1) Conversation history
    if let Some(history_block) = format_history_block(&request.conversation_history) {
        if push_block(
            &context_policy,
            &mut sources,
            &mut blocks,
            "history:conversation",
            ContextSourceKind::History,
            "Conversation History",
            history_block,
            80,
            "conversation_history",
            false,
        ) {
            trace_events.push(make_trace_event(
                &trace_id,
                TraceEventType::SourceCollected,
                Some("history"),
                Some("history:conversation"),
                "conversation history included",
                Some(json!({ "turns": request.conversation_history.len() })),
            ));
        } else {
            trace_events.push(make_trace_event(
                &trace_id,
                TraceEventType::SourceSkipped,
                Some("history"),
                Some("history:conversation"),
                "history source excluded by policy",
                None,
            ));
        }
    }

    // 2) Manual blocks
    for (idx, manual) in request.manual_blocks.iter().enumerate() {
        if manual.content.trim().is_empty() {
            continue;
        }
        let source_id = manual
            .id
            .clone()
            .unwrap_or_else(|| format!("manual:{}", idx + 1));
        if push_block(
            &context_policy,
            &mut sources,
            &mut blocks,
            source_id.clone(),
            ContextSourceKind::Manual,
            manual
                .title
                .clone()
                .unwrap_or_else(|| format!("Manual Context {}", idx + 1)),
            manual.content.clone(),
            manual.priority.unwrap_or(100),
            "manual_included",
            true,
        ) {
            trace_events.push(make_trace_event(
                &trace_id,
                TraceEventType::SourceCollected,
                Some("manual"),
                Some(&source_id),
                "manual context included",
                None,
            ));
        } else {
            trace_events.push(make_trace_event(
                &trace_id,
                TraceEventType::SourceSkipped,
                Some("manual"),
                Some(&source_id),
                "manual context excluded by policy",
                None,
            ));
        }
    }

    // 3) Rules
    if !request.rules.is_empty() {
        let rules_text = request
            .rules
            .iter()
            .filter(|r| !r.trim().is_empty())
            .map(|r| format!("- {}", r.trim()))
            .collect::<Vec<_>>()
            .join("\n");
        if !rules_text.is_empty() {
            if push_block(
                &context_policy,
                &mut sources,
                &mut blocks,
                "rules:session",
                ContextSourceKind::Rules,
                "Session Rules",
                rules_text,
                95,
                "rules_included",
                true,
            ) {
                trace_events.push(make_trace_event(
                    &trace_id,
                    TraceEventType::SourceCollected,
                    Some("rules"),
                    Some("rules:session"),
                    "rules included",
                    Some(json!({ "count": request.rules.len() })),
                ));
            } else {
                trace_events.push(make_trace_event(
                    &trace_id,
                    TraceEventType::SourceSkipped,
                    Some("rules"),
                    Some("rules:session"),
                    "rules source excluded by policy",
                    None,
                ));
            }
        }
    }

    // 4) Config-driven sources (knowledge/memory/skills)
    if let Some(config) = request.context_sources.as_ref() {
        // 4a) Memory
        if config.memory.as_ref().map(|m| m.enabled).unwrap_or(false) {
            if is_source_excluded(
                &context_policy,
                &ContextSourceKind::Memory,
                "memory:retrieved",
            ) {
                trace_events.push(make_trace_event(
                    &trace_id,
                    TraceEventType::SourceSkipped,
                    Some("memory"),
                    Some("memory:retrieved"),
                    "memory source excluded by policy",
                    None,
                ));
            } else {
                let mcfg = config.memory.as_ref().unwrap();
                let mut memory_entries: Vec<MemoryEntry> = Vec::new();
                let mut seen_ids = HashSet::new();
                let excluded_ids: HashSet<&str> = mcfg
                    .excluded_memory_ids
                    .iter()
                    .map(|id| id.as_str())
                    .collect();

                if fault_injection.memory_timeout {
                    trace_events.push(make_trace_event(
                        &trace_id,
                        TraceEventType::SourceFailed,
                        Some("memory"),
                        Some("memory:search"),
                        "memory timeout injected by chaos probe",
                        Some(json!({ "fault": "memory_timeout" })),
                    ));
                } else {
                    if fault_injection.ranker_unavailable {
                        trace_events.push(make_trace_event(
                            &trace_id,
                            TraceEventType::SourceFailed,
                            Some("memory"),
                            Some("memory:ranker"),
                            "memory ranker unavailable; lexical fallback active",
                            Some(json!({ "fault": "ranker_unavailable" })),
                        ));
                    }

                    let memory_store = match app_state.get_memory_store_arc().await {
                        Ok(store) => Some(store),
                        Err(e) => {
                            trace_events.push(make_trace_event(
                                &trace_id,
                                TraceEventType::SourceFailed,
                                Some("memory"),
                                Some("memory:store"),
                                "memory store unavailable",
                                Some(json!({ "error": e.to_string() })),
                            ));
                            None
                        }
                    };

                    if let Some(memory_store) = memory_store {
                        if !mcfg.selected_memory_ids.is_empty() {
                            for id in &mcfg.selected_memory_ids {
                                if let Ok(Some(entry)) = memory_store.get_memory(id) {
                                    if excluded_ids.contains(entry.id.as_str()) {
                                        continue;
                                    }
                                    if seen_ids.insert(entry.id.clone()) {
                                        memory_entries.push(entry);
                                    }
                                }
                            }
                        }

                        let categories = if mcfg.selected_categories.is_empty() {
                            None
                        } else {
                            let parsed: Vec<MemoryCategory> = mcfg
                                .selected_categories
                                .iter()
                                .filter_map(|s| MemoryCategory::from_str(s).ok())
                                .collect();
                            if parsed.is_empty() {
                                None
                            } else {
                                Some(parsed)
                            }
                        };

                        let mut scopes = HashSet::new();
                        for scope in &mcfg.selected_scopes {
                            scopes.insert(scope.trim().to_ascii_lowercase());
                        }
                        if scopes.is_empty() {
                            scopes.insert("project".to_string());
                            scopes.insert("global".to_string());
                            if mcfg.session_id.is_some() {
                                scopes.insert("session".to_string());
                            }
                        }

                        let mut search_specs: Vec<String> = Vec::new();
                        if scopes.contains("project") {
                            search_specs.push(project_path.clone());
                        }
                        if scopes.contains("global") {
                            search_specs.push(GLOBAL_PROJECT_PATH.to_string());
                        }
                        if scopes.contains("session") {
                            if let Some(sid) = mcfg.session_id.as_deref() {
                                if let Some(scope_path) = build_session_project_path(sid) {
                                    search_specs.push(scope_path);
                                }
                            }
                        }

                        for scope_path in search_specs {
                            let req = MemorySearchRequestV2 {
                                project_path: scope_path,
                                query: request.query.clone(),
                                categories: categories.clone(),
                                top_k: 10,
                                min_importance: 0.3,
                                intent: parse_memory_intent(request.intent.as_deref()),
                                enable_semantic: context_policy.memory_v2_ranker
                                    && !fault_injection.ranker_unavailable,
                                enable_lexical: true,
                            };

                            match search_memories_v2_async(memory_store.as_ref(), &req).await {
                                Ok(results) => {
                                    for row in results {
                                        let entry = row.entry;
                                        if excluded_ids.contains(entry.id.as_str()) {
                                            continue;
                                        }
                                        if seen_ids.insert(entry.id.clone()) {
                                            memory_entries.push(entry);
                                        }
                                    }
                                }
                                Err(e) => {
                                    trace_events.push(make_trace_event(
                                        &trace_id,
                                        TraceEventType::SourceFailed,
                                        Some("memory"),
                                        Some("memory:search"),
                                        "memory search failed",
                                        Some(json!({ "error": e.to_string() })),
                                    ));
                                }
                            }
                        }
                    }
                }

                if !memory_entries.is_empty() {
                    let memory_block = build_memory_section(Some(&memory_entries));
                    if !memory_block.is_empty() {
                        if push_block(
                            &context_policy,
                            &mut sources,
                            &mut blocks,
                            "memory:retrieved",
                            ContextSourceKind::Memory,
                            "Project Memory",
                            memory_block,
                            85,
                            "memory_included",
                            false,
                        ) {
                            trace_events.push(make_trace_event(
                                &trace_id,
                                TraceEventType::SourceCollected,
                                Some("memory"),
                                Some("memory:retrieved"),
                                "memory context included",
                                Some(json!({ "entries": memory_entries.len() })),
                            ));
                        }
                    }
                }
            }
        }

        // 4b) Knowledge
        if config
            .knowledge
            .as_ref()
            .map(|k| k.enabled)
            .unwrap_or(false)
        {
            if is_source_excluded(
                &context_policy,
                &ContextSourceKind::Knowledge,
                "knowledge:retrieved",
            ) {
                trace_events.push(make_trace_event(
                    &trace_id,
                    TraceEventType::SourceSkipped,
                    Some("knowledge"),
                    Some("knowledge:retrieved"),
                    "knowledge source excluded by policy",
                    None,
                ));
            } else {
                if fault_injection.knowledge_timeout {
                    trace_events.push(make_trace_event(
                        &trace_id,
                        TraceEventType::SourceFailed,
                        Some("knowledge"),
                        Some("knowledge:retrieved"),
                        "knowledge timeout injected by chaos probe",
                        Some(json!({ "fault": "knowledge_timeout" })),
                    ));
                } else {
                    let kcfg = config.knowledge.as_ref().unwrap();
                    ensure_knowledge_initialized_public(&knowledge_state, &app_state).await;
                    match knowledge_state.get_pipeline().await {
                        Ok(pipeline) => {
                            let provider = KnowledgeContextProvider::new(pipeline);
                            let project_id = request
                                .project_id
                                .clone()
                                .unwrap_or_else(|| config.project_id.clone());
                            let query_cfg = KnowledgeContextConfig {
                                collection_ids: if kcfg.selected_collections.is_empty() {
                                    None
                                } else {
                                    Some(kcfg.selected_collections.clone())
                                },
                                document_refs: if kcfg.selected_documents.is_empty() {
                                    None
                                } else {
                                    Some(kcfg.selected_documents.clone())
                                },
                                ..KnowledgeContextConfig::default()
                            };
                            match provider
                                .query_for_context(&project_id, &request.query, &query_cfg)
                                .await
                            {
                                Ok(chunks) => {
                                    let block =
                                        KnowledgeContextProvider::format_context_block(&chunks);
                                    if !block.is_empty() {
                                        if push_block(
                                            &context_policy,
                                            &mut sources,
                                            &mut blocks,
                                            "knowledge:retrieved",
                                            ContextSourceKind::Knowledge,
                                            "Knowledge Base",
                                            block,
                                            75,
                                            "knowledge_included",
                                            false,
                                        ) {
                                            trace_events.push(make_trace_event(
                                                &trace_id,
                                                TraceEventType::SourceCollected,
                                                Some("knowledge"),
                                                Some("knowledge:retrieved"),
                                                "knowledge context included",
                                                Some(json!({ "chunks": chunks.len() })),
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    trace_events.push(make_trace_event(
                                        &trace_id,
                                        TraceEventType::SourceFailed,
                                        Some("knowledge"),
                                        Some("knowledge:retrieved"),
                                        "knowledge query failed",
                                        Some(json!({ "error": e.to_string() })),
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            trace_events.push(make_trace_event(
                                &trace_id,
                                TraceEventType::SourceFailed,
                                Some("knowledge"),
                                Some("knowledge:pipeline"),
                                "knowledge pipeline unavailable",
                                Some(json!({ "error": e.to_string() })),
                            ));
                        }
                    }
                }
            }
        }

        // 4c) Skills
        if config.skills.as_ref().map(|s| s.enabled).unwrap_or(false) {
            if is_source_excluded(
                &context_policy,
                &ContextSourceKind::Skills,
                "skills:selected",
            ) {
                trace_events.push(make_trace_event(
                    &trace_id,
                    TraceEventType::SourceSkipped,
                    Some("skills"),
                    Some("skills:selected"),
                    "skills source excluded by policy",
                    None,
                ));
            } else {
                let scfg = config.skills.as_ref().unwrap();
                let (skills_block, skill_expertise) = select_skills_for_task_filtered(
                    &app_state,
                    &project_path,
                    &request.query,
                    InjectionPhase::Always,
                    &scfg.selected_skill_ids,
                )
                .await;

                if !skills_block.is_empty() {
                    if push_block(
                        &context_policy,
                        &mut sources,
                        &mut blocks,
                        "skills:selected",
                        ContextSourceKind::Skills,
                        "Skills",
                        skills_block,
                        70,
                        "skills_included",
                        false,
                    ) {
                        trace_events.push(make_trace_event(
                            &trace_id,
                            TraceEventType::SourceCollected,
                            Some("skills"),
                            Some("skills:selected"),
                            "skills context included",
                            Some(json!({ "expertise": skill_expertise })),
                        ));
                    }
                }
            }
        }
    }

    let (mut retained_blocks, mut compaction) =
        apply_budget_and_compaction(blocks, &mut sources, input_budget, &compaction_policy);

    if fault_injection.compaction_quality_fail {
        compaction.quality_score = 0.05;
        compaction.quality_basis = json!({
            "fault": "compaction_quality_fail",
            "note": "quality forced low by chaos probe",
        });
        compaction.compaction_actions.push(CompactionAction {
            stage: "fault_injection".to_string(),
            action: "quality_override".to_string(),
            source_id: "compaction".to_string(),
            before_tokens: compaction.before_tokens,
            after_tokens: compaction.after_tokens,
            reason: "chaos_probe".to_string(),
        });
    }

    retained_blocks.sort_by(|a, b| b.priority.cmp(&a.priority));

    let used_input_tokens = retained_blocks.iter().map(|b| b.token_cost).sum::<usize>();
    let budget = ContextBudget {
        input_token_budget: input_budget,
        reserved_output_tokens,
        hard_limit,
        used_input_tokens,
        over_budget: used_input_tokens > input_budget,
    };

    trace_events.push(make_trace_event(
        &trace_id,
        TraceEventType::Compaction,
        None,
        None,
        if compaction.triggered {
            "compaction applied"
        } else {
            "compaction skipped"
        },
        Some(json!({
            "before_tokens": compaction.before_tokens,
            "after_tokens": compaction.after_tokens,
            "trigger_reason": compaction.trigger_reason,
            "strategy": compaction.strategy,
            "actions_count": compaction.compaction_actions.len(),
            "quality_score": compaction.quality_score,
            "quality_basis": compaction.quality_basis,
        })),
    ));

    if fault_injection.compaction_quality_fail {
        persist_trace_events(
            &app_state,
            request.session_id.as_deref(),
            Some(&turn_id),
            &trace_events,
        )
        .await;
        return Ok(CommandResponse::err(
            "compaction quality below threshold (fault injection)",
        ));
    }

    let assembled_prompt = build_prompt(&request.query, &retained_blocks);
    trace_events.push(make_trace_event(
        &trace_id,
        TraceEventType::AssembleDone,
        None,
        None,
        "context envelope assembled",
        Some(json!({
            "blocks": retained_blocks.len(),
            "used_tokens": budget.used_input_tokens,
        })),
    ));

    let envelope = ContextEnvelope {
        request_meta: ContextRequestMeta {
            turn_id: turn_id.clone(),
            session_id: request.session_id.clone(),
            mode,
            query: request.query.clone(),
            intent: request.intent.clone(),
        },
        budget,
        sources,
        blocks: retained_blocks,
        compaction,
        trace_id: trace_id.clone(),
        assembled_prompt,
    };

    persist_trace_events(
        &app_state,
        request.session_id.as_deref(),
        Some(&turn_id),
        &trace_events,
    )
    .await;

    Ok(CommandResponse::ok(envelope))
}

#[tauri::command]
pub async fn assemble_turn_context(
    request: PrepareTurnContextV2Request,
    app_state: State<'_, AppState>,
    knowledge_state: State<'_, KnowledgeState>,
) -> Result<CommandResponse<ContextAssemblyResponse>, String> {
    let project_path = request.project_path.trim().to_string();
    let query = request.query.trim().to_string();
    if project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required"));
    }
    if query.is_empty() {
        return Ok(CommandResponse::err("query is required"));
    }

    let injected_source_kinds = infer_injected_source_kinds(&request);
    let context_policy = load_context_policy(app_state.inner()).await;
    if !context_policy.context_v2_pipeline {
        let fallback_prompt =
            build_legacy_fallback_prompt(&request, app_state.inner(), knowledge_state.inner())
                .await;
        return Ok(CommandResponse::ok(build_legacy_fallback_response(
            &request,
            fallback_prompt,
            "policy_context_v2_disabled",
            Some("context_v2_pipeline disabled by policy".to_string()),
            injected_source_kinds,
        )));
    }

    let primary_result = prepare_turn_context_v2_internal(
        request.clone(),
        app_state.clone(),
        knowledge_state.clone(),
    )
    .await;
    if let Ok(primary) = &primary_result {
        if primary.success {
            if let Some(envelope) = primary.data.clone() {
                return Ok(CommandResponse::ok(map_envelope_to_assembly_response(
                    envelope,
                    injected_source_kinds,
                )));
            }
        }
    }

    let fallback_reason = match primary_result {
        Ok(primary) => primary
            .error
            .or_else(|| Some("prepare_turn_context_v2 returned no envelope".to_string())),
        Err(err) => Some(format!("prepare_turn_context_v2 failed: {}", err)),
    };
    let fallback_prompt =
        build_legacy_fallback_prompt(&request, app_state.inner(), knowledge_state.inner()).await;

    Ok(CommandResponse::ok(build_legacy_fallback_response(
        &request,
        fallback_prompt,
        "legacy_fallback",
        fallback_reason,
        injected_source_kinds,
    )))
}

#[tauri::command]
pub async fn prepare_turn_context_v2(
    request: PrepareTurnContextV2Request,
    app_state: State<'_, AppState>,
    knowledge_state: State<'_, KnowledgeState>,
) -> Result<CommandResponse<ContextEnvelope>, String> {
    tracing::warn!(
        "prepare_turn_context_v2 is deprecated; routing request through assemble_turn_context"
    );
    let assembled = assemble_turn_context(request, app_state, knowledge_state).await?;
    if !assembled.success {
        return Ok(CommandResponse::err(
            assembled
                .error
                .unwrap_or_else(|| "assemble_turn_context failed".to_string()),
        ));
    }

    match assembled.data {
        Some(data) => Ok(CommandResponse::ok(ContextEnvelope {
            request_meta: data.request_meta,
            budget: data.budget,
            sources: data.sources,
            blocks: data.blocks,
            compaction: data.compaction,
            trace_id: data.trace_id,
            assembled_prompt: data.assembled_prompt,
        })),
        None => Ok(CommandResponse::err(
            "assemble_turn_context returned no data".to_string(),
        )),
    }
}

#[tauri::command]
pub async fn get_context_trace(
    trace_id: String,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<ContextTrace>, String> {
    let trace_id = trace_id.trim().to_string();
    if trace_id.is_empty() {
        return Ok(CommandResponse::err("trace_id is required"));
    }

    match app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            let mut stmt = conn.prepare(
                "SELECT trace_id, event_type, source_kind, source_id, message, metadata, created_at
                 FROM context_trace_events
                 WHERE trace_id = ?1
                 ORDER BY id ASC",
            )?;

            let rows = stmt
                .query_map(rusqlite::params![trace_id], |row| {
                    let metadata_text: String = row.get(5)?;
                    Ok(ContextTraceEvent {
                        trace_id: row.get(0)?,
                        event_type: row.get(1)?,
                        source_kind: row.get(2)?,
                        source_id: row.get(3)?,
                        message: row.get(4)?,
                        metadata: serde_json::from_str(&metadata_text).ok(),
                        created_at: row.get(6)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect::<Vec<_>>();

            Ok(rows)
        })
        .await
    {
        Ok(events) => Ok(CommandResponse::ok(ContextTrace { trace_id, events })),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn set_context_policy(
    policy: ContextPolicy,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<PolicyAck>, String> {
    let json = match serde_json::to_string(&policy) {
        Ok(v) => v,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match app_state
        .with_database(|db| db.set_setting(CONTEXT_POLICY_KEY, &json))
        .await
    {
        Ok(_) => Ok(CommandResponse::ok(PolicyAck {
            key: CONTEXT_POLICY_KEY.to_string(),
            updated_at: now_string(),
        })),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn get_context_policy(
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<ContextPolicy>, String> {
    match app_state
        .with_database(|db| db.get_setting(CONTEXT_POLICY_KEY))
        .await
    {
        Ok(Some(raw)) => match serde_json::from_str::<ContextPolicy>(&raw) {
            Ok(policy) => Ok(CommandResponse::ok(policy)),
            Err(_) => Ok(CommandResponse::ok(ContextPolicy::default())),
        },
        Ok(None) => Ok(CommandResponse::ok(ContextPolicy::default())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn save_context_artifact(
    input: SaveContextArtifactInput,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<ContextArtifactMeta>, String> {
    if input.name.trim().is_empty() {
        return Ok(CommandResponse::err("artifact name is required"));
    }
    if input.project_path.trim().is_empty() {
        return Ok(CommandResponse::err("project_path is required"));
    }

    let artifact_id = uuid::Uuid::new_v4().to_string();
    let now = now_string();
    let envelope_json = match serde_json::to_string(&input.envelope) {
        Ok(v) => v,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let name = input.name.trim().to_string();
    let project_path = input.project_path.trim().to_string();
    let session_id = input.session_id.clone();

    match app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            conn.execute(
                "INSERT INTO context_artifacts (id, name, project_path, session_id, envelope_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    artifact_id,
                    name,
                    project_path,
                    session_id,
                    envelope_json,
                    now,
                    now,
                ],
            )?;
            Ok(())
        })
        .await
    {
        Ok(_) => Ok(CommandResponse::ok(ContextArtifactMeta {
            id: artifact_id,
            name,
            project_path,
            session_id,
            created_at: now.clone(),
            updated_at: now,
        })),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn list_context_artifacts(
    project_path: String,
    session_id: Option<String>,
    limit: Option<usize>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<ContextArtifactMeta>>, String> {
    let project_path = project_path.trim().to_string();
    if project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required"));
    }
    let lim = limit.unwrap_or(50).max(1).min(500);

    match app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            let mut rows = Vec::new();
            if let Some(ref sid) = session_id {
                let mut stmt = conn.prepare(
                    "SELECT id, name, project_path, session_id, created_at, updated_at
                     FROM context_artifacts
                     WHERE project_path = ?1 AND session_id = ?2
                     ORDER BY updated_at DESC
                     LIMIT ?3",
                )?;
                let iter =
                    stmt.query_map(rusqlite::params![project_path, sid, lim as i64], |row| {
                        Ok(ContextArtifactMeta {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            project_path: row.get(2)?,
                            session_id: row.get(3)?,
                            created_at: row.get(4)?,
                            updated_at: row.get(5)?,
                        })
                    })?;
                rows.extend(iter.filter_map(|r| r.ok()));
            } else {
                let mut stmt = conn.prepare(
                    "SELECT id, name, project_path, session_id, created_at, updated_at
                     FROM context_artifacts
                     WHERE project_path = ?1
                     ORDER BY updated_at DESC
                     LIMIT ?2",
                )?;
                let iter = stmt.query_map(rusqlite::params![project_path, lim as i64], |row| {
                    Ok(ContextArtifactMeta {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        project_path: row.get(2)?,
                        session_id: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                })?;
                rows.extend(iter.filter_map(|r| r.ok()));
            }
            Ok(rows)
        })
        .await
    {
        Ok(rows) => Ok(CommandResponse::ok(rows)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn apply_context_artifact(
    artifact_id: String,
    session_id: Option<String>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<ApplyResult>, String> {
    let artifact_id = artifact_id.trim().to_string();
    if artifact_id.is_empty() {
        return Ok(CommandResponse::err("artifact_id is required"));
    }

    let loaded = app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            let mut stmt =
                conn.prepare("SELECT envelope_json FROM context_artifacts WHERE id = ?1 LIMIT 1")?;
            let envelope_json: String =
                stmt.query_row(rusqlite::params![artifact_id.clone()], |row| row.get(0))?;
            Ok(envelope_json)
        })
        .await;

    let envelope_json = match loaded {
        Ok(raw) => raw,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let envelope = match serde_json::from_str::<ContextEnvelope>(&envelope_json) {
        Ok(v) => v,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Invalid artifact payload: {}",
                e
            )))
        }
    };

    if let Some(ref sid) = session_id {
        let key = format!("context_artifact_applied:{}", sid);
        let _ = app_state
            .with_database(|db| db.set_setting(&key, &artifact_id))
            .await;
    }

    Ok(CommandResponse::ok(ApplyResult {
        artifact_id,
        session_id,
        applied: true,
        envelope,
    }))
}

#[tauri::command]
pub async fn delete_context_artifact(
    artifact_id: String,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    let artifact_id = artifact_id.trim().to_string();
    if artifact_id.is_empty() {
        return Ok(CommandResponse::err("artifact_id is required"));
    }

    match app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            let deleted = conn.execute(
                "DELETE FROM context_artifacts WHERE id = ?1",
                rusqlite::params![artifact_id],
            )?;
            Ok(deleted > 0)
        })
        .await
    {
        Ok(ok) => Ok(CommandResponse::ok(ok)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn get_context_rollout(
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<ContextRolloutConfig>, String> {
    match app_state
        .with_database(|db| db.get_setting(CONTEXT_ROLLOUT_KEY))
        .await
    {
        Ok(Some(raw)) => match serde_json::from_str::<ContextRolloutConfig>(&raw) {
            Ok(cfg) => Ok(CommandResponse::ok(clamp_rollout_config(cfg))),
            Err(_) => Ok(CommandResponse::ok(ContextRolloutConfig::default())),
        },
        Ok(None) => Ok(CommandResponse::ok(ContextRolloutConfig::default())),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn set_context_rollout(
    config: ContextRolloutConfig,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<PolicyAck>, String> {
    let normalized = clamp_rollout_config(config);
    let json = match serde_json::to_string(&normalized) {
        Ok(v) => v,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    match app_state
        .with_database(|db| db.set_setting(CONTEXT_ROLLOUT_KEY, &json))
        .await
    {
        Ok(_) => Ok(CommandResponse::ok(PolicyAck {
            key: CONTEXT_ROLLOUT_KEY.to_string(),
            updated_at: now_string(),
        })),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

fn parse_event_time(value: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn percentile_ms(values: &mut [f64], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((values.len().saturating_sub(1) as f64) * percentile).round() as usize;
    values[idx.min(values.len().saturating_sub(1))]
}

#[derive(Default)]
struct TraceAggregation {
    project_path: Option<String>,
    start_at: Option<chrono::DateTime<Utc>>,
    end_at: Option<chrono::DateTime<Utc>>,
    source_failed: bool,
    degraded: bool,
    variant: Option<String>,
    compaction_saving_tokens: i64,
}

#[tauri::command]
pub async fn get_context_ops_dashboard(
    request: ContextOpsDashboardRequest,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<ContextOpsDashboard>, String> {
    let project_path = request.project_path.trim().to_string();
    if project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required"));
    }

    let window_hours = request
        .window_hours
        .unwrap_or(DEFAULT_CONTEXT_WINDOW_HOURS)
        .clamp(1, 24 * 30);
    let window_end_dt = Utc::now();
    let window_start_dt = window_end_dt - chrono::Duration::hours(window_hours as i64);
    let window_start = window_start_dt.to_rfc3339();

    let traces = match app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            let mut stmt = conn.prepare(
                "SELECT trace_id, event_type, metadata, created_at
                 FROM context_trace_events
                 WHERE created_at >= ?1
                 ORDER BY id ASC",
            )?;

            let rows = stmt
                .query_map(rusqlite::params![window_start], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect::<Vec<_>>();
            Ok(rows)
        })
        .await
    {
        Ok(rows) => rows,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let mut grouped: HashMap<String, TraceAggregation> = HashMap::new();
    for (trace_id, event_type, metadata_text, created_at) in traces {
        let entry = grouped.entry(trace_id).or_default();
        let metadata = serde_json::from_str::<serde_json::Value>(&metadata_text).ok();
        let created_at_dt = parse_event_time(&created_at);
        match TraceEventType::from_str(&event_type) {
            Some(TraceEventType::CollectStart) => {
                if let Some(ref m) = metadata {
                    if let Some(path) = m.get("project_path").and_then(|v| v.as_str()) {
                        entry.project_path = Some(path.to_string());
                    }
                }
                if entry.start_at.is_none() {
                    entry.start_at = created_at_dt;
                }
            }
            Some(TraceEventType::AssembleDone) => {
                entry.end_at = created_at_dt;
            }
            Some(TraceEventType::SourceFailed) => {
                entry.source_failed = true;
                entry.degraded = true;
            }
            Some(TraceEventType::RolloutAssignment) => {
                if let Some(ref m) = metadata {
                    entry.variant = m
                        .get("variant")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
            Some(TraceEventType::Compaction) => {
                if let Some(ref m) = metadata {
                    let before = m.get("before_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                    let after = m.get("after_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
                    entry.compaction_saving_tokens += (before - after).max(0);
                }
            }
            _ => {}
        }
    }

    let mut total_traces = 0usize;
    let mut assembled_traces = 0usize;
    let mut degraded_traces = 0usize;
    let mut source_failure_traces = 0usize;
    let mut compaction_saving_total = 0i64;
    let mut latencies_ms: Vec<f64> = Vec::new();
    let mut variant_latencies: HashMap<String, Vec<f64>> = HashMap::new();
    let mut variant_counts: HashMap<String, usize> = HashMap::new();
    let mut variant_degraded_counts: HashMap<String, usize> = HashMap::new();

    for (_, trace) in grouped {
        if trace.project_path.as_deref() != Some(project_path.as_str()) {
            continue;
        }
        total_traces += 1;
        if trace.degraded {
            degraded_traces += 1;
        }
        if trace.source_failed {
            source_failure_traces += 1;
        }
        compaction_saving_total += trace.compaction_saving_tokens;

        let variant = trace
            .variant
            .clone()
            .unwrap_or_else(|| "unassigned".to_string());
        *variant_counts.entry(variant.clone()).or_insert(0) += 1;
        if trace.degraded {
            *variant_degraded_counts.entry(variant.clone()).or_insert(0) += 1;
        }

        if let (Some(start), Some(end)) = (trace.start_at, trace.end_at) {
            if end >= start {
                assembled_traces += 1;
                let latency_ms = (end - start).num_milliseconds() as f64;
                latencies_ms.push(latency_ms);
                variant_latencies
                    .entry(variant)
                    .or_default()
                    .push(latency_ms);
            }
        }
    }

    let availability = if total_traces == 0 {
        1.0
    } else {
        assembled_traces as f32 / total_traces as f32
    };
    let degraded_rate = if total_traces == 0 {
        0.0
    } else {
        degraded_traces as f32 / total_traces as f32
    };

    let p50_ms = percentile_ms(&mut latencies_ms.clone(), 0.50) as f32;
    let p95_ms = percentile_ms(&mut latencies_ms, 0.95) as f32;

    let avg_compaction_saving_tokens = if total_traces == 0 {
        0.0
    } else {
        compaction_saving_total as f32 / total_traces as f32
    };

    let mut ab_variants: Vec<ContextOpsVariantStat> = variant_counts
        .iter()
        .map(|(variant, traces)| {
            let degraded = *variant_degraded_counts.get(variant).unwrap_or(&0);
            let degraded_rate = if *traces == 0 {
                0.0
            } else {
                degraded as f32 / *traces as f32
            };
            let avg_latency_ms = variant_latencies
                .get(variant)
                .map(|values| {
                    if values.is_empty() {
                        0.0
                    } else {
                        (values.iter().sum::<f64>() / values.len() as f64) as f32
                    }
                })
                .unwrap_or(0.0);
            ContextOpsVariantStat {
                variant: variant.clone(),
                traces: *traces,
                degraded_rate,
                avg_latency_ms,
            }
        })
        .collect();
    ab_variants.sort_by(|a, b| b.traces.cmp(&a.traces));

    let mut alerts = Vec::new();
    if p95_ms > 300.0 {
        alerts.push(ContextOpsAlert {
            code: "prepare_context_p95".to_string(),
            severity: "high".to_string(),
            message: "prepare_turn_context_v2 p95 latency exceeded SLO".to_string(),
            value: p95_ms,
            threshold: 300.0,
        });
    }
    if degraded_rate > 0.10 {
        alerts.push(ContextOpsAlert {
            code: "degraded_rate".to_string(),
            severity: "high".to_string(),
            message: "degraded rate exceeded threshold".to_string(),
            value: degraded_rate,
            threshold: 0.10,
        });
    }
    if availability < 0.999 {
        alerts.push(ContextOpsAlert {
            code: "availability".to_string(),
            severity: "critical".to_string(),
            message: "context assemble availability below target".to_string(),
            value: availability,
            threshold: 0.999,
        });
    }

    let recent_chaos_runs = match app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            let mut stmt = conn.prepare(
                "SELECT run_id, project_path, session_id, report_json, created_at
                 FROM context_chaos_runs
                 WHERE project_path = ?1
                 ORDER BY created_at DESC
                 LIMIT 5",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![project_path], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect::<Vec<_>>();
            Ok(rows)
        })
        .await
    {
        Ok(rows) => rows
            .into_iter()
            .map(
                |(run_id, project_path, session_id, report_json, created_at)| {
                    let parsed = serde_json::from_str::<ContextChaosProbeReport>(&report_json).ok();
                    ContextChaosRunMeta {
                        run_id,
                        project_path,
                        session_id,
                        created_at,
                        iterations: parsed.as_ref().map(|r| r.iterations).unwrap_or(0),
                        fallback_success_rate: parsed
                            .as_ref()
                            .map(|r| r.fallback_success_rate)
                            .unwrap_or(0.0),
                    }
                },
            )
            .collect(),
        Err(_) => Vec::new(),
    };

    let policy = load_context_policy(&app_state).await;
    let rollout = load_context_rollout(&app_state).await;

    Ok(CommandResponse::ok(ContextOpsDashboard {
        project_path,
        window_start: window_start_dt.to_rfc3339(),
        window_end: window_end_dt.to_rfc3339(),
        window_hours,
        total_traces,
        assembled_traces,
        availability,
        degraded_traces,
        degraded_rate,
        source_failure_traces,
        prepare_context_p50_ms: p50_ms,
        prepare_context_p95_ms: p95_ms,
        total_compaction_saving_tokens: compaction_saving_total,
        avg_compaction_saving_tokens,
        ab_variants,
        alerts,
        policy,
        rollout,
        recent_chaos_runs,
        runbook_path: CONTEXT_RUNBOOK_PATH.to_string(),
    }))
}

#[tauri::command]
pub async fn run_context_chaos_probe(
    request: ContextChaosProbeRequest,
    app_state: State<'_, AppState>,
    knowledge_state: State<'_, KnowledgeState>,
) -> Result<CommandResponse<ContextChaosProbeReport>, String> {
    let project_path = request.project_path.trim().to_string();
    if project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required"));
    }
    let iterations = request.iterations.unwrap_or(20).clamp(1, 500);
    let failure_probability = request.failure_probability.unwrap_or(0.15).clamp(0.0, 1.0);
    let created_at = now_string();
    let run_id = uuid::Uuid::new_v4().to_string();
    let session_id = request.session_id.clone();

    let scenario_names = [
        "memory_provider_timeout",
        "knowledge_provider_timeout",
        "ranker_unavailable",
        "compaction_quality_fail",
    ];

    let mut scenarios = Vec::new();
    let mut injected_faults = 0u32;
    let mut fallback_successes = 0u32;

    for i in 0..iterations {
        for scenario in scenario_names {
            let inject_seed = format!("{}:{}:{}:inject", run_id, scenario, i);
            let inject_roll = stable_bucket(&inject_seed) as f32 / 100.0;
            let injected = inject_roll < failure_probability;
            if !injected {
                continue;
            }
            injected_faults += 1;

            let fault = match scenario {
                "memory_provider_timeout" => ContextFaultInjection {
                    memory_timeout: true,
                    ..ContextFaultInjection::default()
                },
                "knowledge_provider_timeout" => ContextFaultInjection {
                    knowledge_timeout: true,
                    ..ContextFaultInjection::default()
                },
                "ranker_unavailable" => ContextFaultInjection {
                    ranker_unavailable: true,
                    ..ContextFaultInjection::default()
                },
                "compaction_quality_fail" => ContextFaultInjection {
                    compaction_quality_fail: true,
                    ..ContextFaultInjection::default()
                },
                _ => ContextFaultInjection::default(),
            };

            let chaos_context_sources = ContextSourceConfig {
                project_id: "default".to_string(),
                knowledge: Some(KnowledgeSourceConfig {
                    enabled: true,
                    selected_collections: Vec::new(),
                    selected_documents: Vec::new(),
                }),
                memory: Some(MemorySourceConfig {
                    enabled: true,
                    selected_categories: Vec::new(),
                    selected_memory_ids: Vec::new(),
                    excluded_memory_ids: Vec::new(),
                    selected_scopes: vec!["project".to_string(), "global".to_string()],
                    session_id: session_id.clone(),
                }),
                skills: Some(SkillsSourceConfig {
                    enabled: false,
                    selected_skill_ids: Vec::new(),
                }),
            };

            let probe_request = PrepareTurnContextV2Request {
                project_path: project_path.clone(),
                query: format!(
                    "Chaos probe {} iteration {} - verify fallback behavior.",
                    scenario, i
                ),
                project_id: Some("default".to_string()),
                session_id: session_id.clone(),
                mode: Some("chaos_probe".to_string()),
                turn_id: Some(format!("chaos:{}:{}:{}", run_id, scenario, i)),
                intent: Some("qa".to_string()),
                conversation_history: Vec::new(),
                context_sources: Some(chaos_context_sources),
                rules: Vec::new(),
                manual_blocks: Vec::new(),
                input_token_budget: Some(if scenario == "compaction_quality_fail" {
                    640
                } else {
                    DEFAULT_INPUT_TOKEN_BUDGET
                }),
                reserved_output_tokens: Some(2048),
                hard_limit: Some(if scenario == "compaction_quality_fail" {
                    2688
                } else {
                    DEFAULT_INPUT_TOKEN_BUDGET + DEFAULT_RESERVED_OUTPUT_TOKENS
                }),
                compaction_policy: None,
                fault_injection: Some(fault),
            };

            let probe_result =
                assemble_turn_context(probe_request, app_state.clone(), knowledge_state.clone())
                    .await;
            let fallback_ok = match probe_result {
                Ok(response) if response.success => {
                    if let Some(data) = response.data {
                        let basic_ok = !data.assembled_prompt.trim().is_empty();
                        let scenario_requires_fallback = scenario == "compaction_quality_fail";
                        if scenario_requires_fallback {
                            basic_ok && data.fallback_used
                        } else {
                            basic_ok
                        }
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if fallback_ok {
                fallback_successes += 1;
            }

            scenarios.push(ContextChaosScenarioResult {
                scenario: scenario.to_string(),
                injected: true,
                fallback_ok,
                warning_emitted: !fallback_ok,
            });
        }
    }

    let fallback_success_rate = if injected_faults == 0 {
        1.0
    } else {
        fallback_successes as f32 / injected_faults as f32
    };
    let recommendation = if fallback_success_rate < 0.85 {
        "Fallback reliability below target; pause rollout and investigate provider health."
            .to_string()
    } else if fallback_success_rate < 0.95 {
        "Fallback reliability marginal; keep rollout limited and increase monitoring.".to_string()
    } else {
        "Fallback reliability healthy; safe to continue gradual rollout.".to_string()
    };

    let report = ContextChaosProbeReport {
        run_id: run_id.clone(),
        project_path: project_path.clone(),
        session_id: session_id.clone(),
        created_at: created_at.clone(),
        iterations,
        failure_probability,
        injected_faults,
        fallback_success_rate,
        scenarios,
        recommendation,
    };
    let report_json = match serde_json::to_string(&report) {
        Ok(v) => v,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    let persisted = app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            conn.execute(
                "INSERT INTO context_chaos_runs (run_id, project_path, session_id, report_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![run_id, project_path, session_id, report_json, created_at],
            )?;
            Ok(())
        })
        .await;

    match persisted {
        Ok(_) => Ok(CommandResponse::ok(report)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[tauri::command]
pub async fn list_context_chaos_runs(
    project_path: String,
    limit: Option<usize>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<ContextChaosRunMeta>>, String> {
    let project_path = project_path.trim().to_string();
    if project_path.is_empty() {
        return Ok(CommandResponse::err("project_path is required"));
    }
    let lim = limit.unwrap_or(20).clamp(1, 200);

    match app_state
        .with_database(|db| {
            let conn = db.get_connection()?;
            let mut stmt = conn.prepare(
                "SELECT run_id, project_path, session_id, report_json, created_at
                 FROM context_chaos_runs
                 WHERE project_path = ?1
                 ORDER BY created_at DESC
                 LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![project_path, lim as i64], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .filter_map(|r| r.ok())
                .map(
                    |(run_id, project_path, session_id, report_json, created_at)| {
                        let parsed =
                            serde_json::from_str::<ContextChaosProbeReport>(&report_json).ok();
                        ContextChaosRunMeta {
                            run_id,
                            project_path,
                            session_id,
                            created_at,
                            iterations: parsed.as_ref().map(|r| r.iterations).unwrap_or(0),
                            fallback_success_rate: parsed
                                .as_ref()
                                .map(|r| r.fallback_success_rate)
                                .unwrap_or(0.0),
                        }
                    },
                )
                .collect::<Vec<_>>();
            Ok(rows)
        })
        .await
    {
        Ok(rows) => Ok(CommandResponse::ok(rows)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_assemble_request() -> PrepareTurnContextV2Request {
        PrepareTurnContextV2Request {
            project_path: "/tmp/project".to_string(),
            query: "test query".to_string(),
            project_id: Some("default".to_string()),
            session_id: Some("session-1".to_string()),
            mode: Some("standalone".to_string()),
            turn_id: Some("turn-1".to_string()),
            intent: Some("qa".to_string()),
            conversation_history: Vec::new(),
            context_sources: None,
            rules: Vec::new(),
            manual_blocks: Vec::new(),
            input_token_budget: Some(1024),
            reserved_output_tokens: Some(256),
            hard_limit: Some(2048),
            compaction_policy: None,
            fault_injection: None,
        }
    }

    #[test]
    fn test_compaction_event_contract_name() {
        assert_eq!(TraceEventType::Compaction.as_str(), "compaction");
        assert_ne!(TraceEventType::Compaction.as_str(), "compaction_done");
    }

    #[test]
    fn test_source_selector_matches_kind_and_id() {
        assert!(source_selector_matches(
            &ContextSourceKind::Memory,
            "memory:retrieved",
            "memory"
        ));
        assert!(source_selector_matches(
            &ContextSourceKind::Memory,
            "memory:retrieved",
            "id:memory:retrieved"
        ));
        assert!(source_selector_matches(
            &ContextSourceKind::Knowledge,
            "knowledge:retrieved",
            "exclude:knowledge"
        ));
        assert!(!source_selector_matches(
            &ContextSourceKind::Skills,
            "skills:selected",
            "memory"
        ));
    }

    #[test]
    fn test_push_block_respects_policy_exclusion_and_pinning() {
        let mut sources = Vec::new();
        let mut blocks = Vec::new();
        let mut policy = ContextPolicy::default();
        policy.excluded_sources = vec!["knowledge".to_string()];
        policy.pinned_sources = vec!["memory".to_string()];

        let inserted_knowledge = push_block(
            &policy,
            &mut sources,
            &mut blocks,
            "knowledge:retrieved",
            ContextSourceKind::Knowledge,
            "Knowledge",
            "k",
            50,
            "knowledge",
            false,
        );
        assert!(!inserted_knowledge);
        assert!(sources.is_empty());
        assert!(blocks.is_empty());

        let inserted_memory = push_block(
            &policy,
            &mut sources,
            &mut blocks,
            "memory:retrieved",
            ContextSourceKind::Memory,
            "Memory",
            "m",
            40,
            "memory",
            false,
        );
        assert!(inserted_memory);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].reason, "pinned_by_policy");
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].anchor);
        assert!(blocks[0].priority >= 120);
    }

    #[test]
    fn test_apply_budget_preserves_anchor() {
        let mut sources = vec![
            ContextSourceRef {
                id: "a".to_string(),
                kind: ContextSourceKind::Manual,
                label: "A".to_string(),
                token_cost: 120,
                included: true,
                reason: "manual".to_string(),
            },
            ContextSourceRef {
                id: "b".to_string(),
                kind: ContextSourceKind::History,
                label: "B".to_string(),
                token_cost: 80,
                included: true,
                reason: "history".to_string(),
            },
        ];
        let blocks = vec![
            ContextBlock {
                source_id: "a".to_string(),
                title: "A".to_string(),
                content: "A".repeat(480),
                token_cost: 120,
                priority: 100,
                reason: "selected".to_string(),
                anchor: true,
            },
            ContextBlock {
                source_id: "b".to_string(),
                title: "B".to_string(),
                content: "B".repeat(320),
                token_cost: 80,
                priority: 10,
                reason: "selected".to_string(),
                anchor: false,
            },
        ];

        let (retained, report) = apply_budget_and_compaction(
            blocks,
            &mut sources,
            120,
            &CompactionPolicy {
                preserve_anchors: true,
                ..CompactionPolicy::default()
            },
        );

        assert!(report.triggered);
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].source_id, "a");
        assert!(sources.iter().any(|s| s.id == "b" && !s.included));
    }

    #[test]
    fn test_apply_budget_emits_compaction_actions() {
        let mut sources = vec![
            ContextSourceRef {
                id: "low".to_string(),
                kind: ContextSourceKind::Memory,
                label: "Low".to_string(),
                token_cost: 360,
                included: true,
                reason: "selected".to_string(),
            },
            ContextSourceRef {
                id: "long".to_string(),
                kind: ContextSourceKind::Knowledge,
                label: "Long".to_string(),
                token_cost: 420,
                included: true,
                reason: "selected".to_string(),
            },
        ];
        let blocks = vec![
            ContextBlock {
                source_id: "low".to_string(),
                title: "Low".to_string(),
                content: "low-priority\n".repeat(200),
                token_cost: 360,
                priority: 5,
                reason: "selected".to_string(),
                anchor: false,
            },
            ContextBlock {
                source_id: "long".to_string(),
                title: "Long".to_string(),
                content: "line-a\nline-b\nline-c\nline-d\nline-e\nline-f\n".repeat(160),
                token_cost: 420,
                priority: 80,
                reason: "selected".to_string(),
                anchor: false,
            },
        ];

        let (_retained, report) =
            apply_budget_and_compaction(blocks, &mut sources, 260, &CompactionPolicy::default());

        assert!(report.triggered);
        assert!(!report.compaction_actions.is_empty());
        assert!(
            report.strategy == "trim_then_semantic_summary" || report.strategy == "priority_trim"
        );
        assert!(report.quality_basis.is_object());
    }

    #[test]
    fn test_build_legacy_fallback_response_uses_legacy_strategy() {
        let request = make_assemble_request();
        let prompt = "fallback prompt with selected source context".to_string();
        let response = build_legacy_fallback_response(
            &request,
            prompt.clone(),
            "legacy_fallback",
            Some("prepare failed".to_string()),
            vec!["memory".to_string(), "knowledge".to_string()],
        );

        assert!(response.fallback_used);
        assert_eq!(response.compaction.strategy, "legacy_with_selected_sources");
        assert_eq!(response.compaction.trigger_reason, "legacy_fallback");
        assert_eq!(response.fallback_reason.as_deref(), Some("prepare failed"));
        assert_eq!(response.assembled_prompt, prompt);
        assert_eq!(response.request_meta.turn_id, "turn-1");
        assert_eq!(response.request_meta.mode, "standalone");
        assert_eq!(response.budget.input_token_budget, 1024);
        assert_eq!(response.budget.reserved_output_tokens, 256);
        assert_eq!(response.budget.hard_limit, 2048);
        assert_eq!(
            response.budget.used_input_tokens,
            estimate_tokens_rough(&response.assembled_prompt)
        );
        assert_eq!(
            response.injected_source_kinds,
            vec!["memory".to_string(), "knowledge".to_string()]
        );
    }

    #[test]
    fn test_build_legacy_fallback_response_respects_minimum_budget_clamps() {
        let mut request = make_assemble_request();
        request.input_token_budget = Some(12);
        request.reserved_output_tokens = Some(16);
        request.hard_limit = Some(32);

        let response = build_legacy_fallback_response(
            &request,
            "short prompt".to_string(),
            "policy_context_v2_disabled",
            Some("policy off".to_string()),
            Vec::new(),
        );

        assert_eq!(response.budget.input_token_budget, 256);
        assert_eq!(response.budget.reserved_output_tokens, 128);
        assert_eq!(response.budget.hard_limit, 384);
        assert_eq!(
            response.compaction.trigger_reason,
            "policy_context_v2_disabled"
        );
    }
}
