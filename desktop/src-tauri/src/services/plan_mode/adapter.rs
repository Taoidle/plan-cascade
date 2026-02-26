//! Domain Adapter Trait
//!
//! Defines the interface for pluggable domain adapters in Plan Mode.
//! Each adapter customizes analysis, planning, execution, and validation
//! for a specific task domain (writing, research, general, etc.).

use std::sync::Arc;

use async_trait::async_trait;

use super::types::{
    CriterionResult, Plan, PlanStep, StepOutput, TaskDomain,
};
use crate::services::llm::provider::LlmProvider;
use crate::services::persona::types::{Persona, PersonaRole};

use super::types::PlanPersonaRole;

// ============================================================================
// Default Persona Builders
// ============================================================================

/// Build a Plan Mode persona using PlanPersonaRole but wrapped in the existing Persona struct.
/// We reuse Persona struct from the persona system, but with a synthetic PersonaRole::TechLead
/// as a placeholder since PlanPersonaRole doesn't map to PersonaRole variants.
/// The identity_prompt and thinking_style carry the real persona information.
pub fn build_plan_persona(role: PlanPersonaRole) -> Persona {
    match role {
        PlanPersonaRole::Planner => Persona {
            role: PersonaRole::TechLead, // placeholder — identity_prompt carries real info
            identity_prompt: r#"You are a strategic Planner with expertise in task decomposition and project planning. You excel at:

- **Task Analysis**: Understanding complex tasks and breaking them into manageable, well-defined steps
- **Dependency Mapping**: Identifying which steps depend on others and optimizing parallel execution
- **Scope Definition**: Ensuring each step has clear boundaries, deliverables, and completion criteria
- **Risk Identification**: Spotting potential blockers and suggesting mitigation strategies

You think systematically about:
1. What is the end goal and how do we measure success?
2. What are the natural sub-tasks and their relationships?
3. Which steps can run in parallel vs. must be sequential?
4. What information does each step need from previous steps?"#.to_string(),
            thinking_style: "Strategic and systematic. Decompose before executing. Optimize for parallel execution while respecting dependencies.".to_string(),
            expertise: vec![
                "Task decomposition".to_string(),
                "Dependency analysis".to_string(),
                "Project planning".to_string(),
                "Scope management".to_string(),
            ],
            expert_temperature: 0.7,
            formatter_temperature: 0.1,
        },

        PlanPersonaRole::Analyst => Persona {
            role: PersonaRole::BusinessAnalyst, // closest match
            identity_prompt: r#"You are a perceptive Analyst specializing in requirements clarification. You excel at:

- **Goal Clarification**: Helping users articulate exactly what they want to achieve
- **Assumption Surfacing**: Identifying implicit assumptions that need to be validated
- **Scope Negotiation**: Helping define what's in and out of scope
- **Constraint Discovery**: Uncovering limitations, preferences, and non-obvious requirements

Your approach:
1. Listen carefully to what's being asked
2. Identify gaps and ambiguities in the request
3. Ask targeted, specific questions (not vague ones)
4. Summarize understanding back to confirm alignment"#.to_string(),
            thinking_style: "Curious and thorough. Ask clarifying questions that genuinely reduce ambiguity. Avoid asking obvious questions.".to_string(),
            expertise: vec![
                "Requirements analysis".to_string(),
                "Goal clarification".to_string(),
                "Constraint identification".to_string(),
                "Scope definition".to_string(),
            ],
            expert_temperature: 0.7,
            formatter_temperature: 0.2,
        },

        PlanPersonaRole::Executor => Persona {
            role: PersonaRole::Developer, // closest match
            identity_prompt: r#"You are a capable Executor who completes tasks thoroughly and accurately. You excel at:

- **Task Completion**: Following instructions precisely and delivering complete outputs
- **Tool Usage**: Leveraging available tools (web search, file writing, etc.) effectively
- **Quality Output**: Producing well-structured, clear, and comprehensive results
- **Context Integration**: Using information from previous steps to inform your work

Your approach:
1. Understand the step requirements and completion criteria
2. Review context from previous steps
3. Execute methodically, ensuring all criteria are addressed
4. Produce clear, well-formatted output"#.to_string(),
            thinking_style: "Methodical and thorough. Complete each criterion before moving on. Produce well-structured output.".to_string(),
            expertise: vec![
                "Task execution".to_string(),
                "Content creation".to_string(),
                "Tool usage".to_string(),
                "Quality output".to_string(),
            ],
            expert_temperature: 0.5,
            formatter_temperature: 0.1,
        },

        PlanPersonaRole::Reviewer => Persona {
            role: PersonaRole::QaEngineer, // closest match
            identity_prompt: r#"You are a meticulous Reviewer who validates output quality. You excel at:

- **Criteria Evaluation**: Assessing whether specific completion criteria have been met
- **Quality Assessment**: Judging the overall quality, completeness, and accuracy of outputs
- **Gap Identification**: Finding missing elements, logical errors, or incomplete sections
- **Constructive Feedback**: Providing specific, actionable improvement suggestions

Your approach:
1. Review each completion criterion individually
2. Assess overall quality and coherence
3. Identify any gaps or issues
4. Provide a clear met/not-met verdict with explanation"#.to_string(),
            thinking_style: "Precise and critical. Evaluate each criterion objectively. Provide specific evidence for your assessment.".to_string(),
            expertise: vec![
                "Quality assessment".to_string(),
                "Criteria evaluation".to_string(),
                "Gap analysis".to_string(),
                "Constructive feedback".to_string(),
            ],
            expert_temperature: 0.3,
            formatter_temperature: 0.1,
        },
    }
}

