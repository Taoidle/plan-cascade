import { getNextTurnId } from '../../lib/conversationUtils';
import { useWorkflowKernelStore } from '../workflowKernel';
import type { NonCardStreamLineType, StreamLine } from './types';

function cloneStreamLines(lines: StreamLine[]): StreamLine[] {
  return lines.map((line) => ({ ...line }));
}

function nextStreamLineId(lines: StreamLine[]): number {
  return lines.reduce((max, line) => Math.max(max, line.id), 0) + 1;
}

function getLastTurnId(lines: StreamLine[]): number | undefined {
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    const turnId = lines[index]?.turnId;
    if (typeof turnId === 'number' && Number.isFinite(turnId)) {
      return turnId;
    }
  }
  return undefined;
}

export function getActiveKernelChatRootSessionId(): string | null {
  const kernel = useWorkflowKernelStore.getState();
  return kernel.activeRootSessionId ?? kernel.sessionId ?? null;
}

export function getKernelChatTranscript(rootSessionId: string | null): StreamLine[] {
  if (!rootSessionId) {
    return [];
  }
  return cloneStreamLines(
    useWorkflowKernelStore.getState().getCachedModeTranscript(rootSessionId, 'chat').lines as StreamLine[],
  );
}

export function getActiveKernelChatTranscript(): { rootSessionId: string | null; lines: StreamLine[] } {
  const rootSessionId = getActiveKernelChatRootSessionId();
  return {
    rootSessionId,
    lines: getKernelChatTranscript(rootSessionId),
  };
}

export function resolveKernelChatRootSessionIdForRuntime(
  source: 'claude' | 'standalone',
  rawSessionId: string | null,
): string | null {
  const normalizedRawSessionId = rawSessionId?.trim() ?? '';
  if (!normalizedRawSessionId) {
    return null;
  }

  const linkedSessionId = `${source}:${normalizedRawSessionId}`;
  const kernel = useWorkflowKernelStore.getState();
  if (kernel.session?.linkedModeSessions?.chat === linkedSessionId) {
    return kernel.session.sessionId;
  }

  for (const item of kernel.sessionCatalog) {
    const bindingSessionId = item.modeRuntimeMeta?.chat?.bindingSessionId ?? null;
    if (bindingSessionId === linkedSessionId) {
      return item.sessionId;
    }
  }

  return null;
}

export function buildAppendedUserLine(content: string, transcriptLines: StreamLine[]): StreamLine {
  return {
    id: nextStreamLineId(transcriptLines),
    content,
    type: 'info',
    timestamp: Date.now(),
    turnId: getNextTurnId(transcriptLines),
    turnBoundary: 'user',
  };
}

export function buildReplacementUserLine(content: string, previousLine: StreamLine): StreamLine {
  return {
    id: previousLine.id,
    content,
    type: 'info',
    timestamp: Date.now(),
    turnId: previousLine.turnId,
    turnBoundary: 'user',
  };
}

export function buildAppendedAssistantLine(
  content: string,
  transcriptLines: StreamLine[],
  type: NonCardStreamLineType = 'text',
): StreamLine {
  return {
    id: nextStreamLineId(transcriptLines),
    content,
    type,
    timestamp: Date.now(),
    turnId: getLastTurnId(transcriptLines),
    ...(type === 'text' ? { turnBoundary: 'assistant' as const } : {}),
  };
}

export async function patchKernelChatTranscript(
  rootSessionId: string,
  patch: {
    replaceFromLineId?: number | null;
    appendedLines: StreamLine[];
  },
): Promise<StreamLine[]> {
  const result = await useWorkflowKernelStore.getState().patchModeTranscript(rootSessionId, 'chat', {
    replaceFromLineId: patch.replaceFromLineId ?? null,
    appendedLines: patch.appendedLines.map((line) => ({ ...line })),
  });
  return cloneStreamLines((result?.lines ?? []) as StreamLine[]);
}

export async function replaceKernelChatTranscriptLines(
  rootSessionId: string,
  lines: StreamLine[],
): Promise<StreamLine[]> {
  return patchKernelChatTranscript(rootSessionId, {
    replaceFromLineId: 0,
    appendedLines: cloneStreamLines(lines),
  });
}

export async function appendKernelChatRootLine(
  rootSessionId: string,
  content: string,
  type: NonCardStreamLineType = 'text',
): Promise<StreamLine[] | null> {
  if (!rootSessionId || !content) {
    return null;
  }

  const transcriptLines = getKernelChatTranscript(rootSessionId);
  const line = buildAppendedAssistantLine(content, transcriptLines, type);
  return patchKernelChatTranscript(rootSessionId, {
    replaceFromLineId: null,
    appendedLines: [line],
  });
}

export async function appendKernelChatRuntimeLine(
  source: 'claude' | 'standalone',
  rawSessionId: string | null,
  content: string,
  type: NonCardStreamLineType = 'text',
): Promise<StreamLine[] | null> {
  const normalizedContent = content;
  if (!normalizedContent) {
    return null;
  }

  const rootSessionId = resolveKernelChatRootSessionIdForRuntime(source, rawSessionId);
  if (!rootSessionId) {
    return null;
  }

  return appendKernelChatRootLine(rootSessionId, normalizedContent, type);
}

export async function ensureActiveChatModeSessionLinked(
  source: 'claude' | 'standalone',
  rawSessionId: string | null,
): Promise<void> {
  const normalizedRawSessionId = rawSessionId?.trim() ?? '';
  if (!normalizedRawSessionId) {
    return;
  }

  const kernel = useWorkflowKernelStore.getState();
  if (!kernel.sessionId) {
    return;
  }

  const linkedSessionId = `${source}:${normalizedRawSessionId}`;
  if (kernel.session?.linkedModeSessions?.chat === linkedSessionId) {
    return;
  }

  await kernel.linkModeSession('chat', linkedSessionId);
}
