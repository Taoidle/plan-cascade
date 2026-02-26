//! Session Bridge
//!
//! Bridges remote commands to local Standalone LLM sessions.
//! Creates OrchestratorService instances, manages streaming modes,
//! rate limiting, path sandboxing, and session lifecycle.

use super::adapters::RemoteAdapter;
use super::types::{
    RemoteError, RemoteResponse, RemoteSessionMapping, SessionType, StreamingMode,
};
use crate::commands::proxy::resolve_provider_proxy;
use crate::commands::standalone::{
    get_api_key_with_aliases, normalize_provider_name, provider_type_from_name,
};
use crate::services::llm::{ProviderConfig, ProviderType};
use crate::services::orchestrator::{OrchestratorConfig, OrchestratorService};
use crate::services::streaming::UnifiedStreamEvent;
use crate::storage::{Database, KeyringService};
use rusqlite::params;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, RwLock};

/// Default provider when none is specified.
const DEFAULT_PROVIDER: &str = "anthropic";
/// Default model when none is specified.
const DEFAULT_MODEL: &str = "claude-sonnet-4-5-20250929";

// ---------------------------------------------------------------------------
// BridgeServices
// ---------------------------------------------------------------------------

/// Aggregated services needed by SessionBridge to create real OrchestratorService instances.
pub struct BridgeServices {
    /// Keyring for API key retrieval
    pub keyring: Arc<KeyringService>,
    /// Live orchestrator instances keyed by session_id
    pub orchestrators: Arc<RwLock<HashMap<String, Arc<OrchestratorService>>>>,
    /// Allowed project paths (empty = allow all)
    pub allowed_paths: Vec<PathBuf>,
    /// Minimum interval (ms) between messages per chat (rate limiting)
    pub rate_limit_interval_ms: u64,
}

// ---------------------------------------------------------------------------
// SessionBridge
// ---------------------------------------------------------------------------

/// Bridges remote commands to local session operations.
///
/// Maintains mapping between remote chat IDs and local session IDs,
/// creates OrchestratorService instances for Standalone LLM execution,
/// and implements three streaming modes: WaitForComplete, PeriodicUpdate, LiveEdit.
pub struct SessionBridge {
    /// Mapping: chat_id -> local session
    pub(crate) sessions: RwLock<HashMap<i64, RemoteSessionMapping>>,
    /// Database for persistence
    pub(crate) db: Arc<Database>,
    /// Optional services for real orchestrator creation
    services: Option<BridgeServices>,
    /// Rate limit: last message timestamp per chat_id
    last_message_times: RwLock<HashMap<i64, Instant>>,
    /// Tracks which session_ids are currently executing
    executing: RwLock<HashMap<String, bool>>,
}

