import { beforeEach, describe, expect, it } from 'vitest';
import { ToolCallStreamFilter } from '../../utils/toolCallFilter';
import { useSettingsStore } from '../settings';
import { createSessionPersistenceController } from './sessionPersistence';
import type { ExecutionState } from './types';

const SESSION_KEY = 'execution-session-persistence-test';

function createExecutionState(overrides: Partial<ExecutionState> = {}): ExecutionState {
  return {
    status: 'idle',
    connectionStatus: 'connected',
    taskId: null,
    activeExecutionId: null,
    isCancelling: false,
    pendingCancelBeforeSessionReady: false,
    taskDescription: '',
    strategy: null,
    stories: [],
    batches: [],
    currentBatch: 0,
    currentStoryId: null,
    progress: 0,
    result: null,
    startedAt: null,
    logs: [],
    history: [],
    isSubmitting: false,
    apiError: null,
    strategyAnalysis: null,
    isAnalyzingStrategy: false,
    strategyOptions: [],
    streamingOutput: [],
    streamLineCounter: 0,
    currentTurnStartLineId: 0,
    analysisCoverage: null,
    qualityGateResults: [],
    executionErrors: [],
    estimatedTimeRemaining: null,
    isChatSession: false,
    standaloneTurns: [],
    standaloneSessionId: null,
    latestUsage: null,
    sessionUsageTotals: null,
    turnUsageTotals: null,
    toolCallFilter: new ToolCallStreamFilter(),
    attachments: [],
    workspaceReferences: [],
    backgroundSessions: {},
    runtimeRegistry: {},
    activeRuntimeHandleId: null,
    activeSessionId: null,
    foregroundParentSessionId: null,
    foregroundBgId: null,
    foregroundOriginHistoryId: null,
    foregroundOriginSessionId: null,
    foregroundDirty: false,
    activeAgentId: null,
    activeAgentName: null,
    addAttachment: () => undefined,
    removeAttachment: () => undefined,
    clearAttachments: () => undefined,
    setWorkspaceReferences: () => undefined,
    clearWorkspaceReferences: () => undefined,
    backgroundCurrentSession: () => undefined,
    parkForegroundRuntime: () => null,
    restoreForegroundChatRuntime: () => undefined,
    switchToSession: () => undefined,
    removeBackgroundSession: () => undefined,
    initialize: () => undefined,
    cleanup: () => undefined,
    start: async () => undefined,
    pause: async () => undefined,
    resume: async () => undefined,
    cancel: async () => undefined,
    sendFollowUp: async () => undefined,
    reset: () => undefined,
    updateStory: () => undefined,
    addLog: () => undefined,
    setStories: () => undefined,
    setStrategy: () => undefined,
    loadHistory: () => undefined,
    saveToHistory: () => undefined,
    clearHistory: () => undefined,
    deleteHistory: () => undefined,
    renameHistory: () => undefined,
    restoreFromHistory: () => undefined,
    analyzeStrategy: async () => null,
    loadStrategyOptions: async () => undefined,
    clearStrategyAnalysis: () => undefined,
    appendStreamLine: () => undefined,
    appendCard: () => undefined,
    clearStreamingOutput: () => undefined,
    updateQualityGate: () => undefined,
    addExecutionError: () => undefined,
    dismissError: () => undefined,
    clearExecutionErrors: () => undefined,
    retryStory: async () => undefined,
    rollbackToTurn: () => undefined,
    forkSessionAtTurn: () => undefined,
    regenerateResponse: async () => undefined,
    editAndResend: async () => undefined,
    appendStandaloneTurn: () => undefined,
    ...overrides,
  };
}

