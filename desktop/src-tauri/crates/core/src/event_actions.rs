//! Event Actions
//!
//! Portable EventActions types extracted from the main crate's
//! `services::core::event_actions` module.
//!
//! These types are self-contained (only depend on serde + serde_json + HashMap)
//! and are used by the tools crate to attach side-effect declarations to
//! `ToolResult` without depending on the full orchestrator.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventActions {
    /// Key-value pairs to merge into session state.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub state_delta: HashMap<String, Value>,

    /// Request to transfer execution to another agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer_to_agent: Option<String>,

    /// Request to create a checkpoint at this point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_request: Option<CheckpointRequest>,

    /// Result of a quality gate evaluation.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_actions_none_has_no_actions() {
        let actions = EventActions::none();
        assert!(!actions.has_actions());
    }

    #[test]
    fn test_event_actions_with_state() {
        let actions = EventActions::none().with_state("progress", serde_json::json!(0.75));
        assert!(actions.has_actions());
        assert_eq!(
            actions.state_delta.get("progress").unwrap(),
            &serde_json::json!(0.75)
        );
    }

    #[test]
    fn test_event_actions_with_transfer() {
        let actions = EventActions::none().with_transfer("review-agent");
        assert!(actions.has_actions());
        assert_eq!(actions.transfer_to_agent.as_deref(), Some("review-agent"));
    }

    #[test]
    fn test_event_actions_with_checkpoint() {
        let actions = EventActions::none().with_checkpoint("after-lint");
        assert!(actions.has_actions());
        assert_eq!(
            actions.checkpoint_request.as_ref().unwrap().label,
            "after-lint"
        );
        assert!(actions
            .checkpoint_request
            .as_ref()
            .unwrap()
            .description
            .is_none());
    }

    #[test]
    fn test_event_actions_with_checkpoint_described() {
        let actions =
            EventActions::none().with_checkpoint_described("after-lint", "All lint checks passed");
        assert!(actions.has_actions());
        let cp = actions.checkpoint_request.as_ref().unwrap();
        assert_eq!(cp.label, "after-lint");
        assert_eq!(cp.description.as_deref(), Some("All lint checks passed"));
    }

    #[test]
    fn test_event_actions_with_quality_gate() {
        let actions = EventActions::none().with_quality_gate("typecheck", true, None);
        assert!(actions.has_actions());
        let qg = actions.quality_gate_result.as_ref().unwrap();
        assert_eq!(qg.gate_name, "typecheck");
        assert!(qg.passed);
        assert!(qg.details.is_none());
    }

    #[test]
    fn test_event_actions_merge() {
        let a = EventActions::none()
            .with_state("x", serde_json::json!(1))
            .with_transfer("agent-a");
        let b = EventActions::none()
            .with_state("y", serde_json::json!(2))
            .with_checkpoint("cp1");
        let merged = a.merge(b);
        assert_eq!(merged.state_delta.len(), 2);
        assert_eq!(merged.transfer_to_agent.as_deref(), Some("agent-a"));
        assert!(merged.checkpoint_request.is_some());
    }

    #[test]
    fn test_event_actions_merge_override() {
        let a = EventActions::none().with_transfer("agent-a");
        let b = EventActions::none().with_transfer("agent-b");
        let merged = a.merge(b);
        assert_eq!(merged.transfer_to_agent.as_deref(), Some("agent-b"));
    }
}
