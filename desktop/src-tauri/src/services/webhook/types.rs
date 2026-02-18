//! Webhook Core Types
//!
//! Core data types for the webhook notification system.
//! All types support serialization via serde for database persistence
//! and Tauri IPC communication.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported notification channel types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WebhookChannelType {
    Slack,
    Feishu,
    Telegram,
    Discord,
    Custom,
}

impl fmt::Display for WebhookChannelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Slack => write!(f, "slack"),
            Self::Feishu => write!(f, "feishu"),
            Self::Telegram => write!(f, "telegram"),
            Self::Discord => write!(f, "discord"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl WebhookChannelType {
    /// Parse from a database string representation.
    pub fn from_str_value(s: &str) -> Option<Self> {
        match s {
            "slack" => Some(Self::Slack),
            "feishu" => Some(Self::Feishu),
            "telegram" => Some(Self::Telegram),
            "discord" => Some(Self::Discord),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

/// Webhook channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookChannelConfig {
    pub id: String,
    pub name: String,
    pub channel_type: WebhookChannelType,
    pub enabled: bool,
    pub url: String,
    /// Token/secret stored in OS Keyring. Excluded from serialization
    /// to prevent accidental exposure in IPC responses or logs.
    #[serde(skip_serializing, default)]
    pub secret: Option<String>,
    pub scope: WebhookScope,
    pub events: Vec<WebhookEventType>,
    pub template: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Notification scope
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WebhookScope {
    /// Triggers for all sessions
    Global,
    /// Only triggers for specific session IDs
    Sessions(Vec<String>),
}

/// Events that can trigger webhooks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WebhookEventType {
    /// Task/session completed successfully
    TaskComplete,
    /// Task/session failed with error
    TaskFailed,
    /// Task cancelled by user
    TaskCancelled,
    /// Story completed (in expert mode)
    StoryComplete,
    /// All stories in a PRD completed
    PrdComplete,
    /// Long-running task progress milestone (25%, 50%, 75%)
    ProgressMilestone,
}

impl fmt::Display for WebhookEventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskComplete => write!(f, "TaskComplete"),
            Self::TaskFailed => write!(f, "TaskFailed"),
            Self::TaskCancelled => write!(f, "TaskCancelled"),
            Self::StoryComplete => write!(f, "StoryComplete"),
            Self::PrdComplete => write!(f, "PrdComplete"),
            Self::ProgressMilestone => write!(f, "ProgressMilestone"),
        }
    }
}

/// Token usage summary (embedded in payload)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsageSummary {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// Webhook delivery payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event_type: WebhookEventType,
    pub session_id: Option<String>,
    pub session_name: Option<String>,
    pub project_path: Option<String>,
    pub summary: String,
    pub details: Option<serde_json::Value>,
    pub timestamp: String,
    pub duration_ms: Option<u64>,
    pub token_usage: Option<TokenUsageSummary>,
    /// Source identifier when the event was triggered by a remote session.
    /// Format: "via <adapter_type> @<username>" (e.g., "via Telegram @user").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_source: Option<String>,
}

impl Default for WebhookPayload {
    fn default() -> Self {
        Self {
            event_type: WebhookEventType::TaskComplete,
            session_id: None,
            session_name: None,
            project_path: None,
            summary: String::new(),
            details: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_ms: None,
            token_usage: None,
            remote_source: None,
        }
    }
}

/// Delivery record for audit/retry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: String,
    pub channel_id: String,
    pub event_type: WebhookEventType,
    pub payload: WebhookPayload,
    pub status: DeliveryStatus,
    pub status_code: Option<u16>,
    pub response_body: Option<String>,
    pub attempts: u32,
    pub last_attempt_at: String,
    pub created_at: String,
}

impl WebhookDelivery {
    /// Create a new pending delivery record.
    pub fn new(config: &WebhookChannelConfig, payload: &WebhookPayload) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            channel_id: config.id.clone(),
            event_type: payload.event_type.clone(),
            payload: payload.clone(),
            status: DeliveryStatus::Pending,
            status_code: None,
            response_body: None,
            attempts: 0,
            last_attempt_at: now.clone(),
            created_at: now,
        }
    }
}

