import type { DebugModeSession } from '../types/debugMode';
import type {
  QualityDecision,
  QualityDecisionAction,
  QualityBehavior,
  QualityGateOutcome,
  QualityGateStatus,
  QualitySeverity,
  QualityRunSnapshot,
  ModeQualitySnapshot,
  QualitySettings,
} from '../types/workflowQuality';
import type { WorkflowMode } from '../types/workflowKernel';
import type { StepOutputData, StepValidationResultData } from '../types/planModeCard';
import type { DimensionScore, StoryQualityGateResults } from '../store/taskMode';
import { DEFAULT_MODE_QUALITY_SNAPSHOT, resolveModeQualityConfig } from '../types/workflowQuality';

function nowIso(): string {
  return new Date().toISOString();
}

function toGateStatus(value: string | undefined): QualityGateStatus {
  switch (value) {
    case 'pending':
    case 'running':
    case 'passed':
    case 'failed':
    case 'warning':
    case 'skipped':
      return value;
    default:
      return 'warning';
  }
}

function normalizeTaskGateStatus(
  status: string | undefined,
  gateId: string,
  message: string | undefined,
): QualityGateStatus {
  if (status === 'skipped' && gateId !== 'quality_gates_disabled' && message) {
    return 'warning';
  }
  return toGateStatus(status);
}

function toSeverity(status: QualityGateStatus, blocking: boolean): QualitySeverity {
  if (status === 'failed') return blocking ? 'hard_fail' : 'soft_fail';
  if (status === 'warning') return 'warning';
  return 'info';
}

function filterOutcomes(outcomes: QualityGateOutcome[], selectedGateIds: string[]): QualityGateOutcome[] {
  const outcomeMap = new Map(outcomes.map((outcome) => [outcome.gateId, outcome]));
  return selectedGateIds
    .map((gateId) => outcomeMap.get(gateId))
    .filter((outcome): outcome is QualityGateOutcome => !!outcome);
}

function summarizeRun(
  run: QualityRunSnapshot,
  behavior: QualityBehavior,
  requireApprovalOnRisk: boolean = false,
): QualityRunSnapshot {
  const blockingStatus: 'passed' | 'failed' = run.outcomes.some(
    (outcome) => outcome.blocking && outcome.status === 'failed',
  )
    ? 'failed'
    : 'passed';
  const requiresApproval =
    requireApprovalOnRisk &&
    run.outcomes.some(
      (outcome) => outcome.gateId === 'patch_safety' && outcome.blocking && outcome.status === 'failed',
    );
  return {
    ...run,
    status:
      blockingStatus === 'failed'
        ? 'failed'
        : run.outcomes.some((outcome) => outcome.status === 'warning')
          ? 'warning'
          : 'passed',
    blockingStatus,
    decision: requiresApproval ? 'approval_required' : buildDecision(blockingStatus, run.mode, behavior),
    recommendedAction: requiresApproval ? 'approve_and_continue' : buildAction(blockingStatus, run.mode, behavior),
    retryable: blockingStatus === 'failed',
  };
}

function withRun(snapshot: ModeQualitySnapshot | null | undefined, run: QualityRunSnapshot): ModeQualitySnapshot {
  const base =
    snapshot ??
    ({
      ...DEFAULT_MODE_QUALITY_SNAPSHOT,
      profileId: run.mode,
      defaultBehavior: run.mode === 'task' ? 'auto_retry_if_retryable' : 'manual_review',
    } satisfies ModeQualitySnapshot);
  const runs = [...(base.runs ?? [])];
  const existingIndex = runs.findIndex((item) => item.runId === run.runId);
  if (existingIndex >= 0) {
    runs[existingIndex] = run;
  } else {
    runs.unshift(run);
  }
  return {
    ...base,
    profileId: run.mode,
    activeRunId: null,
    lastDecision: run.decision,
    updatedAt: run.completedAt ?? run.startedAt,
    runs,
  };
}

function buildDecision(
  blockingStatus: 'passed' | 'failed',
  mode: WorkflowMode,
  behavior: QualityBehavior,
): QualityDecision {
  if (blockingStatus === 'passed') return 'pass';
  if (mode === 'task') return 'retry';
  if (behavior === 'auto_retry_if_retryable') return 'retry';
  if (behavior === 'warn_and_continue') return 'warn';
  return 'needs_review';
}

function buildAction(
  blockingStatus: 'passed' | 'failed',
  mode: WorkflowMode,
  behavior: QualityBehavior,
): QualityDecisionAction | null {
  if (blockingStatus === 'passed') return null;
  if (mode === 'task') return 'retry';
  if (behavior === 'auto_retry_if_retryable') return 'retry';
  return 'approve_and_continue';
}

