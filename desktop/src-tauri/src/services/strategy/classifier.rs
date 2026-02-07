//! Intent Classifier
//!
//! Classifies user input into different intents and maps task descriptions
//! to execution strategies. This is a Rust port of the Python
//! `intent_classifier.py` module.
//!
//! Uses a rule-based approach with regex patterns for fast, zero-cost
//! classification. Can be extended with LLM classification for uncertain
//! cases via the existing `services/llm/` provider abstraction.

use regex::Regex;
use serde::{Deserialize, Serialize};

/// User intent types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Intent {
    /// General conversation
    Chat,
    /// Execute a development task
    Task,
    /// Query information about the project
    Query,
    /// Cannot determine
    Unclear,
}

impl std::fmt::Display for Intent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Intent::Chat => write!(f, "chat"),
            Intent::Task => write!(f, "task"),
            Intent::Query => write!(f, "query"),
            Intent::Unclear => write!(f, "unclear"),
        }
    }
}

/// Result of intent classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentResult {
    /// Classified intent
    pub intent: Intent,
    /// Confidence in the classification (0.0 - 1.0)
    pub confidence: f64,
    /// Human-readable reasoning
    pub reasoning: String,
    /// Suggested UI mode
    pub suggested_mode: String,
}

impl IntentResult {
    /// Check if the confidence is above a threshold.
    pub fn is_confident(&self, threshold: f64) -> bool {
        self.confidence >= threshold
    }
}

/// Pattern entry: compiled regex + associated score.
struct PatternEntry {
    regex: Regex,
    score: f64,
}

/// Intent classifier using rule-based heuristics.
///
/// The classifier matches the user message against sets of regex patterns
/// for each intent category (task, query, chat) and returns the highest-
/// scoring match.
pub struct IntentClassifier {
    task_patterns: Vec<PatternEntry>,
    query_patterns: Vec<PatternEntry>,
    chat_patterns: Vec<PatternEntry>,
    expert_patterns: Vec<Regex>,
}

impl Default for IntentClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl IntentClassifier {
    /// Create a new classifier with compiled regex patterns.
    pub fn new() -> Self {
        Self {
            task_patterns: Self::compile_patterns(&[
                (r"(?i)(implement|create|add|modify|delete|fix|refactor|optimize)\s+", 0.85),
                (r"(?i)(build|make|write)\s+(a|an|the)\s+", 0.80),
                (r"(?i)(change|update|replace)\s+.{0,30}\s+(to|with)", 0.80),
                (r"(?i)please\s+(implement|create|add|fix|build)", 0.85),
                (r"(?i)(port|migrate|convert|rewrite)\s+", 0.85),
                (r"(?i)(set up|setup|configure|install)\s+", 0.80),
                (r"(?i)(remove|delete|drop)\s+(the\s+)?", 0.75),
                (r"(?i)(integrate|connect|hook up)\s+", 0.80),
            ]),
            query_patterns: Self::compile_patterns(&[
                (r"(?i)(what|where|which|how|why)\s+(is|are|does|do|did|can|should)", 0.85),
                (r"(?i)(explain|describe|tell me about)", 0.85),
                (r"(?i)(show|list|find)\s+(me\s+)?(the|all|any)", 0.75),
                (r"(?i)(analyze|inspect|review|check)\s+", 0.70),
                (r"\?$", 0.60),
            ]),
            chat_patterns: Self::compile_patterns(&[
                (r"(?i)^(hi|hello|hey|thanks|thank you)", 0.90),
                (r"(?i)(what do you think|your opinion|your thoughts)", 0.80),
                (r"(?i)(sounds good|okay|alright|got it)", 0.85),
                (r"(?i)(let's discuss|can we talk about)", 0.75),
                (r"(?i)^(yes|no|sure|nope|yep)$", 0.85),
            ]),
            expert_patterns: Self::compile_regex_list(&[
                r"(?i)(multiple|complex|comprehensive|full|complete)",
                r"(?i)(architecture|design|plan|refactor)",
                r"(?i)(multi-file|multi-module|cross-module)",
                r"(?i)(system|platform|framework)",
            ]),
        }
    }

