//! Agentic Lifecycle Hooks
//!
//! Plugin-style hook system that decouples cross-cutting concerns (memory loading,
//! skill matching, memory extraction) from the agentic loop.
//!
//! ## Architecture (ADR-003)
//!
//! Hooks execute sequentially in registration order using Vec-based storage.
//! Each hook is an async callback stored as a boxed closure. Errors are logged
//! but do not disrupt the main agentic loop.
//!
//! ## Hook Points
//!
//! 1. `on_session_start`  - Session initialization (load memories, detect skills)
//! 2. `on_user_message`   - User message preprocessing (skill matching, message modification)
//! 3. `on_before_llm`     - Pre-LLM call (context injection)
//! 4. `on_after_llm`      - Post-LLM response (response analysis)
//! 5. `on_before_tool`    - Pre-tool execution (permission checks, skip decisions)
//! 6. `on_after_tool`     - Post-tool execution (result tracking)
//! 7. `on_session_end`    - Session teardown (memory extraction)
//! 8. `on_compaction`     - Context compaction (memory extraction from compacted content)

use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::services::memory::query_policy_v2::{memory_query_tuning_v2, MemoryQueryPresetV2};
use crate::services::memory::query_v2::{
    list_memory_entries_v2 as list_memory_entries_unified_v2,
    query_memory_entries_v2 as query_memory_entries_unified_v2, MemoryScopeV2, MemoryStatusV2,
    UnifiedMemoryQueryRequestV2,
};
use crate::services::memory::store::ProjectMemoryStore;
use crate::services::memory::store::{MemoryCategory, NewMemoryEntry};
use crate::services::skills::generator::SkillGeneratorStore;
use crate::services::skills::model::GeneratedSkill;
use crate::services::skills::model::{InjectionPhase, SelectionPolicy, SkillIndex, SkillMatch};
use crate::services::skills::select::select_skills_for_session;
use crate::utils::configure_background_process;

/// Context provided to all hooks, describing the current session state.
#[derive(Debug, Clone)]
pub struct HookContext {
    /// Unique session identifier
    pub session_id: String,
    /// Project root path
    pub project_path: PathBuf,
    /// LLM provider name (e.g., "anthropic", "openai")
    pub provider_name: String,
    /// LLM model name (e.g., "claude-3-5-sonnet-20241022")
    pub model_name: String,
    /// Execution identifier if the parent command provided one.
    pub execution_id: Option<String>,
    /// Structured task/content type if known (e.g. "prd").
    pub task_type: Option<String>,
}

/// Summary of a completed session, provided to on_session_end hooks.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    /// Description of the task from the first user message
    pub task_description: String,
    /// Files read during the session (paths)
    pub files_read: Vec<String>,
    /// Key findings extracted from the conversation
    pub key_findings: Vec<String>,
    /// Tool usage counts: tool_name -> invocation count
    pub tool_usage: HashMap<String, usize>,
    /// Total agentic loop iterations
    pub total_turns: u32,
    /// Whether the session completed successfully
    pub success: bool,
    /// Conversation text content for LLM memory extraction (up to ~8000 chars)
    pub conversation_content: String,
}

/// Result of an on_before_tool hook: can request skipping the tool call.
#[derive(Debug, Clone)]
pub struct BeforeToolResult {
    /// If true, skip this tool execution and use `skip_reason` as the tool result
    pub skip: bool,
    /// Reason for skipping (injected as tool result if skip=true)
    pub skip_reason: Option<String>,
    /// Replacement JSON arguments for the tool call.
    pub modified_arguments: Option<String>,
}

impl Default for BeforeToolResult {
    fn default() -> Self {
        Self {
            skip: false,
            skip_reason: None,
            modified_arguments: None,
        }
    }
}

/// Result of an on_user_message hook.
#[derive(Debug, Clone, Default)]
pub struct UserMessageHookResult {
    /// Replacement message to pass to downstream hooks/providers.
    pub modified_message: Option<String>,
    /// If set, stop the current turn before the LLM call.
    pub stop_reason: Option<String>,
}

/// Result of an on_after_tool hook: can optionally inject context into the conversation.
#[derive(Debug, Clone, Default)]
pub struct AfterToolResult {
    /// If present, this context is appended to the tool result message so the LLM sees it.
    pub injected_context: Option<String>,
    /// Replacement output to persist/show instead of the original tool result text.
    pub replacement_output: Option<String>,
    /// If set, treat the tool output as blocked and convert it into an error.
    pub block_reason: Option<String>,
}

impl AfterToolResult {
    /// Create a result with injected context.
    pub fn with_context(context: String) -> Self {
        Self {
            injected_context: Some(context),
            replacement_output: None,
            block_reason: None,
        }
    }
}

/// Result of an on_after_llm hook.
#[derive(Debug, Clone, Default)]
pub struct AfterLlmResult {
    /// Replacement assistant text.
    pub replacement_text: Option<String>,
    /// If set, the current turn should be terminated and surfaced as an error.
    pub block_reason: Option<String>,
}

// ============================================================================
// Type Aliases for Hook Callbacks
// ============================================================================

/// Hook fired at session start. Receives session context.
pub type OnSessionStartHook = Box<
    dyn Fn(HookContext) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync,
>;

/// Hook fired on user message. Can modify the message or stop the current turn.
pub type OnUserMessageHook = Box<
    dyn Fn(
            HookContext,
            String,
        ) -> Pin<Box<dyn Future<Output = Result<UserMessageHookResult, String>> + Send>>
        + Send
        + Sync,
>;

/// Hook fired before each LLM call. Receives current iteration count.
pub type OnBeforeLlmHook = Box<
    dyn Fn(HookContext, u32) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
        + Send
        + Sync,
>;

/// Hook fired after each LLM response. Receives the response text (if any).
pub type OnAfterLlmHook = Box<
    dyn Fn(
            HookContext,
            Option<String>,
        ) -> Pin<Box<dyn Future<Output = Result<AfterLlmResult, String>> + Send>>
        + Send
        + Sync,
>;

/// Hook fired before tool execution. Returns BeforeToolResult to optionally skip.
pub type OnBeforeToolHook = Box<
    dyn Fn(
            HookContext,
            String,
            String,
        ) -> Pin<Box<dyn Future<Output = Result<BeforeToolResult, String>> + Send>>
        + Send
        + Sync,
>;

/// Hook fired after tool execution. Receives tool name, success flag, and output snippet.
/// Returns `AfterToolResult` which can optionally inject context into the conversation.
pub type OnAfterToolHook = Box<
    dyn Fn(
            HookContext,
            String,
            bool,
            Option<String>,
        ) -> Pin<Box<dyn Future<Output = Result<AfterToolResult, String>> + Send>>
        + Send
        + Sync,
>;

/// Hook fired at session end. Receives the session summary.
pub type OnSessionEndHook = Box<
    dyn Fn(HookContext, SessionSummary) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
        + Send
        + Sync,
>;

/// Hook fired during context compaction. Receives compacted snippets.
pub type OnCompactionHook = Box<
    dyn Fn(HookContext, Vec<String>) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
        + Send
        + Sync,
>;

// ============================================================================
// AgenticHooks Registry
// ============================================================================

/// Registry of lifecycle hooks for the agentic loop.
///
/// Hooks are stored in Vecs and executed sequentially in registration order.
/// Errors from individual hooks are logged via `tracing` but do not prevent
/// subsequent hooks from executing or disrupt the main agentic loop.
pub struct AgenticHooks {
    on_session_start: Vec<OnSessionStartHook>,
    on_user_message: Vec<OnUserMessageHook>,
    on_before_llm: Vec<OnBeforeLlmHook>,
    on_after_llm: Vec<OnAfterLlmHook>,
    on_before_tool: Vec<OnBeforeToolHook>,
    on_after_tool: Vec<OnAfterToolHook>,
    on_session_end: Vec<OnSessionEndHook>,
    on_compaction: Vec<OnCompactionHook>,
    requested_stop: Arc<RwLock<Option<String>>>,
}

impl std::fmt::Debug for AgenticHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgenticHooks")
            .field("on_session_start", &self.on_session_start.len())
            .field("on_user_message", &self.on_user_message.len())
            .field("on_before_llm", &self.on_before_llm.len())
            .field("on_after_llm", &self.on_after_llm.len())
            .field("on_before_tool", &self.on_before_tool.len())
            .field("on_after_tool", &self.on_after_tool.len())
            .field("on_session_end", &self.on_session_end.len())
            .field("on_compaction", &self.on_compaction.len())
            .finish()
    }
}

impl AgenticHooks {
    /// Create a new empty hook registry.
    pub fn new() -> Self {
        Self {
            on_session_start: Vec::new(),
            on_user_message: Vec::new(),
            on_before_llm: Vec::new(),
            on_after_llm: Vec::new(),
            on_before_tool: Vec::new(),
            on_after_tool: Vec::new(),
            on_session_end: Vec::new(),
            on_compaction: Vec::new(),
            requested_stop: Arc::new(RwLock::new(None)),
        }
    }

    /// Returns true if no hooks are registered.
    pub fn is_empty(&self) -> bool {
        self.on_session_start.is_empty()
            && self.on_user_message.is_empty()
            && self.on_before_llm.is_empty()
            && self.on_after_llm.is_empty()
            && self.on_before_tool.is_empty()
            && self.on_after_tool.is_empty()
            && self.on_session_end.is_empty()
            && self.on_compaction.is_empty()
    }

    /// Total number of registered hooks across all hook points.
    pub fn total_hooks(&self) -> usize {
        self.on_session_start.len()
            + self.on_user_message.len()
            + self.on_before_llm.len()
            + self.on_after_llm.len()
            + self.on_before_tool.len()
            + self.on_after_tool.len()
            + self.on_session_end.len()
            + self.on_compaction.len()
    }

    // ========================================================================
    // Registration Methods
    // ========================================================================

    /// Register a hook to fire at session start.
    pub fn register_on_session_start(&mut self, hook: OnSessionStartHook) {
        self.on_session_start.push(hook);
    }

    /// Register a hook to fire on user message.
    pub fn register_on_user_message(&mut self, hook: OnUserMessageHook) {
        self.on_user_message.push(hook);
    }

