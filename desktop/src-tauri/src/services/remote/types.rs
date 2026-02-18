//! Remote Session Control Types
//!
//! Core types for the remote session control feature including configuration,
//! commands, status tracking, session mapping, and error handling.

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Adapter & Configuration Types
// ---------------------------------------------------------------------------

/// Remote adapter type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RemoteAdapterType {
    Telegram,
    // Future: Slack, Discord, WebSocket API, etc.
}

impl fmt::Display for RemoteAdapterType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteAdapterType::Telegram => write!(f, "telegram"),
        }
    }
}

/// Remote gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteGatewayConfig {
    pub enabled: bool,
    pub adapter: RemoteAdapterType,
    pub auto_start: bool,
}

impl Default for RemoteGatewayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            adapter: RemoteAdapterType::Telegram,
            auto_start: false,
        }
    }
}

/// Telegram-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramAdapterConfig {
    #[serde(skip_serializing, default)]
    pub bot_token: Option<String>,
    pub allowed_chat_ids: Vec<i64>,
    pub allowed_user_ids: Vec<i64>,
    pub require_password: bool,
    #[serde(skip_serializing, default)]
    pub access_password: Option<String>,
    #[serde(default = "default_max_message_length")]
    pub max_message_length: usize,
    pub streaming_mode: StreamingMode,
}

fn default_max_message_length() -> usize {
    4000
}

impl Default for TelegramAdapterConfig {
    fn default() -> Self {
        Self {
            bot_token: None,
            allowed_chat_ids: Vec::new(),
            allowed_user_ids: Vec::new(),
            require_password: false,
            access_password: None,
            max_message_length: 4000,
            streaming_mode: StreamingMode::WaitForComplete,
        }
    }
}

/// How to handle streaming LLM output
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StreamingMode {
    /// Wait for completion, send final result
    WaitForComplete,
    /// Send periodic progress updates (every N seconds)
    PeriodicUpdate { interval_secs: u32 },
    /// Edit message in-place with latest content (Telegram editMessageText)
    LiveEdit { throttle_ms: u64 },
}

impl Default for StreamingMode {
    fn default() -> Self {
        Self::WaitForComplete
    }
}

// ---------------------------------------------------------------------------
// Command Types
// ---------------------------------------------------------------------------

/// Remote command parsed from user message
#[derive(Debug, Clone, PartialEq)]
pub enum RemoteCommand {
    /// /new <path> [provider] [model] - Create new session
    NewSession {
        project_path: String,
        provider: Option<String>,
        model: Option<String>,
    },
    /// /send <message> or plain text - Send message to active session
    SendMessage { content: String },
    /// /sessions - List active sessions
    ListSessions,
    /// /switch <session_id> - Switch active session
    SwitchSession { session_id: String },
    /// /status - Get current session status
    Status,
    /// /cancel - Cancel current execution
    Cancel,
    /// /close - Close current session
    CloseSession,
    /// /help - Show available commands
    Help,
}

// ---------------------------------------------------------------------------
// Status & Session Types
// ---------------------------------------------------------------------------

/// Gateway runtime status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayStatus {
    pub running: bool,
    pub adapter_type: RemoteAdapterType,
    pub connected_since: Option<String>,
    pub active_remote_sessions: u32,
    pub total_commands_processed: u64,
    pub last_command_at: Option<String>,
    pub error: Option<String>,
    /// Number of reconnect attempts since last successful connection
    #[serde(default)]
    pub reconnect_attempts: u32,
    /// Timestamp of the last connection error
    #[serde(default)]
    pub last_error_at: Option<String>,
    /// Whether the gateway is currently attempting to reconnect
    #[serde(default)]
    pub reconnecting: bool,
}

impl Default for GatewayStatus {
    fn default() -> Self {
        Self {
            running: false,
            adapter_type: RemoteAdapterType::Telegram,
            connected_since: None,
            active_remote_sessions: 0,
            total_commands_processed: 0,
            last_command_at: None,
            error: None,
            reconnect_attempts: 0,
            last_error_at: None,
            reconnecting: false,
        }
    }
}

