//! Remote Gateway Service
//!
//! Manages the adapter lifecycle, processes incoming messages via CommandRouter,
//! and coordinates with SessionBridge. Implements the 5-layer security model
//! and audit logging. Dispatches webhook notifications when remote commands
//! trigger task completions.

use super::adapters::telegram::TelegramAdapter;
use super::adapters::RemoteAdapter;
use super::command_router::{CommandRouter, HELP_TEXT};
use super::response_mapper::ResponseMapper;
use super::session_bridge::SessionBridge;
use super::types::{
    GatewayStatus, IncomingRemoteMessage, ReconnectConfig, RemoteCommand, RemoteError,
    RemoteGatewayConfig, TelegramAdapterConfig,
};
use crate::services::proxy::ProxyConfig;
use crate::services::webhook::integration::format_remote_source;
use crate::services::webhook::types::{WebhookEventType, WebhookPayload};
use crate::services::webhook::WebhookService;
use crate::storage::Database;
use rusqlite::params;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_util::sync::CancellationToken;

/// Remote Gateway Service managing adapter lifecycle and message processing.
pub struct RemoteGatewayService {
    pub(crate) config: RwLock<RemoteGatewayConfig>,
    pub(crate) telegram_config: RwLock<Option<TelegramAdapterConfig>>,
    pub(crate) adapter: Arc<RwLock<Option<Box<dyn RemoteAdapter>>>>,
    pub(crate) session_bridge: Arc<SessionBridge>,
    pub(crate) webhook_service: Option<Arc<WebhookService>>,
    pub(crate) status: Arc<RwLock<GatewayStatus>>,
    pub(crate) cancel_token: CancellationToken,
    pub(crate) db: Arc<Database>,
    /// Chats that have authenticated with password (Layer 4)
    pub(crate) authenticated_chats: Arc<RwLock<HashSet<i64>>>,
    /// Configuration for reconnect behavior with exponential backoff.
    pub(crate) reconnect_config: ReconnectConfig,
}

impl RemoteGatewayService {
    /// Create a new RemoteGatewayService.
    pub fn new(
        config: RemoteGatewayConfig,
        telegram_config: Option<TelegramAdapterConfig>,
        session_bridge: Arc<SessionBridge>,
        db: Arc<Database>,
    ) -> Self {
        Self {
            config: RwLock::new(config),
            telegram_config: RwLock::new(telegram_config),
            adapter: Arc::new(RwLock::new(None)),
            session_bridge,
            webhook_service: None,
            status: Arc::new(RwLock::new(GatewayStatus::default())),
            cancel_token: CancellationToken::new(),
            db,
            authenticated_chats: Arc::new(RwLock::new(HashSet::new())),
            reconnect_config: ReconnectConfig::default(),
        }
    }

    /// Set a custom reconnect configuration.
    pub fn set_reconnect_config(&mut self, config: ReconnectConfig) {
        self.reconnect_config = config;
    }

    /// Set the webhook service for dispatching notifications on remote command completion.
    pub fn set_webhook_service(&mut self, webhook_service: Arc<WebhookService>) {
        self.webhook_service = Some(webhook_service);
    }

    /// Get current gateway status.
    pub async fn get_status(&self) -> GatewayStatus {
        let mut status = self.status.read().await.clone();
        status.active_remote_sessions = self.session_bridge.active_session_count().await;
        status
    }