    /// Register a hook to fire before each LLM call.
    pub fn register_on_before_llm(&mut self, hook: OnBeforeLlmHook) {
        self.on_before_llm.push(hook);
    }

    /// Register a hook to fire after each LLM response.
    pub fn register_on_after_llm(&mut self, hook: OnAfterLlmHook) {
        self.on_after_llm.push(hook);
    }

    /// Register a hook to fire before tool execution.
    pub fn register_on_before_tool(&mut self, hook: OnBeforeToolHook) {
        self.on_before_tool.push(hook);
    }

    /// Register a hook to fire after tool execution.
    pub fn register_on_after_tool(&mut self, hook: OnAfterToolHook) {
        self.on_after_tool.push(hook);
    }

    /// Register a hook to fire at session end.
    pub fn register_on_session_end(&mut self, hook: OnSessionEndHook) {
        self.on_session_end.push(hook);
    }

    /// Register a hook to fire during context compaction.
    pub fn register_on_compaction(&mut self, hook: OnCompactionHook) {
        self.on_compaction.push(hook);
    }

    // ========================================================================
    // Fire Methods (sequential execution with error propagation)
    // ========================================================================

    /// Fire all on_session_start hooks sequentially.
    ///
    /// Errors are logged but do not prevent subsequent hooks from executing.
    pub async fn fire_on_session_start(&self, ctx: &HookContext) {
        for (i, hook) in self.on_session_start.iter().enumerate() {
            if let Err(e) = hook(ctx.clone()).await {
                tracing::info!("[hooks] on_session_start hook {} failed: {}", i, e);
            }
        }
    }

    /// Fire all on_user_message hooks sequentially.
    ///
    /// Each hook can optionally modify the message or stop the current turn.
    pub async fn fire_on_user_message(
        &self,
        ctx: &HookContext,
        message: String,
    ) -> UserMessageHookResult {
        let mut current_message = message;
        let mut aggregate = UserMessageHookResult::default();
        for (i, hook) in self.on_user_message.iter().enumerate() {
            match hook(ctx.clone(), current_message.clone()).await {
                Ok(result) => {
                    if let Some(reason) = result.stop_reason {
                        aggregate.stop_reason = Some(reason);
                        aggregate.modified_message = Some(current_message);
                        return aggregate;
                    }
                    if let Some(modified) = result.modified_message {
                        current_message = modified.clone();
                        aggregate.modified_message = Some(modified);
                    }
                }
                Err(e) => {
                    tracing::info!("[hooks] on_user_message hook {} failed: {}", i, e);
                }
            }
        }
        if aggregate.modified_message.is_none() {
            aggregate.modified_message = Some(current_message);
        }
        aggregate
    }

    /// Fire all on_before_llm hooks sequentially.
    pub async fn fire_on_before_llm(&self, ctx: &HookContext, iteration: u32) {
        for (i, hook) in self.on_before_llm.iter().enumerate() {
            if let Err(e) = hook(ctx.clone(), iteration).await {
                tracing::info!("[hooks] on_before_llm hook {} failed: {}", i, e);
            }
        }
    }

    /// Fire all on_after_llm hooks sequentially.
    pub async fn fire_on_after_llm(
        &self,
        ctx: &HookContext,
        response_text: Option<String>,
    ) -> AfterLlmResult {
        let mut current_response = response_text;
        let mut aggregate = AfterLlmResult::default();
        for (i, hook) in self.on_after_llm.iter().enumerate() {
            match hook(ctx.clone(), current_response.clone()).await {
                Ok(result) => {
                    if let Some(reason) = result.block_reason {
                        aggregate.block_reason = Some(reason);
                        aggregate.replacement_text = current_response;
                        return aggregate;
                    }
                    if let Some(replacement) = result.replacement_text {
                        current_response = Some(replacement.clone());
                        aggregate.replacement_text = Some(replacement);
                    }
                }
                Err(e) => tracing::info!("[hooks] on_after_llm hook {} failed: {}", i, e),
            }
        }
        aggregate
    }

    /// Fire all on_before_tool hooks sequentially.
    ///
    /// If any hook returns `skip=true`, tool execution should be skipped
    /// and the `skip_reason` used as the tool result.
    pub async fn fire_on_before_tool(
        &self,
        ctx: &HookContext,
        tool_name: &str,
        arguments: &str,
    ) -> Option<BeforeToolResult> {
        let mut aggregate = BeforeToolResult::default();
        let mut current_arguments = arguments.to_string();
        for (i, hook) in self.on_before_tool.iter().enumerate() {
            match hook(
                ctx.clone(),
                tool_name.to_string(),
                current_arguments.clone(),
            )
            .await
            {
                Ok(result) if result.skip => {
                    let mut skip_result = result;
                    if skip_result.modified_arguments.is_none() && current_arguments != arguments {
                        skip_result.modified_arguments = Some(current_arguments);
                    }
                    return Some(skip_result);
                }
                Ok(result) => {
                    if let Some(modified_arguments) = result.modified_arguments {
                        current_arguments = modified_arguments.clone();
                        aggregate.modified_arguments = Some(modified_arguments);
                    }
                }
                Err(e) => {
                    tracing::info!("[hooks] on_before_tool hook {} failed: {}", i, e);
                }
            }
        }
        if aggregate.modified_arguments.is_some() {
            Some(aggregate)
        } else {
            None
        }
    }

    /// Fire all on_after_tool hooks sequentially.
    ///
    /// Returns aggregated injected context from all hooks that returned non-empty context.
    /// This context should be appended to the tool result message.
    pub async fn fire_on_after_tool(
        &self,
        ctx: &HookContext,
        tool_name: &str,
        success: bool,
        output_snippet: Option<String>,
    ) -> AfterToolResult {
        let mut aggregate = AfterToolResult::default();
        let mut current_output = output_snippet;
        let mut injected_parts: Vec<String> = Vec::new();
        for (i, hook) in self.on_after_tool.iter().enumerate() {
            match hook(
                ctx.clone(),
                tool_name.to_string(),
                success,
                current_output.clone(),
            )
            .await
            {
                Ok(result) => {
                    if let Some(reason) = result.block_reason {
                        aggregate.block_reason = Some(reason);
                        return aggregate;
                    }
                    if let Some(replacement) = result.replacement_output {
                        current_output = Some(replacement.clone());
                        aggregate.replacement_output = Some(replacement);
                    }
                    if let Some(ctx_text) = result.injected_context {
                        if !ctx_text.trim().is_empty() {
                            injected_parts.push(ctx_text);
                        }
                    }
                }
                Err(e) => {
                    tracing::info!("[hooks] on_after_tool hook {} failed: {}", i, e);
                }
            }
        }
        if !injected_parts.is_empty() {
            aggregate.injected_context = Some(injected_parts.join("\n"));
        }
        aggregate
    }

    /// Fire all on_session_end hooks sequentially.
    pub async fn fire_on_session_end(&self, ctx: &HookContext, summary: SessionSummary) {
        for (i, hook) in self.on_session_end.iter().enumerate() {
            if let Err(e) = hook(ctx.clone(), summary.clone()).await {
                tracing::info!("[hooks] on_session_end hook {} failed: {}", i, e);
            }
        }
    }

    /// Fire all on_compaction hooks sequentially.
    pub async fn fire_on_compaction(&self, ctx: &HookContext, compacted_snippets: Vec<String>) {
        for (i, hook) in self.on_compaction.iter().enumerate() {
            if let Err(e) = hook(ctx.clone(), compacted_snippets.clone()).await {
                tracing::info!("[hooks] on_compaction hook {} failed: {}", i, e);
            }
        }
    }

    pub async fn request_stop(&self, reason: String) {
        let mut stop = self.requested_stop.write().await;
        *stop = Some(reason);
    }

    pub async fn take_requested_stop(&self) -> Option<String> {
        let mut stop = self.requested_stop.write().await;
        stop.take()
    }
}

impl Default for AgenticHooks {
    fn default() -> Self {
        Self::new()
    }
}

/// Build the default set of lifecycle hooks with logging.
///
/// This creates a minimal hook set that logs lifecycle events.
/// Memory and skill integration hooks should be registered separately
/// when those services are available (via `register_memory_hooks` and
/// `register_skill_hooks`).
pub fn build_default_hooks() -> AgenticHooks {
    let mut hooks = AgenticHooks::new();

    // Session start: log session initialization
    hooks.register_on_session_start(Box::new(|ctx| {
        Box::pin(async move {
            tracing::info!(
                "[hooks] Session started: session={}, project={}, provider={}/{}",
                ctx.session_id,
                ctx.project_path.display(),
                ctx.provider_name,
                ctx.model_name,
            );
            Ok(())
        })
    }));

    // Session end: log session completion
    hooks.register_on_session_end(Box::new(|ctx, summary| {
        Box::pin(async move {
            tracing::info!(
                "[hooks] Session ended: session={}, turns={}, success={}, files_read={}, findings={}",
                ctx.session_id,
                summary.total_turns,
                summary.success,
                summary.files_read.len(),
                summary.key_findings.len(),
            );
            Ok(())
        })
    }));

    // Compaction: log compaction event
    hooks.register_on_compaction(Box::new(|ctx, snippets| {
        Box::pin(async move {
            tracing::info!(
                "[hooks] Context compaction: session={}, compacted_snippets={}",
                ctx.session_id,
                snippets.len(),
            );
            Ok(())
        })
    }));

    hooks
}

#[derive(Debug, Clone)]
enum HookDirective {
    Continue,
    Skip(String),
    ModifyArguments(String),
    Stop(String),
    InjectContext(String),
}

fn selected_skill_documents(
    index: &SkillIndex,
    selected: &[SkillMatch],
) -> Vec<crate::services::skills::model::SkillDocument> {
    let selected_ids = selected
        .iter()
        .map(|skill| skill.skill.id.as_str())
        .collect::<HashSet<_>>();
    index
        .skills()
        .iter()
        .filter(|doc| selected_ids.contains(doc.id.as_str()))
        .cloned()
        .collect()
}

fn tool_matches_rule(matcher: &str, tool_name: &str) -> bool {
    Regex::new(matcher)
        .map(|regex| regex.is_match(tool_name))
        .unwrap_or_else(|_| matcher.eq_ignore_ascii_case(tool_name))
}

