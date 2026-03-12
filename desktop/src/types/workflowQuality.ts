import type { WorkflowMode } from './workflowKernel';

export type QualityScope = 'turn' | 'step' | 'story' | 'patch' | 'session';
export type QualityTrigger = 'pre_execution' | 'post_execution' | 'transition' | 'manual';
export type QualityDecision = 'pass' | 'warn' | 'needs_review' | 'retry' | 'block' | 'approval_required';
export type QualityDecisionAction = 'retry' | 'approve_and_continue' | 'ignore_with_warning';
export type QualityBehavior = 'manual_review' | 'auto_retry_if_retryable' | 'warn_and_continue';
export type QualityGateStatus = 'pending' | 'running' | 'passed' | 'failed' | 'warning' | 'skipped';
export type QualitySeverity = 'info' | 'warning' | 'soft_fail' | 'hard_fail';
export type QualityGateSource = 'builtin' | 'plugin' | 'custom' | 'llm' | 'fallback_heuristic' | 'derived';

export interface QualityGateOutcome {
  gateId: string;
  gateName: string;
  status: QualityGateStatus;
  severity: QualitySeverity;
  blocking: boolean;
  source: QualityGateSource;
  message?: string | null;
  durationMs?: number | null;
  retryable?: boolean;
  metadata?: Record<string, unknown> | null;
}

export interface QualityRunSnapshot {
  runId: string;
  mode: WorkflowMode;
  scope: QualityScope;
  scopeId: string | null;
  trigger: QualityTrigger;
  status: QualityGateStatus;
  decision: QualityDecision;
  recommendedAction: QualityDecisionAction | null;
  retryable: boolean;
  blockingStatus: 'passed' | 'failed';
  startedAt: string;
  completedAt: string | null;
  outcomes: QualityGateOutcome[];
  summary?: string | null;
}

export interface QualityProfileSummary {
  profileId: string;
  mode: WorkflowMode;
  defaultBehavior: QualityBehavior;
  description: string;
  defaultGateIds: string[];
}

export interface QualityGateDefinition {
  id: string;
  modes: WorkflowMode[];
  labelKey: string;
  descriptionKey: string;
}

export interface ModeQualitySnapshot {
  enabled: boolean;
  profileId: string;
  defaultBehavior: QualityBehavior;
  activeRunId: string | null;
  lastDecision: QualityDecision | null;
  updatedAt: string | null;
  runs: QualityRunSnapshot[];
}

export interface QualityRetryPolicy {
  enabled: boolean;
  maxAttempts: number;
}

export interface QualityProfileOverride {
  defaultGateIds?: string[];
  enableAiReview?: boolean;
  enableCodeReview?: boolean;
  requireApprovalOnRisk?: boolean;
}

export interface QualityCustomGate {
  id: string;
  name: string;
  command: string;
  modes: WorkflowMode[];
  blocking: boolean;
}

export interface QualityPluginPolicy {
  allowPluginGates: boolean;
  blockedPluginIds: string[];
}

export interface QualitySettings {
  enabled: boolean;
  defaultBehaviorByMode: Record<WorkflowMode, QualityBehavior>;
  retryPolicyByMode: Record<WorkflowMode, QualityRetryPolicy>;
  profileOverridesByMode: Record<WorkflowMode, QualityProfileOverride>;
  customGates: QualityCustomGate[];
  pluginPolicy: QualityPluginPolicy;
}