export function buildTaskQualitySnapshot(
  previous: ModeQualitySnapshot | null | undefined,
  result: StoryQualityGateResults,
  codeReviewScores: DimensionScore[] = [],
  qualitySettings?: QualitySettings | null,
): ModeQualitySnapshot {
  const modeConfig = resolveModeQualityConfig(qualitySettings, 'task');
  const customGateIds = new Set(
    (qualitySettings?.customGates ?? []).filter((gate) => gate.modes.includes('task')).map((gate) => gate.id),
  );
  const allOutcomes: QualityGateOutcome[] = result.gates.map((gate) => {
    const blocking = gate.status === 'failed';
    const status = normalizeTaskGateStatus(gate.status, gate.gateId, gate.message);
    return {
      gateId: gate.gateId,
      gateName: gate.gateName,
      status,
      severity: toSeverity(status, blocking),
      blocking,
      source: customGateIds.has(gate.gateId)
        ? 'custom'
        : result.gateSource === 'llm'
          ? 'llm'
          : result.gateSource === 'fallback_heuristic'
            ? 'fallback_heuristic'
            : 'builtin',
      message: gate.message ?? null,
      durationMs: gate.duration ?? null,
      retryable: gate.status === 'failed',
      metadata: codeReviewScores.length > 0 ? { codeReviewScores } : null,
    };
  });
  const outcomes = filterOutcomes(allOutcomes, modeConfig.selectedGateIds);
  const blockingStatus: 'passed' | 'failed' = outcomes.some(
    (outcome) => outcome.blocking && outcome.status === 'failed',
  )
    ? 'failed'
    : 'passed';

  const decision = buildDecision(blockingStatus, 'task', modeConfig.behavior);
  const run: QualityRunSnapshot = {
    runId: `task:${result.storyId}`,
    mode: 'task',
    scope: 'story',
    scopeId: result.storyId,
    trigger: 'post_execution',
    status:
      blockingStatus === 'failed'
        ? 'failed'
        : outcomes.some((outcome) => outcome.status === 'warning')
          ? 'warning'
          : 'passed',
    decision,
    recommendedAction: buildAction(blockingStatus, 'task', modeConfig.behavior),
    retryable: blockingStatus === 'failed',
    blockingStatus,
    startedAt: previous?.updatedAt ?? nowIso(),
    completedAt: nowIso(),
    outcomes,
    summary: `${result.storyId}: ${result.overallStatus}`,
  };

  return withRun(previous, run);
}

export function appendQualityOutcomes(
  previous: ModeQualitySnapshot | null | undefined,
  snapshot: ModeQualitySnapshot,
  additionalOutcomes: QualityGateOutcome[],
  behavior: QualityBehavior,
  requireApprovalOnRisk: boolean = false,
): ModeQualitySnapshot {
  if (additionalOutcomes.length === 0 || snapshot.runs.length === 0) {
    return snapshot;
  }
  const [latestRun] = snapshot.runs;
  const mergedRun = summarizeRun(
    {
      ...latestRun,
      outcomes: [...latestRun.outcomes, ...additionalOutcomes],
      completedAt: latestRun.completedAt ?? nowIso(),
    },
    behavior,
    requireApprovalOnRisk,
  );
  return withRun(previous, mergedRun);
}

function hasValidationFailures(validationResult: StepValidationResultData | undefined): boolean {
  return (validationResult?.checks ?? []).some((check) => check.passed === false);
}

