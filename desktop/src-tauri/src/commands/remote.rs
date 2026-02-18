//! Remote Control Commands
//!
//! Tauri commands for remote session control via Telegram Bot.
//! Manages gateway lifecycle, configuration, session monitoring,
//! and audit logging.
//!
//! ## IPC Commands
//!
//! - `get_remote_gateway_status` — Get gateway running status and stats
//! - `start_remote_gateway` — Start the remote gateway with Telegram adapter
//! - `stop_remote_gateway` — Stop the remote gateway
//! - `get_remote_config` — Retrieve remote gateway configuration
//! - `update_remote_config` — Update remote gateway configuration
//! - `get_telegram_config` — Get Telegram adapter configuration
//! - `update_telegram_config` — Update Telegram adapter configuration
//! - `list_remote_sessions` — List active remote session mappings
//! - `disconnect_remote_session` — Disconnect a remote session by chat_id
//! - `get_remote_audit_log` — Query audit log with pagination

use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use crate::commands::proxy::resolve_provider_proxy;
use crate::models::response::CommandResponse;
use crate::services::remote::gateway::RemoteGatewayService;
use crate::services::remote::session_bridge::SessionBridge;
use crate::services::remote::types::{
    GatewayStatus, RemoteAuditEntry, RemoteGatewayConfig, RemoteSessionMapping,
    TelegramAdapterConfig, UpdateRemoteConfigRequest, UpdateTelegramConfigRequest,
};
use crate::state::AppState;
use crate::storage::KeyringService;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Settings DB key for remote gateway config.
const REMOTE_CONFIG_KEY: &str = "remote_gateway_config";

/// Settings DB key for Telegram adapter config.
const TELEGRAM_CONFIG_KEY: &str = "remote_telegram_config";

/// Keyring key for Telegram bot token.
const KEYRING_BOT_TOKEN: &str = "remote_telegram_bot_token";

/// Keyring key for remote access password.
const KEYRING_ACCESS_PASSWORD: &str = "remote_access_password";

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Tauri-managed state for remote gateway.
pub struct RemoteState {
    pub gateway: Arc<RwLock<Option<RemoteGatewayService>>>,
}

impl RemoteState {
    pub fn new() -> Self {
        Self {
            gateway: Arc::new(RwLock::new(None)),
        }
    }
}

impl Default for RemoteState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Response Types
// ---------------------------------------------------------------------------

/// Audit log query response with pagination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogResponse {
    pub entries: Vec<RemoteAuditEntry>,
    pub total: u32,
}

// ---------------------------------------------------------------------------
// IPC Commands
// ---------------------------------------------------------------------------

/// Get the current remote gateway status.
#[tauri::command]
pub async fn get_remote_gateway_status(
    state: State<'_, RemoteState>,
) -> Result<CommandResponse<GatewayStatus>, String> {
    let guard = state.gateway.read().await;
    match guard.as_ref() {
        Some(gateway) => {
            let status = gateway.get_status().await;
            Ok(CommandResponse::ok(status))
        }
        None => Ok(CommandResponse::ok(GatewayStatus::default())),
    }
}

