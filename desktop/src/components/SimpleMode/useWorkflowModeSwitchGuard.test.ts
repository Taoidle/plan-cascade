import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { TFunction } from 'i18next';
import { resolveModeSwitchBlockReason, useWorkflowModeSwitchGuard } from './useWorkflowModeSwitchGuard';

function createT(): TFunction<'simpleMode'> {
  return ((key: string, options?: { defaultValue?: string }) =>
    options?.defaultValue ?? key) as TFunction<'simpleMode'>;
}

function createBaseParams() {
  return {
    workflowMode: 'task' as const,
    isRunning: false,
    workflowPhase: 'idle',
    planPhase: 'idle',
    isTaskWorkflowActive: false,
    isPlanWorkflowActive: false,
    hasStructuredInterviewQuestion: false,
    hasPlanClarifyQuestion: false,
    setWorkflowMode: vi.fn(),
    transitionWorkflowKernelMode: vi.fn(),
    showToast: vi.fn(),
    t: createT(),
  };
}

describe('useWorkflowModeSwitchGuard', () => {
  it('flags task workflow as active when task phase is analyzing', () => {
    const reason = resolveModeSwitchBlockReason({
      isRunning: false,
      workflowMode: 'task',
      workflowPhase: 'analyzing',
      planPhase: 'idle',
      isTaskWorkflowActive: false,
      isPlanWorkflowActive: false,
      hasStructuredInterviewQuestion: false,
      hasPlanClarifyQuestion: false,
    });
    expect(reason).toBe('task_workflow_active');
  });

  it('prioritizes structured question pending over other states', () => {
    const reason = resolveModeSwitchBlockReason({
      isRunning: true,
      workflowMode: 'task',
      workflowPhase: 'executing',
      planPhase: 'executing',
      isTaskWorkflowActive: true,
      isPlanWorkflowActive: true,
      hasStructuredInterviewQuestion: true,
      hasPlanClarifyQuestion: false,
    });
    expect(reason).toBe('structured_question_pending');
  });

  it('opens confirmation dialog and blocks direct mode switch when guarded', () => {
    const params = createBaseParams();
    const { result } = renderHook(() => useWorkflowModeSwitchGuard({ ...params, workflowPhase: 'analyzing' }));

    act(() => {
      result.current.handleWorkflowModeChange('plan');
    });

    expect(result.current.modeSwitchConfirmOpen).toBe(true);
    expect(result.current.modeSwitchBlockReason).toBe('task_workflow_active');
    expect(params.setWorkflowMode).not.toHaveBeenCalled();
  });
});
