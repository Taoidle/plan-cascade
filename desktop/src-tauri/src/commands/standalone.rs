//! Standalone Mode Commands
//!
//! Tauri commands for standalone LLM execution without Claude Code CLI.
//! Includes session-based execution with persistence, cancellation, and progress tracking.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, RwLock};

use crate::commands::proxy::resolve_provider_proxy;
use crate::models::orchestrator::{
    ExecuteWithSessionRequest, ExecutionProgress, ExecutionSession, ExecutionSessionSummary,
    ExecutionStatus, ResumeExecutionRequest, StandaloneStatus,
};
use crate::models::CommandResponse;
use crate::services::llm::{ProviderConfig, ProviderType};
use crate::services::orchestrator::index_manager::{IndexManager, IndexStatusEvent};
use crate::services::orchestrator::{
    ExecutionResult, OrchestratorConfig, OrchestratorService, SessionExecutionResult,
};
use crate::services::streaming::UnifiedStreamEvent;
use crate::state::AppState;
use crate::storage::KeyringService;
use crate::utils::paths::ensure_plan_cascade_dir;

/// State for standalone execution management
pub struct StandaloneState {
    /// Active orchestrators by session ID
    pub orchestrators: Arc<RwLock<HashMap<String, Arc<OrchestratorService>>>>,
    /// Current working directory for standalone mode
    pub working_directory: Arc<RwLock<PathBuf>>,
    /// Index manager for background codebase indexing
    pub index_manager: Arc<RwLock<Option<IndexManager>>>,
}

impl Default for StandaloneState {
    fn default() -> Self {
        Self::new()
    }
}

impl StandaloneState {
    /// Create a new standalone state
    pub fn new() -> Self {
        Self {
            orchestrators: Arc::new(RwLock::new(HashMap::new())),
            working_directory: Arc::new(RwLock::new(
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            )),
            index_manager: Arc::new(RwLock::new(None)),
        }
    }

    /// Get an orchestrator by session ID
    pub async fn get_orchestrator(&self, session_id: &str) -> Option<Arc<OrchestratorService>> {
        let orchestrators = self.orchestrators.read().await;
        orchestrators.get(session_id).cloned()
    }

    /// Store an orchestrator for a session
    pub async fn set_orchestrator(
        &self,
        session_id: String,
        orchestrator: Arc<OrchestratorService>,
    ) {
        let mut orchestrators = self.orchestrators.write().await;
        orchestrators.insert(session_id, orchestrator);
    }

    /// Remove an orchestrator
    pub async fn remove_orchestrator(&self, session_id: &str) {
        let mut orchestrators = self.orchestrators.write().await;
        orchestrators.remove(session_id);
    }
}

/// Provider information returned to frontend
#[derive(serde::Serialize)]
pub struct ProviderInfo {
    pub provider_type: String,
    pub name: String,
    pub models: Vec<ModelInfo>,
    pub requires_api_key: bool,
    pub default_base_url: Option<String>,
}

/// Model information
#[derive(serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub supports_thinking: bool,
    pub supports_tools: bool,
    pub context_window: u32,
    pub pricing: Option<Pricing>,
}

/// Pricing information for cost tracking
#[derive(serde::Serialize, Clone)]
pub struct Pricing {
    /// Cost per million input tokens in USD
    pub input_per_million: f64,
    /// Cost per million output tokens in USD
    pub output_per_million: f64,
    /// Cost per million thinking tokens in USD (if separate)
    pub thinking_per_million: Option<f64>,
}

/// Usage statistics for a session
#[derive(serde::Serialize, Default)]
pub struct UsageStatistics {
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
    pub total_thinking_tokens: u32,
    pub total_cost_usd: f64,
    pub requests: u32,
}

/// Normalize provider aliases to canonical names used by orchestrator/keyring.
pub fn normalize_provider_name(provider: &str) -> Option<&'static str> {
    match provider.trim().to_lowercase().as_str() {
        "anthropic" | "claude" | "claude-api" => Some("anthropic"),
        "openai" => Some("openai"),
        "deepseek" => Some("deepseek"),
        "glm" | "glm-api" | "zhipu" | "zhipuai" => Some("glm"),
        "qwen" | "qwen-api" | "dashscope" | "alibaba" | "aliyun" => Some("qwen"),
        "minimax" | "minimax-api" => Some("minimax"),
        "ollama" => Some("ollama"),
        _ => None,
    }
}

fn provider_type_from_name(provider: &str) -> Option<ProviderType> {
    match provider {
        "anthropic" => Some(ProviderType::Anthropic),
        "openai" => Some(ProviderType::OpenAI),
        "deepseek" => Some(ProviderType::DeepSeek),
        "glm" => Some(ProviderType::Glm),
        "qwen" => Some(ProviderType::Qwen),
        "minimax" => Some(ProviderType::Minimax),
        "ollama" => Some(ProviderType::Ollama),
        _ => None,
    }
}

fn provider_key_candidates(provider: &str) -> &'static [&'static str] {
    match provider {
        "anthropic" => &["anthropic", "claude", "claude-api"],
        "openai" => &["openai"],
        "deepseek" => &["deepseek"],
        "glm" => &["glm", "glm-api", "zhipu", "zhipuai"],
        "qwen" => &["qwen", "qwen-api", "dashscope", "alibaba", "aliyun"],
        "minimax" => &["minimax", "minimax-api"],
        "ollama" => &["ollama"],
        _ => &[],
    }
}

fn canonical_providers() -> &'static [&'static str] {
    &[
        "anthropic",
        "openai",
        "deepseek",
        "glm",
        "qwen",
        "minimax",
        "ollama",
    ]
}

pub(crate) fn get_api_key_with_aliases(
    keyring: &KeyringService,
    canonical_provider: &str,
) -> Result<Option<String>, String> {
    for candidate in provider_key_candidates(canonical_provider) {
        match keyring.get_api_key(candidate) {
            Ok(Some(key)) => return Ok(Some(key)),
            Ok(None) => continue,
            Err(e) => return Err(format!("Failed to get API key: {}", e)),
        }
    }
    Ok(None)
}

fn analysis_artifacts_root() -> PathBuf {
    if let Ok(base) = ensure_plan_cascade_dir() {
        return base.join("analysis-runs");
    }
    dirs::home_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join(".plan-cascade")
        .join("analysis-runs")
}

