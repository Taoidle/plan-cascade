import { create } from 'zustand';
import type { FileAttachmentData } from '../types/attachment';
import type { WorkflowMode } from '../types/workflowKernel';
import type { QueuePriority, QueueStatus, QueuedChatMessage } from '../components/SimpleMode/queuePersistence';

export const DEFAULT_SIMPLE_QUEUE_LIMIT = 20;

interface EnqueueMessageInput {
  sessionId: string;
  prompt: string;
  submitAsFollowUp: boolean;
  mode: WorkflowMode;
  attachments: FileAttachmentData[];
  priority?: QueuePriority;
}

interface EnqueueMessageResult {
  ok: boolean;
  reason: 'session_required' | 'limit_reached' | null;
  item: QueuedChatMessage | null;
}

interface SimpleQueueState {
  items: QueuedChatMessage[];
  nextEnqueueSeq: number;

  hydrate: (items: QueuedChatMessage[]) => void;
  enqueue: (input: EnqueueMessageInput, maxPerSession?: number) => EnqueueMessageResult;
  remove: (id: string) => void;
  clearSession: (sessionId: string) => void;
  clearAll: () => void;
  move: (id: string, direction: 'up' | 'down' | 'top' | 'bottom') => void;
  setPriority: (id: string, priority: QueuePriority) => void;
  markStatus: (id: string, status: QueueStatus, lastError?: string | null) => void;
  incrementAttempts: (id: string, lastError?: string | null) => number;
  resetForRetry: (id: string) => void;
  consume: (id: string) => void;
}

function priorityRank(priority: QueuePriority): number {
  if (priority === 'high') return 0;
  if (priority === 'normal') return 1;
  return 2;
}

function sortQueueItems(items: QueuedChatMessage[]): QueuedChatMessage[] {
  return [...items].sort((a, b) => {
    if (a.sessionId !== b.sessionId) {
      return a.sessionId.localeCompare(b.sessionId);
    }
    return a.enqueueSeq - b.enqueueSeq;
  });
}

function normalizeHydratedItems(items: QueuedChatMessage[]): {
  items: QueuedChatMessage[];
  nextEnqueueSeq: number;
} {
  const normalized: QueuedChatMessage[] = items.map((item, index) => ({
    ...item,
    priority: item.priority === 'high' || item.priority === 'low' ? item.priority : 'normal',
    status:
      item.status === 'running' ||
      item.status === 'succeeded' ||
      item.status === 'failed' ||
      item.status === 'blocked' ||
      item.status === 'retrying'
        ? item.status
        : 'pending',
    enqueueSeq: Number.isFinite(item.enqueueSeq) ? item.enqueueSeq : index,
    attempts: Number.isFinite(item.attempts) ? item.attempts : 0,
    lastError: typeof item.lastError === 'string' ? item.lastError : null,
  }));

  const next = normalized.reduce((max, item) => Math.max(max, item.enqueueSeq + 1), 0);
  return {
    items: sortQueueItems(normalized),
    nextEnqueueSeq: next,
  };
}

export function selectSessionQueueItems(items: QueuedChatMessage[], sessionId: string): QueuedChatMessage[] {
  return items.filter((item) => item.sessionId === sessionId);
}

export function selectNextQueueDispatchItem(items: QueuedChatMessage[], sessionId: string): QueuedChatMessage | null {
  const sessionItems = selectSessionQueueItems(items, sessionId);
  const candidates = sessionItems.filter((item) => item.status === 'pending' || item.status === 'retrying');
  candidates.sort((a, b) => {
    const priorityDiff = priorityRank(a.priority) - priorityRank(b.priority);
    if (priorityDiff !== 0) return priorityDiff;
    return a.enqueueSeq - b.enqueueSeq;
  });
  return candidates[0] ?? null;
}

