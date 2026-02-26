//! Research Adapter
//!
//! Domain adapter for research tasks (market analysis, literature review, etc.).
//! Uses scope→search→analyze→synthesize decomposition pattern.

use async_trait::async_trait;

use crate::services::persona::types::Persona;
use crate::services::plan_mode::adapter::{build_plan_persona, DomainAdapter};
use crate::services::plan_mode::types::{
    Plan, PlanStep, StepOutput, TaskDomain, PlanPersonaRole,
};

/// Research-focused adapter for investigation and analysis tasks.
pub struct ResearchAdapter;

#[async_trait]
impl DomainAdapter for ResearchAdapter {
    fn id(&self) -> &str {
        "research"
    }

    fn display_name(&self) -> &str {
        "Research & Analysis"
    }

    fn supported_domains(&self) -> Vec<TaskDomain> {
        vec![TaskDomain::Research]
    }

    fn planning_persona(&self) -> Persona {
        let mut persona = build_plan_persona(PlanPersonaRole::Planner);
        persona.identity_prompt = r#"You are a research methodology expert. You excel at:

- **Research Design**: Structuring research tasks into systematic phases
- **Source Strategy**: Planning what types of sources to consult and how
- **Analysis Framework**: Defining clear analytical approaches for the gathered data
- **Synthesis Planning**: Organizing findings into coherent, actionable insights

When planning research tasks:
1. Start with a scope/framing step to define research questions
2. Add parallel search/investigation steps for different source types
3. Include an analysis/comparison step to evaluate findings
4. End with a synthesis step that produces actionable conclusions"#.to_string();
        persona
    }

    fn execution_persona(&self, step: &PlanStep) -> Persona {
        let mut persona = build_plan_persona(PlanPersonaRole::Executor);
        let title_lower = step.title.to_lowercase();

        if title_lower.contains("scope") || title_lower.contains("frame") || title_lower.contains("define") {
            persona.identity_prompt = r#"You are a research strategist. Define clear research questions, scope boundaries, and key areas to investigate. Be specific about what to look for and what to exclude."#.to_string();
            persona.expert_temperature = 0.5;
        } else if title_lower.contains("search") || title_lower.contains("gather") || title_lower.contains("investigate") {
            persona.identity_prompt = r#"You are a thorough researcher. Search for relevant information, cite your sources, and capture key findings. Cast a wide net but stay focused on the research questions. Note any conflicting information."#.to_string();
            persona.expert_temperature = 0.4;
        } else if title_lower.contains("analy") || title_lower.contains("compare") || title_lower.contains("evaluate") {
            persona.identity_prompt = r#"You are an analytical researcher. Evaluate the gathered evidence critically. Identify patterns, trends, contradictions, and gaps. Compare different perspectives objectively."#.to_string();
            persona.expert_temperature = 0.3;
        } else if title_lower.contains("synth") || title_lower.contains("conclude") || title_lower.contains("report") {
            persona.identity_prompt = r#"You are a research synthesizer. Combine findings into clear, well-structured conclusions. Present actionable insights supported by evidence. Acknowledge limitations and areas for further research."#.to_string();
            persona.expert_temperature = 0.5;
        }

        persona
    }

    fn decomposition_prompt(&self, task: &str, context: Option<&str>) -> String {
        let context_section = context
            .map(|c| format!("\n\n## Additional Context\n{}", c))
            .unwrap_or_default();

        format!(
            r#"Decompose the following research task into a structured plan.

## Task
{task}
{context_section}

## Research-Specific Instructions
Follow this decomposition pattern:
1. **Scope & Frame** step — define research questions, boundaries, and key areas
2. **Search/Investigate** step(s) — gather information from different sources (can run in parallel)
3. **Analyze & Compare** step — evaluate and cross-reference findings
4. **Synthesize & Conclude** step — produce final insights and recommendations

Make search steps parallel where possible (they should depend on the scope step but not on each other).
The analysis step should depend on all search steps.
The synthesis step should depend on the analysis step.

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

    fn available_tools(&self, _step: &PlanStep) -> Vec<String> {
        // Research tasks need web search heavily
        vec![
            "web_search".to_string(),
            "read_file".to_string(),
            "write_file".to_string(),
        ]
    }

    fn after_execution(&self, plan: &Plan, outputs: &[StepOutput]) -> Option<String> {
        let total_sources: usize = outputs.iter().map(|o| o.artifacts.len()).sum();
        let completed = outputs.len();
        let total = plan.steps.len();

        Some(format!(
            "Research plan '{}' completed: {}/{} steps, {} sources/artifacts collected.",
            plan.title, completed, total, total_sources
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_research_adapter_properties() {
        let adapter = ResearchAdapter;
        assert_eq!(adapter.id(), "research");
        assert_eq!(adapter.display_name(), "Research & Analysis");
        assert!(adapter.supported_domains().contains(&TaskDomain::Research));
    }
}
