//! Strategy Commands
//!
//! Tauri commands for task strategy analysis and intent classification.
//! Exposes the strategy analyzer and classifier services to the frontend.

use crate::models::CommandResponse;
use crate::services::strategy::analyzer::{
    AnalysisContext, ExecutionStrategy, StrategyAnalysis, StrategyAnalyzer, StrategyDecision,
    StrategyOption,
};
use crate::services::strategy::classifier::{IntentClassifier, IntentResult};
use crate::state::AppState;

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
pub async fn classify_intent(message: String) -> CommandResponse<IntentResult> {
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

/// Analyze a task description for Chat/Task mode recommendation.
///
/// Wraps the existing StrategyAnalyzer with additional heuristics to determine
/// whether the task should be handled in Chat mode (simple, direct) or Task mode
/// (structured with PRD, stories, and quality gates).
///
/// # Arguments
/// * `description` - The task description to analyze
/// * `context` - Optional analysis context (greenfield, codebase size, etc.)
///
/// # Returns
/// `CommandResponse<StrategyAnalysis>` with recommended mode, risk level,
/// parallelization benefit, and the underlying strategy decision.
#[tauri::command]
pub async fn analyze_task_for_mode(
    description: String,
    context: Option<AnalysisContext>,
) -> CommandResponse<StrategyAnalysis> {
    if description.trim().is_empty() {
        return CommandResponse::err("Task description cannot be empty");
    }

    let analysis =
        crate::services::strategy::analyzer::analyze_task_for_mode(&description, context.as_ref());
    CommandResponse::ok(analysis)
}

/// Enhance a keyword-based strategy analysis using an LLM.
///
/// Takes the task description and a pre-computed keyword analysis, calls the
/// configured LLM provider for a more accurate strategy recommendation, and
/// returns the enhanced analysis. Falls back to the keyword analysis on failure.
///
/// # Arguments
/// * `description` - The task description
/// * `keyword_analysis` - Pre-computed keyword-based analysis to enhance
/// * `provider` - LLM provider name (e.g., "anthropic", "openai")
/// * `model` - Model name (e.g., "claude-sonnet-4-20250514")
/// * `api_key` - Optional explicit API key (uses keyring if not provided)
/// * `base_url` - Optional custom base URL for the provider
/// * `app_state` - Tauri application state
///
/// # Returns
/// `CommandResponse<StrategyAnalysis>` with the LLM-enhanced analysis.
#[tauri::command]
pub async fn enhance_strategy_with_llm(
    description: String,
    keyword_analysis: StrategyAnalysis,
    provider: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    locale: Option<String>,
    app_state: tauri::State<'_, AppState>,
) -> Result<CommandResponse<StrategyAnalysis>, String> {
    if description.trim().is_empty() {
        return Ok(CommandResponse::err("Task description cannot be empty"));
    }

    let resolved_provider = provider.unwrap_or_else(|| "anthropic".to_string());
    let resolved_model = model.unwrap_or_default();

    let llm = match crate::commands::task_mode::resolve_llm_provider(
        &resolved_provider,
        &resolved_model,
        api_key,
        base_url,
        &app_state,
    )
    .await
    {
        Ok(p) => p,
        Err(e) => return Ok(CommandResponse::err(e)),
    };

    let locale_str = locale.unwrap_or_else(|| "en".to_string());

    // Call LLM analyzer with 30s timeout
    match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        crate::services::strategy::enhance_strategy_analysis(
            llm,
            &description,
            &keyword_analysis,
            &locale_str,
        ),
    )
    .await
    {
        Ok(Ok(enhanced)) => Ok(CommandResponse::ok(enhanced)),
        Ok(Err(e)) => Ok(CommandResponse::err(e)),
        Err(_) => Ok(CommandResponse::err(
            "Strategy LLM analysis timed out after 30 seconds",
        )),
    }
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
        let result =
            analyze_task_strategy("build a system with architecture".to_string(), Some(ctx)).await;
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
        assert_eq!(
            intent.intent,
            crate::services::strategy::classifier::Intent::Task
        );
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

    #[tokio::test]
    async fn test_analyze_task_for_mode_simple() {
        let result = analyze_task_for_mode("fix a typo".to_string(), None).await;
        assert!(result.success);
        let analysis = result.data.unwrap();
        assert_eq!(
            analysis.recommended_mode,
            crate::services::strategy::analyzer::ExecutionMode::Chat
        );
    }

    #[tokio::test]
    async fn test_analyze_task_for_mode_complex() {
        let result = analyze_task_for_mode(
            "Build a comprehensive platform with multiple features, microservices, complete solution, full stack, end to end: 1. Auth 2. Payments 3. Dashboard 4. Analytics 5. Testing".to_string(),
            None,
        )
        .await;
        assert!(result.success);
        let analysis = result.data.unwrap();
        assert_eq!(
            analysis.recommended_mode,
            crate::services::strategy::analyzer::ExecutionMode::Task
        );
    }

    #[tokio::test]
    async fn test_analyze_task_for_mode_empty() {
        let result = analyze_task_for_mode("".to_string(), None).await;
        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