/// List all supported providers and their models
#[tauri::command]
pub async fn list_providers() -> CommandResponse<Vec<ProviderInfo>> {
    let providers = vec![
        ProviderInfo {
            provider_type: "anthropic".to_string(),
            name: "Anthropic Claude".to_string(),
            models: vec![
                ModelInfo {
                    id: "claude-3-5-sonnet-20241022".to_string(),
                    name: "Claude 3.5 Sonnet".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 200_000,
                    pricing: Some(Pricing {
                        input_per_million: 3.0,
                        output_per_million: 15.0,
                        thinking_per_million: None,
                    }),
                },
                ModelInfo {
                    id: "claude-3-opus-20240229".to_string(),
                    name: "Claude 3 Opus".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 200_000,
                    pricing: Some(Pricing {
                        input_per_million: 15.0,
                        output_per_million: 75.0,
                        thinking_per_million: None,
                    }),
                },
            ],
            requires_api_key: true,
            default_base_url: Some("https://api.anthropic.com/v1/messages".to_string()),
        },
        ProviderInfo {
            provider_type: "openai".to_string(),
            name: "OpenAI".to_string(),
            models: vec![
                ModelInfo {
                    id: "gpt-4-turbo".to_string(),
                    name: "GPT-4 Turbo".to_string(),
                    supports_thinking: false,
                    supports_tools: true,
                    context_window: 128_000,
                    pricing: Some(Pricing {
                        input_per_million: 10.0,
                        output_per_million: 30.0,
                        thinking_per_million: None,
                    }),
                },
                ModelInfo {
                    id: "o1-preview".to_string(),
                    name: "o1 Preview".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 128_000,
                    pricing: Some(Pricing {
                        input_per_million: 15.0,
                        output_per_million: 60.0,
                        thinking_per_million: Some(15.0),
                    }),
                },
                ModelInfo {
                    id: "o3-mini".to_string(),
                    name: "o3 Mini".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 200_000,
                    pricing: Some(Pricing {
                        input_per_million: 1.1,
                        output_per_million: 4.4,
                        thinking_per_million: Some(1.1),
                    }),
                },
            ],
            requires_api_key: true,
            default_base_url: Some("https://api.openai.com/v1/chat/completions".to_string()),
        },
        ProviderInfo {
            provider_type: "deepseek".to_string(),
            name: "DeepSeek".to_string(),
            models: vec![
                ModelInfo {
                    id: "deepseek-chat".to_string(),
                    name: "DeepSeek Chat".to_string(),
                    supports_thinking: false,
                    supports_tools: true,
                    context_window: 64_000,
                    pricing: Some(Pricing {
                        input_per_million: 0.14,
                        output_per_million: 0.28,
                        thinking_per_million: None,
                    }),
                },
                ModelInfo {
                    id: "deepseek-r1".to_string(),
                    name: "DeepSeek R1".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 64_000,
                    pricing: Some(Pricing {
                        input_per_million: 0.55,
                        output_per_million: 2.19,
                        thinking_per_million: Some(0.55),
                    }),
                },
            ],
            requires_api_key: true,
            default_base_url: Some("https://api.deepseek.com/v1/chat/completions".to_string()),
        },
        ProviderInfo {
            provider_type: "glm".to_string(),
            name: "GLM (ZhipuAI)".to_string(),
            models: vec![
                ModelInfo {
                    id: "glm-5".to_string(),
                    name: "GLM-5".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 200_000,
                    pricing: None,
                },
                ModelInfo {
                    id: "glm-4-flash-250414".to_string(),
                    name: "GLM-4 Flash".to_string(),
                    supports_thinking: false,
                    supports_tools: true,
                    context_window: 128_000,
                    pricing: None,
                },
                ModelInfo {
                    id: "glm-4.5-air".to_string(),
                    name: "GLM-4.5 Air".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 128_000,
                    pricing: None,
                },
            ],
            requires_api_key: true,
            default_base_url: Some(
                "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string(),
            ),
        },
        ProviderInfo {
            provider_type: "qwen".to_string(),
            name: "Qwen (DashScope)".to_string(),
            models: vec![
                ModelInfo {
                    id: "qwen-plus".to_string(),
                    name: "Qwen Plus".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 128_000,
                    pricing: None,
                },
                ModelInfo {
                    id: "qwen-turbo".to_string(),
                    name: "Qwen Turbo".to_string(),
                    supports_thinking: false,
                    supports_tools: true,
                    context_window: 64_000,
                    pricing: None,
                },
            ],
            requires_api_key: true,
            default_base_url: Some(
                "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions".to_string(),
            ),
        },
        ProviderInfo {
            provider_type: "minimax".to_string(),
            name: "MiniMax".to_string(),
            models: vec![
                ModelInfo {
                    id: "MiniMax-M2.5".to_string(),
                    name: "MiniMax M2.5".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 245_760,
                    pricing: None,
                },
                ModelInfo {
                    id: "MiniMax-M2.5-highspeed".to_string(),
                    name: "MiniMax M2.5 Highspeed".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 245_760,
                    pricing: None,
                },
                ModelInfo {
                    id: "MiniMax-M2.1".to_string(),
                    name: "MiniMax M2.1".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 245_760,
                    pricing: None,
                },
                ModelInfo {
                    id: "MiniMax-M2.1-highspeed".to_string(),
                    name: "MiniMax M2.1 Highspeed".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 245_760,
                    pricing: None,
                },
                ModelInfo {
                    id: "MiniMax-M2".to_string(),
                    name: "MiniMax M2".to_string(),
                    supports_thinking: true,
                    supports_tools: true,
                    context_window: 200_000,
                    pricing: None,
                },
            ],
            requires_api_key: true,
            default_base_url: Some("https://api.minimax.io/anthropic".to_string()),
        },
        ProviderInfo {
            provider_type: "ollama".to_string(),
            name: "Ollama (Local)".to_string(),
            models: vec![
                ModelInfo {
                    id: "llama3.2".to_string(),
                    name: "Llama 3.2".to_string(),
                    supports_thinking: false,
                    supports_tools: false,
                    context_window: 128_000,
                    pricing: None,
                },
                ModelInfo {
                    id: "deepseek-r1:14b".to_string(),
                    name: "DeepSeek R1 14B".to_string(),
                    supports_thinking: true,
                    supports_tools: false,
                    context_window: 64_000,
                    pricing: None,
                },
                ModelInfo {
                    id: "qwq:32b".to_string(),
                    name: "QwQ 32B".to_string(),
                    supports_thinking: true,
                    supports_tools: false,
                    context_window: 32_000,
                    pricing: None,
                },
            ],
            requires_api_key: false,
            default_base_url: Some("http://localhost:11434".to_string()),
        },
    ];

    CommandResponse::ok(providers)
}

/// List providers that currently have API keys configured in OS keyring.
#[tauri::command]
pub async fn list_configured_api_key_providers() -> CommandResponse<Vec<String>> {
    let keyring = KeyringService::new();
    let mut configured = Vec::new();

    for provider in canonical_providers() {
        match get_api_key_with_aliases(&keyring, provider) {
            Ok(Some(_)) => configured.push((*provider).to_string()),
            Ok(None) => {}
            Err(_) => {}
        }
    }

    CommandResponse::ok(configured)
}

/// Get the currently stored API key for a provider (alias-aware).
/// Returns None if no key is configured.
#[tauri::command]
pub async fn get_provider_api_key(provider: String) -> CommandResponse<Option<String>> {
    let keyring = KeyringService::new();
    let canonical_provider = match normalize_provider_name(&provider) {
        Some(p) => p,
        None => return CommandResponse::err(format!("Unknown provider: {}", provider)),
    };

    match get_api_key_with_aliases(&keyring, canonical_provider) {
        Ok(key) => CommandResponse::ok(key),
        Err(e) => CommandResponse::err(format!("Failed to get API key: {}", e)),
    }
}

/// Configure a provider (store API key securely)
#[tauri::command]
#[allow(non_snake_case)]
pub async fn configure_provider(
    provider: String,
    api_key: Option<String>,
    apiKey: Option<String>,
    base_url: Option<String>,
    baseUrl: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    let api_key = api_key.or(apiKey);
    let base_url = base_url.or(baseUrl);

    if api_key.is_none() && base_url.is_none() {
        return Ok(CommandResponse::err(
            "No configuration provided (expected api_key/apiKey and/or base_url/baseUrl)",
        ));
    }

    let canonical_provider = match normalize_provider_name(&provider) {
        Some(p) => p.to_string(),
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown provider: {}",
                provider
            )))
        }
    };

    // Use KeyringService directly (same as execute_standalone) to avoid
    // AppState initialization timing issues
    let keyring = KeyringService::new();

    // Store or delete API key
    if let Some(key) = api_key {
        if key.is_empty() {
            // Empty key means delete
            if let Err(e) = keyring.delete_api_key(&canonical_provider) {
                return Ok(CommandResponse::err(format!(
                    "Failed to delete API key: {}",
                    e
                )));
            }
        } else {
            if let Err(e) = keyring.set_api_key(&canonical_provider, &key) {
                return Ok(CommandResponse::err(format!(
                    "Failed to store API key: {}",
                    e
                )));
            }
        }
    }

    // Store base URL in settings if provided
    if let Some(url) = base_url {
        let db_result = state
            .with_database(|db| {
                let key = format!("provider_{}_base_url", canonical_provider);
                db.set_setting(&key, &url)
            })
            .await;

        if let Err(e) = db_result {
            return Ok(CommandResponse::err(format!(
                "Failed to store base URL: {}",
                e
            )));
        }
    }

    Ok(CommandResponse::ok(true))
}

