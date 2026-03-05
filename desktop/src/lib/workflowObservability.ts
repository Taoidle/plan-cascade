import { useWorkflowObservabilityStore } from '../store/workflowObservability';
import type { WorkflowMode, WorkflowSession } from '../types/workflowKernel';

function sessionPhaseForMode(session: WorkflowSession, mode: WorkflowMode): string | null {
  if (mode === 'chat') return session.modeSnapshots.chat?.phase ?? null;
  if (mode === 'plan') return session.modeSnapshots.plan?.phase ?? null;
  return session.modeSnapshots.task?.phase ?? null;
}

function linkedModeSessionId(session: WorkflowSession, mode: WorkflowMode): string | null {
  return session.linkedModeSessions[mode] ?? null;
}

interface ReportInteractiveActionFailureParams {
  card: string;
  action: string;
  errorCode: string;
  message?: string | null;
  session?: WorkflowSession | null;
  phaseAfter?: string | null;
}

export async function reportInteractiveActionFailure({
  card,
  action,
  errorCode,
  message,
  session,
  phaseAfter,
}: ReportInteractiveActionFailureParams): Promise<void> {
  const mode = session?.activeMode ?? null;
  const phaseBefore = mode && session ? sessionPhaseForMode(session, mode) : null;
  await useWorkflowObservabilityStore.getState().recordInteractiveActionFailure({
    card,
    action,
    errorCode,
    message: message ?? null,
    mode,
    kernelSessionId: session?.sessionId ?? null,
    modeSessionId: mode && session ? linkedModeSessionId(session, mode) : null,
    phaseBefore,
    phaseAfter: phaseAfter ?? phaseBefore,
  });
}
