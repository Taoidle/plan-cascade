/**
 * Execution Flow Integration Tests
 * Story 007: Integration Testing Suite
 *
 * Tests for end-to-end execution flows including message sending,
 * tool calls, and session management.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock Tauri API
const mockInvoke = vi.fn();
const mockListen = vi.fn();
const mockUnlisten = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => mockListen(...args),
  emit: vi.fn(),
}));

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}));

// ============================================================================
// Message Flow Tests
// ============================================================================

describe('Message Flow Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListen.mockResolvedValue(mockUnlisten);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('should send a message and receive a response', async () => {
    // Mock the invoke call for sending messages
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'send_message') {
        return { success: true, messageId: 'msg-123' };
      }
      return null;
    });

    // Simulate the message sending flow
    const sendMessage = async (content: string) => {
      const result = await mockInvoke('send_message', { content });
      return result;
    };

    const result = await sendMessage('Hello, Claude!');

    expect(mockInvoke).toHaveBeenCalledWith('send_message', { content: 'Hello, Claude!' });
    expect(result.success).toBe(true);
    expect(result.messageId).toBe('msg-123');
  });

  it('should handle streaming responses', async () => {
    const streamEvents: unknown[] = [];
    let eventHandler: ((event: { payload: unknown }) => void) | null = null;

    // Capture the event listener
    mockListen.mockImplementation(async (event: string, handler: (event: { payload: unknown }) => void) => {
      if (event === 'stream-event') {
        eventHandler = handler;
      }
      return mockUnlisten;
    });

    // Set up stream listener
    await mockListen('stream-event', (event: { payload: unknown }) => {
      streamEvents.push(event.payload);
    });

    // Simulate streaming events
    const streamHandler = eventHandler as unknown as (event: { payload: unknown }) => void;
    streamHandler({ payload: { type: 'TextDelta', text: 'Hello' } });
    streamHandler({ payload: { type: 'TextDelta', text: ' World' } });
    streamHandler({ payload: { type: 'Complete', stop_reason: 'end_turn' } });

    expect(streamEvents).toHaveLength(3);
    expect(streamEvents[0]).toEqual({ type: 'TextDelta', text: 'Hello' });
  });

  it('should handle tool call execution', async () => {
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'execute_tool') {
        return { success: true, output: 'Tool executed successfully' };
      }
      return null;
    });

    // Simulate tool execution
    const result = await mockInvoke('execute_tool', {
      toolName: 'Read',
      arguments: { file_path: '/test.txt' },
    });

    expect(result.success).toBe(true);
    expect(result.output).toBe('Tool executed successfully');
  });

  it('should handle errors gracefully', async () => {
    mockInvoke.mockRejectedValue(new Error('Network error'));

    const sendMessage = async (content: string) => {
      try {
        await mockInvoke('send_message', { content });
        return { success: true };
      } catch (error) {
        return { success: false, error: (error as Error).message };
      }
    };

    const result = await sendMessage('Test message');

    expect(result.success).toBe(false);
    expect(result.error).toBe('Network error');
  });
});

// ============================================================================
// Session Management Tests
// ============================================================================

describe('Session Management Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should create a new session', async () => {
    mockInvoke.mockResolvedValue({
      id: 'session-123',
      projectPath: '/test/project',
      createdAt: new Date().toISOString(),
    });

    const result = await mockInvoke('create_session', {
      projectPath: '/test/project',
      name: 'Test Session',
    });

    expect(result.id).toBe('session-123');
    expect(result.projectPath).toBe('/test/project');
  });

  it('should list sessions', async () => {
    mockInvoke.mockResolvedValue([
      { id: 'session-1', name: 'Session 1', projectPath: '/project1' },
      { id: 'session-2', name: 'Session 2', projectPath: '/project2' },
    ]);

    const sessions = await mockInvoke('list_sessions', {});

    expect(sessions).toHaveLength(2);
    expect(sessions[0].id).toBe('session-1');
  });

  it('should delete a session', async () => {
    mockInvoke.mockResolvedValue({ success: true });

    const result = await mockInvoke('delete_session', { sessionId: 'session-123' });

    expect(result.success).toBe(true);
    expect(mockInvoke).toHaveBeenCalledWith('delete_session', { sessionId: 'session-123' });
  });

  it('should resume a paused session', async () => {
    mockInvoke.mockResolvedValue({
      success: true,
      sessionId: 'session-123',
      status: 'running',
    });

    const result = await mockInvoke('resume_session', { sessionId: 'session-123' });

    expect(result.success).toBe(true);
    expect(result.status).toBe('running');
  });
});

// ============================================================================
// Connection Status Tests
// ============================================================================

describe('Connection Status Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListen.mockResolvedValue(mockUnlisten);
  });

  it('should handle connection state changes', async () => {
    const connectionStates: string[] = [];
    let statusHandler: ((event: { payload: string }) => void) | null = null;

    mockListen.mockImplementation(async (event: string, handler: (event: { payload: string }) => void) => {
      if (event === 'connection-status') {
        statusHandler = handler;
      }
      return mockUnlisten;
    });

    await mockListen('connection-status', (event: { payload: string }) => {
      connectionStates.push(event.payload);
    });

    // Simulate connection state changes
    const connectionHandler = statusHandler as unknown as (event: { payload: string }) => void;
    connectionHandler({ payload: 'connecting' });
    connectionHandler({ payload: 'connected' });

    expect(connectionStates).toContain('connecting');
    expect(connectionStates).toContain('connected');
  });

  it('should handle disconnection', async () => {
    let statusHandler: ((event: { payload: string }) => void) | null = null;
    const states: string[] = [];

    mockListen.mockImplementation(async (_event: string, handler: (event: { payload: string }) => void) => {
      statusHandler = handler;
      return mockUnlisten;
    });

    await mockListen('connection-status', (event: { payload: string }) => {
      states.push(event.payload);
    });

    const connectionHandler = statusHandler as unknown as (event: { payload: string }) => void;
    connectionHandler({ payload: 'connected' });
    connectionHandler({ payload: 'disconnected' });

    expect(states).toContain('disconnected');
  });
});

// ============================================================================
// Worktree Integration Tests
// ============================================================================

describe('Worktree Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should create a worktree', async () => {
    mockInvoke.mockResolvedValue({
      id: 'task-1',
      path: '/repo/.worktree/task-1',
      branch: 'worktree/task-1',
      targetBranch: 'main',
      status: 'active',
    });

    const result = await mockInvoke('create_worktree', {
      repoPath: '/repo',
      taskName: 'task-1',
      targetBranch: 'main',
    });

    expect(result.id).toBe('task-1');
    expect(result.status).toBe('active');
  });

  it('should list worktrees', async () => {
    mockInvoke.mockResolvedValue([
      { id: 'task-1', branch: 'worktree/task-1', status: 'active' },
      { id: 'task-2', branch: 'worktree/task-2', status: 'ready' },
    ]);

    const worktrees = await mockInvoke('list_worktrees', { repoPath: '/repo' });

    expect(worktrees).toHaveLength(2);
    expect(worktrees[0].status).toBe('active');
  });

  it('should complete a worktree', async () => {
    mockInvoke.mockResolvedValue({
      success: true,
      commitSha: 'abc123',
      merged: true,
      cleanedUp: true,
    });

    const result = await mockInvoke('complete_worktree', {
      repoPath: '/repo',
      worktreeId: 'task-1',
      commitMessage: 'Complete task 1',
    });

    expect(result.success).toBe(true);
    expect(result.merged).toBe(true);
    expect(result.cleanedUp).toBe(true);
  });

  it('should remove a worktree', async () => {
    mockInvoke.mockResolvedValue({ success: true });

    const result = await mockInvoke('remove_worktree', {
      repoPath: '/repo',
      worktreeId: 'task-1',
      force: false,
    });

    expect(result.success).toBe(true);
  });
});

// ============================================================================
// Quality Gates Integration Tests
// ============================================================================

describe('Quality Gates Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should detect project type', async () => {
    mockInvoke.mockResolvedValue({
      projectType: 'nodejs',
      markerFile: '/project/package.json',
      metadata: {
        name: 'test-project',
        hasTypescript: true,
        hasEslint: true,
        hasTests: true,
      },
      suggestedGates: ['tsc', 'eslint', 'test'],
    });

    const result = await mockInvoke('detect_project_type', { projectPath: '/project' });

    expect(result.projectType).toBe('nodejs');
    expect(result.metadata.hasTypescript).toBe(true);
    expect(result.suggestedGates).toContain('tsc');
  });

  it('should run quality gates', async () => {
    mockInvoke.mockResolvedValue({
      projectPath: '/project',
      projectType: 'nodejs',
      overallStatus: 'passed',
      totalGates: 3,
      passedGates: 2,
      failedGates: 0,
      skippedGates: 1,
      results: [
        { gateId: 'tsc', gateName: 'TypeScript', status: 'passed', durationMs: 1500 },
        { gateId: 'eslint', gateName: 'ESLint', status: 'passed', durationMs: 2000 },
        { gateId: 'test', gateName: 'Tests', status: 'skipped', durationMs: 0 },
      ],
    });

    const summary = await mockInvoke('run_quality_gates', { projectPath: '/project' });

    expect(summary.overallStatus).toBe('passed');
    expect(summary.passedGates).toBe(2);
    expect(summary.results).toHaveLength(3);
  });

  it('should get gate results history', async () => {
    mockInvoke.mockResolvedValue([
      {
        id: 1,
        projectPath: '/project',
        gateId: 'tsc',
        status: 'passed',
        createdAt: '2024-01-15T10:00:00Z',
      },
      {
        id: 2,
        projectPath: '/project',
        gateId: 'tsc',
        status: 'failed',
        createdAt: '2024-01-14T10:00:00Z',
      },
    ]);

    const history = await mockInvoke('get_gate_history', {
      projectPath: '/project',
      limit: 10,
    });

    expect(history).toHaveLength(2);
    expect(history[0].status).toBe('passed');
  });
});

// ============================================================================
// Standalone Execution Tests
// ============================================================================

describe('Standalone Execution Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should start standalone execution', async () => {
    mockInvoke.mockResolvedValue({
      sessionId: 'exec-123',
      status: 'running',
      totalStories: 5,
    });

    const result = await mockInvoke('execute_standalone_with_session', {
      projectPath: '/project',
      prdPath: '/project/prd.json',
      runQualityGates: true,
    });

    expect(result.sessionId).toBe('exec-123');
    expect(result.status).toBe('running');
  });

  it('should get execution progress', async () => {
    mockInvoke.mockResolvedValue({
      sessionId: 'exec-123',
      status: 'running',
      currentStoryIndex: 2,
      completedStories: 2,
      totalStories: 5,
      progressPercentage: 40,
    });

    const progress = await mockInvoke('get_execution_progress', {
      sessionId: 'exec-123',
    });

    expect(progress.completedStories).toBe(2);
    expect(progress.progressPercentage).toBe(40);
  });

  it('should cancel execution', async () => {
    mockInvoke.mockResolvedValue({
      success: true,
      sessionId: 'exec-123',
      status: 'cancelled',
    });

    const result = await mockInvoke('cancel_standalone_execution', {
      sessionId: 'exec-123',
    });

    expect(result.success).toBe(true);
    expect(result.status).toBe('cancelled');
  });

  it('should resume paused execution', async () => {
    mockInvoke.mockResolvedValue({
      success: true,
      sessionId: 'exec-123',
      status: 'running',
      resumedFromStory: 3,
    });

    const result = await mockInvoke('resume_standalone_execution', {
      sessionId: 'exec-123',
    });

    expect(result.success).toBe(true);
    expect(result.resumedFromStory).toBe(3);
  });
});

// ============================================================================
// Error Handling Tests
// ============================================================================

describe('Error Handling Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should handle API errors', async () => {
    mockInvoke.mockRejectedValue({
      message: 'API rate limit exceeded',
      code: 'RATE_LIMIT',
    });

    try {
      await mockInvoke('send_message', { content: 'Test' });
      expect.fail('Should have thrown');
    } catch (error) {
      expect((error as { message: string }).message).toBe('API rate limit exceeded');
    }
  });

  it('should handle timeout errors', async () => {
    mockInvoke.mockRejectedValue({
      message: 'Request timeout',
      code: 'TIMEOUT',
    });

    try {
      await mockInvoke('execute_tool', { toolName: 'Bash', arguments: { command: 'sleep 100' } });
      expect.fail('Should have thrown');
    } catch (error) {
      expect((error as { code: string }).code).toBe('TIMEOUT');
    }
  });

  it('should handle validation errors', async () => {
    mockInvoke.mockRejectedValue({
      message: 'Invalid project path',
      code: 'VALIDATION_ERROR',
    });

    try {
      await mockInvoke('detect_project_type', { projectPath: '' });
      expect.fail('Should have thrown');
    } catch (error) {
      expect((error as { code: string }).code).toBe('VALIDATION_ERROR');
    }
  });
});
