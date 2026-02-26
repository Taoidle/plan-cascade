//! Writing Adapter
//!
//! Domain adapter for content creation tasks (blog posts, articles, reports, etc.).
//! Uses outline→draft→review decomposition pattern.

use async_trait::async_trait;

use crate::services::persona::types::Persona;
use crate::services::plan_mode::adapter::{build_plan_persona, DomainAdapter};
use crate::services::plan_mode::types::{
    Plan, PlanStep, StepOutput, TaskDomain, PlanPersonaRole,
};

/// Writing-focused adapter for content creation tasks.
pub struct WritingAdapter;

#[async_trait]
impl DomainAdapter for WritingAdapter {
    fn id(&self) -> &str {
        "writing"
    }

    fn display_name(&self) -> &str {
        "Writing & Content"
    }

    fn supported_domains(&self) -> Vec<TaskDomain> {
        vec![TaskDomain::Writing]
    }

    fn planning_persona(&self) -> Persona {
        let mut persona = build_plan_persona(PlanPersonaRole::Planner);
        persona.identity_prompt = r#"You are a content strategist and writing planner. You excel at:

- **Content Structure**: Organizing ideas into clear, logical outlines
- **Audience Analysis**: Tailoring content structure to the target audience
- **Writing Workflow**: Breaking writing tasks into outline → research → draft → review stages
- **Quality Standards**: Defining clear criteria for tone, style, completeness, and accuracy

When planning writing tasks:
1. Start with an outline/structure step
2. Add research steps for facts and references if needed
3. Break large content into section drafts
4. Always include a review/polish step at the end"#.to_string();
        persona
    }

    fn execution_persona(&self, step: &PlanStep) -> Persona {
        let mut persona = build_plan_persona(PlanPersonaRole::Executor);
        let title_lower = step.title.to_lowercase();

        if title_lower.contains("outline") || title_lower.contains("structure") {
            persona.identity_prompt = r#"You are an expert content outliner. Create clear, detailed outlines that serve as a solid foundation for writing. Include section headings, key points per section, and suggested flow."#.to_string();
            persona.expert_temperature = 0.6;
        } else if title_lower.contains("review") || title_lower.contains("edit") || title_lower.contains("polish") {
            persona.identity_prompt = r#"You are a skilled editor and content reviewer. Review content for clarity, coherence, grammar, tone consistency, and completeness. Provide specific improvements, not just general feedback."#.to_string();
            persona.expert_temperature = 0.3;
        } else {
            persona.identity_prompt = r#"You are a skilled writer who produces clear, engaging, well-structured content. Write in the appropriate tone and style for the target audience. Ensure completeness and accuracy."#.to_string();
            persona.expert_temperature = 0.7;
        }

        persona
    }

    fn decomposition_prompt(&self, task: &str, context: Option<&str>) -> String {
        let context_section = context
            .map(|c| format!("\n\n## Additional Context\n{}", c))
            .unwrap_or_default();

        format!(
            r#"Decompose the following writing task into a structured plan.

## Task
{task}
{context_section}

## Writing-Specific Instructions
Follow this decomposition pattern:
1. **Outline/Structure** step — define the content structure, headings, and flow
2. **Research** step(s) — gather facts, references, and supporting material (if needed)
3. **Draft** step(s) — write each major section (break large content into parts)
4. **Review & Polish** step — edit for clarity, tone, grammar, and completeness

Ensure the review step depends on all draft steps.

Respond in JSON format:
```json
{{
  "title": "Plan title",
  "description": "Overall plan description",
  "steps": [
    {{
      "id": "step-1",
      "title": "Step title",
      "description": "Detailed description",
      "priority": "high|medium|low",
      "dependencies": [],
      "completionCriteria": ["Criterion 1"],
      "expectedOutput": "Description of expected output"
    }}
  ]
}}
```"#
        )
    }

    fn available_tools(&self, step: &PlanStep) -> Vec<String> {
        let title_lower = step.title.to_lowercase();
        let mut tools = vec![
            "write_file".to_string(),
            "read_file".to_string(),
        ];

        if title_lower.contains("research") || title_lower.contains("gather") {
            tools.push("web_search".to_string());
        }

        tools
    }

    fn after_execution(&self, plan: &Plan, outputs: &[StepOutput]) -> Option<String> {
        // Find the last review/polish step output as the final content
        let final_output = outputs
            .iter()
            .rev()
            .find(|o| {
                plan.steps
                    .iter()
                    .any(|s| s.id == o.step_id && (s.title.to_lowercase().contains("review") || s.title.to_lowercase().contains("polish")))
            })
            .or_else(|| outputs.last());

        match final_output {
            Some(output) => {
                let preview = if output.content.len() > 500 {
                    format!("{}...", &output.content[..500])
                } else {
                    output.content.clone()
                };
                Some(format!(
                    "Writing plan '{}' completed. Final output preview:\n\n{}",
                    plan.title, preview
                ))
            }
            None => Some(format!("Writing plan '{}' completed with no output.", plan.title)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writing_adapter_properties() {
        let adapter = WritingAdapter;
        assert_eq!(adapter.id(), "writing");
        assert_eq!(adapter.display_name(), "Writing & Content");
        assert!(adapter.supported_domains().contains(&TaskDomain::Writing));
    }

    #[test]
    fn test_writing_execution_persona_varies() {
        use std::collections::HashMap;
        use crate::services::plan_mode::types::StepPriority;

        let adapter = WritingAdapter;

        let outline_step = PlanStep {
            id: "s1".to_string(),
            title: "Create Outline".to_string(),
            description: "".to_string(),
            priority: StepPriority::High,
            dependencies: vec![],
            completion_criteria: vec![],
            expected_output: "".to_string(),
            metadata: HashMap::new(),
        };
        let persona = adapter.execution_persona(&outline_step);
        assert!(persona.identity_prompt.contains("outliner"));

        let review_step = PlanStep {
            id: "s2".to_string(),
            title: "Review and Polish".to_string(),
            description: "".to_string(),
            priority: StepPriority::Medium,
            dependencies: vec![],
            completion_criteria: vec![],
            expected_output: "".to_string(),
            metadata: HashMap::new(),
        };
        let persona = adapter.execution_persona(&review_step);
        assert!(persona.identity_prompt.contains("editor"));
    }
}
