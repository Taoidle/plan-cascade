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

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::services::memory::retrieval::search_memories;
use crate::services::memory::store::ProjectMemoryStore;
use crate::services::memory::store::{MemoryCategory, MemorySearchRequest, NewMemoryEntry};
use crate::services::skills::model::{InjectionPhase, SelectionPolicy, SkillIndex, SkillMatch};
use crate::services::skills::select::select_skills_for_session;

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
}

/// Result of an on_before_tool hook: can request skipping the tool call.
#[derive(Debug, Clone)]
pub struct BeforeToolResult {
    /// If true, skip this tool execution and use `skip_reason` as the tool result
    pub skip: bool,
    /// Reason for skipping (injected as tool result if skip=true)
    pub skip_reason: Option<String>,
}

impl Default for BeforeToolResult {
    fn default() -> Self {
        Self {
            skip: false,
            skip_reason: None,
        }
    }
}

/// Result of an on_after_tool hook: can optionally inject context into the conversation.
#[derive(Debug, Clone, Default)]
pub struct AfterToolResult {
    /// If present, this context is appended to the tool result message so the LLM sees it.
    pub injected_context: Option<String>,
}

impl AfterToolResult {
    /// Create a result with injected context.
    pub fn with_context(context: String) -> Self {
        Self {
            injected_context: Some(context),
        }
    }
}

// ============================================================================
// Type Aliases for Hook Callbacks
// ============================================================================

/// Hook fired at session start. Receives session context.
pub type OnSessionStartHook = Box<
    dyn Fn(HookContext) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync,
>;

/// Hook fired on user message. Returns Option<modified_message>.
/// If None, the original message is used unchanged.
pub type OnUserMessageHook = Box<
    dyn Fn(
            HookContext,
            String,
        ) -> Pin<Box<dyn Future<Output = Result<Option<String>, String>> + Send>>
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
    dyn Fn(HookContext, Option<String>) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>>
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
/// Errors from individual hooks are logged via `eprintln!` but do not prevent
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
                eprintln!("[hooks] on_session_start hook {} failed: {}", i, e);
            }
        }
    }

    /// Fire all on_user_message hooks sequentially.
    ///
    /// Each hook can optionally modify the message. If a hook returns
    /// `Ok(Some(modified))`, the modified message is passed to subsequent
    /// hooks and ultimately used in place of the original.
    pub async fn fire_on_user_message(&self, ctx: &HookContext, message: String) -> String {
        let mut current_message = message;
        for (i, hook) in self.on_user_message.iter().enumerate() {
            match hook(ctx.clone(), current_message.clone()).await {
                Ok(Some(modified)) => {
                    current_message = modified;
                }
                Ok(None) => {
                    // No modification
                }
                Err(e) => {
                    eprintln!("[hooks] on_user_message hook {} failed: {}", i, e);
                }
            }
        }
        current_message
    }

    /// Fire all on_before_llm hooks sequentially.
    pub async fn fire_on_before_llm(&self, ctx: &HookContext, iteration: u32) {
        for (i, hook) in self.on_before_llm.iter().enumerate() {
            if let Err(e) = hook(ctx.clone(), iteration).await {
                eprintln!("[hooks] on_before_llm hook {} failed: {}", i, e);
            }
        }
    }

    /// Fire all on_after_llm hooks sequentially.
    pub async fn fire_on_after_llm(&self, ctx: &HookContext, response_text: Option<String>) {
        for (i, hook) in self.on_after_llm.iter().enumerate() {
            if let Err(e) = hook(ctx.clone(), response_text.clone()).await {
                eprintln!("[hooks] on_after_llm hook {} failed: {}", i, e);
            }
        }
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
        for (i, hook) in self.on_before_tool.iter().enumerate() {
            match hook(ctx.clone(), tool_name.to_string(), arguments.to_string()).await {
                Ok(result) if result.skip => {
                    return Some(result);
                }
                Ok(_) => {
                    // No skip requested
                }
                Err(e) => {
                    eprintln!("[hooks] on_before_tool hook {} failed: {}", i, e);
                }
            }
        }
        None
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
    ) -> Option<String> {
        let mut injected_parts: Vec<String> = Vec::new();
        for (i, hook) in self.on_after_tool.iter().enumerate() {
            match hook(
                ctx.clone(),
                tool_name.to_string(),
                success,
                output_snippet.clone(),
            )
            .await
            {
                Ok(result) => {
                    if let Some(ctx_text) = result.injected_context {
                        if !ctx_text.trim().is_empty() {
                            injected_parts.push(ctx_text);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[hooks] on_after_tool hook {} failed: {}", i, e);
                }
            }
        }
        if injected_parts.is_empty() {
            None
        } else {
            Some(injected_parts.join("\n"))
        }
    }

    /// Fire all on_session_end hooks sequentially.
    pub async fn fire_on_session_end(&self, ctx: &HookContext, summary: SessionSummary) {
        for (i, hook) in self.on_session_end.iter().enumerate() {
            if let Err(e) = hook(ctx.clone(), summary.clone()).await {
                eprintln!("[hooks] on_session_end hook {} failed: {}", i, e);
            }
        }
    }

    /// Fire all on_compaction hooks sequentially.
    pub async fn fire_on_compaction(&self, ctx: &HookContext, compacted_snippets: Vec<String>) {
        for (i, hook) in self.on_compaction.iter().enumerate() {
            if let Err(e) = hook(ctx.clone(), compacted_snippets.clone()).await {
                eprintln!("[hooks] on_compaction hook {} failed: {}", i, e);
            }
        }
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
            eprintln!(
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
            eprintln!(
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
            eprintln!(
                "[hooks] Context compaction: session={}, compacted_snippets={}",
                ctx.session_id,
                snippets.len(),
            );
            Ok(())
        })
    }));

    hooks
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
            eprintln!(
                "[hooks] Skill detection: session={}, detected_skills={}",
                ctx.session_id, count,
            );
            Ok(())
        })
    }));

    // on_user_message: refine skill selection based on message content
    let index_clone2 = skill_index.clone();
    let policy_clone2 = policy;
    let skills_store2 = selected_skills;
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
            Ok(None) // do not modify the message
        })
    }));
}