export const QUALITY_GATE_DEFINITIONS: QualityGateDefinition[] = [
  {
    id: 'change_safety',
    modes: ['chat'],
    labelKey: 'quality.gates.change_safety.label',
    descriptionKey: 'quality.gates.change_safety.description',
  },
  {
    id: 'output_completeness',
    modes: ['plan'],
    labelKey: 'quality.gates.output_completeness.label',
    descriptionKey: 'quality.gates.output_completeness.description',
  },
  {
    id: 'criteria_validation',
    modes: ['plan'],
    labelKey: 'quality.gates.criteria_validation.label',
    descriptionKey: 'quality.gates.criteria_validation.description',
  },
  {
    id: 'evidence_sufficiency',
    modes: ['plan', 'debug'],
    labelKey: 'quality.gates.evidence_sufficiency.label',
    descriptionKey: 'quality.gates.evidence_sufficiency.description',
  },
  {
    id: 'artifact_integrity',
    modes: ['plan'],
    labelKey: 'quality.gates.artifact_integrity.label',
    descriptionKey: 'quality.gates.artifact_integrity.description',
  },
  {
    id: 'dor',
    modes: ['task'],
    labelKey: 'quality.gates.dor.label',
    descriptionKey: 'quality.gates.dor.description',
  },
  {
    id: 'format',
    modes: ['task'],
    labelKey: 'quality.gates.format.label',
    descriptionKey: 'quality.gates.format.description',
  },
  {
    id: 'typecheck',
    modes: ['task'],
    labelKey: 'quality.gates.typecheck.label',
    descriptionKey: 'quality.gates.typecheck.description',
  },
  {
    id: 'test',
    modes: ['task'],
    labelKey: 'quality.gates.test.label',
    descriptionKey: 'quality.gates.test.description',
  },
  {
    id: 'lint',
    modes: ['task'],
    labelKey: 'quality.gates.lint.label',
    descriptionKey: 'quality.gates.lint.description',
  },
  {
    id: 'ai_verify',
    modes: ['task'],
    labelKey: 'quality.gates.ai_verify.label',
    descriptionKey: 'quality.gates.ai_verify.description',
  },
  {
    id: 'code_review',
    modes: ['task'],
    labelKey: 'quality.gates.code_review.label',
    descriptionKey: 'quality.gates.code_review.description',
  },
  {
    id: 'dod',
    modes: ['task'],
    labelKey: 'quality.gates.dod.label',
    descriptionKey: 'quality.gates.dod.description',
  },
  {
    id: 'repro_confidence',
    modes: ['debug'],
    labelKey: 'quality.gates.repro_confidence.label',
    descriptionKey: 'quality.gates.repro_confidence.description',
  },
  {
    id: 'patch_safety',
    modes: ['debug'],
    labelKey: 'quality.gates.patch_safety.label',
    descriptionKey: 'quality.gates.patch_safety.description',
  },
  {
    id: 'verification_success',
    modes: ['debug'],
    labelKey: 'quality.gates.verification_success.label',
    descriptionKey: 'quality.gates.verification_success.description',
  },
  {
    id: 'regression_risk',
    modes: ['debug'],
    labelKey: 'quality.gates.regression_risk.label',
    descriptionKey: 'quality.gates.regression_risk.description',
  },
];

export const DEFAULT_QUALITY_GATE_IDS_BY_MODE: Record<WorkflowMode, string[]> = {
  chat: ['change_safety'],
  plan: ['output_completeness', 'criteria_validation', 'evidence_sufficiency', 'artifact_integrity'],
  task: ['dor', 'format', 'typecheck', 'test', 'lint', 'ai_verify', 'code_review', 'dod'],
  debug: ['evidence_sufficiency', 'repro_confidence', 'patch_safety', 'verification_success', 'regression_risk'],
};

export interface ResolvedModeQualityConfig {
  enabled: boolean;
  behavior: QualityBehavior;
  retryPolicy: QualityRetryPolicy;
  selectedGateIds: string[];
  requireApprovalOnRisk: boolean;
}

export function getQualityGateDefinitionsForMode(mode: WorkflowMode): QualityGateDefinition[] {
  return QUALITY_GATE_DEFINITIONS.filter((definition) => definition.modes.includes(mode));
}

export function sanitizeQualityGateIds(mode: WorkflowMode, gateIds: string[] | undefined | null): string[] {
  const allowed = new Set(DEFAULT_QUALITY_GATE_IDS_BY_MODE[mode]);
  if (!Array.isArray(gateIds)) {
    return [...DEFAULT_QUALITY_GATE_IDS_BY_MODE[mode]];
  }
  return gateIds.filter((gateId, index) => allowed.has(gateId) && gateIds.indexOf(gateId) === index);
}

