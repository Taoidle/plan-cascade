//! Webhook Event Hook Integration
//!
//! Shared helper functions for integrating webhook dispatch into
//! existing execution event streams (standalone and claude_code).
//! These are called from the event forwarder tasks in the command layer.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::services::streaming::UnifiedStreamEvent;

use super::service::WebhookService;
use super::types::*;

const PROGRESS_MILESTONES: [u8; 3] = [25, 50, 75];
const TERMINAL_REGISTRY_TTL_SECS: u64 = 600;
const FAILED_STOP_REASON_HINTS: [&str; 4] =
    ["fail", "error", "max_iterations", "analysis_gate_failed"];

static SENT_PROGRESS_MILESTONES: OnceLock<Mutex<HashMap<String, HashSet<u8>>>> = OnceLock::new();
static SENT_TERMINAL_EVENTS: OnceLock<Mutex<HashMap<String, TerminalEventRecord>>> =
    OnceLock::new();

fn progress_registry() -> &'static Mutex<HashMap<String, HashSet<u8>>> {
    SENT_PROGRESS_MILESTONES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn terminal_registry() -> &'static Mutex<HashMap<String, TerminalEventRecord>> {
    SENT_TERMINAL_EVENTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalEventKind {
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
struct TerminalEventRecord {
    kind: TerminalEventKind,
    created_at: Instant,
}

/// Called by event forwarders when an execution event is detected.
///
/// Dispatch is fire-and-forget (`tokio::spawn`) and never blocks the primary
/// command flow.
pub fn dispatch_on_event(
    event: &UnifiedStreamEvent,
    session_id: &str,
    execution_id: Option<&str>,
    session_name: Option<&str>,
    project_path: Option<&str>,
    webhook_service: Arc<WebhookService>,
    duration_ms: Option<u64>,
) {
    dispatch_on_event_inner(
        event,
        session_id,
        execution_id,
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
    execution_id: Option<&str>,
    session_name: Option<&str>,
    project_path: Option<&str>,
    webhook_service: Arc<WebhookService>,
    duration_ms: Option<u64>,
    remote_source: &str,
) {
    dispatch_on_event_inner(
        event,
        session_id,
        execution_id,
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
    execution_id: Option<&str>,
    session_name: Option<&str>,
    project_path: Option<&str>,
    webhook_service: Arc<WebhookService>,
    duration_ms: Option<u64>,
    remote_source: Option<&str>,
) {
    purge_terminal_registry();
    let execution_key = execution_registry_key(session_id, execution_id);
    let payloads = build_payloads(
        event,
        session_id,
        &execution_key,
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
    execution_key: &str,
    session_name: Option<&str>,
    project_path: Option<&str>,
    duration_ms: Option<u64>,
    remote_source: Option<&str>,
) -> Vec<WebhookPayload> {
    let source = remote_source.map(|s| s.to_string());
    match event {
        UnifiedStreamEvent::Complete { stop_reason } => {
            let completion_kind = classify_completion(stop_reason.as_deref());
            if should_skip_terminal_on_complete(execution_key, completion_kind) {
                clear_terminal_event(execution_key);
                return Vec::new();
            }

            let (event_type, summary) = match completion_kind {
                CompletionKind::Cancelled => (
                    WebhookEventType::TaskCancelled,
                    match &source {
                        Some(src) => format!("Task cancelled ({})", src),
                        None => "Task cancelled".to_string(),
                    },
                ),
                CompletionKind::Failed => (
                    WebhookEventType::TaskFailed,
                    match (&source, stop_reason.as_deref()) {
                        (Some(src), Some(reason)) if !reason.trim().is_empty() => {
                            format!("Task failed ({}): {}", src, reason)
                        }
                        (Some(src), _) => format!("Task failed ({})", src),
                        (None, Some(reason)) if !reason.trim().is_empty() => {
                            format!("Task failed: {}", reason)
                        }
                        (None, _) => "Task failed".to_string(),
                    },
                ),
                CompletionKind::Success => (
                    WebhookEventType::TaskComplete,
                    match &source {
                        Some(src) => format!("Task completed successfully ({})", src),
                        None => "Task completed successfully".to_string(),
                    },
                ),
            };

            clear_terminal_event(execution_key);
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
        UnifiedStreamEvent::Error { message, code } => {
            let terminal_kind = classify_error(code.as_deref(), message);
            if has_sent_terminal_event(execution_key, terminal_kind) {
                return Vec::new();
            }
            remember_terminal_event(execution_key, terminal_kind);

            let (event_type, summary) = match terminal_kind {
                TerminalEventKind::Cancelled => (
                    WebhookEventType::TaskCancelled,
                    match &source {
                        Some(src) => format!("Task cancelled ({})", src),
                        None => "Task cancelled".to_string(),
                    },
                ),
                TerminalEventKind::Failed => (
                    WebhookEventType::TaskFailed,
                    match &source {
                        Some(src) => format!("Task failed ({}): {}", src, message),
                        None => format!("Task failed: {}", message),
                    },
                ),
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
            clear_terminal_event(execution_key);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionKind {
    Success,
    Failed,
    Cancelled,
}

fn classify_completion(stop_reason: Option<&str>) -> CompletionKind {
    let Some(reason) = stop_reason else {
        return CompletionKind::Success;
    };
    let normalized = reason.trim().to_ascii_lowercase();
    if normalized == "cancelled" || normalized == "canceled" {
        return CompletionKind::Cancelled;
    }
    if FAILED_STOP_REASON_HINTS
        .iter()
        .any(|hint| normalized.contains(hint))
    {
        return CompletionKind::Failed;
    }
    CompletionKind::Success
}

fn classify_error(code: Option<&str>, message: &str) -> TerminalEventKind {
    let code_norm = code.unwrap_or_default().trim().to_ascii_lowercase();
    if code_norm == "cancelled" || code_norm == "canceled" {
        return TerminalEventKind::Cancelled;
    }
    let message_norm = message.trim().to_ascii_lowercase();
    if message_norm.contains("cancelled") || message_norm.contains("canceled") {
        return TerminalEventKind::Cancelled;
    }
    TerminalEventKind::Failed
}

fn execution_registry_key(session_id: &str, execution_id: Option<&str>) -> String {
    let eid = execution_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    if eid.is_empty() {
        session_id.to_string()
    } else {
        format!("{}:{}", session_id, eid)
    }
}

fn has_sent_terminal_event(execution_key: &str, expected: TerminalEventKind) -> bool {
    let Ok(registry) = terminal_registry().lock() else {
        return false;
    };
    registry
        .get(execution_key)
        .map(|entry| entry.kind == expected)
        .unwrap_or(false)
}

fn should_skip_terminal_on_complete(execution_key: &str, completion_kind: CompletionKind) -> bool {
    let Ok(registry) = terminal_registry().lock() else {
        return false;
    };
    let Some(entry) = registry.get(execution_key) else {
        return false;
    };

    match (entry.kind, completion_kind) {
        // Error first then Complete(success) must not emit TaskComplete.
        (TerminalEventKind::Failed, CompletionKind::Success) => true,
        // De-duplicate terminal type already emitted from Error stream.
        (TerminalEventKind::Failed, CompletionKind::Failed) => true,
        (TerminalEventKind::Cancelled, CompletionKind::Cancelled) => true,
        // If cancellation was already emitted from Error code/message,
        // suppress later Complete(success) as noise.
        (TerminalEventKind::Cancelled, CompletionKind::Success) => true,
        _ => false,
    }
}

fn remember_terminal_event(execution_key: &str, kind: TerminalEventKind) {
    let Ok(mut registry) = terminal_registry().lock() else {
        return;
    };
    registry.insert(
        execution_key.to_string(),
        TerminalEventRecord {
            kind,
            created_at: Instant::now(),
        },
    );
}

fn clear_terminal_event(execution_key: &str) {
    if let Ok(mut registry) = terminal_registry().lock() {
        registry.remove(execution_key);
    }
}

fn purge_terminal_registry() {
    let Ok(mut registry) = terminal_registry().lock() else {
        return;
    };
    let ttl = Duration::from_secs(TERMINAL_REGISTRY_TTL_SECS);
    registry.retain(|_, entry| entry.created_at.elapsed() < ttl);
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

    fn reset_terminal(session_id: &str) {
        clear_terminal_event(session_id);
    }

    #[test]
    fn test_complete_cancelled_maps_to_task_cancelled() {
        reset_terminal("s1");
        let event = UnifiedStreamEvent::Complete {
            stop_reason: Some("cancelled".to_string()),
        };
        let payloads = build_payloads(&event, "s1", "s1", None, None, None, None);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::TaskCancelled);
    }

    #[test]
    fn test_complete_maps_to_task_complete() {
        reset_terminal("s1");
        let event = UnifiedStreamEvent::Complete { stop_reason: None };
        let payloads = build_payloads(&event, "s1", "s1", None, None, None, None);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::TaskComplete);
    }

    #[test]
    fn test_error_maps_to_task_failed() {
        reset_terminal("s1");
        let event = UnifiedStreamEvent::Error {
            message: "boom".to_string(),
            code: None,
        };
        let payloads = build_payloads(&event, "s1", "s1", None, None, None, None);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::TaskFailed);
    }

    #[test]
    fn test_story_complete_success_maps_to_story_complete() {
        reset_terminal("s1");
        let event = UnifiedStreamEvent::StoryComplete {
            session_id: "s1".to_string(),
            story_id: "story-1".to_string(),
            success: true,
            error: None,
        };
        let payloads = build_payloads(&event, "s1", "s1", None, None, None, None);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::StoryComplete);
    }

    #[test]
    fn test_prd_complete_maps_from_session_complete_success() {
        reset_terminal("s1");
        let event = UnifiedStreamEvent::SessionComplete {
            session_id: "s1".to_string(),
            success: true,
            completed_stories: 3,
            total_stories: 3,
        };
        let payloads = build_payloads(&event, "s1", "s1", None, None, None, None);
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
        let payloads = build_payloads(
            &event_30,
            "session-progress",
            "session-progress",
            None,
            None,
            None,
            None,
        );
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::ProgressMilestone);
        assert!(payloads[0].summary.contains("25%"));

        let payloads_again = build_payloads(
            &event_30,
            "session-progress",
            "session-progress",
            None,
            None,
            None,
            None,
        );
        assert!(payloads_again.is_empty());

        let event_55 = UnifiedStreamEvent::SessionProgress {
            session_id: "session-progress".to_string(),
            progress: serde_json::json!({ "percentage": 55.0 }),
        };
        let payloads_55 = build_payloads(
            &event_55,
            "session-progress",
            "session-progress",
            None,
            None,
            None,
            None,
        );
        assert_eq!(payloads_55.len(), 1);
        assert!(payloads_55[0].summary.contains("50%"));

        let event_80 = UnifiedStreamEvent::SessionProgress {
            session_id: "session-progress".to_string(),
            progress: serde_json::json!({ "percentage": 80.0 }),
        };
        let payloads_80 = build_payloads(
            &event_80,
            "session-progress",
            "session-progress",
            None,
            None,
            None,
            None,
        );
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

    #[test]
    fn test_error_then_complete_success_emits_only_failed() {
        reset_terminal("s-fail");
        let err = UnifiedStreamEvent::Error {
            message: "boom".to_string(),
            code: Some("internal_error".to_string()),
        };
        let err_payloads = build_payloads(&err, "s-fail", "s-fail", None, None, None, None);
        assert_eq!(err_payloads.len(), 1);
        assert_eq!(err_payloads[0].event_type, WebhookEventType::TaskFailed);

        let complete = UnifiedStreamEvent::Complete { stop_reason: None };
        let complete_payloads =
            build_payloads(&complete, "s-fail", "s-fail", None, None, None, None);
        assert!(complete_payloads.is_empty());
    }

    #[test]
    fn test_cancelled_error_then_complete_cancelled_dedupes() {
        reset_terminal("s-cancel");
        let err = UnifiedStreamEvent::Error {
            message: "Execution cancelled".to_string(),
            code: Some("cancelled".to_string()),
        };
        let err_payloads = build_payloads(&err, "s-cancel", "s-cancel", None, None, None, None);
        assert_eq!(err_payloads.len(), 1);
        assert_eq!(err_payloads[0].event_type, WebhookEventType::TaskCancelled);

        let complete = UnifiedStreamEvent::Complete {
            stop_reason: Some("cancelled".to_string()),
        };
        let complete_payloads =
            build_payloads(&complete, "s-cancel", "s-cancel", None, None, None, None);
        assert!(complete_payloads.is_empty());
    }

    #[test]
    fn test_complete_failed_stop_reason_maps_to_task_failed() {
        reset_terminal("s-stop-fail");
        let complete = UnifiedStreamEvent::Complete {
            stop_reason: Some("analysis_gate_failed".to_string()),
        };
        let payloads = build_payloads(
            &complete,
            "s-stop-fail",
            "s-stop-fail",
            None,
            None,
            None,
            None,
        );
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].event_type, WebhookEventType::TaskFailed);
    }
}
