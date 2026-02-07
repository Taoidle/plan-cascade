//! Strategy Commands
//!
//! Tauri commands for task strategy analysis and intent classification.
//! Exposes the strategy analyzer and classifier services to the frontend.

use crate::models::CommandResponse;
use crate::services::strategy::analyzer::{
    AnalysisContext, ExecutionStrategy, StrategyAnalyzer, StrategyDecision, StrategyOption,
};
use crate::services::strategy::classifier::{IntentClassifier, IntentResult};

/// Analyze a task description and return a strategy recommendation.
///
/// Scores the task across scope, complexity, risk, and parallelization
/// dimensions, then returns the recommended execution strategy with
/// confidence score and reasoning.
///
/// # Arguments
/// * `description` - The task description to analyze
/// * `context` - Optional analysis context (greenfield, codebase size, etc.)
///
/// # Returns
/// `CommandResponse<StrategyDecision>` with strategy, confidence, reasoning,
/// estimated stories/features/duration, and dimension scores.
#[tauri::command]
pub async fn analyze_task_strategy(
    description: String,
    context: Option<AnalysisContext>,
) -> CommandResponse<StrategyDecision> {
    if description.trim().is_empty() {
        return CommandResponse::err("Task description cannot be empty");
    }

    let decision = StrategyAnalyzer::analyze(&description, context.as_ref());
    CommandResponse::ok(decision)
}

/// Return all available execution strategies with human-readable descriptions.
///
/// Used by the frontend to populate strategy selection UI elements
/// in both Simple and Expert modes.
///
/// # Returns
/// `CommandResponse<Vec<StrategyOption>>` with value, label, description,
/// and suggested story count ranges.
#[tauri::command]
pub async fn get_strategy_options() -> CommandResponse<Vec<StrategyOption>> {
    let options = crate::services::strategy::analyzer::get_strategy_options();
    CommandResponse::ok(options)
}

/// Classify user intent from a message.
///
/// Determines whether the user input is a task, query, or chat message,
/// and suggests the appropriate UI mode (simple or expert).
///
/// # Arguments
/// * `message` - The user's input message
///
/// # Returns
/// `CommandResponse<IntentResult>` with intent, confidence, reasoning,
/// and suggested mode.
#[tauri::command]
pub async fn classify_intent(
    message: String,
) -> CommandResponse<IntentResult> {
    if message.trim().is_empty() {
        return CommandResponse::err("Message cannot be empty");
    }

    let classifier = IntentClassifier::new();
    let result = classifier.classify(&message);
    CommandResponse::ok(result)
}

/// Override a strategy recommendation.
///
/// Takes an existing strategy decision and replaces its strategy with
/// the user's chosen one, setting confidence to 1.0.
///
/// # Arguments
/// * `description` - Original task description (re-analyzed for base decision)
/// * `new_strategy` - The strategy the user wants to use
/// * `reason` - Why the user is overriding
///
/// # Returns
/// `CommandResponse<StrategyDecision>` with the overridden decision.
#[tauri::command]
pub async fn override_task_strategy(
    description: String,
    new_strategy: ExecutionStrategy,
    reason: String,
) -> CommandResponse<StrategyDecision> {
    if description.trim().is_empty() {
        return CommandResponse::err("Task description cannot be empty");
    }

    let base_decision = StrategyAnalyzer::analyze(&description, None);
    let overridden = StrategyAnalyzer::override_strategy(&base_decision, new_strategy, &reason);
    CommandResponse::ok(overridden)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_analyze_task_strategy_simple() {
        let result = analyze_task_strategy("fix a typo in readme".to_string(), None).await;
        assert!(result.success);
        let decision = result.data.unwrap();
        assert_eq!(decision.strategy, ExecutionStrategy::Direct);
        assert!(decision.confidence > 0.0);
    }

    #[tokio::test]
    async fn test_analyze_task_strategy_empty() {
        let result = analyze_task_strategy("".to_string(), None).await;
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[tokio::test]
    async fn test_analyze_task_strategy_with_context() {
        let ctx = AnalysisContext {
            is_greenfield: true,
            existing_codebase_size: 0,
            has_worktrees: false,
        };
        let result = analyze_task_strategy(
            "build a system with architecture".to_string(),
            Some(ctx),
        )
        .await;
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_get_strategy_options_returns_all() {
        let result = get_strategy_options().await;
        assert!(result.success);
        let options = result.data.unwrap();
        assert_eq!(options.len(), 4);
    }

    #[tokio::test]
    async fn test_classify_intent_task() {
        let result = classify_intent("implement a new feature".to_string()).await;
        assert!(result.success);
        let intent = result.data.unwrap();
        assert_eq!(intent.intent, crate::services::strategy::classifier::Intent::Task);
    }

    #[tokio::test]
    async fn test_classify_intent_empty() {
        let result = classify_intent("  ".to_string()).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_override_task_strategy() {
        let result = override_task_strategy(
            "fix a typo".to_string(),
            ExecutionStrategy::MegaPlan,
            "I want the full plan".to_string(),
        )
        .await;
        assert!(result.success);
        let decision = result.data.unwrap();
        assert_eq!(decision.strategy, ExecutionStrategy::MegaPlan);
        assert!((decision.confidence - 1.0).abs() < f64::EPSILON);
    }
}
