//! LLM-Enhanced Strategy Analyzer
//!
//! Uses an LLM to analyze task descriptions and recommend execution strategies.
//! Falls back to keyword-based analysis on failure. Implements retry-with-repair
//! per ADR-F002 for JSON parse failures.

use std::sync::Arc;

use serde::Deserialize;
use tracing::debug;

use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message};

use super::analyzer::{
    Benefit, DimensionScores, ExecutionMode, ExecutionStrategy, RiskLevel, StrategyAnalysis,
    StrategyDecision,
};

// ============================================================================
// System Prompt
// ============================================================================

const STRATEGY_SYSTEM_PROMPT: &str = r#"You are a software project analyst. Analyze the given task description and recommend an execution strategy.

Available strategies:
- "direct": Simple, single-step tasks (bug fixes, typos, small changes, single-file edits)
- "hybrid_auto": Medium tasks requiring multiple stories with dependencies (feature implementation, API integration, refactoring)
- "hybrid_worktree": Same as hybrid_auto but with Git worktree isolation (high-risk changes, database migrations, security-sensitive)
- "mega_plan": Large projects with multiple independent features (platform builds, full-stack apps, multi-service architectures)

Respond with ONLY valid JSON matching this schema:
{
  "strategy": "direct" | "hybrid_auto" | "hybrid_worktree" | "mega_plan",
  "confidence": 0.0-1.0,
  "reasoning": "Brief explanation of your recommendation",
  "estimatedStories": 1-50,
  "riskLevel": "low" | "medium" | "high",
  "parallelizationBenefit": "none" | "moderate" | "significant",
  "hasDependencies": true/false,
  "functionalAreas": ["area1", "area2", ...]
}

No markdown fences, no explanatory text. Just the raw JSON object."#;

// ============================================================================
// LLM Response Schema
// ============================================================================

/// Deserialization target for the LLM's JSON response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LlmStrategyResponse {
    strategy: String,
    confidence: f64,
    reasoning: String,
    estimated_stories: u32,
    risk_level: String,
    parallelization_benefit: String,
    has_dependencies: bool,
    functional_areas: Vec<String>,
}

