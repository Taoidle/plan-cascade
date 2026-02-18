//! Event + Actions Architecture
//!
//! Implements the Event + Actions pattern where events are immutable data
//! and side effects are declared via an `EventActions` struct.
//!
//! Instead of events directly causing state mutations, agent transfers,
//! or checkpoint creation, they declare their desired side effects in an
//! `EventActions` struct. The orchestrator then processes these actions
//! after handling the event.
//!
//! Benefits:
//! - Events become pure data (easy to serialize, log, replay)
//! - Side effects are explicit and testable
//! - Orchestrator has full control over action processing order
//! - Easy to add new action types without modifying event variants
//!
//! Integration with `AgentEvent`:
//! - `AgentEvent` gains an optional `actions: Option<EventActions>` field
//! - Defaults to `None` for backward compatibility
//! - Existing code that doesn't use actions is unaffected

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// EventActions
// ============================================================================

/// Actions that can be declared alongside an immutable event.
///
/// Instead of events causing side effects directly, they declare
/// what side effects should happen via this struct. The orchestrator
/// then processes the actions after handling the event.
///
/// This pattern makes events immutable and side effects explicit,
/// improving testability and debuggability.
///
/// # Example
///
/// ```ignore
/// let actions = EventActions::none()
///     .with_state("progress", json!(0.75))
///     .with_checkpoint("after-lint-pass");
///
/// // Attach to an event
/// let event_with_actions = AgentEventWithActions {
///     event: AgentEvent::ToolResult { name: "lint".into(), result: "ok".into() },
///     actions: Some(actions),
/// };
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventActions {
    /// Key-value pairs to merge into session state.
    ///
    /// These are applied atomically after the event is processed.
    /// Keys follow the session state prefix convention (user:, app:, temp:).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub state_delta: HashMap<String, Value>,

    /// Request to transfer execution to another agent.
    ///
    /// When set, the orchestrator should hand off execution to the
    /// named agent after processing this event.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer_to_agent: Option<String>,

    /// Request to create a checkpoint at this point.
    ///
    /// The orchestrator should persist the current execution state
    /// so it can be resumed from this point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_request: Option<CheckpointRequest>,

    /// Result of a quality gate evaluation.
    ///
    /// Allows tools and agents to report quality gate results as
    /// actions rather than requiring the orchestrator to interpret
    /// tool output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_gate_result: Option<QualityGateActionResult>,
}

/// Request to create a checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckpointRequest {
    /// Label for the checkpoint.
    pub label: String,
    /// Optional description of what state is being checkpointed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Result of a quality gate evaluation, declared as an action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QualityGateActionResult {
    /// Name of the quality gate (e.g., "typecheck", "lint", "test").
    pub gate_name: String,
    /// Whether the gate passed.
    pub passed: bool,
    /// Details or error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl EventActions {
    /// Create empty actions (no side effects).
    pub fn none() -> Self {
        Self::default()
    }

    /// Check if there are any actions to process.
    pub fn has_actions(&self) -> bool {
        !self.state_delta.is_empty()
            || self.transfer_to_agent.is_some()
            || self.checkpoint_request.is_some()
            || self.quality_gate_result.is_some()
    }

    /// Builder: add a state delta entry.
    pub fn with_state(mut self, key: impl Into<String>, value: Value) -> Self {
        self.state_delta.insert(key.into(), value);
        self
    }

    /// Builder: set transfer target.
    pub fn with_transfer(mut self, agent: impl Into<String>) -> Self {
        self.transfer_to_agent = Some(agent.into());
        self
    }

    /// Builder: set checkpoint request.
    pub fn with_checkpoint(mut self, label: impl Into<String>) -> Self {
        self.checkpoint_request = Some(CheckpointRequest {
            label: label.into(),
            description: None,
        });
        self
    }

    /// Builder: set checkpoint request with description.
    pub fn with_checkpoint_described(
        mut self,
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.checkpoint_request = Some(CheckpointRequest {
            label: label.into(),
            description: Some(description.into()),
        });
        self
    }

    /// Builder: set quality gate result.
    pub fn with_quality_gate(
        mut self,
        gate_name: impl Into<String>,
        passed: bool,
        details: Option<String>,
    ) -> Self {
        self.quality_gate_result = Some(QualityGateActionResult {
            gate_name: gate_name.into(),
            passed,
            details,
        });
        self
    }

    /// Merge another `EventActions` into this one.
    ///
    /// State delta entries are merged (later values override earlier).
    /// For optional fields, the `other` value takes precedence if set.
    pub fn merge(mut self, other: EventActions) -> Self {
        for (k, v) in other.state_delta {
            self.state_delta.insert(k, v);
        }
        if other.transfer_to_agent.is_some() {
            self.transfer_to_agent = other.transfer_to_agent;
        }
        if other.checkpoint_request.is_some() {
            self.checkpoint_request = other.checkpoint_request;
        }
        if other.quality_gate_result.is_some() {
            self.quality_gate_result = other.quality_gate_result;
        }
        self
    }
}

