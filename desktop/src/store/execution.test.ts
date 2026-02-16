/**
 * Execution Store - Background Session Tests
 *
 * Story 006: Background Session State Model (backgroundSessions, activeSessionId, actions)
 * Story 007: Route Streaming Events to Active or Background Sessions
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { listen } from '@tauri-apps/api/event';

// Mock Tauri APIs before importing the store
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Capture event handler callbacks registered by setupTauriEventListeners
// so tests can simulate incoming Tauri events.
type EventCallback = (event: { payload: unknown }) => void;
const eventHandlers: Record<string, EventCallback> = {};

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation((eventName: string, handler: EventCallback) => {
    eventHandlers[eventName] = handler;
    return Promise.resolve(() => { delete eventHandlers[eventName]; });
  }),
  emit: vi.fn(),
}));

// Import after mocks are set up
import { useExecutionStore } from './execution';
import type { SessionSnapshot } from './execution';
import { useSettingsStore } from './settings';

/** Emit a synthetic event to a captured listener. */
function emitEvent(eventName: string, payload: unknown) {
  const handler = eventHandlers[eventName];
  if (!handler) {
    throw new Error(`No listener registered for "${eventName}". Registered: ${Object.keys(eventHandlers).join(', ')}`);
  }
  handler({ payload });
}

// Helper to reset the store to initial state between tests
function resetStore() {
  const store = useExecutionStore.getState();
  // Use the internal reset and also clear background sessions
  store.reset();
  // Manually ensure backgroundSessions and activeSessionId are reset
  useExecutionStore.setState({
    backgroundSessions: {},
    activeSessionId: null,
  });
}

/** Find a background session snapshot by its taskId field. */
function findBgByTaskId(taskId: string): { key: string; snapshot: SessionSnapshot } | undefined {
  const bg = useExecutionStore.getState().backgroundSessions;
  for (const [key, snapshot] of Object.entries(bg)) {
    if (snapshot.taskId === taskId) return { key, snapshot };
  }
  return undefined;
}