/// Configuration for reconnect behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectConfig {
    /// Maximum number of reconnect attempts before giving up (default: 5)
    pub max_attempts: u32,
    /// Base delay in milliseconds for exponential backoff (default: 1000)
    pub base_delay_ms: u64,
    /// Maximum delay in milliseconds (default: 30000)
    pub max_delay_ms: u64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
        }
    }
}

impl ReconnectConfig {
    /// Calculate the delay for a given reconnect attempt using exponential backoff.
    ///
    /// Formula: `min(2^attempt * base_delay_ms, max_delay_ms)`
    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        let delay = self
            .base_delay_ms
            .saturating_mul(2u64.saturating_pow(attempt));
        delay.min(self.max_delay_ms)
    }
}

/// Mapping between remote chat and local session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSessionMapping {
    pub chat_id: i64,
    pub user_id: i64,
    pub local_session_id: Option<String>,
    pub session_type: SessionType,
    pub created_at: String,
    /// Adapter type that created this session (e.g., "Telegram")
    #[serde(default)]
    pub adapter_type_name: Option<String>,
    /// Username of the remote user who created this session (e.g., "@testuser")
    #[serde(default)]
    pub username: Option<String>,
}

/// Session type for remote-created sessions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionType {
    ClaudeCode,
    Standalone { provider: String, model: String },
}