    /// Classify a message using rule-based heuristics.
    pub fn classify(&self, message: &str) -> IntentResult {
        let task_score = self.match_patterns(message, &self.task_patterns);
        let query_score = self.match_patterns(message, &self.query_patterns);
        let chat_score = self.match_patterns(message, &self.chat_patterns);

        // Determine best intent
        let (best_intent, best_score) = [
            (Intent::Task, task_score),
            (Intent::Query, query_score),
            (Intent::Chat, chat_score),
        ]
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((Intent::Unclear, 0.0));

        // Check for expert mode indicators
        let suggested_mode = if best_intent == Intent::Task
            && self
                .expert_patterns
                .iter()
                .any(|p| p.is_match(message))
        {
            "expert"
        } else {
            "simple"
        };

        // Build reasoning
        let reasoning = if best_score >= 0.7 {
            format!("High confidence match for {} patterns", best_intent)
        } else if best_score >= 0.5 {
            format!("Moderate match for {} patterns", best_intent)
        } else {
            "No strong pattern match, uncertain".to_string()
        };

        let final_intent = if best_score >= 0.3 {
            best_intent
        } else {
            Intent::Unclear
        };

        IntentResult {
            intent: final_intent,
            confidence: best_score,
            reasoning,
            suggested_mode: suggested_mode.to_string(),
        }
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    fn compile_patterns(raw: &[(&str, f64)]) -> Vec<PatternEntry> {
        raw.iter()
            .filter_map(|(pattern, score)| {
                Regex::new(pattern).ok().map(|regex| PatternEntry {
                    regex,
                    score: *score,
                })
            })
            .collect()
    }

    fn compile_regex_list(patterns: &[&str]) -> Vec<Regex> {
        patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect()
    }

    fn match_patterns(&self, message: &str, patterns: &[PatternEntry]) -> f64 {
        patterns
            .iter()
            .filter(|entry| entry.regex.is_match(message))
            .map(|entry| entry.score)
            .fold(0.0_f64, f64::max)
    }
}

/// Get intent choices for user confirmation UI.
pub fn get_intent_choices() -> Vec<IntentChoice> {
    vec![
        IntentChoice {
            value: "task".to_string(),
            label: "Execute Task".to_string(),
            description: "Implement, create, modify, or fix something".to_string(),
        },
        IntentChoice {
            value: "query".to_string(),
            label: "Query Info".to_string(),
            description: "Ask questions, analyze, or get information".to_string(),
        },
        IntentChoice {
            value: "chat".to_string(),
            label: "Just Chat".to_string(),
            description: "General discussion or conversation".to_string(),
        },
    ]
}

/// A choice option for intent confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentChoice {
    pub value: String,
    pub label: String,
    pub description: String,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_intent() {
        let classifier = IntentClassifier::new();

        let result = classifier.classify("implement a login page with OAuth");
        assert_eq!(result.intent, Intent::Task);
        assert!(result.confidence >= 0.7);

        let result = classifier.classify("fix the broken CSS on the dashboard");
        assert_eq!(result.intent, Intent::Task);

        let result = classifier.classify("please create a new API endpoint");
        assert_eq!(result.intent, Intent::Task);
    }

    #[test]
    fn test_query_intent() {
        let classifier = IntentClassifier::new();

        let result = classifier.classify("what is the project structure?");
        assert_eq!(result.intent, Intent::Query);
        assert!(result.confidence >= 0.6);

        let result = classifier.classify("explain how the authentication works");
        assert_eq!(result.intent, Intent::Query);

        let result = classifier.classify("where is the config file?");
        assert_eq!(result.intent, Intent::Query);
    }

    #[test]
    fn test_chat_intent() {
        let classifier = IntentClassifier::new();

        let result = classifier.classify("hello");
        assert_eq!(result.intent, Intent::Chat);
        assert!(result.confidence >= 0.8);

        let result = classifier.classify("thanks for the help");
        assert_eq!(result.intent, Intent::Chat);

        let result = classifier.classify("sounds good");
        assert_eq!(result.intent, Intent::Chat);
    }

    #[test]
    fn test_expert_mode_suggestion() {
        let classifier = IntentClassifier::new();

        let result = classifier.classify("refactor the entire architecture of the system");
        assert_eq!(result.intent, Intent::Task);
        assert_eq!(result.suggested_mode, "expert");

        let result = classifier.classify("fix a typo");
        assert_eq!(result.suggested_mode, "simple");
    }

    #[test]
    fn test_unclear_intent() {
        let classifier = IntentClassifier::new();

        let result = classifier.classify("hmm");
        assert_eq!(result.intent, Intent::Unclear);
        assert!(result.confidence < 0.3);
    }

    #[test]
    fn test_is_confident() {
        let result = IntentResult {
            intent: Intent::Task,
            confidence: 0.85,
            reasoning: "test".to_string(),
            suggested_mode: "simple".to_string(),
        };
        assert!(result.is_confident(0.7));
        assert!(!result.is_confident(0.9));
    }

    #[test]
    fn test_get_intent_choices() {
        let choices = get_intent_choices();
        assert_eq!(choices.len(), 3);
        assert_eq!(choices[0].value, "task");
        assert_eq!(choices[1].value, "query");
        assert_eq!(choices[2].value, "chat");
    }

    #[test]
    fn test_question_mark_boosts_query() {
        let classifier = IntentClassifier::new();

        let result = classifier.classify("is this working?");
        // The question mark pattern should contribute to query score
        assert!(result.confidence >= 0.5);
    }
}
