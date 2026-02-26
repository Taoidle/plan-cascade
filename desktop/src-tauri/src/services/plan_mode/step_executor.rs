//! Step Executor
//!
//! Phase 5: Execute steps in dependency-resolved batches.
//! Runs steps in parallel within each batch using tokio tasks.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{RwLock, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::services::llm::provider::LlmProvider;
use crate::services::llm::types::{LlmRequestOptions, Message, MessageRole};
use crate::utils::error::{AppError, AppResult};

use super::adapter::DomainAdapter;
use super::types::{
    OutputFormat, Plan, PlanExecutionProgress, PlanModeProgressEvent, StepExecutionState,
    StepOutput, PLAN_MODE_EVENT_CHANNEL,
};

/// Configuration for step execution.
#[derive(Debug, Clone)]
pub struct StepExecutionConfig {
    /// Maximum parallel steps per batch
    pub max_parallel: usize,
    /// Maximum output tokens per context injection (~4000 chars per dep)
    pub max_dep_output_chars: usize,
    /// Total cap for all dependency outputs
    pub max_total_dep_chars: usize,
}

impl Default for StepExecutionConfig {
    fn default() -> Self {
        Self {
            max_parallel: 4,
            max_dep_output_chars: 4000,
            max_total_dep_chars: 16000,
        }
    }
}

