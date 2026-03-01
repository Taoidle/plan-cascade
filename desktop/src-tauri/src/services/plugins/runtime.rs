//! Plugin runtime event log for hook observability.
//!
//! Keeps an in-memory ring buffer (500 events) to support UI diagnostics.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::services::plugins::models::PluginRuntimeEvent;

const MAX_EVENTS: usize = 500;

static EVENT_SEQ: AtomicU64 = AtomicU64::new(1);
static EVENTS: OnceLock<Mutex<Vec<PluginRuntimeEvent>>> = OnceLock::new();

fn events() -> &'static Mutex<Vec<PluginRuntimeEvent>> {
    EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn next_id() -> String {
    let seq = EVENT_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("plugin-runtime-{}-{}", chrono::Utc::now().timestamp_millis(), seq)
}

fn push_event(event: PluginRuntimeEvent) {
    if let Ok(mut guard) = events().lock() {
        guard.push(event);
        if guard.len() > MAX_EVENTS {
            let drop_n = guard.len() - MAX_EVENTS;
            guard.drain(0..drop_n);
        }
    }
}

/// Record hook start and return the generated event id.
pub fn record_hook_start(plugin_name: &str, hook_event: &str, session_id: Option<&str>) -> String {
    let event_id = next_id();
    push_event(PluginRuntimeEvent {
        event_id: event_id.clone(),
        plugin_name: plugin_name.to_string(),
        hook_event: hook_event.to_string(),
        phase: "start".to_string(),
        session_id: session_id.map(|s| s.to_string()),
        success: None,
        duration_ms: None,
        exit_code: None,
        stderr_snippet: None,
        created_at: chrono::Utc::now().timestamp(),
    });
    event_id
}

/// Record hook completion.
pub fn record_hook_finish(
    event_id: &str,
    plugin_name: &str,
    hook_event: &str,
    session_id: Option<&str>,
    success: bool,
    duration_ms: u64,
    exit_code: i32,
    stderr_snippet: Option<String>,
) {
    push_event(PluginRuntimeEvent {
        event_id: event_id.to_string(),
        plugin_name: plugin_name.to_string(),
        hook_event: hook_event.to_string(),
        phase: "finish".to_string(),
        session_id: session_id.map(|s| s.to_string()),
        success: Some(success),
        duration_ms: Some(duration_ms),
        exit_code: Some(exit_code),
        stderr_snippet,
        created_at: chrono::Utc::now().timestamp(),
    });
}

/// List runtime events with optional filtering.
pub fn list_runtime_events(
    plugin_name: Option<&str>,
    session_id: Option<&str>,
    limit: usize,
) -> Vec<PluginRuntimeEvent> {
    let mut out = if let Ok(guard) = events().lock() {
        guard.clone()
    } else {
        Vec::new()
    };

    if let Some(name) = plugin_name {
        out.retain(|e| e.plugin_name == name);
    }
    if let Some(session) = session_id {
        out.retain(|e| e.session_id.as_deref() == Some(session));
    }

    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out.into_iter().take(limit).collect()
}

