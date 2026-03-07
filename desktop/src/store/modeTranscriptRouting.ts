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
    resolveRootSessionFromCatalog(kernel.sessionCatalog, mode, modeSessionId ?? null)
  );
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
    console.warn('[workflow-kernel] dropping mode card without resolved root session', {
      mode,
      modeSessionId,
      cardType: payload.cardType,
    });
    return;
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
    console.warn('[workflow-kernel] dropping mode stream line without resolved root session', {
      mode,
      modeSessionId: options?.modeSessionId ?? null,
      type,
    });
    return;
  }

  const line = buildStreamLineBase(content, type, options?.turnBoundary, options?.turnId);
  await appendModeTranscriptLines(rootSessionId, mode, [line]);
}