/// Execute all steps in a plan according to batch ordering.
pub async fn execute_plan(
    session_id: &str,
    plan: &mut Plan,
    adapter: Arc<dyn DomainAdapter>,
    provider: Arc<dyn LlmProvider>,
    config: StepExecutionConfig,
    language_instruction: String,
    app_handle: tauri::AppHandle,
    cancellation_token: CancellationToken,
) -> AppResult<(HashMap<String, StepOutput>, HashMap<String, StepExecutionState>)> {
    // Lifecycle hook
    adapter.before_execution(plan);

    let step_outputs: Arc<RwLock<HashMap<String, StepOutput>>> =
        Arc::new(RwLock::new(HashMap::new()));
    let step_states: Arc<RwLock<HashMap<String, StepExecutionState>>> =
        Arc::new(RwLock::new(HashMap::new()));

    // Initialize all steps as pending
    {
        let mut states = step_states.write().await;
        for step in &plan.steps {
            states.insert(step.id.clone(), StepExecutionState::Pending);
        }
    }

    let total_batches = plan.batches.len();
    let total_steps = plan.steps.len();
    let semaphore = Arc::new(Semaphore::new(config.max_parallel));

    // Build step lookup
    let step_map: HashMap<String, &super::types::PlanStep> = plan
        .steps
        .iter()
        .map(|s| (s.id.clone(), s))
        .collect();

    for (batch_idx, batch) in plan.batches.iter().enumerate() {
        if cancellation_token.is_cancelled() {
            // Mark remaining as cancelled
            let mut states = step_states.write().await;
            for step in &plan.steps {
                if !states.get(&step.id).map_or(false, |s| s.is_terminal()) {
                    states.insert(step.id.clone(), StepExecutionState::Cancelled);
                }
            }
            emit_event(
                &app_handle,
                PlanModeProgressEvent::execution_cancelled(session_id, batch_idx, total_batches),
            );
            break;
        }

        let completed_so_far = {
            let states = step_states.read().await;
            states
                .values()
                .filter(|s| matches!(s, StepExecutionState::Completed { .. }))
                .count()
        };
        let progress_pct = (completed_so_far as f64 / total_steps as f64) * 100.0;

        emit_event(
            &app_handle,
            PlanModeProgressEvent::batch_started(session_id, batch_idx, total_batches, progress_pct),
        );

        // Execute all steps in this batch in parallel
        let mut handles = Vec::new();

        for step_id in &batch.step_ids {
            let step = match step_map.get(step_id) {
                Some(s) => (*s).clone(),
                None => continue,
            };

            let sem = semaphore.clone();
            let adapter = adapter.clone();
            let provider = provider.clone();
            let outputs = step_outputs.clone();
            let states = step_states.clone();
            let handle = app_handle.clone();
            let session = session_id.to_string();
            let cancel = cancellation_token.clone();
            let plan_clone = plan.clone();
            let cfg = config.clone();
            let lang_inst = language_instruction.clone();
            let batch_index = batch_idx;
            let total_b = total_batches;
            let total_s = total_steps;

            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();

                if cancel.is_cancelled() {
                    let mut s = states.write().await;
                    s.insert(step.id.clone(), StepExecutionState::Cancelled);
                    return;
                }

                // Mark as running
                {
                    let mut s = states.write().await;
                    s.insert(step.id.clone(), StepExecutionState::Running);
                }

                let completed_count = {
                    let s = states.read().await;
                    s.values()
                        .filter(|st| matches!(st, StepExecutionState::Completed { .. }))
                        .count()
                };
                let pct = (completed_count as f64 / total_s as f64) * 100.0;
                emit_event(
                    &handle,
                    PlanModeProgressEvent::step_started(&session, batch_index, total_b, &step.id, pct),
                );

                let start = Instant::now();

                // Gather dependency outputs
                let dep_outputs = {
                    let outs = outputs.read().await;
                    let mut deps = Vec::new();
                    for dep_id in &step.dependencies {
                        if let Some(output) = outs.get(dep_id) {
                            let dep_title = plan_clone
                                .steps
                                .iter()
                                .find(|s| s.id == *dep_id)
                                .map(|s| s.title.clone())
                                .unwrap_or_else(|| dep_id.clone());
                            deps.push((dep_title, output.clone()));
                        }
                    }
                    deps
                };

                // Truncate dependency outputs to fit budget
                let dep_outputs = truncate_dep_outputs(dep_outputs, cfg.max_dep_output_chars, cfg.max_total_dep_chars);

                // Execute the step
                match execute_single_step(&step, &dep_outputs, &plan_clone, adapter.as_ref(), provider.as_ref(), &lang_inst).await {
                    Ok(output) => {
                        let duration_ms = start.elapsed().as_millis() as u64;
                        {
                            let mut s = states.write().await;
                            s.insert(step.id.clone(), StepExecutionState::Completed { duration_ms });
                        }
                        {
                            let mut outs = outputs.write().await;
                            outs.insert(step.id.clone(), output);
                        }

                        let completed_count = {
                            let s = states.read().await;
                            s.values()
                                .filter(|st| matches!(st, StepExecutionState::Completed { .. }))
                                .count()
                        };
                        let pct = (completed_count as f64 / total_s as f64) * 100.0;
                        emit_event(
                            &handle,
                            PlanModeProgressEvent::step_completed(&session, batch_index, total_b, &step.id, pct),
                        );
                    }
                    Err(e) => {
                        let reason = format!("{e}");
                        {
                            let mut s = states.write().await;
                            s.insert(step.id.clone(), StepExecutionState::Failed { reason: reason.clone() });
                        }

                        let completed_count = {
                            let s = states.read().await;
                            s.values()
                                .filter(|st| matches!(st, StepExecutionState::Completed { .. }))
                                .count()
                        };
                        let pct = (completed_count as f64 / total_s as f64) * 100.0;
                        emit_event(
                            &handle,
                            PlanModeProgressEvent::step_failed(&session, batch_index, total_b, &step.id, &reason, pct),
                        );
                    }
                }
            });

            handles.push(task);
        }

        // Wait for all tasks in this batch
        for handle in handles {
            let _ = handle.await;
        }
    }

    // Final event
    if !cancellation_token.is_cancelled() {
        emit_event(
            &app_handle,
            PlanModeProgressEvent::execution_completed(session_id, total_batches, 100.0),
        );
    }

    let final_outputs = step_outputs.read().await.clone();
    let final_states = step_states.read().await.clone();

    Ok((final_outputs, final_states))
}

/// Build the current progress from step states.
pub fn build_progress(
    step_states: &HashMap<String, StepExecutionState>,
    current_batch: usize,
    total_batches: usize,
    total_steps: usize,
) -> PlanExecutionProgress {
    let steps_completed = step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::Completed { .. }))
        .count();
    let steps_failed = step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::Failed { .. }))
        .count();

    PlanExecutionProgress {
        current_batch,
        total_batches,
        steps_completed,
        steps_failed,
        total_steps,
        progress_pct: if total_steps > 0 {
            (steps_completed as f64 / total_steps as f64) * 100.0
        } else {
            0.0
        },
    }
}

