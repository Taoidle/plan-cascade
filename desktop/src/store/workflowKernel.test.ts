import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockInvoke = vi.fn();
type EventCallback = (event: { payload: unknown }) => void;
const eventHandlers: Record<string, EventCallback> = {};

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation((eventName: string, handler: EventCallback) => {
    eventHandlers[eventName] = handler;
    return Promise.resolve(() => {
      delete eventHandlers[eventName];
    });
  }),
}));

import { useWorkflowKernelStore } from './workflowKernel';
import type { WorkflowKernelUpdatedEvent, WorkflowSession } from '../types/workflowKernel';

function emitKernelEvent(payload: WorkflowKernelUpdatedEvent) {
  const handler = eventHandlers['workflow-kernel-updated'];
  if (!handler) {
    throw new Error('workflow-kernel-updated listener not registered');
  }
  handler({ payload });
}

function mockSession(sessionId: string): WorkflowSession {
  return {
    sessionId,
    status: 'active',
    activeMode: 'chat',
    modeSnapshots: {
      chat: {
        phase: 'ready',
        draftInput: '',
        turnCount: 0,
        lastUserMessage: null,
        lastAssistantMessage: null,
      },
      plan: null,
      task: null,
    },
    handoffContext: {
      conversationContext: [],
      artifactRefs: [],
      contextSources: [],
      metadata: {},
    },
    linkedModeSessions: {},
    lastError: null,
    createdAt: '2026-03-02T00:00:00Z',
    updatedAt: '2026-03-02T00:00:00Z',
    lastCheckpointId: null,
  };
}

describe('workflowKernel store', () => {
  beforeEach(() => {
    useWorkflowKernelStore.getState().reset();
    vi.clearAllMocks();
    Object.keys(eventHandlers).forEach((key) => delete eventHandlers[key]);
  });

  it('updates state from workflow-kernel-updated event', async () => {
    await useWorkflowKernelStore.getState().subscribeToUpdates();

    const session = mockSession('kernel-1');
    emitKernelEvent({
      source: 'test',
      revision: 2,
      sessionState: {
        session,
        events: [],
        checkpoints: [],
      },
    });

    const state = useWorkflowKernelStore.getState();
    expect(state.sessionId).toBe('kernel-1');
    expect(state.activeMode).toBe('chat');
    expect(state.revision).toBe(2);
  });

  it('calls workflow_link_mode_session and updates session', async () => {
    const session = {
      ...mockSession('kernel-1'),
      linkedModeSessions: { task: 'task-session-1' },
    };
    useWorkflowKernelStore.setState({ sessionId: 'kernel-1', session: mockSession('kernel-1') });

    mockInvoke.mockResolvedValueOnce({
      success: true,
      data: session,
      error: null,
    });

    const result = await useWorkflowKernelStore.getState().linkModeSession('task', 'task-session-1');

    expect(result?.linkedModeSessions.task).toBe('task-session-1');
    expect(mockInvoke).toHaveBeenCalledWith('workflow_link_mode_session', {
      sessionId: 'kernel-1',
      mode: 'task',
      modeSessionId: 'task-session-1',
    });
  });
});