impl fmt::Display for SessionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionType::ClaudeCode => write!(f, "ClaudeCode"),
            SessionType::Standalone { provider, model } => {
                write!(f, "Standalone({}/{})", provider, model)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Message Types
// ---------------------------------------------------------------------------

/// Incoming message from remote platform
#[derive(Debug, Clone)]
pub struct IncomingRemoteMessage {
    pub adapter_type: RemoteAdapterType,
    pub chat_id: i64,
    pub user_id: i64,
    pub username: Option<String>,
    pub text: String,
    pub message_id: i64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Response collected from a local session execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteResponse {
    pub text: String,
    pub thinking: Option<String>,
    pub tool_summary: Option<String>,
}

// ---------------------------------------------------------------------------
// Audit Types
// ---------------------------------------------------------------------------

/// Audit log entry for remote commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteAuditEntry {
    pub id: String,
    pub adapter_type: String,
    pub chat_id: i64,
    pub user_id: i64,
    pub username: Option<String>,
    pub command_text: String,
    pub command_type: String,
    pub result_status: String,
    pub error_message: Option<String>,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Error Types
// ---------------------------------------------------------------------------

/// Remote control error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum RemoteError {
    #[error("Remote gateway is not enabled")]
    NotEnabled,

    #[error("No active session for this chat")]
    NoActiveSession,

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Failed to send message: {0}")]
    SendFailed(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Unauthorized access")]
    Unauthorized,

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

// ---------------------------------------------------------------------------
// Request Types (for Tauri commands)
// ---------------------------------------------------------------------------

/// Request to update remote gateway config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRemoteConfigRequest {
    pub enabled: Option<bool>,
    pub adapter: Option<RemoteAdapterType>,
    pub auto_start: Option<bool>,
}

/// Request to update Telegram adapter config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTelegramConfigRequest {
    pub bot_token: Option<String>,
    pub allowed_chat_ids: Option<Vec<i64>>,
    pub allowed_user_ids: Option<Vec<i64>>,
    pub require_password: Option<bool>,
    pub access_password: Option<String>,
    pub max_message_length: Option<usize>,
    pub streaming_mode: Option<StreamingMode>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_adapter_type_serialize() {
        let t = RemoteAdapterType::Telegram;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"Telegram\"");

        let parsed: RemoteAdapterType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, RemoteAdapterType::Telegram);
    }

    #[test]
    fn test_remote_adapter_type_display() {
        assert_eq!(RemoteAdapterType::Telegram.to_string(), "telegram");
    }

    #[test]
    fn test_remote_gateway_config_default() {
        let config = RemoteGatewayConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.adapter, RemoteAdapterType::Telegram);
        assert!(!config.auto_start);
    }

    #[test]
    fn test_remote_gateway_config_serialize() {
        let config = RemoteGatewayConfig {
            enabled: true,
            adapter: RemoteAdapterType::Telegram,
            auto_start: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"enabled\":true"));
        assert!(json.contains("\"auto_start\":true"));

        let parsed: RemoteGatewayConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.enabled);
        assert!(parsed.auto_start);
    }

    #[test]
    fn test_telegram_adapter_config_default() {
        let config = TelegramAdapterConfig::default();
        assert!(config.bot_token.is_none());
        assert!(config.allowed_chat_ids.is_empty());
        assert!(config.allowed_user_ids.is_empty());
        assert!(!config.require_password);
        assert!(config.access_password.is_none());
        assert_eq!(config.max_message_length, 4000);
        assert_eq!(config.streaming_mode, StreamingMode::WaitForComplete);
    }

    #[test]
    fn test_telegram_adapter_config_skip_serializing_secrets() {
        let config = TelegramAdapterConfig {
            bot_token: Some("my-secret-token".to_string()),
            access_password: Some("my-password".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        // bot_token and access_password should NOT appear in serialized output
        assert!(!json.contains("my-secret-token"));
        assert!(!json.contains("my-password"));
    }

    #[test]
    fn test_streaming_mode_serialize() {
        let modes = vec![
            (StreamingMode::WaitForComplete, "\"WaitForComplete\""),
            (
                StreamingMode::PeriodicUpdate { interval_secs: 10 },
                "{\"PeriodicUpdate\":{\"interval_secs\":10}}",
            ),
            (
                StreamingMode::LiveEdit { throttle_ms: 2000 },
                "{\"LiveEdit\":{\"throttle_ms\":2000}}",
            ),
        ];

        for (mode, expected) in modes {
            let json = serde_json::to_string(&mode).unwrap();
            assert_eq!(json, expected);

            let parsed: StreamingMode = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, mode);
        }
    }

    #[test]
    fn test_gateway_status_default() {
        let status = GatewayStatus::default();
        assert!(!status.running);
        assert_eq!(status.adapter_type, RemoteAdapterType::Telegram);
        assert!(status.connected_since.is_none());
        assert_eq!(status.active_remote_sessions, 0);
        assert_eq!(status.total_commands_processed, 0);
        assert!(status.last_command_at.is_none());
        assert!(status.error.is_none());
        assert_eq!(status.reconnect_attempts, 0);
        assert!(status.last_error_at.is_none());
        assert!(!status.reconnecting);
    }

    #[test]
    fn test_gateway_status_serialize() {
        let status = GatewayStatus {
            running: true,
            adapter_type: RemoteAdapterType::Telegram,
            connected_since: Some("2026-02-18T14:30:00Z".to_string()),
            active_remote_sessions: 2,
            total_commands_processed: 47,
            last_command_at: Some("2026-02-18T14:35:00Z".to_string()),
            error: None,
            reconnect_attempts: 0,
            last_error_at: None,
            reconnecting: false,
        };
        let json = serde_json::to_string(&status).unwrap();
        let parsed: GatewayStatus = serde_json::from_str(&json).unwrap();
        assert!(parsed.running);
        assert_eq!(parsed.active_remote_sessions, 2);
        assert_eq!(parsed.total_commands_processed, 47);
    }

    #[test]
    fn test_gateway_status_backward_compat() {
        // Old JSON without reconnect fields should deserialize with defaults
        let old_json = r#"{
            "running": true,
            "adapter_type": "Telegram",
            "connected_since": null,
            "active_remote_sessions": 0,
            "total_commands_processed": 0,
            "last_command_at": null,
            "error": null
        }"#;
        let parsed: GatewayStatus = serde_json::from_str(old_json).unwrap();
        assert_eq!(parsed.reconnect_attempts, 0);
        assert!(parsed.last_error_at.is_none());
        assert!(!parsed.reconnecting);
    }

    #[test]
    fn test_reconnect_config_default() {
        let config = ReconnectConfig::default();
        assert_eq!(config.max_attempts, 5);
        assert_eq!(config.base_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
    }

    #[test]
    fn test_reconnect_config_delay_calculation() {
        let config = ReconnectConfig::default();

        // attempt 0: 2^0 * 1000 = 1000ms
        assert_eq!(config.delay_for_attempt(0), 1000);
        // attempt 1: 2^1 * 1000 = 2000ms
        assert_eq!(config.delay_for_attempt(1), 2000);
        // attempt 2: 2^2 * 1000 = 4000ms
        assert_eq!(config.delay_for_attempt(2), 4000);
        // attempt 3: 2^3 * 1000 = 8000ms
        assert_eq!(config.delay_for_attempt(3), 8000);
        // attempt 4: 2^4 * 1000 = 16000ms
        assert_eq!(config.delay_for_attempt(4), 16000);
        // attempt 5: 2^5 * 1000 = 32000ms -> capped at 30000
        assert_eq!(config.delay_for_attempt(5), 30000);
        // attempt 10: capped at 30000
        assert_eq!(config.delay_for_attempt(10), 30000);
    }

    #[test]
    fn test_reconnect_config_custom() {
        let config = ReconnectConfig {
            max_attempts: 3,
            base_delay_ms: 500,
            max_delay_ms: 5000,
        };
        assert_eq!(config.delay_for_attempt(0), 500);
        assert_eq!(config.delay_for_attempt(1), 1000);
        assert_eq!(config.delay_for_attempt(2), 2000);
        assert_eq!(config.delay_for_attempt(3), 4000);
        assert_eq!(config.delay_for_attempt(4), 5000); // capped
    }

    #[test]
    fn test_reconnect_config_serialize() {
        let config = ReconnectConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ReconnectConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_attempts, 5);
        assert_eq!(parsed.base_delay_ms, 1000);
        assert_eq!(parsed.max_delay_ms, 30000);
    }

    #[test]
    fn test_session_type_serialize() {
        let claude = SessionType::ClaudeCode;
        let json = serde_json::to_string(&claude).unwrap();
        assert_eq!(json, "\"ClaudeCode\"");

        let standalone = SessionType::Standalone {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-5-20250929".to_string(),
        };
        let json = serde_json::to_string(&standalone).unwrap();
        assert!(json.contains("anthropic"));
        assert!(json.contains("claude-sonnet-4-5-20250929"));

        let parsed: SessionType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, standalone);
    }

    #[test]
    fn test_session_type_display() {
        assert_eq!(SessionType::ClaudeCode.to_string(), "ClaudeCode");
        assert_eq!(
            SessionType::Standalone {
                provider: "openai".to_string(),
                model: "gpt-4".to_string()
            }
            .to_string(),
            "Standalone(openai/gpt-4)"
        );
    }

    #[test]
    fn test_remote_session_mapping_serialize() {
        let mapping = RemoteSessionMapping {
            chat_id: 123456789,
            user_id: 111222333,
            local_session_id: Some("session-abc-123".to_string()),
            session_type: SessionType::ClaudeCode,
            created_at: "2026-02-18T14:30:00Z".to_string(),
            adapter_type_name: Some("Telegram".to_string()),
            username: Some("testuser".to_string()),
        };
        let json = serde_json::to_string(&mapping).unwrap();
        let parsed: RemoteSessionMapping = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.chat_id, 123456789);
        assert_eq!(parsed.user_id, 111222333);
        assert_eq!(
            parsed.local_session_id,
            Some("session-abc-123".to_string())
        );
    }

    #[test]
    fn test_remote_session_mapping_with_source_info() {
        let mapping = RemoteSessionMapping {
            chat_id: 123456789,
            user_id: 111222333,
            local_session_id: Some("session-abc-123".to_string()),
            session_type: SessionType::ClaudeCode,
            created_at: "2026-02-18T14:30:00Z".to_string(),
            adapter_type_name: Some("Telegram".to_string()),
            username: Some("testuser".to_string()),
        };
        let json = serde_json::to_string(&mapping).unwrap();
        assert!(json.contains("Telegram"));
        assert!(json.contains("testuser"));
        let parsed: RemoteSessionMapping = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.adapter_type_name, Some("Telegram".to_string()));
        assert_eq!(parsed.username, Some("testuser".to_string()));
    }

    #[test]
    fn test_remote_session_mapping_backward_compat() {
        // Old JSON without adapter_type_name and username should deserialize with None defaults
        let old_json = r#"{
            "chat_id": 123,
            "user_id": 456,
            "local_session_id": "sess-1",
            "session_type": "ClaudeCode",
            "created_at": "2026-02-18T12:00:00Z"
        }"#;
        let parsed: RemoteSessionMapping = serde_json::from_str(old_json).unwrap();
        assert_eq!(parsed.chat_id, 123);
        assert!(parsed.adapter_type_name.is_none());
        assert!(parsed.username.is_none());
    }

    #[test]
    fn test_remote_response_serialize() {
        let response = RemoteResponse {
            text: "Hello world".to_string(),
            thinking: Some("Let me think...".to_string()),
            tool_summary: Some("[Grep]: found 5 matches".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap();
        let parsed: RemoteResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.text, "Hello world");
        assert_eq!(parsed.thinking, Some("Let me think...".to_string()));
    }

    #[test]
    fn test_remote_audit_entry_serialize() {
        let entry = RemoteAuditEntry {
            id: "audit-001".to_string(),
            adapter_type: "telegram".to_string(),
            chat_id: 123456789,
            user_id: 111222333,
            username: Some("testuser".to_string()),
            command_text: "/new ~/projects/myapp".to_string(),
            command_type: "NewSession".to_string(),
            result_status: "success".to_string(),
            error_message: None,
            created_at: "2026-02-18T14:30:00Z".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: RemoteAuditEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "audit-001");
        assert_eq!(parsed.command_type, "NewSession");
        assert_eq!(parsed.result_status, "success");
    }

    #[test]
    fn test_remote_error_variants() {
        let errors: Vec<RemoteError> = vec![
            RemoteError::NotEnabled,
            RemoteError::NoActiveSession,
            RemoteError::SessionNotFound("sess-123".to_string()),
            RemoteError::SendFailed("network timeout".to_string()),
            RemoteError::ExecutionFailed("compilation error".to_string()),
            RemoteError::Unauthorized,
            RemoteError::ConfigError("missing bot token".to_string()),
        ];

        // All errors should implement Display
        for error in &errors {
            let msg = error.to_string();
            assert!(!msg.is_empty());
        }

        // Check specific messages
        assert_eq!(
            RemoteError::NotEnabled.to_string(),
            "Remote gateway is not enabled"
        );
        assert_eq!(
            RemoteError::NoActiveSession.to_string(),
            "No active session for this chat"
        );
        assert!(RemoteError::SessionNotFound("sess-123".to_string())
            .to_string()
            .contains("sess-123"));
    }

    #[test]
    fn test_update_remote_config_request_serialize() {
        let request = UpdateRemoteConfigRequest {
            enabled: Some(true),
            adapter: Some(RemoteAdapterType::Telegram),
            auto_start: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        let parsed: UpdateRemoteConfigRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.enabled, Some(true));
        assert!(parsed.auto_start.is_none());
    }

    #[test]
    fn test_update_telegram_config_request_serialize() {
        let request = UpdateTelegramConfigRequest {
            bot_token: Some("new-token".to_string()),
            allowed_chat_ids: Some(vec![123, 456]),
            allowed_user_ids: None,
            require_password: Some(true),
            access_password: Some("pass123".to_string()),
            max_message_length: Some(3000),
            streaming_mode: Some(StreamingMode::PeriodicUpdate { interval_secs: 5 }),
        };
        let json = serde_json::to_string(&request).unwrap();
        let parsed: UpdateTelegramConfigRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.allowed_chat_ids, Some(vec![123, 456]));
        assert_eq!(parsed.max_message_length, Some(3000));
    }

    #[test]
    fn test_remote_command_equality() {
        assert_eq!(RemoteCommand::Help, RemoteCommand::Help);
        assert_eq!(RemoteCommand::Status, RemoteCommand::Status);
        assert_eq!(RemoteCommand::Cancel, RemoteCommand::Cancel);
        assert_eq!(RemoteCommand::ListSessions, RemoteCommand::ListSessions);
        assert_eq!(RemoteCommand::CloseSession, RemoteCommand::CloseSession);
        assert_ne!(RemoteCommand::Help, RemoteCommand::Status);
    }
}
