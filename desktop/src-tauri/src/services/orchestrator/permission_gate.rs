//! Permission Gate Service
//!
//! Central gate that intercepts tool executions and routes them through
//! the permission approval flow when required. Each session has an independent
//! permission level and "always allow" rule set.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};
use uuid::Uuid;

use super::permissions::{
    PermissionLevel, PermissionResponse, classify_tool_risk, needs_approval,
};
use crate::services::streaming::UnifiedStreamEvent;

/// Central permission gate shared across the orchestrator and its sub-agents.
///
/// Thread-safe: all fields use interior mutability (RwLock / Mutex).
/// Designed to be wrapped in `Arc` and cloned into sub-agents.
pub struct PermissionGate {
    /// Per-session permission levels. Missing key → defaults to Standard.
    session_levels: RwLock<HashMap<String, PermissionLevel>>,
    /// Per-session "always allow" tool sets.
    /// Key: session_id, Value: set of tool names that have been permanently allowed.
    session_allow_rules: RwLock<HashMap<String, HashSet<String>>>,
    /// Pending approval requests awaiting frontend response.
    /// Key: request_id, Value: oneshot sender to unblock the waiting future.
    pending_requests: Mutex<HashMap<String, oneshot::Sender<PermissionResponse>>>,
    /// Event sender connected to the agentic loop's stream channel.
    /// Set at the start of each execution via `set_event_tx`.
    event_tx: RwLock<Option<mpsc::Sender<UnifiedStreamEvent>>>,
}

impl PermissionGate {
    /// Create a new permission gate with no sessions registered.
    pub fn new() -> Self {
        Self {
            session_levels: RwLock::new(HashMap::new()),
            session_allow_rules: RwLock::new(HashMap::new()),
            pending_requests: Mutex::new(HashMap::new()),
            event_tx: RwLock::new(None),
        }
    }

    /// Connect the event sender for streaming permission request events to the frontend.
    pub async fn set_event_tx(&self, tx: mpsc::Sender<UnifiedStreamEvent>) {
        let mut guard = self.event_tx.write().await;
        *guard = Some(tx);
    }

    /// Clear the event sender (e.g., when execution ends).
    pub async fn clear_event_tx(&self) {
        let mut guard = self.event_tx.write().await;
        *guard = None;
    }

    /// Set the permission level for a session.
    pub async fn set_session_level(&self, session_id: &str, level: PermissionLevel) {
        let mut levels = self.session_levels.write().await;
        levels.insert(session_id.to_string(), level);
    }

    /// Get the permission level for a session (defaults to Standard).
    pub async fn get_session_level(&self, session_id: &str) -> PermissionLevel {
        let levels = self.session_levels.read().await;
        levels.get(session_id).copied().unwrap_or_default()
    }

    /// Core permission check.
    ///
    /// - If the tool doesn't need approval → returns `Ok(())` immediately.
    /// - If the tool is in the session's "always allow" set → returns `Ok(())`.
    /// - Otherwise, sends a `ToolPermissionRequest` event and blocks on a oneshot
    ///   channel until the frontend responds via `resolve()`.
    ///
    /// Returns `Err(reason)` if the user denies the tool or if the event channel is unavailable.
    pub async fn check(
        &self,
        session_id: &str,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> Result<(), String> {
        let level = self.get_session_level(session_id).await;

        // Fast path: no approval needed for this tool at this level
        if !needs_approval(tool_name, args, level) {
            return Ok(());
        }

        // Check session-level "always allow" rules
        {
            let rules = self.session_allow_rules.read().await;
            if let Some(allowed) = rules.get(session_id) {
                if allowed.contains(tool_name) {
                    return Ok(());
                }
            }
        }

        // Need approval: create a oneshot channel and send the request event
        let request_id = Uuid::new_v4().to_string();
        let (resp_tx, resp_rx) = oneshot::channel::<PermissionResponse>();

        // Register the pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(request_id.clone(), resp_tx);
        }

        // Send the permission request event to the frontend
        let risk = classify_tool_risk(tool_name, args);
        let event = UnifiedStreamEvent::ToolPermissionRequest {
            request_id: request_id.clone(),
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments: serde_json::to_string(args).unwrap_or_default(),
            risk: risk.as_str().to_string(),
        };

        {
            let tx_guard = self.event_tx.read().await;
            if let Some(tx) = tx_guard.as_ref() {
                if tx.send(event).await.is_err() {
                    // Channel closed — clean up and deny
                    let mut pending = self.pending_requests.lock().await;
                    pending.remove(&request_id);
                    return Err("Permission request channel closed".to_string());
                }
            } else {
                // No event channel — clean up and deny
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&request_id);
                return Err("No event channel available for permission requests".to_string());
            }
        }

