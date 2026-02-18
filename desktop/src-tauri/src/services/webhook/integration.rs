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
    let payload = match build_payload(event, session_id, session_name, project_path, duration_ms) {
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
) -> Option<WebhookPayload> {
    match event {
        UnifiedStreamEvent::Complete { .. } => Some(WebhookPayload {
            event_type: WebhookEventType::TaskComplete,
            session_id: Some(session_id.to_string()),
            session_name: session_name.map(|s| s.to_string()),
            project_path: project_path.map(|s| s.to_string()),
            summary: "Task completed successfully".to_string(),
            details: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_ms,
            token_usage: None,
        }),
        UnifiedStreamEvent::Error { message, code: _ } => Some(WebhookPayload {
            event_type: WebhookEventType::TaskFailed,
            session_id: Some(session_id.to_string()),
            session_name: session_name.map(|s| s.to_string()),
            project_path: project_path.map(|s| s.to_string()),
            summary: format!("Task failed: {}", message),
            details: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_ms,
            token_usage: None,
        }),
        _ => None,
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
        );

        assert!(payload.is_some());
        let p = payload.unwrap();
        assert_eq!(p.event_type, WebhookEventType::TaskComplete);
        assert_eq!(p.session_id, Some("session-123".to_string()));
        assert_eq!(p.session_name, Some("My Session".to_string()));
        assert_eq!(p.duration_ms, Some(5000));
        assert!(p.summary.contains("completed successfully"));
    }

    #[test]
    fn test_build_payload_error_event() {
        let event = UnifiedStreamEvent::Error {
            message: "Connection timeout".to_string(),
            code: None,
        };

        let payload = build_payload(&event, "session-456", None, None, None);

        assert!(payload.is_some());
        let p = payload.unwrap();
        assert_eq!(p.event_type, WebhookEventType::TaskFailed);
        assert!(p.summary.contains("Connection timeout"));
    }

    #[test]
    fn test_build_payload_non_terminal_event() {
        let event = UnifiedStreamEvent::TextDelta {
            content: "hello".to_string(),
        };

        let payload = build_payload(&event, "session-789", None, None, None);
        assert!(payload.is_none());
    }
}
