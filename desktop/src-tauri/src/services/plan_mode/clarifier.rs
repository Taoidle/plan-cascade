//! Plan Mode Clarifier
//!
//! Progressive Q&A question generation for the clarification phase.
//! Generates one question at a time via LLM until complete or hard cap reached.

use std::sync::Arc;

use tracing::warn;

use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message, MessageRole};
use crate::utils::error::AppResult;

use super::adapter::DomainAdapter;
use super::analyzer::extract_json_object;
use super::types::{
    ClarificationAnswer, ClarificationInputType, ClarificationQuestion, PlanAnalysis,
};

/// Hard cap on the number of clarification questions.
const MAX_CLARIFICATION_QUESTIONS: usize = 5;

/// Marker the LLM outputs when clarification is complete.
const CLARIFICATION_COMPLETE_MARKER: &str = "[CLARIFICATION_COMPLETE]";

/// Generate the next clarification question based on previous Q&A.
///
/// Returns `Ok(Some(question))` if a new question is generated,
/// `Ok(None)` if clarification is complete or should be skipped.
/// Never returns an error — on failure, logs a warning and returns `Ok(None)`.
pub async fn generate_clarification_question(
    description: &str,
    analysis: &PlanAnalysis,
    previous_qa: &[ClarificationAnswer],
    conversation_context: Option<&str>,
    language_instruction: &str,
    adapter: &dyn DomainAdapter,
    provider: Arc<dyn LlmProvider>,
) -> AppResult<Option<ClarificationQuestion>> {
    // Hard cap: no more questions after MAX_CLARIFICATION_QUESTIONS
    if previous_qa.len() >= MAX_CLARIFICATION_QUESTIONS {
        return Ok(None);
    }

    let persona = adapter.clarification_persona();

    // Build previous Q&A context
    let qa_section = if previous_qa.is_empty() {
        String::new()
    } else {
        let mut s = "\n\n## Previous Q&A\n".to_string();
        for (i, qa) in previous_qa.iter().enumerate() {
            let q_text = if qa.question_text.is_empty() {
                format!("Question {}", i + 1)
            } else {
                qa.question_text.clone()
            };
            if qa.skipped {
                s.push_str(&format!("Q{}: {}\nA: (skipped)\n\n", i + 1, q_text));
            } else {
                s.push_str(&format!("Q{}: {}\nA: {}\n\n", i + 1, q_text, qa.answer));
            }
        }
        s
    };

    let context_section = conversation_context
        .map(|c| format!("\n\n## Conversation Context\n{}", c))
        .unwrap_or_default();

    let system = format!(
        "{}\n\n{}\n\n\
         ## Output Language\n\
         {language_instruction}\n\n\
         You are helping clarify a user's task before creating a plan.\n\n\
         Generate exactly ONE clarification question that addresses the most important \
         missing information. The question should be specific and targeted.\n\n\
         If you have enough information from the task description and previous answers \
         to proceed with planning, respond ONLY with:\n\
         {CLARIFICATION_COMPLETE_MARKER}\n\n\
         Otherwise, respond with a JSON object:\n\
         ```json\n\
         {{\n\
           \"question\": \"Your specific clarification question\",\n\
           \"hint\": \"A helpful hint or example answer\",\n\
           \"inputType\": \"text|textarea|boolean\"\n\
         }}\n\
         ```\n\n\
         IMPORTANT: The question and hint MUST follow the output language instruction above.\n\
         Only ask questions about genuinely missing critical information. \
         Do not ask about obvious details or repeat previous questions.",
        persona.identity_prompt,
        persona.thinking_style,
    );

    let user_msg = format!(
        "## Task\n{description}\n\n\
         ## Analysis Summary\n\
         Domain: {}\n\
         Complexity: {}/10\n\
         Suggested approach: {}\n\
         Reasoning: {}\
         {qa_section}{context_section}\n\n\
         Generate the next clarification question, or respond with {CLARIFICATION_COMPLETE_MARKER} \
         if you have enough information.",
        analysis.domain,
        analysis.complexity,
        analysis.suggested_approach,
        analysis.reasoning,
    );

    let messages = vec![Message::text(MessageRole::User, user_msg)];

    let options = LlmRequestOptions {
        temperature_override: Some(persona.expert_temperature),
        ..Default::default()
    };

    let response = match provider
        .send_message(messages, Some(system), vec![], options)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("Clarification question generation failed: {e}");
            return Ok(None);
        }
    };

    let text = response.content.as_deref().unwrap_or("");

    // Check for completion marker
    if text.contains(CLARIFICATION_COMPLETE_MARKER) {
        return Ok(None);
    }

    // Parse the question
    match parse_clarification_question(text, previous_qa.len()) {
        Some(q) => Ok(Some(q)),
        None => {
            warn!(
                "Failed to parse clarification question from LLM response: {}",
                &text[..text.len().min(200)]
            );
            Ok(None)
        }
    }
}

