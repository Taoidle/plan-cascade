import type { WorkflowMode } from '../types/workflowKernel';

export type ModeSwitchBlockReason =
  | 'running_execution'
  | 'task_workflow_active'
  | 'plan_workflow_active'
  | 'structured_question_pending'
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
  'completed',
  'failed',
  'cancelled',
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

export type TaskPhase = (typeof TASK_PHASES)[number];
export type PlanPhase = (typeof PLAN_PHASES)[number];
export type NormalizedTaskPhase = TaskPhase | 'unknown';
export type NormalizedPlanPhase = PlanPhase | 'unknown';

const TASK_PHASE_SET = new Set<string>(TASK_PHASES);
const PLAN_PHASE_SET = new Set<string>(PLAN_PHASES);
const TASK_BUSY_PHASES = new Set<TaskPhase>([
  'analyzing',
  'exploring',
  'requirement_analysis',
  'generating_prd',
  'generating_design_doc',
  'executing',
]);
const PLAN_BUSY_PHASES = new Set<PlanPhase>(['analyzing', 'planning', 'executing']);
const TERMINAL_PHASES = new Set<string>(['idle', 'completed', 'failed', 'cancelled']);

const reportedUnknownPhaseKeys = new Set<string>();

function normalizeRawPhase(phase: string | null | undefined): string {
  return (phase ?? '').trim().toLowerCase();
}

export function normalizeTaskPhase(phase: string | null | undefined): NormalizedTaskPhase {
  const normalized = normalizeRawPhase(phase);
  return TASK_PHASE_SET.has(normalized) ? (normalized as TaskPhase) : 'unknown';
}

export function normalizePlanPhase(phase: string | null | undefined): NormalizedPlanPhase {
  const normalized = normalizeRawPhase(phase);
  return PLAN_PHASE_SET.has(normalized) ? (normalized as PlanPhase) : 'unknown';
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

export function isTaskPhaseBusy(phase: string | null | undefined): boolean {
  const normalized = normalizeTaskPhase(phase);
  if (normalized === 'unknown') return true;
  return TASK_BUSY_PHASES.has(normalized);
}

export function isPlanPhaseBusy(phase: string | null | undefined): boolean {
  const normalized = normalizePlanPhase(phase);
  if (normalized === 'unknown') return true;
  return PLAN_BUSY_PHASES.has(normalized);
}

export function isKernelLifecyclePhaseTerminal(phase: string | null | undefined): boolean {
  const normalized = normalizeRawPhase(phase);
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
  return !isKernelLifecyclePhaseTerminal(params.phase);
}

export function resolveModeSwitchBlockReasonFromKernel(params: {
  isRunning: boolean;
  workflowMode: WorkflowMode;
  workflowPhase: string;
  planPhase: string;
  isTaskWorkflowActive: boolean;
  isPlanWorkflowActive: boolean;
  hasStructuredInterviewQuestion: boolean;
  hasPlanClarifyQuestion: boolean;
}): ModeSwitchBlockReason {
  if (params.hasStructuredInterviewQuestion || params.hasPlanClarifyQuestion) {
    return 'structured_question_pending';
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

  return null;
}

export function markUnknownPhaseForReporting(mode: 'task' | 'plan', rawPhase: string | null | undefined): boolean {
  const normalized = normalizeRawPhase(rawPhase);
  if (!normalized) return false;
  const known = mode === 'task' ? TASK_PHASE_SET.has(normalized) : PLAN_PHASE_SET.has(normalized);
  if (known) return false;

  const key = `${mode}:${normalized}`;
  if (reportedUnknownPhaseKeys.has(key)) return false;
  reportedUnknownPhaseKeys.add(key);
  return true;
}
