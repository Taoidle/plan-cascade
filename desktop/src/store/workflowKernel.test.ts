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
import { useSimpleSessionStore } from './simpleSessionStore';
import type {
  WorkflowKernelUpdatedEvent,
  WorkflowModeTranscriptUpdatedEvent,
  WorkflowSession,
} from '../types/workflowKernel';

function emitKernelEvent(payload: WorkflowKernelUpdatedEvent) {
  const handler = eventHandlers['workflow-kernel-updated'];
  if (!handler) {
    throw new Error('workflow-kernel-updated listener not registered');
  }
  handler({ payload });
}

function emitTranscriptEvent(payload: WorkflowModeTranscriptUpdatedEvent) {
  const handler = eventHandlers['workflow-mode-transcript-updated'];
  if (!handler) {
    throw new Error('workflow-mode-transcript-updated listener not registered');
  }
  handler({ payload });
}

function mockSession(sessionId: string): WorkflowSession {
  return {
    sessionId,
    sessionKind: 'simple_root',
    displayTitle: 'Test session',
    workspacePath: '/tmp/project',
    status: 'active',
    activeMode: 'chat',
    modeSnapshots: {
      chat: {
        phase: 'ready',
        pendingInput: '',
        activeTurnId: null,
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
    backgroundState: 'foreground',
    contextLedger: {
      conversationTurnCount: 0,
      artifactRefCount: 0,
      contextSourceKinds: [],
      lastCompactionAt: null,
      ledgerVersion: 1,
    },
    modeRuntimeMeta: {},
    lastError: null,
    createdAt: '2026-03-02T00:00:00Z',
    updatedAt: '2026-03-02T00:00:00Z',
    lastCheckpointId: null,
  };
}

describe('workflowKernel store', () => {
  beforeEach(() => {
    useWorkflowKernelStore.getState().reset();
    useSimpleSessionStore.getState().reset();
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

  it('marks chat turn failure and applies returned snapshot', async () => {
    const base = mockSession('kernel-2');
    const chatSession: WorkflowSession = {
      ...base,
      activeMode: 'chat',
      modeSnapshots: {
        chat: {
          phase: 'submitting',
          pendingInput: 'hello',
          activeTurnId: 'turn-1',
          turnCount: 1,
          lastUserMessage: 'hello',
          lastAssistantMessage: null,
        },
        plan: null,
        task: null,
      },
    };
    const updatedChatSession: WorkflowSession = {
      ...chatSession,
      modeSnapshots: {
        ...chatSession.modeSnapshots,
        chat: {
          ...chatSession.modeSnapshots.chat!,
          phase: 'failed',
          pendingInput: '',
          activeTurnId: null,
        },
      },
      lastError: 'send failed',
    };

    useWorkflowKernelStore.setState({
      sessionId: 'kernel-2',
      activeMode: 'chat',
      session: chatSession,
    });

    mockInvoke.mockResolvedValueOnce({
      success: true,
      data: updatedChatSession,
      error: null,
    });

    const result = await useWorkflowKernelStore.getState().markChatTurnFailed('send failed');

    expect(result?.modeSnapshots.chat?.phase).toBe('failed');
    expect(mockInvoke).toHaveBeenCalledWith('workflow_mark_chat_turn_failed', {
      sessionId: 'kernel-2',
      error: 'send failed',
    });
  });

  it('loads, appends, and stores mode transcripts through workflow kernel commands', async () => {
    mockInvoke
      .mockResolvedValueOnce({
        success: true,
        data: {
          sessionId: 'kernel-3',
          mode: 'plan',
          revision: 2,
          lines: [{ id: 1, type: 'info', content: 'loaded' }],
        },
        error: null,
      })
      .mockResolvedValueOnce({
        success: true,
        data: {
          sessionId: 'kernel-3',
          mode: 'plan',
          revision: 3,
          lines: [{ id: 2, type: 'info', content: 'appended' }],
        },
        error: null,
      })
      .mockResolvedValueOnce({
        success: true,
        data: {
          sessionId: 'kernel-3',
          mode: 'plan',
          revision: 4,
          lines: [{ id: 1, type: 'info', content: 'stored' }],
        },
        error: null,
      });

    const loaded = await useWorkflowKernelStore.getState().getModeTranscript('kernel-3', 'plan');
    expect(loaded?.revision).toBe(2);
    expect(mockInvoke).toHaveBeenNthCalledWith(1, 'workflow_get_mode_transcript', {
      sessionId: 'kernel-3',
      mode: 'plan',
    });

    const appended = await useWorkflowKernelStore.getState().patchModeTranscript('kernel-3', 'plan', {
      replaceFromLineId: null,
      appendedLines: [{ id: 2, type: 'info', content: 'appended' }],
    });
    expect(appended?.revision).toBe(3);
    expect(mockInvoke).toHaveBeenNthCalledWith(2, 'workflow_patch_mode_transcript', {
      sessionId: 'kernel-3',
      mode: 'plan',
      replaceFromLineId: null,
      appendedLines: [{ id: 2, type: 'info', content: 'appended' }],
    });

    const stored = await useWorkflowKernelStore.getState().patchModeTranscript('kernel-3', 'plan', {
      replaceFromLineId: 0,
      appendedLines: [{ id: 1, type: 'info', content: 'stored' }],
    });
    expect(stored?.revision).toBe(4);
    expect(mockInvoke).toHaveBeenNthCalledWith(3, 'workflow_patch_mode_transcript', {
      sessionId: 'kernel-3',
      mode: 'plan',
      replaceFromLineId: 0,
      appendedLines: [{ id: 1, type: 'info', content: 'stored' }],
    });
    expect(useWorkflowKernelStore.getState().getCachedModeTranscript('kernel-3', 'plan').lines).toEqual([
      { id: 1, type: 'info', content: 'stored' },
    ]);
  });

  it('renames a workflow session through workflow kernel command', async () => {
    useWorkflowKernelStore.setState({
      sessionId: 'kernel-5',
      session: mockSession('kernel-5'),
    });

    mockInvoke.mockResolvedValueOnce({
      success: true,
      data: {
        ...mockSession('kernel-5'),
        displayTitle: 'Renamed session',
      },
      error: null,
    });

    const result = await useWorkflowKernelStore.getState().renameSession('kernel-5', 'Renamed session');

    expect(result?.displayTitle).toBe('Renamed session');
    expect(mockInvoke).toHaveBeenCalledWith('workflow_rename_session', {
      sessionId: 'kernel-5',
      displayTitle: 'Renamed session',
    });
  });

  it('archives a workflow session and updates catalog state', async () => {
    useWorkflowKernelStore.setState({
      sessionId: 'kernel-5',
      activeRootSessionId: 'kernel-5',
      session: mockSession('kernel-5'),
    });

    mockInvoke.mockResolvedValueOnce({
      success: true,
      data: {
        activeSessionId: null,
        sessions: [
          {
            sessionId: 'kernel-5',
            sessionKind: 'simple_root',
            displayTitle: 'Archived session',
            workspacePath: '/tmp/project',
            status: 'archived',
            activeMode: 'chat',
            backgroundState: 'background_idle',
            updatedAt: '2026-03-02T00:00:00Z',
            createdAt: '2026-03-01T00:00:00Z',
            lastError: null,
            contextLedger: {
              conversationTurnCount: 0,
              artifactRefCount: 0,
              contextSourceKinds: [],
              lastCompactionAt: null,
              ledgerVersion: 1,
            },
            modeSnapshots: { chat: null, plan: null, task: null },
            modeRuntimeMeta: {},
          },
        ],
      },
      error: null,
    });

    const result = await useWorkflowKernelStore.getState().archiveSession('kernel-5');

    expect(result?.activeSessionId).toBeNull();
    expect(result?.sessions[0]?.status).toBe('archived');
    expect(mockInvoke).toHaveBeenCalledWith('workflow_archive_session', {
      sessionId: 'kernel-5',
    });
    expect(useWorkflowKernelStore.getState().sessionId).toBeNull();
  });

  it('restores an archived workflow session and hydrates session state', async () => {
    mockInvoke.mockResolvedValueOnce({
      success: true,
      data: {
        session: {
          ...mockSession('kernel-7'),
          status: 'active',
          displayTitle: 'Restored session',
        },
        events: [],
        checkpoints: [],
      },
      error: null,
    });

    const result = await useWorkflowKernelStore.getState().restoreSession('kernel-7');

    expect(result?.session.displayTitle).toBe('Restored session');
    expect(mockInvoke).toHaveBeenCalledWith('workflow_restore_session', {
      sessionId: 'kernel-7',
    });
    expect(useWorkflowKernelStore.getState().sessionId).toBe('kernel-7');
  });

  it('deletes a workflow session and hydrates the next active session when available', async () => {
    useWorkflowKernelStore.setState({
      sessionId: 'kernel-5',
      activeRootSessionId: 'kernel-5',
      session: mockSession('kernel-5'),
    });

    mockInvoke
      .mockResolvedValueOnce({
        success: true,
        data: {
          activeSessionId: 'kernel-6',
          sessions: [
            {
              sessionId: 'kernel-6',
              sessionKind: 'simple_root',
              displayTitle: 'Remaining session',
              workspacePath: '/tmp/project-2',
              status: 'active',
              activeMode: 'plan',
              backgroundState: 'foreground',
              updatedAt: '2026-03-02T00:00:00Z',
              createdAt: '2026-03-01T00:00:00Z',
              lastError: null,
              contextLedger: {
                conversationTurnCount: 0,
                artifactRefCount: 0,
                contextSourceKinds: [],
                lastCompactionAt: null,
                ledgerVersion: 1,
              },
              modeSnapshots: { chat: null, plan: null, task: null },
              modeRuntimeMeta: {},
            },
          ],
        },
        error: null,
      })
      .mockResolvedValueOnce({
        success: true,
        data: {
          session: {
            ...mockSession('kernel-6'),
            activeMode: 'plan',
          },
          events: [],
          checkpoints: [],
        },
        error: null,
      });

    const result = await useWorkflowKernelStore.getState().deleteSession('kernel-5');

    expect(result?.activeSessionId).toBe('kernel-6');
    expect(mockInvoke).toHaveBeenNthCalledWith(1, 'workflow_delete_session', {
      sessionId: 'kernel-5',
      deleteWorktree: null,
    });
    expect(mockInvoke).toHaveBeenNthCalledWith(2, 'workflow_get_session_state', {
      sessionId: 'kernel-6',
    });
    expect(useWorkflowKernelStore.getState().sessionId).toBe('kernel-6');
  });

  it('appends workflow context items through workflow kernel command', async () => {
    useWorkflowKernelStore.setState({
      sessionId: 'kernel-4',
      session: mockSession('kernel-4'),
    });

    mockInvoke.mockResolvedValueOnce({
      success: true,
      data: mockSession('kernel-4'),
      error: null,
    });

    const result = await useWorkflowKernelStore.getState().appendContextItems('chat', {
      conversationContext: [{ user: 'U', assistant: 'A' }],
      artifactRefs: [],
      contextSources: ['chat_transcript_sync'],
      metadata: { source: 'test' },
    });

    expect(result?.sessionId).toBe('kernel-4');
    expect(mockInvoke).toHaveBeenCalledWith('workflow_append_context_items', {
      sessionId: 'kernel-4',
      targetMode: 'chat',
      handoff: {
        conversationContext: [{ user: 'U', assistant: 'A' }],
        summaryItems: [],
        artifactRefs: [],
        contextSources: ['chat_transcript_sync'],
        metadata: { source: 'test' },
      },
    });
  });

  it('applies transcript events to the workflow kernel transcript cache', async () => {
    await useWorkflowKernelStore.getState().subscribeToUpdates();

    emitTranscriptEvent({
      sessionId: 'kernel-4',
      mode: 'task',
      revision: 4,
      appendedLines: [{ id: 1, type: 'info', content: 'background update' }],
      replaceFromLineId: 0,
      source: 'test',
    });

    expect(useWorkflowKernelStore.getState().getCachedModeTranscript('kernel-4', 'task').lines).toEqual([
      { id: 1, type: 'info', content: 'background update' },
    ]);
  });

  it('applies transcript patch events using replaceFromLineId semantics', async () => {
    await useWorkflowKernelStore.getState().subscribeToUpdates();
    useWorkflowKernelStore.setState({
      modeTranscriptsBySession: {
        'kernel-5': {
          chat: {
            revision: 1,
            loaded: true,
            unread: false,
            lines: [
              { id: 1, type: 'info', content: 'user', turnBoundary: 'user', turnId: 1 },
              { id: 2, type: 'text', content: 'old answer', turnId: 1 },
            ],
          },
        },
      },
    });

    emitTranscriptEvent({
      sessionId: 'kernel-5',
      mode: 'chat',
      revision: 2,
      appendedLines: [{ id: 1, type: 'info', content: 'edited user', turnBoundary: 'user', turnId: 1 }],
      replaceFromLineId: 1,
      lines: [{ id: 1, type: 'info', content: 'edited user', turnBoundary: 'user', turnId: 1 }],
      source: 'test',
    });

    expect(useWorkflowKernelStore.getState().getCachedModeTranscript('kernel-5', 'chat').lines).toEqual([
      { id: 1, type: 'info', content: 'edited user', turnBoundary: 'user', turnId: 1 },
    ]);
  });

  it('preserves trailing cards when assistant-side transcript patches arrive', async () => {
    await useWorkflowKernelStore.getState().subscribeToUpdates();
    useWorkflowKernelStore.setState({
      modeTranscriptsBySession: {
        'kernel-6': {
          chat: {
            revision: 1,
            loaded: true,
            unread: false,
            lines: [
              { id: 1, type: 'info', content: 'user', turnBoundary: 'user', turnId: 1 },
              { id: 2, type: 'text', content: 'tool output', turnBoundary: 'assistant', turnId: 1 },
              {
                id: 3,
                type: 'card',
                content: '{"cardType":"file_change"}',
                cardPayload: { cardType: 'file_change', cardId: 'card-1', interactive: false, data: {} },
                turnId: 1,
              },
            ],
          },
        },
      },
    });

    emitTranscriptEvent({
      sessionId: 'kernel-6',
      mode: 'chat',
      revision: 2,
      appendedLines: [
        { id: 2, type: 'text', content: 'tool output updated', turnBoundary: 'assistant', turnId: 1 },
        { id: 4, type: 'tool', content: '[tool] read started', turnId: 1 },
      ],
      replaceFromLineId: 2,
      lines: [
        { id: 1, type: 'info', content: 'user', turnBoundary: 'user', turnId: 1 },
        { id: 2, type: 'text', content: 'tool output updated', turnBoundary: 'assistant', turnId: 1 },
        { id: 4, type: 'tool', content: '[tool] read started', turnId: 1 },
        {
          id: 3,
          type: 'card',
          content: '{"cardType":"file_change"}',
          cardPayload: { cardType: 'file_change', cardId: 'card-1', interactive: false, data: {} },
          turnId: 1,
        },
      ],
      source: 'test',
    });

    expect(useWorkflowKernelStore.getState().getCachedModeTranscript('kernel-6', 'chat').lines).toEqual([
      { id: 1, type: 'info', content: 'user', turnBoundary: 'user', turnId: 1 },
      { id: 2, type: 'text', content: 'tool output updated', turnBoundary: 'assistant', turnId: 1 },
      { id: 4, type: 'tool', content: '[tool] read started', turnId: 1 },
      {
        id: 3,
        type: 'card',
        content: '{"cardType":"file_change"}',
        cardPayload: { cardType: 'file_change', cardId: 'card-1', interactive: false, data: {} },
        turnId: 1,
      },
    ]);
  });
});
