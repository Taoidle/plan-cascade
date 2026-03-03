//! Webhook Event Hook Integration
//!
//! Shared helper functions for integrating webhook dispatch into
//! existing execution event streams (standalone and claude_code).
//! These are called from the event forwarder tasks in the command layer.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use crate::services::streaming::UnifiedStreamEvent;

use super::service::WebhookService;
use super::types::*;

const PROGRESS_MILESTONES: [u8; 3] = [25, 50, 75];

static SENT_PROGRESS_MILESTONES: OnceLock<Mutex<HashMap<String, HashSet<u8>>>> = OnceLock::new();

fn progress_registry() -> &'static Mutex<HashMap<String, HashSet<u8>>> {
    SENT_PROGRESS_MILESTONES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Called by event forwarders when an execution event is detected.
///
/// Dispatch is fire-and-forget (`tokio::spawn`) and never blocks the primary
/// command flow.
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

/// Called by remote flows with `remote_source` metadata.
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

fn dispatch_on_event_inner(
    event: &UnifiedStreamEvent,
    session_id: &str,
    session_name: Option<&str>,
    project_path: Option<&str>,
    webhook_service: Arc<WebhookService>,
    duration_ms: Option<u64>,
    remote_source: Option<&str>,
) {
    let payloads = build_payloads(
        event,
        session_id,
        session_name,
        project_path,
        duration_ms,
        remote_source,
    );
    if payloads.is_empty() {
        return;
    }

    for payload in payloads {
        let svc = webhook_service.clone();
        let sid = session_id.to_string();
        tokio::spawn(async move {
            let deliveries = svc.dispatch(payload).await;
            if !deliveries.is_empty() {
                tracing::debug!(
                    session_id = %sid,
                    deliveries = deliveries.len(),
                    "webhook dispatched from event integration"
                );
            }
        });
    }
}

fn build_payloads(
    event: &UnifiedStreamEvent,
    session_id: &str,
    session_name: Option<&str>,
    project_path: Option<&str>,
    duration_ms: Option<u64>,
    remote_source: Option<&str>,
) -> Vec<WebhookPayload> {
    let source = remote_source.map(|s| s.to_string());
    match event {
        UnifiedStreamEvent::Complete { stop_reason } => {
            let cancelled = stop_reason
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("cancelled") || value.eq_ignore_ascii_case("canceled"))
                .unwrap_or(false);
            let event_type = if cancelled {
                WebhookEventType::TaskCancelled
            } else {
                WebhookEventType::TaskComplete
            };
            let summary = match (&source, cancelled) {
                (Some(src), true) => format!("Task cancelled ({})", src),
                (Some(src), false) => format!("Task completed successfully ({})", src),
                (None, true) => "Task cancelled".to_string(),
                (None, false) => "Task completed successfully".to_string(),
            };

            vec![base_payload(
                event_type,
                session_id,
                session_name,
                project_path,
                duration_ms,
                source,
                summary,
                None,
            )]
        }
        UnifiedStreamEvent::Error { message, code: _ } => {
            let summary = match &source {
                Some(src) => format!("Task failed ({}): {}", src, message),
                None => format!("Task failed: {}", message),
            };
            vec![base_payload(
                WebhookEventType::TaskFailed,
                session_id,
                session_name,
                project_path,
                duration_ms,
                source,
                summary,
                None,
            )]
        }
        UnifiedStreamEvent::StoryComplete {
            story_id,
            success,
            error,
            ..
        } => {
            if !success {
                return Vec::new();
            }
            let summary = match &source {
                Some(src) => format!("Story {} completed ({})", story_id, src),
                None => format!("Story {} completed", story_id),
            };
            vec![base_payload(
                WebhookEventType::StoryComplete,
                session_id,
                session_name,
                project_path,
                duration_ms,
                source,
                summary,
                Some(serde_json::json!({
                    "story_id": story_id,
                    "success": success,
                    "error": error
                })),
            )]
        }
        UnifiedStreamEvent::SessionComplete {
            success,
            completed_stories,
            total_stories,
            ..
        } => {
            clear_progress_milestones(session_id);
            if !success {
                return Vec::new();
            }

            let summary = match &source {
                Some(src) => format!(
                    "PRD execution completed ({}) [{}/{} stories]",
                    src, completed_stories, total_stories
                ),
                None => format!(
                    "PRD execution completed [{}/{} stories]",
                    completed_stories, total_stories
                ),
            };
            vec![base_payload(
                WebhookEventType::PrdComplete,
                session_id,
                session_name,
                project_path,
                duration_ms,
                source,
                summary,
                Some(serde_json::json!({
                    "completed_stories": completed_stories,
                    "total_stories": total_stories
                })),
            )]
        }
        UnifiedStreamEvent::SessionProgress { progress, .. } => {
            let percentage = match extract_progress_percentage(progress) {
                Some(value) => value,
                None => return Vec::new(),
            };
            let milestone = match pick_new_milestone(session_id, percentage) {
                Some(value) => value,
                None => return Vec::new(),
            };
            let summary = match &source {
                Some(src) => format!("Progress milestone reached: {}% ({})", milestone, src),
                None => format!("Progress milestone reached: {}%", milestone),
            };
            vec![base_payload(
                WebhookEventType::ProgressMilestone,
                session_id,
                session_name,
                project_path,
                duration_ms,
                source,
                summary,
                Some(serde_json::json!({
                    "percentage": percentage,
                    "milestone": milestone
                })),
            )]
        }
        _ => Vec::new(),
    }
}

