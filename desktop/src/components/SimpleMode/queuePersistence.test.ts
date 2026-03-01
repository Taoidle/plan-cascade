import { beforeEach, describe, expect, it } from 'vitest';
import {
  SIMPLE_CHAT_QUEUE_STORAGE_KEY,
  clearPersistedSimpleChatQueue,
  loadPersistedSimpleChatQueue,
  persistSimpleChatQueue,
} from './queuePersistence';

describe('queuePersistence utilities', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('persists and restores queue entries', () => {
    persistSimpleChatQueue(
      localStorage,
      [
        { id: 'q1', prompt: 'first', submitAsFollowUp: false },
        { id: 'q2', prompt: 'second', submitAsFollowUp: true },
      ],
      '/workspace/a',
    );

    const restored = loadPersistedSimpleChatQueue(localStorage, '/workspace/a', 3);
    expect(restored).toEqual([
      { id: 'q1', prompt: 'first', submitAsFollowUp: false },
      { id: 'q2', prompt: 'second', submitAsFollowUp: true },
    ]);
  });

  it('drops persisted queue when workspace mismatches', () => {
    persistSimpleChatQueue(localStorage, [{ id: 'q1', prompt: 'first', submitAsFollowUp: false }], '/workspace/a');
    const restored = loadPersistedSimpleChatQueue(localStorage, '/workspace/b', 3);
    expect(restored).toEqual([]);
    expect(localStorage.getItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY)).toBeNull();
  });

  it('clears persisted queue', () => {
    persistSimpleChatQueue(localStorage, [{ id: 'q1', prompt: 'first', submitAsFollowUp: false }], '/workspace/a');
    clearPersistedSimpleChatQueue(localStorage);
    expect(localStorage.getItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY)).toBeNull();
  });
});