        // Block until the frontend responds (no timeout — user may take any amount of time)
        match resp_rx.await {
            Ok(response) => {
                if response.always_allow {
                    // Add to session allow rules
                    let mut rules = self.session_allow_rules.write().await;
                    rules
                        .entry(session_id.to_string())
                        .or_default()
                        .insert(tool_name.to_string());
                }
                if response.allowed {
                    Ok(())
                } else {
                    Err(format!(
                        "Tool '{}' execution denied by user",
                        tool_name
                    ))
                }
            }
            Err(_) => {
                // Sender was dropped (e.g., cancellation)
                Err("Permission request was cancelled".to_string())
            }
        }
    }

    /// Resolve a pending permission request (called by the Tauri command handler).
    pub async fn resolve(&self, request_id: &str, response: PermissionResponse) {
        let mut pending = self.pending_requests.lock().await;
        if let Some(tx) = pending.remove(request_id) {
            // If the receiver has already been dropped (e.g., timeout), this is a no-op
            let _ = tx.send(response);
        }
    }

    /// Cancel all pending permission requests for a session.
    ///
    /// Used when the user cancels execution. Drops the oneshot senders,
    /// causing the waiting futures to receive `Err(RecvError)`.
    pub async fn cancel_session_requests(&self, session_id: &str) {
        let mut pending = self.pending_requests.lock().await;
        // We can't filter by session_id from the request alone since we only
        // store the sender, but we drop ALL pending requests when a session is
        // cancelled. In practice, there is only one active session at a time.
        // For more granular control, we'd need to store session_id alongside the sender.
        //
        // Note: Dropping the oneshot::Sender causes the receiver to get RecvError,
        // which the `check()` method handles as "cancelled".
        let _ = session_id; // used for documentation; drop all pending
        pending.clear();
    }

    /// Clean up all permission state for a session.
    ///
    /// Called when a session ends or a new session begins.
    pub async fn cleanup_session(&self, session_id: &str) {
        {
            let mut levels = self.session_levels.write().await;
            levels.remove(session_id);
        }
        {
            let mut rules = self.session_allow_rules.write().await;
            rules.remove(session_id);
        }
        // Cancel any pending requests
        self.cancel_session_requests(session_id).await;
    }
}

