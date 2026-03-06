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

  it('stores transcript lines per root session and mode', () => {
    const store = useSimpleSessionStore.getState();
    store.setModeLines('root-a', 'chat', [{ id: 1, content: 'chat' }]);
    store.setModeLines('root-a', 'task', [{ id: 2, content: 'task' }]);

    expect(store.getModeLines('root-a', 'chat')).toEqual([{ id: 1, content: 'chat' }]);
    expect(store.getModeLines('root-a', 'task')).toEqual([{ id: 2, content: 'task' }]);
    expect(store.getModeLines('root-b', 'chat')).toEqual([]);
  });
});
