import { useExecutionStore } from './execution';
import { useWorkflowKernelStore } from './workflowKernel';
import type { StreamLine, StreamLineType } from './execution/types';
import type { CardPayload } from '../types/workflowCard';
import type { WorkflowMode, WorkflowSession, WorkflowSessionCatalogItem } from '../types/workflowKernel';

let routedLineCounter = 0;
const transcriptAppendQueue = new Map<string, Promise<void>>();

function nextRoutedLineId(): number {
  routedLineCounter += 1;
  return Date.now() + routedLineCounter;
}

function resolveRootSessionFromCurrentSession(
  session: WorkflowSession | null,
  mode: WorkflowMode,
  modeSessionId: string | null,
): string | null {
  if (!session) return null;
  if (!modeSessionId) return session.sessionId;
  const linkedSessionId = session.linkedModeSessions?.[mode] ?? null;
  return linkedSessionId === modeSessionId ? session.sessionId : null;
}

function resolveRootSessionFromCatalog(
  catalog: WorkflowSessionCatalogItem[],
  mode: WorkflowMode,
  modeSessionId: string | null,
): string | null {
  if (!modeSessionId) return null;
  for (const item of catalog) {
    const bindingSessionId = item.modeRuntimeMeta?.[mode]?.bindingSessionId ?? null;
    if (bindingSessionId === modeSessionId) {
      return item.sessionId;
    }
  }
  return null;
}

export function resolveRootSessionForMode(mode: WorkflowMode, modeSessionId?: string | null): string | null {
  const kernel = useWorkflowKernelStore.getState();
  return (
    resolveRootSessionFromCurrentSession(kernel.session, mode, modeSessionId ?? null) ??
    resolveRootSessionFromCatalog(kernel.sessionCatalog, mode, modeSessionId ?? null) ??
    kernel.activeRootSessionId ??
    kernel.sessionId
  );
}

function isTargetModeVisible(rootSessionId: string, mode: WorkflowMode): boolean {
  const kernel = useWorkflowKernelStore.getState();
  return kernel.activeRootSessionId === rootSessionId && kernel.activeMode === mode;
}

function buildStreamLineBase(
  content: string,
  type: Exclude<StreamLineType, 'card'>,
  turnBoundary?: 'user' | 'assistant',
  turnId?: number,
): StreamLine {
  return {
    id: nextRoutedLineId(),
    content,
    type,
    timestamp: Date.now(),
    ...(turnBoundary ? { turnBoundary } : {}),
    ...(turnId != null ? { turnId } : {}),
  };
}

function buildCardLine(payload: CardPayload): StreamLine {
  return {
    id: nextRoutedLineId(),
    content: JSON.stringify(payload),
    type: 'card',
    timestamp: Date.now(),
    cardPayload: payload,
  };
}

async function appendModeTranscriptLines(
  rootSessionId: string,
  mode: WorkflowMode,
  lines: StreamLine[],
): Promise<void> {
  const appendModeTranscript = useWorkflowKernelStore.getState().appendModeTranscript;
  if (typeof appendModeTranscript !== 'function') {
    return;
  }
  const queueKey = `${rootSessionId}:${mode}`;
  const previous = transcriptAppendQueue.get(queueKey) ?? Promise.resolve();
  const next = previous
    .catch(() => undefined)
    .then(async () => {
      await appendModeTranscript(rootSessionId, mode, lines);
    });
  transcriptAppendQueue.set(queueKey, next);
  await next;
}

export async function routeModeCard(
  mode: WorkflowMode,
  payload: CardPayload,
  modeSessionId?: string | null,
): Promise<void> {
  const rootSessionId = resolveRootSessionForMode(mode, modeSessionId);
  if (!rootSessionId) {
    useExecutionStore.getState().appendCard(payload);
    return;
  }
  const shouldMirrorForeground = !modeSessionId || isTargetModeVisible(rootSessionId, mode);
  if (shouldMirrorForeground) {
    useExecutionStore.getState().appendCard(payload);
  }

  const line = buildCardLine(payload);
  await appendModeTranscriptLines(rootSessionId, mode, [line]);
}

export async function routeModeStreamLine(
  mode: WorkflowMode,
  content: string,
  type: Exclude<StreamLineType, 'card'>,
  options?: {
    modeSessionId?: string | null;
    turnBoundary?: 'user' | 'assistant';
    turnId?: number;
  },
): Promise<void> {
  const rootSessionId = resolveRootSessionForMode(mode, options?.modeSessionId ?? null);
  if (!rootSessionId) {
    useExecutionStore.getState().appendStreamLine(content, type, undefined, undefined, {
      turnBoundary: options?.turnBoundary,
      turnId: options?.turnId,
    });
    return;
  }
  const shouldMirrorForeground = !options?.modeSessionId || isTargetModeVisible(rootSessionId, mode);
  if (shouldMirrorForeground) {
    useExecutionStore.getState().appendStreamLine(content, type, undefined, undefined, {
      turnBoundary: options?.turnBoundary,
      turnId: options?.turnId,
    });
  }

  const line = buildStreamLineBase(content, type, options?.turnBoundary, options?.turnId);
  await appendModeTranscriptLines(rootSessionId, mode, [line]);
}