export const useSimpleQueueStore = create<SimpleQueueState>((set, get) => ({
  items: [],
  nextEnqueueSeq: 0,

  hydrate: (items) => {
    const normalized = normalizeHydratedItems(items);
    set(normalized);
  },

  enqueue: (input, maxPerSession = DEFAULT_SIMPLE_QUEUE_LIMIT) => {
    const normalizedSessionId = input.sessionId.trim();
    if (!normalizedSessionId) {
      return { ok: false, reason: 'session_required', item: null };
    }

    const currentSessionItems = selectSessionQueueItems(get().items, normalizedSessionId);
    if (currentSessionItems.length >= maxPerSession) {
      return { ok: false, reason: 'limit_reached', item: null };
    }

    const nextItem: QueuedChatMessage = {
      id: `queued-${Date.now()}-${get().nextEnqueueSeq}`,
      sessionId: normalizedSessionId,
      prompt: input.prompt,
      submitAsFollowUp: input.submitAsFollowUp,
      mode: input.mode,
      attempts: 0,
      attachments: input.attachments,
      priority: input.priority ?? 'normal',
      status: 'pending',
      enqueueSeq: get().nextEnqueueSeq,
      createdAt: new Date().toISOString(),
      lastError: null,
    };

    set((state) => ({
      items: sortQueueItems([...state.items, nextItem]),
      nextEnqueueSeq: state.nextEnqueueSeq + 1,
    }));

    return { ok: true, reason: null, item: nextItem };
  },

  remove: (id) => {
    set((state) => ({
      items: state.items.filter((item) => item.id !== id),
    }));
  },

  clearSession: (sessionId) => {
    set((state) => ({
      items: state.items.filter((item) => item.sessionId !== sessionId),
    }));
  },

  clearAll: () => {
    set({ items: [] });
  },

  move: (id, direction) => {
    set((state) => {
      const source = state.items.find((item) => item.id === id);
      if (!source) return state;

      const sessionItems = state.items
        .filter((item) => item.sessionId === source.sessionId)
        .sort((a, b) => a.enqueueSeq - b.enqueueSeq);
      const currentIndex = sessionItems.findIndex((item) => item.id === id);
      if (currentIndex < 0) return state;

      let targetIndex = currentIndex;
      if (direction === 'up') targetIndex = currentIndex - 1;
      if (direction === 'down') targetIndex = currentIndex + 1;
      if (direction === 'top') targetIndex = 0;
      if (direction === 'bottom') targetIndex = sessionItems.length - 1;
      if (targetIndex < 0 || targetIndex >= sessionItems.length || targetIndex === currentIndex) return state;

      const reordered = [...sessionItems];
      const [moved] = reordered.splice(currentIndex, 1);
      if (!moved) return state;
      reordered.splice(targetIndex, 0, moved);

      const nextSeqById = new Map<string, number>();
      reordered.forEach((item, index) => {
        nextSeqById.set(item.id, index);
      });

      const nextItems = state.items.map((item) => {
        if (item.sessionId !== source.sessionId) return item;
        const nextSeq = nextSeqById.get(item.id);
        if (typeof nextSeq !== 'number') return item;
        return {
          ...item,
          enqueueSeq: nextSeq,
        };
      });

      return { items: sortQueueItems(nextItems) };
    });
  },

  setPriority: (id, priority) => {
    set((state) => ({
      items: sortQueueItems(
        state.items.map((item) =>
          item.id === id
            ? {
                ...item,
                priority,
              }
            : item,
        ),
      ),
    }));
  },

  markStatus: (id, status, lastError = null) => {
    set((state) => ({
      items: state.items.map((item) =>
        item.id === id
          ? {
              ...item,
              status,
              lastError,
            }
          : item,
      ),
    }));
  },

  incrementAttempts: (id, lastError = null) => {
    let attempts = 0;
    set((state) => ({
      items: state.items.map((item) => {
        if (item.id !== id) return item;
        attempts = item.attempts + 1;
        return {
          ...item,
          attempts,
          status: 'retrying',
          lastError,
        };
      }),
    }));
    return attempts;
  },

  resetForRetry: (id) => {
    set((state) => ({
      items: state.items.map((item) =>
        item.id === id
          ? {
              ...item,
              status: 'pending',
              lastError: null,
            }
          : item,
      ),
    }));
  },

  consume: (id) => {
    set((state) => ({
      items: state.items.filter((item) => item.id !== id),
    }));
  },
}));
