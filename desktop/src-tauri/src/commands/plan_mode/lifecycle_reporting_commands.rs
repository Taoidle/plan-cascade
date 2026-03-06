use super::*;

fn truncate_with_ellipsis(content: &str, max_chars: usize) -> String {
    let mut chars = content.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        content.to_string()
    }
}

fn synthesize_terminal_conclusion(
    plan_title: &str,
    terminal_state: &str,
    steps_completed: usize,
    steps_cancelled: usize,
    total_steps: usize,
    step_summaries: &HashMap<String, String>,
    failure_reasons: &HashMap<String, String>,
) -> String {
    let mut lines = vec![
        format!("# {} — Execution Summary", plan_title),
        format!(
            "- Terminal state: `{}`\n- Steps completed: {}/{}\n- Steps cancelled: {}",
            terminal_state, steps_completed, total_steps, steps_cancelled
        ),
    ];

    if !step_summaries.is_empty() {
        lines.push("## Step outcomes".to_string());
        let mut ids: Vec<_> = step_summaries.keys().cloned().collect();
        ids.sort();
        for step_id in ids.into_iter().take(6) {
            if let Some(summary) = step_summaries.get(&step_id) {
                lines.push(format!("- {}: {}", step_id, summary));
            }
        }
    }

    if !failure_reasons.is_empty() {
        lines.push("## Failures".to_string());
        let mut ids: Vec<_> = failure_reasons.keys().cloned().collect();
        ids.sort();
        for step_id in ids {
            if let Some(reason) = failure_reasons.get(&step_id) {
                lines.push(format!("- {}: {}", step_id, reason));
            }
        }
    } else if terminal_state == "cancelled" {
        lines.push("Execution was cancelled before all planned steps finished.".to_string());
    }

    lines.join("\n")
}

fn compute_retry_stats_from_session(
    step_attempts: &HashMap<String, usize>,
    step_states: &HashMap<String, StepExecutionState>,
) -> PlanRetryStats {
    let mut total_retries = 0usize;
    let mut steps_retried = 0usize;
    for attempts in step_attempts.values() {
        if *attempts > 1 {
            steps_retried += 1;
            total_retries += attempts.saturating_sub(1);
        }
    }

    let exhausted_failures = step_states
        .iter()
        .filter(|(step_id, state)| {
            matches!(state, StepExecutionState::Failed { .. })
                && step_attempts
                    .get(step_id.as_str())
                    .map(|attempts| *attempts > 1)
                    .unwrap_or(false)
        })
        .count();

    PlanRetryStats {
        total_retries,
        steps_retried,
        exhausted_failures,
    }
}

/// Get current execution status.
#[tauri::command]
pub async fn get_plan_execution_status(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<PlanExecutionStatusResponse>, String> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    let total_steps = session.plan.as_ref().map_or(0, |p| p.steps.len());
    let total_batches = session.plan.as_ref().map_or(0, |p| p.batches.len());

    let steps_completed = session
        .step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::Completed { .. }))
        .count();
    let steps_failed = session
        .step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::Failed { .. }))
        .count();

    Ok(CommandResponse::ok(PlanExecutionStatusResponse {
        session_id: session.session_id.clone(),
        phase: session.phase,
        total_steps,
        steps_completed,
        steps_failed,
        total_batches,
        progress_pct: if total_steps > 0 {
            (steps_completed as f64 / total_steps as f64) * 100.0
        } else {
            0.0
        },
    }))
}

