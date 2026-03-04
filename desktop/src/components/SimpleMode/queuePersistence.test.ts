import { beforeEach, describe, expect, it } from 'vitest';
import {
  SIMPLE_CHAT_QUEUE_STORAGE_KEY,
  clearPersistedSimpleChatQueue,
  loadPersistedSimpleChatQueue,
  loadPersistedSimpleChatQueueWithMeta,
  persistSimpleChatQueue,
  snapshotQueueAttachments,
  type QueuedChatMessage,
} from './queuePersistence';

function createQueuedMessage(overrides?: Partial<QueuedChatMessage>): QueuedChatMessage {
  return {
    id: 'q-base',
    sessionId: 'session-a',
    prompt: 'base',
    submitAsFollowUp: false,
    mode: 'chat',
    attempts: 0,
    attachments: [],
    priority: 'normal',
    status: 'pending',
    enqueueSeq: 0,
    createdAt: new Date('2026-01-01T00:00:00.000Z').toISOString(),
    lastError: null,
    ...overrides,
  };
}

describe('queuePersistence utilities', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('persists and restores queue entries', () => {
    const sourceQueue: QueuedChatMessage[] = [
      createQueuedMessage({
        id: 'q1',
        prompt: 'first',
        enqueueSeq: 1,
      }),
      createQueuedMessage({
        id: 'q2',
        prompt: 'second',
        submitAsFollowUp: true,
        mode: 'task',
        attempts: 1,
        status: 'failed',
        lastError: 'failed-once',
        enqueueSeq: 2,
        attachments: [
          {
            id: 'att-1',
            name: 'spec.md',
            path: '/tmp/spec.md',
            size: 32,
            type: 'text',
            content: '# spec',
          },
        ],
      }),
    ];

    const persisted = persistSimpleChatQueue(localStorage, sourceQueue, '/workspace/a');
    expect(persisted).toBe(true);

    const restored = loadPersistedSimpleChatQueue(localStorage, '/workspace/a', 3, 'session-a');
    expect(restored).toEqual(sourceQueue);
  });

  it('drops persisted queue when workspace mismatches', () => {
    persistSimpleChatQueue(localStorage, [createQueuedMessage({ id: 'q1', prompt: 'first' })], '/workspace/a');
    const restored = loadPersistedSimpleChatQueue(localStorage, '/workspace/b', 3);
    expect(restored).toEqual([]);
    expect(localStorage.getItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY)).toBeNull();
  });

  it('clears persisted queue', () => {
    persistSimpleChatQueue(localStorage, [createQueuedMessage({ id: 'q1', prompt: 'first' })], '/workspace/a');
    clearPersistedSimpleChatQueue(localStorage);
    expect(localStorage.getItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY)).toBeNull();
  });

  it('returns false when queue persistence throws', () => {
    const storage = {
      getItem: (_key: string) => null,
      setItem: (_key: string, _value: string) => {
        throw new Error('quota exceeded');
      },
      removeItem: (_key: string) => {},
      clear: () => {},
      key: (_index: number) => null,
      length: 0,
    } as Storage;

    const persisted = persistSimpleChatQueue(
      storage,
      [createQueuedMessage({ id: 'q1', prompt: 'first' })],
      '/workspace/a',
    );

    expect(persisted).toBe(false);
  });

  it('drops non-serializable attachments when creating queue snapshots', () => {
    const circular = { id: 'bad' } as unknown as { self?: unknown };
    circular.self = circular;

    const { attachments, droppedCount } = snapshotQueueAttachments([
      {
        id: 'ok',
        name: 'a.txt',
        path: '/tmp/a.txt',
        size: 10,
        type: 'text',
        content: 'ok',
      },
      {
        id: 'bad',
        name: 'bad.txt',
        path: '/tmp/bad.txt',
        size: 10,
        type: 'text',
        content: circular as unknown as string,
      },
    ]);

    expect(attachments).toHaveLength(1);
    expect(attachments[0].id).toBe('ok');
    expect(droppedCount).toBe(1);
  });

  it('hydrates legacy v3 queue with fallback session id', () => {
    localStorage.setItem(
      'plan-cascade-simple-chat-queue-v3',
      JSON.stringify({
        version: 3,
        workspacePath: '/workspace/a',
        queue: [
          {
            id: 'legacy-1',
            prompt: 'legacy prompt',
            submitAsFollowUp: false,
            mode: 'chat',
            attempts: 0,
            attachments: [],
          },
        ],
      }),
    );

    const restored = loadPersistedSimpleChatQueue(localStorage, '/workspace/a', 3, 'legacy-session');
    expect(restored).toHaveLength(1);
    expect(restored[0].sessionId).toBe('legacy-session');
    expect(restored[0].priority).toBe('normal');
    expect(restored[0].status).toBe('pending');
  });

  it('returns migration and cross-session metadata', () => {
    localStorage.setItem(
      SIMPLE_CHAT_QUEUE_STORAGE_KEY,
      JSON.stringify({
        version: 4,
        workspacePath: '/workspace/a',
        queue: [
          createQueuedMessage({ id: 'q-session-a', sessionId: 'session-a' }),
          createQueuedMessage({ id: 'q-session-b', sessionId: 'session-b', enqueueSeq: 1 }),
        ],
      }),
    );

    const restored = loadPersistedSimpleChatQueueWithMeta(localStorage, '/workspace/a', 10, 'session-a');
    expect(restored.sourceVersion).toBe(4);
    expect(restored.migratedFromVersion).toBeNull();
    expect(restored.crossSessionCount).toBe(1);
    expect(restored.queue).toHaveLength(2);
  });
});
