import { describe, expect, it } from 'vitest';
import type { ExecutionHistoryItem } from '../../store/execution';
import type { WorkflowSessionCatalogItem } from '../../types/workflowKernel';
import {
  buildSessionTreeViewModel,
  deriveSidebarSessionStatus,
  normalizeSidebarSessionTitle,
} from './sessionTreeViewModel';

function createHistoryItem(overrides: Partial<ExecutionHistoryItem> = {}): ExecutionHistoryItem {
  return {
    id: overrides.id ?? 'history-1',
    title: overrides.title,
    taskDescription: overrides.taskDescription ?? 'History session',
    workspacePath: overrides.workspacePath ?? '/repo/app',
    strategy: overrides.strategy ?? 'direct',
    status: overrides.status ?? 'completed',
    startedAt: overrides.startedAt ?? Date.now() - 30_000,
    completedAt: overrides.completedAt,
    duration: overrides.duration ?? 1000,
    completedStories: overrides.completedStories ?? 1,
    totalStories: overrides.totalStories ?? 1,
    success: overrides.success ?? true,
    error: overrides.error,
    conversationContent: overrides.conversationContent,
    conversationLines: overrides.conversationLines,
    sessionId: overrides.sessionId,
    llmBackend: overrides.llmBackend,
    llmProvider: overrides.llmProvider,
    llmModel: overrides.llmModel,
  };
}

function createWorkflowSession(overrides: Partial<WorkflowSessionCatalogItem> = {}): WorkflowSessionCatalogItem {
  return {
    sessionId: overrides.sessionId ?? 'root-1',
    sessionKind: 'simple_root',
    displayTitle: overrides.displayTitle ?? 'New chat',
    workspacePath: 'workspacePath' in overrides ? (overrides.workspacePath ?? null) : '/repo/app',
    status: overrides.status ?? 'active',
    activeMode: overrides.activeMode ?? 'chat',
    backgroundState: overrides.backgroundState ?? 'background_idle',
    createdAt: overrides.createdAt ?? new Date(Date.now() - 60_000).toISOString(),
    updatedAt: overrides.updatedAt ?? new Date().toISOString(),
    lastError: overrides.lastError ?? null,
    contextLedger: overrides.contextLedger ?? {
      conversationTurnCount: 2,
      artifactRefCount: 0,
      contextSourceKinds: ['simple_mode'],
      lastCompactionAt: null,
      ledgerVersion: 1,
    },
    modeSnapshots: overrides.modeSnapshots ?? {
      chat: {
        phase: 'ready',
        pendingInput: '',
        activeTurnId: null,
        turnCount: 2,
        lastUserMessage: 'Build the new sidebar tree for session management',
        lastAssistantMessage: null,
      },
      plan: {
        phase: 'idle',
        planId: null,
        runningStepId: null,
        pendingClarification: null,
        retryableSteps: [],
        planRevision: 0,
        lastEditOperation: null,
      },
      task: {
        phase: 'idle',
        prdId: null,
        currentStoryId: null,
        interviewSessionId: null,
        pendingInterview: null,
        completedStories: 0,
        failedStories: 0,
      },
    },
    modeRuntimeMeta: overrides.modeRuntimeMeta ?? {},
  };
}

