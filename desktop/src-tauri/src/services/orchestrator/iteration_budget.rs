use serde::{Deserialize, Serialize};

use super::analysis_index::AnalysisProfile;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionKind {
    StandaloneRoot,
    TaskStory,
    PlanStep,
    AnalysisPhase,
    SubAgentExplore,
    SubAgentPlan,
    SubAgentGeneral,
    SubAgentBash,
    AgentComposerLlmStep,
    AgentComposerLoopStep,
}

impl Default for ExecutionKind {
    fn default() -> Self {
        Self::StandaloneRoot
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct IterationBudget {
    pub soft_limit: u32,
    pub hard_limit: u32,
    pub review_window: u32,
    pub review_interval: u32,
}

#[derive(Debug, Clone, Default)]
pub struct IterationBudgetHints {
    pub prompt_chars: usize,
    pub complexity_score: usize,
    pub has_specialized_tools: bool,
    pub analysis_profile: Option<AnalysisProfile>,
    pub soft_limit_override: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IterationProgressSnapshot {
    pub iteration: u32,
    pub file_change_count: u32,
    pub criteria_met_count: u32,
    pub observed_paths_count: u32,
    pub sampled_reads_count: u32,
    pub candidate_chars: usize,
    pub candidate_deliverable: bool,
    pub successful_tool_results: u32,
    pub unique_tool_fingerprints: u32,
    pub artifact_count: u32,
    pub tool_evidence_count: u32,
    pub subagent_completion_count: u32,
    pub shared_state_fingerprint: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IterationProgressAssessment {
    Progressing,
    Stalled,
}

const REVIEW_WINDOW: u32 = 8;
const REVIEW_INTERVAL: u32 = 8;

pub fn build_iteration_budget(kind: ExecutionKind, hints: &IterationBudgetHints) -> IterationBudget {
    let base = match kind {
        ExecutionKind::StandaloneRoot => 80,
        ExecutionKind::TaskStory => 140,
        ExecutionKind::PlanStep => 96,
        ExecutionKind::AnalysisPhase => 120,
        ExecutionKind::SubAgentExplore => 140,
        ExecutionKind::SubAgentPlan => 120,
        ExecutionKind::SubAgentGeneral => 100,
        ExecutionKind::SubAgentBash => 24,
        ExecutionKind::AgentComposerLlmStep => 80,
        ExecutionKind::AgentComposerLoopStep => 40,
    };
    let cap = match kind {
        ExecutionKind::StandaloneRoot => 160,
        ExecutionKind::TaskStory => 220,
        ExecutionKind::PlanStep => 180,
        ExecutionKind::AnalysisPhase => 220,
        ExecutionKind::SubAgentExplore => 220,
        ExecutionKind::SubAgentPlan => 180,
        ExecutionKind::SubAgentGeneral => 160,
        ExecutionKind::SubAgentBash => 40,
        ExecutionKind::AgentComposerLlmStep => 160,
        ExecutionKind::AgentComposerLoopStep => 80,
    };

    let mut soft_limit = hints.soft_limit_override.unwrap_or(base);
    if hints.soft_limit_override.is_none() {
        if hints.prompt_chars > 16_000 {
            soft_limit += 20;
        }
        if hints.complexity_score >= 8 {
            soft_limit += 20;
        }
        if hints.has_specialized_tools {
            soft_limit += 20;
        }
        if matches!(
            kind,
            ExecutionKind::AnalysisPhase | ExecutionKind::SubAgentExplore | ExecutionKind::SubAgentPlan
        ) && !matches!(hints.analysis_profile, Some(AnalysisProfile::Fast))
        {
            soft_limit += 20;
        }
        soft_limit = soft_limit.clamp(base, cap);
    } else {
        soft_limit = soft_limit.clamp(1, cap.max(1_000));
    }

    IterationBudget {
        soft_limit,
        hard_limit: soft_limit.saturating_mul(5),
        review_window: REVIEW_WINDOW,
        review_interval: REVIEW_INTERVAL,
    }
}

pub fn assess_progress(
    previous: &IterationProgressSnapshot,
    current: &IterationProgressSnapshot,
) -> IterationProgressAssessment {
    if current.file_change_count > previous.file_change_count
        || current.criteria_met_count > previous.criteria_met_count
        || current.observed_paths_count > previous.observed_paths_count
        || current.sampled_reads_count > previous.sampled_reads_count
        || current.shared_state_fingerprint != previous.shared_state_fingerprint
        || (!previous.candidate_deliverable && current.candidate_deliverable)
    {
        return IterationProgressAssessment::Progressing;
    }

    let mut weak_signals = 0u32;
    if current.successful_tool_results > previous.successful_tool_results {
        weak_signals += 1;
    }
    if current.unique_tool_fingerprints > previous.unique_tool_fingerprints {
        weak_signals += 1;
    }
    if current.candidate_chars >= previous.candidate_chars.saturating_add(400) {
        weak_signals += 1;
    }
    if current.artifact_count > previous.artifact_count
        || current.tool_evidence_count > previous.tool_evidence_count
    {
        weak_signals += 1;
    }
    if current.subagent_completion_count > previous.subagent_completion_count {
        weak_signals += 1;
    }

    if weak_signals >= 2 {
        IterationProgressAssessment::Progressing
    } else {
        IterationProgressAssessment::Stalled
    }
}