// Also support snake_case variants from LLMs
impl LlmStrategyResponse {
    fn from_value(value: serde_json::Value) -> Result<Self, String> {
        // Try camelCase first, then snake_case normalization
        let json_str = serde_json::to_string(&value).map_err(|e| e.to_string())?;

        // Normalize common snake_case field names to camelCase
        let normalized = json_str
            .replace("\"estimated_stories\"", "\"estimatedStories\"")
            .replace("\"risk_level\"", "\"riskLevel\"")
            .replace(
                "\"parallelization_benefit\"",
                "\"parallelizationBenefit\"",
            )
            .replace("\"has_dependencies\"", "\"hasDependencies\"")
            .replace("\"functional_areas\"", "\"functionalAreas\"");

        serde_json::from_str(&normalized).map_err(|e| {
            format!(
                "Failed to parse LLM strategy response: {}. JSON: {:?}",
                e,
                normalized.chars().take(300).collect::<String>()
            )
        })
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Enhance a keyword-based strategy analysis using an LLM.
///
/// Sends the task description and keyword analysis to the LLM for a more
/// accurate assessment. On parse failure, retries once with a repair prompt
/// (ADR-F002 pattern). Returns the enhanced `StrategyAnalysis` or an error.
pub async fn enhance_strategy_analysis(
    provider: Arc<dyn LlmProvider>,
    description: &str,
    keyword_analysis: &StrategyAnalysis,
) -> Result<StrategyAnalysis, String> {
    let user_message = build_user_message(description, keyword_analysis);
    let messages = vec![Message::user(&user_message)];

    let options = LlmRequestOptions::default();

    // First attempt
    let response = provider
        .send_message(
            messages.clone(),
            Some(STRATEGY_SYSTEM_PROMPT.to_string()),
            vec![],
            options.clone(),
        )
        .await
        .map_err(|e| format!("LLM strategy analysis request failed: {}", e))?;

    let response_text = extract_response_text(&response)?;

    debug!(
        len = response_text.len(),
        preview = %response_text.chars().take(300).collect::<String>(),
        "llm_analyzer: first attempt response"
    );

    match parse_strategy_response(&response_text) {
        Ok(llm_response) => {
            return Ok(build_enhanced_analysis(llm_response, keyword_analysis));
        }
        Err(first_error) => {
            debug!(error = %first_error, "llm_analyzer: first attempt parse failed, retrying with repair prompt");

            // ADR-F002: Retry once with repair prompt
            let repair_message = build_repair_prompt(&response_text, &first_error);
            let mut retry_messages = messages;
            retry_messages.push(Message::assistant(&response_text));
            retry_messages.push(Message::user(&repair_message));

            let retry_response = provider
                .send_message(
                    retry_messages,
                    Some(STRATEGY_SYSTEM_PROMPT.to_string()),
                    vec![],
                    options,
                )
                .await
                .map_err(|e| format!("LLM strategy analysis retry failed: {}", e))?;

            let retry_text = extract_response_text(&retry_response)?;

            match parse_strategy_response(&retry_text) {
                Ok(llm_response) => Ok(build_enhanced_analysis(llm_response, keyword_analysis)),
                Err(second_error) => Err(format!(
                    "Failed to parse LLM strategy response after retry. \
                     First error: {}. Retry error: {}",
                    first_error, second_error
                )),
            }
        }
    }
}

// ============================================================================
// Prompt Building
// ============================================================================

fn build_user_message(description: &str, keyword_analysis: &StrategyAnalysis) -> String {
    format!(
        "Analyze the following task and recommend an execution strategy.\n\n\
         Task description:\n{}\n\n\
         Preliminary keyword analysis (for reference — you may override):\n\
         - Recommended mode: {}\n\
         - Estimated stories: {}\n\
         - Risk level: {:?}\n\
         - Confidence: {:.0}%\n\n\
         Provide your analysis as JSON.",
        description,
        keyword_analysis.recommended_mode,
        keyword_analysis.estimated_stories,
        keyword_analysis.risk_level,
        keyword_analysis.confidence * 100.0,
    )
}

fn build_repair_prompt(original_response: &str, parse_error: &str) -> String {
    format!(
        "Your previous response could not be parsed as valid JSON.\n\n\
         Parse error: {}\n\n\
         Your previous response was:\n{}\n\n\
         Please respond with ONLY a valid JSON object matching the schema. \
         No markdown fences, no explanatory text. Just the raw JSON object \
         starting with {{ and ending with }}.",
        parse_error, original_response
    )
}

// ============================================================================
// Response Parsing
// ============================================================================

/// Extract text content from an LLM response.
fn extract_response_text(
    response: &crate::services::llm::types::LlmResponse,
) -> Result<String, String> {
    if let Some(ref text) = response.content {
        if !text.trim().is_empty() {
            return Ok(text.clone());
        }
    }
    if let Some(ref thinking) = response.thinking {
        if !thinking.trim().is_empty() {
            return Ok(thinking.clone());
        }
    }
    Err(format!(
        "LLM strategy response contained no text (model: {}, stop_reason: {:?})",
        response.model, response.stop_reason
    ))
}

/// Extract JSON object from response text, handling markdown fences and surrounding text.
fn extract_json_from_response(response_text: &str) -> String {
    let trimmed = response_text.trim();

    // Try markdown code fences
    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        let content_start = if let Some(nl) = after_fence.find('\n') {
            nl + 1
        } else {
            0
        };
        let content = &after_fence[content_start..];
        if let Some(end) = content.find("```") {
            return content[..end].trim().to_string();
        }
    }

    // Try to find the first { and last } for a JSON object
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start <= end {
            return trimmed[start..=end].to_string();
        }
    }

    trimmed.to_string()
}

/// Parse the LLM response text into a structured strategy response.
fn parse_strategy_response(response_text: &str) -> Result<LlmStrategyResponse, String> {
    if response_text.trim().is_empty() {
        return Err("LLM returned empty response".to_string());
    }

    let json_str = extract_json_from_response(response_text);

    if json_str.trim().is_empty() {
        return Err(format!(
            "Could not extract JSON from LLM response (starts with: {:?})",
            response_text.chars().take(100).collect::<String>()
        ));
    }

    let value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
        format!(
            "Invalid JSON: {}. Content: {:?}",
            e,
            json_str.chars().take(200).collect::<String>()
        )
    })?;

    LlmStrategyResponse::from_value(value)
}

// ============================================================================
// Analysis Building
// ============================================================================