/// Cancel plan execution.
#[tauri::command]
pub async fn cancel_plan_execution(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<bool>, String> {
    // Verify session
    {
        let sessions = state.sessions.read().await;
        let session = sessions.get(&session_id);
        if session.is_none() {
            return Ok(CommandResponse::err("No active plan mode session"));
        }
    }

    // Cancel via token
    let ct_guard = state.cancellation_tokens.read().await;
    if let Some(token) = ct_guard.get(&session_id) {
        token.cancel();
    } else {
        return Ok(CommandResponse::err("No execution in progress to cancel"));
    }

    Ok(CommandResponse::ok(true))
}

/// Cancel a running plan pre-execution operation (analysis/clarification/planning).
#[tauri::command]
pub async fn cancel_plan_operation(
    session_id: Option<String>,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<bool>, String> {
    let mut cancelled_any = false;
    let tokens = state.operation_cancellation_tokens.read().await;

    match session_id {
        Some(sid) => {
            if let Some((_, token)) = tokens.get(&sid) {
                token.cancel();
                cancelled_any = true;
            }
        }
        None => {
            for (_, token) in tokens.values() {
                token.cancel();
                cancelled_any = true;
            }
        }
    }

    if cancelled_any {
        Ok(CommandResponse::ok(true))
    } else {
        Ok(CommandResponse::err(
            "No plan operation in progress to cancel",
        ))
    }
}

/// Get the final execution report.
#[tauri::command]
pub async fn get_plan_execution_report(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<PlanExecutionReport>, String> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    let plan = session
        .plan
        .as_ref()
        .ok_or_else(|| "No plan generated".to_string())?;

    let total_steps = plan.steps.len();
    let steps_completed = session
        .step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::Completed { .. }))
        .count();
    let steps_failed = session
        .step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::Failed { .. }))
        .count();
    let steps_cancelled = session
        .step_states
        .values()
        .filter(|s| matches!(s, StepExecutionState::Cancelled))
        .count();
    let steps_attempted = steps_completed + steps_failed + steps_cancelled;

    let total_duration_ms: u64 = session
        .step_states
        .values()
        .filter_map(|s| match s {
            StepExecutionState::Completed { duration_ms } => Some(*duration_ms),
            _ => None,
        })
        .sum();

    let step_summaries: HashMap<String, String> = session
        .step_outputs
        .iter()
        .map(|(id, output)| {
            let summary = if output.summary.trim().is_empty() {
                let source = if output.full_content.trim().is_empty() {
                    output.content.as_str()
                } else {
                    output.full_content.as_str()
                };
                truncate_with_ellipsis(source, 200)
            } else {
                truncate_with_ellipsis(&output.summary, 200)
            };
            (id.clone(), summary)
        })
        .collect();

    let failure_reasons: HashMap<String, String> = session
        .step_states
        .iter()
        .filter_map(|(id, state)| match state {
            StepExecutionState::Failed { reason } => Some((id.clone(), reason.clone())),
            _ => None,
        })
        .collect();

    let terminal_state = match session.phase {
        PlanModePhase::Completed => "completed",
        PlanModePhase::Cancelled => "cancelled",
        PlanModePhase::Failed => "failed",
        _ if steps_failed > 0 => "failed",
        _ if steps_completed == total_steps => "completed",
        _ => "failed",
    }
    .to_string();

    let cancelled_by = if terminal_state == "cancelled" {
        Some("user".to_string())
    } else {
        None
    };
    let final_conclusion_markdown = synthesize_terminal_conclusion(
        &plan.title,
        &terminal_state,
        steps_completed,
        steps_cancelled,
        total_steps,
        &step_summaries,
        &failure_reasons,
    );
    let mut highlights: Vec<String> = plan
        .steps
        .iter()
        .filter_map(|step| {
            step_summaries
                .get(step.id.as_str())
                .map(|summary| format!("{}: {}", step.title, summary))
        })
        .take(5)
        .collect();
    if highlights.is_empty() {
        highlights.push("No completed step summaries available.".to_string());
    }
    let next_actions = if terminal_state == "completed" {
        vec![
            "Review generated artifacts and publish final deliverables.".to_string(),
            "Run domain-specific verification on outputs before handoff.".to_string(),
        ]
    } else if terminal_state == "cancelled" {
        vec![
            "Resume from the latest completed steps after addressing cancellation causes."
                .to_string(),
            "Re-run blocked steps first to avoid repeating completed work.".to_string(),
        ]
    } else {
        vec![
            "Inspect failed steps and retry after fixing blocking issues.".to_string(),
            "Do not continue to dependent batches until blocking steps are resolved.".to_string(),
        ]
    };
    let retry_stats =
        compute_retry_stats_from_session(&session.step_attempts, &session.step_states);
    let is_cancelled_terminal = terminal_state == "cancelled";

    Ok(CommandResponse::ok(PlanExecutionReport {
        session_id: session.session_id.clone(),
        plan_title: plan.title.clone(),
        success: steps_failed == 0 && steps_completed == total_steps,
        terminal_state,
        total_steps,
        steps_completed,
        steps_failed,
        steps_cancelled,
        steps_attempted,
        steps_failed_before_cancel: if is_cancelled_terminal {
            steps_failed
        } else {
            0
        },
        total_duration_ms,
        step_summaries,
        failure_reasons,
        cancelled_by,
        run_id: format!("report-{}", session.session_id),
        final_conclusion_markdown,
        highlights,
        next_actions,
        retry_stats,
    }))
}

