import type { ExecutionState } from './types';

interface PendingDelta {
  text: string;
  thinking: string;
  subAgentId?: string;
  subAgentDepth?: number;
}

const pendingDeltas = new Map<string, PendingDelta>();
let flushTimer: ReturnType<typeof setTimeout> | null = null;
const flushCallbacks = new Set<() => void>();
const ROOT_DELTA_KEY = '__root__';

export function getPending(subAgentId?: string, depth?: number): PendingDelta {
  const key = subAgentId || ROOT_DELTA_KEY;
  let entry = pendingDeltas.get(key);
  if (!entry) {
    entry = { text: '', thinking: '', subAgentId, subAgentDepth: depth };
    pendingDeltas.set(key, entry);
  }
  return entry;
}

export function flushPendingDeltas(get: () => ExecutionState): void {
  if (flushTimer) {
    clearTimeout(flushTimer);
    flushTimer = null;
  }
  for (const [, delta] of pendingDeltas) {
    if (delta.text) {
      get().appendStreamLine(delta.text, 'text', delta.subAgentId, delta.subAgentDepth);
      delta.text = '';
    }
    if (delta.thinking) {
      get().appendStreamLine(delta.thinking, 'thinking', delta.subAgentId, delta.subAgentDepth);
      delta.thinking = '';
    }
  }
  const callbacks = Array.from(flushCallbacks);
  flushCallbacks.clear();
  for (const callback of callbacks) {
    callback();
  }
}

export function scheduleFlush(get: () => ExecutionState, afterFlush?: (() => void) | null): void {
  if (afterFlush) {
    flushCallbacks.add(afterFlush);
  }
  if (flushTimer) return;
  flushTimer = setTimeout(() => {
    flushTimer = null;
    flushPendingDeltas(get);
  }, 50);
}

export function clearPendingDeltas(): void {
  if (flushTimer) {
    clearTimeout(flushTimer);
    flushTimer = null;
  }
  pendingDeltas.clear();
  flushCallbacks.clear();
}