// ============================================================================
// Domain Adapter Trait
// ============================================================================

/// Trait for domain-specific adapters that customize Plan Mode behavior.
#[async_trait]
pub trait DomainAdapter: Send + Sync {
    /// Unique adapter identifier.
    fn id(&self) -> &str;

    /// Human-readable display name.
    fn display_name(&self) -> &str;

    /// Domains this adapter supports.
    fn supported_domains(&self) -> Vec<TaskDomain>;

    // -- Persona customization --

    /// Persona for the analysis phase.
    fn analysis_persona(&self) -> Persona {
        build_plan_persona(PlanPersonaRole::Planner)
    }

    /// Persona for the clarification phase.
    fn clarification_persona(&self) -> Persona {
        build_plan_persona(PlanPersonaRole::Analyst)
    }

    /// Persona for the planning/decomposition phase.
    fn planning_persona(&self) -> Persona {
        build_plan_persona(PlanPersonaRole::Planner)
    }

    /// Persona for step execution (may vary per step).
    fn execution_persona(&self, _step: &PlanStep) -> Persona {
        build_plan_persona(PlanPersonaRole::Executor)
    }

    /// Persona for validation.
    fn validation_persona(&self) -> Persona {
        build_plan_persona(PlanPersonaRole::Reviewer)
    }

    // -- Prompt customization --

    /// System prompt for the clarification phase.
    fn clarification_prompt(&self) -> String {
        r#"You are helping clarify a user's task before creating a plan.

Analyze the task description and identify any ambiguities, missing information, or assumptions that need to be validated.

Generate 1-3 targeted clarification questions. Each question should:
- Address a specific gap in the requirements
- Have a clear purpose (not be vague)
- Include a helpful hint or example

Respond in JSON format:
```json
{
  "questions": [
    {
      "questionId": "q1",
      "question": "What is the target audience for this content?",
      "hint": "e.g., technical professionals, general public, executives",
      "inputType": "text"
    }
  ]
}
```"#.to_string()
    }

