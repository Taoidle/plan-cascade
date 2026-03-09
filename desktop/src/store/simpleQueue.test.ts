import { beforeEach, describe, expect, it } from 'vitest';
import { selectNextQueueDispatchItem, useSimpleQueueStore } from './simpleQueue';

function resetStore() {
  useSimpleQueueStore.setState({
    items: [],
    nextEnqueueSeq: 0,
  });
}

describe('simpleQueue store', () => {
  beforeEach(() => {
    resetStore();
  });

  it('prioritizes high priority items for dispatch', () => {
    const store = useSimpleQueueStore.getState();
    store.enqueue({
      sessionId: 'session-1',
      prompt: 'normal',
      submitAsFollowUp: true,
      mode: 'chat',
      attachments: [],
      references: [],
      priority: 'normal',
    });
    store.enqueue({
      sessionId: 'session-1',
      prompt: 'high',
      submitAsFollowUp: true,
      mode: 'chat',
      attachments: [],
      references: [],
      priority: 'high',
    });

    const next = selectNextQueueDispatchItem(useSimpleQueueStore.getState().items, 'session-1');
    expect(next?.prompt).toBe('high');
  });

  it('supports move top and move bottom within the same session', () => {
    const store = useSimpleQueueStore.getState();
    const first = store.enqueue({
      sessionId: 'session-1',
      prompt: 'first',
      submitAsFollowUp: true,
      mode: 'chat',
      attachments: [],
      references: [],
    }).item;
    const second = store.enqueue({
      sessionId: 'session-1',
      prompt: 'second',
      submitAsFollowUp: true,
      mode: 'chat',
      attachments: [],
      references: [],
    }).item;
    const third = store.enqueue({
      sessionId: 'session-1',
      prompt: 'third',
      submitAsFollowUp: true,
      mode: 'chat',
      attachments: [],
      references: [],
    }).item;

    expect(first && second && third).toBeTruthy();
    if (!first || !second || !third) return;

    store.move(third.id, 'top');
    let prompts = useSimpleQueueStore
      .getState()
      .items.filter((item) => item.sessionId === 'session-1')
      .sort((a, b) => a.enqueueSeq - b.enqueueSeq)
      .map((item) => item.prompt);
    expect(prompts).toEqual(['third', 'first', 'second']);

    store.move(third.id, 'bottom');
    prompts = useSimpleQueueStore
      .getState()
      .items.filter((item) => item.sessionId === 'session-1')
      .sort((a, b) => a.enqueueSeq - b.enqueueSeq)
      .map((item) => item.prompt);
    expect(prompts).toEqual(['first', 'second', 'third']);
  });
});
