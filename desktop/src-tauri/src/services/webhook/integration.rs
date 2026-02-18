//! Webhook Event Hook Integration
//!
//! Shared helper functions for integrating webhook dispatch into
//! existing execution event streams (standalone and claude_code).
//! These are called from the event forwarder tasks in the command layer.

use std::sync::Arc;

use crate::services::streaming::UnifiedStreamEvent;

use super::service::WebhookService;
use super::types::*;

/// Called by event forwarders when a terminal execution event is detected.
///
/// Constructs a `WebhookPayload` from the event details and dispatches it
/// to all matching webhook channels. The dispatch is fire-and-forget
/// (wrapped in `tokio::spawn`) so it does not block the event stream.
///
/// # Arguments
///
/// * `event` - The terminal stream event (Complete or Error)
/// * `session_id` - The session ID that produced this event
/// * `session_name` - Optional human-readable session name
/// * `project_path` - Optional project directory path
/// * `webhook_service` - Reference to the webhook service
/// * `duration_ms` - Optional execution duration in milliseconds
pub fn dispatch_on_event(
    event: &UnifiedStreamEvent,
    session_id: &str,
    session_name: Option<&str>,
    project_path: Option<&str>,
    webhook_service: Arc<WebhookService>,
    duration_ms: Option<u64>,
) {
    dispatch_on_event_inner(
        event,
        session_id,
        session_name,
        project_path,
        webhook_service,
        duration_ms,
        None,
    );
}

/// Called by the remote gateway when a remote command triggers a task completion.
///
/// Same as `dispatch_on_event` but includes `remote_source` metadata
/// (e.g., "via Telegram @username") in the webhook payload.
pub fn dispatch_on_remote_event(
    event: &UnifiedStreamEvent,
    session_id: &str,
    session_name: Option<&str>,
    project_path: Option<&str>,
    webhook_service: Arc<WebhookService>,
    duration_ms: Option<u64>,
    remote_source: &str,
) {
    dispatch_on_event_inner(
        event,
        session_id,
        session_name,
        project_path,
        webhook_service,
        duration_ms,
        Some(remote_source),
    );
}

/// Internal dispatch helper supporting optional remote_source.
fn dispatch_on_event_inner(
    event: &UnifiedStreamEvent,
    session_id: &str,
    session_name: Option<&str>,
    project_path: Option<&str>,
    webhook_service: Arc<WebhookService>,
    duration_ms: Option<u64>,
    remote_source: Option<&str>,
) {
    let payload = match build_payload(
        event,
        session_id,
        session_name,
        project_path,
        duration_ms,
        remote_source,
    ) {
        Some(p) => p,
        None => return, // Not a terminal event
    };

    let session_id_owned = session_id.to_string();

    // Fire-and-forget: dispatch should not block event stream processing
    tokio::spawn(async move {
        let deliveries = webhook_service.dispatch(payload).await;
        if !deliveries.is_empty() {
            tracing::debug!(
                "Webhook dispatched {} deliveries for session {}",
                deliveries.len(),
                session_id_owned
            );
        }
    });
}

/// Build a `WebhookPayload` from an execution event.
///
/// Returns `None` for non-terminal events (only Complete and Error produce payloads).
fn build_payload(
    event: &UnifiedStreamEvent,
    session_id: &str,
    session_name: Option<&str>,
    project_path: Option<&str>,
    duration_ms: Option<u64>,
    remote_source: Option<&str>,
) -> Option<WebhookPayload> {
    let source = remote_source.map(|s| s.to_string());

    match event {
        UnifiedStreamEvent::Complete { .. } => {
            let summary = match &source {
                Some(src) => format!("Task completed successfully ({})", src),
                None => "Task completed successfully".to_string(),
            };
            Some(WebhookPayload {
                event_type: WebhookEventType::TaskComplete,
                session_id: Some(session_id.to_string()),
                session_name: session_name.map(|s| s.to_string()),
                project_path: project_path.map(|s| s.to_string()),
                summary,
                details: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms,
                token_usage: None,
                remote_source: source,
            })
        }
        UnifiedStreamEvent::Error { message, code: _ } => {
            let summary = match &source {
                Some(src) => format!("Task failed ({}): {}", src, message),
                None => format!("Task failed: {}", message),
            };
            Some(WebhookPayload {
                event_type: WebhookEventType::TaskFailed,
                session_id: Some(session_id.to_string()),
                session_name: session_name.map(|s| s.to_string()),
                project_path: project_path.map(|s| s.to_string()),
                summary,
                details: None,
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_ms,
                token_usage: None,
                remote_source: source,
            })
        }
        _ => None,
    }
}