    /// System prompt for task decomposition.
    fn decomposition_prompt(&self, task: &str, context: Option<&str>) -> String {
        let context_section = context
            .map(|c| format!("\n\n## Additional Context\n{}", c))
            .unwrap_or_default();

        format!(
            r#"Decompose the following task into a structured plan with concrete steps.

## Task
{task}
{context_section}

## Instructions
1. Break the task into 2-8 concrete steps
2. Each step should be independently completable
3. Define clear dependencies between steps (which steps must complete first)
4. Specify completion criteria for each step
5. Describe the expected output format

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
      "completionCriteria": ["Criterion 1", "Criterion 2"],
      "expectedOutput": "Description of expected output"
    }}
  ]
}}
```"#
        )
    }

    /// System prompt for step execution.
    fn step_execution_prompt(
        &self,
        step: &PlanStep,
        dep_outputs: &[(String, StepOutput)],
        plan: &Plan,
    ) -> String {
        let mut prompt = format!(
            "## Overall Plan Context\n\
             Plan: {}\n\
             Description: {}\n\n\
             ## Your Current Step\n\
             **{}**: {}\n\n\
             ### Completion Criteria\n",
            plan.title, plan.description, step.title, step.description
        );

        for (i, criterion) in step.completion_criteria.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, criterion));
        }

        if !step.expected_output.is_empty() {
            prompt.push_str(&format!(
                "\n### Expected Output\n{}\n",
                step.expected_output
            ));
        }

        if !dep_outputs.is_empty() {
            prompt.push_str("\n## Context from Previous Steps\n");
            for (dep_title, output) in dep_outputs {
                let truncated = truncate_output(&output.content, 4000);
                prompt.push_str(&format!(
                    "\n### Output from: {}\n{}\n",
                    dep_title, truncated
                ));
            }
        }

        prompt
    }

    // -- Tool control --

    /// Available tool names for a given step.
    fn available_tools(&self, _step: &PlanStep) -> Vec<String> {
        vec![
            "web_search".to_string(),
            "read_file".to_string(),
            "write_file".to_string(),
        ]
    }

    // -- Validation --

    /// Validate step output against completion criteria.
    async fn validate_step(
        &self,
        step: &PlanStep,
        output: &StepOutput,
        _provider: Arc<dyn LlmProvider>,
    ) -> Vec<CriterionResult> {
        // Default: mark all criteria as met (LLM validation in concrete adapters)
        step.completion_criteria
            .iter()
            .map(|c| CriterionResult {
                criterion: c.clone(),
                met: true,
                explanation: "Default validation — criteria assumed met".to_string(),
            })
            .collect()
    }

    // -- Lifecycle hooks --

    /// Called before execution begins. Can modify the plan.
    fn before_execution(&self, _plan: &mut Plan) {}

    /// Called after execution completes. Returns optional summary.
    fn after_execution(&self, _plan: &Plan, _outputs: &[StepOutput]) -> Option<String> {
        None
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Truncate output to approximately `max_chars` characters.
fn truncate_output(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        content.to_string()
    } else {
        let truncated = &content[..max_chars];
        format!("{}...\n\n[Output truncated — {} chars total]", truncated, content.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_plan_personas() {
        let planner = build_plan_persona(PlanPersonaRole::Planner);
        assert!(planner.identity_prompt.contains("Planner"));
        assert!(planner.expert_temperature > 0.0);

        let analyst = build_plan_persona(PlanPersonaRole::Analyst);
        assert!(analyst.identity_prompt.contains("Analyst"));

        let executor = build_plan_persona(PlanPersonaRole::Executor);
        assert!(executor.identity_prompt.contains("Executor"));

        let reviewer = build_plan_persona(PlanPersonaRole::Reviewer);
        assert!(reviewer.identity_prompt.contains("Reviewer"));
    }

    #[test]
    fn test_truncate_output() {
        let short = "Hello world";
        assert_eq!(truncate_output(short, 100), "Hello world");

        let long = "A".repeat(200);
        let result = truncate_output(&long, 50);
        assert!(result.contains("[Output truncated"));
        assert!(result.starts_with(&"A".repeat(50)));
    }
}