/// Execute a single step using the adapter and LLM provider.
async fn execute_single_step(
    step: &super::types::PlanStep,
    dep_outputs: &[(String, StepOutput)],
    plan: &Plan,
    adapter: &dyn DomainAdapter,
    provider: &dyn LlmProvider,
    language_instruction: &str,
) -> AppResult<StepOutput> {
    let persona = adapter.execution_persona(step);

    let system = format!(
        "{}\n\n{}\n\n## Output Language\n{}",
        persona.identity_prompt, persona.thinking_style, language_instruction
    );

    let user_prompt = adapter.step_execution_prompt(step, dep_outputs, plan);
    let full_prompt = format!(
        "{user_prompt}\n\n---\nExecute this step and produce the required output. \
         Address all completion criteria."
    );

    let messages = vec![Message::text(MessageRole::User, full_prompt)];

    let options = LlmRequestOptions {
        temperature_override: Some(persona.expert_temperature),
        ..Default::default()
    };

    let response = provider
        .send_message(messages, Some(system), vec![], options)
        .await
        .map_err(|e| AppError::Internal(format!("Step execution LLM error: {e}")))?;

    let content = response
        .content
        .unwrap_or_else(|| "No output produced".to_string());

    Ok(StepOutput {
        step_id: step.id.clone(),
        content,
        format: OutputFormat::Markdown,
        criteria_met: vec![], // Populated during validation phase
        artifacts: vec![],
    })
}

/// Truncate dependency outputs to fit within budget.
fn truncate_dep_outputs(
    deps: Vec<(String, StepOutput)>,
    max_per_dep: usize,
    max_total: usize,
) -> Vec<(String, StepOutput)> {
    let mut total_chars = 0;
    let mut result = Vec::new();

    for (title, mut output) in deps {
        if total_chars >= max_total {
            break;
        }

        let remaining = max_total - total_chars;
        let limit = remaining.min(max_per_dep);

        if output.content.len() > limit {
            output.content = format!(
                "{}...\n[Truncated — {} chars total]",
                &output.content[..limit],
                output.content.len()
            );
        }

        total_chars += output.content.len();
        result.push((title, output));
    }

    result
}

/// Emit a plan mode progress event.
fn emit_event(handle: &tauri::AppHandle, event: PlanModeProgressEvent) {
    use tauri::Emitter;
    let _ = handle.emit(PLAN_MODE_EVENT_CHANNEL, &event);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_dep_outputs() {
        let deps = vec![
            (
                "Step 1".to_string(),
                StepOutput {
                    step_id: "s1".to_string(),
                    content: "A".repeat(5000),
                    format: OutputFormat::Text,
                    criteria_met: vec![],
                    artifacts: vec![],
                },
            ),
            (
                "Step 2".to_string(),
                StepOutput {
                    step_id: "s2".to_string(),
                    content: "B".repeat(3000),
                    format: OutputFormat::Text,
                    criteria_met: vec![],
                    artifacts: vec![],
                },
            ),
        ];

        let result = truncate_dep_outputs(deps, 4000, 6000);
        assert_eq!(result.len(), 2);
        assert!(result[0].1.content.len() <= 4100); // 4000 + truncation message
        assert!(result[1].1.content.len() <= 2100); // Remaining budget
    }

    #[test]
    fn test_build_progress() {
        let mut states = HashMap::new();
        states.insert("s1".to_string(), StepExecutionState::Completed { duration_ms: 100 });
        states.insert("s2".to_string(), StepExecutionState::Failed { reason: "err".to_string() });
        states.insert("s3".to_string(), StepExecutionState::Pending);

        let progress = build_progress(&states, 1, 2, 3);
        assert_eq!(progress.steps_completed, 1);
        assert_eq!(progress.steps_failed, 1);
        assert_eq!(progress.total_steps, 3);
        assert!((progress.progress_pct - 33.33).abs() < 1.0);
    }
}
