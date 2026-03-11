//! Permission Gate Service
//!
//! Central gate that intercepts tool executions and routes them through
//! the permission approval flow when required. Each session has an independent
//! permission level and "always allow" rule set.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use uuid::Uuid;

use super::permissions::{
    evaluate_policy, PermissionLevel, PermissionPolicyConfig, PermissionResponse, PolicyAction,
    PolicyDecision, PolicyInput,
};
use crate::services::debug_mode::{
    evaluate_debug_tool_access, runtime_capabilities_for_profile, DebugCapabilityProfile,
    DebugRuntimeCapabilities,
};
use crate::services::streaming::UnifiedStreamEvent;
use crate::services::tools::runtime_tools;

/// Max time to wait for a frontend permission decision before auto-deny.
const PERMISSION_RESPONSE_TIMEOUT_SECS: u64 = 300;

struct PendingPermissionRequest {
    session_id: String,
    sender: oneshot::Sender<PermissionResponse>,
}

/// Central permission gate shared across the orchestrator and its sub-agents.
///
/// Thread-safe: all fields use interior mutability (RwLock / Mutex).
/// Designed to be wrapped in `Arc` and cloned into sub-agents.
pub struct PermissionGate {
    /// Per-session permission levels. Missing key → defaults to Strict.
    session_levels: RwLock<HashMap<String, PermissionLevel>>,
    /// Per-session "always allow" scope keys.
    /// Key: session_id, Value: policy scope keys approved with "always allow".
    session_allow_rules: RwLock<HashMap<String, HashSet<String>>>,
    /// Pending approval requests awaiting frontend response.
    /// Key: request_id, Value: oneshot sender to unblock the waiting future.
    pending_requests: Mutex<HashMap<String, PendingPermissionRequest>>,
    /// Policy v2 runtime config (domain allowlist etc.).
    policy_config: RwLock<PermissionPolicyConfig>,
    /// Optional per-session debug capability ceilings.
    session_debug_capabilities: RwLock<HashMap<String, DebugRuntimeCapabilities>>,
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
            policy_config: RwLock::new(PermissionPolicyConfig::default()),
            session_debug_capabilities: RwLock::new(HashMap::new()),
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
    ///
    /// If the level changes, previously granted session-level "always allow"
    /// rules are cleared to avoid stale policy carry-over across modes.
    pub async fn set_session_level(&self, session_id: &str, level: PermissionLevel) {
        let previous = {
            let mut levels = self.session_levels.write().await;
            levels.insert(session_id.to_string(), level)
        };

        if previous.is_some_and(|prev| prev != level) {
            let mut rules = self.session_allow_rules.write().await;
            rules.remove(session_id);
        }
    }

    /// Get the permission level for a session (defaults to Strict).
    pub async fn get_session_level(&self, session_id: &str) -> PermissionLevel {
        let levels = self.session_levels.read().await;
        levels.get(session_id).copied().unwrap_or_default()
    }

    /// Replace the Policy v2 config.
    pub async fn set_policy_config(&self, config: PermissionPolicyConfig) {
        let mut guard = self.policy_config.write().await;
        *guard = config;
    }

    /// Get the current Policy v2 config snapshot.
    pub async fn get_policy_config(&self) -> PermissionPolicyConfig {
        self.policy_config.read().await.clone()
    }

    /// Attach or clear a debug capability profile for a session.
    pub async fn set_debug_capability_profile(
        &self,
        session_id: &str,
        profile: Option<DebugCapabilityProfile>,
    ) {
        let mut capabilities = self.session_debug_capabilities.write().await;
        if let Some(profile) = profile {
            capabilities.insert(session_id.to_string(), runtime_capabilities_for_profile(profile));
        } else {
            capabilities.remove(session_id);
        }
    }

    /// Get the debug capability snapshot for a session if one is configured.
    pub async fn get_debug_runtime_capabilities(
        &self,
        session_id: &str,
    ) -> Option<DebugRuntimeCapabilities> {
        self.session_debug_capabilities
            .read()
            .await
            .get(session_id)
            .cloned()
    }

