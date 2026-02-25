//! Settings Commands
//!
//! Commands for reading and updating application settings.

use tauri::State;

use crate::models::export::{SettingsImportResult, UnifiedSettingsExport};
use crate::models::response::CommandResponse;
use crate::models::settings::{AppConfig, SettingsUpdate};
use crate::services::settings_export;
use crate::state::AppState;

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
    state: State<'_, AppState>,
    update: SettingsUpdate,
) -> Result<CommandResponse<AppConfig>, String> {
    match state.update_config(update).await {
        Ok(config) => Ok(CommandResponse::ok(config)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
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
    state: State<'_, AppState>,
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
                            match serde_json::from_str::<
                                std::collections::HashMap<String, String>,
                            >(&decrypted_json)
                            {
                                Ok(secrets) => {
                                    if let Err(e) = state.import_all_secrets(&secrets).await {
                                        secret_warnings.push(format!(
                                            "Failed to import secrets: {}",
                                            e
                                        ));
                                    }
                                }
                                Err(e) => {
                                    secret_warnings.push(format!(
                                        "Failed to parse decrypted secrets: {}",
                                        e
                                    ));
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
                analytics_enabled: Some(new_config.analytics_enabled),
                auto_save_interval: Some(new_config.auto_save_interval),
                max_recent_projects: Some(new_config.max_recent_projects),
                debug_mode: Some(new_config.debug_mode),
                search_provider: Some(new_config.search_provider),
            };
            config_service.update_config(update)?;
            Ok(())
        })
        .await
    {
        Ok(()) => result.imported_sections.push("config".to_string()),
        Err(e) => result
            .errors
            .push(format!("config: {}", e)),
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
    match import_guardrails(db, &backend.guardrails) {
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
    guardrails: &[crate::models::export::GuardrailRuleExport],
) -> crate::utils::error::AppResult<()> {
    let conn = db.get_connection()?;
    conn.execute("DELETE FROM guardrail_rules", [])?;
    for rule in guardrails {
        conn.execute(
            "INSERT INTO guardrail_rules (id, name, pattern, action, enabled) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![rule.id, rule.name, rule.pattern, rule.action, rule.enabled as i32],
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
    for server_value in servers {
        let server: crate::models::McpServer = serde_json::from_value(server_value.clone())?;
        match db.get_mcp_server(&server.id)? {
            Some(_) => db.update_mcp_server(&server)?,
            None => db.insert_mcp_server(&server)?,
        }
    }
    Ok(())
}