/// Register Memory-related hooks onto an AgenticHooks instance.
///
/// This wires the ProjectMemoryStore into the agentic lifecycle:
///
/// 1. **on_session_start**: Load relevant memories for the project and store
///    them for system prompt injection.
///
/// 2. **on_session_end**: Extract new memories from the session summary
///    and persist them using the MemoryExtractor heuristic approach.
///
/// 3. **on_compaction**: Extract key information from compacted content
///    snippets and store them as memories for future sessions.
///
/// The loaded memories are stored in the `Arc<RwLock<Vec<MemoryEntry>>>` shared
/// state, which can be read by the system prompt builder.
pub fn register_memory_hooks(
    hooks: &mut AgenticHooks,
    memory_store: Arc<ProjectMemoryStore>,
    loaded_memories: Arc<RwLock<Vec<crate::services::memory::store::MemoryEntry>>>,
) {
    // on_session_start: load relevant memories for this project
    let store_clone = memory_store.clone();
    let memories_out = loaded_memories.clone();
    hooks.register_on_session_start(Box::new(move |ctx| {
        let store = store_clone.clone();
        let out = memories_out.clone();
        Box::pin(async move {
            let project_path = ctx.project_path.to_string_lossy().to_string();
            let request = MemorySearchRequest {
                project_path,
                query: String::new(), // empty query = load by importance
                categories: None,
                top_k: 10,
                min_importance: 0.1,
            };
            match search_memories(&store, &request) {
                Ok(results) => {
                    let count = results.len();
                    let entries: Vec<_> = results.into_iter().map(|r| r.entry).collect();
                    let mut w = out.write().await;
                    *w = entries;
                    eprintln!(
                        "[hooks] Memory loaded: session={}, memories={}",
                        ctx.session_id, count,
                    );
                }
                Err(e) => {
                    eprintln!(
                        "[hooks] Memory load failed: session={}, error={}",
                        ctx.session_id, e,
                    );
                }
            }
            Ok(())
        })
    }));

    // on_session_end: extract new memories from the session summary
    let store_clone2 = memory_store.clone();
    hooks.register_on_session_end(Box::new(move |ctx, summary| {
        let store = store_clone2.clone();
        Box::pin(async move {
            let project_path = ctx.project_path.to_string_lossy().to_string();

            // Build a simple conversation summary from the available data
            let conversation_summary = format!(
                "Task: {}. {} files read. {} key findings. {} tool calls across {} turns. Success: {}.",
                summary.task_description,
                summary.files_read.len(),
                summary.key_findings.len(),
                summary.tool_usage.values().sum::<usize>(),
                summary.total_turns,
                summary.success,
            );

            // Load existing memories to avoid duplicates
            let existing = store
                .list_memories(&project_path, None, 0, 100)
                .unwrap_or_default();

            // Use rule-based extraction from key_findings
            // (LLM-based extraction requires an LLM call which is not available here)
            let mut new_memories = Vec::new();
            for finding in &summary.key_findings {
                if finding.trim().is_empty() {
                    continue;
                }
                // Skip if similar content already exists
                let already_exists = existing.iter().any(|m| m.content.contains(finding.as_str()));
                if already_exists {
                    continue;
                }
                new_memories.push(NewMemoryEntry {
                    project_path: project_path.clone(),
                    category: MemoryCategory::Fact,
                    content: finding.clone(),
                    keywords: crate::services::memory::retrieval::extract_query_keywords(finding),
                    importance: 0.5,
                    source_session_id: Some(ctx.session_id.clone()),
                    source_context: Some(conversation_summary.clone()),
                });
            }

            let count = new_memories.len();
            if !new_memories.is_empty() {
                match store.add_memories(new_memories) {
                    Ok(_) => {
                        eprintln!(
                            "[hooks] Memory extracted: session={}, new_memories={}",
                            ctx.session_id, count,
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "[hooks] Memory extraction failed: session={}, error={}",
                            ctx.session_id, e,
                        );
                    }
                }
            }
            Ok(())
        })
    }));

    // on_compaction: extract key information from compacted content
    let store_clone3 = memory_store;
    hooks.register_on_compaction(Box::new(move |ctx, snippets| {
        let store = store_clone3.clone();
        Box::pin(async move {
            let project_path = ctx.project_path.to_string_lossy().to_string();

            // Extract potential memories from compacted snippets
            let mut new_memories = Vec::new();
            for snippet in &snippets {
                // Only extract from substantial snippets
                if snippet.len() < 50 {
                    continue;
                }
                // Limit each snippet contribution
                let content = if snippet.len() > 200 {
                    format!("{}...", &snippet[..200])
                } else {
                    snippet.clone()
                };
                new_memories.push(NewMemoryEntry {
                    project_path: project_path.clone(),
                    category: MemoryCategory::Fact,
                    content,
                    keywords: crate::services::memory::retrieval::extract_query_keywords(snippet),
                    importance: 0.3, // lower importance for compacted content
                    source_session_id: Some(ctx.session_id.clone()),
                    source_context: Some("compaction".to_string()),
                });
            }

            let count = new_memories.len();
            if !new_memories.is_empty() {
                // Use upsert to avoid duplicates
                for entry in new_memories {
                    if let Err(e) = store.upsert_memory(entry) {
                        eprintln!(
                            "[hooks] Compaction memory upsert failed: session={}, error={}",
                            ctx.session_id, e,
                        );
                    }
                }
                eprintln!(
                    "[hooks] Compaction memory extracted: session={}, candidates={}",
                    ctx.session_id, count,
                );
            }
            Ok(())
        })
    }));
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
        hooks.register_on_user_message(Box::new(|_ctx, _msg| Box::pin(async move { Ok(None) })));

        let ctx = test_context();
        let result = hooks.fire_on_user_message(&ctx, "hello".to_string()).await;
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_fire_on_user_message_with_modification() {
        let mut hooks = AgenticHooks::new();
        hooks.register_on_user_message(Box::new(|_ctx, msg| {
            Box::pin(async move { Ok(Some(format!("[enhanced] {}", msg))) })
        }));

        let ctx = test_context();
        let result = hooks.fire_on_user_message(&ctx, "hello".to_string()).await;
        assert_eq!(result, "[enhanced] hello");
    }

    #[tokio::test]
    async fn test_fire_on_user_message_chained_modifications() {
        let mut hooks = AgenticHooks::new();
        hooks.register_on_user_message(Box::new(|_ctx, msg| {
            Box::pin(async move { Ok(Some(format!("[hook1] {}", msg))) })
        }));
        hooks.register_on_user_message(Box::new(|_ctx, msg| {
            Box::pin(async move { Ok(Some(format!("[hook2] {}", msg))) })
        }));

        let ctx = test_context();
        let result = hooks.fire_on_user_message(&ctx, "hello".to_string()).await;
        assert_eq!(result, "[hook2] [hook1] hello");
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
                Ok(())
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
    fn test_register_skill_hooks_adds_two_hooks() {
        let mut hooks = AgenticHooks::new();
        assert_eq!(hooks.total_hooks(), 0);

        let skill_index = Arc::new(RwLock::new(SkillIndex::new(vec![])));
        let policy = SelectionPolicy::default();
        let selected_skills = Arc::new(RwLock::new(Vec::new()));

        register_skill_hooks(&mut hooks, skill_index, policy, selected_skills);

        // Should register 2 hooks: on_session_start + on_user_message
        assert_eq!(hooks.total_hooks(), 2);
    }

    #[test]
    fn test_register_skill_hooks_plus_defaults() {
        let mut hooks = build_default_hooks();
        assert_eq!(hooks.total_hooks(), 3); // default = 3

        let skill_index = Arc::new(RwLock::new(SkillIndex::new(vec![])));
        let policy = SelectionPolicy::default();
        let selected_skills = Arc::new(RwLock::new(Vec::new()));

        register_skill_hooks(&mut hooks, skill_index, policy, selected_skills);

        // defaults(3) + skill hooks(2) = 5
        assert_eq!(hooks.total_hooks(), 5);
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
    fn test_register_memory_hooks_adds_three_hooks() {
        let mut hooks = AgenticHooks::new();
        assert_eq!(hooks.total_hooks(), 0);

        let store = create_test_memory_store();
        let loaded_memories = Arc::new(RwLock::new(Vec::new()));

        register_memory_hooks(&mut hooks, store, loaded_memories);

        // Should register 3 hooks: on_session_start + on_session_end + on_compaction
        assert_eq!(hooks.total_hooks(), 3);
    }

    #[test]
    fn test_register_memory_hooks_plus_defaults() {
        let mut hooks = build_default_hooks();
        assert_eq!(hooks.total_hooks(), 3);

        let store = create_test_memory_store();
        let loaded_memories = Arc::new(RwLock::new(Vec::new()));

        register_memory_hooks(&mut hooks, store, loaded_memories);

        // defaults(3) + memory hooks(3) = 6
        assert_eq!(hooks.total_hooks(), 6);
    }

    #[tokio::test]
    async fn test_register_memory_hooks_session_start_fires() {
        let mut hooks = AgenticHooks::new();

        let store = create_test_memory_store();
        let loaded_memories: Arc<RwLock<Vec<crate::services::memory::store::MemoryEntry>>> =
            Arc::new(RwLock::new(Vec::new()));

        register_memory_hooks(&mut hooks, store, loaded_memories.clone());

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

        register_memory_hooks(&mut hooks, store.clone(), loaded_memories);

        let ctx = test_context();
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
        };

        hooks.fire_on_session_end(&ctx, summary).await;

        // Verify memories were extracted from key_findings
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
    async fn test_register_memory_hooks_compaction_fires() {
        let mut hooks = AgenticHooks::new();

        let store = create_test_memory_store();
        let loaded_memories = Arc::new(RwLock::new(Vec::new()));

        register_memory_hooks(&mut hooks, store.clone(), loaded_memories);

        let ctx = test_context();
        let snippets = vec![
            "The authentication module uses JWT tokens with RSA256 signing for stateless session management across microservices".to_string(),
            "short".to_string(), // too short, should be skipped
        ];

        hooks.fire_on_compaction(&ctx, snippets).await;

        // Only the substantial snippet should be stored
        let project_path = ctx.project_path.to_string_lossy().to_string();
        let memories = store
            .list_memories(&project_path, None, 0, 100)
            .unwrap_or_default();
        assert_eq!(
            memories.len(),
            1,
            "Should have extracted 1 memory from substantial snippet, skipping short one"
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
        register_memory_hooks(&mut hooks, store, loaded_memories);

        // defaults(3) + skill(2) + memory(3) = 8
        assert_eq!(hooks.total_hooks(), 8);
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
        register_memory_hooks(&mut hooks, store.clone(), loaded_memories.clone());

        assert_eq!(
            hooks.total_hooks(),
            8,
            "defaults(3) + skill(2) + memory(3) = 8"
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

        // 4. Fire session_end -> memories extracted
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
        };
        hooks.fire_on_session_end(&ctx, summary).await;

        // Verify memories were stored
        let project_path = ctx.project_path.to_string_lossy().to_string();
        let stored = store
            .list_memories(&project_path, None, 0, 100)
            .unwrap_or_default();
        assert_eq!(
            stored.len(),
            2,
            "Should have stored 2 memories from key_findings"
        );

        // 5. Fire compaction -> more memories
        let snippets = vec![
            "The database schema uses foreign key constraints with CASCADE delete for referential integrity across all entity tables".to_string(),
        ];
        hooks.fire_on_compaction(&ctx, snippets).await;

        let stored_after = store
            .list_memories(&project_path, None, 0, 100)
            .unwrap_or_default();
        assert_eq!(
            stored_after.len(),
            3,
            "Should now have 3 memories (2 from findings + 1 from compaction)"
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
        register_memory_hooks(&mut hooks, store, loaded_memories);

        // Expected distribution:
        // on_session_start: 1 (default) + 1 (skill) + 1 (memory) = 3
        // on_user_message: 1 (skill) = 1
        // on_session_end: 1 (default) + 1 (memory) = 2
        // on_compaction: 1 (default) + 1 (memory) = 2
        // Total: 3 + 1 + 2 + 2 = 8
        assert_eq!(hooks.total_hooks(), 8);

        // Verify the debug format reflects the distribution
        let debug = format!("{:?}", hooks);
        assert!(debug.contains("on_session_start: 3"));
        assert!(debug.contains("on_user_message: 1"));
        assert!(debug.contains("on_session_end: 2"));
        assert!(debug.contains("on_compaction: 2"));
    }
}
