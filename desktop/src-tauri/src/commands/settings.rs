//! Settings Commands
//!
//! Commands for reading and updating application settings.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::app_shell::{apply_runtime_preferences, AppShellState};
use crate::commands::webhook::WebhookState;
use crate::models::export::{SettingsImportResult, UnifiedSettingsExport};
use crate::models::response::CommandResponse;
use crate::models::settings::{AppConfig, SettingsUpdate};
use crate::services::settings_export;
use crate::state::AppState;

const KB_QUERY_RUNS_V2_FLAG: &str = "kb_query_runs_v2";
const KB_PICKER_SERVER_SEARCH_FLAG: &str = "kb_picker_server_search";
const KB_INGEST_JOB_SCOPED_PROGRESS_FLAG: &str = "kb_ingest_job_scoped_progress";
const LSP_PREFERENCES_KEY: &str = "lsp_preferences_v1";
const DEFAULT_LSP_DEBOUNCE_MS: u64 = 3000;
const MIN_LSP_DEBOUNCE_MS: u64 = 500;
const MAX_LSP_DEBOUNCE_MS: u64 = 60_000;
const CLEAR_ALL_DATA_DIRECTORIES: [&str; 8] = [
    "artifacts",
    "analysis-runs",
    "file-changes",
    "knowledge-hnsw",
    "hnsw_indexes",
    "marketplace-cache",
    "plugins",
    "agents",
];
const CLEAR_ALL_DATA_FILES: [&str; 4] = [
    "plugin-settings.json",
    "knowledge-tfidf-vocab.json",
    ".secret_key",
    "secrets.json",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeFeatureFlags {
    #[serde(default = "default_true")]
    pub kb_query_runs_v2: bool,
    #[serde(default = "default_true")]
    pub kb_picker_server_search: bool,
    #[serde(default = "default_true")]
    pub kb_ingest_job_scoped_progress: bool,
}

impl Default for KnowledgeFeatureFlags {
    fn default() -> Self {
        Self {
            kb_query_runs_v2: true,
            kb_picker_server_search: true,
            kb_ingest_job_scoped_progress: true,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Get current application settings
#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppState>,
) -> Result<CommandResponse<AppConfig>, String> {
    match state.get_config().await {
        Ok(config) => Ok(CommandResponse::ok(config)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update application settings with a partial update
#[tauri::command]
pub async fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    shell_state: State<'_, AppShellState>,
    webhook_state: State<'_, WebhookState>,
    update: SettingsUpdate,
) -> Result<CommandResponse<AppConfig>, String> {
    let should_sync_webhook_locale = update.language.is_some();
    match state.update_config(update).await {
        Ok(config) => {
            let config_for_base_urls = config.clone();
            if let Err(error) = state
                .with_database(move |db| {
                    sync_provider_base_urls(db, &config_for_base_urls)?;
                    Ok(())
                })
                .await
            {
                tracing::warn!("Failed to sync provider base URLs from settings: {}", error);
            }
            if let Err(error) = apply_runtime_preferences(&app, shell_state.inner(), &config) {
                tracing::warn!("Failed to apply runtime shell preferences: {}", error);
            }
            if should_sync_webhook_locale {
                webhook_state
                    .set_locale_if_initialized(&config.language)
                    .await;
            }
            Ok(CommandResponse::ok(config))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

fn sync_provider_base_urls(
    db: &crate::storage::Database,
    config: &AppConfig,
) -> crate::utils::error::AppResult<()> {
    for provider in [
        "anthropic",
        "openai",
        "deepseek",
        "glm",
        "qwen",
        "minimax",
        "ollama",
    ] {
        let key = format!("provider_{}_base_url", provider);
        let value = config.provider_base_url(provider).unwrap_or_default();
        db.set_setting(&key, &value)?;
    }
    Ok(())
}

/// Get knowledge-related runtime feature flags persisted in DB settings.
#[tauri::command]
pub async fn get_knowledge_feature_flags(
    state: State<'_, AppState>,
) -> Result<CommandResponse<KnowledgeFeatureFlags>, String> {
    match state
        .with_database(|db| {
            Ok(KnowledgeFeatureFlags {
                kb_query_runs_v2: read_feature_flag(db, KB_QUERY_RUNS_V2_FLAG, true)?,
                kb_picker_server_search: read_feature_flag(db, KB_PICKER_SERVER_SEARCH_FLAG, true)?,
                kb_ingest_job_scoped_progress: read_feature_flag(
                    db,
                    KB_INGEST_JOB_SCOPED_PROGRESS_FLAG,
                    true,
                )?,
            })
        })
        .await
    {
        Ok(flags) => Ok(CommandResponse::ok(flags)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Persist knowledge-related runtime feature flags into DB settings.
#[tauri::command]
pub async fn set_knowledge_feature_flags(
    state: State<'_, AppState>,
    flags: KnowledgeFeatureFlags,
) -> Result<CommandResponse<KnowledgeFeatureFlags>, String> {
    let normalized = flags.clone();
    let to_persist = normalized.clone();
    match state
        .with_database(move |db| {
            persist_feature_flag(db, KB_QUERY_RUNS_V2_FLAG, to_persist.kb_query_runs_v2)?;
            persist_feature_flag(
                db,
                KB_PICKER_SERVER_SEARCH_FLAG,
                to_persist.kb_picker_server_search,
            )?;
            persist_feature_flag(
                db,
                KB_INGEST_JOB_SCOPED_PROGRESS_FLAG,
                to_persist.kb_ingest_job_scoped_progress,
            )?;
            Ok(())
        })
        .await
    {
        Ok(()) => Ok(CommandResponse::ok(normalized)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Reset all settings to defaults (frontend callers should also reset local state).
#[tauri::command]
pub async fn reset_all_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    shell_state: State<'_, AppShellState>,
    webhook_state: State<'_, WebhookState>,
) -> Result<CommandResponse<bool>, String> {
    if let Err(e) = state
        .with_config_mut(|config_service| {
            config_service.reset()?;
            Ok(())
        })
        .await
    {
        return Ok(CommandResponse::err(format!(
            "Failed to reset config: {}",
            e
        )));
    }

    if let Err(e) = state.with_database(reset_backend_settings).await {
        return Ok(CommandResponse::err(format!(
            "Failed to reset backend settings: {}",
            e
        )));
    }

    if let Err(e) = state.import_all_secrets(&HashMap::new()).await {
        return Ok(CommandResponse::err(format!(
            "Failed to reset secrets: {}",
            e
        )));
    }

    if let Err(e) = clear_plugin_settings_file() {
        return Ok(CommandResponse::err(format!(
            "Failed to reset plugin settings: {}",
            e
        )));
    }

    if let Ok(config) = state.get_config().await {
        if let Err(error) = apply_runtime_preferences(&app, shell_state.inner(), &config) {
            tracing::warn!("Failed to apply runtime shell preferences: {}", error);
        }
        webhook_state
            .set_locale_if_initialized(&config.language)
            .await;
    }

    Ok(CommandResponse::ok(true))
}

/// Clear all persisted application data:
/// - frontend local data is cleared by caller
/// - backend DB rows, config, keyring secrets, and ~/.plan-cascade persistent artifacts
#[tauri::command]
pub async fn clear_all_data(
    app: AppHandle,
    state: State<'_, AppState>,
    shell_state: State<'_, AppShellState>,
) -> Result<CommandResponse<bool>, String> {
    if let Err(e) = state.with_database(clear_all_database_business_data).await {
        return Ok(CommandResponse::err(format!(
            "Failed to clear database data: {}",
            e
        )));
    }

    if let Err(e) = state
        .with_config_mut(|config_service| {
            config_service.reset()?;
            Ok(())
        })
        .await
    {
        return Ok(CommandResponse::err(format!(
            "Failed to reset config: {}",
            e
        )));
    }

    if let Err(e) = state.import_all_secrets(&HashMap::new()).await {
        return Ok(CommandResponse::err(format!(
            "Failed to clear secrets: {}",
            e
        )));
    }

    let plan_dir = match crate::utils::paths::plan_cascade_dir() {
        Ok(dir) => dir,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to resolve data dir: {}",
                e
            )))
        }
    };
    if let Err(e) = clear_plan_cascade_persisted_data(&plan_dir) {
        return Ok(CommandResponse::err(format!(
            "Failed to clear persisted files: {}",
            e
        )));
    }

    if let Ok(config) = state.get_config().await {
        if let Err(error) = apply_runtime_preferences(&app, shell_state.inner(), &config) {
            tracing::warn!("Failed to apply runtime shell preferences: {}", error);
        }
    }

    Ok(CommandResponse::ok(true))
}

/// Export all settings (frontend + backend + optionally encrypted secrets)
#[tauri::command]
pub async fn export_all_settings(
    state: State<'_, AppState>,
    frontend_state: serde_json::Value,
    password: Option<String>,
) -> Result<CommandResponse<UnifiedSettingsExport>, String> {
    // 1. Get AppConfig
    let config = match state.get_config().await {
        Ok(c) => c,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    // 2. Collect all backend settings
    let backend = match state
        .with_database(|db| settings_export::collect_backend_settings(db, &config))
        .await
    {
        Ok(b) => b,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    // 3. Optionally encrypt secrets
    let (has_encrypted_secrets, encrypted_secrets) = match password {
        Some(ref pw) if !pw.is_empty() => {
            match state.export_all_secrets().await {
                Ok(secrets) => {
                    if secrets.is_empty() {
                        (false, None)
                    } else {
                        let json = match serde_json::to_string(&secrets) {
                            Ok(j) => j,
                            Err(e) => return Ok(CommandResponse::err(e.to_string())),
                        };
                        match settings_export::encrypt_with_password(&json, pw) {
                            Ok(encrypted) => (true, Some(encrypted)),
                            Err(e) => return Ok(CommandResponse::err(e.to_string())),
                        }
                    }
                }
                Err(e) => {
                    // Secrets export failed, but we can still export everything else
                    tracing::warn!("Failed to export secrets: {}", e);
                    (false, None)
                }
            }
        }
        _ => (false, None),
    };

    let export = UnifiedSettingsExport {
        version: "6.0".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        has_encrypted_secrets,
        frontend: frontend_state,
        backend,
        encrypted_secrets,
    };

    Ok(CommandResponse::ok(export))
}

/// Import settings from a unified export JSON
#[tauri::command]
pub async fn import_all_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    shell_state: State<'_, AppShellState>,
    export_json: String,
    password: Option<String>,
) -> Result<CommandResponse<SettingsImportResult>, String> {
    // Parse the export JSON
    let parsed: serde_json::Value = match serde_json::from_str(&export_json) {
        Ok(v) => v,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to parse export JSON: {}",
                e
            )))
        }
    };

    let version = parsed["version"].as_str().unwrap_or("");

    // Handle v5.1 legacy format: { version: "5.1", settings: {...} }
    if version.starts_with("5.") {
        let frontend = parsed.get("settings").cloned();
        let result = SettingsImportResult {
            success: true,
            frontend,
            imported_sections: vec!["frontend (legacy v5.x)".to_string()],
            skipped_sections: vec!["backend (not in v5.x format)".to_string()],
            warnings: vec![
                "Legacy v5.x format detected. Only frontend settings were imported.".to_string(),
            ],
            errors: Vec::new(),
        };
        return Ok(CommandResponse::ok(result));
    }

    // Handle v6.0 format
    if version != "6.0" {
        return Ok(CommandResponse::err(format!(
            "Unsupported export version: '{}'. Expected '6.0' or '5.x'.",
            version
        )));
    }

    let export: UnifiedSettingsExport = match serde_json::from_str(&export_json) {
        Ok(e) => e,
        Err(e) => {
            return Ok(CommandResponse::err(format!(
                "Failed to parse v6.0 export: {}",
                e
            )))
        }
    };

    // Import encrypted secrets if present and password provided
    let mut secret_warnings = Vec::new();
    if export.has_encrypted_secrets {
        if let Some(ref encrypted) = export.encrypted_secrets {
            match &password {
                Some(pw) if !pw.is_empty() => {
                    match settings_export::decrypt_with_password(encrypted, pw) {
                        Ok(decrypted_json) => {
                            match serde_json::from_str::<std::collections::HashMap<String, String>>(
                                &decrypted_json,
                            ) {
                                Ok(secrets) => {
                                    if let Err(e) = state.import_all_secrets(&secrets).await {
                                        secret_warnings
                                            .push(format!("Failed to import secrets: {}", e));
                                    }
                                }
                                Err(e) => {
                                    secret_warnings
                                        .push(format!("Failed to parse decrypted secrets: {}", e));
                                }
                            }
                        }
                        Err(_) => {
                            return Ok(CommandResponse::ok(SettingsImportResult {
                                success: false,
                                frontend: Some(export.frontend),
                                imported_sections: Vec::new(),
                                skipped_sections: Vec::new(),
                                warnings: Vec::new(),
                                errors: vec![
                                    "Wrong password: failed to decrypt API keys.".to_string()
                                ],
                            }));
                        }
                    }
                }
                _ => {
                    secret_warnings.push(
                        "Export contains encrypted secrets but no password was provided. API keys were not imported.".to_string(),
                    );
                }
            }
        }
    }

    // Import backend settings (database-backed sections)
    let mut result = match state
        .with_database(|db| {
            let mut result = SettingsImportResult {
                success: true,
                frontend: None,
                imported_sections: Vec::new(),
                skipped_sections: Vec::new(),
                warnings: Vec::new(),
                errors: Vec::new(),
            };
            import_db_sections(db, &export.backend, &mut result);
            Ok(result)
        })
        .await
    {
        Ok(r) => r,
        Err(e) => SettingsImportResult {
            success: false,
            frontend: None,
            imported_sections: Vec::new(),
            skipped_sections: Vec::new(),
            warnings: Vec::new(),
            errors: vec![format!("Database access failed: {}", e)],
        },
    };

    // Import config separately (needs ConfigService)
    match state
        .with_config_mut(|config_service| {
            let new_config: AppConfig = serde_json::from_value(export.backend.config.clone())?;
            let update = SettingsUpdate {
                theme: Some(new_config.theme),
                language: Some(new_config.language),
                default_provider: Some(new_config.default_provider),
                default_model: Some(new_config.default_model),
                model_by_provider: Some(new_config.model_by_provider),
                glm_endpoint: Some(new_config.glm_endpoint),
                minimax_endpoint: Some(new_config.minimax_endpoint),
                qwen_endpoint: Some(new_config.qwen_endpoint),
                custom_provider_base_urls: Some(new_config.custom_provider_base_urls),
                custom_provider_endpoints: Some(new_config.custom_provider_endpoints),
                selected_custom_provider_endpoint_ids: Some(
                    new_config.selected_custom_provider_endpoint_ids,
                ),
                analytics_enabled: Some(new_config.analytics_enabled),
                auto_save_interval: Some(new_config.auto_save_interval),
                max_recent_projects: Some(new_config.max_recent_projects),
                debug_mode: Some(new_config.debug_mode),
                search_provider: Some(new_config.search_provider),
                close_to_background_enabled: Some(new_config.close_to_background_enabled),
                worktree_auto_cleanup_on_session_delete: Some(
                    new_config.worktree_auto_cleanup_on_session_delete,
                ),
            };
            config_service.update_config(update)?;
            Ok(())
        })
        .await
    {
        Ok(()) => result.imported_sections.push("config".to_string()),
        Err(e) => result.errors.push(format!("config: {}", e)),
    }

    if let Ok(config) = state.get_config().await {
        if let Err(error) = apply_runtime_preferences(&app, shell_state.inner(), &config) {
            tracing::warn!("Failed to apply runtime shell preferences: {}", error);
        }
    }

    // Merge secret warnings
    result.warnings.extend(secret_warnings);

    // Set frontend data for Zustand application
    result.frontend = Some(export.frontend);
    result.success = result.errors.is_empty();

    Ok(CommandResponse::ok(result))
}

/// Import database-backed sections (everything except config which needs ConfigService).
fn import_db_sections(
    db: &crate::storage::Database,
    backend: &crate::models::export::BackendSettingsExport,
    result: &mut SettingsImportResult,
) {
    use crate::services::settings_export as se;

    // Embedding
    if let Some(ref embedding) = backend.embedding {
        match serde_json::to_string(embedding)
            .map_err(|e| crate::utils::error::AppError::Serialization(e))
            .and_then(|s| db.set_setting("embedding_config", &s))
        {
            Ok(()) => result.imported_sections.push("embedding".to_string()),
            Err(e) => result.errors.push(format!("embedding: {}", e)),
        }
    } else {
        result.skipped_sections.push("embedding".to_string());
    }

    // LSP preferences
    if let Some(ref lsp) = backend.lsp {
        match import_lsp_preferences(db, lsp) {
            Ok(()) => result.imported_sections.push("lsp".to_string()),
            Err(e) => result.errors.push(format!("lsp: {}", e)),
        }
    } else {
        result.skipped_sections.push("lsp".to_string());
    }

    // Proxy
    match import_proxy(db, &backend.proxy) {
        Ok(()) => result.imported_sections.push("proxy".to_string()),
        Err(e) => result.errors.push(format!("proxy: {}", e)),
    }

    // Webhooks
    match import_webhooks(db, &backend.webhooks) {
        Ok(()) => result.imported_sections.push("webhooks".to_string()),
        Err(e) => result.errors.push(format!("webhooks: {}", e)),
    }

    // Guardrails
    match import_guardrails(db, backend.guardrail_mode.as_deref(), &backend.guardrails) {
        Ok(()) => result.imported_sections.push("guardrails".to_string()),
        Err(e) => result.errors.push(format!("guardrails: {}", e)),
    }

    // Remote
    match import_remote(db, &backend.remote) {
        Ok(()) => result.imported_sections.push("remote".to_string()),
        Err(e) => result.errors.push(format!("remote: {}", e)),
    }

    // A2A agents
    match import_a2a_agents(db, &backend.a2a_agents) {
        Ok(()) => result.imported_sections.push("a2a_agents".to_string()),
        Err(e) => result.errors.push(format!("a2a_agents: {}", e)),
    }

    // MCP servers
    match import_mcp_servers(db, &backend.mcp_servers) {
        Ok(()) => result.imported_sections.push("mcp_servers".to_string()),
        Err(e) => result.errors.push(format!("mcp_servers: {}", e)),
    }

    // Plugin settings
    if let Some(ref ps) = backend.plugin_settings {
        match se::import_plugin_settings(ps) {
            Ok(()) => result.imported_sections.push("plugin_settings".to_string()),
            Err(e) => result.errors.push(format!("plugin_settings: {}", e)),
        }
    } else {
        result.skipped_sections.push("plugin_settings".to_string());
    }
}

fn import_proxy(
    db: &crate::storage::Database,
    proxy: &crate::models::export::ProxyExport,
) -> crate::utils::error::AppResult<()> {
    if let Some(ref global) = proxy.global {
        let json_str = serde_json::to_string(global)?;
        db.set_setting("proxy_global", &json_str)?;
    }
    if let Some(obj) = proxy.strategies.as_object() {
        for (key, value) in obj {
            let json_str = serde_json::to_string(value)?;
            db.set_setting(key, &json_str)?;
        }
    }
    if let Some(obj) = proxy.custom_configs.as_object() {
        for (key, value) in obj {
            let json_str = serde_json::to_string(value)?;
            db.set_setting(key, &json_str)?;
        }
    }
    Ok(())
}

fn import_lsp_preferences(
    db: &crate::storage::Database,
    lsp: &crate::models::export::LspPreferencesExport,
) -> crate::utils::error::AppResult<()> {
    let debounce = if lsp.incremental_debounce_ms == 0 {
        DEFAULT_LSP_DEBOUNCE_MS
    } else {
        lsp.incremental_debounce_ms
    }
    .clamp(MIN_LSP_DEBOUNCE_MS, MAX_LSP_DEBOUNCE_MS);
    let payload = serde_json::json!({
        "autoEnrich": lsp.auto_enrich,
        "incrementalDebounceMs": debounce,
    });
    db.set_setting(LSP_PREFERENCES_KEY, &payload.to_string())?;
    Ok(())
}

fn import_webhooks(
    db: &crate::storage::Database,
    webhooks: &[serde_json::Value],
) -> crate::utils::error::AppResult<()> {
    let conn = db.get_connection()?;
    conn.execute("DELETE FROM webhook_channels", [])?;
    drop(conn);
    for wh_value in webhooks {
        let ch: crate::services::webhook::types::WebhookChannelConfig =
            serde_json::from_value(wh_value.clone())?;
        db.insert_webhook_channel(&ch)?;
    }
    Ok(())
}

fn import_guardrails(
    db: &crate::storage::Database,
    guardrail_mode: Option<&str>,
    guardrails: &[crate::models::export::GuardrailRuleExport],
) -> crate::utils::error::AppResult<()> {
    let conn = db.get_connection()?;
    conn.execute("DELETE FROM guardrail_rules", [])?;
    let mode = crate::services::guardrail::GuardrailMode::parse(guardrail_mode.unwrap_or("strict"))
        .unwrap_or_default();
    db.set_setting("guardrail_mode_v1", &mode.to_string())?;
    for rule in guardrails {
        conn.execute(
            "INSERT INTO guardrail_rules
             (id, name, guardrail_type, builtin_key, pattern, action, scope, enabled, editable, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'), datetime('now'))",
            rusqlite::params![
                rule.id,
                rule.name,
                if rule.guardrail_type.is_empty() { "custom".to_string() } else { rule.guardrail_type.clone() },
                rule.builtin_key,
                rule.pattern,
                rule.action,
                serde_json::to_string(&rule.scope)?,
                rule.enabled as i32,
                rule.editable as i32,
                rule.description,
            ],
        )?;
    }
    Ok(())
}

fn import_remote(
    db: &crate::storage::Database,
    remote: &crate::models::export::RemoteExport,
) -> crate::utils::error::AppResult<()> {
    if let Some(ref gateway) = remote.gateway {
        let json_str = serde_json::to_string(gateway)?;
        db.set_setting("remote_gateway_config", &json_str)?;
    }
    if let Some(ref telegram) = remote.telegram {
        let json_str = serde_json::to_string(telegram)?;
        db.set_setting("remote_telegram_config", &json_str)?;
    }
    Ok(())
}

fn import_a2a_agents(
    db: &crate::storage::Database,
    agents: &[serde_json::Value],
) -> crate::utils::error::AppResult<()> {
    let conn = db.get_connection()?;
    conn.execute("DELETE FROM remote_agents", [])?;
    for agent_value in agents {
        let id = agent_value["id"].as_str().unwrap_or_default();
        let base_url = agent_value["base_url"].as_str().unwrap_or_default();
        let name = agent_value["name"].as_str().unwrap_or_default();
        let description = agent_value["description"].as_str().unwrap_or_default();
        let capabilities = serde_json::to_string(&agent_value["capabilities"])?;
        let endpoint = agent_value["endpoint"].as_str().unwrap_or_default();
        let version = agent_value["version"].as_str().unwrap_or_default();
        let auth_required = agent_value["auth_required"].as_bool().unwrap_or(false) as i32;
        let supported_inputs = serde_json::to_string(&agent_value["supported_inputs"])?;

        conn.execute(
            "INSERT INTO remote_agents (id, base_url, name, description, capabilities, endpoint, version, auth_required, supported_inputs)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![id, base_url, name, description, capabilities, endpoint, version, auth_required, supported_inputs],
        )?;
    }
    Ok(())
}

fn import_mcp_servers(
    db: &crate::storage::Database,
    servers: &[serde_json::Value],
) -> crate::utils::error::AppResult<()> {
    let keyring = crate::storage::KeyringService::new();

    for server_value in servers {
        let server: crate::models::McpServer = serde_json::from_value(server_value.clone())?;
        let mut server = server;

        if !server.env.is_empty() {
            let key = format!("mcp/{}/env", server.id);
            let raw = serde_json::to_string(&server.env)?;
            keyring.set_api_key(&key, &raw)?;
            server.has_env_secret = true;
            server.env.clear();
        }

        if !server.headers.is_empty() {
            let key = format!("mcp/{}/headers", server.id);
            let raw = serde_json::to_string(&server.headers)?;
            keyring.set_api_key(&key, &raw)?;
            server.has_headers_secret = true;
            server.headers.clear();
        }

        match db.get_mcp_server(&server.id)? {
            Some(_) => db.update_mcp_server(&server)?,
            None => db.insert_mcp_server(&server)?,
        }
    }
    Ok(())
}

fn reset_backend_settings(db: &crate::storage::Database) -> crate::utils::error::AppResult<()> {
    use crate::services::orchestrator::embedding_provider::{
        CODEBASE_INDEX_CONFIG_KEY, EMBEDDING_CONFIG_SETTING_KEY,
    };

    for key in [
        EMBEDDING_CONFIG_SETTING_KEY,
        CODEBASE_INDEX_CONFIG_KEY,
        "proxy_global",
        "remote_gateway_config",
        "remote_telegram_config",
        KB_QUERY_RUNS_V2_FLAG,
        KB_PICKER_SERVER_SEARCH_FLAG,
        KB_INGEST_JOB_SCOPED_PROGRESS_FLAG,
        LSP_PREFERENCES_KEY,
        "feature.kb_query_runs_v2",
        "feature.kb_picker_server_search",
        "feature.kb_ingest_job_scoped_progress",
    ] {
        db.delete_setting(key)?;
    }

    for prefix in ["proxy_strategy_", "proxy_custom_", "provider_"] {
        delete_settings_by_prefix(db, prefix)?;
    }

    let keyring = crate::storage::KeyringService::new();
    if let Ok(servers) = db.list_mcp_servers() {
        for server in servers {
            let _ = keyring.delete_api_key(&format!("mcp/{}/env", server.id));
            let _ = keyring.delete_api_key(&format!("mcp/{}/headers", server.id));
        }
    }

    let conn = db.get_connection()?;
    conn.execute("DELETE FROM webhook_channels", [])?;
    conn.execute("DELETE FROM guardrail_rules", [])?;
    conn.execute("DELETE FROM guardrail_events", [])?;
    conn.execute("DELETE FROM remote_agents", [])?;
    conn.execute("DELETE FROM mcp_servers", [])?;

    Ok(())
}

fn delete_settings_by_prefix(
    db: &crate::storage::Database,
    prefix: &str,
) -> crate::utils::error::AppResult<()> {
    for (key, _) in db.get_settings_by_prefix(prefix)? {
        db.delete_setting(&key)?;
    }
    Ok(())
}

fn clear_plugin_settings_file() -> crate::utils::error::AppResult<()> {
    let plugin_settings_path =
        crate::utils::paths::plan_cascade_dir()?.join("plugin-settings.json");
    if plugin_settings_path.exists() {
        std::fs::remove_file(plugin_settings_path)?;
    }
    Ok(())
}

fn clear_all_database_business_data(
    db: &crate::storage::Database,
) -> crate::utils::error::AppResult<()> {
    let conn = db.get_connection()?;
    let tables = list_user_tables(&conn)?;

    for table in tables {
        let escaped_table = table.replace('"', "\"\"");
        let sql = format!("DELETE FROM \"{}\"", escaped_table);
        if let Err(e) = conn.execute(&sql, []) {
            if is_read_only_shadow_table_error(&e) {
                tracing::warn!("Skipping readonly shadow table '{}': {}", table, e);
                continue;
            }
            return Err(e.into());
        }
    }

    Ok(())
}

fn list_user_tables(conn: &rusqlite::Connection) -> crate::utils::error::AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
    )?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    let mut tables = Vec::new();
    for row in rows {
        tables.push(row?);
    }
    Ok(tables)
}

fn is_read_only_shadow_table_error(err: &rusqlite::Error) -> bool {
    err.to_string()
        .to_ascii_lowercase()
        .contains("shadow table")
}

fn clear_plan_cascade_persisted_data(base_dir: &Path) -> crate::utils::error::AppResult<()> {
    for relative_dir in CLEAR_ALL_DATA_DIRECTORIES {
        let path = base_dir.join(relative_dir);
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
        }
    }

    for relative_file in CLEAR_ALL_DATA_FILES {
        let path = base_dir.join(relative_file);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
    }

    Ok(())
}

fn parse_bool_setting(value: Option<String>, default_value: bool) -> bool {
    match value
        .as_deref()
        .map(str::trim)
        .map(|v| v.to_ascii_lowercase())
    {
        Some(v) if v == "1" || v == "true" || v == "yes" || v == "on" => true,
        Some(v) if v == "0" || v == "false" || v == "no" || v == "off" => false,
        _ => default_value,
    }
}

fn read_feature_flag(
    db: &crate::storage::Database,
    key: &str,
    default_value: bool,
) -> crate::utils::error::AppResult<bool> {
    for setting_key in [format!("feature.{}", key), key.to_string()] {
        if let Some(raw) = db.get_setting(&setting_key)? {
            return Ok(parse_bool_setting(Some(raw), default_value));
        }
    }
    Ok(default_value)
}

fn persist_feature_flag(
    db: &crate::storage::Database,
    key: &str,
    enabled: bool,
) -> crate::utils::error::AppResult<()> {
    let value = if enabled { "true" } else { "false" };
    db.set_setting(&format!("feature.{}", key), value)?;
    db.set_setting(key, value)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::Database;

    fn create_test_db() -> Database {
        let manager = r2d2_sqlite::SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("pool");
        {
            let conn = pool.get().expect("conn");
            conn.execute(
                "CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL, created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP)",
                [],
            )
            .expect("create settings");
        }
        Database::from_pool_for_test(pool)
    }

    #[test]
    fn persist_feature_flag_writes_both_legacy_and_feature_keys() {
        let db = create_test_db();
        persist_feature_flag(&db, KB_QUERY_RUNS_V2_FLAG, false).expect("persist flag");

        assert_eq!(
            db.get_setting("feature.kb_query_runs_v2").unwrap(),
            Some("false".to_string())
        );
        assert_eq!(
            db.get_setting("kb_query_runs_v2").unwrap(),
            Some("false".to_string())
        );
    }

    #[test]
    fn read_feature_flag_prefers_prefixed_value_and_falls_back_to_default() {
        let db = create_test_db();
        db.set_setting("kb_query_runs_v2", "false")
            .expect("set legacy flag");
        db.set_setting("feature.kb_query_runs_v2", "true")
            .expect("set feature flag");

        let resolved = read_feature_flag(&db, KB_QUERY_RUNS_V2_FLAG, false).expect("read flag");
        assert!(resolved);

        let defaulted =
            read_feature_flag(&db, KB_PICKER_SERVER_SEARCH_FLAG, true).expect("read default");
        assert!(defaulted);
    }

    #[test]
    fn clear_all_database_business_data_removes_rows_from_user_tables() {
        let db = create_test_db();
        let conn = db.get_connection().expect("conn");
        conn.execute(
            "CREATE TABLE IF NOT EXISTS test_data (id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
            [],
        )
        .expect("create test_data");
        conn.execute("INSERT INTO settings (key, value) VALUES ('a', '1')", [])
            .expect("insert settings");
        conn.execute("INSERT INTO test_data (value) VALUES ('hello')", [])
            .expect("insert test_data");
        drop(conn);

        clear_all_database_business_data(&db).expect("clear data");

        let conn = db.get_connection().expect("conn");
        let settings_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM settings", [], |row| row.get(0))
            .expect("count settings");
        let data_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM test_data", [], |row| row.get(0))
            .expect("count test_data");
        assert_eq!(settings_count, 0);
        assert_eq!(data_count, 0);
    }

    #[test]
    fn clear_plan_cascade_persisted_data_removes_targets_only() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        for relative_dir in CLEAR_ALL_DATA_DIRECTORIES {
            let dir_path = temp_dir.path().join(relative_dir);
            std::fs::create_dir_all(&dir_path).expect("create dir");
            std::fs::write(dir_path.join("marker.txt"), "x").expect("write marker");
        }
        for relative_file in CLEAR_ALL_DATA_FILES {
            std::fs::write(temp_dir.path().join(relative_file), "x").expect("write file");
        }
        let untouched = temp_dir.path().join("keep.txt");
        std::fs::write(&untouched, "keep").expect("write untouched");

        clear_plan_cascade_persisted_data(temp_dir.path()).expect("clear persisted");

        for relative_dir in CLEAR_ALL_DATA_DIRECTORIES {
            assert!(
                !temp_dir.path().join(relative_dir).exists(),
                "expected dir to be removed: {}",
                relative_dir
            );
        }
        for relative_file in CLEAR_ALL_DATA_FILES {
            assert!(
                !temp_dir.path().join(relative_file).exists(),
                "expected file to be removed: {}",
                relative_file
            );
        }
        assert!(untouched.exists(), "untouched file should remain");
    }
}
