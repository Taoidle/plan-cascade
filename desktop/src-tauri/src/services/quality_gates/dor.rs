//! Definition of Ready (DoR) Gate
//!
//! Validates that a story is ready for implementation:
//! - Has a title
//! - Has a description
//! - Has at least 2 acceptance criteria
//! - All dependencies are resolved (completed)

use serde::{Deserialize, Serialize};

use crate::services::quality_gates::pipeline::{GatePhase, PipelineGateResult};

/// Story information for DoR validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryForValidation {
    /// Story ID
    pub id: String,
    /// Story title
    pub title: String,
    /// Story description
    pub description: String,
    /// Acceptance criteria
    pub acceptance_criteria: Vec<String>,
    /// Dependencies (story IDs)
    pub dependencies: Vec<String>,
}

/// DoR Gate that validates story readiness.
pub struct DoRGate {
    /// The story to validate
    story: StoryForValidation,
    /// IDs of stories that have been completed
    completed_story_ids: Vec<String>,
}

impl DoRGate {
    /// Create a new DoR gate.
    pub fn new(story: StoryForValidation, completed_story_ids: Vec<String>) -> Self {
        Self {
            story,
            completed_story_ids,
        }
    }

    /// Run the DoR validation.
    pub fn run(&self) -> PipelineGateResult {
        let mut failures = Vec::new();

        // Check title
        if self.story.title.trim().is_empty() {
            failures.push("Story must have a title".to_string());
        }

        // Check description
        if self.story.description.trim().is_empty() {
            failures.push("Story must have a description".to_string());
        }

        // Check acceptance criteria (at least 2)
        if self.story.acceptance_criteria.len() < 2 {
            failures.push(format!(
                "Story must have at least 2 acceptance criteria (found {})",
                self.story.acceptance_criteria.len()
            ));
        }

        // Check empty acceptance criteria
        let empty_criteria = self
            .story
            .acceptance_criteria
            .iter()
            .filter(|c| c.trim().is_empty())
            .count();
        if empty_criteria > 0 {
            failures.push(format!(
                "{} acceptance criteria are empty",
                empty_criteria
            ));
        }

        // Check dependencies are resolved
        let unresolved: Vec<&String> = self
            .story
            .dependencies
            .iter()
            .filter(|dep| !self.completed_story_ids.contains(dep))
            .collect();
        if !unresolved.is_empty() {
            failures.push(format!(
                "Unresolved dependencies: {}",
                unresolved
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        if failures.is_empty() {
            PipelineGateResult::passed(
                "dor",
                "Definition of Ready",
                GatePhase::PreValidation,
                0,
            )
        } else {
            PipelineGateResult::failed(
                "dor",
                "Definition of Ready",
                GatePhase::PreValidation,
                0,
                format!("Story '{}' is not ready: {} issues found", self.story.id, failures.len()),
                failures,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::quality_gates::GateStatus;

    fn valid_story() -> StoryForValidation {
        StoryForValidation {
            id: "story-001".to_string(),
            title: "Implement feature".to_string(),
            description: "Build the authentication module".to_string(),
            acceptance_criteria: vec![
                "Users can log in".to_string(),
                "Sessions are persisted".to_string(),
            ],
            dependencies: vec![],
        }
    }

    #[test]
    fn test_dor_passes_valid_story() {
        let gate = DoRGate::new(valid_story(), vec![]);
        let result = gate.run();
        assert!(result.passed);
        assert_eq!(result.status, GateStatus::Passed);
    }

    #[test]
    fn test_dor_fails_empty_title() {
        let mut story = valid_story();
        story.title = "".to_string();
        let gate = DoRGate::new(story, vec![]);
        let result = gate.run();
        assert!(!result.passed);
        assert!(result.findings.iter().any(|f| f.contains("title")));
    }

    #[test]
    fn test_dor_fails_empty_description() {
        let mut story = valid_story();
        story.description = "  ".to_string();
        let gate = DoRGate::new(story, vec![]);
        let result = gate.run();
        assert!(!result.passed);
        assert!(result.findings.iter().any(|f| f.contains("description")));
    }

    #[test]
    fn test_dor_fails_insufficient_acceptance_criteria() {
        let mut story = valid_story();
        story.acceptance_criteria = vec!["Only one criterion".to_string()];
        let gate = DoRGate::new(story, vec![]);
        let result = gate.run();
        assert!(!result.passed);
        assert!(result
            .findings
            .iter()
            .any(|f| f.contains("acceptance criteria")));
    }

    #[test]
    fn test_dor_fails_unresolved_dependencies() {
        let mut story = valid_story();
        story.dependencies = vec!["story-000".to_string()];
        let gate = DoRGate::new(story, vec![]);
        let result = gate.run();
        assert!(!result.passed);
        assert!(result
            .findings
            .iter()
            .any(|f| f.contains("Unresolved dependencies")));
    }

    #[test]
    fn test_dor_passes_with_resolved_dependencies() {
        let mut story = valid_story();
        story.dependencies = vec!["story-000".to_string()];
        let gate = DoRGate::new(story, vec!["story-000".to_string()]);
        let result = gate.run();
        assert!(result.passed);
    }

    #[test]
    fn test_dor_reports_multiple_failures() {
        let story = StoryForValidation {
            id: "bad-story".to_string(),
            title: "".to_string(),
            description: "".to_string(),
            acceptance_criteria: vec![],
            dependencies: vec!["story-000".to_string()],
        };
        let gate = DoRGate::new(story, vec![]);
        let result = gate.run();
        assert!(!result.passed);
        assert!(result.findings.len() >= 3); // title + desc + criteria + deps
    }
}