describe('sessionPersistence', () => {
  beforeEach(() => {
    localStorage.removeItem(SESSION_KEY);
    useSettingsStore.setState({
      backend: 'openai',
      provider: 'openai',
      model: 'gpt-4o',
      workspacePath: '/tmp/project',
    });
  });

  it('roundtrips V2 runtime registry and legacy session tree', () => {
    const controller = createSessionPersistenceController({
      sessionStateKey: SESSION_KEY,
      hasMeaningfulForegroundContent: (state) => state.streamingOutput.length > 0,
      buildHistorySessionId: () => null,
    });

    controller.persistNow(
      createExecutionState({
        taskDescription: 'Foreground chat',
        status: 'running',
        taskId: 'chat-1',
        isChatSession: true,
        streamingOutput: [{ id: 1, content: 'hello', type: 'text', timestamp: 1000 }],
        streamLineCounter: 1,
        runtimeRegistry: {
          'claude:chat-1': {
            id: 'claude:chat-1',
            source: 'claude',
            rawSessionId: 'chat-1',
            rootSessionId: 'root-1',
            mode: 'chat',
            status: 'running',
            streamingOutput: [{ id: 1, content: 'hello', type: 'text', timestamp: 1000 }],
            streamLineCounter: 1,
            currentTurnStartLineId: 0,
            standaloneTurns: [],
            latestUsage: { input_tokens: 1, output_tokens: 2 },
            sessionUsageTotals: { input_tokens: 1, output_tokens: 2 },
            startedAt: 1000,
            workspacePath: '/tmp/project',
            llmBackend: 'openai',
            llmProvider: 'openai',
            llmModel: 'gpt-4o',
            updatedAt: 1000,
          },
        },
        activeRuntimeHandleId: 'claude:chat-1',
        backgroundSessions: {
          'bg-legacy': {
            id: 'bg-legacy',
            taskDescription: 'Legacy branch',
            status: 'idle',
            streamingOutput: [],
            streamLineCounter: 0,
            currentTurnStartLineId: 0,
            taskId: null,
            isChatSession: false,
            standaloneTurns: [],
            standaloneSessionId: 'standalone-1',
            latestUsage: null,
            sessionUsageTotals: null,
            startedAt: 500,
            toolCallFilter: new ToolCallStreamFilter(),
            llmBackend: 'openai',
            llmProvider: 'openai',
            llmModel: 'gpt-4o',
          },
        },
        activeSessionId: 'bg-legacy',
      }),
    );

    const raw = JSON.parse(localStorage.getItem(SESSION_KEY) || '{}') as { version?: number };
    expect(raw.version).toBe(2);

    const restored = controller.load();
    expect(restored?.state.runtimeRegistry?.['claude:chat-1']).toBeDefined();
    expect(restored?.state.activeRuntimeHandleId).toBe('claude:chat-1');
    expect(restored?.state.backgroundSessions?.['bg-legacy']).toBeDefined();
    expect(restored?.state.activeSessionId).toBe('bg-legacy');
    expect(restored?.state.taskId).toBe('chat-1');
    expect(restored?.foregroundWorkspacePath).toBe('/tmp/project');
  });

  it('does not persist chat snapshots into legacy session tree when mirrored in runtimeRegistry', () => {
    const controller = createSessionPersistenceController({
      sessionStateKey: SESSION_KEY,
      hasMeaningfulForegroundContent: (state) => state.streamingOutput.length > 0,
      buildHistorySessionId: () => null,
    });

    controller.persistNow(
      createExecutionState({
        runtimeRegistry: {
          'claude:chat-1': {
            id: 'claude:chat-1',
            source: 'claude',
            rawSessionId: 'chat-1',
            rootSessionId: 'root-1',
            mode: 'chat',
            status: 'idle',
            streamingOutput: [{ id: 1, content: 'hello', type: 'text', timestamp: 1000 }],
            streamLineCounter: 1,
            currentTurnStartLineId: 0,
            standaloneTurns: [],
            latestUsage: null,
            sessionUsageTotals: null,
            startedAt: 1000,
            workspacePath: '/tmp/project',
            llmBackend: 'openai',
            llmProvider: 'openai',
            llmModel: 'gpt-4o',
            updatedAt: 1000,
          },
        },
        backgroundSessions: {
          'bg-chat-legacy': {
            id: 'bg-chat-legacy',
            taskDescription: 'Mirrored chat',
            status: 'idle',
            streamingOutput: [{ id: 1, content: 'hello', type: 'text', timestamp: 1000 }],
            streamLineCounter: 1,
            currentTurnStartLineId: 0,
            taskId: 'chat-1',
            isChatSession: true,
            standaloneTurns: [],
            standaloneSessionId: null,
            latestUsage: null,
            sessionUsageTotals: null,
            startedAt: 1000,
            toolCallFilter: new ToolCallStreamFilter(),
            llmBackend: 'openai',
            llmProvider: 'openai',
            llmModel: 'gpt-4o',
          },
          'bg-legacy': {
            id: 'bg-legacy',
            taskDescription: 'Legacy branch',
            status: 'idle',
            streamingOutput: [],
            streamLineCounter: 0,
            currentTurnStartLineId: 0,
            taskId: null,
            isChatSession: false,
            standaloneTurns: [],
            standaloneSessionId: 'standalone-1',
            latestUsage: null,
            sessionUsageTotals: null,
            startedAt: 500,
            toolCallFilter: new ToolCallStreamFilter(),
            llmBackend: 'openai',
            llmProvider: 'openai',
            llmModel: 'gpt-4o',
          },
        },
        activeSessionId: 'bg-chat-legacy',
      }),
    );

    const raw = JSON.parse(localStorage.getItem(SESSION_KEY) || '{}') as {
      legacySessionTree?: { backgroundSessions?: Record<string, unknown>; activeSessionId?: string | null };
    };
    expect(raw.legacySessionTree?.backgroundSessions).toEqual({
      'bg-legacy': expect.any(Object),
    });
    expect(raw.legacySessionTree?.activeSessionId).toBeNull();
  });

  it('migrates V1 chat snapshots into runtimeRegistry on load', () => {
    localStorage.setItem(
      SESSION_KEY,
      JSON.stringify({
        version: 1,
        activeSessionId: 'bg-chat-1',
        backgroundSessions: {
          'bg-chat-1': {
            id: 'bg-chat-1',
            taskDescription: 'Background chat',
            status: 'running',
            streamingOutput: [{ id: 1, content: 'hello', type: 'text', timestamp: 1000 }],
            streamLineCounter: 1,
            currentTurnStartLineId: 0,
            taskId: 'chat-1',
            isChatSession: true,
            standaloneTurns: [],
            standaloneSessionId: null,
            latestUsage: null,
            sessionUsageTotals: null,
            startedAt: 1000,
            llmBackend: 'openai',
            llmProvider: 'openai',
            llmModel: 'gpt-4o',
          },
        },
        foreground: null,
      }),
    );

    const controller = createSessionPersistenceController({
      sessionStateKey: SESSION_KEY,
      hasMeaningfulForegroundContent: (state) => state.streamingOutput.length > 0,
      buildHistorySessionId: () => null,
    });

    const restored = controller.load();
    const runtime = restored?.state.runtimeRegistry?.['claude:chat-1'];
    expect(runtime).toBeDefined();
    expect(runtime?.rawSessionId).toBe('chat-1');
    expect(restored?.state.backgroundSessions?.['bg-chat-1']).toBeDefined();
    expect(restored?.state.activeSessionId).toBe('bg-chat-1');
  });
});
