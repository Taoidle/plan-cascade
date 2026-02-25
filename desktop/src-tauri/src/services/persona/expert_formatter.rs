//! Expert-Formatter Pipeline
//!
//! Generic two-step pipeline for persona-based LLM calls:
//! 1. **Expert step**: Persona-guided free-form analysis (natural language)
//! 2. **Formatter step**: Convert analysis into structured JSON output
//!
//! This separation improves reasoning quality by decoupling thinking from formatting.

use std::sync::Arc;

use serde::de::DeserializeOwned;
use tracing::debug;

use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message};

use super::prompt_builder;
use super::types::{Persona, PersonaConfig, PersonaRole};

/// Result of the expert-formatter pipeline.
#[derive(Debug, Clone)]
pub struct ExpertFormatterResult<T> {
    /// Natural language expert analysis (displayed to user as card content)
    pub expert_analysis: String,
    /// Parsed structured output from the formatter step
    pub structured_output: T,
    /// Which persona role produced this result
    pub persona_role: PersonaRole,
}

/// Run the expert-formatter pipeline.
///
/// Step 1: Expert call with persona system prompt → natural language analysis
/// Step 2: Formatter call with JSON schema → structured output (with retry on parse failure)
///
/// # Arguments
/// * `expert_provider` - LLM provider for the expert step
/// * `formatter_provider` - Optional separate provider for formatter (falls back to expert_provider)
/// * `persona` - The persona guiding the expert step
/// * `phase_instructions` - Phase-specific instructions for the expert
/// * `project_context` - Optional project context from exploration
/// * `user_messages` - User messages to include in the conversation
/// * `target_json_schema` - JSON schema description for the formatter
/// * `config` - Optional pipeline configuration overrides
pub async fn run_expert_formatter<T: DeserializeOwned>(
    expert_provider: Arc<dyn LlmProvider>,
    formatter_provider: Option<Arc<dyn LlmProvider>>,
    persona: &Persona,
    phase_instructions: &str,
    project_context: Option<&str>,
    user_messages: Vec<Message>,
    target_json_schema: &str,
    _config: Option<&PersonaConfig>,
) -> Result<ExpertFormatterResult<T>, String> {
    // === Step 1: Expert Analysis ===
    let expert_system_prompt =
        prompt_builder::build_expert_system_prompt(persona, phase_instructions, project_context);

    let expert_options = LlmRequestOptions {
        temperature_override: Some(persona.expert_temperature),
        ..Default::default()
    };

    debug!(
        persona = persona.role.id(),
        msg_count = user_messages.len(),
        "expert_formatter: starting expert step"
    );

    let expert_response = expert_provider
        .send_message(
            user_messages.clone(),
            Some(expert_system_prompt),
            vec![],
            expert_options,
        )
        .await
        .map_err(|e| format!("Expert step failed: {}", e))?;

    let expert_analysis = extract_response_text(&expert_response)?;

    if expert_analysis.trim().is_empty() {
        return Err("Expert step returned empty analysis".to_string());
    }

    debug!(
        persona = persona.role.id(),
        analysis_len = expert_analysis.len(),
        "expert_formatter: expert step complete"
    );

    // === Step 2: Formatter (JSON structuring) ===
    let formatter = formatter_provider.unwrap_or_else(|| expert_provider.clone());
    let formatter_system_prompt = prompt_builder::build_formatter_system_prompt(target_json_schema);
    let formatter_user_msg = prompt_builder::build_formatter_user_message(&expert_analysis);

    let formatter_options = LlmRequestOptions {
        temperature_override: Some(persona.formatter_temperature),
        ..Default::default()
    };

    let formatter_response = formatter
        .send_message(
            vec![Message::user(&formatter_user_msg)],
            Some(formatter_system_prompt.clone()),
            vec![],
            formatter_options.clone(),
        )
        .await
        .map_err(|e| format!("Formatter step failed: {}", e))?;

    let formatter_text = extract_response_text(&formatter_response)?;
    let json_str = extract_json_from_response(&formatter_text);

    // Try parsing
    match serde_json::from_str::<T>(&json_str) {
        Ok(output) => {
            debug!(
                persona = persona.role.id(),
                "expert_formatter: formatter step parsed successfully"
            );
            Ok(ExpertFormatterResult {
                expert_analysis,
                structured_output: output,
                persona_role: persona.role,
            })
        }
        Err(first_error) => {
            // Retry once with repair prompt (ADR-F002 pattern)
            debug!(
                persona = persona.role.id(),
                error = %first_error,
                "expert_formatter: formatter parse failed, retrying with repair"
            );

            let repair_msg = format!(
                "Your previous response could not be parsed as valid JSON.\n\n\
                 Parse error: {}\n\n\
                 Your previous response was:\n{}\n\n\
                 Please respond with ONLY valid JSON. No markdown fences, no explanatory text.",
                first_error, formatter_text
            );

            let retry_messages = vec![
                Message::user(&formatter_user_msg),
                Message::assistant(&formatter_text),
                Message::user(&repair_msg),
            ];

            let retry_response = formatter
                .send_message(
                    retry_messages,
                    Some(formatter_system_prompt),
                    vec![],
                    formatter_options,
                )
                .await
                .map_err(|e| format!("Formatter retry failed: {}", e))?;

            let retry_text = extract_response_text(&retry_response)?;
            let retry_json = extract_json_from_response(&retry_text);

            let output = serde_json::from_str::<T>(&retry_json).map_err(|e| {
                format!(
                    "Formatter failed to produce valid JSON after retry: {}. \
                     First error: {}",
                    e, first_error
                )
            })?;

            Ok(ExpertFormatterResult {
                expert_analysis,
                structured_output: output,
                persona_role: persona.role,
            })
        }
    }
}

/// Extract the text content from an LLM response.
///
/// Checks `content` first, then falls back to `thinking` (for reasoning models).
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
        "LLM response contained no text content (model: {}, stop_reason: {:?})",
        response.model, response.stop_reason
    ))
}

/// Extract JSON from an LLM response string.
///
/// Handles markdown code fences and extracts the JSON object/array.
fn extract_json_from_response(response_text: &str) -> String {
    let trimmed = response_text.trim();

    // Try to extract from markdown code fences
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

    // Try to find JSON object { ... }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if start <= end {
            return trimmed[start..=end].to_string();
        }
    }

    // Try to find JSON array [ ... ]
    if let (Some(start), Some(end)) = (trimmed.find('['), trimmed.rfind(']')) {
        if start <= end {
            return trimmed[start..=end].to_string();
        }
    }

    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_markdown_fences() {
        let input = r#"Here's the JSON:
```json
{"key": "value"}
```
Some trailing text"#;
        assert_eq!(extract_json_from_response(input), r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_object() {
        let input = r#"Some text {"key": "value"} more text"#;
        assert_eq!(extract_json_from_response(input), r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_array() {
        let input = r#"Here are the stories: [{"id": "1"}, {"id": "2"}]"#;
        assert_eq!(
            extract_json_from_response(input),
            r#"[{"id": "1"}, {"id": "2"}]"#
        );
    }

    #[test]
    fn test_extract_json_raw() {
        let input = r#"{"key": "value"}"#;
        assert_eq!(extract_json_from_response(input), r#"{"key": "value"}"#);
    }
}