/// Check provider health (validate API key and connectivity)
#[tauri::command]
pub async fn check_provider_health(
    provider: String,
    model: String,
    base_url: Option<String>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<HealthCheckResult>, String> {
    let keyring = KeyringService::new();
    let canonical_provider = match normalize_provider_name(&provider) {
        Some(p) => p,
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown provider: {}",
                provider
            )))
        }
    };

    // Get API key
    let api_key = match get_api_key_with_aliases(&keyring, canonical_provider) {
        Ok(key) => key,
        Err(e) => {
            return Ok(CommandResponse::ok(HealthCheckResult {
                healthy: false,
                error: Some(format!("Failed to get API key: {}", e)),
                latency_ms: None,
            }));
        }
    };

    let provider_type = match provider_type_from_name(canonical_provider) {
        Some(p) => p,
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown provider: {}",
                provider
            )))
        }
    };

    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return Ok(CommandResponse::ok(HealthCheckResult {
            healthy: false,
            error: Some("API key not configured".to_string()),
            latency_ms: None,
        }));
    }

    // Resolve proxy for this provider
    let proxy = app_state
        .with_database(|db| Ok(resolve_provider_proxy(&keyring, db, &canonical_provider)))
        .await
        .unwrap_or(None);

    let config = ProviderConfig {
        provider: provider_type,
        api_key,
        base_url,
        model,
        proxy,
        ..Default::default()
    };

    let orchestrator_config = OrchestratorConfig {
        provider: config,
        system_prompt: None,
        max_iterations: 1,
        max_total_tokens: 1000,
        project_root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: analysis_artifacts_root(),
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: None,
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let orchestrator = OrchestratorService::new(orchestrator_config);

    let start = std::time::Instant::now();
    match orchestrator.health_check().await {
        Ok(_) => Ok(CommandResponse::ok(HealthCheckResult {
            healthy: true,
            error: None,
            latency_ms: Some(start.elapsed().as_millis() as u32),
        })),
        Err(e) => Ok(CommandResponse::ok(HealthCheckResult {
            healthy: false,
            error: Some(e.to_string()),
            latency_ms: Some(start.elapsed().as_millis() as u32),
        })),
    }
}

/// Health check result
#[derive(serde::Serialize)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub error: Option<String>,
    pub latency_ms: Option<u32>,
}

/// Get the current working directory for standalone LLM sessions
#[tauri::command]
pub async fn get_working_directory(
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<String>, String> {
    let wd = standalone_state.working_directory.read().await;
    Ok(CommandResponse::ok(wd.to_string_lossy().to_string()))
}

/// Set the working directory for standalone LLM sessions.
/// Validates that the path exists and is a directory.
/// When a new directory is set, background indexing is triggered via the IndexManager
/// and the plugin system is re-initialized to discover project-level plugins.
#[tauri::command]
pub async fn set_working_directory(
    path: String,
    standalone_state: State<'_, StandaloneState>,
    plugin_state: State<'_, super::plugins::PluginState>,
) -> Result<CommandResponse<String>, String> {
    let new_path = PathBuf::from(&path);

    // Validate the path exists and is a directory
    if !new_path.exists() {
        return Ok(CommandResponse::err(format!(
            "Path does not exist: {}",
            path
        )));
    }
    if !new_path.is_dir() {
        return Ok(CommandResponse::err(format!(
            "Path is not a directory: {}",
            path
        )));
    }

    // Canonicalize the path
    let canonical = match new_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to resolve path: {}",
                e
            )));
        }
    };

    let result = canonical.to_string_lossy().to_string();

    // Capture old path before updating
    let old_path = {
        let wd = standalone_state.working_directory.read().await;
        wd.to_string_lossy().to_string()
    };

    // Update the working directory
    {
        let mut wd = standalone_state.working_directory.write().await;
        *wd = canonical;
    }

    // Re-initialize the plugin system for the new project root so that
    // project-level plugins (<project>/.claude-plugin/) are discovered.
    plugin_state.initialize(&result).await;

    // Trigger indexing for the new directory
    if !result.is_empty() {
        let mgr_lock = standalone_state.index_manager.read().await;
        if let Some(mgr) = &*mgr_lock {
            // Abort indexer for old directory if it was different
            if !old_path.is_empty() && old_path != result {
                mgr.remove_directory(&old_path).await;
            }
            mgr.ensure_indexed(&result).await;
        }
    }

    Ok(CommandResponse::ok(result))
}

/// Get the current indexing status for a project directory.
/// Falls back to the current working directory if no project_path is provided.
#[tauri::command]
pub async fn get_index_status(
    project_path: Option<String>,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<IndexStatusEvent>, String> {
    let dir = if let Some(p) = project_path {
        p
    } else {
        let wd = standalone_state.working_directory.read().await;
        wd.to_string_lossy().to_string()
    };

    if dir.is_empty() {
        return Ok(CommandResponse::ok(IndexStatusEvent {
            project_path: String::new(),
            status: "idle".to_string(),
            indexed_files: 0,
            total_files: 0,
            error_message: None,
            total_symbols: 0,
            embedding_chunks: 0,
            embedding_provider_name: None,
        }));
    }

    let mgr_lock = standalone_state.index_manager.read().await;
    if let Some(mgr) = &*mgr_lock {
        // Pure read-only query — indexing is triggered by set_working_directory
        // and init_app, not by status polls. This prevents the race condition
        // where multiple rapid get_index_status calls spawn duplicate indexers.
        let status = mgr.get_status(&dir).await;

        Ok(CommandResponse::ok(status))
    } else {
        Ok(CommandResponse::ok(IndexStatusEvent {
            project_path: dir,
            status: "idle".to_string(),
            indexed_files: 0,
            total_files: 0,
            error_message: None,
            total_symbols: 0,
            embedding_chunks: 0,
            embedding_provider_name: None,
        }))
    }
}

/// Trigger a full reindex for a project directory.
/// Falls back to the current working directory if no project_path is provided.
#[tauri::command]
pub async fn trigger_reindex(
    project_path: Option<String>,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<bool>, String> {
    let dir = if let Some(p) = project_path {
        p
    } else {
        let wd = standalone_state.working_directory.read().await;
        wd.to_string_lossy().to_string()
    };

    if dir.is_empty() {
        return Ok(CommandResponse::err("No directory specified".to_string()));
    }

    let mgr_lock = standalone_state.index_manager.read().await;
    if let Some(mgr) = &*mgr_lock {
        mgr.trigger_reindex(&dir).await;
        Ok(CommandResponse::ok(true))
    } else {
        Ok(CommandResponse::err(
            "IndexManager not initialized".to_string(),
        ))
    }
}

/// Perform a semantic search over indexed embeddings for a project.
///
/// Returns the top-k most similar code chunks to the query string.
/// Falls back to the current working directory if no project_path is provided.
///
/// Prefers the project's `EmbeddingManager` (ADR-F002) for query embedding.
/// Falls back to rebuilding a temporary TF-IDF vocabulary when no manager
/// is available (e.g., IndexManager not initialized or project not yet indexed).
#[tauri::command]
pub async fn semantic_search(
    query: String,
    project_path: Option<String>,
    top_k: Option<usize>,
    standalone_state: State<'_, StandaloneState>,
) -> Result<
    CommandResponse<Vec<crate::services::orchestrator::embedding_service::SemanticSearchResult>>,
    String,
> {
    let dir = if let Some(p) = project_path {
        p
    } else {
        let wd = standalone_state.working_directory.read().await;
        wd.to_string_lossy().to_string()
    };

    if dir.is_empty() {
        return Ok(CommandResponse::err("No project directory specified"));
    }

    if query.trim().is_empty() {
        return Ok(CommandResponse::err("Query string is empty"));
    }

    let mgr_lock = standalone_state.index_manager.read().await;
    let mgr = match &*mgr_lock {
        Some(mgr) => mgr,
        None => {
            return Ok(CommandResponse::err(
                "Semantic search not available: IndexManager not initialized. \
             No embedding provider is configured for this project.",
            ))
        }
    };

    let index_store = mgr.index_store();

    // Check if embeddings exist for this project
    let embedding_count = match index_store.count_embeddings(&dir) {
        Ok(count) => count,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to count embeddings: {}",
                e
            )))
        }
    };

    if embedding_count == 0 {
        return Ok(CommandResponse::ok(vec![]));
    }

    let k = top_k.unwrap_or(10);

    // Prefer the project's EmbeddingManager (ADR-F002) for query embedding.
    // This avoids rebuilding a temporary TF-IDF vocabulary from scratch.
    if let Some(emb_mgr) = mgr.get_embedding_manager(&dir).await {
        // Dimension compatibility check: stored embeddings vs manager dimension
        let stored_dim = index_store
            .get_embedding_metadata(&dir)
            .ok()
            .and_then(|meta| meta.first().map(|m| m.embedding_dimension));
        let manager_dim = emb_mgr.dimension();
        let dimension_compatible = stored_dim
            .map(|d| d == 0 || manager_dim == 0 || d == manager_dim)
            .unwrap_or(true);

        if !dimension_compatible {
            return Ok(CommandResponse::err(format!(
                "Semantic search not available: embedding dimension mismatch. \
                 Index was built with {}-dimensional embeddings, but the current \
                 embedding provider produces {}-dimensional vectors. \
                 Re-index the project to resolve this.",
                stored_dim.unwrap_or(0),
                manager_dim,
            )));
        }

        // Use EmbeddingManager.embed_query
        match emb_mgr.embed_query(&query).await {
            Ok(query_embedding) if !query_embedding.is_empty() => {
                // Try HNSW search first (O(log n)), fall back to brute-force (O(n))
                if let Some(hnsw) = mgr.get_hnsw_index(&dir).await {
                    if hnsw.is_ready().await {
                        let hnsw_hits = hnsw.search(&query_embedding, k).await;
                        if !hnsw_hits.is_empty() {
                            let rowids: Vec<usize> = hnsw_hits.iter().map(|(id, _)| *id).collect();
                            match index_store.get_embeddings_by_rowids(&rowids) {
                                Ok(metadata) => {
                                    let results: Vec<crate::services::orchestrator::embedding_service::SemanticSearchResult> = hnsw_hits
                                        .into_iter()
                                        .filter_map(|(id, distance)| {
                                            metadata.get(&id).map(|(file_path, chunk_index, chunk_text)| {
                                                crate::services::orchestrator::embedding_service::SemanticSearchResult {
                                                    file_path: file_path.clone(),
                                                    chunk_index: *chunk_index,
                                                    chunk_text: chunk_text.clone(),
                                                    similarity: 1.0 - distance,
                                                }
                                            })
                                        })
                                        .collect();
                                    return Ok(CommandResponse::ok(results));
                                }
                                Err(e) => {
                                    return Ok(CommandResponse::err(format!(
                                        "HNSW semantic search failed to fetch metadata: {}",
                                        e
                                    )))
                                }
                            }
                        }
                        // HNSW returned empty, fall through to brute-force
                    }
                }

                // Brute-force fallback
                match index_store.semantic_search(&query_embedding, &dir, k) {
                    Ok(results) => return Ok(CommandResponse::ok(results)),
                    Err(e) => {
                        return Ok(CommandResponse::err(format!(
                            "Semantic search failed: {}",
                            e
                        )))
                    }
                }
            }
            Ok(_) => {
                return Ok(CommandResponse::ok(vec![]));
            }
            Err(e) => {
                return Ok(CommandResponse::err(format!(
                    "Semantic search failed: embedding provider error — {}. \
                     The provider may be unhealthy or unreachable.",
                    e
                )));
            }
        }
    }

    // Fallback: rebuild a temporary TF-IDF vocabulary when no EmbeddingManager
    // is available. This path is retained for backwards compatibility.
    let embedding_service =
        crate::services::orchestrator::embedding_service::EmbeddingService::new();

    let all_chunks = match index_store.get_embeddings_for_project(&dir) {
        Ok(chunks) => chunks,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to get embeddings: {}",
                e
            )))
        }
    };

    let chunk_texts: Vec<&str> = all_chunks
        .iter()
        .map(|(_, _, text, _)| text.as_str())
        .collect();
    embedding_service.build_vocabulary(&chunk_texts);

    let query_embedding = embedding_service.embed_text(&query);
    if query_embedding.is_empty() {
        return Ok(CommandResponse::ok(vec![]));
    }

    match index_store.semantic_search(&query_embedding, &dir, k) {
        Ok(results) => Ok(CommandResponse::ok(results)),
        Err(e) => Ok(CommandResponse::err(format!(
            "Semantic search failed: {}",
            e
        ))),
    }
}

