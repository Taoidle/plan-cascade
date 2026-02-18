//! EventActions Applicator
//!
//! Processes `EventActions` declared alongside agent events. The orchestrator
//! calls `apply_actions()` after handling each event to execute the declared
//! side effects in a deterministic order:
//!
//! 1. **state_delta** — Merge key-value pairs into session state
//! 2. **checkpoint_request** — Create a Timeline checkpoint
//! 3. **quality_gate_result** — Record gate result and emit to frontend
//! 4. **transfer_to_agent** — Hand off execution to another agent
//!
//! This ordering ensures that state is updated before checkpoints capture it,
//! quality gate results are recorded before any transfer, and transfers happen
//! last since they change the executing agent.

use std::collections::HashMap;

use serde_json::Value;
use tokio::sync::mpsc;

use crate::services::core::event_actions::{EventActions, QualityGateActionResult};
use crate::services::core::builders::SessionStateKey;
use crate::services::streaming::UnifiedStreamEvent;
use crate::services::timeline::TimelineService;
use crate::utils::error::{AppError, AppResult};

// ============================================================================
// Action Application Result
// ============================================================================

/// Result of applying an `EventActions` bundle.
///
/// Returned by `apply_actions()` to inform the caller of side effects
/// that were applied and any transfer request that should be handled
/// by the orchestrator's main loop.
#[derive(Debug, Default)]
pub struct ApplyActionsResult {
    /// Number of state delta entries merged.
    pub state_entries_merged: usize,
    /// Whether a checkpoint was created.
    pub checkpoint_created: bool,
    /// ID of the checkpoint if one was created.
    pub checkpoint_id: Option<String>,
    /// Whether a quality gate result was recorded.
    pub quality_gate_recorded: bool,
    /// Transfer target agent name, if a transfer was requested.
    ///
    /// The caller (orchestrator main loop) is responsible for invoking
    /// the TransferHandler with this target.
    pub transfer_target: Option<String>,
}

// ============================================================================
// State Delta Application
// ============================================================================

/// Validate and merge state delta entries into a session state map.
///
/// Each key in the `state_delta` is validated via `SessionStateKey`. Keys
/// that fail validation are collected into an error list; valid keys are
/// merged into the `session_state` map.
///
/// Keys without a recognized prefix (`user:`, `app:`, `temp:`) are
/// auto-prefixed with `app:` to ensure all agent-initiated state changes
/// are scoped to the application namespace by default.
///
/// Returns the number of entries successfully merged and any validation errors.
pub fn merge_state_delta(
    state_delta: &HashMap<String, Value>,
    session_state: &mut HashMap<String, Value>,
) -> (usize, Vec<String>) {
    let mut merged = 0usize;
    let mut errors = Vec::new();

    // Sort keys for deterministic application order
    let mut keys: Vec<&String> = state_delta.keys().collect();
    keys.sort();

    for key in keys {
        let value = &state_delta[key];
        // Auto-prefix keys without a recognized scope
        let effective_key = if key.starts_with("user:")
            || key.starts_with("app:")
            || key.starts_with("temp:")
        {
            key.clone()
        } else {
            format!("app:{}", key)
        };

        match SessionStateKey::new(&effective_key) {
            Ok(_validated) => {
                session_state.insert(effective_key, value.clone());
                merged += 1;
            }
            Err(e) => {
                errors.push(format!("Invalid state key '{}': {}", key, e));
            }
        }
    }

    (merged, errors)
}

// ============================================================================
// Checkpoint Application
// ============================================================================

/// Create a Timeline checkpoint from a `CheckpointRequest`.
///
/// Uses the provided `TimelineService` to create a checkpoint with the
/// given label and optional tracked files. Returns the checkpoint ID.
pub fn apply_checkpoint(
    timeline: &TimelineService,
    project_path: &str,
    session_id: &str,
    label: &str,
    tracked_files: &[String],
) -> AppResult<String> {
    let checkpoint = timeline.create_checkpoint(
        project_path,
        session_id,
        label,
        tracked_files,
    )?;
    Ok(checkpoint.id)
}

// ============================================================================
// Quality Gate Result Application
// ============================================================================

