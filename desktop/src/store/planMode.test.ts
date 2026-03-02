import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

import { usePlanModeStore } from './planMode';

describe('planMode store', () => {
  beforeEach(() => {
    usePlanModeStore.getState().reset();
    vi.clearAllMocks();
  });

  it('fetches step output via get_step_output', async () => {
    usePlanModeStore.setState({ sessionId: 'plan-session-1' });
    mockInvoke.mockResolvedValueOnce({
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

    const output = await usePlanModeStore.getState().fetchStepOutput('S001');

    expect(output?.stepId).toBe('S001');
    expect(mockInvoke).toHaveBeenCalledWith('get_step_output', {
      sessionId: 'plan-session-1',
      stepId: 'S001',
    });
  });

  it('stores error when get_step_output fails', async () => {
    usePlanModeStore.setState({ sessionId: 'plan-session-1' });
    mockInvoke.mockResolvedValueOnce({
      success: false,
      data: null,
      error: 'No output available',
    });

    const output = await usePlanModeStore.getState().fetchStepOutput('S999');

    expect(output).toBeNull();
    expect(usePlanModeStore.getState().error).toBe('No output available');
  });
});