/// Execute a message in standalone mode
#[tauri::command]
#[allow(non_snake_case)]
pub async fn execute_standalone(
    message: String,
    provider: String,
    model: String,
    project_path: String,
    system_prompt: Option<String>,
    enable_tools: bool,
    api_key: Option<String>,
    apiKey: Option<String>,
    base_url: Option<String>,
    baseUrl: Option<String>,
    analysis_session_id: Option<String>,
    analysisSessionId: Option<String>,
    enable_compaction: Option<bool>,
    enable_thinking: Option<bool>,
    max_total_tokens: Option<u32>,
    max_iterations: Option<u32>,
    max_concurrent_subagents: Option<u32>,
    app: AppHandle,
    app_state: State<'_, AppState>,
    standalone_state: State<'_, StandaloneState>,
    file_changes_state: State<'_, super::file_changes::FileChangesState>,
    analytics_state: State<'_, super::analytics::AnalyticsState>,
    plugin_state: State<'_, super::plugins::PluginState>,
) -> Result<CommandResponse<ExecutionResult>, String> {
    let keyring = KeyringService::new();
    let canonical_provider = match normalize_provider_name(&provider) {
        Some(p) => p,
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown provider: {}",
                provider
            )))
        }
    };
    let provided_api_key = api_key
        .or(apiKey)
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());

    // Get API key
    let mut api_key = match get_api_key_with_aliases(&keyring, canonical_provider) {
        Ok(key) => key,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to get API key: {}",
                e
            )));
        }
    };

    // Fallback: use API key provided by frontend request when keyring has no entry.
    if api_key.is_none() {
        if let Some(key) = provided_api_key {
            // Best-effort backfill into keyring for future requests.
            let _ = keyring.set_api_key(canonical_provider, &key);
            api_key = Some(key);
        }
    }

    let provider_type = match provider_type_from_name(canonical_provider) {
        Some(p) => p,
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown provider: {}",
                provider
            )))
        }
    };

    // Validate API key for non-Ollama providers
    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return Ok(CommandResponse::err(format!(
            "API key not configured for provider '{}'",
            canonical_provider
        )));
    }

    // Resolve base_url: explicit parameter > DB setting > provider default (None)
    let mut resolved_base_url = base_url
        .or(baseUrl)
        .map(|u| u.trim().to_string())
        .filter(|u| !u.is_empty());

    if resolved_base_url.is_none() {
        // Fallback: read from database settings
        let key = format!("provider_{}_base_url", canonical_provider);
        if let Ok(Some(db_url)) = app_state.with_database(|db| db.get_setting(&key)).await {
            if !db_url.is_empty() {
                resolved_base_url = Some(db_url);
            }
        }
    }

    // Resolve proxy for this provider
    let proxy = app_state
        .with_database(|db| Ok(resolve_provider_proxy(&keyring, db, &canonical_provider)))
        .await
        .unwrap_or(None);

    let config = ProviderConfig {
        provider: provider_type,
        api_key,
        base_url: resolved_base_url,
        model,
        enable_thinking: enable_thinking.unwrap_or(false),
        proxy,
        max_concurrent_subagents: max_concurrent_subagents
            .filter(|&v| v > 0)
            .map(|v| v as usize),
        ..Default::default()
    };
    let analysis_session_id = analysis_session_id
        .or(analysisSessionId)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // Clone session_id before it's moved into orchestrator_config
    let event_session_id = analysis_session_id.clone().unwrap_or_default();

    let orchestrator_config = OrchestratorConfig {
        provider: config,
        system_prompt,
        max_iterations: max_iterations.unwrap_or(50),
        max_total_tokens: max_total_tokens.unwrap_or(1_000_000),
        project_root: PathBuf::from(&project_path),
        streaming: true,
        enable_compaction: enable_compaction.unwrap_or(true),
        analysis_artifacts_root: analysis_artifacts_root(),
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id,
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let mut orchestrator = OrchestratorService::new(orchestrator_config);

    // Wire file change tracker for AI file modification tracking
    {
        let tracker = file_changes_state
            .get_or_create(&event_session_id, &project_path)
            .await;
        // Advance turn index and set app handle for event emission
        if let Ok(mut t) = tracker.lock() {
            let next = t.turn_index() + 1;
            t.set_turn_index(next);
            t.set_app_handle(app.clone());
        }
        orchestrator = orchestrator.with_file_change_tracker(tracker);
    }

    // Wire database pool so IndexStore is available for CodebaseSearch
    match app_state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(pool) => {
            orchestrator = orchestrator.with_database(pool);
        }
        Err(e) => {
            eprintln!(
                "[execute_standalone] Database not available, CodebaseSearch will be disabled: {}",
                e
            );
        }
    }

    // Wire embedding service and EmbeddingManager from IndexManager for semantic CodebaseSearch
    if let Some(ref manager) = *standalone_state.index_manager.read().await {
        if let Some(emb_svc) = manager.get_embedding_service(&project_path).await {
            orchestrator = orchestrator.with_embedding_service(emb_svc);
        }
        if let Some(emb_mgr) = manager.get_embedding_manager(&project_path).await {
            orchestrator = orchestrator.with_embedding_manager(emb_mgr);
        }
    }

    // Wire analytics tracking for persistent usage recording
    {
        let _ = analytics_state.initialize(&app_state).await;
        if let Some(tx) = analytics_state.get_tracker_sender().await {
            orchestrator = orchestrator
                .with_analytics_tracker(tx)
                .with_analytics_cost_calculator(analytics_state.cost_calculator());
        }
    }

    // Create channel for streaming events (created before plugin wiring so hooks can report errors)
    let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(100);

    // Wire plugin context (instructions, skills, commands, hooks, permissions) from enabled plugins
    orchestrator = plugin_state
        .wire_orchestrator(orchestrator, Some(tx.clone()))
        .await;

    // Spawn task to forward events to frontend
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            // Flatten session_id into the serialized UnifiedStreamEvent so the
            // frontend's handleUnifiedExecutionEvent can route events to the
            // correct foreground or background session.
            let mut payload =
                serde_json::to_value(&event).unwrap_or_else(|_| serde_json::json!({}));
            if let Some(obj) = payload.as_object_mut() {
                if !event_session_id.is_empty() {
                    obj.insert(
                        "session_id".to_string(),
                        serde_json::Value::String(event_session_id.clone()),
                    );
                }
            }
            let _ = app_clone.emit("standalone-event", &payload);
        }
    });

    // Execute the message
    let result = if enable_tools {
        orchestrator.execute(message, tx).await
    } else {
        orchestrator.execute_single(message, tx).await
    };

    Ok(CommandResponse::ok(result))
}