fn base_payload(
    event_type: WebhookEventType,
    session_id: &str,
    session_name: Option<&str>,
    project_path: Option<&str>,
    duration_ms: Option<u64>,
    remote_source: Option<String>,
    summary: String,
    details: Option<serde_json::Value>,
) -> WebhookPayload {
    WebhookPayload {
        event_type,
        session_id: Some(session_id.to_string()),
        session_name: session_name.map(|s| s.to_string()),
        project_path: project_path.map(|s| s.to_string()),
        summary,
        details,
        timestamp: chrono::Utc::now().to_rfc3339(),
        duration_ms,
        token_usage: None,
        remote_source,
    }
}

fn extract_progress_percentage(progress: &serde_json::Value) -> Option<u8> {
    let mut value = progress.get("percentage").and_then(|v| v.as_f64());
    if value.is_none() {
        value = progress.get("progress").and_then(|v| v.as_f64());
    }
    let mut percentage = value?;
    if (0.0..=1.0).contains(&percentage) {
        percentage *= 100.0;
    }
    let rounded = percentage.round();
    if !(0.0..=100.0).contains(&rounded) {
        return None;
    }
    Some(rounded as u8)
}

fn pick_new_milestone(session_id: &str, percentage: u8) -> Option<u8> {
    let mut selected = None;
    let mut registry = progress_registry().lock().ok()?;
    let sent = registry.entry(session_id.to_string()).or_default();
    for milestone in PROGRESS_MILESTONES {
        if percentage >= milestone && !sent.contains(&milestone) {
            sent.insert(milestone);
            selected = Some(milestone);
            break;
        }
    }
    selected
}

fn clear_progress_milestones(session_id: &str) {
    if let Ok(mut registry) = progress_registry().lock() {
        registry.remove(session_id);
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

    fn reset_progress(session_id: &str) {
        clear_progress_milestones(session_id);
    }

    #[test]
    fn test_complete_cancelled_maps_to_task_cancelled() {
        let event = UnifiedStreamEvent::Complete {
            stop_reason: Some("cancelled".to_string()),
        };
        let payloads = build_payloads(&event, "s1", None, None, None, None);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::TaskCancelled);
    }

    #[test]
    fn test_complete_maps_to_task_complete() {
        let event = UnifiedStreamEvent::Complete { stop_reason: None };
        let payloads = build_payloads(&event, "s1", None, None, None, None);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::TaskComplete);
    }

    #[test]
    fn test_error_maps_to_task_failed() {
        let event = UnifiedStreamEvent::Error {
            message: "boom".to_string(),
            code: None,
        };
        let payloads = build_payloads(&event, "s1", None, None, None, None);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::TaskFailed);
    }

    #[test]
    fn test_story_complete_success_maps_to_story_complete() {
        let event = UnifiedStreamEvent::StoryComplete {
            session_id: "s1".to_string(),
            story_id: "story-1".to_string(),
            success: true,
            error: None,
        };
        let payloads = build_payloads(&event, "s1", None, None, None, None);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::StoryComplete);
    }

    #[test]
    fn test_prd_complete_maps_from_session_complete_success() {
        let event = UnifiedStreamEvent::SessionComplete {
            session_id: "s1".to_string(),
            success: true,
            completed_stories: 3,
            total_stories: 3,
        };
        let payloads = build_payloads(&event, "s1", None, None, None, None);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::PrdComplete);
    }

    #[test]
    fn test_progress_milestone_fires_once_per_threshold() {
        reset_progress("session-progress");

        let event_30 = UnifiedStreamEvent::SessionProgress {
            session_id: "session-progress".to_string(),
            progress: serde_json::json!({ "percentage": 30.0 }),
        };
        let payloads = build_payloads(&event_30, "session-progress", None, None, None, None);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::ProgressMilestone);
        assert!(payloads[0].summary.contains("25%"));

        let payloads_again = build_payloads(&event_30, "session-progress", None, None, None, None);
        assert!(payloads_again.is_empty());

        let event_55 = UnifiedStreamEvent::SessionProgress {
            session_id: "session-progress".to_string(),
            progress: serde_json::json!({ "percentage": 55.0 }),
        };
        let payloads_55 = build_payloads(&event_55, "session-progress", None, None, None, None);
        assert_eq!(payloads_55.len(), 1);
        assert!(payloads_55[0].summary.contains("50%"));

        let event_80 = UnifiedStreamEvent::SessionProgress {
            session_id: "session-progress".to_string(),
            progress: serde_json::json!({ "percentage": 80.0 }),
        };
        let payloads_80 = build_payloads(&event_80, "session-progress", None, None, None, None);
        assert_eq!(payloads_80.len(), 1);
        assert!(payloads_80[0].summary.contains("75%"));

        reset_progress("session-progress");
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