/// Get a single step's output.
#[tauri::command]
pub async fn get_step_output(
    session_id: String,
    step_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<StepOutput>, String> {
    let sessions = state.sessions.read().await;
    let session = sessions
        .get(&session_id)
        .ok_or_else(|| "No active plan mode session".to_string())?;

    match session.step_outputs.get(&step_id) {
        Some(output) => Ok(CommandResponse::ok(output.clone())),
        None => Ok(CommandResponse::err(format!(
            "No output for step '{}'",
            step_id
        ))),
    }
}

/// Exit plan mode and clean up.
#[tauri::command]
pub async fn exit_plan_mode(
    session_id: String,
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<bool>, String> {
    let removed_session = {
        let mut sessions = state.sessions.write().await;
        sessions.remove(&session_id).is_some()
    };
    let _ = state.delete_persisted_session(&session_id).await;
    if !removed_session {
        return Ok(CommandResponse::err("No active plan mode session"));
    }

    let removed_token = {
        let mut tokens = state.cancellation_tokens.write().await;
        tokens.remove(&session_id)
    };
    if let Some(token) = removed_token {
        token.cancel();
    }

    let removed_operation_token = {
        let mut tokens = state.operation_cancellation_tokens.write().await;
        tokens.remove(&session_id)
    };
    if let Some((_, token)) = removed_operation_token {
        token.cancel();
    }

    Ok(CommandResponse::ok(true))
}

/// List available domain adapters.
#[tauri::command]
pub async fn list_plan_adapters(
    state: tauri::State<'_, PlanModeState>,
) -> Result<CommandResponse<Vec<AdapterInfo>>, String> {
    let registry = state.adapter_registry.read().await;
    Ok(CommandResponse::ok(registry.list()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_retry_stats_counts_attempts_and_exhausted_failures() {
        let mut states = HashMap::new();
        states.insert(
            "step-1".to_string(),
            StepExecutionState::Completed { duration_ms: 100 },
        );
        states.insert(
            "step-2".to_string(),
            StepExecutionState::Failed {
                reason: "still failed".to_string(),
            },
        );
        states.insert(
            "step-3".to_string(),
            StepExecutionState::Failed {
                reason: "one shot".to_string(),
            },
        );

        let mut attempts = HashMap::new();
        attempts.insert("step-1".to_string(), 3usize);
        attempts.insert("step-2".to_string(), 2usize);
        attempts.insert("step-3".to_string(), 1usize);

        let stats = compute_retry_stats_from_session(&attempts, &states);
        assert_eq!(stats.total_retries, 3);
        assert_eq!(stats.steps_retried, 2);
        assert_eq!(stats.exhausted_failures, 1);
    }
}