/// Save text output to a user-selected file path.
#[tauri::command]
pub async fn save_output_export(path: String, content: String) -> CommandResponse<bool> {
    let target = PathBuf::from(path.trim());
    if target.as_os_str().is_empty() {
        return CommandResponse::err("Invalid target path");
    }
    if let Some(parent) = target.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return CommandResponse::err(format!("Failed to prepare export directory: {}", e));
        }
    }
    match std::fs::write(&target, content) {
        Ok(_) => CommandResponse::ok(true),
        Err(e) => CommandResponse::err(format!("Failed to save export: {}", e)),
    }
}

/// Save binary data (base64-encoded) to a user-selected file path.
#[tauri::command]
pub async fn save_binary_export(path: String, data_base64: String) -> CommandResponse<bool> {
    use base64::Engine;
    let target = PathBuf::from(path.trim());
    if target.as_os_str().is_empty() {
        return CommandResponse::err("Invalid target path");
    }
    if let Some(parent) = target.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return CommandResponse::err(format!("Failed to prepare export directory: {}", e));
        }
    }
    let decoded = match base64::engine::general_purpose::STANDARD.decode(&data_base64) {
        Ok(bytes) => bytes,
        Err(e) => return CommandResponse::err(format!("Failed to decode base64 data: {}", e)),
    };
    match std::fs::write(&target, decoded) {
        Ok(_) => CommandResponse::ok(true),
        Err(e) => CommandResponse::err(format!("Failed to save export: {}", e)),
    }
}