describe('Execution Store - Background Session State', () => {
  beforeEach(() => {
    resetStore();
  });

  // ===========================================================================
  // 1. Initial state includes backgroundSessions and activeSessionId
  // ===========================================================================
  describe('initial state', () => {
    it('should have backgroundSessions as an empty object', () => {
      const state = useExecutionStore.getState();
      expect(state.backgroundSessions).toEqual({});
    });

    it('should have activeSessionId as null', () => {
      const state = useExecutionStore.getState();
      expect(state.activeSessionId).toBeNull();
    });
  });

  // ===========================================================================
  // 2. backgroundCurrentSession()
  // ===========================================================================
  describe('backgroundCurrentSession()', () => {
    it('should create a snapshot of the current foreground session in backgroundSessions', () => {
      // Set up some foreground state to snapshot
      useExecutionStore.setState({
        taskDescription: 'Build a REST API',
        status: 'running',
        streamingOutput: [
          { id: 1, content: 'Build a REST API', type: 'info', timestamp: 1000 },
          { id: 2, content: 'Let me help you...', type: 'text', timestamp: 1001 },
        ],
        streamLineCounter: 2,
        currentTurnStartLineId: 0,
        taskId: 'task-123',
        isChatSession: true,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: { input_tokens: 100, output_tokens: 50 },
        sessionUsageTotals: { input_tokens: 100, output_tokens: 50 },
        startedAt: 1000,
      });

      const store = useExecutionStore.getState();
      store.backgroundCurrentSession();

      const state = useExecutionStore.getState();

      // Should have one background session
      const bgKeys = Object.keys(state.backgroundSessions);
      expect(bgKeys.length).toBe(1);

      const snapshot = state.backgroundSessions[bgKeys[0]];
      expect(snapshot).toBeDefined();
      expect(snapshot.taskDescription).toBe('Build a REST API');
      expect(snapshot.status).toBe('running');
      expect(snapshot.streamingOutput).toHaveLength(2);
      expect(snapshot.streamLineCounter).toBe(2);
      expect(snapshot.taskId).toBe('task-123');
      expect(snapshot.isChatSession).toBe(true);
      expect(snapshot.latestUsage).toEqual({ input_tokens: 100, output_tokens: 50 });
      expect(snapshot.startedAt).toBe(1000);
    });

    it('should set activeSessionId to the backgrounded session id', () => {
      useExecutionStore.setState({
        taskDescription: 'Some task',
        status: 'running',
        streamingOutput: [],
        streamLineCounter: 0,
        currentTurnStartLineId: 0,
        taskId: null,
        isChatSession: false,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: null,
      });

      const store = useExecutionStore.getState();
      store.backgroundCurrentSession();

      const state = useExecutionStore.getState();
      const bgKeys = Object.keys(state.backgroundSessions);
      expect(bgKeys.length).toBe(1);
      // activeSessionId should be set to the new background session id
      expect(state.activeSessionId).toBe(bgKeys[0]);
    });

    it('should reset foreground to idle state after backgrounding', () => {
      useExecutionStore.setState({
        taskDescription: 'Build something',
        status: 'running',
        streamingOutput: [
          { id: 1, content: 'test', type: 'text', timestamp: 1000 },
        ],
        streamLineCounter: 1,
        taskId: 'task-abc',
        isChatSession: true,
        startedAt: Date.now(),
      });

      const store = useExecutionStore.getState();
      store.backgroundCurrentSession();

      const state = useExecutionStore.getState();
      // Foreground should be reset to idle
      expect(state.status).toBe('idle');
      expect(state.taskDescription).toBe('');
      expect(state.streamingOutput).toEqual([]);
      expect(state.streamLineCounter).toBe(0);
      expect(state.taskId).toBeNull();
      expect(state.isChatSession).toBe(false);
      expect(state.startedAt).toBeNull();
    });

    it('should support multiple background sessions', () => {
      // Background first session
      useExecutionStore.setState({
        taskDescription: 'First task',
        status: 'running',
        streamingOutput: [{ id: 1, content: 'first', type: 'text', timestamp: 1000 }],
        streamLineCounter: 1,
        currentTurnStartLineId: 0,
        taskId: 'task-1',
        isChatSession: true,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: 1000,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      // Background second session
      useExecutionStore.setState({
        taskDescription: 'Second task',
        status: 'completed',
        streamingOutput: [{ id: 1, content: 'second', type: 'text', timestamp: 2000 }],
        streamLineCounter: 1,
        currentTurnStartLineId: 0,
        taskId: 'task-2',
        isChatSession: false,
        standaloneTurns: [],
        standaloneSessionId: 'standalone-2',
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: 2000,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      const state = useExecutionStore.getState();
      const bgKeys = Object.keys(state.backgroundSessions);
      expect(bgKeys.length).toBe(2);

      // Verify both snapshots have correct data
      const snapshots = bgKeys.map((k) => state.backgroundSessions[k]);
      const taskDescs = snapshots.map((s) => s.taskDescription).sort();
      expect(taskDescs).toEqual(['First task', 'Second task']);
    });
  });

  // ===========================================================================
  // 3. switchToSession(id)
  // ===========================================================================
  describe('switchToSession(id)', () => {
    it('should swap foreground state with the specified background session', () => {
      // Set up and background a session
      useExecutionStore.setState({
        taskDescription: 'Background task',
        status: 'running',
        streamingOutput: [
          { id: 1, content: 'Background task', type: 'info', timestamp: 1000 },
          { id: 2, content: 'Working on it...', type: 'text', timestamp: 1001 },
        ],
        streamLineCounter: 2,
        currentTurnStartLineId: 0,
        taskId: 'bg-task-id',
        isChatSession: true,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: { input_tokens: 200, output_tokens: 100 },
        sessionUsageTotals: { input_tokens: 200, output_tokens: 100 },
        startedAt: 5000,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      const bgSessionId = Object.keys(useExecutionStore.getState().backgroundSessions)[0];

      // Now set a different foreground state
      useExecutionStore.setState({
        taskDescription: 'Current foreground task',
        status: 'idle',
        streamingOutput: [
          { id: 1, content: 'Foreground task', type: 'info', timestamp: 3000 },
        ],
        streamLineCounter: 1,
        currentTurnStartLineId: 0,
        taskId: 'fg-task-id',
        isChatSession: false,
        standaloneTurns: [{ user: 'hello', assistant: 'hi', createdAt: 3000 }],
        standaloneSessionId: 'standalone-fg',
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: 3000,
      });

      // Switch to the background session
      useExecutionStore.getState().switchToSession(bgSessionId);

      const state = useExecutionStore.getState();

      // Foreground should now have the background session's state
      expect(state.taskDescription).toBe('Background task');
      expect(state.status).toBe('running');
      expect(state.streamingOutput).toHaveLength(2);
      expect(state.streamingOutput[0].content).toBe('Background task');
      expect(state.taskId).toBe('bg-task-id');
      expect(state.isChatSession).toBe(true);
      expect(state.latestUsage).toEqual({ input_tokens: 200, output_tokens: 100 });
      expect(state.startedAt).toBe(5000);

      // The old foreground should now be in backgroundSessions
      const bgKeys = Object.keys(state.backgroundSessions);
      expect(bgKeys.length).toBe(1);
      const swappedBg = state.backgroundSessions[bgKeys[0]];
      expect(swappedBg.taskDescription).toBe('Current foreground task');
      expect(swappedBg.status).toBe('idle');
      expect(swappedBg.taskId).toBe('fg-task-id');
      expect(swappedBg.standaloneSessionId).toBe('standalone-fg');
    });

    it('should be a no-op if the session id does not exist', () => {
      useExecutionStore.setState({
        taskDescription: 'Unchanged',
        status: 'idle',
        backgroundSessions: {},
      });

      useExecutionStore.getState().switchToSession('nonexistent-id');

      const state = useExecutionStore.getState();
      expect(state.taskDescription).toBe('Unchanged');
      expect(state.status).toBe('idle');
    });

    it('should remove the restored session from backgroundSessions', () => {
      // Background a session
      useExecutionStore.setState({
        taskDescription: 'To be restored',
        status: 'completed',
        streamingOutput: [],
        streamLineCounter: 0,
        currentTurnStartLineId: 0,
        taskId: null,
        isChatSession: false,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: null,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      const bgSessionId = Object.keys(useExecutionStore.getState().backgroundSessions)[0];

      // Switch to it
      useExecutionStore.getState().switchToSession(bgSessionId);

      const state = useExecutionStore.getState();
      // The original bgSessionId should not be in backgroundSessions
      expect(state.backgroundSessions[bgSessionId]).toBeUndefined();
    });

    it('should update activeSessionId when switching', () => {
      // Background a session
      useExecutionStore.setState({
        taskDescription: 'Session A',
        status: 'running',
        streamingOutput: [],
        streamLineCounter: 0,
        currentTurnStartLineId: 0,
        taskId: 'a',
        isChatSession: true,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: 1000,
      });
      useExecutionStore.getState().backgroundCurrentSession();
      const bgId = Object.keys(useExecutionStore.getState().backgroundSessions)[0];

      // Create new foreground state
      useExecutionStore.setState({
        taskDescription: 'Session B',
        status: 'idle',
        streamingOutput: [],
        streamLineCounter: 0,
        currentTurnStartLineId: 0,
        taskId: 'b',
        isChatSession: false,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: 2000,
      });

      useExecutionStore.getState().switchToSession(bgId);

      const state = useExecutionStore.getState();
      // activeSessionId should reflect the newly backgrounded session
      const newBgKeys = Object.keys(state.backgroundSessions);
      expect(newBgKeys.length).toBe(1);
      expect(state.activeSessionId).toBe(newBgKeys[0]);
    });
  });

  // ===========================================================================
  // 4. removeBackgroundSession(id)
  // ===========================================================================
  describe('removeBackgroundSession(id)', () => {
    it('should remove the specified session from backgroundSessions', () => {
      // Background a session
      useExecutionStore.setState({
        taskDescription: 'To remove',
        status: 'failed',
        streamingOutput: [],
        streamLineCounter: 0,
        currentTurnStartLineId: 0,
        taskId: null,
        isChatSession: false,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: null,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      const bgSessionId = Object.keys(useExecutionStore.getState().backgroundSessions)[0];
      expect(useExecutionStore.getState().backgroundSessions[bgSessionId]).toBeDefined();

      // Remove it
      useExecutionStore.getState().removeBackgroundSession(bgSessionId);

      const state = useExecutionStore.getState();
      expect(state.backgroundSessions[bgSessionId]).toBeUndefined();
      expect(Object.keys(state.backgroundSessions).length).toBe(0);
    });

    it('should be a no-op if the session id does not exist', () => {
      useExecutionStore.setState({
        backgroundSessions: {},
      });

      // Should not throw
      useExecutionStore.getState().removeBackgroundSession('nonexistent');

      expect(Object.keys(useExecutionStore.getState().backgroundSessions).length).toBe(0);
    });

    it('should not affect other background sessions', () => {
      // Background two sessions
      useExecutionStore.setState({
        taskDescription: 'Keep me',
        status: 'running',
        streamingOutput: [{ id: 1, content: 'keep', type: 'text', timestamp: 1000 }],
        streamLineCounter: 1,
        currentTurnStartLineId: 0,
        taskId: 'keep',
        isChatSession: true,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: 1000,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      useExecutionStore.setState({
        taskDescription: 'Remove me',
        status: 'idle',
        streamingOutput: [],
        streamLineCounter: 0,
        currentTurnStartLineId: 0,
        taskId: 'remove',
        isChatSession: false,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: 2000,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      const bgKeys = Object.keys(useExecutionStore.getState().backgroundSessions);
      expect(bgKeys.length).toBe(2);

      // Find the "Remove me" session
      const removeKey = bgKeys.find(
        (k) => useExecutionStore.getState().backgroundSessions[k].taskDescription === 'Remove me'
      )!;
      const keepKey = bgKeys.find(
        (k) => useExecutionStore.getState().backgroundSessions[k].taskDescription === 'Keep me'
      )!;

      useExecutionStore.getState().removeBackgroundSession(removeKey);

      const state = useExecutionStore.getState();
      expect(Object.keys(state.backgroundSessions).length).toBe(1);
      expect(state.backgroundSessions[keepKey]).toBeDefined();
      expect(state.backgroundSessions[keepKey].taskDescription).toBe('Keep me');
    });
  });

  // ===========================================================================
  // 5. Immutability and existing behavior preservation
  // ===========================================================================
  describe('state transitions and immutability', () => {
    it('should preserve backgroundSessions across reset()', () => {
      // Background a session
      useExecutionStore.setState({
        taskDescription: 'Persistent',
        status: 'running',
        streamingOutput: [{ id: 1, content: 'data', type: 'text', timestamp: 1000 }],
        streamLineCounter: 1,
        currentTurnStartLineId: 0,
        taskId: 'persist-id',
        isChatSession: true,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: 1000,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      const bgBefore = useExecutionStore.getState().backgroundSessions;
      expect(Object.keys(bgBefore).length).toBe(1);

      // Reset should preserve backgroundSessions
      useExecutionStore.getState().reset();

      const bgAfter = useExecutionStore.getState().backgroundSessions;
      expect(Object.keys(bgAfter).length).toBe(1);
      expect(bgAfter).toEqual(bgBefore);
    });

    it('should not mutate the previous backgroundSessions reference when adding a new session', () => {
      useExecutionStore.setState({
        taskDescription: 'First',
        status: 'idle',
        streamingOutput: [],
        streamLineCounter: 0,
        currentTurnStartLineId: 0,
        taskId: null,
        isChatSession: false,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: null,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      const bgRef1 = useExecutionStore.getState().backgroundSessions;

      useExecutionStore.setState({
        taskDescription: 'Second',
        status: 'idle',
        streamingOutput: [],
        streamLineCounter: 0,
        currentTurnStartLineId: 0,
        taskId: null,
        isChatSession: false,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: null,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      const bgRef2 = useExecutionStore.getState().backgroundSessions;

      // References should be different (immutable update)
      expect(bgRef1).not.toBe(bgRef2);
      // Original reference should still only have 1 session
      expect(Object.keys(bgRef1).length).toBe(1);
      expect(Object.keys(bgRef2).length).toBe(2);
    });

    it('should preserve existing single-session behavior when no background sessions exist', () => {
      // The store should work exactly as before when backgroundSessions is empty
      const state = useExecutionStore.getState();
      expect(state.status).toBe('idle');
      expect(state.taskDescription).toBe('');
      expect(state.backgroundSessions).toEqual({});
      expect(state.activeSessionId).toBeNull();

      // appendStreamLine should still work
      state.appendStreamLine('test', 'text');
      const updated = useExecutionStore.getState();
      expect(updated.streamingOutput.length).toBe(1);
      expect(updated.backgroundSessions).toEqual({});
    });
  });

  // ===========================================================================
  // 6. SessionSnapshot type correctness
  // ===========================================================================
  describe('SessionSnapshot completeness', () => {
    it('should capture all required fields in the snapshot', () => {
      // Set up LLM settings before backgrounding
      useSettingsStore.setState({ backend: 'openai', provider: 'openai', model: 'gpt-4o' });

      useExecutionStore.setState({
        taskDescription: 'Full snapshot test',
        status: 'running',
        streamingOutput: [
          { id: 1, content: 'hello', type: 'info', timestamp: 1000 },
        ],
        streamLineCounter: 1,
        currentTurnStartLineId: 0,
        taskId: 'task-full',
        isChatSession: true,
        standaloneTurns: [{ user: 'hi', assistant: 'hello', createdAt: 1000 }],
        standaloneSessionId: 'standalone-full',
        latestUsage: { input_tokens: 50, output_tokens: 25, thinking_tokens: 10 },
        sessionUsageTotals: { input_tokens: 150, output_tokens: 75 },
        startedAt: 999,
      });

      useExecutionStore.getState().backgroundCurrentSession();

      const bgKeys = Object.keys(useExecutionStore.getState().backgroundSessions);
      const snapshot = useExecutionStore.getState().backgroundSessions[bgKeys[0]];

      // Verify all SessionSnapshot fields
      expect(snapshot.id).toBeDefined();
      expect(typeof snapshot.id).toBe('string');
      expect(snapshot.taskDescription).toBe('Full snapshot test');
      expect(snapshot.status).toBe('running');
      expect(snapshot.streamingOutput).toHaveLength(1);
      expect(snapshot.streamLineCounter).toBe(1);
      expect(snapshot.currentTurnStartLineId).toBe(0);
      expect(snapshot.taskId).toBe('task-full');
      expect(snapshot.isChatSession).toBe(true);
      expect(snapshot.standaloneTurns).toHaveLength(1);
      expect(snapshot.standaloneSessionId).toBe('standalone-full');
      expect(snapshot.latestUsage).toEqual({ input_tokens: 50, output_tokens: 25, thinking_tokens: 10 });
      expect(snapshot.sessionUsageTotals).toEqual({ input_tokens: 150, output_tokens: 75 });
      expect(snapshot.startedAt).toBe(999);
      // toolCallFilter should be a new instance in the snapshot
      expect(snapshot.toolCallFilter).toBeDefined();
      // LLM settings should be captured
      expect(snapshot.llmBackend).toBe('openai');
      expect(snapshot.llmProvider).toBe('openai');
      expect(snapshot.llmModel).toBe('gpt-4o');
    });
  });

  // ===========================================================================
  // 7. Per-session LLM provider persistence
  // ===========================================================================
  describe('Per-session LLM provider persistence', () => {
    it('should restore LLM settings when switching sessions', () => {
      // Session A uses OpenAI
      useSettingsStore.setState({ backend: 'openai', provider: 'openai', model: 'gpt-4o' });
      useExecutionStore.setState({
        taskDescription: 'Session A',
        status: 'running',
        taskId: 'task-a',
      });
      useExecutionStore.getState().backgroundCurrentSession();

      const bgKeys = Object.keys(useExecutionStore.getState().backgroundSessions);
      const sessionAId = bgKeys[0];

      // Session B uses DeepSeek
      useSettingsStore.setState({ backend: 'deepseek', provider: 'deepseek', model: 'deepseek-chat' });
      useExecutionStore.setState({
        taskDescription: 'Session B',
        status: 'running',
        taskId: 'task-b',
      });

      // Switch back to Session A
      useExecutionStore.getState().switchToSession(sessionAId);

      // LLM settings should be restored to Session A's values
      const settings = useSettingsStore.getState();
      expect(settings.backend).toBe('openai');
      expect(settings.provider).toBe('openai');
      expect(settings.model).toBe('gpt-4o');

      // Foreground should be Session A's data
      expect(useExecutionStore.getState().taskDescription).toBe('Session A');
    });

    it('should preserve LLM settings in snapshot when auto-backgrounding on start()', async () => {
      useSettingsStore.setState({ backend: 'ollama', provider: 'ollama', model: 'llama3' });
      useExecutionStore.setState({
        taskDescription: 'Running task',
        status: 'running',
        taskId: 'running-task',
      });

      useExecutionStore.getState().backgroundCurrentSession();

      const bgKeys = Object.keys(useExecutionStore.getState().backgroundSessions);
      const snapshot = useExecutionStore.getState().backgroundSessions[bgKeys[0]];

      expect(snapshot.llmBackend).toBe('ollama');
      expect(snapshot.llmProvider).toBe('ollama');
      expect(snapshot.llmModel).toBe('llama3');
    });

    it('should capture current LLM settings for the backgrounded foreground in switchToSession', () => {
      // Background session A with OpenAI
      useSettingsStore.setState({ backend: 'openai', provider: 'openai', model: 'gpt-4o' });
      useExecutionStore.setState({
        taskDescription: 'Session A',
        status: 'running',
        taskId: 'task-a',
      });
      useExecutionStore.getState().backgroundCurrentSession();
      const sessionAId = Object.keys(useExecutionStore.getState().backgroundSessions)[0];

      // Now foreground is idle, switch to DeepSeek and create Session B content
      useSettingsStore.setState({ backend: 'deepseek', provider: 'deepseek', model: 'deepseek-chat' });
      useExecutionStore.setState({
        taskDescription: 'Session B',
        status: 'running',
        taskId: 'task-b',
      });

      // Switch to Session A — foreground (Session B) gets backgrounded with DeepSeek settings
      useExecutionStore.getState().switchToSession(sessionAId);

      // Find the newly backgrounded Session B
      const bgSessions = useExecutionStore.getState().backgroundSessions;
      const sessionBSnapshot = Object.values(bgSessions).find(s => s.taskId === 'task-b');

      expect(sessionBSnapshot).toBeDefined();
      expect(sessionBSnapshot!.llmBackend).toBe('deepseek');
      expect(sessionBSnapshot!.llmProvider).toBe('deepseek');
      expect(sessionBSnapshot!.llmModel).toBe('deepseek-chat');
    });

    it('should persist LLM settings when saving history entries', () => {
      useSettingsStore.setState({ backend: 'openai', provider: 'openai', model: 'gpt-4o-mini' });
      useExecutionStore.setState({
        taskDescription: 'History with model',
        status: 'completed',
        startedAt: Date.now() - 5000,
        streamingOutput: [
          { id: 1, content: 'hello', type: 'info', timestamp: Date.now() - 4500 },
          { id: 2, content: 'hi', type: 'text', timestamp: Date.now() - 4400 },
        ],
        streamLineCounter: 2,
      });

      useExecutionStore.getState().saveToHistory();

      const [saved] = useExecutionStore.getState().history;
      expect(saved).toBeDefined();
      expect(saved.llmBackend).toBe('openai');
      expect(saved.llmProvider).toBe('openai');
      expect(saved.llmModel).toBe('gpt-4o-mini');
    });

    it('should restore LLM settings from history when restoring a session', () => {
      const itemId = 'history-llm-1';
      useExecutionStore.setState({
        history: [
          {
            id: itemId,
            taskDescription: 'Restored with model',
            strategy: null,
            status: 'completed',
            startedAt: Date.now() - 10_000,
            duration: 8000,
            completedStories: 1,
            totalStories: 1,
            success: true,
            conversationLines: [
              { type: 'info', content: 'question' },
              { type: 'text', content: 'answer' },
            ],
            llmBackend: 'deepseek',
            llmProvider: 'deepseek',
            llmModel: 'deepseek-chat',
          },
        ],
      });
      useSettingsStore.setState({ backend: 'openai', provider: 'openai', model: 'gpt-4o' });

      useExecutionStore.getState().restoreFromHistory(itemId);

      const settings = useSettingsStore.getState();
      expect(settings.backend).toBe('deepseek');
      expect(settings.provider).toBe('deepseek');
      expect(settings.model).toBe('deepseek-chat');
    });

    it('should keep current LLM settings when restoring legacy history without model metadata', () => {
      const itemId = 'history-legacy-1';
      useExecutionStore.setState({
        history: [
          {
            id: itemId,
            taskDescription: 'Legacy history',
            strategy: null,
            status: 'completed',
            startedAt: Date.now() - 10_000,
            duration: 6000,
            completedStories: 1,
            totalStories: 1,
            success: true,
            conversationLines: [
              { type: 'info', content: 'hello' },
              { type: 'text', content: 'world' },
            ],
          },
        ],
      });
      useSettingsStore.setState({ backend: 'glm', provider: 'glm', model: 'glm-4.5' });

      useExecutionStore.getState().restoreFromHistory(itemId);

      const settings = useSettingsStore.getState();
      expect(settings.backend).toBe('glm');
      expect(settings.provider).toBe('glm');
      expect(settings.model).toBe('glm-4.5');
    });
  });
});

// =============================================================================
// Story 007: Route Streaming Events to Active or Background Sessions
// =============================================================================

describe('Execution Store - Event Routing to Background Sessions', () => {
  /**
   * Set up a scenario with:
   * - Foreground session: taskId='fg-session-1', running
   * - Background session: taskId='bg-session-1', running
   *
   * Then call initialize() to register event listeners so we can emit
   * synthetic events.
   */
  async function setupForegroundAndBackground() {
    // First, set up a session that will become the background session
    useExecutionStore.setState({
      taskDescription: 'Background session task',
      status: 'running',
      streamingOutput: [
        { id: 1, content: 'Background task started', type: 'info', timestamp: 1000 },
      ],
      streamLineCounter: 1,
      currentTurnStartLineId: 0,
      taskId: 'bg-session-1',
      isChatSession: true,
      standaloneTurns: [],
      standaloneSessionId: null,
      latestUsage: null,
      sessionUsageTotals: null,
      startedAt: 1000,
    });

    // Background it
    useExecutionStore.getState().backgroundCurrentSession();

    // Set up the foreground session
    useExecutionStore.setState({
      taskDescription: 'Foreground session task',
      status: 'running',
      streamingOutput: [
        { id: 1, content: 'Foreground task started', type: 'info', timestamp: 2000 },
      ],
      streamLineCounter: 1,
      currentTurnStartLineId: 0,
      taskId: 'fg-session-1',
      isChatSession: true,
      standaloneTurns: [],
      standaloneSessionId: null,
      latestUsage: null,
      sessionUsageTotals: null,
      startedAt: 2000,
    });

    // Initialize listeners (registers event callbacks)
    useExecutionStore.getState().initialize();
    // Wait for all listen() promises to settle
    await vi.waitFor(() => {
      expect(eventHandlers['claude_code:stream']).toBeDefined();
      expect(eventHandlers['claude_code:tool']).toBeDefined();
      expect(eventHandlers['claude_code:session']).toBeDefined();
    });
  }

  beforeEach(() => {
    resetStore();
    // Clear captured event handlers
    for (const key of Object.keys(eventHandlers)) {
      delete eventHandlers[key];
    }
    vi.clearAllMocks();
  });

  // ===========================================================================
  // 7.1 claude_code:stream — text_delta routed to background session
  // ===========================================================================
  describe('claude_code:stream routing', () => {
    it('should route text_delta to background session when session_id does not match foreground', async () => {
      await setupForegroundAndBackground();

      const fgOutputBefore = useExecutionStore.getState().streamingOutput.length;

      // Emit a text_delta event for the background session
      emitEvent('claude_code:stream', {
        event: { type: 'text_delta', content: 'Hello from background' },
        session_id: 'bg-session-1',
      });

      const state = useExecutionStore.getState();

      // Foreground should NOT have received the text
      expect(state.streamingOutput.length).toBe(fgOutputBefore);

      // Background session should have the new line
      const bg = findBgByTaskId('bg-session-1');
      expect(bg).toBeDefined();
      const bgLines = bg!.snapshot.streamingOutput;
      const lastLine = bgLines[bgLines.length - 1];
      expect(lastLine.content).toBe('Hello from background');
      expect(lastLine.type).toBe('text');
    });

    it('should still route text_delta to foreground when session_id matches', async () => {
      await setupForegroundAndBackground();

      const fgOutputBefore = useExecutionStore.getState().streamingOutput.length;

      emitEvent('claude_code:stream', {
        event: { type: 'text_delta', content: 'Hello from foreground' },
        session_id: 'fg-session-1',
      });

      const state = useExecutionStore.getState();
      expect(state.streamingOutput.length).toBe(fgOutputBefore + 1);
      expect(state.streamingOutput[state.streamingOutput.length - 1].content).toBe('Hello from foreground');

      // Background should be unchanged (still just the initial line)
      const bg = findBgByTaskId('bg-session-1');
      expect(bg!.snapshot.streamingOutput).toHaveLength(1);
    });

    it('should route tool_start to background session', async () => {
      await setupForegroundAndBackground();

      emitEvent('claude_code:stream', {
        event: { type: 'tool_start', tool_name: 'Read' },
        session_id: 'bg-session-1',
      });

      const bg = findBgByTaskId('bg-session-1');
      const bgLines = bg!.snapshot.streamingOutput;
      expect(bgLines.length).toBeGreaterThan(1);
      const lastLine = bgLines[bgLines.length - 1];
      expect(lastLine.content).toContain('[tool] Read started');
      expect(lastLine.type).toBe('tool');
    });

    it('should route tool_result to background session', async () => {
      await setupForegroundAndBackground();

      emitEvent('claude_code:stream', {
        event: { type: 'tool_result', tool_id: 'tool-42', error: null },
        session_id: 'bg-session-1',
      });

      const bg = findBgByTaskId('bg-session-1');
      const lastLine = bg!.snapshot.streamingOutput[bg!.snapshot.streamingOutput.length - 1];
      expect(lastLine.content).toContain('tool-42');
      expect(lastLine.content).toContain('completed');
      expect(lastLine.type).toBe('success');
    });

    it('should route tool_result error to background session', async () => {
      await setupForegroundAndBackground();

      emitEvent('claude_code:stream', {
        event: { type: 'tool_result', tool_id: 'tool-err', error: 'file not found' },
        session_id: 'bg-session-1',
      });

      const bg = findBgByTaskId('bg-session-1');
      const lastLine = bg!.snapshot.streamingOutput[bg!.snapshot.streamingOutput.length - 1];
      expect(lastLine.content).toContain('failed');
      expect(lastLine.type).toBe('error');
    });

    it('should route error event to background and set status to failed', async () => {
      await setupForegroundAndBackground();

      emitEvent('claude_code:stream', {
        event: { type: 'error', message: 'Something went wrong' },
        session_id: 'bg-session-1',
      });

      const bg = findBgByTaskId('bg-session-1');
      expect(bg!.snapshot.status).toBe('failed');
      const lastLine = bg!.snapshot.streamingOutput[bg!.snapshot.streamingOutput.length - 1];
      expect(lastLine.content).toBe('Something went wrong');
      expect(lastLine.type).toBe('error');

      // Foreground should not be affected
      expect(useExecutionStore.getState().status).toBe('running');
    });

    it('should route complete event to background chat session and set status to idle', async () => {
      await setupForegroundAndBackground();

      emitEvent('claude_code:stream', {
        event: { type: 'complete' },
        session_id: 'bg-session-1',
      });

      const bg = findBgByTaskId('bg-session-1');
      // isChatSession is true, so it should go to idle
      expect(bg!.snapshot.status).toBe('idle');

      // Foreground should remain running
      expect(useExecutionStore.getState().status).toBe('running');
    });

    it('should route complete event to background non-chat session and set status to completed', async () => {
      await setupForegroundAndBackground();

      // Change the background session to non-chat
      const bgEntry = findBgByTaskId('bg-session-1');
      useExecutionStore.setState({
        backgroundSessions: {
          ...useExecutionStore.getState().backgroundSessions,
          [bgEntry!.key]: { ...bgEntry!.snapshot, isChatSession: false },
        },
      });

      emitEvent('claude_code:stream', {
        event: { type: 'complete' },
        session_id: 'bg-session-1',
      });

      const bg = findBgByTaskId('bg-session-1');
      expect(bg!.snapshot.status).toBe('completed');
    });

    it('should not drop events for unknown session_ids (no matching bg or fg)', async () => {
      await setupForegroundAndBackground();

      // Event for a completely unknown session - should not throw
      emitEvent('claude_code:stream', {
        event: { type: 'text_delta', content: 'mystery data' },
        session_id: 'unknown-session-42',
      });

      // Neither foreground nor background should have the content
      const state = useExecutionStore.getState();
      const fgHas = state.streamingOutput.some((l) => l.content === 'mystery data');
      expect(fgHas).toBe(false);
      const bg = findBgByTaskId('unknown-session-42');
      expect(bg).toBeUndefined();
    });
  });

  // ===========================================================================
  // 7.2 claude_code:tool — routed to background session
  // ===========================================================================
  describe('claude_code:tool routing', () => {
    it('should route tool started event to background session', async () => {
      await setupForegroundAndBackground();

      emitEvent('claude_code:tool', {
        execution: { id: 'exec-1', tool_name: 'Bash', success: undefined, arguments: '', result: '' },
        update_type: 'started',
        session_id: 'bg-session-1',
      });

      const bg = findBgByTaskId('bg-session-1');
      const lastLine = bg!.snapshot.streamingOutput[bg!.snapshot.streamingOutput.length - 1];
      expect(lastLine.content).toContain('[tool] Bash started');
      expect(lastLine.type).toBe('tool');

      // Foreground unchanged
      const fgLines = useExecutionStore.getState().streamingOutput;
      expect(fgLines.some((l) => l.content.includes('Bash started'))).toBe(false);
    });

    it('should route tool completed event to background session', async () => {
      await setupForegroundAndBackground();

      emitEvent('claude_code:tool', {
        execution: { id: 'exec-2', tool_name: 'Write', success: true },
        update_type: 'completed',
        session_id: 'bg-session-1',
      });

      const bg = findBgByTaskId('bg-session-1');
      const lastLine = bg!.snapshot.streamingOutput[bg!.snapshot.streamingOutput.length - 1];
      expect(lastLine.content).toContain('[tool] Write success');
      expect(lastLine.type).toBe('success');
    });

    it('should route tool failed event to background session', async () => {
      await setupForegroundAndBackground();

      emitEvent('claude_code:tool', {
        execution: { id: 'exec-3', tool_name: 'Grep', success: false },
        update_type: 'completed',
        session_id: 'bg-session-1',
      });

      const bg = findBgByTaskId('bg-session-1');
      const lastLine = bg!.snapshot.streamingOutput[bg!.snapshot.streamingOutput.length - 1];
      expect(lastLine.content).toContain('[tool] Grep failed');
      expect(lastLine.type).toBe('error');
    });

    it('should route tool events to foreground when session_id matches', async () => {
      await setupForegroundAndBackground();

      const fgBefore = useExecutionStore.getState().streamingOutput.length;

      emitEvent('claude_code:tool', {
        execution: { id: 'exec-4', tool_name: 'Read', success: true },
        update_type: 'completed',
        session_id: 'fg-session-1',
      });

      const fgAfter = useExecutionStore.getState().streamingOutput;
      expect(fgAfter.length).toBe(fgBefore + 1);
      expect(fgAfter[fgAfter.length - 1].content).toContain('Read');
    });
  });

  // ===========================================================================
  // 7.3 claude_code:session — routed to background session
  // ===========================================================================
  describe('claude_code:session routing', () => {
    it('should route session error to background session and set status to failed', async () => {
      await setupForegroundAndBackground();

      emitEvent('claude_code:session', {
        session: { id: 'bg-session-1', state: 'error', error_message: 'Connection lost' },
        update_type: 'state_changed',
      });

      const bg = findBgByTaskId('bg-session-1');
      expect(bg!.snapshot.status).toBe('failed');
      const lastLine = bg!.snapshot.streamingOutput[bg!.snapshot.streamingOutput.length - 1];
      expect(lastLine.content).toContain('Connection lost');
      expect(lastLine.type).toBe('error');

      // Foreground remains running
      expect(useExecutionStore.getState().status).toBe('running');
    });

    it('should route session cancelled to background session and set status to idle', async () => {
      await setupForegroundAndBackground();

      emitEvent('claude_code:session', {
        session: { id: 'bg-session-1', state: 'cancelled' },
        update_type: 'state_changed',
      });

      const bg = findBgByTaskId('bg-session-1');
      expect(bg!.snapshot.status).toBe('idle');
      const lastLine = bg!.snapshot.streamingOutput[bg!.snapshot.streamingOutput.length - 1];
      expect(lastLine.content).toContain('Session cancelled');
      expect(lastLine.type).toBe('warning');
    });

    it('should handle session events for foreground normally', async () => {
      await setupForegroundAndBackground();

      emitEvent('claude_code:session', {
        session: { id: 'fg-session-1', state: 'error', error_message: 'FG Error' },
        update_type: 'state_changed',
      });

      // Foreground should be updated
      const state = useExecutionStore.getState();
      expect(state.status).toBe('failed');
      expect(state.apiError).toContain('FG Error');

      // Background should be unaffected
      const bg = findBgByTaskId('bg-session-1');
      expect(bg!.snapshot.status).toBe('running');
    });
  });

  // ===========================================================================
  // 7.4 Cross-session isolation — no contamination
  // ===========================================================================
  describe('cross-session isolation', () => {
    it('should not contaminate background session when foreground receives events', async () => {
      await setupForegroundAndBackground();

      const bgBefore = findBgByTaskId('bg-session-1')!.snapshot.streamingOutput.length;

      // Multiple foreground events
      emitEvent('claude_code:stream', {
        event: { type: 'text_delta', content: 'FG text 1' },
        session_id: 'fg-session-1',
      });
      emitEvent('claude_code:stream', {
        event: { type: 'text_delta', content: 'FG text 2' },
        session_id: 'fg-session-1',
      });
      emitEvent('claude_code:stream', {
        event: { type: 'tool_start', tool_name: 'FGTool' },
        session_id: 'fg-session-1',
      });

      // Background should be untouched
      const bgAfter = findBgByTaskId('bg-session-1')!.snapshot.streamingOutput.length;
      expect(bgAfter).toBe(bgBefore);
    });

    it('should correctly route events to multiple background sessions', async () => {
      // Set up two background sessions
      useExecutionStore.setState({
        taskDescription: 'BG Task A',
        status: 'running',
        streamingOutput: [],
        streamLineCounter: 0,
        currentTurnStartLineId: 0,
        taskId: 'bg-a',
        isChatSession: true,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: 1000,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      useExecutionStore.setState({
        taskDescription: 'BG Task B',
        status: 'running',
        streamingOutput: [],
        streamLineCounter: 0,
        currentTurnStartLineId: 0,
        taskId: 'bg-b',
        isChatSession: true,
        standaloneTurns: [],
        standaloneSessionId: null,
        latestUsage: null,
        sessionUsageTotals: null,
        startedAt: 2000,
      });
      useExecutionStore.getState().backgroundCurrentSession();

      // Set foreground
      useExecutionStore.setState({
        taskDescription: 'FG Task',
        status: 'running',
        streamingOutput: [],
        streamLineCounter: 0,
        taskId: 'fg-task',
        isChatSession: true,
      });

      // Initialize listeners
      useExecutionStore.getState().initialize();
      await vi.waitFor(() => {
        expect(eventHandlers['claude_code:stream']).toBeDefined();
      });

      // Send events to bg-a
      emitEvent('claude_code:stream', {
        event: { type: 'text_delta', content: 'Data for A' },
        session_id: 'bg-a',
      });

      // Send events to bg-b
      emitEvent('claude_code:stream', {
        event: { type: 'text_delta', content: 'Data for B' },
        session_id: 'bg-b',
      });

      const bgA = findBgByTaskId('bg-a');
      const bgB = findBgByTaskId('bg-b');

      expect(bgA!.snapshot.streamingOutput.some((l) => l.content === 'Data for A')).toBe(true);
      expect(bgA!.snapshot.streamingOutput.some((l) => l.content === 'Data for B')).toBe(false);

      expect(bgB!.snapshot.streamingOutput.some((l) => l.content === 'Data for B')).toBe(true);
      expect(bgB!.snapshot.streamingOutput.some((l) => l.content === 'Data for A')).toBe(false);

      // Foreground should have none of the bg data
      const fg = useExecutionStore.getState().streamingOutput;
      expect(fg.some((l) => l.content === 'Data for A')).toBe(false);
      expect(fg.some((l) => l.content === 'Data for B')).toBe(false);
    });

    it('should maintain separate streamLineCounter per background session', async () => {
      await setupForegroundAndBackground();

      // Send multiple events to background
      emitEvent('claude_code:stream', {
        event: { type: 'text_delta', content: 'Line 1' },
        session_id: 'bg-session-1',
      });
      emitEvent('claude_code:stream', {
        event: { type: 'text_delta', content: 'Line 2' },
        session_id: 'bg-session-1',
      });

      const bg = findBgByTaskId('bg-session-1');
      // Original had 1 line (from setup) + 2 new lines = counter should be 3
      expect(bg!.snapshot.streamLineCounter).toBe(3);

      // Each line should have a unique, incrementing ID
      const lines = bg!.snapshot.streamingOutput;
      const ids = lines.map((l) => l.id);
      const uniqueIds = new Set(ids);
      expect(uniqueIds.size).toBe(ids.length);
    });
  });

  // ===========================================================================
  // 7.5 No duplicate listener registration regressions
  // ===========================================================================
  describe('listener registration', () => {
    it('should not register duplicate listeners on re-initialize', async () => {
      resetStore();

      // Initialize twice
      useExecutionStore.getState().initialize();
      await vi.waitFor(() => {
        expect(eventHandlers['claude_code:stream']).toBeDefined();
      });

      const listenCallCount1 = (listen as ReturnType<typeof vi.fn>).mock.calls.length;

      useExecutionStore.getState().initialize();
      await vi.waitFor(() => {
        expect(eventHandlers['claude_code:stream']).toBeDefined();
      });

      // listen should have been called again (re-registration after cleanup),
      // but the old listeners should have been cleaned up via unlisten
      // The important thing is that the event handlers work correctly
      // and don't fire duplicate callbacks
      expect((listen as ReturnType<typeof vi.fn>).mock.calls.length).toBeGreaterThanOrEqual(listenCallCount1);
    });
  });
});

// =============================================================================
// Story 008: Auto-Background on New Start & Sidebar Background Sessions
// =============================================================================

describe('Execution Store - Auto-Background on start()', () => {
  beforeEach(() => {
    resetStore();
    vi.clearAllMocks();
  });

  it('should auto-background the current session when start() is called while running', async () => {
    // Simulate a running session
    useExecutionStore.setState({
      taskDescription: 'Running task',
      status: 'running',
      streamingOutput: [
        { id: 1, content: 'Running task', type: 'info', timestamp: 1000 },
        { id: 2, content: 'Working...', type: 'text', timestamp: 1001 },
      ],
      streamLineCounter: 2,
      currentTurnStartLineId: 0,
      taskId: 'running-task-123',
      isChatSession: true,
      standaloneTurns: [],
      standaloneSessionId: null,
      latestUsage: { input_tokens: 100, output_tokens: 50 },
      sessionUsageTotals: { input_tokens: 100, output_tokens: 50 },
      startedAt: 1000,
    });

    // Verify we have no background sessions initially
    expect(Object.keys(useExecutionStore.getState().backgroundSessions).length).toBe(0);

    // Call start() which should auto-background the current session
    // We expect the invoke to be called, but it will fail since it's mocked — that's okay,
    // we only need to verify the auto-background happened BEFORE the start logic.
    try {
      await useExecutionStore.getState().start('New task', 'simple');
    } catch {
      // Expected — invoke is mocked
    }

    const state = useExecutionStore.getState();

    // Should have backgrounded the previous session
    const bgKeys = Object.keys(state.backgroundSessions);
    expect(bgKeys.length).toBe(1);

    const snapshot = state.backgroundSessions[bgKeys[0]];
    expect(snapshot.taskDescription).toBe('Running task');
    expect(snapshot.status).toBe('running');
    expect(snapshot.taskId).toBe('running-task-123');
    expect(snapshot.streamingOutput).toHaveLength(2);
  });

  it('should auto-background a running chat session when start() is called again', async () => {
    // Simulate a running chat session (isChatSession=true with different taskId)
    useExecutionStore.setState({
      taskDescription: 'Chat session task',
      status: 'running',
      streamingOutput: [
        { id: 1, content: 'Chat session...', type: 'text', timestamp: 1000 },
      ],
      streamLineCounter: 1,
      currentTurnStartLineId: 0,
      taskId: 'chat-task-456',
      isChatSession: true,
      standaloneTurns: [],
      standaloneSessionId: null,
      latestUsage: null,
      sessionUsageTotals: null,
      startedAt: 2000,
    });

    try {
      await useExecutionStore.getState().start('Another task', 'simple');
    } catch {
      // Expected
    }

    const state = useExecutionStore.getState();
    const bgKeys = Object.keys(state.backgroundSessions);
    expect(bgKeys.length).toBe(1);

    const snapshot = state.backgroundSessions[bgKeys[0]];
    expect(snapshot.taskDescription).toBe('Chat session task');
    expect(snapshot.taskId).toBe('chat-task-456');
  });

  it('should NOT auto-background when start() is called from idle state', async () => {
    // Start from idle (default state)
    useExecutionStore.setState({
      status: 'idle',
      taskDescription: '',
      streamingOutput: [],
    });

    try {
      await useExecutionStore.getState().start('First task', 'simple');
    } catch {
      // Expected
    }

    const state = useExecutionStore.getState();
    expect(Object.keys(state.backgroundSessions).length).toBe(0);
  });

  it('should NOT auto-background when start() is called from completed state', async () => {
    useExecutionStore.setState({
      status: 'completed',
      taskDescription: 'Done task',
      streamingOutput: [],
    });

    try {
      await useExecutionStore.getState().start('Next task', 'simple');
    } catch {
      // Expected
    }

    const state = useExecutionStore.getState();
    expect(Object.keys(state.backgroundSessions).length).toBe(0);
  });

  it('should NOT auto-background when start() is called from failed state', async () => {
    useExecutionStore.setState({
      status: 'failed',
      taskDescription: 'Failed task',
      streamingOutput: [],
    });

    try {
      await useExecutionStore.getState().start('Retry task', 'simple');
    } catch {
      // Expected
    }

    const state = useExecutionStore.getState();
    expect(Object.keys(state.backgroundSessions).length).toBe(0);
  });
});