    /// Core permission check.
    ///
    /// Uses Policy v2 (`deny > prompt > allow`) and applies session-level
    /// "always allow" rules on approval scope keys.
    ///
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
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        self.check_with_context(session_id, tool_name, args, &cwd, &cwd)
            .await
    }

    /// Context-aware permission check used by the tool execution path.
    ///
    /// `working_dir` and `project_root` are required for path-scoped Policy v2
    /// rules (for example read outside workspace).
    pub async fn check_with_context(
        &self,
        session_id: &str,
        tool_name: &str,
        args: &serde_json::Value,
        working_dir: &Path,
        project_root: &Path,
    ) -> Result<(), String> {
        let level = self.get_session_level(session_id).await;
        let config = self.get_policy_config().await;
        let mut decision = evaluate_policy(
            PolicyInput {
                tool_name,
                args,
                level,
                working_dir,
                project_root,
            },
            &config,
        );

        if let Some(debug_capabilities) = self.get_debug_runtime_capabilities(session_id).await {
            let runtime_metadata = runtime_tools::metadata_for(tool_name);
            let access = evaluate_debug_tool_access(
                &debug_capabilities,
                tool_name,
                None,
                args,
                runtime_metadata.as_ref(),
            );
            if !access.allowed {
                return Err(access.blocked_reason.unwrap_or_else(|| {
                    format!(
                        "Debug capability profile blocked tool '{tool_name}' for session '{session_id}'."
                    )
                }));
            }

            if access.requires_approval && decision.action == PolicyAction::Allow {
                let scope_key = format!(
                    "debug:{}:{}:{}",
                    tool_name.to_ascii_lowercase(),
                    match access.classification.capability_class {
                        crate::services::debug_mode::DebugCapabilityClass::Observe => "observe",
                        crate::services::debug_mode::DebugCapabilityClass::Experiment => {
                            "experiment"
                        }
                        crate::services::debug_mode::DebugCapabilityClass::Mutate => "mutate",
                    },
                    access
                        .classification
                        .tool_category
                        .as_deref()
                        .unwrap_or("generic")
                );
                decision = PolicyDecision::prompt(
                    decision.risk,
                    format!(
                        "Debug mode requires approval: {}",
                        access.classification.rationale
                    ),
                    scope_key,
                );
            }
        }

        if decision.action == PolicyAction::Allow {
            return Ok(());
        }
        if decision.action == PolicyAction::Deny {
            return Err(decision.reason);
        }

        let approval_scope_key = decision.approval_scope_key.clone();
        if approval_scope_key.is_empty() {
            return Err("Policy decision missing approval scope key".to_string());
        }

        // Check session-level "always allow" rules
        {
            let rules = self.session_allow_rules.read().await;
            if let Some(allowed) = rules.get(session_id) {
                if allowed.contains(&approval_scope_key) {
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
            pending.insert(
                request_id.clone(),
                PendingPermissionRequest {
                    session_id: session_id.to_string(),
                    sender: resp_tx,
                },
            );
        }

        // Send the permission request event to the frontend
        let event = UnifiedStreamEvent::ToolPermissionRequest {
            request_id: request_id.clone(),
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments: serde_json::to_string(args).unwrap_or_default(),
            risk: decision.risk.as_str().to_string(),
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

        // Block until the frontend responds (with timeout for robustness).
        match tokio::time::timeout(
            Duration::from_secs(PERMISSION_RESPONSE_TIMEOUT_SECS),
            resp_rx,
        )
        .await
        {
            Ok(Ok(response)) => {
                if response.always_allow {
                    // Add to session allow rules
                    let mut rules = self.session_allow_rules.write().await;
                    rules
                        .entry(session_id.to_string())
                        .or_default()
                        .insert(approval_scope_key);
                }
                if response.allowed {
                    Ok(())
                } else {
                    Err(format!(
                        "Tool '{}' execution denied by user: {}",
                        tool_name, decision.reason
                    ))
                }
            }
            Ok(Err(_)) => {
                // Sender was dropped (e.g., cancellation)
                Err("Permission request was cancelled".to_string())
            }
            Err(_) => {
                let mut pending = self.pending_requests.lock().await;
                pending.remove(&request_id);
                Err(format!(
                    "Permission request timed out after {} seconds",
                    PERMISSION_RESPONSE_TIMEOUT_SECS
                ))
            }
        }
    }

    /// Resolve a pending permission request (called by the Tauri command handler).
    pub async fn resolve(&self, request_id: &str, response: PermissionResponse) {
        let mut pending = self.pending_requests.lock().await;
        if let Some(entry) = pending.remove(request_id) {
            // If the receiver has already been dropped (e.g., timeout), this is a no-op
            let _ = entry.sender.send(response);
        }
    }

    /// Cancel all pending permission requests for a session.
    ///
    /// Used when the user cancels execution. Drops the oneshot senders,
    /// causing the waiting futures to receive `Err(RecvError)`.
    pub async fn cancel_session_requests(&self, session_id: &str) {
        let mut pending = self.pending_requests.lock().await;
        pending.retain(|_, req| req.session_id != session_id);
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
        {
            let mut capabilities = self.session_debug_capabilities.write().await;
            capabilities.remove(session_id);
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
    use std::sync::Arc;
    use tempfile::TempDir;

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
        gate.set_session_level("session-1", PermissionLevel::Standard)
            .await;
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

        // Should now default to Strict (Bash needs approval, no event_tx → error)
        let result = gate
            .check("session-1", "Bash", &serde_json::json!({}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_default_session_level_is_strict() {
        let gate = PermissionGate::new();
        let level = gate.get_session_level("unregistered-session").await;
        assert_eq!(level, PermissionLevel::Strict);
    }

    #[tokio::test]
    async fn test_level_change_clears_always_allow_rules() {
        let gate = Arc::new(PermissionGate::new());
        let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(16);
        gate.set_event_tx(tx).await;
        gate.set_session_level("session-1", PermissionLevel::Standard)
            .await;

        let gate_clone = Arc::clone(&gate);
        let check_handle = tokio::spawn(async move {
            gate_clone
                .check("session-1", "Bash", &serde_json::json!({}))
                .await
        });

        let event = rx.recv().await.unwrap();
        let request_id = if let UnifiedStreamEvent::ToolPermissionRequest { request_id, .. } = event
        {
            request_id
        } else {
            panic!("Expected ToolPermissionRequest");
        };

        gate.resolve(
            &request_id,
            PermissionResponse {
                request_id: request_id.clone(),
                allowed: true,
                always_allow: true,
            },
        )
        .await;
        assert!(check_handle.await.unwrap().is_ok());

        // Change level -> stale allow rules must be cleared.
        gate.set_session_level("session-1", PermissionLevel::Strict)
            .await;
        gate.clear_event_tx().await;

        let result = gate
            .check("session-1", "Bash", &serde_json::json!({}))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No event channel"));
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

    #[tokio::test]
    async fn test_read_outside_workspace_requires_approval() {
        let gate = Arc::new(PermissionGate::new());
        let workspace = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let outside_file = outside.path().join("outside.txt");
        std::fs::write(&outside_file, "x").unwrap();

        let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(16);
        gate.set_event_tx(tx).await;

        let gate_clone = Arc::clone(&gate);
        let ws = workspace.path().to_path_buf();
        let check_handle = tokio::spawn(async move {
            gate_clone
                .check_with_context(
                    "session-1",
                    "Read",
                    &serde_json::json!({
                        "file_path": outside_file.to_string_lossy().to_string()
                    }),
                    &ws,
                    &ws,
                )
                .await
        });

        let event = rx.recv().await.unwrap();
        let request_id = if let UnifiedStreamEvent::ToolPermissionRequest { request_id, .. } = event
        {
            request_id
        } else {
            panic!("Expected ToolPermissionRequest");
        };
        gate.resolve(
            &request_id,
            PermissionResponse {
                request_id: request_id.clone(),
                allowed: true,
                always_allow: false,
            },
        )
        .await;

        let result = check_handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_allowlisted_bash_network_command_auto_allows_without_prompt() {
        let gate = PermissionGate::new();
        gate.set_session_level("session-1", PermissionLevel::Permissive)
            .await;
        gate.set_policy_config(PermissionPolicyConfig {
            network_domain_allowlist: vec!["example.com".to_string()],
        })
        .await;

        let cwd = std::env::current_dir().unwrap();
        let result = gate
            .check_with_context(
                "session-1",
                "Bash",
                &serde_json::json!({"command": "curl https://api.example.com/v1"}),
                &cwd,
                &cwd,
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_debug_profile_blocks_mutating_tool_in_prod() {
        let gate = PermissionGate::new();
        gate.set_session_level("session-1", PermissionLevel::Permissive)
            .await;
        gate.set_debug_capability_profile(
            "session-1",
            Some(DebugCapabilityProfile::ProdObserveOnly),
        )
        .await;

        let result = gate
            .check("session-1", "Edit", &serde_json::json!({ "file_path": "src/app.ts" }))
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("blocks Mutate tools"));
    }

    #[tokio::test]
    async fn test_debug_profile_prompts_for_experiment_in_staging() {
        let gate = Arc::new(PermissionGate::new());
        gate.set_session_level("session-1", PermissionLevel::Permissive)
            .await;
        gate.set_debug_capability_profile(
            "session-1",
            Some(DebugCapabilityProfile::StagingLimited),
        )
        .await;
        let (tx, mut rx) = mpsc::channel::<UnifiedStreamEvent>(16);
        gate.set_event_tx(tx).await;

        let gate_clone = Arc::clone(&gate);
        let check_handle = tokio::spawn(async move {
            gate_clone
                .check("session-1", "Browser", &serde_json::json!({ "action": "navigate" }))
                .await
        });

        let event = rx.recv().await.unwrap();
        let request_id = if let UnifiedStreamEvent::ToolPermissionRequest { request_id, .. } = event
        {
            request_id
        } else {
            panic!("Expected ToolPermissionRequest");
        };
        gate.resolve(
            &request_id,
            PermissionResponse {
                request_id: request_id.clone(),
                allowed: true,
                always_allow: false,
            },
        )
        .await;

        assert!(check_handle.await.unwrap().is_ok());
    }
}