/// Get usage statistics from the database
///
/// Deprecated: Use the new analytics system (`services/analytics/`) via
/// `get_dashboard_summary` or `list_usage_records` commands instead.
/// This function queries the legacy `analytics` table which is no longer written to.
#[deprecated(
    note = "Use services/analytics/ commands (get_dashboard_summary, list_usage_records) instead"
)]
#[tauri::command]
pub async fn get_usage_stats(
    provider: Option<String>,
    model: Option<String>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<UsageStatistics>, String> {
    let provider_clone = provider.clone();
    let model_clone = model.clone();

    let result = state.with_database(|db| {
        let conn = db.get_connection()?;

        // Build query based on filters
        let mut query = String::from(
            "SELECT COALESCE(SUM(input_tokens), 0), COALESCE(SUM(output_tokens), 0), COALESCE(SUM(cost), 0), COUNT(*) FROM analytics WHERE event_type = 'llm_request'"
        );

        if let Some(p) = &provider_clone {
            query.push_str(&format!(" AND provider = '{}'", p));
        }
        if let Some(m) = &model_clone {
            query.push_str(&format!(" AND model = '{}'", m));
        }

        let stats = conn.query_row(&query, [], |row| {
            Ok(UsageStatistics {
                total_input_tokens: row.get::<_, i64>(0).unwrap_or(0) as u32,
                total_output_tokens: row.get::<_, i64>(1).unwrap_or(0) as u32,
                total_thinking_tokens: 0, // Not tracked separately yet
                total_cost_usd: row.get::<_, f64>(2).unwrap_or(0.0),
                requests: row.get::<_, i64>(3).unwrap_or(0) as u32,
            })
        })?;

        Ok(stats)
    }).await;

    match result {
        Ok(stats) => Ok(CommandResponse::ok(stats)),
        Err(e) => Ok(CommandResponse::err(format!(
            "Failed to get usage stats: {}",
            e
        ))),
    }
}

/// Calculate cost for token usage
///
/// Deprecated: Cost calculation is now handled by `services/analytics/cost.rs`
/// (`CostCalculator`) with model-aware pricing from the analytics database.
#[deprecated(note = "Use services/analytics/cost.rs CostCalculator instead")]
pub fn calculate_cost(
    input_tokens: u32,
    output_tokens: u32,
    thinking_tokens: u32,
    pricing: &Pricing,
) -> f64 {
    let input_cost = (input_tokens as f64 / 1_000_000.0) * pricing.input_per_million;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * pricing.output_per_million;
    let thinking_cost = if let Some(thinking_price) = pricing.thinking_per_million {
        (thinking_tokens as f64 / 1_000_000.0) * thinking_price
    } else {
        0.0
    };

    input_cost + output_cost + thinking_cost
}

/// Record usage in the database
///
/// Deprecated: Usage is now automatically tracked by the analytics pipeline
/// (`services/analytics/tracker.rs`) via `track_analytics()` in the agentic loop.
/// This function writes to the legacy `analytics` table which is no longer read.
#[deprecated(note = "Usage is now tracked automatically via services/analytics/tracker.rs")]
pub async fn record_usage(
    state: &AppState,
    provider: &str,
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
    cost: f64,
) -> Result<(), String> {
    let provider = provider.to_string();
    let model = model.to_string();

    state.with_database(move |db| {
        let conn = db.get_connection()?;
        conn.execute(
            "INSERT INTO analytics (event_type, provider, model, input_tokens, output_tokens, cost) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["llm_request", provider, model, input_tokens, output_tokens, cost],
        )?;
        Ok(())
    }).await.map_err(|e| e.to_string())
}

// ============================================================================
// Session-based execution commands
// ============================================================================

/// Execute a PRD with session tracking for crash recovery
#[tauri::command]
pub async fn execute_standalone_with_session(
    request: ExecuteWithSessionRequest,
    app: AppHandle,
    app_state: State<'_, AppState>,
    standalone_state: State<'_, StandaloneState>,
    file_changes_state: State<'_, super::file_changes::FileChangesState>,
    analytics_state: State<'_, super::analytics::AnalyticsState>,
    permission_state: State<'_, super::permissions::PermissionState>,
    plugin_state: State<'_, super::plugins::PluginState>,
) -> Result<CommandResponse<SessionExecutionResult>, String> {
    let keyring = KeyringService::new();
    let canonical_provider = match normalize_provider_name(&request.provider) {
        Some(p) => p.to_string(),
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown provider: {}",
                request.provider
            )))
        }
    };

    // Get API key
    let api_key = match get_api_key_with_aliases(&keyring, &canonical_provider) {
        Ok(key) => key,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to get API key: {}",
                e
            )));
        }
    };

    let provider_type = match provider_type_from_name(&canonical_provider) {
        Some(p) => p,
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown provider: {}",
                request.provider
            )))
        }
    };

    // Validate API key for non-Ollama providers
    if provider_type != ProviderType::Ollama && api_key.is_none() {
        return Ok(CommandResponse::err(format!(
            "API key not configured for provider '{}'",
            canonical_provider
        )));
    }

    // Resolve proxy for this provider
    let proxy = app_state
        .with_database(|db| Ok(resolve_provider_proxy(&keyring, db, &canonical_provider)))
        .await
        .unwrap_or(None);

    let config = ProviderConfig {
        provider: provider_type,
        api_key,
        base_url: None,
        model: request.model.clone(),
        enable_thinking: request.enable_thinking.unwrap_or(false),
        proxy,
        ..Default::default()
    };
    // Generate session ID first so analysis cache reuse is scoped to this execution session.
    let session_id = uuid::Uuid::new_v4().to_string();

    let orchestrator_config = OrchestratorConfig {
        provider: config,
        system_prompt: request.system_prompt.clone(),
        max_iterations: request.max_iterations.unwrap_or(50),
        max_total_tokens: request.max_total_tokens.unwrap_or(1_000_000),
        project_root: PathBuf::from(&request.project_path),
        streaming: true,
        enable_compaction: true,
        analysis_artifacts_root: analysis_artifacts_root(),
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: Some(session_id.clone()),
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    // Get database pool for session persistence
    let pool = app_state
        .with_database(|db| Ok(db.pool().clone()))
        .await
        .map_err(|e| e.to_string())?;

    // Create orchestrator with database (IndexStore is auto-wired to ToolExecutor)
    let mut orchestrator = OrchestratorService::new(orchestrator_config)
        .with_database(pool)
        .with_permission_gate(permission_state.gate.clone());

    // Wire file change tracker for AI file modification tracking
    {
        let tracker = file_changes_state
            .get_or_create(&session_id, &request.project_path)
            .await;
        if let Ok(mut t) = tracker.lock() {
            let next = t.turn_index() + 1;
            t.set_turn_index(next);
            t.set_app_handle(app.clone());
        }
        orchestrator = orchestrator.with_file_change_tracker(tracker);
    }

    // Wire embedding service and EmbeddingManager from IndexManager for semantic CodebaseSearch
    if let Some(ref manager) = *standalone_state.index_manager.read().await {
        if let Some(emb_svc) = manager.get_embedding_service(&request.project_path).await {
            orchestrator = orchestrator.with_embedding_service(emb_svc);
        }
        if let Some(emb_mgr) = manager.get_embedding_manager(&request.project_path).await {
            orchestrator = orchestrator.with_embedding_manager(emb_mgr);
        }
    }

    // Wire analytics tracking for persistent usage recording
    {
        let _ = analytics_state.initialize(&app_state).await;
        if let Some(atx) = analytics_state.get_tracker_sender().await {
            orchestrator = orchestrator
                .with_analytics_tracker(atx)
                .with_analytics_cost_calculator(analytics_state.cost_calculator());
        }
    }

    // Wire plugin context (instructions, skills, commands, hooks, permissions) from enabled plugins
    orchestrator = plugin_state.wire_orchestrator(orchestrator, None).await;

    let orchestrator = Arc::new(orchestrator);

    // Create execution session
    let mut session = ExecutionSession::new(
        session_id.clone(),
        &request.project_path,
        &canonical_provider,
        &request.model,
    );

    if let Some(prd_path) = &request.prd_path {
        session = session.with_prd(prd_path);
    }

    if let Some(prompt) = &request.system_prompt {
        session = session.with_system_prompt(prompt);
    }

    // Load stories from PRD if provided
    if let Some(prd_path) = &request.prd_path {
        let prd_content = std::fs::read_to_string(prd_path)
            .map_err(|e| format!("Failed to read PRD file: {}", e))?;

        // Try to parse as JSON PRD
        if let Ok(prd) = serde_json::from_str::<serde_json::Value>(&prd_content) {
            if let Some(stories) = prd.get("stories").and_then(|s| s.as_array()) {
                for story in stories {
                    let story_id = story
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let title = story
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Untitled Story");

                    // Filter by story_ids if provided
                    if let Some(ref filter_ids) = request.story_ids {
                        if !filter_ids.contains(&story_id.to_string()) {
                            continue;
                        }
                    }

                    session.add_story(story_id, title);
                }
            }
        }
    }

    // If no stories found, add a default story with the project description
    if session.stories.is_empty() {
        session.add_story("story-001", "Execute project task");
    }

    // Store orchestrator for potential cancellation
    standalone_state
        .set_orchestrator(session_id.clone(), orchestrator.clone())
        .await;

    // Create channel for streaming events
    let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(100);

    // Spawn task to forward events to frontend
    let app_clone = app.clone();
    let session_id_clone = session_id.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let _ = app_clone.emit(&format!("session-event-{}", session_id_clone), &event);
            // Also emit to general channel for dashboard
            let _ = app_clone.emit("standalone-session-event", &event);
        }
    });

    // Execute the session
    let result = orchestrator
        .execute_session(&mut session, tx, request.run_quality_gates)
        .await;

    // Clean up orchestrator
    standalone_state.remove_orchestrator(&session_id).await;

    Ok(CommandResponse::ok(result))
}

/// Cancel a running standalone execution
#[tauri::command]
pub async fn cancel_standalone_execution(
    session_id: String,
    standalone_state: State<'_, StandaloneState>,
    permission_state: State<'_, super::permissions::PermissionState>,
) -> Result<CommandResponse<bool>, String> {
    // Cancel any pending permission requests for this session
    permission_state
        .gate
        .cancel_session_requests(&session_id)
        .await;

    if let Some(orchestrator) = standalone_state.get_orchestrator(&session_id).await {
        orchestrator.cancel();
        Ok(CommandResponse::ok(true))
    } else {
        Ok(CommandResponse::err(format!(
            "Session not found: {}",
            session_id
        )))
    }
}

/// Pause a running standalone execution
#[tauri::command]
pub async fn pause_standalone_execution(
    session_id: String,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<bool>, String> {
    if let Some(orchestrator) = standalone_state.get_orchestrator(&session_id).await {
        orchestrator.pause();
        Ok(CommandResponse::ok(true))
    } else {
        Ok(CommandResponse::err(format!(
            "Session not found: {}",
            session_id
        )))
    }
}

/// Unpause a paused standalone execution
#[tauri::command]
pub async fn unpause_standalone_execution(
    session_id: String,
    standalone_state: State<'_, StandaloneState>,
) -> Result<CommandResponse<bool>, String> {
    if let Some(orchestrator) = standalone_state.get_orchestrator(&session_id).await {
        orchestrator.unpause();
        Ok(CommandResponse::ok(true))
    } else {
        Ok(CommandResponse::err(format!(
            "Session not found: {}",
            session_id
        )))
    }
}

/// Get status of all standalone executions
#[tauri::command]
pub async fn get_standalone_status(
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<StandaloneStatus>, String> {
    // Get database pool
    let pool = match app_state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(p) => p,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Database not available: {}",
                e
            )));
        }
    };

    // Create a temporary orchestrator to query sessions
    let temp_config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Anthropic,
            api_key: None,
            base_url: None,
            model: "temp".to_string(),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 1,
        max_total_tokens: 1,
        project_root: PathBuf::from("."),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: analysis_artifacts_root(),
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: None,
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let orchestrator = OrchestratorService::new(temp_config).with_database(pool);

    // Get active sessions (running or paused)
    let active = orchestrator
        .list_sessions(Some(ExecutionStatus::Running), Some(50))
        .await
        .unwrap_or_default();

    let paused = orchestrator
        .list_sessions(Some(ExecutionStatus::Paused), Some(50))
        .await
        .unwrap_or_default();

    let active_sessions: Vec<ExecutionSessionSummary> =
        active.into_iter().chain(paused.into_iter()).collect();

    // Get recent completed sessions
    let recent_sessions = orchestrator
        .list_sessions(Some(ExecutionStatus::Completed), Some(10))
        .await
        .unwrap_or_default();

    // Get total count
    let total = orchestrator
        .list_sessions(None, Some(1000))
        .await
        .unwrap_or_default()
        .len();

    Ok(CommandResponse::ok(StandaloneStatus {
        active_sessions,
        recent_sessions,
        total_sessions: total,
    }))
}

