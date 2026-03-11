import type { WorkflowMode } from '../types/workflowKernel';

export type ModeSwitchBlockReason =
  | 'running_execution'
  | 'task_workflow_active'
  | 'plan_workflow_active'
  | 'debug_workflow_active'
  | 'structured_question_pending'
  | 'approval_pending'
  | 'active_experiment'
  | 'verification_running'
  | null;

export const TASK_PHASES = [
  'idle',
  'analyzing',
  'configuring',
  'interviewing',
  'exploring',
  'requirement_analysis',
  'generating_prd',
  'reviewing_prd',
  'architecture_review',
  'generating_design_doc',
  'executing',
  'paused',
  'completed',
  'failed',
  'cancelled',
] as const;

export const CHAT_PHASES = [
  'ready',
  'submitting',
  'streaming',
  'paused',
  'failed',
  'cancelled',
  'interrupted',
] as const;
export const PLAN_PHASES = [
  'idle',
  'analyzing',
  'clarifying',
  'clarification_error',
  'planning',
  'reviewing_plan',
  'executing',
  'completed',
  'failed',
  'cancelled',
] as const;
export const DEBUG_PHASES = [
  'intaking',
  'clarifying',
  'gathering_signal',
  'reproducing',
  'hypothesizing',
  'testing_hypothesis',
  'identifying_root_cause',
  'proposing_fix',
  'patch_review',
  'patching',
  'verifying',
  'completed',
  'failed',
  'cancelled',
] as const;

export type TaskPhase = (typeof TASK_PHASES)[number];
export type PlanPhase = (typeof PLAN_PHASES)[number];
export type ChatPhase = (typeof CHAT_PHASES)[number];
export type DebugPhase = (typeof DEBUG_PHASES)[number];
export type NormalizedChatPhase = ChatPhase | 'unknown';
export type NormalizedTaskPhase = TaskPhase | 'unknown';
export type NormalizedPlanPhase = PlanPhase | 'unknown';
export type NormalizedDebugPhase = DebugPhase | 'unknown';

const CHAT_PHASE_SET = new Set<string>(CHAT_PHASES);
const TASK_PHASE_SET = new Set<string>(TASK_PHASES);
const PLAN_PHASE_SET = new Set<string>(PLAN_PHASES);
const DEBUG_PHASE_SET = new Set<string>(DEBUG_PHASES);
const CHAT_BUSY_PHASES = new Set<ChatPhase>(['submitting', 'streaming', 'paused']);
const TASK_BUSY_PHASES = new Set<TaskPhase>([
  'analyzing',
  'exploring',
  'requirement_analysis',
  'generating_prd',
  'generating_design_doc',
  'executing',
  'paused',
]);
const PLAN_BUSY_PHASES = new Set<PlanPhase>(['analyzing', 'planning', 'executing']);
const DEBUG_BUSY_PHASES = new Set<DebugPhase>([
  'clarifying',
  'gathering_signal',
  'reproducing',
  'testing_hypothesis',
  'identifying_root_cause',
  'patching',
  'verifying',
]);
const TERMINAL_PHASES = new Set<string>(['idle', 'completed', 'failed', 'cancelled']);
const CHAT_TERMINAL_PHASES = new Set<string>(['ready', 'failed', 'cancelled', 'interrupted']);

const reportedUnknownPhaseKeys = new Set<string>();

function normalizeRawPhase(phase: string | null | undefined): string {
  return (phase ?? '').trim().toLowerCase();
}

export function normalizeTaskPhase(phase: string | null | undefined): NormalizedTaskPhase {
  const normalized = normalizeRawPhase(phase);
  return TASK_PHASE_SET.has(normalized) ? (normalized as TaskPhase) : 'unknown';
}

export function normalizeChatPhase(phase: string | null | undefined): NormalizedChatPhase {
  const normalized = normalizeRawPhase(phase);
  if (normalized === 'running') return 'streaming';
  if (normalized === 'idle' || normalized === 'completed') return 'ready';
  return CHAT_PHASE_SET.has(normalized) ? (normalized as ChatPhase) : 'unknown';
}

export function normalizePlanPhase(phase: string | null | undefined): NormalizedPlanPhase {
  const normalized = normalizeRawPhase(phase);
  return PLAN_PHASE_SET.has(normalized) ? (normalized as PlanPhase) : 'unknown';
}

export function normalizeDebugPhase(phase: string | null | undefined): NormalizedDebugPhase {
  const normalized = normalizeRawPhase(phase);
  return DEBUG_PHASE_SET.has(normalized) ? (normalized as DebugPhase) : 'unknown';
}

export function isChatPhaseTerminal(phase: string | null | undefined): boolean {
  const normalized = normalizeChatPhase(phase);
  if (normalized === 'unknown') return false;
  return CHAT_TERMINAL_PHASES.has(normalized);
}

