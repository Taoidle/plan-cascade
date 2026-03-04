import type { WorkflowSession, WorkflowStatus } from '../types/workflowKernel';

export interface KernelTaskRuntime {
  phase: string;
  linkedSessionId: string | null;
  interviewId: string | null;
}

export interface KernelPlanRuntime {
  phase: string;
  linkedSessionId: string | null;
}

export function selectKernelTaskRuntime(session: WorkflowSession | null): KernelTaskRuntime {
  return {
    phase: session?.modeSnapshots.task?.phase ?? 'idle',
    linkedSessionId: session?.linkedModeSessions?.task ?? null,
    interviewId: session?.modeSnapshots.task?.pendingInterview?.interviewId ?? null,
  };
}

export function selectKernelPlanRuntime(session: WorkflowSession | null): KernelPlanRuntime {
  return {
    phase: session?.modeSnapshots.plan?.phase ?? 'idle',
    linkedSessionId: session?.linkedModeSessions?.plan ?? null,
  };
}

export function isKernelModeActive(
  session: WorkflowSession | null,
  mode: 'chat' | 'plan' | 'task',
  expectedStatus: WorkflowStatus = 'active',
): boolean {
  return session?.status === expectedStatus && session.activeMode === mode;
}
