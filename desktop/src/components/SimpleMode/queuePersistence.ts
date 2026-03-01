export interface QueuedChatMessage {
  id: string;
  prompt: string;
  submitAsFollowUp: boolean;
}

interface PersistedQueueV1 {
  version: 1;
  workspacePath: string | null;
  queue: QueuedChatMessage[];
}

export const SIMPLE_CHAT_QUEUE_STORAGE_KEY = 'plan-cascade-simple-chat-queue-v1';

function isQueuedChatMessage(value: unknown): value is QueuedChatMessage {
  if (!value || typeof value !== 'object') return false;
  const candidate = value as Partial<QueuedChatMessage>;
  return (
    typeof candidate.id === 'string' &&
    typeof candidate.prompt === 'string' &&
    typeof candidate.submitAsFollowUp === 'boolean'
  );
}

export function loadPersistedSimpleChatQueue(
  storage: Storage,
  currentWorkspacePath: string,
  maxEntries: number,
): QueuedChatMessage[] {
  try {
    const raw = storage.getItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
    if (!raw) return [];

    const parsed = JSON.parse(raw) as Partial<PersistedQueueV1>;
    if (parsed.version !== 1 || !Array.isArray(parsed.queue)) {
      storage.removeItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
      return [];
    }

    const validQueue = parsed.queue.filter(isQueuedChatMessage).slice(0, maxEntries);
    if (validQueue.length === 0) {
      storage.removeItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
      return [];
    }

    const persistedWorkspace = typeof parsed.workspacePath === 'string' ? parsed.workspacePath : null;
    if (persistedWorkspace && currentWorkspacePath && persistedWorkspace !== currentWorkspacePath) {
      storage.removeItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
      return [];
    }

    return validQueue;
  } catch {
    storage.removeItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
    return [];
  }
}

export function persistSimpleChatQueue(storage: Storage, queue: QueuedChatMessage[], workspacePath: string): void {
  if (queue.length === 0) {
    storage.removeItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
    return;
  }

  const payload: PersistedQueueV1 = {
    version: 1,
    workspacePath: workspacePath || null,
    queue,
  };

  try {
    storage.setItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY, JSON.stringify(payload));
  } catch {
    // Ignore storage failures.
  }
}

export function clearPersistedSimpleChatQueue(storage: Storage): void {
  try {
    storage.removeItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
  } catch {
    // Ignore storage failures.
  }
}