/// Build an enhanced StrategyAnalysis from the LLM response.
///
/// Maps the LLM's string-based strategy/risk/benefit to the typed enums,
/// and reconstructs the StrategyDecision substructure.
fn build_enhanced_analysis(
    llm: LlmStrategyResponse,
    keyword_analysis: &StrategyAnalysis,
) -> StrategyAnalysis {
    let strategy = match llm.strategy.as_str() {
        "direct" => ExecutionStrategy::Direct,
        "hybrid_auto" => ExecutionStrategy::HybridAuto,
        "hybrid_worktree" => ExecutionStrategy::HybridWorktree,
        "mega_plan" => ExecutionStrategy::MegaPlan,
        _ => keyword_analysis.strategy_decision.strategy,
    };

    let risk_level = match llm.risk_level.as_str() {
        "low" => RiskLevel::Low,
        "medium" => RiskLevel::Medium,
        "high" => RiskLevel::High,
        _ => keyword_analysis.risk_level,
    };

    let parallelization_benefit = match llm.parallelization_benefit.as_str() {
        "none" => Benefit::None,
        "moderate" => Benefit::Moderate,
        "significant" => Benefit::Significant,
        _ => keyword_analysis.parallelization_benefit,
    };

    let recommended_mode = match strategy {
        ExecutionStrategy::Direct => ExecutionMode::Chat,
        _ => ExecutionMode::Task,
    };

    let confidence = llm.confidence.clamp(0.0, 1.0);
    let estimated_stories = llm.estimated_stories.max(1);

    // Rebuild dimension scores from the LLM's assessment
    let dimension_scores = DimensionScores {
        scope: match strategy {
            ExecutionStrategy::MegaPlan => 0.9,
            ExecutionStrategy::HybridAuto | ExecutionStrategy::HybridWorktree => 0.5,
            ExecutionStrategy::Direct => 0.1,
        },
        complexity: match strategy {
            ExecutionStrategy::MegaPlan => 0.8,
            ExecutionStrategy::HybridWorktree => 0.6,
            ExecutionStrategy::HybridAuto => 0.4,
            ExecutionStrategy::Direct => 0.1,
        },
        risk: match risk_level {
            RiskLevel::High => 0.8,
            RiskLevel::Medium => 0.4,
            RiskLevel::Low => 0.1,
        },
        parallelization: match parallelization_benefit {
            Benefit::Significant => 0.8,
            Benefit::Moderate => 0.4,
            Benefit::None => 0.1,
        },
    };

    let estimated_features = match strategy {
        ExecutionStrategy::MegaPlan => (estimated_stories / 3).max(2),
        _ => 1,
    };

    let estimated_duration_hours = match strategy {
        ExecutionStrategy::MegaPlan => estimated_features as f64 * 4.0,
        ExecutionStrategy::HybridAuto | ExecutionStrategy::HybridWorktree => {
            estimated_stories as f64 * 1.0
        }
        ExecutionStrategy::Direct => 0.5,
    };

    let strategy_decision = StrategyDecision {
        strategy,
        confidence,
        reasoning: llm.reasoning.clone(),
        estimated_stories,
        estimated_features,
        estimated_duration_hours,
        complexity_indicators: llm.functional_areas.clone(),
        recommendations: keyword_analysis.strategy_decision.recommendations.clone(),
        dimension_scores: dimension_scores.clone(),
    };

    StrategyAnalysis {
        functional_areas: llm.functional_areas,
        estimated_stories,
        has_dependencies: llm.has_dependencies,
        risk_level,
        parallelization_benefit,
        recommended_mode,
        confidence,
        reasoning: llm.reasoning,
        strategy_decision,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_clean_object() {
        let input = r#"{"strategy": "direct", "confidence": 0.9}"#;
        let result = extract_json_from_response(input);
        assert!(result.contains("\"strategy\""));
    }

    #[test]
    fn test_extract_json_from_markdown_fences() {
        let input = "```json\n{\"strategy\": \"direct\"}\n```";
        let result = extract_json_from_response(input);
        assert_eq!(result, "{\"strategy\": \"direct\"}");
    }

    #[test]
    fn test_extract_json_from_surrounding_text() {
        let input = "Here is my analysis: {\"strategy\": \"hybrid_auto\"} hope that helps.";
        let result = extract_json_from_response(input);
        assert!(result.starts_with('{'));
        assert!(result.ends_with('}'));
    }

    #[test]
    fn test_parse_valid_strategy_response() {
        let json = r#"{
            "strategy": "hybrid_auto",
            "confidence": 0.85,
            "reasoning": "Task requires multiple stories",
            "estimatedStories": 5,
            "riskLevel": "medium",
            "parallelizationBenefit": "moderate",
            "hasDependencies": true,
            "functionalAreas": ["auth", "api", "database"]
        }"#;
        let result = parse_strategy_response(json).unwrap();
        assert_eq!(result.strategy, "hybrid_auto");
        assert!((result.confidence - 0.85).abs() < f64::EPSILON);
        assert_eq!(result.estimated_stories, 5);
        assert_eq!(result.functional_areas.len(), 3);
    }

    #[test]
    fn test_parse_snake_case_response() {
        let json = r#"{
            "strategy": "direct",
            "confidence": 0.95,
            "reasoning": "Simple fix",
            "estimated_stories": 1,
            "risk_level": "low",
            "parallelization_benefit": "none",
            "has_dependencies": false,
            "functional_areas": ["readme"]
        }"#;
        let result = parse_strategy_response(json).unwrap();
        assert_eq!(result.strategy, "direct");
        assert_eq!(result.estimated_stories, 1);
    }

    #[test]
    fn test_parse_empty_response_returns_error() {
        let result = parse_strategy_response("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_json_returns_error() {
        let result = parse_strategy_response("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn test_build_user_message_contains_description() {
        let analysis = make_test_analysis();
        let msg = build_user_message("Build a REST API", &analysis);
        assert!(msg.contains("Build a REST API"));
        assert!(msg.contains("keyword analysis"));
    }

    #[test]
    fn test_build_repair_prompt_contains_error() {
        let prompt = build_repair_prompt("bad json", "Expected object");
        assert!(prompt.contains("Expected object"));
        assert!(prompt.contains("bad json"));
    }

    #[test]
    fn test_build_enhanced_analysis_maps_strategy() {
        let llm = LlmStrategyResponse {
            strategy: "mega_plan".to_string(),
            confidence: 0.9,
            reasoning: "Complex project".to_string(),
            estimated_stories: 12,
            risk_level: "high".to_string(),
            parallelization_benefit: "significant".to_string(),
            has_dependencies: true,
            functional_areas: vec!["auth".to_string(), "api".to_string()],
        };
        let keyword = make_test_analysis();
        let result = build_enhanced_analysis(llm, &keyword);

        assert_eq!(
            result.strategy_decision.strategy,
            ExecutionStrategy::MegaPlan
        );
        assert_eq!(result.recommended_mode, ExecutionMode::Task);
        assert_eq!(result.risk_level, RiskLevel::High);
        assert_eq!(result.parallelization_benefit, Benefit::Significant);
        assert_eq!(result.estimated_stories, 12);
        assert!(result.has_dependencies);
        assert_eq!(result.functional_areas.len(), 2);
    }

    #[test]
    fn test_build_enhanced_analysis_clamps_confidence() {
        let llm = LlmStrategyResponse {
            strategy: "direct".to_string(),
            confidence: 1.5, // Over 1.0
            reasoning: "Test".to_string(),
            estimated_stories: 1,
            risk_level: "low".to_string(),
            parallelization_benefit: "none".to_string(),
            has_dependencies: false,
            functional_areas: vec![],
        };
        let keyword = make_test_analysis();
        let result = build_enhanced_analysis(llm, &keyword);
        assert!(result.confidence <= 1.0);
    }

    #[test]
    fn test_build_enhanced_analysis_unknown_strategy_falls_back() {
        let llm = LlmStrategyResponse {
            strategy: "unknown_strategy".to_string(),
            confidence: 0.8,
            reasoning: "Test".to_string(),
            estimated_stories: 3,
            risk_level: "low".to_string(),
            parallelization_benefit: "none".to_string(),
            has_dependencies: false,
            functional_areas: vec![],
        };
        let keyword = make_test_analysis();
        let result = build_enhanced_analysis(llm, &keyword);
        // Falls back to keyword analysis strategy
        assert_eq!(
            result.strategy_decision.strategy,
            keyword.strategy_decision.strategy
        );
    }

    // Helper to create a minimal StrategyAnalysis for tests
    fn make_test_analysis() -> StrategyAnalysis {
        StrategyAnalysis {
            functional_areas: vec![],
            estimated_stories: 1,
            has_dependencies: false,
            risk_level: RiskLevel::Low,
            parallelization_benefit: Benefit::None,
            recommended_mode: ExecutionMode::Chat,
            confidence: 0.5,
            reasoning: "Keyword analysis".to_string(),
            strategy_decision: StrategyDecision {
                strategy: ExecutionStrategy::Direct,
                confidence: 0.5,
                reasoning: "Keyword analysis".to_string(),
                estimated_stories: 1,
                estimated_features: 1,
                estimated_duration_hours: 0.5,
                complexity_indicators: vec![],
                recommendations: vec!["Consider quality gates".to_string()],
                dimension_scores: DimensionScores {
                    scope: 0.1,
                    complexity: 0.1,
                    risk: 0.0,
                    parallelization: 0.0,
                },
            },
        }
    }
}
