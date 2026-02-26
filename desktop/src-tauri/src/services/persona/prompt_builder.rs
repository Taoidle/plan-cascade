//! Persona Prompt Builder
//!
//! Builds system prompts for the expert and formatter steps of the persona pipeline.

use super::types::Persona;

/// Build the system prompt for the expert step.
///
/// The expert step produces natural language analysis. The persona's identity prompt,
/// thinking style, and expertise are injected to guide reasoning quality.
pub fn build_expert_system_prompt(
    persona: &Persona,
    phase_instructions: &str,
    project_context: Option<&str>,
) -> String {
    build_expert_system_prompt_with_locale(persona, phase_instructions, project_context, None)
}

/// Build the system prompt for the expert step with optional locale guidance.
///
/// When locale is provided, the prompt explicitly requires response language alignment
/// while preserving original code symbols and file paths.
pub fn build_expert_system_prompt_with_locale(
    persona: &Persona,
    phase_instructions: &str,
    project_context: Option<&str>,
    locale: Option<&str>,
) -> String {
    let mut parts = Vec::with_capacity(5);

    // Persona identity
    parts.push(persona.identity_prompt.clone());

    // Thinking style guidance
    parts.push(format!(
        "\n## Thinking Approach\n{}",
        persona.thinking_style
    ));

    // Domain expertise context
    if !persona.expertise.is_empty() {
        parts.push(format!(
            "\n## Your Expertise\n{}",
            persona
                .expertise
                .iter()
                .map(|e| format!("- {}", e))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    // Project context (from exploration phase)
    if let Some(ctx) = project_context {
        parts.push(format!("\n## Project Context\n{}", ctx));
    }

    // Response language guidance
    parts.push(format!(
        "\n## Response Language\n{}",
        locale_response_instruction(locale)
    ));

    // Phase-specific instructions
    parts.push(format!("\n## Your Task\n{}", phase_instructions));

    // Output guidance for expert step
    parts.push(
        "\n## Output Format\nProvide your analysis in clear, structured natural language. \
         Use markdown headers and bullet points for organization. \
         Think step by step and explain your reasoning."
            .to_string(),
    );

    parts.join("\n")
}

fn locale_response_instruction(locale: Option<&str>) -> &'static str {
    let normalized = locale.unwrap_or("en").to_lowercase();
    if normalized.starts_with("zh") {
        "Respond in Simplified Chinese. Keep code symbols, identifiers, and file paths in original form."
    } else if normalized.starts_with("ja") {
        "Respond in Japanese. Keep code symbols, identifiers, and file paths in original form."
    } else {
        "Respond in English. Keep code symbols, identifiers, and file paths in original form."
    }
}

/// Build the system prompt for the formatter step.
///
/// The formatter step converts natural language analysis into structured JSON.
/// It receives the expert's analysis and a target JSON schema.
pub fn build_formatter_system_prompt(target_json_schema: &str) -> String {
    format!(
        r#"You are a precise data formatter. Your job is to convert a natural language analysis into a structured JSON output.

## Rules
1. Output ONLY valid JSON — no markdown fences, no explanatory text, no comments.
2. The JSON must conform exactly to the schema below.
3. Extract all relevant information from the provided analysis.
4. If a required field cannot be determined from the analysis, use a sensible default.
5. Preserve the original intent and details from the analysis.

## Target JSON Schema
{target_json_schema}

## Output
Respond with ONLY the JSON object/array. Start with {{ or [ and end with }} or ]."#
    )
}

/// Build the user message for the formatter step.
///
/// Includes the expert's analysis as context for JSON extraction.
pub fn build_formatter_user_message(expert_analysis: &str) -> String {
    format!(
        "Convert the following expert analysis into the specified JSON format:\n\n{}",
        expert_analysis
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::persona::registry::PersonaRegistry;
    use crate::services::persona::types::PersonaRole;

    #[test]
    fn test_build_expert_system_prompt_basic() {
        let persona = PersonaRegistry::get(PersonaRole::ProductManager);
        let prompt =
            build_expert_system_prompt(&persona, "Analyze the following task requirements.", None);

        assert!(prompt.contains("Technical Product Manager"));
        assert!(prompt.contains("Thinking Approach"));
        assert!(prompt.contains("Your Expertise"));
        assert!(prompt.contains("Your Task"));
        assert!(prompt.contains("Analyze the following task requirements."));
    }

    #[test]
    fn test_build_expert_system_prompt_with_context() {
        let persona = PersonaRegistry::get(PersonaRole::SoftwareArchitect);
        let prompt = build_expert_system_prompt(
            &persona,
            "Review this PRD.",
            Some("Tech stack: Rust, TypeScript. Framework: Tauri."),
        );

        assert!(prompt.contains("Project Context"));
        assert!(prompt.contains("Rust, TypeScript"));
        assert!(prompt.contains("Review this PRD."));
    }

    #[test]
    fn test_build_expert_system_prompt_with_locale() {
        let persona = PersonaRegistry::get(PersonaRole::ProductManager);
        let prompt = build_expert_system_prompt_with_locale(
            &persona,
            "Analyze requirements.",
            None,
            Some("zh-CN"),
        );

        assert!(prompt.contains("Response Language"));
        assert!(prompt.contains("Simplified Chinese"));
        assert!(prompt.contains("Analyze requirements."));
    }

    #[test]
    fn test_build_formatter_system_prompt() {
        let schema = r#"{"type": "array", "items": {"type": "object", "properties": {"id": {"type": "string"}}}}"#;
        let prompt = build_formatter_system_prompt(schema);

        assert!(prompt.contains("precise data formatter"));
        assert!(prompt.contains(schema));
        assert!(prompt.contains("ONLY valid JSON"));
    }

    #[test]
    fn test_build_formatter_user_message() {
        let analysis = "The task requires 3 stories: setup, implementation, testing.";
        let msg = build_formatter_user_message(analysis);

        assert!(msg.contains("expert analysis"));
        assert!(msg.contains("3 stories"));
    }
}