fn parse_hook_directive(output: &str) -> HookDirective {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return HookDirective::Continue;
    }

    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
            let action = value
                .get("action")
                .and_then(|value| value.as_str())
                .unwrap_or("continue");
            return match action {
                "skip" | "block" => HookDirective::Skip(
                    value
                        .get("reason")
                        .and_then(|reason| reason.as_str())
                        .unwrap_or("Skipped by skill hook")
                        .to_string(),
                ),
                "modify_arguments" => {
                    let args = value
                        .get("arguments")
                        .cloned()
                        .unwrap_or(Value::Null)
                        .to_string();
                    HookDirective::ModifyArguments(args)
                }
                "stop" => HookDirective::Stop(
                    value
                        .get("reason")
                        .and_then(|reason| reason.as_str())
                        .unwrap_or("Stopped by skill hook")
                        .to_string(),
                ),
                "inject_context" => HookDirective::InjectContext(
                    value
                        .get("context")
                        .and_then(|context| context.as_str())
                        .unwrap_or(trimmed)
                        .to_string(),
                ),
                _ => HookDirective::Continue,
            };
        }
    }

    if let Some(rest) = trimmed.strip_prefix("skip:") {
        return HookDirective::Skip(rest.trim().to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("block:") {
        return HookDirective::Skip(rest.trim().to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("args:") {
        return HookDirective::ModifyArguments(rest.trim().to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("stop:") {
        return HookDirective::Stop(rest.trim().to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("context:") {
        return HookDirective::InjectContext(rest.trim().to_string());
    }

    HookDirective::InjectContext(trimmed.to_string())
}

async fn run_hook_command(
    ctx: &HookContext,
    skill_name: &str,
    phase: &str,
    command: &str,
    tool_name: Option<&str>,
    payload: Option<&str>,
) -> Result<String, String> {
    let mut cmd = Command::new("sh");
    cmd.arg("-lc").arg(command).current_dir(&ctx.project_path);
    cmd.env("PLAN_CASCADE_SESSION_ID", &ctx.session_id);
    cmd.env("PLAN_CASCADE_PROJECT_PATH", ctx.project_path.as_os_str());
    cmd.env("PLAN_CASCADE_PROVIDER", &ctx.provider_name);
    cmd.env("PLAN_CASCADE_MODEL", &ctx.model_name);
    cmd.env("PLAN_CASCADE_SKILL_NAME", skill_name);
    cmd.env("PLAN_CASCADE_HOOK_PHASE", phase);
    if let Some(tool_name) = tool_name {
        cmd.env("PLAN_CASCADE_TOOL_NAME", tool_name);
    }
    if let Some(payload) = payload {
        cmd.env("PLAN_CASCADE_HOOK_PAYLOAD", payload);
    }
    configure_background_process(&mut cmd);

    let output = cmd.output().await.map_err(|error| {
        format!(
            "failed to execute skill hook command '{}' for skill '{}': {}",
            command, skill_name, error
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!(
                "skill hook command '{}' exited with status {}",
                command, output.status
            )
        } else {
            stderr
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Register Skill-related hooks onto an AgenticHooks instance.
///
/// This wires the SkillIndex into the agentic lifecycle:
///
/// 1. **on_session_start**: Auto-detect applicable skills for the project
///    using the SkillIndex detection rules and store selected skills.
///
/// 2. **on_user_message**: Refine skill selection based on user message
///    content using lexical scoring.
///
/// The selected skills are stored in the `Arc<RwLock<Vec<SkillMatch>>>` shared
/// state, which can be read by the system prompt builder to inject skill content.
pub fn register_skill_hooks(
    hooks: &mut AgenticHooks,
    skill_index: Arc<RwLock<SkillIndex>>,
    policy: SelectionPolicy,
    selected_skills: Arc<RwLock<Vec<SkillMatch>>>,
) {
    // on_session_start: auto-detect applicable skills for the project
    let index_clone = skill_index.clone();
    let policy_clone = policy.clone();
    let skills_store = selected_skills.clone();
    hooks.register_on_session_start(Box::new(move |ctx| {
        let index = index_clone.clone();
        let policy = policy_clone.clone();
        let store = skills_store.clone();
        Box::pin(async move {
            let guard = index.read().await;
            let matches = select_skills_for_session(
                &guard,
                &ctx.project_path,
                "", // no user message yet at session start
                &InjectionPhase::Always,
                &policy,
            );
            let count = matches.len();
            let mut w = store.write().await;
            *w = matches;
            tracing::info!(
                "[hooks] Skill detection: session={}, detected_skills={}",
                ctx.session_id,
                count,
            );
            Ok(())
        })
    }));

    // on_user_message: refine skill selection based on message content
    let index_clone2 = skill_index.clone();
    let policy_clone2 = policy;
    let skills_store2 = selected_skills.clone();
    hooks.register_on_user_message(Box::new(move |ctx, msg| {
        let index = index_clone2.clone();
        let policy = policy_clone2.clone();
        let store = skills_store2.clone();
        Box::pin(async move {
            let guard = index.read().await;
            let matches = select_skills_for_session(
                &guard,
                &ctx.project_path,
                &msg,
                &InjectionPhase::Always,
                &policy,
            );
            if !matches.is_empty() {
                let mut w = store.write().await;
                *w = matches;
            }
            Ok(UserMessageHookResult::default())
        })
    }));

    // on_before_tool: execute selected skill pre-tool hooks, allowing argument rewrites or skips.
    let index_clone3 = skill_index.clone();
    let skills_store3 = selected_skills.clone();
    hooks.register_on_before_tool(Box::new(move |ctx, tool_name, arguments| {
        let index = index_clone3.clone();
        let store = skills_store3.clone();
        Box::pin(async move {
            let guard = index.read().await;
            let selected = store.read().await;
            let selected_docs = selected_skill_documents(&guard, selected.as_slice());

            let mut result = BeforeToolResult::default();
            for doc in selected_docs {
                let Some(skill_hooks) = &doc.hooks else {
                    continue;
                };
                for rule in &skill_hooks.pre_tool_use {
                    if !tool_matches_rule(&rule.matcher, &tool_name) {
                        continue;
                    }
                    for action in &rule.hooks {
                        let output = run_hook_command(
                            &ctx,
                            &doc.name,
                            "pre_tool_use",
                            &action.command,
                            Some(&tool_name),
                            Some(&arguments),
                        )
                        .await?;
                        match parse_hook_directive(&output) {
                            HookDirective::Continue => {}
                            HookDirective::ModifyArguments(updated) => {
                                result.modified_arguments = Some(updated);
                            }
                            HookDirective::Skip(reason) => {
                                result.skip = true;
                                result.skip_reason = Some(reason);
                                return Ok(result);
                            }
                            HookDirective::Stop(reason) => {
                                result.skip = true;
                                result.skip_reason =
                                    Some(format!("Stopped by skill hook: {}", reason));
                                return Ok(result);
                            }
                            HookDirective::InjectContext(context) => {
                                tracing::info!(
                                    "[hooks] pre_tool_use context note from skill '{}': {}",
                                    doc.name,
                                    context
                                );
                            }
                        }
                    }
                }
            }

            Ok(result)
        })
    }));

    // on_after_tool: execute selected skill post-tool hooks and append injected context.
    let index_clone4 = skill_index.clone();
    let skills_store4 = selected_skills.clone();
    hooks.register_on_after_tool(Box::new(move |ctx, tool_name, success, output_snippet| {
        let index = index_clone4.clone();
        let store = skills_store4.clone();
        Box::pin(async move {
            let guard = index.read().await;
            let selected = store.read().await;
            let selected_docs = selected_skill_documents(&guard, selected.as_slice());
            let mut injected_context = Vec::new();

            for doc in selected_docs {
                let Some(skill_hooks) = &doc.hooks else {
                    continue;
                };
                for rule in &skill_hooks.post_tool_use {
                    if !tool_matches_rule(&rule.matcher, &tool_name) {
                        continue;
                    }
                    for action in &rule.hooks {
                        let payload = serde_json::json!({
                            "tool_name": tool_name,
                            "success": success,
                            "output": output_snippet,
                        })
                        .to_string();
                        let output = run_hook_command(
                            &ctx,
                            &doc.name,
                            "post_tool_use",
                            &action.command,
                            Some(&tool_name),
                            Some(&payload),
                        )
                        .await?;
                        match parse_hook_directive(&output) {
                            HookDirective::Continue => {}
                            HookDirective::InjectContext(context) => injected_context.push(context),
                            HookDirective::Skip(reason) | HookDirective::Stop(reason) => {
                                injected_context.push(format!(
                                    "Skill '{}' requested follow-up stop: {}",
                                    doc.name, reason
                                ));
                            }
                            HookDirective::ModifyArguments(_) => {}
                        }
                    }
                }
            }

            if injected_context.is_empty() {
                Ok(AfterToolResult::default())
            } else {
                Ok(AfterToolResult::with_context(injected_context.join("\n")))
            }
        })
    }));

    // on_before_llm: allow selected skills to run stop guards before each reasoning turn.
    let index_clone5 = skill_index;
    let skills_store5 = selected_skills;
    let stop_requests = hooks.requested_stop.clone();
    hooks.register_on_before_llm(Box::new(move |ctx, _iteration| {
        let index = index_clone5.clone();
        let store = skills_store5.clone();
        let stop_requests = stop_requests.clone();
        Box::pin(async move {
            let guard = index.read().await;
            let selected = store.read().await;
            let selected_docs = selected_skill_documents(&guard, selected.as_slice());

            for doc in selected_docs {
                let Some(skill_hooks) = &doc.hooks else {
                    continue;
                };
                for action in &skill_hooks.stop {
                    let output =
                        run_hook_command(&ctx, &doc.name, "stop", &action.command, None, None)
                            .await?;
                    match parse_hook_directive(&output) {
                        HookDirective::Stop(reason) => {
                            let mut pending = stop_requests.write().await;
                            *pending = Some(reason);
                            return Ok(());
                        }
                        HookDirective::Skip(reason) => {
                            let mut pending = stop_requests.write().await;
                            *pending = Some(reason);
                            return Ok(());
                        }
                        HookDirective::Continue
                        | HookDirective::ModifyArguments(_)
                        | HookDirective::InjectContext(_) => {}
                    }
                }
            }

            Ok(())
        })
    }));
}

/// Configuration for memory injection behavior in lifecycle hooks.
#[derive(Debug, Clone)]
pub struct MemoryHookConfig {
    /// Whether memory entries should be injected into the prompt.
    pub injection_enabled: bool,
    /// Whether automatic extraction should run from lifecycle hooks.
    pub extraction_enabled: bool,
    /// Workflow root session id used by frontend status routing.
    pub root_session_id: Option<String>,
    /// Review mode for extracted memories.
    pub review_mode: Option<String>,
    /// Optional explicit LLM reviewer in `llm:provider:model` form.
    pub review_agent_ref: Option<String>,
    /// Provider config snapshot for automatic extraction / inherited review.
    pub extraction_provider_config: Option<crate::services::llm::types::ProviderConfig>,
    /// Optional base URL override for explicit review agent provider.
    pub review_base_url: Option<String>,
    /// App handle for emitting pipeline status events and resolving app state.
    pub app_handle: Option<tauri::AppHandle>,
    /// Allowed scope names: `project`, `global`, `session`.
    pub selected_scopes: Vec<String>,
    /// Allowed categories. Empty means all categories.
    pub selected_categories: Vec<MemoryCategory>,
    /// Explicit allowlist of memory ids. Empty means no id-level allowlist.
    pub selected_memory_ids: Vec<String>,
    /// Explicit denylist of memory ids to exclude.
    pub excluded_memory_ids: Vec<String>,
}

impl Default for MemoryHookConfig {
    fn default() -> Self {
        Self {
            injection_enabled: true,
            extraction_enabled: true,
            root_session_id: None,
            review_mode: None,
            review_agent_ref: None,
            extraction_provider_config: None,
            review_base_url: None,
            app_handle: None,
            selected_scopes: vec![],
            selected_categories: vec![],
            selected_memory_ids: vec![],
            excluded_memory_ids: vec![],
        }
    }
}

async fn load_memories_with_unified_query(
    store: &ProjectMemoryStore,
    project_path: &str,
    query: &str,
    session_id: &str,
    scopes: &HashSet<String>,
    categories: Option<&Vec<MemoryCategory>>,
    selected_ids: &HashSet<String>,
    excluded_ids: &HashSet<String>,
) -> Vec<crate::services::memory::store::MemoryEntry> {
    let mut parsed_scopes = Vec::new();
    if scopes.contains("project") {
        parsed_scopes.push(MemoryScopeV2::Project);
    }
    if scopes.contains("global") {
        parsed_scopes.push(MemoryScopeV2::Global);
    }
    if scopes.contains("session") && !session_id.trim().is_empty() {
        parsed_scopes.push(MemoryScopeV2::Session);
    }

    let mut include_ids: Vec<String> = selected_ids.iter().cloned().collect();
    include_ids.sort();
    let mut exclude_ids: Vec<String> = excluded_ids.iter().cloned().collect();
    exclude_ids.sort();

    let query_text = query.trim();
    let has_query = !query_text.is_empty();
    let tuning = memory_query_tuning_v2(MemoryQueryPresetV2::HookSessionStart);
    let request = UnifiedMemoryQueryRequestV2 {
        project_path: project_path.to_string(),
        query: if has_query {
            query_text.to_string()
        } else {
            String::new()
        },
        scopes: parsed_scopes,
        categories: categories.cloned().unwrap_or_default(),
        include_ids,
        exclude_ids,
        session_id: if session_id.trim().is_empty() {
            None
        } else {
            Some(session_id.to_string())
        },
        top_k_total: tuning.top_k_total,
        min_importance: tuning.min_importance,
        per_scope_budget: tuning.per_scope_budget,
        intent: crate::services::memory::retrieval::MemorySearchIntent::Default,
        enable_semantic: has_query,
        enable_lexical: true,
        statuses: vec![MemoryStatusV2::Active],
    };

    match query_memory_entries_unified_v2(store, &request).await {
        Ok(rows) => rows.results.into_iter().map(|row| row.entry).collect(),
        Err(e) => {
            tracing::warn!(
                "[hooks] Unified memory query failed: project={}, session={}, error={}",
                project_path,
                session_id,
                e
            );
            vec![]
        }
    }
}

/// Register Memory-related hooks onto an AgenticHooks instance.
///
/// This wires the ProjectMemoryStore into the agentic lifecycle:
///
/// 1. **on_session_start**: Load relevant memories (project + global) and store
///    them for system prompt injection.
///
/// 2. **on_session_end**: Extract new memories using LLM-driven extraction
///    (with rule-based fallback when no provider is available).
///
/// 3. **on_compaction**: Log-only (no memory writes — extraction is done at
///    session end via LLM).
///
/// The loaded memories are stored in the `Arc<RwLock<Vec<MemoryEntry>>>` shared
/// state, which can be read by the system prompt builder.
pub fn register_memory_hooks(
    hooks: &mut AgenticHooks,
    memory_store: Arc<ProjectMemoryStore>,
    loaded_memories: Arc<RwLock<Vec<crate::services::memory::store::MemoryEntry>>>,
    llm_provider: Option<Arc<dyn crate::services::llm::provider::LlmProvider>>,
) {
    register_memory_hooks_with_config(hooks, memory_store, loaded_memories, llm_provider, None);
}

/// Register memory-related hooks with optional injection filter configuration.
pub fn register_memory_hooks_with_config(
    hooks: &mut AgenticHooks,
    memory_store: Arc<ProjectMemoryStore>,
    loaded_memories: Arc<RwLock<Vec<crate::services::memory::store::MemoryEntry>>>,
    llm_provider: Option<Arc<dyn crate::services::llm::provider::LlmProvider>>,
    memory_hook_config: Option<MemoryHookConfig>,
) {
    let memory_hook_config = memory_hook_config.unwrap_or_default();
    let injection_enabled = memory_hook_config.injection_enabled;
    let extraction_enabled = memory_hook_config.extraction_enabled;
    let allowed_categories = if memory_hook_config.selected_categories.is_empty() {
        None
    } else {
        Some(memory_hook_config.selected_categories.clone())
    };
    let extraction_root_session_id = memory_hook_config.root_session_id.clone();
    let extraction_review_mode = memory_hook_config.review_mode.clone();
    let extraction_review_agent_ref = memory_hook_config.review_agent_ref.clone();
    let extraction_app_handle = memory_hook_config.app_handle.clone();
    let extraction_provider_config = memory_hook_config
        .extraction_provider_config
        .clone()
        .or_else(|| {
            llm_provider
                .as_ref()
                .map(|provider| provider.config().clone())
        });
    let extraction_review_base_url = memory_hook_config.review_base_url.clone();
    let selected_ids: HashSet<String> =
        memory_hook_config.selected_memory_ids.into_iter().collect();
    let excluded_ids: HashSet<String> =
        memory_hook_config.excluded_memory_ids.into_iter().collect();
    let mut scopes_set: HashSet<String> = memory_hook_config
        .selected_scopes
        .iter()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if scopes_set.is_empty() {
        scopes_set.insert("project".to_string());
        scopes_set.insert("global".to_string());
        scopes_set.insert("session".to_string());
    }

    // on_session_start: load project + global memories (browse mode, no query)
    let store_clone = memory_store.clone();
    let memories_out = loaded_memories.clone();
    let scopes_on_start = scopes_set.clone();
    let categories_on_start = allowed_categories.clone();
    let selected_ids_on_start = selected_ids.clone();
    let excluded_ids_on_start = excluded_ids.clone();
    hooks.register_on_session_start(Box::new(move |ctx| {
        let store = store_clone.clone();
        let out = memories_out.clone();
        let scopes = scopes_on_start.clone();
        let categories = categories_on_start.clone();
        let selected_ids = selected_ids_on_start.clone();
        let excluded_ids = excluded_ids_on_start.clone();
        Box::pin(async move {
            if !injection_enabled {
                let mut w = out.write().await;
                w.clear();
                tracing::info!(
                    "[hooks] Memory injection disabled: session={}",
                    ctx.session_id
                );
                return Ok(());
            }

            if let Err(e) = store.cleanup_expired_session_memories(14) {
                tracing::warn!(
                    "[hooks] Session-scope TTL cleanup failed: session={}, error={}",
                    ctx.session_id,
                    e
                );
            }

            let project_path = ctx.project_path.to_string_lossy().to_string();
            let filtered_entries = load_memories_with_unified_query(
                &store,
                &project_path,
                "",
                &ctx.session_id,
                &scopes,
                categories.as_ref(),
                &selected_ids,
                &excluded_ids,
            )
            .await;

            let count = filtered_entries.len();
            let mut w = out.write().await;
            *w = filtered_entries;
            tracing::info!(
                "[hooks] Memory loaded: session={}, memories={} (project+global+session)",
                ctx.session_id,
                count,
            );
            Ok(())
        })
    }));

    // on_user_message: refresh memory selection with semantic query
    let store_clone_msg = memory_store.clone();
    let memories_out_msg = loaded_memories.clone();
    let scopes_on_message = scopes_set.clone();
    let categories_on_message = allowed_categories.clone();
    let selected_ids_on_message = selected_ids.clone();
    let excluded_ids_on_message = excluded_ids.clone();
    hooks.register_on_user_message(Box::new(move |ctx, message| {
        let store = store_clone_msg.clone();
        let out = memories_out_msg.clone();
        let scopes = scopes_on_message.clone();
        let categories = categories_on_message.clone();
        let selected_ids = selected_ids_on_message.clone();
        let excluded_ids = excluded_ids_on_message.clone();
        Box::pin(async move {
            if !injection_enabled {
                let mut w = out.write().await;
                w.clear();
                return Ok(UserMessageHookResult::default());
            }
            let query = message.trim().to_string();
            if query.is_empty() {
                return Ok(UserMessageHookResult::default());
            }

            let project_path = ctx.project_path.to_string_lossy().to_string();
            let filtered_entries = load_memories_with_unified_query(
                &store,
                &project_path,
                &query,
                &ctx.session_id,
                &scopes,
                categories.as_ref(),
                &selected_ids,
                &excluded_ids,
            )
            .await;

            let count = filtered_entries.len();
            let mut w = out.write().await;
            *w = filtered_entries;
            tracing::info!(
                "[hooks] Memory refreshed from message: session={}, memories={}",
                ctx.session_id,
                count
            );
            Ok(UserMessageHookResult::default())
        })
    }));

    // on_session_end: LLM-driven memory extraction with rule-based fallback
    let store_clone2 = memory_store.clone();
    let provider_clone = llm_provider;
    hooks.register_on_session_end(Box::new(move |ctx, summary| {
        let store = store_clone2.clone();
        let provider = provider_clone.clone();
        let root_session_id = extraction_root_session_id.clone();
        let review_mode = extraction_review_mode.clone();
        let review_agent_ref = extraction_review_agent_ref.clone();
        let app_handle = extraction_app_handle.clone();
        let provider_config = extraction_provider_config.clone();
        let review_base_url = extraction_review_base_url.clone();
        Box::pin(async move {
            let project_path = ctx.project_path.to_string_lossy().to_string();
            if !extraction_enabled {
                maybe_generate_skill_from_session(provider.clone(), &store, &ctx, &summary, &project_path).await;
                return Ok(());
            }

            if !summary.success {
                tracing::info!(
                    "[hooks] Memory extraction skipped (source=orchestrator_hook, unsuccessful session): session={}",
                    ctx.session_id
                );
                return Ok(());
            }

            // Skip extraction for trivial sessions
            if summary.total_turns < 3 || summary.conversation_content.len() < 100 {
                tracing::info!(
                    "[hooks] Memory extraction skipped (source=orchestrator_hook, trivial session): session={}, turns={}, content_len={}",
                    ctx.session_id, summary.total_turns, summary.conversation_content.len(),
                );
                return Ok(());
            }

            let unified_extraction_enabled = std::env::var("UNIFIED_SESSION_EXTRACTION")
                .ok()
                .map(|value| {
                    matches!(
                        value.trim().to_ascii_lowercase().as_str(),
                        "1" | "true" | "yes" | "on"
                    )
                })
                .unwrap_or(true);
            if !unified_extraction_enabled {
                tracing::info!(
                    "[hooks] Memory extraction skipped: UNIFIED_SESSION_EXTRACTION disabled (source=orchestrator_hook, session={})",
                    ctx.session_id
                );
                maybe_generate_skill_from_session(provider.clone(), &store, &ctx, &summary, &project_path).await;
                return Ok(());
            }

            if let Some(app) = app_handle.as_ref() {
                if let Err(error) = crate::commands::memory::extract_session_memories_internal(
                    app,
                    project_path.clone(),
                    summary.task_description.clone(),
                    summary.conversation_content.clone(),
                    None,
                    Some(ctx.session_id.clone()),
                    root_session_id.clone().or_else(|| Some(ctx.session_id.clone())),
                    review_mode.clone(),
                    review_agent_ref.clone(),
                    provider_config.clone(),
                    review_base_url.clone(),
                )
                .await
                {
                    tracing::info!(
                        "[hooks] Unified memory extraction failed (source=orchestrator_hook): session={}, error={}",
                        ctx.session_id, error,
                    );
                }
            } else {
                tracing::warn!(
                    "[hooks] Memory extraction skipped: missing app handle (session={})",
                    ctx.session_id
                );
            }

            maybe_generate_skill_from_session(provider.clone(), &store, &ctx, &summary, &project_path).await;

            Ok(())
        })
    }));

    // on_compaction: log only (no memory writes)
    hooks.register_on_compaction(Box::new(move |ctx, snippets| {
        Box::pin(async move {
            tracing::info!(
                "[hooks] Context compaction: session={}, snippets={}",
                ctx.session_id,
                snippets.len(),
            );
            Ok(())
        })
    }));
}

/// Rule-based fallback memory extraction from key_findings.
/// Used when no LLM provider is available or when LLM extraction fails.
fn rule_based_memory_extraction(
    store: &ProjectMemoryStore,
    ctx: &HookContext,
    summary: &SessionSummary,
    project_path: &str,
    existing: &[crate::services::memory::store::MemoryEntry],
) {
    let conversation_summary = format!(
        "Task: {}. {} files read. {} key findings. {} tool calls across {} turns. Success: {}.",
        summary.task_description,
        summary.files_read.len(),
        summary.key_findings.len(),
        summary.tool_usage.values().sum::<usize>(),
        summary.total_turns,
        summary.success,
    );

    let mut new_memories = Vec::new();
    for finding in &summary.key_findings {
        if finding.trim().is_empty() {
            continue;
        }
        let already_exists = existing
            .iter()
            .any(|m| m.content.contains(finding.as_str()));
        if already_exists {
            continue;
        }
        new_memories.push(NewMemoryEntry {
            project_path: project_path.to_string(),
            category: MemoryCategory::Fact,
            content: finding.clone(),
            keywords: crate::services::memory::retrieval::extract_query_keywords(finding),
            importance: 0.5,
            source_session_id: Some(ctx.session_id.clone()),
            source_context: Some(format!("rule_extract:auto_v2; {}", conversation_summary)),
        });
    }

    let count = new_memories.len();
    if !new_memories.is_empty() {
        match store.add_memories(new_memories) {
            Ok(_) => {
                tracing::info!(
                    "[hooks] Rule-based memory extracted: session={}, new_memories={}",
                    ctx.session_id,
                    count,
                );
            }
            Err(e) => {
                tracing::info!(
                    "[hooks] Rule-based memory extraction failed: session={}, error={}",
                    ctx.session_id,
                    e,
                );
            }
        }
    }
}

/// P1: Optionally auto-generate a reusable skill from a successful, non-trivial session.
///
/// Generation criteria:
/// - Session must be successful
/// - Session must include at least 3 tool calls
/// - Conversation content should be substantial
/// - LLM provider must be available
///
/// Duplicate prevention:
/// - Reject if name collides (case-insensitive)
/// - Reject if token Jaccard similarity with existing generated skills >= 0.80
async fn maybe_generate_skill_from_session(
    provider: Option<Arc<dyn crate::services::llm::provider::LlmProvider>>,
    memory_store: &ProjectMemoryStore,
    ctx: &HookContext,
    summary: &SessionSummary,
    project_path: &str,
) {
    let total_tool_calls: usize = summary.tool_usage.values().sum();
    if !summary.success || total_tool_calls < 3 || summary.conversation_content.len() < 160 {
        return;
    }

    let Some(provider) = provider else {
        return;
    };

    use crate::services::llm::types::{
        LlmRequestOptions, Message, MessageContent as LlmMessageContent,
        MessageRole as LlmMessageRole,
    };

    let mut tool_usage_lines: Vec<String> = summary
        .tool_usage
        .iter()
        .map(|(name, count)| format!("- {}: {} calls", name, count))
        .collect();
    tool_usage_lines.sort();

    let findings_preview = summary
        .key_findings
        .iter()
        .take(12)
        .map(|f| format!("- {}", f))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "Generate ONE reusable coding skill from this successful session.\n\
         Return ONLY JSON with fields: name, description, tags, body.\n\
         Constraints:\n\
         - name: 3-8 words, imperative style.\n\
         - description: <= 120 chars.\n\
         - tags: 2-6 lowercase tags.\n\
         - body: markdown instructions with a heading and numbered steps.\n\
         - Avoid project-specific paths and private names.\n\
         - Make it broadly reusable for similar tasks.\n\n\
         ## Task\n{}\n\n\
         ## Tool Usage\n{}\n\n\
         ## Key Findings\n{}\n\n\
         ## Conversation Excerpt\n{}",
        summary.task_description,
        tool_usage_lines.join("\n"),
        if findings_preview.is_empty() {
            "- (none)".to_string()
        } else {
            findings_preview
        },
        summary
            .conversation_content
            .chars()
            .take(5000)
            .collect::<String>(),
    );

    let messages = vec![Message {
        role: LlmMessageRole::User,
        content: vec![LlmMessageContent::Text { text: prompt }],
    }];

    let opts = LlmRequestOptions {
        temperature_override: Some(0.2),
        ..Default::default()
    };

    let response = match tokio::time::timeout(
        std::time::Duration::from_secs(25),
        provider.send_message(messages, None, vec![], opts),
    )
    .await
    {
        Ok(Ok(resp)) => resp,
        Ok(Err(e)) => {
            tracing::info!(
                "[hooks] Skill generation skipped (LLM error): session={}, error={}",
                ctx.session_id,
                e
            );
            return;
        }
        Err(_) => {
            tracing::info!(
                "[hooks] Skill generation skipped (LLM timeout): session={}",
                ctx.session_id
            );
            return;
        }
    };

    let Some(response_text) = response.content else {
        return;
    };

    let Some(candidate) = parse_generated_skill_response(&response_text, &ctx.session_id) else {
        tracing::info!(
            "[hooks] Skill generation skipped (invalid payload): session={}",
            ctx.session_id
        );
        return;
    };

    let skill_store = SkillGeneratorStore::from_pool(memory_store.pool().clone());
    let existing = skill_store
        .list_generated_skills(project_path, true)
        .unwrap_or_default();

    let duplicate = existing.iter().any(|skill| {
        skill.name.eq_ignore_ascii_case(&candidate.name)
            || generated_skill_similarity(
                &candidate.name,
                &candidate.description,
                &candidate.body,
                &skill.name,
                &skill.description,
                &skill.body,
            ) >= 0.80
    });
    if duplicate {
        tracing::info!(
            "[hooks] Skill generation deduped: session={}, candidate={}",
            ctx.session_id,
            candidate.name
        );
        return;
    }

    match skill_store.save_generated_skill(project_path, &candidate) {
        Ok(saved) => {
            tracing::info!(
                "[hooks] Generated skill saved: session={}, skill_id={}, name={}",
                ctx.session_id,
                saved.id,
                saved.name
            );
        }
        Err(e) => {
            tracing::info!(
                "[hooks] Skill generation save failed: session={}, error={}",
                ctx.session_id,
                e
            );
        }
    }
}

fn parse_generated_skill_response(response_text: &str, session_id: &str) -> Option<GeneratedSkill> {
    let json_str = extract_first_json_object(response_text)?;
    let parsed: serde_json::Value = serde_json::from_str(&json_str).ok()?;

    let name = parsed.get("name")?.as_str()?.trim().to_string();
    let description = parsed
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let mut body = parsed
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if name.is_empty() || body.len() < 80 {
        return None;
    }

    if !body.starts_with('#') {
        body = format!("# {}\n\n{}", name, body);
    }

    let tags = parsed
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .take(8)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec!["generated".to_string()]);

    Some(GeneratedSkill {
        name,
        description: if description.is_empty() {
            "Auto-generated skill from successful session".to_string()
        } else {
            description
        },
        tags,
        body,
        source_session_ids: vec![session_id.to_string()],
    })
}

fn extract_first_json_object(text: &str) -> Option<String> {
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim().to_string());
        }
    }
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        let after_lang = if let Some(nl) = after.find('\n') {
            &after[nl + 1..]
        } else {
            after
        };
        if let Some(end) = after_lang.find("```") {
            let content = after_lang[..end].trim();
            if content.starts_with('{') {
                return Some(content.to_string());
            }
        }
    }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return Some(text[start..=end].to_string());
        }
    }
    None
}

fn generated_skill_similarity(
    name_a: &str,
    desc_a: &str,
    body_a: &str,
    name_b: &str,
    desc_b: &str,
    body_b: &str,
) -> f32 {
    let tokens_a = skill_tokens(&format!("{} {} {}", name_a, desc_a, body_a));
    let tokens_b = skill_tokens(&format!("{} {} {}", name_b, desc_b, body_b));
    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }
    let intersection = tokens_a.intersection(&tokens_b).count() as f32;
    let union = tokens_a.union(&tokens_b).count() as f32;
    if union <= f32::EPSILON {
        0.0
    } else {
        intersection / union
    }
}

fn skill_tokens(text: &str) -> HashSet<String> {
    crate::services::memory::retrieval::extract_query_keywords(text)
        .into_iter()
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    fn test_context() -> HookContext {
        HookContext {
            session_id: "test-session-001".to_string(),
            project_path: PathBuf::from("/tmp/test-project"),
            provider_name: "anthropic".to_string(),
            model_name: "claude-3-5-sonnet".to_string(),
        }
    }

    #[test]
    fn test_hook_context_fields() {
        let ctx = test_context();
        assert_eq!(ctx.session_id, "test-session-001");
        assert_eq!(ctx.project_path, PathBuf::from("/tmp/test-project"));
        assert_eq!(ctx.provider_name, "anthropic");
        assert_eq!(ctx.model_name, "claude-3-5-sonnet");
    }

    #[test]
    fn test_session_summary_fields() {
        let summary = SessionSummary {
            task_description: "Fix the bug".to_string(),
            files_read: vec!["src/main.rs".to_string()],
            key_findings: vec!["Found the issue".to_string()],
            tool_usage: {
                let mut m = HashMap::new();
                m.insert("Read".to_string(), 3);
                m.insert("Grep".to_string(), 1);
                m
            },
            total_turns: 5,
            success: true,
            conversation_content: String::new(),
        };
        assert_eq!(summary.task_description, "Fix the bug");
        assert_eq!(summary.files_read.len(), 1);
        assert_eq!(summary.total_turns, 5);
        assert!(summary.success);
    }

    #[test]
    fn test_before_tool_result_default() {
        let result = BeforeToolResult::default();
        assert!(!result.skip);
        assert!(result.skip_reason.is_none());
    }

    #[test]
    fn test_empty_hooks() {
        let hooks = AgenticHooks::new();
        assert!(hooks.is_empty());
        assert_eq!(hooks.total_hooks(), 0);
    }

    #[test]
    fn test_default_hooks_not_empty() {
        let hooks = build_default_hooks();
        assert!(!hooks.is_empty());
        // Default hooks register 3 hooks: session_start, session_end, compaction
        assert_eq!(hooks.total_hooks(), 3);
    }

    #[tokio::test]
    async fn test_fire_on_session_start() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let mut hooks = AgenticHooks::new();
        hooks.register_on_session_start(Box::new(move |_ctx| {
            let c = counter_clone.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }));

        let ctx = test_context();
        hooks.fire_on_session_start(&ctx).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_fire_on_session_start_error_does_not_panic() {
        let mut hooks = AgenticHooks::new();
        hooks.register_on_session_start(Box::new(|_ctx| {
            Box::pin(async move { Err("intentional error".to_string()) })
        }));

        let ctx = test_context();
        // Should not panic even with error
        hooks.fire_on_session_start(&ctx).await;
    }

    #[tokio::test]
    async fn test_fire_on_session_start_multiple_hooks_sequential() {
        let order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let order1 = order.clone();
        let order2 = order.clone();

        let mut hooks = AgenticHooks::new();
        hooks.register_on_session_start(Box::new(move |_ctx| {
            let o = order1.clone();
            Box::pin(async move {
                o.lock().unwrap().push(1);
                Ok(())
            })
        }));
        hooks.register_on_session_start(Box::new(move |_ctx| {
            let o = order2.clone();
            Box::pin(async move {
                o.lock().unwrap().push(2);
                Ok(())
            })
        }));

        let ctx = test_context();
        hooks.fire_on_session_start(&ctx).await;

        let result = order.lock().unwrap().clone();
        assert_eq!(
            result,
            vec![1, 2],
            "Hooks should execute in registration order"
        );
    }

    #[tokio::test]
    async fn test_fire_on_user_message_no_modification() {
        let mut hooks = AgenticHooks::new();
        hooks.register_on_user_message(Box::new(|_ctx, _msg| {
            Box::pin(async move { Ok(UserMessageHookResult::default()) })
        }));

        let ctx = test_context();
        let result = hooks.fire_on_user_message(&ctx, "hello".to_string()).await;
        assert_eq!(result.modified_message.as_deref(), Some("hello"));
        assert!(result.stop_reason.is_none());
    }

    #[tokio::test]
    async fn test_fire_on_user_message_with_modification() {
        let mut hooks = AgenticHooks::new();
        hooks.register_on_user_message(Box::new(|_ctx, msg| {
            Box::pin(async move {
                Ok(UserMessageHookResult {
                    modified_message: Some(format!("[enhanced] {}", msg)),
                    stop_reason: None,
                })
            })
        }));

        let ctx = test_context();
        let result = hooks.fire_on_user_message(&ctx, "hello".to_string()).await;
        assert_eq!(result.modified_message.as_deref(), Some("[enhanced] hello"));
    }

    #[tokio::test]
    async fn test_fire_on_user_message_chained_modifications() {
        let mut hooks = AgenticHooks::new();
        hooks.register_on_user_message(Box::new(|_ctx, msg| {
            Box::pin(async move {
                Ok(UserMessageHookResult {
                    modified_message: Some(format!("[hook1] {}", msg)),
                    stop_reason: None,
                })
            })
        }));
        hooks.register_on_user_message(Box::new(|_ctx, msg| {
            Box::pin(async move {
                Ok(UserMessageHookResult {
                    modified_message: Some(format!("[hook2] {}", msg)),
                    stop_reason: None,
                })
            })
        }));

        let ctx = test_context();
        let result = hooks.fire_on_user_message(&ctx, "hello".to_string()).await;
        assert_eq!(
            result.modified_message.as_deref(),
            Some("[hook2] [hook1] hello")
        );
    }

    #[tokio::test]
    async fn test_fire_on_before_tool_no_skip() {
        let mut hooks = AgenticHooks::new();
        hooks.register_on_before_tool(Box::new(|_ctx, _name, _args| {
            Box::pin(async move { Ok(BeforeToolResult::default()) })
        }));

        let ctx = test_context();
        let result = hooks
            .fire_on_before_tool(&ctx, "Read", r#"{"path": "/tmp/file"}"#)
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_fire_on_before_tool_with_skip() {
        let mut hooks = AgenticHooks::new();
        hooks.register_on_before_tool(Box::new(|_ctx, tool_name, _args| {
            Box::pin(async move {
                if tool_name == "Bash" {
                    Ok(BeforeToolResult {
                        skip: true,
                        skip_reason: Some("Bash disabled by policy".to_string()),
                        modified_arguments: None,
                    })
                } else {
                    Ok(BeforeToolResult::default())
                }
            })
        }));

        let ctx = test_context();

        // Non-blocked tool should pass
        let result = hooks.fire_on_before_tool(&ctx, "Read", "{}").await;
        assert!(result.is_none());

        // Blocked tool should be skipped
        let result = hooks.fire_on_before_tool(&ctx, "Bash", "{}").await;
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.skip);
        assert_eq!(result.skip_reason.unwrap(), "Bash disabled by policy");
    }

    #[tokio::test]
    async fn test_fire_on_after_tool() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let mut hooks = AgenticHooks::new();
        hooks.register_on_after_tool(Box::new(move |_ctx, _name, _success, _output| {
            let c = counter_clone.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(AfterToolResult::default())
            })
        }));

        let ctx = test_context();
        hooks
            .fire_on_after_tool(&ctx, "Read", true, Some("file content".to_string()))
            .await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_fire_on_before_llm() {
        let iteration_seen = Arc::new(AtomicU32::new(0));
        let iter_clone = iteration_seen.clone();

        let mut hooks = AgenticHooks::new();
        hooks.register_on_before_llm(Box::new(move |_ctx, iteration| {
            let i = iter_clone.clone();
            Box::pin(async move {
                i.store(iteration, Ordering::SeqCst);
                Ok(())
            })
        }));

        let ctx = test_context();
        hooks.fire_on_before_llm(&ctx, 5).await;
        assert_eq!(iteration_seen.load(Ordering::SeqCst), 5);
    }

    #[tokio::test]
    async fn test_fire_on_after_llm() {
        let received = Arc::new(std::sync::Mutex::new(None::<String>));
        let received_clone = received.clone();

        let mut hooks = AgenticHooks::new();
        hooks.register_on_after_llm(Box::new(move |_ctx, response| {
            let r = received_clone.clone();
            Box::pin(async move {
                *r.lock().unwrap() = response;
                Ok(())
            })
        }));

        let ctx = test_context();
        hooks
            .fire_on_after_llm(&ctx, Some("response text".to_string()))
            .await;
        assert_eq!(*received.lock().unwrap(), Some("response text".to_string()));
    }

    #[tokio::test]
    async fn test_fire_on_session_end() {
        let captured_turns = Arc::new(AtomicU32::new(0));
        let turns_clone = captured_turns.clone();

        let mut hooks = AgenticHooks::new();
        hooks.register_on_session_end(Box::new(move |_ctx, summary| {
            let t = turns_clone.clone();
            Box::pin(async move {
                t.store(summary.total_turns, Ordering::SeqCst);
                Ok(())
            })
        }));

        let ctx = test_context();
        let summary = SessionSummary {
            task_description: "test".to_string(),
            files_read: vec![],
            key_findings: vec![],
            tool_usage: HashMap::new(),
            total_turns: 42,
            success: true,
            conversation_content: String::new(),
        };
        hooks.fire_on_session_end(&ctx, summary).await;
        assert_eq!(captured_turns.load(Ordering::SeqCst), 42);
    }

    #[tokio::test]
    async fn test_fire_on_compaction() {
        let snippet_count = Arc::new(AtomicU32::new(0));
        let count_clone = snippet_count.clone();

        let mut hooks = AgenticHooks::new();
        hooks.register_on_compaction(Box::new(move |_ctx, snippets| {
            let c = count_clone.clone();
            Box::pin(async move {
                c.store(snippets.len() as u32, Ordering::SeqCst);
                Ok(())
            })
        }));

        let ctx = test_context();
        hooks
            .fire_on_compaction(&ctx, vec!["snippet1".to_string(), "snippet2".to_string()])
            .await;
        assert_eq!(snippet_count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_error_in_first_hook_does_not_block_second() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = counter.clone();

        let mut hooks = AgenticHooks::new();
        // First hook fails
        hooks.register_on_session_start(Box::new(|_ctx| {
            Box::pin(async move { Err("hook 1 failed".to_string()) })
        }));
        // Second hook should still execute
        hooks.register_on_session_start(Box::new(move |_ctx| {
            let c = counter_clone.clone();
            Box::pin(async move {
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }));

        let ctx = test_context();
        hooks.fire_on_session_start(&ctx).await;
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "Second hook should execute despite first hook's error"
        );
    }

    #[test]
    fn test_hooks_debug_format() {
        let hooks = build_default_hooks();
        let debug = format!("{:?}", hooks);
        assert!(debug.contains("AgenticHooks"));
        assert!(debug.contains("on_session_start: 1"));
    }

    #[test]
    fn test_total_hooks_count() {
        let mut hooks = AgenticHooks::new();
        assert_eq!(hooks.total_hooks(), 0);

        hooks.register_on_session_start(Box::new(|_ctx| Box::pin(async move { Ok(()) })));
        assert_eq!(hooks.total_hooks(), 1);

        hooks.register_on_before_llm(Box::new(|_ctx, _iter| Box::pin(async move { Ok(()) })));
        assert_eq!(hooks.total_hooks(), 2);
    }

    // ========================================================================
    // Story-001: register_skill_hooks tests
    // ========================================================================

    #[test]
    fn test_register_skill_hooks_adds_runtime_hooks() {
        let mut hooks = AgenticHooks::new();
        assert_eq!(hooks.total_hooks(), 0);

        let skill_index = Arc::new(RwLock::new(SkillIndex::new(vec![])));
        let policy = SelectionPolicy::default();
        let selected_skills = Arc::new(RwLock::new(Vec::new()));

        register_skill_hooks(&mut hooks, skill_index, policy, selected_skills);

        // session_start + user_message + before_tool + after_tool + before_llm
        assert_eq!(hooks.total_hooks(), 5);
    }

    #[test]
    fn test_register_skill_hooks_plus_defaults() {
        let mut hooks = build_default_hooks();
        assert_eq!(hooks.total_hooks(), 3); // default = 3

        let skill_index = Arc::new(RwLock::new(SkillIndex::new(vec![])));
        let policy = SelectionPolicy::default();
        let selected_skills = Arc::new(RwLock::new(Vec::new()));

        register_skill_hooks(&mut hooks, skill_index, policy, selected_skills);

        // defaults(3) + skill hooks(5) = 8
        assert_eq!(hooks.total_hooks(), 8);
    }

    #[tokio::test]
    async fn test_register_skill_hooks_session_start_fires() {
        let mut hooks = AgenticHooks::new();

        let skill_index = Arc::new(RwLock::new(SkillIndex::new(vec![])));
        let policy = SelectionPolicy::default();
        let selected_skills: Arc<RwLock<Vec<SkillMatch>>> = Arc::new(RwLock::new(Vec::new()));

        register_skill_hooks(&mut hooks, skill_index, policy, selected_skills.clone());

        let ctx = test_context();
        // Should not panic, and should set selected_skills (empty index -> empty result)
        hooks.fire_on_session_start(&ctx).await;

        let skills = selected_skills.read().await;
        // With empty index, no skills detected
        assert!(skills.is_empty());
    }

    #[tokio::test]
    async fn test_register_skill_hooks_user_message_fires() {
        let mut hooks = AgenticHooks::new();

        let skill_index = Arc::new(RwLock::new(SkillIndex::new(vec![])));
        let policy = SelectionPolicy::default();
        let selected_skills: Arc<RwLock<Vec<SkillMatch>>> = Arc::new(RwLock::new(Vec::new()));

        register_skill_hooks(&mut hooks, skill_index, policy, selected_skills.clone());

        let ctx = test_context();
        // user_message hook should not modify the message
        let result = hooks
            .fire_on_user_message(&ctx, "test message".to_string())
            .await;
        assert_eq!(
            result, "test message",
            "Skill hook should not modify the user message"
        );
    }

    // ========================================================================
    // Story-002: register_memory_hooks tests
    // ========================================================================

    fn create_test_memory_store() -> Arc<ProjectMemoryStore> {
        let db = crate::storage::database::Database::new_in_memory().unwrap();
        let embedding_service =
            Arc::new(crate::services::orchestrator::embedding_service::EmbeddingService::new());
        Arc::new(ProjectMemoryStore::from_database(&db, embedding_service))
    }

    #[test]
    fn test_register_memory_hooks_adds_four_hooks() {
        let mut hooks = AgenticHooks::new();
        assert_eq!(hooks.total_hooks(), 0);

        let store = create_test_memory_store();
        let loaded_memories = Arc::new(RwLock::new(Vec::new()));

        register_memory_hooks(&mut hooks, store, loaded_memories, None);

        // Should register 4 hooks: on_session_start + on_user_message + on_session_end + on_compaction
        assert_eq!(hooks.total_hooks(), 4);
    }

    #[test]
    fn test_register_memory_hooks_plus_defaults() {
        let mut hooks = build_default_hooks();
        assert_eq!(hooks.total_hooks(), 3);

        let store = create_test_memory_store();
        let loaded_memories = Arc::new(RwLock::new(Vec::new()));

        register_memory_hooks(&mut hooks, store, loaded_memories, None);

        // defaults(3) + memory hooks(4) = 7
        assert_eq!(hooks.total_hooks(), 7);
    }

    #[tokio::test]
    async fn test_register_memory_hooks_session_start_fires() {
        let mut hooks = AgenticHooks::new();

        let store = create_test_memory_store();
        let loaded_memories: Arc<RwLock<Vec<crate::services::memory::store::MemoryEntry>>> =
            Arc::new(RwLock::new(Vec::new()));

        register_memory_hooks(&mut hooks, store, loaded_memories.clone(), None);

        let ctx = test_context();
        // Should not panic, and should attempt to load memories (empty store -> empty result)
        hooks.fire_on_session_start(&ctx).await;

        let memories = loaded_memories.read().await;
        assert!(memories.is_empty(), "Empty store should yield no memories");
    }

    #[tokio::test]
    async fn test_register_memory_hooks_session_end_extracts_findings() {
        let mut hooks = AgenticHooks::new();

        let store = create_test_memory_store();
        let loaded_memories = Arc::new(RwLock::new(Vec::new()));

        // No LLM provider -> falls back to rule-based extraction
        register_memory_hooks(&mut hooks, store.clone(), loaded_memories, None);

        let ctx = test_context();
        // conversation_content must be >= 100 chars and total_turns >= 3 to pass threshold
        let conversation_content = "[User]: Fix the auth bug in src/auth.rs where tokens expire immediately. The JWT validation is being bypassed for admin routes.\n\n[Assistant]: I found two issues in the authentication module.".to_string();
        let summary = SessionSummary {
            task_description: "Fix authentication bug".to_string(),
            files_read: vec!["src/auth.rs".to_string()],
            key_findings: vec![
                "The token expiry was set to 0 instead of 3600".to_string(),
                "JWT validation was bypassed for admin routes".to_string(),
            ],
            tool_usage: HashMap::new(),
            total_turns: 5,
            success: true,
            conversation_content,
        };

        hooks.fire_on_session_end(&ctx, summary).await;

        // Verify memories were extracted from key_findings (rule-based fallback)
        let project_path = ctx.project_path.to_string_lossy().to_string();
        let memories = store
            .list_memories(&project_path, None, 0, 100)
            .unwrap_or_default();
        assert_eq!(
            memories.len(),
            2,
            "Should have extracted 2 memories from key_findings"
        );
    }

    #[tokio::test]
    async fn test_register_memory_hooks_session_end_skips_trivial() {
        let mut hooks = AgenticHooks::new();

        let store = create_test_memory_store();
        let loaded_memories = Arc::new(RwLock::new(Vec::new()));

        register_memory_hooks(&mut hooks, store.clone(), loaded_memories, None);

        let ctx = test_context();
        // Trivial session: only 2 turns and short content
        let summary = SessionSummary {
            task_description: "Quick question".to_string(),
            files_read: vec![],
            key_findings: vec!["Some finding".to_string()],
            tool_usage: HashMap::new(),
            total_turns: 2,
            success: true,
            conversation_content: "short".to_string(),
        };

        hooks.fire_on_session_end(&ctx, summary).await;

        // Should skip extraction for trivial sessions
        let project_path = ctx.project_path.to_string_lossy().to_string();
        let memories = store
            .list_memories(&project_path, None, 0, 100)
            .unwrap_or_default();
        assert_eq!(
            memories.len(),
            0,
            "Should skip memory extraction for trivial sessions"
        );
    }

    #[tokio::test]
    async fn test_register_memory_hooks_compaction_no_memory_writes() {
        let mut hooks = AgenticHooks::new();

        let store = create_test_memory_store();
        let loaded_memories = Arc::new(RwLock::new(Vec::new()));

        register_memory_hooks(&mut hooks, store.clone(), loaded_memories, None);

        let ctx = test_context();
        let snippets = vec![
            "The authentication module uses JWT tokens with RSA256 signing for stateless session management across microservices".to_string(),
            "short".to_string(),
        ];

        hooks.fire_on_compaction(&ctx, snippets).await;

        // Compaction should NOT write any memories (log-only)
        let project_path = ctx.project_path.to_string_lossy().to_string();
        let memories = store
            .list_memories(&project_path, None, 0, 100)
            .unwrap_or_default();
        assert_eq!(
            memories.len(),
            0,
            "Compaction hook should not write memories (log-only)"
        );
    }

    #[test]
    fn test_register_all_hooks_combined() {
        let mut hooks = build_default_hooks();
        assert_eq!(hooks.total_hooks(), 3);

        // Register skill hooks
        let skill_index = Arc::new(RwLock::new(SkillIndex::new(vec![])));
        let policy = SelectionPolicy::default();
        let selected_skills = Arc::new(RwLock::new(Vec::new()));
        register_skill_hooks(&mut hooks, skill_index, policy, selected_skills);

        // Register memory hooks
        let store = create_test_memory_store();
        let loaded_memories = Arc::new(RwLock::new(Vec::new()));
        register_memory_hooks(&mut hooks, store, loaded_memories, None);

        // defaults(3) + skill(2) + memory(4) = 9
        assert_eq!(hooks.total_hooks(), 9);
        assert!(!hooks.is_empty());
    }

    // ========================================================================
    // Story-005: End-to-end integration tests
    // ========================================================================

    /// Full lifecycle integration test:
    /// 1. Build hooks with defaults + skill + memory
    /// 2. Fire session_start -> skills detected, memories loaded
    /// 3. Fire user_message -> skill selection refined
    /// 4. Fire session_end -> memories extracted from findings
    /// 5. Fire compaction -> memories extracted from snippets
    /// 6. Verify shared state correctness at each step
    #[tokio::test]
    async fn test_full_session_lifecycle_wire_up() {
        // 1. Build hooks with all registrations
        let mut hooks = build_default_hooks();
        let initial_count = hooks.total_hooks();
        assert_eq!(initial_count, 3, "Default hooks should have 3");

        // Skill setup
        let skill_index = Arc::new(RwLock::new(SkillIndex::new(vec![])));
        let policy = SelectionPolicy::default();
        let selected_skills: Arc<RwLock<Vec<SkillMatch>>> = Arc::new(RwLock::new(Vec::new()));
        register_skill_hooks(&mut hooks, skill_index, policy, selected_skills.clone());

        // Memory setup
        let store = create_test_memory_store();
        let loaded_memories: Arc<RwLock<Vec<crate::services::memory::store::MemoryEntry>>> =
            Arc::new(RwLock::new(Vec::new()));
        register_memory_hooks(&mut hooks, store.clone(), loaded_memories.clone(), None);

        assert_eq!(
            hooks.total_hooks(),
            9,
            "defaults(3) + skill(2) + memory(4) = 9"
        );

        let ctx = test_context();

        // 2. Fire session_start -> skills detected, memories loaded
        hooks.fire_on_session_start(&ctx).await;
        {
            let skills = selected_skills.read().await;
            assert!(skills.is_empty(), "Empty index should yield no skills");
        }
        {
            let memories = loaded_memories.read().await;
            assert!(memories.is_empty(), "Empty store should yield no memories");
        }

        // 3. Fire user_message -> skill selection refined
        let msg = hooks
            .fire_on_user_message(&ctx, "implement the Rust module".to_string())
            .await;
        assert_eq!(
            msg, "implement the Rust module",
            "Message should not be modified"
        );

        // 4. Fire session_end -> memories extracted (rule-based, no LLM provider)
        let conversation_content = "[User]: Implement the authentication module with JWT RS256 signing and proper session management with 3600 second expiry.\n\n[Assistant]: I've implemented the auth module. Found that tokens use RS256 and sessions expire after 3600s.".to_string();
        let summary = SessionSummary {
            task_description: "Implement authentication module".to_string(),
            files_read: vec!["src/auth.rs".to_string(), "src/main.rs".to_string()],
            key_findings: vec![
                "The auth module uses JWT with RS256 signing".to_string(),
                "Session tokens expire after 3600 seconds".to_string(),
            ],
            tool_usage: {
                let mut m = HashMap::new();
                m.insert("Read".to_string(), 5);
                m.insert("Edit".to_string(), 3);
                m
            },
            total_turns: 8,
            success: true,
            conversation_content,
        };
        hooks.fire_on_session_end(&ctx, summary).await;

        // Verify memories were stored (rule-based extraction from key_findings)
        let project_path = ctx.project_path.to_string_lossy().to_string();
        let stored = store
            .list_memories(&project_path, None, 0, 100)
            .unwrap_or_default();
        assert_eq!(
            stored.len(),
            2,
            "Should have stored 2 memories from key_findings"
        );

        // 5. Fire compaction -> log-only, no memory writes
        let snippets = vec![
            "The database schema uses foreign key constraints with CASCADE delete for referential integrity across all entity tables".to_string(),
        ];
        hooks.fire_on_compaction(&ctx, snippets).await;

        let stored_after = store
            .list_memories(&project_path, None, 0, 100)
            .unwrap_or_default();
        assert_eq!(
            stored_after.len(),
            2,
            "Compaction should not add memories (log-only), still 2 from session_end"
        );
    }

    /// Verify that sub-agent hooks remain empty per design.
    /// Sub-agents should NOT inherit parent hooks because they have
    /// independent context windows.
    #[test]
    fn test_sub_agent_hooks_remain_empty() {
        // Sub-agents are constructed with AgenticHooks::new() (empty)
        let sub_agent_hooks = AgenticHooks::new();
        assert!(sub_agent_hooks.is_empty());
        assert_eq!(sub_agent_hooks.total_hooks(), 0);
    }

    /// Verify that the full hook set produces the expected hook distribution.
    #[test]
    fn test_hook_distribution_after_full_registration() {
        let mut hooks = build_default_hooks();

        let skill_index = Arc::new(RwLock::new(SkillIndex::new(vec![])));
        let policy = SelectionPolicy::default();
        let selected_skills = Arc::new(RwLock::new(Vec::new()));
        register_skill_hooks(&mut hooks, skill_index, policy, selected_skills);

        let store = create_test_memory_store();
        let loaded_memories = Arc::new(RwLock::new(Vec::new()));
        register_memory_hooks(&mut hooks, store, loaded_memories, None);

        // Expected distribution:
        // on_session_start: 1 (default) + 1 (skill) + 1 (memory) = 3
        // on_user_message: 1 (skill) + 1 (memory) = 2
        // on_session_end: 1 (default) + 1 (memory) = 2
        // on_compaction: 1 (default) + 1 (memory) = 2
        // Total: 3 + 2 + 2 + 2 = 9
        assert_eq!(hooks.total_hooks(), 9);

        // Verify the debug format reflects the distribution
        let debug = format!("{:?}", hooks);
        assert!(debug.contains("on_session_start: 3"));
        assert!(debug.contains("on_user_message: 2"));
        assert!(debug.contains("on_session_end: 2"));
        assert!(debug.contains("on_compaction: 2"));
    }

    #[test]
    fn test_extract_first_json_object_prefers_fenced_json() {
        let text = r#"prefix
```json
{"name":"Skill A","description":"desc","tags":["rust"],"body":"body"}
```
suffix {"name":"Skill B"}"#;

        let extracted = extract_first_json_object(text).expect("json should be extracted");
        assert_eq!(
            extracted,
            r#"{"name":"Skill A","description":"desc","tags":["rust"],"body":"body"}"#
        );
    }

    #[test]
    fn test_parse_generated_skill_response_normalizes_body_and_tags() {
        let payload = r#"```json
{
  "name": "Refactor Error Handling",
  "description": "Consolidate error mapping flow",
  "tags": ["Rust", "  Errors  ", "Refactor"],
  "body": "1. Audit all error call sites and group them by domain boundaries.\n2. Introduce a shared error enum with conversion helpers and typed context.\n3. Update handlers and add regression tests for each failure branch."
}
```"#;

        let parsed = parse_generated_skill_response(payload, "session-abc")
            .expect("valid payload should parse");
        assert_eq!(parsed.name, "Refactor Error Handling");
        assert_eq!(parsed.description, "Consolidate error mapping flow");
        assert!(parsed.body.starts_with("# Refactor Error Handling\n\n"));
        assert_eq!(
            parsed.tags,
            vec![
                "rust".to_string(),
                "errors".to_string(),
                "refactor".to_string()
            ]
        );
        assert_eq!(parsed.source_session_ids, vec!["session-abc".to_string()]);
    }

    #[test]
    fn test_parse_generated_skill_response_rejects_short_body() {
        let payload =
            r#"{"name":"Tiny Skill","description":"desc","tags":["test"],"body":"too short"}"#;
        let parsed = parse_generated_skill_response(payload, "session-abc");
        assert!(parsed.is_none());
    }

    #[test]
    fn test_generated_skill_similarity_identity_and_distance() {
        let identical = generated_skill_similarity(
            "Refactor HTTP Client",
            "Normalize retries and timeout handling",
            "# Refactor HTTP Client\n\n1. Add retry policy\n2. Add timeout policy\n3. Add tests",
            "Refactor HTTP Client",
            "Normalize retries and timeout handling",
            "# Refactor HTTP Client\n\n1. Add retry policy\n2. Add timeout policy\n3. Add tests",
        );
        assert!(identical > 0.99);

        let distant = generated_skill_similarity(
            "Tune SQL Indexes",
            "Optimize query latency",
            "# Tune SQL Indexes\n\n1. Profile slow queries\n2. Add composite indexes",
            "Design API Contract",
            "Define endpoint response models",
            "# Design API Contract\n\n1. Define schemas\n2. Align error codes",
        );
        assert!(distant < 0.60);
    }
}