export function buildPlanQualitySnapshot(
  previous: ModeQualitySnapshot | null | undefined,
  stepId: string,
  stepTitle: string,
  output: StepOutputData,
  qualitySettings?: QualitySettings | null,
): ModeQualitySnapshot {
  const modeConfig = resolveModeQualityConfig(qualitySettings, 'plan');
  const validationFailed = hasValidationFailures(output.validationResult);
  const criteriaFailed = (output.criteriaMet ?? []).some((criterion) => !criterion.met);
  const hasContent = Boolean(output.summary?.trim() || output.content?.trim() || output.fullContent?.trim());
  const artifactCount = output.artifacts?.length ?? output.evidenceSummary?.artifactCount ?? 0;
  const artifactIntegrityFailed = !hasContent && artifactCount === 0;
  const artifactIntegrityWarning = !artifactIntegrityFailed && artifactCount === 0;
  const allOutcomes: QualityGateOutcome[] = [
    {
      gateId: 'output_completeness',
      gateName: 'Output Completeness',
      status: output.qualityState === 'incomplete' ? 'failed' : 'passed',
      severity: output.qualityState === 'incomplete' ? 'hard_fail' : 'info',
      blocking: output.qualityState === 'incomplete',
      source: 'derived',
      message: output.incompleteReason ?? output.summary ?? null,
      retryable: output.qualityState === 'incomplete',
    },
    {
      gateId: 'criteria_validation',
      gateName: 'Criteria Validation',
      status: criteriaFailed || validationFailed ? 'failed' : 'passed',
      severity: criteriaFailed || validationFailed ? 'hard_fail' : 'info',
      blocking: criteriaFailed || validationFailed,
      source: 'derived',
      message: output.validationResult?.summary ?? null,
      retryable: criteriaFailed || validationFailed,
    },
    {
      gateId: 'evidence_sufficiency',
      gateName: 'Evidence Sufficiency',
      status: (output.toolEvidence?.length ?? 0) > 0 ? 'passed' : 'warning',
      severity: (output.toolEvidence?.length ?? 0) > 0 ? 'info' : 'warning',
      blocking: false,
      source: 'derived',
      message: output.reviewReason ?? null,
      retryable: false,
    },
    {
      gateId: 'artifact_integrity',
      gateName: 'Artifact Integrity',
      status: artifactIntegrityFailed ? 'failed' : artifactIntegrityWarning ? 'warning' : 'passed',
      severity: artifactIntegrityFailed ? 'hard_fail' : artifactIntegrityWarning ? 'warning' : 'info',
      blocking: artifactIntegrityFailed,
      source: 'derived',
      message: artifactIntegrityFailed
        ? (output.incompleteReason ?? output.reviewReason ?? 'No durable output artifact or content was produced.')
        : artifactIntegrityWarning
          ? 'Step completed without attached artifacts.'
          : null,
      retryable: artifactIntegrityFailed,
    },
  ];
  const outcomes = filterOutcomes(allOutcomes, modeConfig.selectedGateIds);

  const blockingStatus: 'passed' | 'failed' = outcomes.some(
    (outcome) => outcome.blocking && outcome.status === 'failed',
  )
    ? 'failed'
    : 'passed';
  const run: QualityRunSnapshot = {
    runId: `plan:${stepId}`,
    mode: 'plan',
    scope: 'step',
    scopeId: stepId,
    trigger: 'post_execution',
    status:
      blockingStatus === 'failed'
        ? 'failed'
        : outcomes.some((outcome) => outcome.status === 'warning')
          ? 'warning'
          : 'passed',
    decision: buildDecision(blockingStatus, 'plan', modeConfig.behavior),
    recommendedAction: buildAction(blockingStatus, 'plan', modeConfig.behavior),
    retryable: blockingStatus === 'failed',
    blockingStatus,
    startedAt: previous?.updatedAt ?? nowIso(),
    completedAt: nowIso(),
    outcomes,
    summary: `${stepTitle}: ${output.outcomeStatus ?? output.qualityState ?? 'completed'}`,
  };
  return withRun(previous, run);
}

