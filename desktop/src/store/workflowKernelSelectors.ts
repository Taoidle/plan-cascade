import type {
  PlanClarificationSnapshot,
  TaskInterviewSnapshot,
  WorkflowSession,
  WorkflowStatus,
} from '../types/workflowKernel';
import {
  isKernelLifecyclePhaseTerminal,
  isPlanPhaseBusy,
  isTaskPhaseBusy,
  isWorkflowModeActive,
  normalizePlanPhase,
  normalizeTaskPhase,
  type NormalizedPlanPhase,
  type NormalizedTaskPhase,
} from './workflowPhaseModel';

export interface KernelRuntimeBase {
  phase: string;
  linkedSessionId: string | null;
  isActive: boolean;
  pendingPrompt: string | null;
}

export interface KernelTaskRuntime extends KernelRuntimeBase {
  normalizedPhase: NormalizedTaskPhase;
  isBusy: boolean;
  pendingInterview: TaskInterviewSnapshot | null;
  interviewId: string | null;
}

export interface KernelPlanRuntime extends KernelRuntimeBase {
  normalizedPhase: NormalizedPlanPhase;
  isBusy: boolean;
  pendingClarification: PlanClarificationSnapshot | null;
}

export type KernelChatRuntime = KernelRuntimeBase;

function normalizePendingPrompt(prompt: string | null | undefined): string | null {
  const normalized = prompt?.trim() ?? '';
  return normalized.length > 0 ? normalized : null;
}

export function isKernelPhaseTerminal(phase: string | null | undefined): boolean {
  return isKernelLifecyclePhaseTerminal(phase);
}

export function isKernelRuntimeBusy(runtime: Pick<KernelRuntimeBase, 'phase' | 'isActive'>): boolean {
  return runtime.isActive && !isKernelPhaseTerminal(runtime.phase);
}

export function selectKernelChatRuntime(session: WorkflowSession | null): KernelChatRuntime {
  return {
    phase: session?.modeSnapshots.chat?.phase ?? 'ready',
    linkedSessionId: session?.linkedModeSessions?.chat ?? null,
    isActive: isWorkflowModeActive({
      mode: 'chat',
      currentMode: session?.activeMode ?? 'chat',
      isKernelSessionActive: session?.status === 'active',
      phase: session?.modeSnapshots.chat?.phase ?? 'ready',
    }),
    pendingPrompt: normalizePendingPrompt(session?.modeSnapshots.chat?.draftInput),
  };
}

export function selectKernelTaskRuntime(session: WorkflowSession | null): KernelTaskRuntime {
  const pendingInterview = session?.modeSnapshots.task?.pendingInterview ?? null;
  const phase = session?.modeSnapshots.task?.phase ?? 'idle';
  const normalizedPhase = normalizeTaskPhase(phase);
  const isActive = isWorkflowModeActive({
    mode: 'task',
    currentMode: session?.activeMode ?? 'chat',
    isKernelSessionActive: session?.status === 'active',
    phase,
  });
  return {
    phase,
    normalizedPhase,
    linkedSessionId: session?.linkedModeSessions?.task ?? null,
    isActive,
    isBusy: isActive && isTaskPhaseBusy(phase),
    pendingPrompt: normalizePendingPrompt(pendingInterview?.question),
    pendingInterview,
    interviewId: pendingInterview?.interviewId ?? null,
  };
}

export function selectKernelPlanRuntime(session: WorkflowSession | null): KernelPlanRuntime {
  const pendingClarification = session?.modeSnapshots.plan?.pendingClarification ?? null;
  const phase = session?.modeSnapshots.plan?.phase ?? 'idle';
  const normalizedPhase = normalizePlanPhase(phase);
  const isActive = isWorkflowModeActive({
    mode: 'plan',
    currentMode: session?.activeMode ?? 'chat',
    isKernelSessionActive: session?.status === 'active',
    phase,
  });
  return {
    phase,
    normalizedPhase,
    linkedSessionId: session?.linkedModeSessions?.plan ?? null,
    isActive,
    isBusy: isActive && isPlanPhaseBusy(phase),
    pendingPrompt: normalizePendingPrompt(pendingClarification?.question),
    pendingClarification,
  };
}

export function isKernelModeActive(
  session: WorkflowSession | null,
  mode: 'chat' | 'plan' | 'task',
  expectedStatus: WorkflowStatus = 'active',
): boolean {
  return isWorkflowModeActive({
    mode,
    currentMode: session?.activeMode ?? 'chat',
    isKernelSessionActive: session?.status === expectedStatus,
    phase:
      mode === 'task'
        ? (session?.modeSnapshots.task?.phase ?? 'idle')
        : mode === 'plan'
          ? (session?.modeSnapshots.plan?.phase ?? 'idle')
          : (session?.modeSnapshots.chat?.phase ?? 'ready'),
  });
}

export interface KernelRuntimeStatus {
  isTaskActive: boolean;
  isPlanActive: boolean;
  isTaskBusy: boolean;
  isPlanBusy: boolean;
}

export function selectKernelRuntimeStatus(session: WorkflowSession | null): KernelRuntimeStatus {
  const taskRuntime = selectKernelTaskRuntime(session);
  const planRuntime = selectKernelPlanRuntime(session);
  return {
    isTaskActive: taskRuntime.isActive,
    isPlanActive: planRuntime.isActive,
    isTaskBusy: taskRuntime.isBusy,
    isPlanBusy: planRuntime.isBusy,
  };
}