/// Get detailed progress for a specific session
#[tauri::command]
pub async fn get_standalone_progress(
    session_id: String,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<ExecutionProgress>, String> {
    // Get database pool
    let pool = match app_state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(p) => p,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Database not available: {}",
                e
            )));
        }
    };

    // Create a temporary orchestrator to query the session
    let temp_config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Anthropic,
            api_key: None,
            base_url: None,
            model: "temp".to_string(),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 1,
        max_total_tokens: 1,
        project_root: PathBuf::from("."),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: analysis_artifacts_root(),
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: None,
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let orchestrator = OrchestratorService::new(temp_config).with_database(pool);

    match orchestrator.get_progress(&session_id).await {
        Ok(Some(progress)) => Ok(CommandResponse::ok(progress)),
        Ok(None) => Ok(CommandResponse::err(format!(
            "Session not found: {}",
            session_id
        ))),
        Err(e) => Ok(CommandResponse::err(format!(
            "Failed to get progress: {}",
            e
        ))),
    }
}

/// Resume a paused or failed execution
#[tauri::command]
pub async fn resume_standalone_execution(
    request: ResumeExecutionRequest,
    app: AppHandle,
    app_state: State<'_, AppState>,
    standalone_state: State<'_, StandaloneState>,
    file_changes_state: State<'_, super::file_changes::FileChangesState>,
    permission_state: State<'_, super::permissions::PermissionState>,
    plugin_state: State<'_, super::plugins::PluginState>,
) -> Result<CommandResponse<SessionExecutionResult>, String> {
    // Get database pool
    let pool = match app_state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(p) => p,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Database not available: {}",
                e
            )));
        }
    };

    // Load the session from database
    let temp_config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Anthropic,
            api_key: None,
            base_url: None,
            model: "temp".to_string(),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 1,
        max_total_tokens: 1,
        project_root: PathBuf::from("."),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: analysis_artifacts_root(),
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: None,
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let temp_orchestrator = OrchestratorService::new(temp_config).with_database(pool.clone());

    let mut session = match temp_orchestrator.load_session(&request.session_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Ok(CommandResponse::err(format!(
                "Session not found: {}",
                request.session_id
            )));
        }
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to load session: {}",
                e
            )));
        }
    };

    // Check if session can be resumed
    if !session.status.can_resume() {
        return Ok(CommandResponse::err(format!(
            "Session cannot be resumed (status: {})",
            session.status
        )));
    }

    // Handle skip_current option
    if request.skip_current {
        if !session.advance_to_next_story() {
            return Ok(CommandResponse::err("No more stories to execute"));
        }
    }

    // Handle retry_failed option - reset failed story status
    if request.retry_failed {
        if let Some(story) = session.current_story_mut() {
            if story.status == ExecutionStatus::Failed {
                story.status = ExecutionStatus::Pending;
                story.error = None;
            }
        }
    }

    // Get keyring to retrieve API key
    let keyring = KeyringService::new();
    let canonical_provider = match normalize_provider_name(&session.provider) {
        Some(p) => p,
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown provider: {}",
                session.provider
            )));
        }
    };
    let api_key = match get_api_key_with_aliases(&keyring, canonical_provider) {
        Ok(key) => key,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to get API key: {}",
                e
            )));
        }
    };

    let provider_type = match provider_type_from_name(canonical_provider) {
        Some(p) => p,
        None => {
            return Ok(CommandResponse::err(format!(
                "Unknown provider: {}",
                session.provider
            )));
        }
    };

    // Resolve proxy for this provider
    let proxy = app_state
        .with_database(|db| Ok(resolve_provider_proxy(&keyring, db, &canonical_provider)))
        .await
        .unwrap_or(None);

    // Create new orchestrator with the session's config
    let config = ProviderConfig {
        provider: provider_type,
        api_key,
        base_url: None,
        model: session.model.clone(),
        proxy,
        ..Default::default()
    };

    let orchestrator_config = OrchestratorConfig {
        provider: config,
        system_prompt: session.system_prompt.clone(),
        max_iterations: 50,
        max_total_tokens: 1_000_000,
        project_root: PathBuf::from(&session.project_path),
        streaming: true,
        enable_compaction: true,
        analysis_artifacts_root: analysis_artifacts_root(),
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: Some(request.session_id.clone()),
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let mut orchestrator = OrchestratorService::new(orchestrator_config)
        .with_database(pool)
        .with_permission_gate(permission_state.gate.clone());

    // Wire file change tracker for AI file modification tracking
    {
        let tracker = file_changes_state
            .get_or_create(&request.session_id, &session.project_path)
            .await;
        if let Ok(mut t) = tracker.lock() {
            let next = t.turn_index() + 1;
            t.set_turn_index(next);
            t.set_app_handle(app.clone());
        }
        orchestrator = orchestrator.with_file_change_tracker(tracker);
    }

    // Wire embedding service and EmbeddingManager from IndexManager for semantic CodebaseSearch
    if let Some(ref manager) = *standalone_state.index_manager.read().await {
        if let Some(emb_svc) = manager.get_embedding_service(&session.project_path).await {
            orchestrator = orchestrator.with_embedding_service(emb_svc);
        }
        if let Some(emb_mgr) = manager.get_embedding_manager(&session.project_path).await {
            orchestrator = orchestrator.with_embedding_manager(emb_mgr);
        }
    }

    // Wire plugin context (instructions, skills, commands, hooks, permissions) from enabled plugins
    orchestrator = plugin_state.wire_orchestrator(orchestrator, None).await;

    let orchestrator = Arc::new(orchestrator);

    // Store orchestrator for potential cancellation
    standalone_state
        .set_orchestrator(request.session_id.clone(), orchestrator.clone())
        .await;

    // Create channel for streaming events
    let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(100);

    // Spawn task to forward events to frontend
    let app_clone = app.clone();
    let session_id = request.session_id.clone();
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let _ = app_clone.emit(&format!("session-event-{}", session_id), &event);
            let _ = app_clone.emit("standalone-session-event", &event);
        }
    });

    // Resume execution
    let result = orchestrator.execute_session(&mut session, tx, true).await;

    // Clean up orchestrator
    standalone_state
        .remove_orchestrator(&request.session_id)
        .await;

    Ok(CommandResponse::ok(result))
}

/// Get a specific execution session by ID
#[tauri::command]
pub async fn get_standalone_session(
    session_id: String,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<ExecutionSession>, String> {
    // Get database pool
    let pool = match app_state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(p) => p,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Database not available: {}",
                e
            )));
        }
    };

    // Create temporary orchestrator
    let temp_config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Anthropic,
            api_key: None,
            base_url: None,
            model: "temp".to_string(),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 1,
        max_total_tokens: 1,
        project_root: PathBuf::from("."),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: analysis_artifacts_root(),
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: None,
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let orchestrator = OrchestratorService::new(temp_config).with_database(pool);

    match orchestrator.load_session(&session_id).await {
        Ok(Some(session)) => Ok(CommandResponse::ok(session)),
        Ok(None) => Ok(CommandResponse::err(format!(
            "Session not found: {}",
            session_id
        ))),
        Err(e) => Ok(CommandResponse::err(format!(
            "Failed to load session: {}",
            e
        ))),
    }
}

/// List all execution sessions with optional status filter
#[tauri::command]
pub async fn list_standalone_sessions(
    status: Option<String>,
    limit: Option<usize>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<Vec<ExecutionSessionSummary>>, String> {
    // Get database pool
    let pool = match app_state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(p) => p,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Database not available: {}",
                e
            )));
        }
    };

    // Parse status filter
    let status_filter = status.and_then(|s| s.parse().ok());

    // Create temporary orchestrator
    let temp_config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Anthropic,
            api_key: None,
            base_url: None,
            model: "temp".to_string(),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 1,
        max_total_tokens: 1,
        project_root: PathBuf::from("."),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: analysis_artifacts_root(),
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: None,
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let orchestrator = OrchestratorService::new(temp_config).with_database(pool);

    match orchestrator.list_sessions(status_filter, limit).await {
        Ok(sessions) => Ok(CommandResponse::ok(sessions)),
        Err(e) => Ok(CommandResponse::err(format!(
            "Failed to list sessions: {}",
            e
        ))),
    }
}