export function isTaskPhaseTerminal(phase: string | null | undefined): boolean {
  const normalized = normalizeTaskPhase(phase);
  if (normalized === 'unknown') return false;
  return TERMINAL_PHASES.has(normalized);
}

export function isPlanPhaseTerminal(phase: string | null | undefined): boolean {
  const normalized = normalizePlanPhase(phase);
  if (normalized === 'unknown') return false;
  return TERMINAL_PHASES.has(normalized);
}

export function isDebugPhaseTerminal(phase: string | null | undefined): boolean {
  const normalized = normalizeDebugPhase(phase);
  if (normalized === 'unknown') return false;
  return normalized === 'completed' || normalized === 'failed' || normalized === 'cancelled';
}

export function isTaskPhaseBusy(phase: string | null | undefined): boolean {
  const normalized = normalizeTaskPhase(phase);
  if (normalized === 'unknown') return true;
  return TASK_BUSY_PHASES.has(normalized);
}

export function isChatPhaseBusy(phase: string | null | undefined): boolean {
  const normalized = normalizeChatPhase(phase);
  if (normalized === 'unknown') return false;
  return CHAT_BUSY_PHASES.has(normalized);
}

export function isPlanPhaseBusy(phase: string | null | undefined): boolean {
  const normalized = normalizePlanPhase(phase);
  if (normalized === 'unknown') return true;
  return PLAN_BUSY_PHASES.has(normalized);
}

export function isDebugPhaseBusy(phase: string | null | undefined): boolean {
  const normalized = normalizeDebugPhase(phase);
  if (normalized === 'unknown') return true;
  return DEBUG_BUSY_PHASES.has(normalized);
}

export function isKernelLifecyclePhaseTerminal(phase: string | null | undefined): boolean {
  const normalized = normalizeRawPhase(phase);
  if (CHAT_PHASE_SET.has(normalized) || normalized === 'running') {
    return isChatPhaseTerminal(normalized);
  }
  return TERMINAL_PHASES.has(normalized);
}

export function isWorkflowModeActive(params: {
  mode: WorkflowMode;
  currentMode: WorkflowMode;
  isKernelSessionActive: boolean;
  phase: string | null | undefined;
}): boolean {
  if (params.mode !== params.currentMode) return false;
  if (!params.isKernelSessionActive) return false;
  if (params.mode === 'task') return !isTaskPhaseTerminal(params.phase);
  if (params.mode === 'plan') return !isPlanPhaseTerminal(params.phase);
  if (params.mode === 'debug') return !isDebugPhaseTerminal(params.phase);
  return !isChatPhaseTerminal(params.phase);
}

export function resolveModeSwitchBlockReasonFromKernel(params: {
  isRunning: boolean;
  workflowMode: WorkflowMode;
  workflowPhase: string;
  planPhase: string;
  debugPhase?: string;
  isTaskWorkflowActive: boolean;
  isPlanWorkflowActive: boolean;
  isDebugWorkflowActive?: boolean;
  hasStructuredInterviewQuestion: boolean;
  hasPlanClarifyQuestion: boolean;
  hasDebugPendingApproval?: boolean;
  hasDebugActiveExperiment?: boolean;
  hasDebugVerificationRunning?: boolean;
}): ModeSwitchBlockReason {
  if (params.hasStructuredInterviewQuestion || params.hasPlanClarifyQuestion) {
    return 'structured_question_pending';
  }
  if (params.hasDebugPendingApproval) {
    return 'approval_pending';
  }
  if (params.hasDebugActiveExperiment) {
    return 'active_experiment';
  }
  if (params.hasDebugVerificationRunning) {
    return 'verification_running';
  }
  if (params.isRunning) {
    return 'running_execution';
  }

  const taskActive =
    params.isTaskWorkflowActive || (params.workflowMode === 'task' && !isTaskPhaseTerminal(params.workflowPhase));
  if (taskActive) {
    return 'task_workflow_active';
  }

  const planActive =
    params.isPlanWorkflowActive || (params.workflowMode === 'plan' && !isPlanPhaseTerminal(params.planPhase));
  if (planActive) {
    return 'plan_workflow_active';
  }

  const debugActive =
    params.isDebugWorkflowActive || (params.workflowMode === 'debug' && !isDebugPhaseTerminal(params.debugPhase));
  if (debugActive) {
    return 'debug_workflow_active';
  }

  return null;
}

export function markUnknownPhaseForReporting(
  mode: 'task' | 'plan' | 'debug',
  rawPhase: string | null | undefined,
): boolean {
  const normalized = normalizeRawPhase(rawPhase);
  if (!normalized) return false;
  const known =
    mode === 'task'
      ? TASK_PHASE_SET.has(normalized)
      : mode === 'plan'
        ? PLAN_PHASE_SET.has(normalized)
        : DEBUG_PHASE_SET.has(normalized);
  if (known) return false;

  const key = `${mode}:${normalized}`;
  if (reportedUnknownPhaseKeys.has(key)) return false;
  reportedUnknownPhaseKeys.add(key);
  return true;
}