/// Start the remote gateway with Telegram adapter.
#[tauri::command]
pub async fn start_remote_gateway(
    remote_state: State<'_, RemoteState>,
    app_state: State<'_, AppState>,
) -> Result<CommandResponse<()>, String> {
    // Read configs from database
    let config_result = app_state
        .with_database(|db| {
            let gateway_config: RemoteGatewayConfig = db
                .get_setting(REMOTE_CONFIG_KEY)?
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();

            let mut telegram_config: Option<TelegramAdapterConfig> = db
                .get_setting(TELEGRAM_CONFIG_KEY)?
                .and_then(|json| serde_json::from_str(&json).ok());

            // Hydrate secrets from keyring
            let keyring = KeyringService::new();
            if let Some(ref mut tg) = telegram_config {
                tg.bot_token = keyring.get_api_key(KEYRING_BOT_TOKEN).ok().flatten();
                tg.access_password = keyring
                    .get_api_key(KEYRING_ACCESS_PASSWORD)
                    .ok()
                    .flatten();
            }

            // Resolve proxy for remote_telegram provider
            let proxy = resolve_provider_proxy(&keyring, db, "remote_telegram");

            Ok((gateway_config, telegram_config, proxy))
        })
        .await;

    let (gateway_config, telegram_config, proxy) = match config_result {
        Ok(c) => c,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    // Get database for gateway
    let db_result = app_state
        .with_database(|db| Ok(Arc::new(db.clone())))
        .await;

    let db = match db_result {
        Ok(d) => d,
        Err(e) => return Ok(CommandResponse::err(e.to_string())),
    };

    // Create session bridge
    let bridge = Arc::new(SessionBridge::new(db.clone()));

    // Load existing mappings from DB
    if let Err(e) = bridge.load_mappings_from_db().await {
        return Ok(CommandResponse::err(format!(
            "Failed to load session mappings: {}",
            e
        )));
    }

    // Create gateway
    let gateway = RemoteGatewayService::new(gateway_config, telegram_config, bridge, db);

    // Start gateway
    if let Err(e) = gateway.start(proxy.as_ref()).await {
        return Ok(CommandResponse::err(format!(
            "Failed to start gateway: {}",
            e
        )));
    }

    // Store gateway in state
    let mut guard = remote_state.gateway.write().await;
    *guard = Some(gateway);

    Ok(CommandResponse::ok(()))
}

/// Stop the remote gateway.
#[tauri::command]
pub async fn stop_remote_gateway(
    state: State<'_, RemoteState>,
) -> Result<CommandResponse<()>, String> {
    let guard = state.gateway.read().await;
    match guard.as_ref() {
        Some(gateway) => match gateway.stop().await {
            Ok(()) => Ok(CommandResponse::ok(())),
            Err(e) => Ok(CommandResponse::err(e.to_string())),
        },
        None => Ok(CommandResponse::err(
            "Gateway is not running".to_string(),
        )),
    }
}

/// Get remote gateway configuration from database.
#[tauri::command]
pub async fn get_remote_config(
    state: State<'_, AppState>,
) -> Result<CommandResponse<RemoteGatewayConfig>, String> {
    let result = state
        .with_database(|db| {
            let config: RemoteGatewayConfig = db
                .get_setting(REMOTE_CONFIG_KEY)?
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();
            Ok(config)
        })
        .await;

    match result {
        Ok(config) => Ok(CommandResponse::ok(config)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update remote gateway configuration.
#[tauri::command]
pub async fn update_remote_config(
    request: UpdateRemoteConfigRequest,
    app_state: State<'_, AppState>,
    remote_state: State<'_, RemoteState>,
) -> Result<CommandResponse<()>, String> {
    // Read existing config
    let save_result = app_state
        .with_database(|db| {
            let mut config: RemoteGatewayConfig = db
                .get_setting(REMOTE_CONFIG_KEY)?
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();

            // Apply updates
            if let Some(enabled) = request.enabled {
                config.enabled = enabled;
            }
            if let Some(adapter) = request.adapter {
                config.adapter = adapter;
            }
            if let Some(auto_start) = request.auto_start {
                config.auto_start = auto_start;
            }

            // Save
            let json = serde_json::to_string(&config).map_err(|e| {
                crate::utils::error::AppError::Internal(format!(
                    "Failed to serialize remote config: {}",
                    e
                ))
            })?;
            db.set_setting(REMOTE_CONFIG_KEY, &json)?;

            Ok(config)
        })
        .await;

    match save_result {
        Ok(config) => {
            // Update running gateway config if active
            let guard = remote_state.gateway.read().await;
            if let Some(gateway) = guard.as_ref() {
                let _ = gateway.update_config(config).await;
            }
            Ok(CommandResponse::ok(()))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Get Telegram adapter configuration.
#[tauri::command]
pub async fn get_telegram_config(
    state: State<'_, AppState>,
) -> Result<CommandResponse<TelegramAdapterConfig>, String> {
    let result = state
        .with_database(|db| {
            let mut config: TelegramAdapterConfig = db
                .get_setting(TELEGRAM_CONFIG_KEY)?
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();

            // Don't return actual bot token value, just indicate if it's set
            let keyring = KeyringService::new();
            let has_token = keyring
                .get_api_key(KEYRING_BOT_TOKEN)
                .ok()
                .flatten()
                .is_some();
            if has_token {
                config.bot_token = Some("***".to_string());
            } else {
                config.bot_token = None; // Explicit null for frontend
            }

            let has_password = keyring
                .get_api_key(KEYRING_ACCESS_PASSWORD)
                .ok()
                .flatten()
                .is_some();
            if has_password {
                config.access_password = Some("***".to_string());
            } else {
                config.access_password = None; // Explicit null for frontend
            }

            Ok(config)
        })
        .await;

    match result {
        Ok(config) => Ok(CommandResponse::ok(config)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// Update Telegram adapter configuration.
#[tauri::command]
pub async fn update_telegram_config(
    request: UpdateTelegramConfigRequest,
    app_state: State<'_, AppState>,
    remote_state: State<'_, RemoteState>,
) -> Result<CommandResponse<()>, String> {
    // Store secrets in keyring
    let keyring = KeyringService::new();
    if let Some(ref token) = request.bot_token {
        if !token.is_empty() && token != "***" {
            if let Err(e) = keyring.set_api_key(KEYRING_BOT_TOKEN, token) {
                return Ok(CommandResponse::err(format!(
                    "Failed to store bot token: {}",
                    e
                )));
            }
        }
    }
    if let Some(ref password) = request.access_password {
        if !password.is_empty() && password != "***" {
            if let Err(e) = keyring.set_api_key(KEYRING_ACCESS_PASSWORD, password) {
                return Ok(CommandResponse::err(format!(
                    "Failed to store access password: {}",
                    e
                )));
            }
        }
    }

    // Save non-secret fields to database
    let save_result = app_state
        .with_database(|db| {
            let mut config: TelegramAdapterConfig = db
                .get_setting(TELEGRAM_CONFIG_KEY)?
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();

            // Apply updates
            if let Some(ids) = request.allowed_chat_ids {
                config.allowed_chat_ids = ids;
            }
            if let Some(ids) = request.allowed_user_ids {
                config.allowed_user_ids = ids;
            }
            if let Some(rp) = request.require_password {
                config.require_password = rp;
            }
            if let Some(mml) = request.max_message_length {
                config.max_message_length = mml;
            }
            if let Some(sm) = request.streaming_mode {
                config.streaming_mode = sm;
            }

            // Don't store bot_token or access_password in database (they go to keyring)
            config.bot_token = None;
            config.access_password = None;

            let json = serde_json::to_string(&config).map_err(|e| {
                crate::utils::error::AppError::Internal(format!(
                    "Failed to serialize telegram config: {}",
                    e
                ))
            })?;
            db.set_setting(TELEGRAM_CONFIG_KEY, &json)?;

            Ok(config)
        })
        .await;

    match save_result {
        Ok(config) => {
            // Update running gateway's telegram config if active
            let guard = remote_state.gateway.read().await;
            if let Some(gateway) = guard.as_ref() {
                let _ = gateway.update_telegram_config(config).await;
            }
            Ok(CommandResponse::ok(()))
        }
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

/// List active remote session mappings.
#[tauri::command]
pub async fn list_remote_sessions(
    state: State<'_, RemoteState>,
) -> Result<CommandResponse<Vec<RemoteSessionMapping>>, String> {
    let guard = state.gateway.read().await;
    match guard.as_ref() {
        Some(gateway) => {
            let sessions = gateway.list_sessions().await;
            Ok(CommandResponse::ok(sessions))
        }
        None => Ok(CommandResponse::ok(vec![])),
    }
}

/// Disconnect a remote session by chat_id.
#[tauri::command]
pub async fn disconnect_remote_session(
    chat_id: i64,
    state: State<'_, RemoteState>,
) -> Result<CommandResponse<()>, String> {
    let guard = state.gateway.read().await;
    match guard.as_ref() {
        Some(gateway) => match gateway.disconnect_session(chat_id).await {
            Ok(()) => Ok(CommandResponse::ok(())),
            Err(e) => Ok(CommandResponse::err(e.to_string())),
        },
        None => Ok(CommandResponse::err(
            "Gateway is not running".to_string(),
        )),
    }
}

/// Query remote audit log with pagination.
#[tauri::command]
pub async fn get_remote_audit_log(
    limit: Option<u32>,
    offset: Option<u32>,
    state: State<'_, AppState>,
) -> Result<CommandResponse<AuditLogResponse>, String> {
    let limit = limit.unwrap_or(50).min(200);
    let offset = offset.unwrap_or(0);

    let result = state
        .with_database(|db| {
            let conn = db.get_connection()?;

            // Get total count
            let total: u32 = conn
                .query_row("SELECT COUNT(*) FROM remote_audit_log", [], |row| {
                    row.get(0)
                })
                .unwrap_or(0);

            // Query entries
            let mut stmt = conn
                .prepare(
                    "SELECT id, adapter_type, chat_id, user_id, username, command_text,
                            command_type, result_status, error_message, created_at
                     FROM remote_audit_log
                     ORDER BY created_at DESC
                     LIMIT ?1 OFFSET ?2",
                )
                .map_err(|e| {
                    crate::utils::error::AppError::Internal(format!(
                        "Failed to prepare audit query: {}",
                        e
                    ))
                })?;

            let entries: Vec<RemoteAuditEntry> = stmt
                .query_map(params![limit, offset], |row| {
                    Ok(RemoteAuditEntry {
                        id: row.get(0)?,
                        adapter_type: row.get(1)?,
                        chat_id: row.get(2)?,
                        user_id: row.get(3)?,
                        username: row.get(4)?,
                        command_text: row.get(5)?,
                        command_type: row.get(6)?,
                        result_status: row.get(7)?,
                        error_message: row.get(8)?,
                        created_at: row.get(9)?,
                    })
                })
                .map_err(|e| {
                    crate::utils::error::AppError::Internal(format!(
                        "Failed to query audit log: {}",
                        e
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(AuditLogResponse { entries, total })
        })
        .await;

    match result {
        Ok(response) => Ok(CommandResponse::ok(response)),
        Err(e) => Ok(CommandResponse::err(e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Auto-start
// ---------------------------------------------------------------------------

/// Check if the remote gateway should auto-start and start it if configured.
///
/// This is called during app initialization (init_app). Failures are logged
/// but do NOT block app startup -- the gateway status will show the error.
///
/// Returns `Ok(true)` if the gateway was started, `Ok(false)` if auto-start
/// is disabled, or `Err` with an error message.
pub async fn try_auto_start_gateway(
    remote_state: &RemoteState,
    app_state: &AppState,
) -> Result<bool, String> {
    // Read gateway config from database
    let config_result = app_state
        .with_database(|db| {
            let gateway_config: RemoteGatewayConfig = db
                .get_setting(REMOTE_CONFIG_KEY)?
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();
            Ok(gateway_config)
        })
        .await;

    let gateway_config = match config_result {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to read remote gateway config for auto-start: {}", e);
            return Err(format!("Failed to read config: {}", e));
        }
    };

    // Check if auto-start is enabled
    if !gateway_config.enabled || !gateway_config.auto_start {
        tracing::debug!(
            "Remote gateway auto-start skipped (enabled={}, auto_start={})",
            gateway_config.enabled,
            gateway_config.auto_start
        );
        return Ok(false);
    }

    // Read Telegram config and hydrate secrets
    let full_config_result = app_state
        .with_database(|db| {
            let mut telegram_config: Option<TelegramAdapterConfig> = db
                .get_setting(TELEGRAM_CONFIG_KEY)?
                .and_then(|json| serde_json::from_str(&json).ok());

            let keyring = KeyringService::new();
            if let Some(ref mut tg) = telegram_config {
                tg.bot_token = keyring.get_api_key(KEYRING_BOT_TOKEN).ok().flatten();
                tg.access_password = keyring
                    .get_api_key(KEYRING_ACCESS_PASSWORD)
                    .ok()
                    .flatten();
            }

            let proxy = resolve_provider_proxy(&keyring, db, "remote_telegram");

            Ok((gateway_config.clone(), telegram_config, proxy))
        })
        .await;

    let (gw_config, telegram_config, proxy) = match full_config_result {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to read telegram config for auto-start: {}", e);
            return Err(format!("Failed to read config: {}", e));
        }
    };

    // Get database for gateway
    let db_result = app_state
        .with_database(|db| Ok(Arc::new(db.clone())))
        .await;

    let db = match db_result {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("Failed to get database for auto-start: {}", e);
            return Err(format!("Database error: {}", e));
        }
    };

    // Create session bridge and load existing mappings
    let bridge = Arc::new(SessionBridge::new(db.clone()));
    if let Err(e) = bridge.load_mappings_from_db().await {
        tracing::warn!("Failed to load session mappings for auto-start: {}", e);
        // Continue anyway -- gateway can still function
    }

    // Create and start gateway
    let gateway = RemoteGatewayService::new(gw_config, telegram_config, bridge, db);
    match gateway.start(proxy.as_ref()).await {
        Ok(()) => {
            tracing::info!("Remote gateway auto-started successfully");
            let mut guard = remote_state.gateway.write().await;
            *guard = Some(gateway);
            Ok(true)
        }
        Err(e) => {
            tracing::warn!("Remote gateway auto-start failed: {}", e);
            Err(format!("Auto-start failed: {}", e))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::remote::types::{RemoteAdapterType, StreamingMode};

    #[test]
    fn test_remote_state_new() {
        let state = RemoteState::new();
        // Gateway should be None initially
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let guard = state.gateway.read().await;
            assert!(guard.is_none());
        });
    }

    #[test]
    fn test_remote_state_default() {
        let state = RemoteState::default();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let guard = state.gateway.read().await;
            assert!(guard.is_none());
        });
    }

    #[test]
    fn test_audit_log_response_serialize() {
        let response = AuditLogResponse {
            entries: vec![RemoteAuditEntry {
                id: "test-id".to_string(),
                adapter_type: "Telegram".to_string(),
                chat_id: 123,
                user_id: 456,
                username: Some("testuser".to_string()),
                command_text: "/help".to_string(),
                command_type: "Help".to_string(),
                result_status: "success".to_string(),
                error_message: None,
                created_at: "2026-02-18T12:00:00Z".to_string(),
            }],
            total: 1,
        };
        let json = serde_json::to_string(&response).unwrap();
        let parsed: AuditLogResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.total, 1);
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].command_type, "Help");
    }

    #[test]
    fn test_keyring_key_constants() {
        assert_eq!(KEYRING_BOT_TOKEN, "remote_telegram_bot_token");
        assert_eq!(KEYRING_ACCESS_PASSWORD, "remote_access_password");
    }

    #[test]
    fn test_settings_key_constants() {
        assert_eq!(REMOTE_CONFIG_KEY, "remote_gateway_config");
        assert_eq!(TELEGRAM_CONFIG_KEY, "remote_telegram_config");
    }

    #[test]
    fn test_update_remote_config_request_partial() {
        let request = UpdateRemoteConfigRequest {
            enabled: Some(true),
            adapter: None,
            auto_start: None,
        };
        assert!(request.enabled.is_some());
        assert!(request.adapter.is_none());
        assert!(request.auto_start.is_none());
    }

    #[test]
    fn test_update_telegram_config_request_partial() {
        let request = UpdateTelegramConfigRequest {
            bot_token: Some("test-token".to_string()),
            allowed_chat_ids: None,
            allowed_user_ids: None,
            require_password: None,
            access_password: None,
            max_message_length: None,
            streaming_mode: None,
        };
        assert!(request.bot_token.is_some());
        assert!(request.allowed_chat_ids.is_none());
    }

    #[test]
    fn test_audit_log_query_with_pagination() {
        let db = crate::storage::Database::new_in_memory().unwrap();
        let conn = db.get_connection().unwrap();

        // Insert test entries
        for i in 0..10 {
            conn.execute(
                "INSERT INTO remote_audit_log (id, adapter_type, chat_id, user_id, username, command_text, command_type, result_status, error_message, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    format!("id-{}", i),
                    "Telegram",
                    123i64,
                    456i64,
                    "testuser",
                    format!("/cmd-{}", i),
                    "Help",
                    "success",
                    None::<String>,
                    format!("2026-02-18T12:00:{:02}Z", i),
                ],
            )
            .unwrap();
        }

        // Query with limit
        let mut stmt = conn
            .prepare(
                "SELECT id, adapter_type, chat_id, user_id, username, command_text,
                        command_type, result_status, error_message, created_at
                 FROM remote_audit_log
                 ORDER BY created_at DESC
                 LIMIT ?1 OFFSET ?2",
            )
            .unwrap();

        let entries: Vec<RemoteAuditEntry> = stmt
            .query_map(params![5u32, 0u32], |row| {
                Ok(RemoteAuditEntry {
                    id: row.get(0)?,
                    adapter_type: row.get(1)?,
                    chat_id: row.get(2)?,
                    user_id: row.get(3)?,
                    username: row.get(4)?,
                    command_text: row.get(5)?,
                    command_type: row.get(6)?,
                    result_status: row.get(7)?,
                    error_message: row.get(8)?,
                    created_at: row.get(9)?,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(entries.len(), 5);

        // Verify offset works
        let entries_offset: Vec<RemoteAuditEntry> = conn
            .prepare(
                "SELECT id, adapter_type, chat_id, user_id, username, command_text,
                        command_type, result_status, error_message, created_at
                 FROM remote_audit_log
                 ORDER BY created_at DESC
                 LIMIT ?1 OFFSET ?2",
            )
            .unwrap()
            .query_map(params![5u32, 5u32], |row| {
                Ok(RemoteAuditEntry {
                    id: row.get(0)?,
                    adapter_type: row.get(1)?,
                    chat_id: row.get(2)?,
                    user_id: row.get(3)?,
                    username: row.get(4)?,
                    command_text: row.get(5)?,
                    command_type: row.get(6)?,
                    result_status: row.get(7)?,
                    error_message: row.get(8)?,
                    created_at: row.get(9)?,
                })
            })
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(entries_offset.len(), 5);

        // Total count
        let total: u32 = conn
            .query_row("SELECT COUNT(*) FROM remote_audit_log", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(total, 10);
    }

    #[test]
    fn test_auto_start_config_check() {
        // Test that auto-start is only enabled when both enabled=true AND auto_start=true
        let config_disabled = RemoteGatewayConfig {
            enabled: false,
            auto_start: true,
            ..Default::default()
        };
        assert!(!config_disabled.enabled || !config_disabled.auto_start);
        // enabled=false -> should not auto-start
        assert!(!(config_disabled.enabled && config_disabled.auto_start));

        let config_no_auto = RemoteGatewayConfig {
            enabled: true,
            auto_start: false,
            ..Default::default()
        };
        assert!(!(config_no_auto.enabled && config_no_auto.auto_start));

        let config_both = RemoteGatewayConfig {
            enabled: true,
            auto_start: true,
            ..Default::default()
        };
        assert!(config_both.enabled && config_both.auto_start);
    }
}