export function sanitizeQualityCustomGates(customGates: QualityCustomGate[] | undefined | null): QualityCustomGate[] {
  if (!Array.isArray(customGates)) {
    return [];
  }
  const seen = new Set<string>();
  const validModes: WorkflowMode[] = ['chat', 'plan', 'task', 'debug'];
  return customGates
    .map((gate, index) => {
      if (!gate || typeof gate !== 'object') return null;
      const id = typeof gate.id === 'string' && gate.id.trim().length > 0 ? gate.id.trim() : `custom-gate-${index + 1}`;
      const name = typeof gate.name === 'string' ? gate.name.trim() : '';
      const command = typeof gate.command === 'string' ? gate.command.trim() : '';
      const modes = Array.isArray(gate.modes)
        ? gate.modes.filter((mode): mode is WorkflowMode => validModes.includes(mode as WorkflowMode))
        : [];
      if (!name || !command || modes.length === 0 || seen.has(id)) {
        return null;
      }
      seen.add(id);
      return {
        id,
        name,
        command,
        modes: modes.filter((mode, modeIndex) => modes.indexOf(mode) === modeIndex),
        blocking: gate.blocking !== false,
      } satisfies QualityCustomGate;
    })
    .filter((gate): gate is QualityCustomGate => !!gate);
}

export function resolveModeQualityConfig(
  quality: QualitySettings | Partial<QualitySettings> | null | undefined,
  mode: WorkflowMode,
): ResolvedModeQualityConfig {
  const defaultBehavior =
    quality?.defaultBehaviorByMode?.[mode] ?? DEFAULT_QUALITY_SETTINGS.defaultBehaviorByMode[mode];
  const retryPolicy = quality?.retryPolicyByMode?.[mode] ?? DEFAULT_QUALITY_SETTINGS.retryPolicyByMode[mode];
  const profileOverride = quality?.profileOverridesByMode?.[mode];
  const builtinGateIds = sanitizeQualityGateIds(mode, profileOverride?.defaultGateIds);
  const customGateIds = (quality?.customGates ?? []).filter((gate) => gate.modes.includes(mode)).map((gate) => gate.id);
  const selectedGateIds = [
    ...builtinGateIds,
    ...customGateIds.filter((gateId, index) => customGateIds.indexOf(gateId) === index),
  ];
  return {
    enabled: quality?.enabled ?? DEFAULT_QUALITY_SETTINGS.enabled,
    behavior: defaultBehavior,
    retryPolicy: retryPolicy ?? DEFAULT_QUALITY_SETTINGS.retryPolicyByMode[mode],
    selectedGateIds,
    requireApprovalOnRisk:
      profileOverride?.requireApprovalOnRisk ??
      DEFAULT_QUALITY_SETTINGS.profileOverridesByMode[mode].requireApprovalOnRisk ??
      false,
  };
}

export const DEFAULT_MODE_QUALITY_SNAPSHOT: ModeQualitySnapshot = {
  enabled: true,
  profileId: 'chat',
  defaultBehavior: 'manual_review',
  activeRunId: null,
  lastDecision: null,
  updatedAt: null,
  runs: [],
};

export const DEFAULT_QUALITY_SETTINGS: QualitySettings = {
  enabled: true,
  defaultBehaviorByMode: {
    chat: 'manual_review',
    plan: 'manual_review',
    task: 'auto_retry_if_retryable',
    debug: 'manual_review',
  },
  retryPolicyByMode: {
    chat: { enabled: true, maxAttempts: 1 },
    plan: { enabled: true, maxAttempts: 1 },
    task: { enabled: true, maxAttempts: 2 },
    debug: { enabled: true, maxAttempts: 1 },
  },
  profileOverridesByMode: {
    chat: {
      defaultGateIds: [...DEFAULT_QUALITY_GATE_IDS_BY_MODE.chat],
      enableAiReview: true,
      enableCodeReview: true,
    },
    plan: {
      defaultGateIds: [...DEFAULT_QUALITY_GATE_IDS_BY_MODE.plan],
      enableAiReview: true,
      enableCodeReview: false,
    },
    task: {
      defaultGateIds: [...DEFAULT_QUALITY_GATE_IDS_BY_MODE.task],
      enableAiReview: true,
      enableCodeReview: true,
    },
    debug: {
      defaultGateIds: [...DEFAULT_QUALITY_GATE_IDS_BY_MODE.debug],
      enableAiReview: false,
      enableCodeReview: false,
      requireApprovalOnRisk: true,
    },
  },
  customGates: [],
  pluginPolicy: {
    allowPluginGates: true,
    blockedPluginIds: [],
  },
};
