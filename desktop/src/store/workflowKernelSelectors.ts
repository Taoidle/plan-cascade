import type {
  PlanClarificationSnapshot,
  TaskInterviewSnapshot,
  WorkflowSession,
  WorkflowStatus,
} from '../types/workflowKernel';

const TERMINAL_PHASES = new Set(['idle', 'completed', 'failed', 'cancelled']);

export interface KernelRuntimeBase {
  phase: string;
  linkedSessionId: string | null;
  isActive: boolean;
  pendingPrompt: string | null;
}

export interface KernelTaskRuntime extends KernelRuntimeBase {
  pendingInterview: TaskInterviewSnapshot | null;
  interviewId: string | null;
}

export interface KernelPlanRuntime extends KernelRuntimeBase {
  pendingClarification: PlanClarificationSnapshot | null;
}

export type KernelChatRuntime = KernelRuntimeBase;

function normalizePendingPrompt(prompt: string | null | undefined): string | null {
  const normalized = prompt?.trim() ?? '';
  return normalized.length > 0 ? normalized : null;
}

export function isKernelPhaseTerminal(phase: string | null | undefined): boolean {
  return TERMINAL_PHASES.has((phase ?? '').toLowerCase());
}

export function isKernelRuntimeBusy(runtime: Pick<KernelRuntimeBase, 'phase' | 'isActive'>): boolean {
  return runtime.isActive && !isKernelPhaseTerminal(runtime.phase);
}

export function selectKernelChatRuntime(session: WorkflowSession | null): KernelChatRuntime {
  return {
    phase: session?.modeSnapshots.chat?.phase ?? 'ready',
    linkedSessionId: null,
    isActive: isKernelModeActive(session, 'chat'),
    pendingPrompt: normalizePendingPrompt(session?.modeSnapshots.chat?.draftInput),
  };
}

export function selectKernelTaskRuntime(session: WorkflowSession | null): KernelTaskRuntime {
  const pendingInterview = session?.modeSnapshots.task?.pendingInterview ?? null;
  return {
    phase: session?.modeSnapshots.task?.phase ?? 'idle',
    linkedSessionId: session?.linkedModeSessions?.task ?? null,
    isActive: isKernelModeActive(session, 'task'),
    pendingPrompt: normalizePendingPrompt(pendingInterview?.question),
    pendingInterview,
    interviewId: pendingInterview?.interviewId ?? null,
  };
}

export function selectKernelPlanRuntime(session: WorkflowSession | null): KernelPlanRuntime {
  const pendingClarification = session?.modeSnapshots.plan?.pendingClarification ?? null;
  return {
    phase: session?.modeSnapshots.plan?.phase ?? 'idle',
    linkedSessionId: session?.linkedModeSessions?.plan ?? null,
    isActive: isKernelModeActive(session, 'plan'),
    pendingPrompt: normalizePendingPrompt(pendingClarification?.question),
    pendingClarification,
  };
}

export function isKernelModeActive(
  session: WorkflowSession | null,
  mode: 'chat' | 'plan' | 'task',
  expectedStatus: WorkflowStatus = 'active',
): boolean {
  return session?.status === expectedStatus && session.activeMode === mode;
}