impl SessionBridge {
    /// Create a new SessionBridge (test-friendly, no orchestrator creation).
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            db,
            services: None,
            last_message_times: RwLock::new(HashMap::new()),
            executing: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new SessionBridge with full services for production use.
    pub fn new_with_services(db: Arc<Database>, services: BridgeServices) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            db,
            services: Some(services),
            last_message_times: RwLock::new(HashMap::new()),
            executing: RwLock::new(HashMap::new()),
        }
    }

    /// Get the number of active remote sessions.
    pub async fn active_session_count(&self) -> u32 {
        self.sessions.read().await.len() as u32
    }

    /// List all session mappings.
    pub async fn list_all_sessions(&self) -> Vec<RemoteSessionMapping> {
        self.sessions.read().await.values().cloned().collect()
    }

    /// Get formatted sessions text for a chat.
    pub async fn list_sessions_text(&self, _chat_id: i64) -> String {
        let sessions = self.sessions.read().await;
        if sessions.is_empty() {
            return "No active remote sessions.".to_string();
        }
        let mut text = "Active Remote Sessions:\n".to_string();
        for (cid, mapping) in sessions.iter() {
            let path_info = mapping
                .project_path
                .as_deref()
                .map(|p| format!(" [{}]", p))
                .unwrap_or_default();
            text.push_str(&format!(
                "  Chat {} -> {} ({}){}\n",
                cid,
                mapping.local_session_id.as_deref().unwrap_or("no session"),
                mapping.session_type,
                path_info,
            ));
        }
        text
    }

    /// Get status text for a chat.
    pub async fn get_status_text(&self, chat_id: i64) -> String {
        let sessions = self.sessions.read().await;
        match sessions.get(&chat_id) {
            Some(mapping) => {
                let path_info = mapping
                    .project_path
                    .as_deref()
                    .unwrap_or("(no project path)");
                let executing = if let Some(sid) = &mapping.local_session_id {
                    let exec = self.executing.read().await;
                    exec.get(sid).copied().unwrap_or(false)
                } else {
                    false
                };
                format!(
                    "Session: {}\nType: {}\nProject: {}\nCreated: {}\nExecuting: {}",
                    mapping.local_session_id.as_deref().unwrap_or("none"),
                    mapping.session_type,
                    path_info,
                    mapping.created_at,
                    if executing { "yes" } else { "no" },
                )
            }
            None => "No active session for this chat.".to_string(),
        }
    }

    /// Cancel execution for a chat's active session.
    pub async fn cancel_execution(&self, chat_id: i64) -> Result<(), RemoteError> {
        let sessions = self.sessions.read().await;
        let mapping = sessions
            .get(&chat_id)
            .ok_or(RemoteError::NoActiveSession)?;
        let session_id = mapping
            .local_session_id
            .as_deref()
            .ok_or(RemoteError::NoActiveSession)?;

        if let Some(ref svc) = self.services {
            let orchestrators = svc.orchestrators.read().await;
            if let Some(orch) = orchestrators.get(session_id) {
                orch.cancel();
            }
        }

        // Clear executing flag
        self.executing.write().await.remove(session_id);

        Ok(())
    }

    /// Close and remove session mapping for a chat.
    pub async fn close_session(&self, chat_id: i64) -> Result<(), RemoteError> {
        let mut sessions = self.sessions.write().await;
        let mapping = sessions
            .remove(&chat_id)
            .ok_or(RemoteError::NoActiveSession)?;

        // Cancel and remove orchestrator
        if let Some(session_id) = &mapping.local_session_id {
            if let Some(ref svc) = self.services {
                let mut orchestrators = svc.orchestrators.write().await;
                if let Some(orch) = orchestrators.remove(session_id) {
                    orch.cancel();
                }
            }
            self.executing.write().await.remove(session_id);
        }

        // Remove from DB
        self.remove_mapping_from_db(chat_id);

        Ok(())
    }

    /// Switch active session for a chat.
    pub async fn switch_session(
        &self,
        chat_id: i64,
        session_id: &str,
    ) -> Result<(), RemoteError> {
        // Verify the target session exists in orchestrators
        if let Some(ref svc) = self.services {
            let orchestrators = svc.orchestrators.read().await;
            if !orchestrators.contains_key(session_id) {
                return Err(RemoteError::SessionNotFound(session_id.to_string()));
            }
        }

        let mut sessions = self.sessions.write().await;
        let mapping = sessions
            .get_mut(&chat_id)
            .ok_or(RemoteError::NoActiveSession)?;
        mapping.local_session_id = Some(session_id.to_string());

        // Persist updated mapping
        let mapping_clone = mapping.clone();
        drop(sessions);
        self.persist_mapping_to_db(&mapping_clone);

        Ok(())
    }

    /// Send message to the active session and collect the response.
    pub async fn send_message(
        &self,
        chat_id: i64,
        content: &str,
        streaming_mode: &StreamingMode,
        adapter: Option<&(dyn RemoteAdapter + '_)>,
    ) -> Result<RemoteResponse, RemoteError> {
        // Rate limit check
        self.check_rate_limit(chat_id)?;

        let sessions = self.sessions.read().await;
        let mapping = sessions
            .get(&chat_id)
            .ok_or(RemoteError::NoActiveSession)?;
        let session_id = mapping
            .local_session_id
            .clone()
            .ok_or(RemoteError::NoActiveSession)?;
        let mapping_clone = mapping.clone();
        drop(sessions);

        // Check services availability
        let svc = self
            .services
            .as_ref()
            .ok_or_else(|| RemoteError::ConfigError("Bridge services not configured".to_string()))?;

        // Check busy
        {
            let exec = self.executing.read().await;
            if exec.get(&session_id).copied().unwrap_or(false) {
                return Err(RemoteError::SessionBusy(
                    "Session is busy, please wait or /cancel".to_string(),
                ));
            }
        }

        // Get or lazily rebuild orchestrator
        let orchestrator = self
            .get_or_rebuild_orchestrator(svc, &session_id, &mapping_clone)
            .await?;

        // Check if orchestrator was cancelled (CancellationToken is not reversible)
        let orchestrator = if orchestrator.is_cancelled() {
            self.recreate_orchestrator_from_mapping(svc, &mapping_clone, &session_id)
                .await?
        } else {
            orchestrator
        };

        // Mark as executing
        self.executing
            .write()
            .await
            .insert(session_id.clone(), true);

        // Create channel for streaming events
        let (tx, rx) = mpsc::channel::<UnifiedStreamEvent>(100);

        // Spawn execution
        let orch = orchestrator.clone();
        let message = content.to_string();
        let exec_handle = tokio::spawn(async move { orch.execute(message, tx).await });

        // Collect results based on streaming mode
        let result = match streaming_mode {
            StreamingMode::WaitForComplete => {
                self.collect_wait_for_complete(rx, exec_handle).await
            }
            StreamingMode::PeriodicUpdate { interval_secs } => {
                self.collect_periodic_update(
                    rx,
                    exec_handle,
                    chat_id,
                    *interval_secs,
                    adapter,
                )
                .await
            }
            StreamingMode::LiveEdit { throttle_ms } => {
                self.collect_live_edit(rx, exec_handle, chat_id, *throttle_ms, adapter)
                    .await
            }
        };

        // Clear executing flag
        self.executing.write().await.remove(&session_id);

        result
    }

    /// Create a new session for a remote chat.
    pub async fn create_session(
        &self,
        chat_id: i64,
        user_id: i64,
        project_path: &str,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> Result<String, RemoteError> {
        self.create_session_with_source(
            chat_id, user_id, project_path, provider, model, None, None,
        )
        .await
    }

    /// Create a new session with remote source tracking.
    pub async fn create_session_with_source(
        &self,
        chat_id: i64,
        user_id: i64,
        project_path: &str,
        provider: Option<&str>,
        model: Option<&str>,
        adapter_type_name: Option<&str>,
        username: Option<&str>,
    ) -> Result<String, RemoteError> {
        let svc = self.services.as_ref();

        // Validate and resolve project path
        let resolved_path = self.validate_project_path(project_path, svc)?;

        // Resolve provider / model
        let (canonical_provider, provider_type, resolved_model) =
            self.resolve_provider_model(provider, model)?;

        // Build session ID
        let session_id = uuid::Uuid::new_v4().to_string();

        // Create orchestrator if services are available
        if let Some(svc) = svc {
            let api_key = if provider_type != ProviderType::Ollama {
                let key = get_api_key_with_aliases(&svc.keyring, canonical_provider)
                    .map_err(|e| RemoteError::ConfigError(e))?;
                if key.is_none() {
                    return Err(RemoteError::ConfigError(format!(
                        "No API key found for provider '{}'",
                        canonical_provider
                    )));
                }
                key
            } else {
                None
            };

            let base_url = self.resolve_base_url(canonical_provider);

            let proxy = resolve_provider_proxy(&svc.keyring, &self.db, canonical_provider);

            let provider_config = ProviderConfig {
                provider: provider_type,
                api_key,
                base_url,
                model: resolved_model.clone(),
                max_tokens: 4096,
                temperature: 0.7,
                proxy,
                ..Default::default()
            };

            let analysis_artifacts_root = self.analysis_artifacts_root();

            let orchestrator_config = OrchestratorConfig {
                provider: provider_config,
                system_prompt: None,
                max_iterations: 50,
                max_total_tokens: 1_000_000,
                project_root: PathBuf::from(&resolved_path),
                streaming: true,
                enable_compaction: true,
                analysis_artifacts_root,
                analysis_profile: Default::default(),
                analysis_limits: Default::default(),
                analysis_session_id: Some(session_id.clone()),
                project_id: None,
                compaction_config: Default::default(),
                task_type: None,
                sub_agent_depth: None,
            };

            let mut orchestrator = OrchestratorService::new(orchestrator_config);

            // Wire database pool for CodebaseSearch/IndexStore
            {
                let pool = self.db.pool().clone();
                orchestrator = orchestrator.with_database(pool);
            }

            svc.orchestrators
                .write()
                .await
                .insert(session_id.clone(), Arc::new(orchestrator));
        }

        let session_type = SessionType::Standalone {
            provider: canonical_provider.to_string(),
            model: resolved_model,
        };

        let mapping = RemoteSessionMapping {
            chat_id,
            user_id,
            local_session_id: Some(session_id.clone()),
            session_type,
            created_at: chrono::Utc::now().to_rfc3339(),
            adapter_type_name: adapter_type_name.map(|s| s.to_string()),
            username: username.map(|s| s.to_string()),
            project_path: Some(resolved_path),
        };

        self.persist_mapping_to_db(&mapping);
        self.sessions.write().await.insert(chat_id, mapping);

        Ok(session_id)
    }

    /// Get the active local session ID for a given chat.
    pub async fn get_active_session_id(&self, chat_id: i64) -> Option<String> {
        let sessions = self.sessions.read().await;
        sessions
            .get(&chat_id)
            .and_then(|m| m.local_session_id.clone())
    }

    /// Load session mappings from database on startup.
    pub async fn load_mappings_from_db(&self) -> Result<(), RemoteError> {
        // Collect all DB results into a Vec before any .await point.
        // rusqlite types (Connection, Statement, MappedRows) are not Send/Sync,
        // so they must be dropped before crossing an await boundary.
        let collected: Vec<RemoteSessionMapping> = {
            let conn = self.db.get_connection().map_err(|e| {
                RemoteError::ConfigError(format!("Failed to get database connection: {}", e))
            })?;

            let mut stmt = conn
                .prepare(
                    "SELECT chat_id, user_id, adapter_type, local_session_id, session_type, created_at, project_path
                     FROM remote_session_mappings",
                )
                .map_err(|e| {
                    RemoteError::ConfigError(format!("Failed to prepare query: {}", e))
                })?;

            let mappings = stmt
                .query_map([], |row| {
                    let session_type_json: String = row.get(4)?;
                    let session_type: super::types::SessionType =
                        serde_json::from_str(&session_type_json)
                            .unwrap_or(super::types::SessionType::ClaudeCode);

                    let adapter_type_name: Option<String> = row.get(2).ok();

                    Ok(RemoteSessionMapping {
                        chat_id: row.get(0)?,
                        user_id: row.get(1)?,
                        local_session_id: row.get(3)?,
                        session_type,
                        created_at: row.get(5)?,
                        adapter_type_name,
                        username: None, // Not stored in current schema
                        project_path: row.get(6).ok().flatten(),
                    })
                })
                .map_err(|e| {
                    RemoteError::ConfigError(format!("Failed to query mappings: {}", e))
                })?;

            mappings.flatten().collect()
        }; // conn, stmt, and MappedRows dropped here

        let mut sessions = self.sessions.write().await;
        for mapping in collected {
            sessions.insert(mapping.chat_id, mapping);
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Check rate limit for a chat.
    fn check_rate_limit(&self, chat_id: i64) -> Result<(), RemoteError> {
        let interval_ms = self
            .services
            .as_ref()
            .map(|s| s.rate_limit_interval_ms)
            .unwrap_or(0);
        if interval_ms == 0 {
            return Ok(());
        }

        // Use try_write to avoid deadlock; skip check if lock is contended
        if let Ok(mut times) = self.last_message_times.try_write() {
            let now = Instant::now();
            if let Some(last) = times.get(&chat_id) {
                let elapsed = now.duration_since(*last).as_millis() as u64;
                if elapsed < interval_ms {
                    return Err(RemoteError::RateLimited(format!(
                        "Please wait {}ms between messages",
                        interval_ms - elapsed
                    )));
                }
            }
            times.insert(chat_id, now);
        }
        Ok(())
    }

    /// Validate and resolve a project path with sandbox checking.
    fn validate_project_path(
        &self,
        path: &str,
        svc: Option<&BridgeServices>,
    ) -> Result<String, RemoteError> {
        // Expand ~ to home directory
        let expanded = if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                home.join(path.strip_prefix("~/").unwrap_or(&path[1..]))
            } else {
                PathBuf::from(path)
            }
        } else {
            PathBuf::from(path)
        };

        // Canonicalize (resolve symlinks) — path must exist
        let canonical = expanded.canonicalize().map_err(|e| {
            RemoteError::ConfigError(format!(
                "Project path '{}' does not exist or is not accessible: {}",
                path, e
            ))
        })?;

        // Must be a directory
        if !canonical.is_dir() {
            return Err(RemoteError::ConfigError(format!(
                "Project path '{}' is not a directory",
                path
            )));
        }

        // Sandbox check
        if let Some(svc) = svc {
            if !svc.allowed_paths.is_empty() {
                let allowed = svc.allowed_paths.iter().any(|allowed| {
                    if let Ok(allowed_canonical) = allowed.canonicalize() {
                        canonical.starts_with(&allowed_canonical)
                    } else {
                        false
                    }
                });
                if !allowed {
                    return Err(RemoteError::PathSandboxViolation(format!(
                        "Path '{}' is outside allowed directories",
                        canonical.display()
                    )));
                }
            }
        }

        Ok(canonical.to_string_lossy().to_string())
    }

    /// Resolve provider and model strings to canonical form.
    fn resolve_provider_model(
        &self,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> Result<(&'static str, ProviderType, String), RemoteError> {
        let provider_str = provider.unwrap_or(DEFAULT_PROVIDER);
        let canonical = normalize_provider_name(provider_str).ok_or_else(|| {
            RemoteError::ConfigError(format!("Unknown provider: '{}'", provider_str))
        })?;
        let provider_type = provider_type_from_name(canonical).ok_or_else(|| {
            RemoteError::ConfigError(format!(
                "Cannot resolve provider type for: '{}'",
                canonical
            ))
        })?;
        let resolved_model = model.unwrap_or(DEFAULT_MODEL).to_string();
        Ok((canonical, provider_type, resolved_model))
    }

    /// Resolve base_url for a provider from DB settings.
    fn resolve_base_url(&self, canonical_provider: &str) -> Option<String> {
        let key = format!("provider_{}_base_url", canonical_provider);
        self.db
            .get_setting(&key)
            .ok()
            .flatten()
            .filter(|url| !url.is_empty())
    }

    /// Get or lazily rebuild orchestrator for a session.
    async fn get_or_rebuild_orchestrator(
        &self,
        svc: &BridgeServices,
        session_id: &str,
        mapping: &RemoteSessionMapping,
    ) -> Result<Arc<OrchestratorService>, RemoteError> {
        // Try existing orchestrator first
        {
            let orchestrators = svc.orchestrators.read().await;
            if let Some(orch) = orchestrators.get(session_id) {
                return Ok(orch.clone());
            }
        }

        // Lazy rebuild from mapping
        self.recreate_orchestrator_from_mapping(svc, mapping, session_id)
            .await
    }

    /// Recreate an orchestrator from a persisted mapping (gateway restart scenario).
    async fn recreate_orchestrator_from_mapping(
        &self,
        svc: &BridgeServices,
        mapping: &RemoteSessionMapping,
        session_id: &str,
    ) -> Result<Arc<OrchestratorService>, RemoteError> {
        let (canonical_provider, provider_type, model) = match &mapping.session_type {
            SessionType::Standalone { provider, model } => {
                let canonical = normalize_provider_name(provider).unwrap_or("anthropic");
                let pt = provider_type_from_name(canonical)
                    .unwrap_or(ProviderType::Anthropic);
                (canonical, pt, model.clone())
            }
            SessionType::ClaudeCode => {
                return Err(RemoteError::ConfigError(
                    "ClaudeCode sessions are not supported in remote mode".to_string(),
                ));
            }
        };

        let project_path = mapping.project_path.as_deref().unwrap_or(".");

        let api_key = if provider_type != ProviderType::Ollama {
            let key = get_api_key_with_aliases(&svc.keyring, canonical_provider)
                .map_err(|e| RemoteError::ConfigError(e))?;
            if key.is_none() {
                return Err(RemoteError::ConfigError(format!(
                    "No API key found for provider '{}'",
                    canonical_provider
                )));
            }
            key
        } else {
            None
        };

        let base_url = self.resolve_base_url(canonical_provider);
        let proxy = resolve_provider_proxy(&svc.keyring, &self.db, canonical_provider);

        let provider_config = ProviderConfig {
            provider: provider_type,
            api_key,
            base_url,
            model,
            max_tokens: 4096,
            temperature: 0.7,
            proxy,
            ..Default::default()
        };

        let analysis_artifacts_root = self.analysis_artifacts_root();

        let orchestrator_config = OrchestratorConfig {
            provider: provider_config,
            system_prompt: None,
            max_iterations: 50,
            max_total_tokens: 1_000_000,
            project_root: PathBuf::from(project_path),
            streaming: true,
            enable_compaction: true,
            analysis_artifacts_root,
            analysis_profile: Default::default(),
            analysis_limits: Default::default(),
            analysis_session_id: Some(session_id.to_string()),
            project_id: None,
            compaction_config: Default::default(),
            task_type: None,
            sub_agent_depth: None,
        };

        let mut orchestrator = OrchestratorService::new(orchestrator_config);

        {
            let pool = self.db.pool().clone();
            orchestrator = orchestrator.with_database(pool);
        }

        let orch = Arc::new(orchestrator);
        svc.orchestrators
            .write()
            .await
            .insert(session_id.to_string(), orch.clone());
        Ok(orch)
    }

    /// Persist a session mapping to the database.
    fn persist_mapping_to_db(&self, mapping: &RemoteSessionMapping) {
        let session_type_json = serde_json::to_string(&mapping.session_type).unwrap_or_default();
        let adapter_type = mapping
            .adapter_type_name
            .as_deref()
            .unwrap_or("unknown");
        let now = chrono::Utc::now().to_rfc3339();

        if let Ok(conn) = self.db.get_connection() {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO remote_session_mappings
                 (chat_id, user_id, adapter_type, local_session_id, session_type, created_at, updated_at, project_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    mapping.chat_id,
                    mapping.user_id,
                    adapter_type,
                    mapping.local_session_id,
                    session_type_json,
                    mapping.created_at,
                    now,
                    mapping.project_path,
                ],
            );
        }
    }

    /// Remove a session mapping from the database.
    fn remove_mapping_from_db(&self, chat_id: i64) {
        if let Ok(conn) = self.db.get_connection() {
            let _ = conn.execute(
                "DELETE FROM remote_session_mappings WHERE chat_id = ?1",
                params![chat_id],
            );
        }
    }

    /// Analysis artifacts root directory.
    fn analysis_artifacts_root(&self) -> PathBuf {
        if let Ok(base) = crate::utils::paths::ensure_plan_cascade_dir() {
            return base.join("analysis-runs");
        }
        dirs::home_dir()
            .unwrap_or_else(|| std::env::temp_dir())
            .join(".plan-cascade")
            .join("analysis-runs")
    }

    // -----------------------------------------------------------------------
    // Streaming mode collectors
    // -----------------------------------------------------------------------

    /// WaitForComplete: accumulate all events, return final response.
    async fn collect_wait_for_complete(
        &self,
        mut rx: mpsc::Receiver<UnifiedStreamEvent>,
        exec_handle: tokio::task::JoinHandle<crate::services::orchestrator::ExecutionResult>,
    ) -> Result<RemoteResponse, RemoteError> {
        let mut text = String::new();
        let mut thinking = String::new();
        let mut tool_summaries = Vec::new();

        while let Some(event) = rx.recv().await {
            match event {
                UnifiedStreamEvent::TextDelta { content } => text.push_str(&content),
                UnifiedStreamEvent::TextReplace { content } => text = content,
                UnifiedStreamEvent::ThinkingDelta { content, .. } => {
                    thinking.push_str(&content);
                }
                UnifiedStreamEvent::ToolComplete {
                    tool_name,
                    arguments,
                    ..
                } => {
                    tool_summaries.push(format!("[{}]: {}", tool_name, truncate_str(&arguments, 100)));
                }
                UnifiedStreamEvent::Error { message, .. } => {
                    return Err(RemoteError::ExecutionFailed(message));
                }
                UnifiedStreamEvent::Complete { .. } => break,
                _ => {}
            }
        }

        // Wait for the execution task to finish
        match exec_handle.await {
            Ok(result) => {
                if !result.success {
                    if let Some(err) = result.error {
                        return Err(RemoteError::ExecutionFailed(err));
                    }
                }
                // Prefer execution result response if text is empty
                if text.is_empty() {
                    if let Some(resp) = result.response {
                        text = resp;
                    }
                }
            }
            Err(e) => {
                return Err(RemoteError::ExecutionFailed(format!(
                    "Execution task panicked: {}",
                    e
                )));
            }
        }

        Ok(RemoteResponse {
            text: if text.is_empty() {
                "(No response)".to_string()
            } else {
                text
            },
            thinking: if thinking.is_empty() {
                None
            } else {
                Some(thinking)
            },
            tool_summary: if tool_summaries.is_empty() {
                None
            } else {
                Some(tool_summaries.join("\n"))
            },
            already_sent: false,
        })
    }

    /// PeriodicUpdate: send progress snapshots every N seconds.
    async fn collect_periodic_update(
        &self,
        mut rx: mpsc::Receiver<UnifiedStreamEvent>,
        exec_handle: tokio::task::JoinHandle<crate::services::orchestrator::ExecutionResult>,
        chat_id: i64,
        interval_secs: u32,
        adapter: Option<&(dyn RemoteAdapter + '_)>,
    ) -> Result<RemoteResponse, RemoteError> {
        let mut text = String::new();
        let mut thinking = String::new();
        let mut tool_summaries = Vec::new();
        let mut last_update = Instant::now();
        let interval = std::time::Duration::from_secs(interval_secs as u64);

        while let Some(event) = rx.recv().await {
            match event {
                UnifiedStreamEvent::TextDelta { content } => text.push_str(&content),
                UnifiedStreamEvent::TextReplace { content } => text = content,
                UnifiedStreamEvent::ThinkingDelta { content, .. } => {
                    thinking.push_str(&content);
                }
                UnifiedStreamEvent::ToolComplete {
                    tool_name,
                    arguments,
                    ..
                } => {
                    tool_summaries.push(format!("[{}]: {}", tool_name, truncate_str(&arguments, 100)));
                }
                UnifiedStreamEvent::Error { message, .. } => {
                    return Err(RemoteError::ExecutionFailed(message));
                }
                UnifiedStreamEvent::Complete { .. } => break,
                _ => {}
            }

            // Send periodic update
            if last_update.elapsed() >= interval && !text.is_empty() {
                if let Some(adapter) = adapter {
                    let snapshot = format!("⏳ In progress...\n\n{}", truncate_str(&text, 3800));
                    let _ = adapter.send_message(chat_id, &snapshot).await;
                }
                last_update = Instant::now();
            }
        }

        // Wait for execution
        if let Ok(result) = exec_handle.await {
            if !result.success {
                if let Some(err) = result.error {
                    return Err(RemoteError::ExecutionFailed(err));
                }
            }
            if text.is_empty() {
                if let Some(resp) = result.response {
                    text = resp;
                }
            }
        }

        // Send final response via adapter
        let final_text = if text.is_empty() {
            "(No response)".to_string()
        } else {
            text.clone()
        };

        if let Some(adapter) = adapter {
            let _ = adapter.send_message(chat_id, &final_text).await;
        }

        Ok(RemoteResponse {
            text: final_text,
            thinking: if thinking.is_empty() {
                None
            } else {
                Some(thinking)
            },
            tool_summary: if tool_summaries.is_empty() {
                None
            } else {
                Some(tool_summaries.join("\n"))
            },
            already_sent: true,
        })
    }

    /// LiveEdit: send initial message, then edit it in-place.
    async fn collect_live_edit(
        &self,
        mut rx: mpsc::Receiver<UnifiedStreamEvent>,
        exec_handle: tokio::task::JoinHandle<crate::services::orchestrator::ExecutionResult>,
        chat_id: i64,
        throttle_ms: u64,
        adapter: Option<&(dyn RemoteAdapter + '_)>,
    ) -> Result<RemoteResponse, RemoteError> {
        let mut text = String::new();
        let mut thinking = String::new();
        let mut tool_summaries = Vec::new();
        let mut msg_id: Option<i64> = None;
        let mut last_edit = Instant::now();
        let throttle = std::time::Duration::from_millis(throttle_ms);

        while let Some(event) = rx.recv().await {
            match event {
                UnifiedStreamEvent::TextDelta { content } => text.push_str(&content),
                UnifiedStreamEvent::TextReplace { content } => text = content,
                UnifiedStreamEvent::ThinkingDelta { content, .. } => {
                    thinking.push_str(&content);
                }
                UnifiedStreamEvent::ToolComplete {
                    tool_name,
                    arguments,
                    ..
                } => {
                    tool_summaries.push(format!("[{}]: {}", tool_name, truncate_str(&arguments, 100)));
                }
                UnifiedStreamEvent::Error { message, .. } => {
                    return Err(RemoteError::ExecutionFailed(message));
                }
                UnifiedStreamEvent::Complete { .. } => break,
                _ => {}
            }

            if !text.is_empty() && last_edit.elapsed() >= throttle {
                if let Some(adapter) = adapter {
                    // Truncate for Telegram's 4096 limit
                    let display_text = truncate_str(&text, 4000);
                    match msg_id {
                        None => {
                            // Send initial message
                            match adapter
                                .send_message_returning_id(chat_id, &display_text)
                                .await
                            {
                                Ok(id) => msg_id = Some(id),
                                Err(_) => {} // Fall through, will retry
                            }
                        }
                        Some(id) if id != 0 => {
                            // Edit existing message
                            let _ = adapter.edit_message(chat_id, id, &display_text).await;
                        }
                        _ => {
                            // msg_id is 0 (default impl), can't edit — just send new
                            let _ = adapter.send_message(chat_id, &display_text).await;
                        }
                    }
                }
                last_edit = Instant::now();
            }
        }

        // Wait for execution
        if let Ok(result) = exec_handle.await {
            if !result.success {
                if let Some(err) = result.error {
                    return Err(RemoteError::ExecutionFailed(err));
                }
            }
            if text.is_empty() {
                if let Some(resp) = result.response {
                    text = resp;
                }
            }
        }

        // Final edit with complete text
        let final_text = if text.is_empty() {
            "(No response)".to_string()
        } else {
            text.clone()
        };

        if let Some(adapter) = adapter {
            let display_text = truncate_str(&final_text, 4000);
            match msg_id {
                Some(id) if id != 0 => {
                    let _ = adapter.edit_message(chat_id, id, &display_text).await;
                }
                _ => {
                    let _ = adapter.send_message(chat_id, &display_text).await;
                }
            }
        }

        Ok(RemoteResponse {
            text: final_text,
            thinking: if thinking.is_empty() {
                None
            } else {
                Some(thinking)
            },
            tool_summary: if tool_summaries.is_empty() {
                None
            } else {
                Some(tool_summaries.join("\n"))
            },
            already_sent: true,
        })
    }
}

/// Truncate a string to a maximum length, appending "..." if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_bridge_new() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        assert_eq!(bridge.active_session_count().await, 0);
    }

    #[tokio::test]
    async fn test_session_bridge_create_session_without_services() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        // Without services, create_session still stores the mapping
        // but with project path validation — needs a real path
        let result = bridge
            .create_session(123, 456, "/tmp", None, None)
            .await;
        assert!(result.is_ok());
        assert_eq!(bridge.active_session_count().await, 1);
    }

    #[tokio::test]
    async fn test_session_bridge_close_session() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        let _ = bridge.create_session(123, 456, "/tmp", None, None).await;
        assert_eq!(bridge.active_session_count().await, 1);
        bridge.close_session(123).await.unwrap();
        assert_eq!(bridge.active_session_count().await, 0);
    }

    #[tokio::test]
    async fn test_session_bridge_close_nonexistent() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        let result = bridge.close_session(999).await;
        assert!(matches!(result, Err(RemoteError::NoActiveSession)));
    }

    #[tokio::test]
    async fn test_session_bridge_cancel_nonexistent() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        let result = bridge.cancel_execution(999).await;
        assert!(matches!(result, Err(RemoteError::NoActiveSession)));
    }

    #[tokio::test]
    async fn test_path_sandbox_violation() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let services = BridgeServices {
            keyring: Arc::new(KeyringService::new()),
            orchestrators: Arc::new(RwLock::new(HashMap::new())),
            allowed_paths: vec![PathBuf::from("/tmp")],
            rate_limit_interval_ms: 0,
        };
        let bridge = SessionBridge::new_with_services(db, services);

        // /tmp should be allowed
        let result = bridge.validate_project_path("/tmp", bridge.services.as_ref());
        assert!(result.is_ok());

        // /etc should be denied (if it exists)
        if Path::new("/etc").exists() {
            let result = bridge.validate_project_path("/etc", bridge.services.as_ref());
            assert!(matches!(result, Err(RemoteError::PathSandboxViolation(_))));
        }
    }

    #[tokio::test]
    async fn test_path_sandbox_empty_allows_all() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let services = BridgeServices {
            keyring: Arc::new(KeyringService::new()),
            orchestrators: Arc::new(RwLock::new(HashMap::new())),
            allowed_paths: vec![], // Empty = allow all
            rate_limit_interval_ms: 0,
        };
        let bridge = SessionBridge::new_with_services(db, services);

        let result = bridge.validate_project_path("/tmp", bridge.services.as_ref());
        assert!(result.is_ok());
    }

    #[test]
    fn test_path_nonexistent() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        let result = bridge.validate_project_path("/nonexistent_path_xyz_123", None);
        assert!(matches!(result, Err(RemoteError::ConfigError(_))));
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let services = BridgeServices {
            keyring: Arc::new(KeyringService::new()),
            orchestrators: Arc::new(RwLock::new(HashMap::new())),
            allowed_paths: vec![],
            rate_limit_interval_ms: 5000, // 5 second interval
        };
        let bridge = SessionBridge::new_with_services(db, services);

        // First message should pass
        assert!(bridge.check_rate_limit(123).is_ok());
        // Second message immediately should be rate limited
        assert!(matches!(
            bridge.check_rate_limit(123),
            Err(RemoteError::RateLimited(_))
        ));
        // Different chat should pass
        assert!(bridge.check_rate_limit(456).is_ok());
    }

    #[test]
    fn test_resolve_provider_model_default() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        let (canonical, pt, model) = bridge.resolve_provider_model(None, None).unwrap();
        assert_eq!(canonical, "anthropic");
        assert_eq!(pt, ProviderType::Anthropic);
        assert_eq!(model, DEFAULT_MODEL);
    }

    #[test]
    fn test_resolve_provider_model_explicit() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        let (canonical, pt, model) = bridge
            .resolve_provider_model(Some("openai"), Some("gpt-4"))
            .unwrap();
        assert_eq!(canonical, "openai");
        assert_eq!(pt, ProviderType::OpenAI);
        assert_eq!(model, "gpt-4");
    }

    #[test]
    fn test_resolve_provider_model_unknown() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        let result = bridge.resolve_provider_model(Some("unknown_provider"), None);
        assert!(matches!(result, Err(RemoteError::ConfigError(_))));
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 8), "hello...");
        assert_eq!(truncate_str("", 5), "");
    }

    #[tokio::test]
    async fn test_list_sessions_text_with_project_path() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        let _ = bridge
            .create_session_with_source(
                123,
                456,
                "/tmp",
                Some("anthropic"),
                Some("claude-sonnet"),
                Some("Telegram"),
                Some("testuser"),
            )
            .await;
        let text = bridge.list_sessions_text(123).await;
        assert!(text.contains("Chat 123"));
        assert!(text.contains("Standalone"));
    }

    #[tokio::test]
    async fn test_get_status_text() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        let _ = bridge
            .create_session(123, 456, "/tmp", None, None)
            .await;
        let text = bridge.get_status_text(123).await;
        assert!(text.contains("Session:"));
        assert!(text.contains("Type:"));
        assert!(text.contains("Project:"));
    }

    #[tokio::test]
    async fn test_get_status_text_no_session() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        let text = bridge.get_status_text(123).await;
        assert_eq!(text, "No active session for this chat.");
    }

    #[tokio::test]
    async fn test_load_mappings_from_db() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db.clone());

        // Insert a mapping manually
        let conn = db.get_connection().unwrap();
        conn.execute(
            "INSERT INTO remote_session_mappings (chat_id, user_id, adapter_type, local_session_id, session_type, created_at, updated_at, project_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                123i64,
                456i64,
                "Telegram",
                "sess-abc",
                r#"{"Standalone":{"provider":"anthropic","model":"claude"}}"#,
                "2026-01-01T00:00:00Z",
                "2026-01-01T00:00:00Z",
                "/tmp",
            ],
        )
        .unwrap();
        drop(conn);

        bridge.load_mappings_from_db().await.unwrap();
        assert_eq!(bridge.active_session_count().await, 1);
        let sessions = bridge.list_all_sessions().await;
        assert_eq!(sessions[0].project_path, Some("/tmp".to_string()));
    }

    #[tokio::test]
    async fn test_send_message_no_services() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = SessionBridge::new(db);
        let _ = bridge.create_session(123, 456, "/tmp", None, None).await;
        let result = bridge
            .send_message(123, "hello", &StreamingMode::WaitForComplete, None)
            .await;
        // Should fail because no services are configured
        assert!(matches!(result, Err(RemoteError::ConfigError(_))));
    }

    #[tokio::test]
    async fn test_send_message_no_session() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let services = BridgeServices {
            keyring: Arc::new(KeyringService::new()),
            orchestrators: Arc::new(RwLock::new(HashMap::new())),
            allowed_paths: vec![],
            rate_limit_interval_ms: 0,
        };
        let bridge = SessionBridge::new_with_services(db, services);
        let result = bridge
            .send_message(123, "hello", &StreamingMode::WaitForComplete, None)
            .await;
        assert!(matches!(result, Err(RemoteError::NoActiveSession)));
    }
}