export function buildDebugQualitySnapshot(
  previous: ModeQualitySnapshot | null | undefined,
  session: DebugModeSession,
  qualitySettings?: QualitySettings | null,
): ModeQualitySnapshot {
  const modeConfig = resolveModeQualityConfig(qualitySettings, 'debug');
  const state = session.state;
  const evidenceSufficient = state.evidenceRefs.length > 0;
  const verificationPassed = (state.verificationReport?.checks ?? []).every((check) => check.status !== 'failed');
  const requiresApproval = !!state.pendingApproval;
  const reproConfidence = state.selectedRootCause?.confidence ?? 0;
  const hasReproContext = state.reproSteps.length > 0 || !!state.targetUrlOrEntry;
  const regressionRiskLevel = state.fixProposal?.riskLevel ?? 'low';
  const allOutcomes: QualityGateOutcome[] = [
    {
      gateId: 'evidence_sufficiency',
      gateName: 'Evidence Sufficiency',
      status: evidenceSufficient ? 'passed' : 'warning',
      severity: evidenceSufficient ? 'info' : 'warning',
      blocking: false,
      source: 'derived',
      message: state.pendingApproval?.description ?? null,
    },
    {
      gateId: 'repro_confidence',
      gateName: 'Reproduction Confidence',
      status: !hasReproContext ? 'warning' : reproConfidence >= 0.7 ? 'passed' : 'warning',
      severity: !hasReproContext || reproConfidence < 0.7 ? 'warning' : 'info',
      blocking: false,
      source: 'derived',
      message: state.selectedRootCause
        ? `Root cause confidence ${(reproConfidence * 100).toFixed(0)}%.`
        : 'No confirmed root cause confidence is available yet.',
    },
    {
      gateId: 'patch_safety',
      gateName: 'Patch Safety',
      status: requiresApproval ? 'failed' : 'passed',
      severity: requiresApproval ? 'hard_fail' : 'info',
      blocking: requiresApproval,
      source: 'derived',
      message: state.pendingApproval?.description ?? null,
      retryable: requiresApproval,
    },
    {
      gateId: 'verification_success',
      gateName: 'Verification Success',
      status: state.verificationReport ? (verificationPassed ? 'passed' : 'failed') : 'warning',
      severity: !state.verificationReport ? 'warning' : verificationPassed ? 'info' : 'hard_fail',
      blocking: !!state.verificationReport && !verificationPassed,
      source: 'derived',
      message: state.verificationReport?.summary ?? null,
      retryable: !!state.verificationReport && !verificationPassed,
    },
    {
      gateId: 'regression_risk',
      gateName: 'Regression Risk',
      status:
        regressionRiskLevel === 'critical'
          ? 'failed'
          : regressionRiskLevel === 'high' || (state.verificationReport?.residualRisks.length ?? 0) > 0
            ? 'warning'
            : 'passed',
      severity:
        regressionRiskLevel === 'critical'
          ? 'hard_fail'
          : regressionRiskLevel === 'high' || (state.verificationReport?.residualRisks.length ?? 0) > 0
            ? 'warning'
            : 'info',
      blocking: regressionRiskLevel === 'critical',
      source: 'derived',
      message:
        state.verificationReport?.residualRisks.join(' ') ||
        (state.fixProposal ? `Patch risk level: ${state.fixProposal.riskLevel}.` : null),
      retryable: regressionRiskLevel === 'critical',
    },
  ];
  const outcomes = filterOutcomes(allOutcomes, modeConfig.selectedGateIds);

  const blockingStatus: 'passed' | 'failed' = outcomes.some(
    (outcome) => outcome.blocking && outcome.status === 'failed',
  )
    ? 'failed'
    : 'passed';
  const run: QualityRunSnapshot = {
    runId: `debug:${session.sessionId}:${state.phase}`,
    mode: 'debug',
    scope: 'patch',
    scopeId: session.sessionId,
    trigger: 'post_execution',
    status:
      blockingStatus === 'failed'
        ? 'failed'
        : outcomes.some((outcome) => outcome.status === 'warning')
          ? 'warning'
          : 'passed',
    decision:
      requiresApproval && modeConfig.requireApprovalOnRisk && modeConfig.selectedGateIds.includes('patch_safety')
        ? 'approval_required'
        : buildDecision(blockingStatus, 'debug', modeConfig.behavior),
    recommendedAction:
      requiresApproval && modeConfig.requireApprovalOnRisk && modeConfig.selectedGateIds.includes('patch_safety')
        ? 'approve_and_continue'
        : buildAction(blockingStatus, 'debug', modeConfig.behavior),
    retryable: blockingStatus === 'failed',
    blockingStatus,
    startedAt: previous?.updatedAt ?? nowIso(),
    completedAt: nowIso(),
    outcomes,
    summary: `${state.phase}: ${state.verificationReport?.summary ?? state.symptomSummary}`,
  };
  return withRun(previous, run);
}

export function buildChatQualitySnapshot(
  previous: ModeQualitySnapshot | null | undefined,
  turnId: string,
  outcomes: QualityGateOutcome[],
  qualitySettings?: QualitySettings | null,
): ModeQualitySnapshot {
  const modeConfig = resolveModeQualityConfig(qualitySettings, 'chat');
  const filteredOutcomes = filterOutcomes(outcomes, modeConfig.selectedGateIds);
  const blockingStatus: 'passed' | 'failed' = filteredOutcomes.some(
    (outcome) => outcome.blocking && outcome.status === 'failed',
  )
    ? 'failed'
    : 'passed';
  const run: QualityRunSnapshot = {
    runId: `chat:${turnId}`,
    mode: 'chat',
    scope: 'turn',
    scopeId: turnId,
    trigger: 'post_execution',
    status:
      blockingStatus === 'failed'
        ? 'failed'
        : filteredOutcomes.some((outcome) => outcome.status === 'warning')
          ? 'warning'
          : 'passed',
    decision: buildDecision(blockingStatus, 'chat', modeConfig.behavior),
    recommendedAction: buildAction(blockingStatus, 'chat', modeConfig.behavior),
    retryable: blockingStatus === 'failed',
    blockingStatus,
    startedAt: previous?.updatedAt ?? nowIso(),
    completedAt: nowIso(),
    outcomes: filteredOutcomes,
    summary: `${turnId}: ${blockingStatus}`,
  };
  return withRun(previous, run);
}