// ============================================================================
// AgentEventWithActions wrapper
// ============================================================================

/// Wrapper that pairs an `AgentEvent` with optional `EventActions`.
///
/// This is the bridge between the existing `AgentEvent` enum and the
/// new actions pattern. It can be used alongside the existing event
/// system without requiring changes to `AgentEvent` itself.
///
/// For backward compatibility, `actions` defaults to `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEventWithActions {
    /// The immutable event data.
    #[serde(flatten)]
    pub event: crate::services::agent_composer::types::AgentEvent,
    /// Optional declared side effects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<EventActions>,
}

impl AgentEventWithActions {
    /// Create a wrapper with no actions (backward compatible).
    pub fn from_event(event: crate::services::agent_composer::types::AgentEvent) -> Self {
        Self {
            event,
            actions: None,
        }
    }

    /// Create a wrapper with actions.
    pub fn with_actions(
        event: crate::services::agent_composer::types::AgentEvent,
        actions: EventActions,
    ) -> Self {
        Self {
            event,
            actions: if actions.has_actions() {
                Some(actions)
            } else {
                None
            },
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::agent_composer::types::AgentEvent;

    // ── EventActions construction tests ──────────────────────────────

    #[test]
    fn test_event_actions_none_has_no_actions() {
        let actions = EventActions::none();
        assert!(!actions.has_actions());
        assert!(actions.state_delta.is_empty());
        assert!(actions.transfer_to_agent.is_none());
        assert!(actions.checkpoint_request.is_none());
        assert!(actions.quality_gate_result.is_none());
    }

    #[test]
    fn test_event_actions_default_equals_none() {
        let default = EventActions::default();
        let none = EventActions::none();
        assert_eq!(default.has_actions(), none.has_actions());
    }

    // ── Builder tests ────────────────────────────────────────────────

    #[test]
    fn test_event_actions_with_state() {
        let actions = EventActions::none()
            .with_state("progress", Value::Number(75.into()));
        assert!(actions.has_actions());
        assert_eq!(actions.state_delta.len(), 1);
        assert_eq!(actions.state_delta["progress"], Value::Number(75.into()));
    }

    #[test]
    fn test_event_actions_with_multiple_state_entries() {
        let actions = EventActions::none()
            .with_state("key1", Value::Bool(true))
            .with_state("key2", Value::String("value".to_string()));
        assert_eq!(actions.state_delta.len(), 2);
    }

    #[test]
    fn test_event_actions_with_transfer() {
        let actions = EventActions::none()
            .with_transfer("reviewer-agent");
        assert!(actions.has_actions());
        assert_eq!(
            actions.transfer_to_agent,
            Some("reviewer-agent".to_string())
        );
    }

    #[test]
    fn test_event_actions_with_checkpoint() {
        let actions = EventActions::none()
            .with_checkpoint("after-lint");
        assert!(actions.has_actions());
        let cp = actions.checkpoint_request.unwrap();
        assert_eq!(cp.label, "after-lint");
        assert!(cp.description.is_none());
    }

    #[test]
    fn test_event_actions_with_checkpoint_described() {
        let actions = EventActions::none()
            .with_checkpoint_described("milestone-1", "All tests pass");
        let cp = actions.checkpoint_request.unwrap();
        assert_eq!(cp.label, "milestone-1");
        assert_eq!(cp.description, Some("All tests pass".to_string()));
    }

    #[test]
    fn test_event_actions_with_quality_gate_pass() {
        let actions = EventActions::none()
            .with_quality_gate("typecheck", true, None);
        assert!(actions.has_actions());
        let qg = actions.quality_gate_result.unwrap();
        assert_eq!(qg.gate_name, "typecheck");
        assert!(qg.passed);
        assert!(qg.details.is_none());
    }

    #[test]
    fn test_event_actions_with_quality_gate_fail() {
        let actions = EventActions::none()
            .with_quality_gate("lint", false, Some("3 warnings found".to_string()));
        let qg = actions.quality_gate_result.unwrap();
        assert!(!qg.passed);
        assert_eq!(qg.details, Some("3 warnings found".to_string()));
    }

    #[test]
    fn test_event_actions_combined_builder() {
        let actions = EventActions::none()
            .with_state("step", Value::String("lint".to_string()))
            .with_transfer("next-agent")
            .with_checkpoint("pre-transfer")
            .with_quality_gate("lint", true, None);

        assert!(actions.has_actions());
        assert_eq!(actions.state_delta.len(), 1);
        assert!(actions.transfer_to_agent.is_some());
        assert!(actions.checkpoint_request.is_some());
        assert!(actions.quality_gate_result.is_some());
    }

    // ── Merge tests ──────────────────────────────────────────────────

    #[test]
    fn test_event_actions_merge_state() {
        let a = EventActions::none()
            .with_state("key1", Value::Bool(true))
            .with_state("key2", Value::Bool(false));

        let b = EventActions::none()
            .with_state("key2", Value::Bool(true))  // overrides
            .with_state("key3", Value::Number(42.into()));

        let merged = a.merge(b);
        assert_eq!(merged.state_delta.len(), 3);
        assert_eq!(merged.state_delta["key1"], Value::Bool(true));
        assert_eq!(merged.state_delta["key2"], Value::Bool(true)); // overridden
        assert_eq!(merged.state_delta["key3"], Value::Number(42.into()));
    }

    #[test]
    fn test_event_actions_merge_transfer_override() {
        let a = EventActions::none().with_transfer("agent-1");
        let b = EventActions::none().with_transfer("agent-2");
        let merged = a.merge(b);
        assert_eq!(merged.transfer_to_agent, Some("agent-2".to_string()));
    }

    #[test]
    fn test_event_actions_merge_keeps_original_when_other_is_none() {
        let a = EventActions::none()
            .with_transfer("agent-1")
            .with_checkpoint("cp-1");
        let b = EventActions::none(); // empty

        let merged = a.merge(b);
        assert_eq!(merged.transfer_to_agent, Some("agent-1".to_string()));
        assert!(merged.checkpoint_request.is_some());
    }

    // ── Serialization tests ──────────────────────────────────────────

    #[test]
    fn test_event_actions_serialization_empty() {
        let actions = EventActions::none();
        let json = serde_json::to_string(&actions).unwrap();
        // Empty fields are skipped
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_event_actions_serialization_with_state() {
        let actions = EventActions::none()
            .with_state("key", Value::String("value".to_string()));
        let json = serde_json::to_string(&actions).unwrap();
        assert!(json.contains("state_delta"));
        assert!(json.contains("\"key\":\"value\""));
    }

    #[test]
    fn test_event_actions_deserialization() {
        let json = r#"{"state_delta":{"x":1},"transfer_to_agent":"agent-b"}"#;
        let actions: EventActions = serde_json::from_str(json).unwrap();
        assert_eq!(actions.state_delta["x"], Value::Number(1.into()));
        assert_eq!(actions.transfer_to_agent, Some("agent-b".to_string()));
        assert!(actions.checkpoint_request.is_none());
        assert!(actions.quality_gate_result.is_none());
    }

    #[test]
    fn test_event_actions_deserialization_empty() {
        let json = "{}";
        let actions: EventActions = serde_json::from_str(json).unwrap();
        assert!(!actions.has_actions());
    }

    #[test]
    fn test_event_actions_roundtrip() {
        let original = EventActions::none()
            .with_state("progress", serde_json::json!(0.5))
            .with_transfer("reviewer")
            .with_checkpoint_described("mid-point", "Halfway done")
            .with_quality_gate("lint", true, Some("All clean".to_string()));

        let json = serde_json::to_string(&original).unwrap();
        let restored: EventActions = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.state_delta.len(), 1);
        assert_eq!(restored.transfer_to_agent, Some("reviewer".to_string()));
        assert_eq!(
            restored.checkpoint_request.as_ref().unwrap().label,
            "mid-point"
        );
        assert!(restored.quality_gate_result.as_ref().unwrap().passed);
    }

    // ── CheckpointRequest tests ──────────────────────────────────────

    #[test]
    fn test_checkpoint_request_serialization() {
        let cp = CheckpointRequest {
            label: "save-point".to_string(),
            description: Some("After all tests pass".to_string()),
        };
        let json = serde_json::to_string(&cp).unwrap();
        assert!(json.contains("save-point"));
        assert!(json.contains("After all tests pass"));
    }

    #[test]
    fn test_checkpoint_request_no_description() {
        let cp = CheckpointRequest {
            label: "quick-save".to_string(),
            description: None,
        };
        let json = serde_json::to_string(&cp).unwrap();
        assert!(!json.contains("description"));
    }

    // ── QualityGateActionResult tests ────────────────────────────────

    #[test]
    fn test_quality_gate_result_serialization() {
        let qg = QualityGateActionResult {
            gate_name: "test".to_string(),
            passed: false,
            details: Some("2 tests failed".to_string()),
        };
        let json = serde_json::to_string(&qg).unwrap();
        assert!(json.contains("\"gate_name\":\"test\""));
        assert!(json.contains("\"passed\":false"));
        assert!(json.contains("2 tests failed"));
    }

    // ── AgentEventWithActions tests ──────────────────────────────────

    #[test]
    fn test_agent_event_with_actions_from_event_no_actions() {
        let event = AgentEvent::TextDelta {
            content: "Hello".to_string(),
        };
        let wrapped = AgentEventWithActions::from_event(event);
        assert!(wrapped.actions.is_none());
    }

    #[test]
    fn test_agent_event_with_actions_with_actions() {
        let event = AgentEvent::ToolResult {
            name: "lint".to_string(),
            result: "ok".to_string(),
        };
        let actions = EventActions::none()
            .with_quality_gate("lint", true, None)
            .with_state("lint_passed", Value::Bool(true));

        let wrapped = AgentEventWithActions::with_actions(event, actions);
        assert!(wrapped.actions.is_some());
        let a = wrapped.actions.unwrap();
        assert!(a.quality_gate_result.unwrap().passed);
        assert_eq!(a.state_delta["lint_passed"], Value::Bool(true));
    }

    #[test]
    fn test_agent_event_with_actions_empty_actions_becomes_none() {
        let event = AgentEvent::Done { output: None };
        let actions = EventActions::none(); // no actual actions

        let wrapped = AgentEventWithActions::with_actions(event, actions);
        // Empty actions should be normalized to None
        assert!(wrapped.actions.is_none());
    }

    #[test]
    fn test_agent_event_with_actions_serialization() {
        let event = AgentEvent::StateUpdate {
            key: "result".to_string(),
            value: serde_json::json!(42),
        };
        let actions = EventActions::none()
            .with_transfer("next-agent");

        let wrapped = AgentEventWithActions::with_actions(event, actions);
        let json = serde_json::to_string(&wrapped).unwrap();
        assert!(json.contains("state_update"));
        assert!(json.contains("transfer_to_agent"));
        assert!(json.contains("next-agent"));
    }

    #[test]
    fn test_agent_event_with_actions_backward_compatible_deserialization() {
        // Simulate an event from old code that doesn't have actions
        let json = r#"{"type":"text_delta","content":"Hello"}"#;
        let wrapped: AgentEventWithActions = serde_json::from_str(json).unwrap();
        assert!(wrapped.actions.is_none());
        match wrapped.event {
            AgentEvent::TextDelta { content } => assert_eq!(content, "Hello"),
            _ => panic!("Expected TextDelta"),
        }
    }
}