describe('sessionTreeViewModel', () => {
  it('groups live and history sessions by pinned path and falls back to No Workspace', () => {
    const groups = buildSessionTreeViewModel({
      pinnedDirectories: ['/repo'],
      workflowSessions: [
        createWorkflowSession({ sessionId: 'live-1', workspacePath: '/repo/app' }),
        createWorkflowSession({
          sessionId: 'live-2',
          workspacePath: null,
          updatedAt: new Date(Date.now() - 10_000).toISOString(),
        }),
      ],
      history: [createHistoryItem({ id: 'history-1', title: 'External', workspacePath: '/other/project' })],
      activeSessionId: 'live-1',
    });

    expect(groups.map((group) => group.label)).toEqual(['repo', 'No Workspace', 'project']);
    expect(groups[0].children.map((item) => item.id)).toContain('live:live-1');
    expect(groups[1].children[0]?.workspacePath).toBeNull();
    expect(groups[2].children[0]?.id).toBe('history:history-1');
  });

  it('sorts sessions within a path by updated time descending', () => {
    const groups = buildSessionTreeViewModel({
      workflowSessions: [
        createWorkflowSession({
          sessionId: 'older',
          displayTitle: 'Older',
          workspacePath: '/repo/app',
          updatedAt: '2026-03-06T08:00:00.000Z',
        }),
        createWorkflowSession({
          sessionId: 'newer',
          displayTitle: 'Newer',
          workspacePath: '/repo/app',
          updatedAt: '2026-03-06T09:00:00.000Z',
        }),
      ],
    });

    expect(groups).toHaveLength(1);
    expect(groups[0].children.map((item) => item.title)).toEqual(['Newer', 'Older']);
  });

  it('filters archived sessions by default and can include them when requested', () => {
    const base = createWorkflowSession({
      sessionId: 'archived-1',
      status: 'archived',
      displayTitle: 'Old archived session',
    });

    expect(
      buildSessionTreeViewModel({
        workflowSessions: [base],
      }),
    ).toHaveLength(0);

    const groups = buildSessionTreeViewModel({
      workflowSessions: [base],
      includeArchived: true,
    });
    expect(groups).toHaveLength(1);
    expect(groups[0].children[0]?.kind).toBe('archived');
  });

  it('deduplicates history items that mirror existing workflow sessions', () => {
    const groups = buildSessionTreeViewModel({
      workflowSessions: [
        createWorkflowSession({
          sessionId: 'live-1',
          displayTitle: 'Billing session',
          workspacePath: '/repo/app',
        }),
      ],
      history: [
        createHistoryItem({
          id: 'history-1',
          title: 'Billing session',
          workspacePath: '/repo/app',
        }),
      ],
    });

    expect(groups).toHaveLength(1);
    expect(groups[0].children).toHaveLength(1);
    expect(groups[0].children[0]?.kind).toBe('live');
  });

  it('sorts path groups by name when requested', () => {
    const groups = buildSessionTreeViewModel({
      workflowSessions: [
        createWorkflowSession({ sessionId: 'b', workspacePath: '/repo/zebra', displayTitle: 'Zebra' }),
        createWorkflowSession({ sessionId: 'a', workspacePath: '/repo/alpha', displayTitle: 'Alpha' }),
      ],
      pathSort: 'name',
    });

    expect(groups.map((group) => group.label)).toEqual(['alpha', 'zebra']);
  });

  it('maps runtime states into stable sidebar statuses', () => {
    expect(
      deriveSidebarSessionStatus({
        modeSnapshots: createWorkflowSession({
          backgroundState: 'background_idle',
          modeSnapshots: {
            chat: {
              phase: 'streaming',
              pendingInput: '',
              activeTurnId: null,
              turnCount: 0,
              lastUserMessage: null,
              lastAssistantMessage: null,
            },
            plan: {
              phase: 'idle',
              planId: null,
              runningStepId: null,
              pendingClarification: null,
              retryableSteps: [],
              planRevision: 0,
              lastEditOperation: null,
            },
            task: {
              phase: 'idle',
              prdId: null,
              currentStoryId: null,
              interviewSessionId: null,
              pendingInterview: null,
              completedStories: 0,
              failedStories: 0,
            },
          },
        }).modeSnapshots,
        backgroundState: 'background_idle',
        lastError: null,
        activeMode: 'chat',
      }),
    ).toBe('running');

    expect(
      deriveSidebarSessionStatus({
        modeSnapshots: createWorkflowSession({
          modeSnapshots: {
            chat: {
              phase: 'failed',
              pendingInput: '',
              activeTurnId: null,
              turnCount: 0,
              lastUserMessage: null,
              lastAssistantMessage: null,
            },
            plan: {
              phase: 'idle',
              planId: null,
              runningStepId: null,
              pendingClarification: null,
              retryableSteps: [],
              planRevision: 0,
              lastEditOperation: null,
            },
            task: {
              phase: 'idle',
              prdId: null,
              currentStoryId: null,
              interviewSessionId: null,
              pendingInterview: null,
              completedStories: 0,
              failedStories: 0,
            },
          },
        }).modeSnapshots,
        backgroundState: 'background_idle',
        lastError: null,
        activeMode: 'chat',
      }),
    ).toBe('attention');

    expect(
      deriveSidebarSessionStatus({
        modeSnapshots: createWorkflowSession().modeSnapshots,
        backgroundState: 'background_idle',
        lastError: null,
        activeMode: 'chat',
      }),
    ).toBe('idle');
  });

  it('normalizes placeholder titles while preserving explicit titles', () => {
    expect(
      normalizeSidebarSessionTitle('New chat', {
        activeMode: 'chat',
        modeSnapshots: createWorkflowSession().modeSnapshots,
      }),
    ).toContain('Build the new sidebar tree');

    expect(
      normalizeSidebarSessionTitle('Custom billing session', {
        activeMode: 'task',
        modeSnapshots: createWorkflowSession().modeSnapshots,
      }),
    ).toBe('Custom billing session');
  });
});