/// Format a remote source string from adapter type and username.
///
/// Returns format: "via <adapter_type> @<username>" or "via <adapter_type>" if no username.
pub fn format_remote_source(adapter_type: &str, username: Option<&str>) -> String {
    match username {
        Some(user) => format!("via {} @{}", adapter_type, user),
        None => format!("via {}", adapter_type),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_payload_complete_event() {
        let event = UnifiedStreamEvent::Complete {
            stop_reason: None,
        };

        let payload = build_payload(
            &event,
            "session-123",
            Some("My Session"),
            Some("/project"),
            Some(5000),
            None,
        );

        assert!(payload.is_some());
        let p = payload.unwrap();
        assert_eq!(p.event_type, WebhookEventType::TaskComplete);
        assert_eq!(p.session_id, Some("session-123".to_string()));
        assert_eq!(p.session_name, Some("My Session".to_string()));
        assert_eq!(p.duration_ms, Some(5000));
        assert!(p.summary.contains("completed successfully"));
        assert!(p.remote_source.is_none());
    }

    #[test]
    fn test_build_payload_error_event() {
        let event = UnifiedStreamEvent::Error {
            message: "Connection timeout".to_string(),
            code: None,
        };

        let payload = build_payload(&event, "session-456", None, None, None, None);

        assert!(payload.is_some());
        let p = payload.unwrap();
        assert_eq!(p.event_type, WebhookEventType::TaskFailed);
        assert!(p.summary.contains("Connection timeout"));
        assert!(p.remote_source.is_none());
    }

    #[test]
    fn test_build_payload_non_terminal_event() {
        let event = UnifiedStreamEvent::TextDelta {
            content: "hello".to_string(),
        };

        let payload = build_payload(&event, "session-789", None, None, None, None);
        assert!(payload.is_none());
    }

    #[test]
    fn test_build_payload_with_remote_source_complete() {
        let event = UnifiedStreamEvent::Complete {
            stop_reason: None,
        };

        let payload = build_payload(
            &event,
            "session-remote-1",
            Some("Remote Session"),
            Some("/projects/app"),
            Some(10000),
            Some("via Telegram @testuser"),
        );

        assert!(payload.is_some());
        let p = payload.unwrap();
        assert_eq!(p.event_type, WebhookEventType::TaskComplete);
        assert_eq!(
            p.remote_source,
            Some("via Telegram @testuser".to_string())
        );
        assert!(p.summary.contains("via Telegram @testuser"));
        assert!(p.summary.contains("completed successfully"));
    }

    #[test]
    fn test_build_payload_with_remote_source_error() {
        let event = UnifiedStreamEvent::Error {
            message: "Out of tokens".to_string(),
            code: None,
        };

        let payload = build_payload(
            &event,
            "session-remote-2",
            None,
            None,
            None,
            Some("via Telegram @admin"),
        );

        assert!(payload.is_some());
        let p = payload.unwrap();
        assert_eq!(p.event_type, WebhookEventType::TaskFailed);
        assert_eq!(
            p.remote_source,
            Some("via Telegram @admin".to_string())
        );
        assert!(p.summary.contains("via Telegram @admin"));
        assert!(p.summary.contains("Out of tokens"));
    }

    #[test]
    fn test_format_remote_source_with_username() {
        let source = format_remote_source("Telegram", Some("testuser"));
        assert_eq!(source, "via Telegram @testuser");
    }

    #[test]
    fn test_format_remote_source_without_username() {
        let source = format_remote_source("Telegram", None);
        assert_eq!(source, "via Telegram");
    }
}
