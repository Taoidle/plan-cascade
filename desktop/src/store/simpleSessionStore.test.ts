import { beforeEach, describe, expect, it } from 'vitest';
import { useSimpleSessionStore } from './simpleSessionStore';

describe('simpleSessionStore', () => {
  beforeEach(() => {
    useSimpleSessionStore.getState().reset();
    localStorage.clear();
  });

  it('isolates drafts by root session and mode', () => {
    const store = useSimpleSessionStore.getState();
    store.setDraft('root-a', 'chat', 'chat draft');
    store.setDraft('root-a', 'task', 'task draft');
    store.setDraft('root-b', 'chat', 'other draft');

    expect(store.getDraft('root-a', 'chat')).toBe('chat draft');
    expect(store.getDraft('root-a', 'task')).toBe('task draft');
    expect(store.getDraft('root-b', 'chat')).toBe('other draft');
    expect(store.getDraft('root-b', 'plan')).toBe('');
  });

  it('tracks unread flags per root session and mode', () => {
    const store = useSimpleSessionStore.getState();
    store.markModeUnread('root-a', 'chat', true);
    store.markModeUnread('root-a', 'task', false);

    expect(store.isModeUnread('root-a', 'chat')).toBe(true);
    expect(store.isModeUnread('root-a', 'task')).toBe(false);
    expect(store.isModeUnread('root-b', 'chat')).toBe(false);
  });
});
