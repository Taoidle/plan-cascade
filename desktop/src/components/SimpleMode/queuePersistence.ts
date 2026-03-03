export interface QueuedChatMessage {
  id: string;
  prompt: string;
  submitAsFollowUp: boolean;
  mode: 'chat' | 'plan' | 'task';
  attempts: number;
}

interface PersistedQueueV1 {
  version: 1;
  workspacePath: string | null;
  queue: Array<{
    id: string;
    prompt: string;
    submitAsFollowUp: boolean;
  }>;
}

interface PersistedQueueV2 {
  version: 2;
  workspacePath: string | null;
  queue: QueuedChatMessage[];
}

export const SIMPLE_CHAT_QUEUE_STORAGE_KEY = 'plan-cascade-simple-chat-queue-v2';

function isQueuedChatMessage(value: unknown): value is QueuedChatMessage {
  if (!value || typeof value !== 'object') return false;
  const candidate = value as Partial<QueuedChatMessage>;
  return (
    typeof candidate.id === 'string' &&
    typeof candidate.prompt === 'string' &&
    typeof candidate.submitAsFollowUp === 'boolean' &&
    (candidate.mode === 'chat' || candidate.mode === 'plan' || candidate.mode === 'task') &&
    typeof candidate.attempts === 'number'
  );
}

export function loadPersistedSimpleChatQueue(
  storage: Storage,
  currentWorkspacePath: string,
  maxEntries: number,
): QueuedChatMessage[] {
  try {
    const raw = storage.getItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
    const rawV1 = storage.getItem('plan-cascade-simple-chat-queue-v1');
    if (!raw && !rawV1) return [];

    const parsed = JSON.parse(raw ?? rawV1 ?? '{}') as {
      version?: number;
      workspacePath?: string | null;
      queue?: unknown[];
    };
    let normalizedQueue: QueuedChatMessage[] = [];

    if (parsed.version === 2 && Array.isArray(parsed.queue)) {
      normalizedQueue = parsed.queue.filter(isQueuedChatMessage);
    } else if (parsed.version === 1 && Array.isArray(parsed.queue)) {
      normalizedQueue = parsed.queue
        .filter(
          (entry): entry is PersistedQueueV1['queue'][number] =>
            !!entry &&
            typeof entry === 'object' &&
            typeof (entry as { id?: unknown }).id === 'string' &&
            typeof (entry as { prompt?: unknown }).prompt === 'string' &&
            typeof (entry as { submitAsFollowUp?: unknown }).submitAsFollowUp === 'boolean',
        )
        .map((entry) => ({
          id: entry.id,
          prompt: entry.prompt,
          submitAsFollowUp: entry.submitAsFollowUp,
          mode: 'chat',
          attempts: 0,
        }));
    }

    const validQueue = normalizedQueue.slice(0, maxEntries);
    if (validQueue.length === 0) {
      storage.removeItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
      storage.removeItem('plan-cascade-simple-chat-queue-v1');
      return [];
    }

    const persistedWorkspace = typeof parsed.workspacePath === 'string' ? parsed.workspacePath : null;
    if (persistedWorkspace && currentWorkspacePath && persistedWorkspace !== currentWorkspacePath) {
      storage.removeItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
      storage.removeItem('plan-cascade-simple-chat-queue-v1');
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
    storage.removeItem('plan-cascade-simple-chat-queue-v1');
    return;
  }

  const payload: PersistedQueueV2 = {
    version: 2,
    workspacePath: workspacePath || null,
    queue,
  };

  try {
    storage.setItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY, JSON.stringify(payload));
    storage.removeItem('plan-cascade-simple-chat-queue-v1');
  } catch {
    // Ignore storage failures.
  }
}

export function clearPersistedSimpleChatQueue(storage: Storage): void {
  try {
    storage.removeItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
    storage.removeItem('plan-cascade-simple-chat-queue-v1');
  } catch {
    // Ignore storage failures.
  }
}