/// Delivery status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeliveryStatus {
    Pending,
    Success,
    Failed,
    Retrying,
}

impl fmt::Display for DeliveryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Success => write!(f, "success"),
            Self::Failed => write!(f, "failed"),
            Self::Retrying => write!(f, "retrying"),
        }
    }
}

impl DeliveryStatus {
    /// Parse from a database string.
    pub fn from_str_value(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "success" => Self::Success,
            "failed" => Self::Failed,
            "retrying" => Self::Retrying,
            _ => Self::Failed,
        }
    }
}

/// Result of testing a webhook channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookTestResult {
    pub success: bool,
    pub latency_ms: Option<u32>,
    pub error: Option<String>,
}

/// Webhook-specific errors
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    #[error("Channel not found: {0}")]
    ChannelNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Keyring error: {0}")]
    KeyringError(String),
}

impl From<reqwest::Error> for WebhookError {
    fn from(err: reqwest::Error) -> Self {
        Self::HttpError(err.to_string())
    }
}

impl From<serde_json::Error> for WebhookError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerializationError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_channel_type_serialization_roundtrip() {
        let types = vec![
            WebhookChannelType::Slack,
            WebhookChannelType::Feishu,
            WebhookChannelType::Telegram,
            WebhookChannelType::Discord,
            WebhookChannelType::Custom,
        ];
        for ct in types {
            let json = serde_json::to_string(&ct).unwrap();
            let parsed: WebhookChannelType = serde_json::from_str(&json).unwrap();
            assert_eq!(ct, parsed);
        }
    }

    #[test]
    fn test_webhook_channel_type_display() {
        assert_eq!(WebhookChannelType::Slack.to_string(), "slack");
        assert_eq!(WebhookChannelType::Feishu.to_string(), "feishu");
        assert_eq!(WebhookChannelType::Telegram.to_string(), "telegram");
        assert_eq!(WebhookChannelType::Discord.to_string(), "discord");
        assert_eq!(WebhookChannelType::Custom.to_string(), "custom");
    }

    #[test]
    fn test_webhook_channel_type_from_str() {
        assert_eq!(
            WebhookChannelType::from_str_value("slack"),
            Some(WebhookChannelType::Slack)
        );
        assert_eq!(WebhookChannelType::from_str_value("unknown"), None);
    }

    #[test]
    fn test_webhook_scope_serialization_roundtrip() {
        let global = WebhookScope::Global;
        let json = serde_json::to_string(&global).unwrap();
        let parsed: WebhookScope = serde_json::from_str(&json).unwrap();
        assert_eq!(global, parsed);

        let sessions = WebhookScope::Sessions(vec!["s1".to_string(), "s2".to_string()]);
        let json = serde_json::to_string(&sessions).unwrap();
        let parsed: WebhookScope = serde_json::from_str(&json).unwrap();
        assert_eq!(sessions, parsed);
    }

    #[test]
    fn test_webhook_event_type_serialization_roundtrip() {
        let events = vec![
            WebhookEventType::TaskComplete,
            WebhookEventType::TaskFailed,
            WebhookEventType::TaskCancelled,
            WebhookEventType::StoryComplete,
            WebhookEventType::PrdComplete,
            WebhookEventType::ProgressMilestone,
        ];
        for evt in events {
            let json = serde_json::to_string(&evt).unwrap();
            let parsed: WebhookEventType = serde_json::from_str(&json).unwrap();
            assert_eq!(evt, parsed);
        }
    }

    #[test]
    fn test_webhook_payload_serialization() {
        let payload = WebhookPayload {
            event_type: WebhookEventType::TaskComplete,
            session_id: Some("session-123".to_string()),
            session_name: Some("My Session".to_string()),
            project_path: Some("/home/user/project".to_string()),
            summary: "Task completed successfully".to_string(),
            details: Some(serde_json::json!({"key": "value"})),
            timestamp: "2026-02-18T12:00:00Z".to_string(),
            duration_ms: Some(5000),
            token_usage: Some(TokenUsageSummary {
                input_tokens: Some(100),
                output_tokens: Some(200),
            }),
            remote_source: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: WebhookPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.summary, "Task completed successfully");
        assert_eq!(parsed.duration_ms, Some(5000));
    }

    #[test]
    fn test_webhook_channel_config_secret_skip_serializing() {
        let config = WebhookChannelConfig {
            id: "ch-001".to_string(),
            name: "Test Channel".to_string(),
            channel_type: WebhookChannelType::Slack,
            enabled: true,
            url: "https://hooks.slack.com/services/test".to_string(),
            secret: Some("super-secret-token".to_string()),
            scope: WebhookScope::Global,
            events: vec![WebhookEventType::TaskComplete],
            template: None,
            created_at: "2026-02-18T12:00:00Z".to_string(),
            updated_at: "2026-02-18T12:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        // Secret should NOT appear in serialized output
        assert!(!json.contains("super-secret-token"));
        assert!(json.contains("Test Channel"));
    }

    #[test]
    fn test_delivery_status_serialization_roundtrip() {
        let statuses = vec![
            DeliveryStatus::Pending,
            DeliveryStatus::Success,
            DeliveryStatus::Failed,
            DeliveryStatus::Retrying,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let parsed: DeliveryStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, parsed);
        }
    }

    #[test]
    fn test_delivery_status_display_and_parse() {
        assert_eq!(DeliveryStatus::Pending.to_string(), "pending");
        assert_eq!(DeliveryStatus::Success.to_string(), "success");
        assert_eq!(DeliveryStatus::Failed.to_string(), "failed");
        assert_eq!(DeliveryStatus::Retrying.to_string(), "retrying");

        assert_eq!(DeliveryStatus::from_str_value("pending"), DeliveryStatus::Pending);
        assert_eq!(DeliveryStatus::from_str_value("success"), DeliveryStatus::Success);
        assert_eq!(DeliveryStatus::from_str_value("unknown"), DeliveryStatus::Failed);
    }

    #[test]
    fn test_webhook_delivery_new() {
        let config = WebhookChannelConfig {
            id: "ch-001".to_string(),
            name: "Test".to_string(),
            channel_type: WebhookChannelType::Slack,
            enabled: true,
            url: "https://hooks.slack.com/test".to_string(),
            secret: None,
            scope: WebhookScope::Global,
            events: vec![WebhookEventType::TaskComplete],
            template: None,
            created_at: "2026-02-18T12:00:00Z".to_string(),
            updated_at: "2026-02-18T12:00:00Z".to_string(),
        };
        let payload = WebhookPayload::default();
        let delivery = WebhookDelivery::new(&config, &payload);

        assert_eq!(delivery.channel_id, "ch-001");
        assert_eq!(delivery.status, DeliveryStatus::Pending);
        assert_eq!(delivery.attempts, 0);
        assert!(!delivery.id.is_empty());
    }

    #[test]
    fn test_webhook_test_result_serialization() {
        let result = WebhookTestResult {
            success: true,
            latency_ms: Some(150),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: WebhookTestResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
        assert_eq!(parsed.latency_ms, Some(150));
    }

    #[test]
    fn test_webhook_payload_default() {
        let payload = WebhookPayload::default();
        assert_eq!(payload.event_type, WebhookEventType::TaskComplete);
        assert!(payload.summary.is_empty());
        assert!(payload.session_id.is_none());
        assert!(!payload.timestamp.is_empty());
        assert!(payload.remote_source.is_none());
    }

    #[test]
    fn test_webhook_payload_with_remote_source() {
        let payload = WebhookPayload {
            event_type: WebhookEventType::TaskComplete,
            session_id: Some("remote-session-1".to_string()),
            summary: "Task completed successfully".to_string(),
            remote_source: Some("via Telegram @user".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("remote_source"));
        assert!(json.contains("via Telegram @user"));

        let parsed: WebhookPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed.remote_source,
            Some("via Telegram @user".to_string())
        );
    }

    #[test]
    fn test_webhook_payload_remote_source_skipped_when_none() {
        let payload = WebhookPayload::default();
        let json = serde_json::to_string(&payload).unwrap();
        // remote_source should be omitted when None
        assert!(!json.contains("remote_source"));
    }
}
