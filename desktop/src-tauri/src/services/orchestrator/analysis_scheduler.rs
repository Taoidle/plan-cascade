//! Analysis Phase Planner
//!
//! Produces deterministic per-phase worker plans for repository analysis.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedSubAgent {
    pub sub_agent_id: String,
    pub role: String,
    pub layer_index: usize,
    pub objective: String,
    pub prompt_suffix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedPhase {
    pub phase_id: String,
    pub title: String,
    pub objective: String,
    pub layers: Vec<String>,
    pub workers: Vec<PlannedSubAgent>,
    pub planning_notes: Vec<String>,
}

pub fn build_phase_plan(
    phase_id: &str,
    title: &str,
    objective: &str,
    layers: &[&str],
    request: &str,
    scope_guidance: &str,
    upstream_summary: &str,
) -> PlannedPhase {
    let phase_role = role_prefix(phase_id);
    let upstream = truncate_for_plan(upstream_summary, 1200);
    let request_digest = truncate_for_plan(request, 360);
    let scope_digest = truncate_for_plan(scope_guidance, 360);

    let workers = layers
        .iter()
        .enumerate()
        .map(|(index, layer)| {
            let layer_index = index + 1;
            let role = format!("{}_layer_{}", phase_role, layer_index);
            let sub_agent_id = format!("{}_worker_{}", phase_id, layer_index);
            let objective = layer
                .split_once(':')
                .map(|(_, rhs)| rhs.trim())
                .unwrap_or(layer)
                .to_string();
            let prompt_suffix = format!(
                "Layer {} objective:\n{}\n\nRequest digest:\n{}\n\nScope digest:\n{}\n\nUpstream context:\n{}\n\nOutput requirements:\n\
                 - Return concise verified findings only\n\
                 - Use concrete file paths for claims\n\
                 - Label uncertain points as Unknown",
                layer_index,
                layer,
                request_digest,
                scope_digest,
                upstream
            );
            PlannedSubAgent {
                sub_agent_id,
                role,
                layer_index,
                objective,
                prompt_suffix,
            }
        })
        .collect::<Vec<_>>();

    PlannedPhase {
        phase_id: phase_id.to_string(),
        title: title.to_string(),
        objective: objective.to_string(),
        layers: layers.iter().map(|layer| layer.to_string()).collect(),
        workers,
        planning_notes: vec![
            "Sub-agent plan is deterministic to keep retries reproducible.".to_string(),
            "Workers should consume artifacts from earlier phases rather than rescanning broadly."
                .to_string(),
        ],
    }
}

fn role_prefix(phase_id: &str) -> &'static str {
    match phase_id {
        "structure_discovery" => "explorer",
        "architecture_trace" => "tracer",
        "consistency_check" => "verifier",
        _ => "analyzer",
    }
}

fn truncate_for_plan(text: &str, limit: usize) -> String {
    if text.is_empty() {
        return "(none)".to_string();
    }
    let trimmed = text.trim();
    if trimmed.len() <= limit {
        return trimmed.to_string();
    }
    format!("{}...", &trimmed[..limit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_phase_plan_generates_worker_per_layer() {
        let plan = build_phase_plan(
            "structure_discovery",
            "Structure Discovery",
            "Find root layout",
            &["Layer 1: inventory", "Layer 2: entrypoints"],
            "Analyze this project",
            "Focus on first-party files",
            "None",
        );
        assert_eq!(plan.phase_id, "structure_discovery");
        assert_eq!(plan.workers.len(), 2);
        assert_eq!(plan.workers[0].sub_agent_id, "structure_discovery_worker_1");
        assert!(plan.workers[1].prompt_suffix.contains("Layer 2"));
    }
}
