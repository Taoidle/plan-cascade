import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockInvoke = vi.fn();
const mockResolveProviderBaseUrl = vi.fn((..._args: unknown[]) => 'https://resolved-base-url');
const mockSettingsState = {
  provider: 'anthropic',
  model: 'claude-sonnet-4-20250514',
};

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('./settings', () => ({
  useSettingsStore: {
    getState: () => mockSettingsState,
  },
}));

vi.mock('../lib/providers', () => ({
  resolveProviderBaseUrl: (provider: string, settings: unknown) => mockResolveProviderBaseUrl(provider, settings),
}));

import { usePlanModeStore } from './planMode';

describe('planMode store', () => {
  beforeEach(() => {
    usePlanModeStore.getState().reset();
    vi.clearAllMocks();
  });

  it('enters plan mode and returns session snapshot', async () => {
    mockInvoke.mockResolvedValueOnce({
      success: true,
      data: {
        sessionId: 'plan-session-1',
        phase: 'analyzing',
        analysis: { complexity: 'medium' },
        currentQuestion: null,
      },
      error: null,
    });

    const result = await usePlanModeStore
      .getState()
      .enterPlanMode(
        'Build auth flow',
        'openai',
        'gpt-4o',
        'https://api.example.com',
        '/tmp/project',
        undefined,
        'ctx',
      );

    expect(result?.sessionId).toBe('plan-session-1');
    expect(usePlanModeStore.getState().isLoading).toBe(false);
    expect(mockInvoke).toHaveBeenCalledWith('enter_plan_mode', {
      request: {
        description: 'Build auth flow',
        provider: 'openai',
        model: 'gpt-4o',
        baseUrl: 'https://api.example.com',
        projectPath: '/tmp/project',
        contextSources: null,
        conversationContext: 'ctx',
        locale: null,
      },
    });
  });

  it('generates plan using active session and returns payload', async () => {
    mockInvoke
      .mockResolvedValueOnce({
        success: true,
        data: {
          sessionId: 'plan-session-1',
          phase: 'analyzing',
          analysis: { complexity: 'medium' },
          currentQuestion: null,
        },
        error: null,
      })
      .mockResolvedValueOnce({
        success: true,
        data: {
          title: 'Execution Plan',
          description: 'Do work',
          steps: [],
          batches: [],
        },
        error: null,
      });

    await usePlanModeStore.getState().enterPlanMode('Build auth flow');
    const plan = await usePlanModeStore.getState().generatePlan('openai', 'gpt-4o', 'https://api.example.com');

    expect(plan?.title).toBe('Execution Plan');
    expect(usePlanModeStore.getState().isLoading).toBe(false);
    expect(mockInvoke).toHaveBeenLastCalledWith('generate_plan', {
      request: {
        sessionId: 'plan-session-1',
        provider: 'openai',
        model: 'gpt-4o',
        baseUrl: 'https://api.example.com',
        projectPath: null,
        contextSources: null,
        conversationContext: null,
        locale: null,
      },
    });
  });

  it('stores error when approve_plan fails', async () => {
    mockInvoke
      .mockResolvedValueOnce({
        success: true,
        data: {
          sessionId: 'plan-session-1',
          phase: 'analyzing',
          analysis: null,
          currentQuestion: null,
        },
        error: null,
      })
      .mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Approve failed',
      });

    await usePlanModeStore.getState().enterPlanMode('Build auth flow');
    const ok = await usePlanModeStore.getState().approvePlan({
      title: 'Execution Plan',
      domain: 'frontend',
      adapterName: 'default',
      description: 'Do work',
      steps: [],
      batches: [],
      editable: false,
    });

    expect(ok).toBe(false);
    expect(usePlanModeStore.getState().isLoading).toBe(false);
    expect(usePlanModeStore.getState().error).toBe('Approve failed');
  });

  it('retries a failed step via retry_plan_step command', async () => {
    mockInvoke
      .mockResolvedValueOnce({
        success: true,
        data: {
          sessionId: 'plan-session-1',
          phase: 'analyzing',
          analysis: null,
          currentQuestion: null,
        },
        error: null,
      })
      .mockResolvedValueOnce({
        success: true,
        data: true,
        error: null,
      });

    await usePlanModeStore.getState().enterPlanMode('Build auth flow');
    const ok = await usePlanModeStore.getState().retryPlanStep('step-2', 'openai', 'gpt-4o', 'https://api.example.com');

    expect(ok).toBe(true);
    expect(usePlanModeStore.getState().isLoading).toBe(false);
    expect(usePlanModeStore.getState().error).toBeNull();
    expect(mockInvoke).toHaveBeenLastCalledWith('retry_plan_step', {
      request: {
        sessionId: 'plan-session-1',
        stepId: 'step-2',
        provider: 'openai',
        model: 'gpt-4o',
        baseUrl: 'https://api.example.com',
        projectPath: null,
        contextSources: null,
        conversationContext: null,
        locale: null,
      },
    });
  });

  it('stores error when retry_plan_step fails', async () => {
    mockInvoke
      .mockResolvedValueOnce({
        success: true,
        data: {
          sessionId: 'plan-session-1',
          phase: 'analyzing',
          analysis: null,
          currentQuestion: null,
        },
        error: null,
      })
      .mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Step not retryable',
      });

    await usePlanModeStore.getState().enterPlanMode('Build auth flow');
    const ok = await usePlanModeStore.getState().retryPlanStep('step-2');

    expect(ok).toBe(false);
    expect(usePlanModeStore.getState().error).toBe('Step not retryable');
    expect(usePlanModeStore.getState().isLoading).toBe(false);
  });

  it('resets cancelling state when cancel_plan_execution fails', async () => {
    mockInvoke
      .mockResolvedValueOnce({
        success: true,
        data: {
          sessionId: 'plan-session-1',
          phase: 'analyzing',
          analysis: null,
          currentQuestion: null,
        },
        error: null,
      })
      .mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'Cannot cancel',
      });

    await usePlanModeStore.getState().enterPlanMode('Build auth flow');
    const ok = await usePlanModeStore.getState().cancelExecution();

    const state = usePlanModeStore.getState();
    expect(ok).toBe(false);
    expect(state.isCancelling).toBe(false);
    expect(state.error).toBe('Cannot cancel');
  });

  it('fetches step output via get_step_output', async () => {
    mockInvoke
      .mockResolvedValueOnce({
        success: true,
        data: {
          sessionId: 'plan-session-1',
          phase: 'analyzing',
          analysis: null,
          currentQuestion: null,
        },
        error: null,
      })
      .mockResolvedValueOnce({
        success: true,
        data: {
          stepId: 'S001',
          content: 'Step output',
          format: 'markdown',
          criteriaMet: [],
          artifacts: [],
        },
        error: null,
      });

    await usePlanModeStore.getState().enterPlanMode('Build auth flow');
    const output = await usePlanModeStore.getState().fetchStepOutput('S001');

    expect(output?.stepId).toBe('S001');
    expect(mockInvoke).toHaveBeenLastCalledWith('get_step_output', {
      sessionId: 'plan-session-1',
      stepId: 'S001',
    });
  });

  it('stores error when get_step_output fails', async () => {
    mockInvoke
      .mockResolvedValueOnce({
        success: true,
        data: {
          sessionId: 'plan-session-1',
          phase: 'analyzing',
          analysis: null,
          currentQuestion: null,
        },
        error: null,
      })
      .mockResolvedValueOnce({
        success: false,
        data: null,
        error: 'No output available',
      });

    await usePlanModeStore.getState().enterPlanMode('Build auth flow');
    const output = await usePlanModeStore.getState().fetchStepOutput('S999');

    expect(output).toBeNull();
    expect(usePlanModeStore.getState().error).toBe('No output available');
  });
});