/// Build a `UnifiedStreamEvent` for a quality gate result.
///
/// Emits a `QualityGatesResult` event to the frontend channel.
pub fn build_quality_gate_event(
    session_id: &str,
    gate_result: &QualityGateActionResult,
) -> UnifiedStreamEvent {
    UnifiedStreamEvent::QualityGatesResult {
        session_id: session_id.to_string(),
        story_id: format!("action:{}", gate_result.gate_name),
        passed: gate_result.passed,
        summary: serde_json::json!({
            "gate_name": gate_result.gate_name,
            "passed": gate_result.passed,
            "details": gate_result.details,
            "source": "event_actions",
        }),
    }
}

// ============================================================================
// Full Action Application
// ============================================================================

/// Apply all actions from an `EventActions` bundle in deterministic order.
///
/// Processing order:
/// 1. `state_delta` — merged into session state
/// 2. `checkpoint_request` — creates a Timeline checkpoint
/// 3. `quality_gate_result` — emits quality gate event to frontend
/// 4. `transfer_to_agent` — recorded for the caller to handle
///
/// The `tx` channel is used to emit events to the frontend.
/// The `transfer_to_agent` action is NOT executed here; instead it is
/// returned in the result for the orchestrator to handle.
pub async fn apply_actions(
    actions: &EventActions,
    session_state: &mut HashMap<String, Value>,
    timeline: Option<&TimelineService>,
    project_path: &str,
    session_id: &str,
    tracked_files: &[String],
    tx: &mpsc::Sender<UnifiedStreamEvent>,
) -> AppResult<ApplyActionsResult> {
    if !actions.has_actions() {
        return Ok(ApplyActionsResult::default());
    }

    let mut result = ApplyActionsResult::default();

    // Step 1: Apply state_delta
    if !actions.state_delta.is_empty() {
        let (merged, errors) = merge_state_delta(&actions.state_delta, session_state);
        result.state_entries_merged = merged;
        if !errors.is_empty() {
            eprintln!(
                "[event-actions] State delta validation errors: {:?}",
                errors
            );
        }
    }

    // Step 2: Apply checkpoint_request
    //
    // Creates a Timeline checkpoint via TimelineService, stores the checkpoint ID
    // and metadata in session state, and emits a ToolResult event to the frontend
    // with structured checkpoint data.
    if let Some(ref cp_req) = actions.checkpoint_request {
        if let Some(timeline_svc) = timeline {
            match apply_checkpoint(
                timeline_svc,
                project_path,
                session_id,
                &cp_req.label,
                tracked_files,
            ) {
                Ok(cp_id) => {
                    result.checkpoint_created = true;
                    result.checkpoint_id = Some(cp_id.clone());

                    // Store checkpoint ID in session state for subsequent actions
                    session_state.insert(
                        "app:last_checkpoint_id".to_string(),
                        Value::String(cp_id.clone()),
                    );

                    // Store checkpoint label for context
                    session_state.insert(
                        "app:last_checkpoint_label".to_string(),
                        Value::String(cp_req.label.clone()),
                    );

                    // Store description if provided
                    if let Some(ref desc) = cp_req.description {
                        session_state.insert(
                            "app:last_checkpoint_description".to_string(),
                            Value::String(desc.clone()),
                        );
                    }

                    // Emit checkpoint event to frontend with full metadata
                    let checkpoint_data = serde_json::json!({
                        "action": "checkpoint_created",
                        "checkpoint_id": cp_id,
                        "label": cp_req.label,
                        "description": cp_req.description,
                        "session_id": session_id,
                        "tracked_files_count": tracked_files.len(),
                    });

                    let _ = tx
                        .send(UnifiedStreamEvent::ToolResult {
                            tool_id: format!("checkpoint:{}", cp_req.label),
                            result: Some(checkpoint_data.to_string()),
                            error: None,
                        })
                        .await;
                }
                Err(e) => {
                    eprintln!(
                        "[event-actions] Failed to create checkpoint '{}': {}",
                        cp_req.label, e
                    );
                }
            }
        } else {
            eprintln!(
                "[event-actions] Checkpoint requested ('{}') but no TimelineService available",
                cp_req.label
            );
        }
    }

    // Step 3: Apply quality_gate_result
    if let Some(ref gate) = actions.quality_gate_result {
        let event = build_quality_gate_event(session_id, gate);
        let _ = tx.send(event).await;
        result.quality_gate_recorded = true;
    }

    // Step 4: Record transfer_to_agent (caller handles actual transfer)
    if let Some(ref target) = actions.transfer_to_agent {
        result.transfer_target = Some(target.clone());
    }

    Ok(result)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::core::event_actions::EventActions;

    // ── merge_state_delta tests ──────────────────────────────────────

    #[test]
    fn test_merge_state_delta_empty() {
        let delta = HashMap::new();
        let mut state = HashMap::new();
        let (merged, errors) = merge_state_delta(&delta, &mut state);
        assert_eq!(merged, 0);
        assert!(errors.is_empty());
        assert!(state.is_empty());
    }

    #[test]
    fn test_merge_state_delta_valid_prefixed_keys() {
        let mut delta = HashMap::new();
        delta.insert("user:name".to_string(), Value::String("Alice".to_string()));
        delta.insert("app:version".to_string(), Value::Number(42.into()));
        delta.insert("temp:scratch".to_string(), Value::Bool(true));

        let mut state = HashMap::new();
        let (merged, errors) = merge_state_delta(&delta, &mut state);

        assert_eq!(merged, 3);
        assert!(errors.is_empty());
        assert_eq!(state["user:name"], Value::String("Alice".to_string()));
        assert_eq!(state["app:version"], Value::Number(42.into()));
        assert_eq!(state["temp:scratch"], Value::Bool(true));
    }

    #[test]
    fn test_merge_state_delta_auto_prefix_unprefixed_keys() {
        let mut delta = HashMap::new();
        delta.insert("progress".to_string(), Value::Number(75.into()));

        let mut state = HashMap::new();
        let (merged, errors) = merge_state_delta(&delta, &mut state);

        assert_eq!(merged, 1);
        assert!(errors.is_empty());
        // Unprefixed key should be auto-prefixed with "app:"
        assert_eq!(state["app:progress"], Value::Number(75.into()));
    }

    #[test]
    fn test_merge_state_delta_overrides_existing() {
        let mut delta = HashMap::new();
        delta.insert("app:count".to_string(), Value::Number(10.into()));

        let mut state = HashMap::new();
        state.insert("app:count".to_string(), Value::Number(5.into()));

        let (merged, _errors) = merge_state_delta(&delta, &mut state);
        assert_eq!(merged, 1);
        assert_eq!(state["app:count"], Value::Number(10.into()));
    }

    #[test]
    fn test_merge_state_delta_deterministic_order() {
        // Keys should be processed alphabetically for determinism
        let mut delta = HashMap::new();
        delta.insert("app:z_last".to_string(), Value::Number(1.into()));
        delta.insert("app:a_first".to_string(), Value::Number(2.into()));
        delta.insert("app:m_middle".to_string(), Value::Number(3.into()));

        let mut state = HashMap::new();
        let (merged, errors) = merge_state_delta(&delta, &mut state);

        assert_eq!(merged, 3);
        assert!(errors.is_empty());
    }

    // ── apply_checkpoint tests ───────────────────────────────────────

    #[test]
    fn test_apply_checkpoint_creates_checkpoint() {
        let timeline = TimelineService::new();
        let temp_dir = std::env::temp_dir().join(format!(
            "ea_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let project_path = temp_dir.to_string_lossy().to_string();

        let result = apply_checkpoint(
            &timeline,
            &project_path,
            "test-session",
            "test-label",
            &[],
        );

        assert!(result.is_ok());
        let cp_id = result.unwrap();
        assert!(!cp_id.is_empty());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // ── build_quality_gate_event tests ───────────────────────────────

    #[test]
    fn test_build_quality_gate_event_passed() {
        let gate = QualityGateActionResult {
            gate_name: "lint".to_string(),
            passed: true,
            details: None,
        };
        let event = build_quality_gate_event("session-1", &gate);
        match event {
            UnifiedStreamEvent::QualityGatesResult {
                session_id,
                story_id,
                passed,
                summary,
            } => {
                assert_eq!(session_id, "session-1");
                assert_eq!(story_id, "action:lint");
                assert!(passed);
                assert_eq!(summary["gate_name"], "lint");
                assert_eq!(summary["source"], "event_actions");
            }
            _ => panic!("Expected QualityGatesResult event"),
        }
    }

    #[test]
    fn test_build_quality_gate_event_failed_with_details() {
        let gate = QualityGateActionResult {
            gate_name: "test".to_string(),
            passed: false,
            details: Some("3 tests failed".to_string()),
        };
        let event = build_quality_gate_event("sess-2", &gate);
        match event {
            UnifiedStreamEvent::QualityGatesResult {
                passed, summary, ..
            } => {
                assert!(!passed);
                assert_eq!(summary["details"], "3 tests failed");
            }
            _ => panic!("Expected QualityGatesResult event"),
        }
    }

    // ── apply_actions tests ──────────────────────────────────────────

    #[tokio::test]
    async fn test_apply_actions_empty_actions_no_side_effects() {
        let actions = EventActions::none();
        let mut state = HashMap::new();
        let (tx, mut rx) = mpsc::channel(16);

        let result = apply_actions(
            &actions,
            &mut state,
            None,
            "/tmp",
            "sess",
            &[],
            &tx,
        )
        .await
        .unwrap();

        assert_eq!(result.state_entries_merged, 0);
        assert!(!result.checkpoint_created);
        assert!(!result.quality_gate_recorded);
        assert!(result.transfer_target.is_none());
        // No events should have been sent
        drop(tx);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_apply_actions_state_delta_only() {
        let actions = EventActions::none()
            .with_state("app:step", Value::String("done".to_string()));
        let mut state = HashMap::new();
        let (tx, _rx) = mpsc::channel(16);

        let result = apply_actions(
            &actions,
            &mut state,
            None,
            "/tmp",
            "sess",
            &[],
            &tx,
        )
        .await
        .unwrap();

        assert_eq!(result.state_entries_merged, 1);
        assert_eq!(state["app:step"], Value::String("done".to_string()));
    }

    #[tokio::test]
    async fn test_apply_actions_quality_gate_emits_event() {
        let actions = EventActions::none()
            .with_quality_gate("typecheck", true, None);
        let mut state = HashMap::new();
        let (tx, mut rx) = mpsc::channel(16);

        let result = apply_actions(
            &actions,
            &mut state,
            None,
            "/tmp",
            "sess-1",
            &[],
            &tx,
        )
        .await
        .unwrap();

        assert!(result.quality_gate_recorded);

        // Should have received a QualityGatesResult event
        drop(tx);
        let event = rx.try_recv().unwrap();
        match event {
            UnifiedStreamEvent::QualityGatesResult { passed, .. } => {
                assert!(passed);
            }
            _ => panic!("Expected QualityGatesResult event"),
        }
    }

    #[tokio::test]
    async fn test_apply_actions_transfer_recorded_not_executed() {
        let actions = EventActions::none()
            .with_transfer("reviewer-agent");
        let mut state = HashMap::new();
        let (tx, _rx) = mpsc::channel(16);

        let result = apply_actions(
            &actions,
            &mut state,
            None,
            "/tmp",
            "sess",
            &[],
            &tx,
        )
        .await
        .unwrap();

        assert_eq!(result.transfer_target, Some("reviewer-agent".to_string()));
    }

    #[tokio::test]
    async fn test_apply_actions_deterministic_order() {
        // All four action types combined
        let actions = EventActions::none()
            .with_state("app:count", Value::Number(1.into()))
            .with_checkpoint("save-1")
            .with_quality_gate("lint", true, None)
            .with_transfer("next-agent");

        let timeline = TimelineService::new();
        let temp_dir = std::env::temp_dir().join(format!(
            "ea_order_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let project_path = temp_dir.to_string_lossy().to_string();

        let mut state = HashMap::new();
        let (tx, mut rx) = mpsc::channel(16);

        let result = apply_actions(
            &actions,
            &mut state,
            Some(&timeline),
            &project_path,
            "sess-order",
            &[],
            &tx,
        )
        .await
        .unwrap();

        // All actions should have been applied
        assert_eq!(result.state_entries_merged, 1);
        assert!(result.checkpoint_created);
        assert!(result.quality_gate_recorded);
        assert_eq!(result.transfer_target, Some("next-agent".to_string()));

        // State should have the delta and the checkpoint ID
        assert_eq!(state["app:count"], Value::Number(1.into()));
        assert!(state.contains_key("app:last_checkpoint_id"));

        // Events should have been emitted: checkpoint ToolResult, then QualityGatesResult
        drop(tx);
        let event1 = rx.try_recv().unwrap();
        match &event1 {
            UnifiedStreamEvent::ToolResult { tool_id, .. } => {
                assert!(tool_id.starts_with("checkpoint:"));
            }
            _ => panic!("Expected checkpoint ToolResult, got {:?}", event1),
        }
        let event2 = rx.try_recv().unwrap();
        match &event2 {
            UnifiedStreamEvent::QualityGatesResult { .. } => {}
            _ => panic!("Expected QualityGatesResult, got {:?}", event2),
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_apply_actions_checkpoint_stores_id_in_state() {
        let actions = EventActions::none()
            .with_checkpoint_described("milestone", "All tests pass");

        let timeline = TimelineService::new();
        let temp_dir = std::env::temp_dir().join(format!(
            "ea_cp_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let project_path = temp_dir.to_string_lossy().to_string();

        let mut state = HashMap::new();
        let (tx, _rx) = mpsc::channel(16);

        let result = apply_actions(
            &actions,
            &mut state,
            Some(&timeline),
            &project_path,
            "sess-cp",
            &[],
            &tx,
        )
        .await
        .unwrap();

        assert!(result.checkpoint_created);
        let cp_id = result.checkpoint_id.as_ref().unwrap();
        assert_eq!(
            state["app:last_checkpoint_id"],
            Value::String(cp_id.clone())
        );

        // Verify checkpoint exists in timeline
        let checkpoint = timeline
            .get_checkpoint(&project_path, "sess-cp", cp_id)
            .unwrap();
        assert_eq!(checkpoint.label, "milestone");

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // ── Checkpoint label and description storage ─────────────────────

    #[tokio::test]
    async fn test_apply_actions_checkpoint_stores_label_and_description() {
        let actions = EventActions::none()
            .with_checkpoint_described("phase-complete", "All unit tests pass");

        let timeline = TimelineService::new();
        let temp_dir = std::env::temp_dir().join(format!(
            "ea_cp_desc_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let project_path = temp_dir.to_string_lossy().to_string();

        let mut state = HashMap::new();
        let (tx, _rx) = mpsc::channel(16);

        let _result = apply_actions(
            &actions,
            &mut state,
            Some(&timeline),
            &project_path,
            "sess-desc",
            &[],
            &tx,
        )
        .await
        .unwrap();

        // Verify label and description are stored in state
        assert_eq!(
            state["app:last_checkpoint_label"],
            Value::String("phase-complete".to_string())
        );
        assert_eq!(
            state["app:last_checkpoint_description"],
            Value::String("All unit tests pass".to_string())
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // ── ToolResult.with_event_actions tests ──────────────────────────

    #[test]
    fn test_tool_result_with_event_actions() {
        use crate::services::tools::ToolResult;

        let actions = EventActions::none()
            .with_state("app:tool_status", Value::String("done".to_string()));

        let result = ToolResult::ok("success").with_event_actions(actions);
        assert!(result.event_actions.is_some());
        let ea = result.event_actions.unwrap();
        assert!(ea.has_actions());
        assert_eq!(ea.state_delta.len(), 1);
    }

    #[test]
    fn test_tool_result_with_empty_event_actions_is_none() {
        use crate::services::tools::ToolResult;

        let actions = EventActions::none(); // empty
        let result = ToolResult::ok("success").with_event_actions(actions);
        assert!(result.event_actions.is_none());
    }

    // ── AgentEvent::Actions variant tests ────────────────────────────

    #[test]
    fn test_agent_event_actions_variant_serialization() {
        use crate::services::agent_composer::types::AgentEvent;

        let actions = EventActions::none()
            .with_state("app:step", Value::String("lint".to_string()))
            .with_quality_gate("lint", true, None);

        let event = AgentEvent::Actions {
            actions: actions.clone(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"actions\""));
        assert!(json.contains("state_delta"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            AgentEvent::Actions { actions: parsed_actions } => {
                assert!(parsed_actions.has_actions());
                assert_eq!(parsed_actions.state_delta.len(), 1);
                assert!(parsed_actions.quality_gate_result.is_some());
            }
            _ => panic!("Expected Actions variant"),
        }
    }

    #[test]
    fn test_agent_event_actions_variant_empty_actions() {
        use crate::services::agent_composer::types::AgentEvent;

        let event = AgentEvent::Actions {
            actions: EventActions::none(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"actions\""));
    }

    // ── State delta with multiple keys ordering ──────────────────────

    #[tokio::test]
    async fn test_apply_actions_state_delta_multiple_keys_all_applied() {
        let actions = EventActions::none()
            .with_state("app:alpha", Value::Number(1.into()))
            .with_state("app:beta", Value::Number(2.into()))
            .with_state("app:gamma", Value::Number(3.into()));

        let mut state = HashMap::new();
        let (tx, _rx) = mpsc::channel(16);

        let result = apply_actions(
            &actions,
            &mut state,
            None,
            "/tmp",
            "sess",
            &[],
            &tx,
        )
        .await
        .unwrap();

        assert_eq!(result.state_entries_merged, 3);
        assert_eq!(state["app:alpha"], Value::Number(1.into()));
        assert_eq!(state["app:beta"], Value::Number(2.into()));
        assert_eq!(state["app:gamma"], Value::Number(3.into()));
    }

    // ── Quality gate with details ────────────────────────────────────

    #[tokio::test]
    async fn test_apply_actions_quality_gate_with_details_emits_correctly() {
        let actions = EventActions::none()
            .with_quality_gate("test", false, Some("2 failures in module X".to_string()));

        let mut state = HashMap::new();
        let (tx, mut rx) = mpsc::channel(16);

        let result = apply_actions(
            &actions,
            &mut state,
            None,
            "/tmp",
            "sess-qg",
            &[],
            &tx,
        )
        .await
        .unwrap();

        assert!(result.quality_gate_recorded);

        drop(tx);
        let event = rx.try_recv().unwrap();
        match event {
            UnifiedStreamEvent::QualityGatesResult {
                passed,
                summary,
                ..
            } => {
                assert!(!passed);
                assert_eq!(summary["details"], "2 failures in module X");
                assert_eq!(summary["gate_name"], "test");
            }
            _ => panic!("Expected QualityGatesResult event"),
        }
    }

    // ── No checkpoint without TimelineService ────────────────────────

    #[tokio::test]
    async fn test_apply_actions_checkpoint_without_timeline_no_error() {
        let actions = EventActions::none()
            .with_checkpoint("save-point");

        let mut state = HashMap::new();
        let (tx, _rx) = mpsc::channel(16);

        // No timeline service provided
        let result = apply_actions(
            &actions,
            &mut state,
            None,
            "/tmp",
            "sess",
            &[],
            &tx,
        )
        .await
        .unwrap();

        // Checkpoint should not be created, but no error
        assert!(!result.checkpoint_created);
        assert!(result.checkpoint_id.is_none());
    }

    // ── Partial actions (only some fields set) ───────────────────────

    #[tokio::test]
    async fn test_apply_actions_only_transfer_no_other_side_effects() {
        let actions = EventActions::none()
            .with_transfer("target-agent");

        let mut state = HashMap::new();
        let (tx, mut rx) = mpsc::channel(16);

        let result = apply_actions(
            &actions,
            &mut state,
            None,
            "/tmp",
            "sess",
            &[],
            &tx,
        )
        .await
        .unwrap();

        assert_eq!(result.state_entries_merged, 0);
        assert!(!result.checkpoint_created);
        assert!(!result.quality_gate_recorded);
        assert_eq!(result.transfer_target, Some("target-agent".to_string()));

        // No events should have been sent (transfer is not emitted as event)
        drop(tx);
        assert!(rx.try_recv().is_err());
    }
}