/// Parse a single clarification question from LLM response text.
fn parse_clarification_question(text: &str, question_index: usize) -> Option<ClarificationQuestion> {
    let json_str = extract_json_object(text)?;

    let parsed: serde_json::Value = serde_json::from_str(&json_str).ok()?;

    let question = parsed.get("question").and_then(|v| v.as_str())?.to_string();

    if question.is_empty() {
        return None;
    }

    let hint = parsed
        .get("hint")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let input_type_str = parsed
        .get("inputType")
        .and_then(|v| v.as_str())
        .unwrap_or("text");

    let input_type = match input_type_str {
        "textarea" => ClarificationInputType::Textarea,
        "boolean" => ClarificationInputType::Boolean,
        _ => ClarificationInputType::Text,
    };

    Some(ClarificationQuestion {
        question_id: format!("q{}", question_index + 1),
        question,
        hint,
        input_type,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_clarification_question_basic() {
        let text = r#"```json
{
  "question": "What is the target audience?",
  "hint": "e.g., developers, managers",
  "inputType": "text"
}
```"#;
        let q = parse_clarification_question(text, 0).unwrap();
        assert_eq!(q.question_id, "q1");
        assert_eq!(q.question, "What is the target audience?");
        assert_eq!(q.hint, Some("e.g., developers, managers".to_string()));
        assert!(matches!(q.input_type, ClarificationInputType::Text));
    }

    #[test]
    fn test_parse_clarification_question_textarea() {
        let text = r#"{"question": "Describe your requirements", "hint": "", "inputType": "textarea"}"#;
        let q = parse_clarification_question(text, 2).unwrap();
        assert_eq!(q.question_id, "q3");
        assert!(matches!(q.input_type, ClarificationInputType::Textarea));
        assert_eq!(q.hint, None); // empty hint → None
    }

    #[test]
    fn test_parse_clarification_question_boolean() {
        let text = r#"{"question": "Do you need SEO?", "hint": "yes or no", "inputType": "boolean"}"#;
        let q = parse_clarification_question(text, 1).unwrap();
        assert_eq!(q.question_id, "q2");
        assert!(matches!(q.input_type, ClarificationInputType::Boolean));
    }

    #[test]
    fn test_parse_clarification_question_missing_question() {
        let text = r#"{"question": "", "hint": "some hint", "inputType": "text"}"#;
        assert!(parse_clarification_question(text, 0).is_none());
    }

    #[test]
    fn test_parse_clarification_question_no_json() {
        let text = "This is just plain text with no JSON.";
        assert!(parse_clarification_question(text, 0).is_none());
    }

    #[test]
    fn test_parse_clarification_question_with_preamble() {
        let text = r#"Here's a clarification question for you:

```json
{
  "question": "What format should the output be?",
  "hint": "PDF, HTML, or Markdown",
  "inputType": "text"
}
```

This will help determine the output format."#;
        let q = parse_clarification_question(text, 4).unwrap();
        assert_eq!(q.question_id, "q5");
        assert_eq!(q.question, "What format should the output be?");
    }

    #[test]
    fn test_completion_marker_detection() {
        let text = "[CLARIFICATION_COMPLETE]";
        assert!(text.contains(CLARIFICATION_COMPLETE_MARKER));
    }
}