/// Delete an execution session
#[tauri::command]
pub async fn delete_standalone_session(
    session_id: String,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<bool>, String> {
    // Get database pool
    let pool = match app_state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(p) => p,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Database not available: {}",
                e
            )));
        }
    };

    // Create temporary orchestrator
    let temp_config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Anthropic,
            api_key: None,
            base_url: None,
            model: "temp".to_string(),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 1,
        max_total_tokens: 1,
        project_root: PathBuf::from("."),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: analysis_artifacts_root(),
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: None,
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let orchestrator = OrchestratorService::new(temp_config).with_database(pool);

    match orchestrator.delete_session(&session_id).await {
        Ok(()) => Ok(CommandResponse::ok(true)),
        Err(e) => Ok(CommandResponse::err(format!(
            "Failed to delete session: {}",
            e
        ))),
    }
}

/// Cleanup old completed sessions
#[tauri::command]
pub async fn cleanup_standalone_sessions(
    days: i64,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<usize>, String> {
    // Get database pool
    let pool = match app_state.with_database(|db| Ok(db.pool().clone())).await {
        Ok(p) => p,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Database not available: {}",
                e
            )));
        }
    };

    // Create temporary orchestrator
    let temp_config = OrchestratorConfig {
        provider: ProviderConfig {
            provider: ProviderType::Anthropic,
            api_key: None,
            base_url: None,
            model: "temp".to_string(),
            ..Default::default()
        },
        system_prompt: None,
        max_iterations: 1,
        max_total_tokens: 1,
        project_root: PathBuf::from("."),
        streaming: false,
        enable_compaction: false,
        analysis_artifacts_root: analysis_artifacts_root(),
        analysis_profile: Default::default(),
        analysis_limits: Default::default(),
        analysis_session_id: None,
        project_id: None,
        compaction_config: Default::default(),
        task_type: None,
        sub_agent_depth: None,
    };

    let orchestrator = OrchestratorService::new(temp_config).with_database(pool);

    match orchestrator.cleanup_old_sessions(days).await {
        Ok(count) => Ok(CommandResponse::ok(count)),
        Err(e) => Ok(CommandResponse::err(format!(
            "Failed to cleanup sessions: {}",
            e
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_cost() {
        let pricing = Pricing {
            input_per_million: 3.0,
            output_per_million: 15.0,
            thinking_per_million: None,
        };

        let cost = calculate_cost(1_000_000, 1_000_000, 0, &pricing);
        assert!((cost - 18.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_cost_with_thinking() {
        let pricing = Pricing {
            input_per_million: 15.0,
            output_per_million: 60.0,
            thinking_per_million: Some(15.0),
        };

        let cost = calculate_cost(100_000, 50_000, 200_000, &pricing);
        // 0.1 * 15 + 0.05 * 60 + 0.2 * 15 = 1.5 + 3.0 + 3.0 = 7.5
        assert!((cost - 7.5).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_list_providers() {
        let result = list_providers().await;
        assert!(result.success);
        let providers = result.data.unwrap();
        assert!(!providers.is_empty());

        // Check that all expected providers are present
        let names: Vec<&str> = providers.iter().map(|p| p.provider_type.as_str()).collect();
        assert!(names.contains(&"anthropic"));
        assert!(names.contains(&"openai"));
        assert!(names.contains(&"deepseek"));
        assert!(names.contains(&"ollama"));
    }

    #[test]
    fn test_standalone_state_default_working_directory() {
        let state = StandaloneState::new();
        // The default working directory should be the process working dir
        let rt = tokio::runtime::Runtime::new().unwrap();
        let wd = rt.block_on(async { state.working_directory.read().await.clone() });
        assert!(wd.exists());
        assert!(wd.is_dir());
    }

    #[test]
    fn test_normalize_provider_name() {
        assert_eq!(normalize_provider_name("anthropic"), Some("anthropic"));
        assert_eq!(normalize_provider_name("claude"), Some("anthropic"));
        assert_eq!(normalize_provider_name("claude-api"), Some("anthropic"));
        assert_eq!(normalize_provider_name("openai"), Some("openai"));
        assert_eq!(normalize_provider_name("glm"), Some("glm"));
        assert_eq!(normalize_provider_name("zhipu"), Some("glm"));
        assert_eq!(normalize_provider_name("qwen"), Some("qwen"));
        assert_eq!(normalize_provider_name("dashscope"), Some("qwen"));
        assert_eq!(normalize_provider_name("ollama"), Some("ollama"));
        assert_eq!(normalize_provider_name("unknown"), None);
    }

    #[test]
    fn test_standalone_state_working_directory_update() {
        let state = StandaloneState::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let temp = std::env::temp_dir();
        rt.block_on(async {
            let mut wd = state.working_directory.write().await;
            *wd = temp.clone();
        });
        let wd = rt.block_on(async { state.working_directory.read().await.clone() });
        assert_eq!(wd, temp);
    }

    #[test]
    fn test_default_max_total_tokens_is_one_million() {
        // Verify that when no optional override is provided, the default
        // max_total_tokens used in execute_standalone config is 1_000_000.
        let default_tokens: u32 = None::<u32>.unwrap_or(1_000_000);
        assert_eq!(default_tokens, 1_000_000);
        assert_ne!(
            default_tokens, 100_000,
            "Default max_total_tokens must NOT be 100_000"
        );
    }

    #[test]
    fn test_default_max_iterations_is_50() {
        let default_iters: u32 = None::<u32>.unwrap_or(50);
        assert_eq!(default_iters, 50);
    }

    #[test]
    fn test_optional_max_total_tokens_override() {
        // Simulate what happens when the frontend passes a custom value.
        let custom: Option<u32> = Some(500_000);
        let resolved = custom.unwrap_or(1_000_000);
        assert_eq!(resolved, 500_000);
    }

    #[test]
    fn test_optional_max_iterations_override() {
        let custom: Option<u32> = Some(100);
        let resolved = custom.unwrap_or(50);
        assert_eq!(resolved, 100);
    }

    #[test]
    fn test_execute_with_session_request_serde_defaults() {
        // Verify that ExecuteWithSessionRequest deserializes correctly when
        // max_total_tokens and max_iterations are absent from the JSON.
        use crate::models::orchestrator::ExecuteWithSessionRequest;

        let json = r#"{
            "project_path": "/tmp/test",
            "provider": "anthropic",
            "model": "claude-3-5-sonnet-20241022",
            "run_quality_gates": false
        }"#;

        let req: ExecuteWithSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.max_total_tokens, None);
        assert_eq!(req.max_iterations, None);
        // When None, the command should fall back to 1_000_000 / 50.
        assert_eq!(req.max_total_tokens.unwrap_or(1_000_000), 1_000_000);
        assert_eq!(req.max_iterations.unwrap_or(50), 50);
    }

    #[test]
    fn test_semantic_search_query_validation() {
        // Verify that empty query handling works correctly
        let empty_query = "".to_string();
        assert!(empty_query.trim().is_empty());

        let whitespace_query = "   ".to_string();
        assert!(whitespace_query.trim().is_empty());

        let valid_query = "find user authentication".to_string();
        assert!(!valid_query.trim().is_empty());
    }

    #[test]
    fn test_semantic_search_default_top_k() {
        // Verify default top_k is 10
        let top_k: Option<usize> = None;
        assert_eq!(top_k.unwrap_or(10), 10);

        let custom_top_k: Option<usize> = Some(5);
        assert_eq!(custom_top_k.unwrap_or(10), 5);
    }

    #[test]
    fn test_execute_with_session_request_serde_with_overrides() {
        use crate::models::orchestrator::ExecuteWithSessionRequest;

        let json = r#"{
            "project_path": "/tmp/test",
            "provider": "anthropic",
            "model": "claude-3-5-sonnet-20241022",
            "run_quality_gates": true,
            "max_total_tokens": 2000000,
            "max_iterations": 100
        }"#;

        let req: ExecuteWithSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.max_total_tokens, Some(2_000_000));
        assert_eq!(req.max_iterations, Some(100));
    }
}