    /// Start the remote gateway with adapter and message processing loop.
    pub async fn start(&self, proxy: Option<&ProxyConfig>) -> Result<(), RemoteError> {
        let config = self.config.read().await;
        if !config.enabled {
            return Err(RemoteError::NotEnabled);
        }

        // Create the adapter based on config
        let telegram_config_guard = self.telegram_config.read().await;
        let telegram_config = telegram_config_guard
            .as_ref()
            .ok_or_else(|| RemoteError::ConfigError("Telegram config not set".to_string()))?
            .clone();

        let adapter: Box<dyn RemoteAdapter> =
            Box::new(TelegramAdapter::new(telegram_config.clone(), proxy)?);

        // Create message channel
        let (tx, mut rx) = mpsc::channel::<IncomingRemoteMessage>(100);

        // Start adapter
        adapter.start(tx).await?;

        // Store adapter
        {
            let mut adapter_guard = self.adapter.write().await;
            *adapter_guard = Some(adapter);
        }

        // Spawn message processing loop
        let bridge = self.session_bridge.clone();
        let adapter_ref = self.adapter.clone();
        let status_ref = self.status.clone();
        let db_ref = self.db.clone();
        let cancel = self.cancel_token.clone();
        let require_password = telegram_config.require_password;
        let access_password = telegram_config.access_password.clone();
        let authenticated_chats = self.authenticated_chats.clone();
        let webhook_service = self.webhook_service.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(msg) = rx.recv() => {
                        Self::handle_message(
                            &msg,
                            &bridge,
                            &adapter_ref,
                            &status_ref,
                            &db_ref,
                            require_password,
                            access_password.as_deref(),
                            &authenticated_chats,
                            webhook_service.as_ref(),
                        ).await;
                    }
                    _ = cancel.cancelled() => {
                        break;
                    }
                }
            }
        });

        // Update status
        let mut status = self.status.write().await;
        status.running = true;
        status.connected_since = Some(chrono::Utc::now().to_rfc3339());
        status.error = None;

        Ok(())
    }

    /// Handle an incoming remote message.
    async fn handle_message(
        msg: &IncomingRemoteMessage,
        bridge: &SessionBridge,
        adapter: &RwLock<Option<Box<dyn RemoteAdapter>>>,
        status: &RwLock<GatewayStatus>,
        db: &Database,
        require_password: bool,
        access_password: Option<&str>,
        authenticated_chats: &RwLock<HashSet<i64>>,
        webhook_service: Option<&Arc<WebhookService>>,
    ) {
        // Update stats
        {
            let mut s = status.write().await;
            s.total_commands_processed += 1;
            s.last_command_at = Some(chrono::Utc::now().to_rfc3339());
        }

        let adapter_guard = adapter.read().await;
        let adapter = match adapter_guard.as_ref() {
            Some(a) => a,
            None => return,
        };

        // Send typing indicator
        let _ = adapter.send_typing(msg.chat_id).await;

        // Layer 4: Password gate check
        if require_password {
            let is_authenticated = authenticated_chats.read().await.contains(&msg.chat_id);
            if !is_authenticated {
                // Check if this message is an /auth command
                let text = msg.text.trim();
                if text.starts_with("/auth ") {
                    let provided_password = text[6..].trim();
                    if let Some(expected) = access_password {
                        if provided_password == expected {
                            authenticated_chats.write().await.insert(msg.chat_id);
                            let _ = adapter
                                .send_message(msg.chat_id, "Authenticated successfully.")
                                .await;
                            Self::write_audit_log(db, msg, "Auth", "success", None);
                            return;
                        }
                    }
                    let _ = adapter
                        .send_message(msg.chat_id, "Authentication failed. Invalid password.")
                        .await;
                    Self::write_audit_log(
                        db,
                        msg,
                        "Auth",
                        "unauthorized",
                        Some("Invalid password"),
                    );
                    return;
                } else {
                    let _ = adapter
                        .send_message(
                            msg.chat_id,
                            "Authentication required. Send /auth <password> to authenticate.",
                        )
                        .await;
                    Self::write_audit_log(
                        db,
                        msg,
                        "Unauthenticated",
                        "unauthorized",
                        Some("Not authenticated"),
                    );
                    return;
                }
            }
        }

        // Parse command
        let command = CommandRouter::parse(&msg.text);
        let command_type = Self::command_type_name(&command);

        // Build remote source identifier for webhook notifications
        let remote_source =
            format_remote_source(&msg.adapter_type.to_string(), msg.username.as_deref());

        // Track whether this is a task-producing command for webhook dispatch
        let mut should_dispatch_webhook = false;
        let mut webhook_event_type = WebhookEventType::TaskComplete;

        // Process command through SessionBridge
        let response = match command {
            RemoteCommand::NewSession {
                project_path,
                provider,
                model,
            } => {
                match bridge
                    .create_session_with_source(
                        msg.chat_id,
                        msg.user_id,
                        &project_path,
                        provider.as_deref(),
                        model.as_deref(),
                        Some(&msg.adapter_type.to_string()),
                        msg.username.as_deref(),
                    )
                    .await
                {
                    Ok(id) => ResponseMapper::format_session_created(&id, &project_path),
                    Err(e) => ResponseMapper::format_error(&e),
                }
            }
            RemoteCommand::SendMessage { content } => {
                match bridge.send_message(msg.chat_id, &content).await {
                    Ok(resp) => {
                        // Task completed via remote command -- dispatch webhook
                        should_dispatch_webhook = true;
                        webhook_event_type = WebhookEventType::TaskComplete;
                        ResponseMapper::format_response(&resp)
                    }
                    Err(RemoteError::NoActiveSession) => {
                        "No active session. Use /new <path> to create one.".to_string()
                    }
                    Err(e) => {
                        // Task failed via remote command -- dispatch webhook
                        should_dispatch_webhook = true;
                        webhook_event_type = WebhookEventType::TaskFailed;
                        ResponseMapper::format_error(&e)
                    }
                }
            }
            RemoteCommand::ListSessions => bridge.list_sessions_text(msg.chat_id).await,
            RemoteCommand::SwitchSession { session_id } => {
                match bridge.switch_session(msg.chat_id, &session_id).await {
                    Ok(()) => format!("Switched to session: {}", session_id),
                    Err(e) => ResponseMapper::format_error(&e),
                }
            }
            RemoteCommand::Status => bridge.get_status_text(msg.chat_id).await,
            RemoteCommand::Cancel => match bridge.cancel_execution(msg.chat_id).await {
                Ok(()) => "Execution cancelled.".to_string(),
                Err(e) => ResponseMapper::format_error(&e),
            },
            RemoteCommand::CloseSession => match bridge.close_session(msg.chat_id).await {
                Ok(()) => "Session closed.".to_string(),
                Err(e) => ResponseMapper::format_error(&e),
            },
            RemoteCommand::Help => HELP_TEXT.to_string(),
        };

        // Send response
        let result_status = if response.contains("Error:") {
            "error"
        } else {
            "success"
        };

        let _ = adapter.send_message(msg.chat_id, &response).await;

        // Write audit log
        Self::write_audit_log(db, msg, command_type, result_status, None);

        // Dispatch webhook notification for task-producing commands
        if should_dispatch_webhook {
            if let Some(webhook_svc) = webhook_service {
                let session_id = bridge
                    .get_active_session_id(msg.chat_id)
                    .await
                    .unwrap_or_default();

                let summary = match webhook_event_type {
                    WebhookEventType::TaskComplete => {
                        format!("Task completed successfully ({})", remote_source)
                    }
                    WebhookEventType::TaskFailed => {
                        format!("Task failed ({})", remote_source)
                    }
                    _ => response.clone(),
                };

                let payload = WebhookPayload {
                    event_type: webhook_event_type,
                    session_id: if session_id.is_empty() {
                        None
                    } else {
                        Some(session_id)
                    },
                    session_name: None,
                    project_path: None,
                    summary,
                    details: None,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    duration_ms: None,
                    token_usage: None,
                    remote_source: Some(remote_source.clone()),
                };

                let svc = webhook_svc.clone();
                tokio::spawn(async move {
                    let deliveries = svc.dispatch(payload).await;
                    if !deliveries.is_empty() {
                        tracing::debug!(
                            "Webhook dispatched {} deliveries for remote command",
                            deliveries.len()
                        );
                    }
                });
            }
        }
    }

    /// Get the command type name for audit logging.
    fn command_type_name(command: &RemoteCommand) -> &'static str {
        match command {
            RemoteCommand::NewSession { .. } => "NewSession",
            RemoteCommand::SendMessage { .. } => "SendMessage",
            RemoteCommand::ListSessions => "ListSessions",
            RemoteCommand::SwitchSession { .. } => "SwitchSession",
            RemoteCommand::Status => "Status",
            RemoteCommand::Cancel => "Cancel",
            RemoteCommand::CloseSession => "CloseSession",
            RemoteCommand::Help => "Help",
        }
    }

    /// Write an audit log entry to the remote_audit_log table.
    fn write_audit_log(
        db: &Database,
        msg: &IncomingRemoteMessage,
        command_type: &str,
        result_status: &str,
        error_message: Option<&str>,
    ) {
        let id = uuid::Uuid::new_v4().to_string();
        let adapter_type = msg.adapter_type.to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        if let Ok(conn) = db.get_connection() {
            let _ = conn.execute(
                "INSERT INTO remote_audit_log (id, adapter_type, chat_id, user_id, username, command_text, command_type, result_status, error_message, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    id,
                    adapter_type,
                    msg.chat_id,
                    msg.user_id,
                    msg.username,
                    msg.text,
                    command_type,
                    result_status,
                    error_message,
                    created_at,
                ],
            );
        }
    }

    /// Record a connection error in the gateway status and attempt reconnect.
    ///
    /// Returns `true` if a reconnect was attempted and succeeded, `false` otherwise.
    pub async fn record_connection_error(&self, error: &str) -> bool {
        let should_reconnect = {
            let mut status = self.status.write().await;
            status.error = Some(error.to_string());
            status.last_error_at = Some(chrono::Utc::now().to_rfc3339());
            status.reconnect_attempts += 1;

            let attempt = status.reconnect_attempts;
            if attempt <= self.reconnect_config.max_attempts {
                status.reconnecting = true;
                tracing::warn!(
                    "Connection error (attempt {}/{}): {}",
                    attempt,
                    self.reconnect_config.max_attempts,
                    error
                );
                Some(attempt)
            } else {
                status.reconnecting = false;
                tracing::error!(
                    "Max reconnect attempts ({}) exceeded. Giving up.",
                    self.reconnect_config.max_attempts
                );
                None
            }
        };

        if let Some(attempt) = should_reconnect {
            let delay = self.reconnect_config.delay_for_attempt(attempt - 1);
            tracing::info!(
                "Waiting {}ms before reconnect attempt {}...",
                delay,
                attempt
            );
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;

            match self.start(None).await {
                Ok(()) => {
                    let mut status = self.status.write().await;
                    status.reconnect_attempts = 0;
                    status.reconnecting = false;
                    status.error = None;
                    tracing::info!("Reconnected successfully after {} attempt(s)", attempt);
                    true
                }
                Err(e) => {
                    tracing::warn!("Reconnect attempt {} failed: {}", attempt, e);
                    false
                }
            }
        } else {
            false
        }
    }

    /// Reset reconnect state (e.g., after a successful manual start).
    pub async fn reset_reconnect_state(&self) {
        let mut status = self.status.write().await;
        status.reconnect_attempts = 0;
        status.last_error_at = None;
        status.reconnecting = false;
    }

    /// Stop the gateway gracefully.
    pub async fn stop(&self) -> Result<(), RemoteError> {
        self.cancel_token.cancel();
        if let Some(adapter) = self.adapter.read().await.as_ref() {
            adapter.stop().await?;
        }
        let mut status = self.status.write().await;
        status.running = false;
        status.connected_since = None;
        // Reset reconnect state on clean stop
        status.reconnect_attempts = 0;
        status.reconnecting = false;
        Ok(())
    }

    /// Update gateway configuration (requires restart to take effect).
    pub async fn update_config(&self, config: RemoteGatewayConfig) -> Result<(), RemoteError> {
        let mut current = self.config.write().await;
        *current = config;
        Ok(())
    }

    /// Update Telegram adapter configuration.
    pub async fn update_telegram_config(
        &self,
        config: TelegramAdapterConfig,
    ) -> Result<(), RemoteError> {
        let mut current = self.telegram_config.write().await;
        *current = Some(config);
        Ok(())
    }

    /// Disconnect a specific remote session by chat_id.
    pub async fn disconnect_session(&self, chat_id: i64) -> Result<(), RemoteError> {
        self.session_bridge.close_session(chat_id).await
    }

    /// Get all remote session mappings.
    pub async fn list_sessions(&self) -> Vec<super::types::RemoteSessionMapping> {
        self.session_bridge.list_all_sessions().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::remote::types::{IncomingRemoteMessage, RemoteAdapterType, RemoteCommand};
    use crate::services::webhook::integration::format_remote_source;

    #[test]
    fn test_command_type_name() {
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::Help),
            "Help"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::Status),
            "Status"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::Cancel),
            "Cancel"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::ListSessions),
            "ListSessions"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::CloseSession),
            "CloseSession"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::NewSession {
                project_path: "".to_string(),
                provider: None,
                model: None,
            }),
            "NewSession"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::SendMessage {
                content: "hi".to_string(),
            }),
            "SendMessage"
        );
        assert_eq!(
            RemoteGatewayService::command_type_name(&RemoteCommand::SwitchSession {
                session_id: "x".to_string(),
            }),
            "SwitchSession"
        );
    }

    #[test]
    fn test_write_audit_log() {
        let db = Database::new_in_memory().unwrap();
        let msg = IncomingRemoteMessage {
            adapter_type: RemoteAdapterType::Telegram,
            chat_id: 123,
            user_id: 456,
            username: Some("testuser".to_string()),
            text: "/help".to_string(),
            message_id: 1,
            timestamp: chrono::Utc::now(),
        };

        RemoteGatewayService::write_audit_log(&db, &msg, "Help", "success", None);

        let conn = db.get_connection().unwrap();
        let (cmd_type, status): (String, String) = conn
            .query_row(
                "SELECT command_type, result_status FROM remote_audit_log ORDER BY created_at DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(cmd_type, "Help");
        assert_eq!(status, "success");
    }

    #[test]
    fn test_write_audit_log_with_error() {
        let db = Database::new_in_memory().unwrap();
        let msg = IncomingRemoteMessage {
            adapter_type: RemoteAdapterType::Telegram,
            chat_id: 123,
            user_id: 456,
            username: None,
            text: "/new ~/secret".to_string(),
            message_id: 2,
            timestamp: chrono::Utc::now(),
        };

        RemoteGatewayService::write_audit_log(
            &db,
            &msg,
            "NewSession",
            "error",
            Some("Unauthorized path"),
        );

        let conn = db.get_connection().unwrap();
        let error_msg: Option<String> = conn
            .query_row(
                "SELECT error_message FROM remote_audit_log ORDER BY created_at DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(error_msg, Some("Unauthorized path".to_string()));
    }

    #[tokio::test]
    async fn test_gateway_new() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db);
        let status = gateway.get_status().await;
        assert!(!status.running);
        assert_eq!(status.total_commands_processed, 0);
        assert!(gateway.webhook_service.is_none());
    }

    #[tokio::test]
    async fn test_gateway_with_webhook_service() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let mut gateway = RemoteGatewayService::new(config, None, bridge, db.clone());
        assert!(gateway.webhook_service.is_none());

        let keyring = Arc::new(crate::storage::KeyringService::new());
        let webhook_svc = Arc::new(WebhookService::new(db, keyring, |_| None));
        gateway.set_webhook_service(webhook_svc);
        assert!(gateway.webhook_service.is_some());
    }

    #[tokio::test]
    async fn test_gateway_start_not_enabled() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig {
            enabled: false,
            ..Default::default()
        };

        let gateway = RemoteGatewayService::new(config, None, bridge, db);
        let result = gateway.start(None).await;
        assert!(result.is_err());
        match result {
            Err(RemoteError::NotEnabled) => {}
            _ => panic!("Expected NotEnabled error"),
        }
    }

    #[tokio::test]
    async fn test_gateway_status_tracking() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db);

        // Manually update status to simulate running
        {
            let mut status = gateway.status.write().await;
            status.running = true;
            status.total_commands_processed = 42;
            status.last_command_at = Some("2026-02-18T14:30:00Z".to_string());
        }

        let status = gateway.get_status().await;
        assert!(status.running);
        assert_eq!(status.total_commands_processed, 42);
    }

    #[tokio::test]
    async fn test_gateway_stop() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db);

        // Set running status
        {
            let mut status = gateway.status.write().await;
            status.running = true;
        }

        gateway.stop().await.unwrap();

        let status = gateway.get_status().await;
        assert!(!status.running);
    }

    #[tokio::test]
    async fn test_gateway_update_config() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db);

        let new_config = RemoteGatewayConfig {
            enabled: true,
            auto_start: true,
            ..Default::default()
        };

        gateway.update_config(new_config).await.unwrap();

        let config = gateway.config.read().await;
        assert!(config.enabled);
        assert!(config.auto_start);
    }

    #[tokio::test]
    async fn test_gateway_update_telegram_config() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db);

        let tg_config = TelegramAdapterConfig {
            bot_token: Some("test-token".to_string()),
            allowed_chat_ids: vec![123],
            ..Default::default()
        };

        gateway.update_telegram_config(tg_config).await.unwrap();

        let tg = gateway.telegram_config.read().await;
        assert!(tg.is_some());
        assert_eq!(tg.as_ref().unwrap().allowed_chat_ids, vec![123]);
    }

    #[tokio::test]
    async fn test_password_authentication_logic() {
        // Test the authentication flow
        let authenticated: RwLock<HashSet<i64>> = RwLock::new(HashSet::new());

        // Chat 123 is not authenticated
        assert!(!authenticated.read().await.contains(&123));

        // Authenticate chat 123
        authenticated.write().await.insert(123);

        // Now chat 123 is authenticated
        assert!(authenticated.read().await.contains(&123));

        // Chat 456 is still not authenticated
        assert!(!authenticated.read().await.contains(&456));
    }

    #[test]
    fn test_remote_source_formatting_in_gateway() {
        let source = format_remote_source("Telegram", Some("testuser"));
        assert_eq!(source, "via Telegram @testuser");

        let source_no_user = format_remote_source("Telegram", None);
        assert_eq!(source_no_user, "via Telegram");
    }

    #[tokio::test]
    async fn test_gateway_reconnect_config_default() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db);
        assert_eq!(gateway.reconnect_config.max_attempts, 5);
        assert_eq!(gateway.reconnect_config.base_delay_ms, 1000);
        assert_eq!(gateway.reconnect_config.max_delay_ms, 30000);
    }

    #[tokio::test]
    async fn test_gateway_set_reconnect_config() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let mut gateway = RemoteGatewayService::new(config, None, bridge, db);
        gateway.set_reconnect_config(ReconnectConfig {
            max_attempts: 3,
            base_delay_ms: 500,
            max_delay_ms: 5000,
        });
        assert_eq!(gateway.reconnect_config.max_attempts, 3);
        assert_eq!(gateway.reconnect_config.base_delay_ms, 500);
        assert_eq!(gateway.reconnect_config.max_delay_ms, 5000);
    }

    #[tokio::test]
    async fn test_gateway_reconnect_state_tracking() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db);

        // Simulate connection error by directly updating status
        {
            let mut status = gateway.status.write().await;
            status.reconnect_attempts = 2;
            status.last_error_at = Some("2026-02-18T15:00:00Z".to_string());
            status.reconnecting = true;
            status.error = Some("Connection lost".to_string());
        }

        let status = gateway.get_status().await;
        assert_eq!(status.reconnect_attempts, 2);
        assert!(status.last_error_at.is_some());
        assert!(status.reconnecting);
        assert_eq!(status.error, Some("Connection lost".to_string()));
    }

    #[tokio::test]
    async fn test_gateway_reset_reconnect_state() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db);

        // Simulate some reconnect state
        {
            let mut status = gateway.status.write().await;
            status.reconnect_attempts = 3;
            status.last_error_at = Some("2026-02-18T15:00:00Z".to_string());
            status.reconnecting = true;
        }

        // Reset
        gateway.reset_reconnect_state().await;

        let status = gateway.get_status().await;
        assert_eq!(status.reconnect_attempts, 0);
        assert!(status.last_error_at.is_none());
        assert!(!status.reconnecting);
    }

    #[tokio::test]
    async fn test_gateway_stop_resets_reconnect_state() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig::default();

        let gateway = RemoteGatewayService::new(config, None, bridge, db);

        // Set running and reconnect state
        {
            let mut status = gateway.status.write().await;
            status.running = true;
            status.reconnect_attempts = 2;
            status.reconnecting = true;
        }

        gateway.stop().await.unwrap();

        let status = gateway.get_status().await;
        assert!(!status.running);
        assert_eq!(status.reconnect_attempts, 0);
        assert!(!status.reconnecting);
    }

    #[tokio::test]
    async fn test_gateway_record_connection_error_increments_attempts() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig {
            enabled: false, // Keep disabled so start() fails fast in reconnect
            ..Default::default()
        };

        let mut gateway = RemoteGatewayService::new(config, None, bridge, db);
        // Use fast backoff for test
        gateway.set_reconnect_config(ReconnectConfig {
            max_attempts: 2,
            base_delay_ms: 1, // 1ms for fast tests
            max_delay_ms: 10,
        });

        // First error
        let reconnected = gateway.record_connection_error("test error 1").await;
        assert!(!reconnected); // start() fails because not enabled

        let status = gateway.get_status().await;
        // After failed reconnect, attempts stays at 1
        assert_eq!(status.reconnect_attempts, 1);
        assert!(status.error.is_some());
        assert!(status.last_error_at.is_some());
    }

    #[tokio::test]
    async fn test_gateway_record_connection_error_max_attempts() {
        let db = Arc::new(Database::new_in_memory().unwrap());
        let bridge = Arc::new(SessionBridge::new(db.clone()));
        let config = RemoteGatewayConfig {
            enabled: false,
            ..Default::default()
        };

        let mut gateway = RemoteGatewayService::new(config, None, bridge, db);
        gateway.set_reconnect_config(ReconnectConfig {
            max_attempts: 2,
            base_delay_ms: 1,
            max_delay_ms: 10,
        });

        // Manually set attempts to max
        {
            let mut status = gateway.status.write().await;
            status.reconnect_attempts = 2;
        }

        // Next error should exceed max, returning false without attempting reconnect
        let reconnected = gateway.record_connection_error("final error").await;
        assert!(!reconnected);

        let status = gateway.get_status().await;
        assert_eq!(status.reconnect_attempts, 3); // incremented past max
        assert!(!status.reconnecting); // gave up
    }
}
