import { beforeEach, describe, expect, it } from 'vitest';
import {
  SIMPLE_CHAT_QUEUE_STORAGE_KEY,
  clearPersistedSimpleChatQueue,
  loadPersistedSimpleChatQueue,
  persistSimpleChatQueue,
  snapshotQueueAttachments,
} from './queuePersistence';

describe('queuePersistence utilities', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('persists and restores queue entries', () => {
    const persisted = persistSimpleChatQueue(
      localStorage,
      [
        {
          id: 'q1',
          prompt: 'first',
          submitAsFollowUp: false,
          mode: 'chat',
          attempts: 0,
          attachments: [],
        },
        {
          id: 'q2',
          prompt: 'second',
          submitAsFollowUp: true,
          mode: 'task',
          attempts: 1,
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
        },
      ],
      '/workspace/a',
    );
    expect(persisted).toBe(true);

    const restored = loadPersistedSimpleChatQueue(localStorage, '/workspace/a', 3);
    expect(restored).toEqual([
      { id: 'q1', prompt: 'first', submitAsFollowUp: false, mode: 'chat', attempts: 0, attachments: [] },
      {
        id: 'q2',
        prompt: 'second',
        submitAsFollowUp: true,
        mode: 'task',
        attempts: 1,
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
      },
    ]);
  });

  it('drops persisted queue when workspace mismatches', () => {
    persistSimpleChatQueue(
      localStorage,
      [{ id: 'q1', prompt: 'first', submitAsFollowUp: false, mode: 'chat', attempts: 0, attachments: [] }],
      '/workspace/a',
    );
    const restored = loadPersistedSimpleChatQueue(localStorage, '/workspace/b', 3);
    expect(restored).toEqual([]);
    expect(localStorage.getItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY)).toBeNull();
  });

  it('clears persisted queue', () => {
    persistSimpleChatQueue(
      localStorage,
      [{ id: 'q1', prompt: 'first', submitAsFollowUp: false, mode: 'chat', attempts: 0, attachments: [] }],
      '/workspace/a',
    );
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
      [{ id: 'q1', prompt: 'first', submitAsFollowUp: false, mode: 'chat', attempts: 0, attachments: [] }],
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
});