impl Default for PermissionGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_only_tool_auto_passes() {
        let gate = PermissionGate::new();
        // Standard level: Read is ReadOnly → should pass without approval
        let result = gate
            .check("session-1", "Read", &serde_json::json!({}))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_safe_write_passes_in_standard() {
        let gate = PermissionGate::new();
        // Standard level: Write is SafeWrite → should pass without approval
        let result = gate
            .check("session-1", "Write", &serde_json::json!({}))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_dangerous_tool_blocked_without_event_tx() {
        let gate = PermissionGate::new();
        // Standard level: Bash is Dangerous → needs approval but no event_tx → error
        let result = gate
            .check("session-1", "Bash", &serde_json::json!({}))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No event channel"));
    }

    #[tokio::test]
    async fn test_dangerous_tool_allowed_in_permissive() {
        let gate = PermissionGate::new();
        gate.set_session_level("session-1", PermissionLevel::Permissive)
            .await;
        let result = gate
            .check("session-1", "Bash", &serde_json::json!({}))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_safe_write_blocked_in_strict() {
        let gate = PermissionGate::new();
        gate.set_session_level("session-1", PermissionLevel::Strict)
            .await;
        // Write needs approval in Strict, but no event_tx
        let result = gate
            .check("session-1", "Write", &serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_allows_tool() {
        let gate = Arc::new(PermissionGate::new());
        let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(16);
        gate.set_event_tx(tx).await;

        let gate_clone = Arc::clone(&gate);
        let check_handle = tokio::spawn(async move {
            gate_clone
                .check("session-1", "Bash", &serde_json::json!({"command": "ls"}))
                .await
        });

        // Wait for the permission request event
        let event = rx.recv().await.unwrap();
        if let UnifiedStreamEvent::ToolPermissionRequest { request_id, .. } = event {
            gate.resolve(
                &request_id,
                PermissionResponse {
                    request_id: request_id.clone(),
                    allowed: true,
                    always_allow: false,
                },
            )
            .await;
        } else {
            panic!("Expected ToolPermissionRequest event");
        }

        let result = check_handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_resolve_denies_tool() {
        let gate = Arc::new(PermissionGate::new());
        let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(16);
        gate.set_event_tx(tx).await;

        let gate_clone = Arc::clone(&gate);
        let check_handle = tokio::spawn(async move {
            gate_clone
                .check("session-1", "Bash", &serde_json::json!({}))
                .await
        });

        let event = rx.recv().await.unwrap();
        if let UnifiedStreamEvent::ToolPermissionRequest { request_id, .. } = event {
            gate.resolve(
                &request_id,
                PermissionResponse {
                    request_id: request_id.clone(),
                    allowed: false,
                    always_allow: false,
                },
            )
            .await;
        }

        let result = check_handle.await.unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("denied"));
    }

    #[tokio::test]
    async fn test_always_allow_persists() {
        let gate = Arc::new(PermissionGate::new());
        let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(16);
        gate.set_event_tx(tx).await;

        // First call: approve with always_allow
        let gate_clone = Arc::clone(&gate);
        let check_handle = tokio::spawn(async move {
            gate_clone
                .check("session-1", "Bash", &serde_json::json!({}))
                .await
        });

        let event = rx.recv().await.unwrap();
        if let UnifiedStreamEvent::ToolPermissionRequest { request_id, .. } = event {
            gate.resolve(
                &request_id,
                PermissionResponse {
                    request_id: request_id.clone(),
                    allowed: true,
                    always_allow: true,
                },
            )
            .await;
        }
        assert!(check_handle.await.unwrap().is_ok());

        // Second call: should auto-pass without sending event
        let result = gate
            .check("session-1", "Bash", &serde_json::json!({}))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_cleanup_session_resets_state() {
        let gate = PermissionGate::new();
        gate.set_session_level("session-1", PermissionLevel::Permissive)
            .await;

        // Verify permissive works
        let result = gate
            .check("session-1", "Bash", &serde_json::json!({}))
            .await;
        assert!(result.is_ok());

        // Clean up
        gate.cleanup_session("session-1").await;

        // Should now default to Standard (Bash needs approval, no event_tx → error)
        let result = gate
            .check("session-1", "Bash", &serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_default_session_level_is_standard() {
        let gate = PermissionGate::new();
        let level = gate.get_session_level("unregistered-session").await;
        assert_eq!(level, PermissionLevel::Standard);
    }

    #[tokio::test]
    async fn test_cancel_session_requests() {
        let gate = Arc::new(PermissionGate::new());
        let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(16);
        gate.set_event_tx(tx).await;

        let gate_clone = Arc::clone(&gate);
        let check_handle = tokio::spawn(async move {
            gate_clone
                .check("session-1", "Bash", &serde_json::json!({}))
                .await
        });

        // Wait for the event to be sent
        let _event = rx.recv().await.unwrap();

        // Cancel all pending requests
        gate.cancel_session_requests("session-1").await;

        // The check should fail with cancellation error
        let result = check_handle.await.unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cancelled"));
    }
}
