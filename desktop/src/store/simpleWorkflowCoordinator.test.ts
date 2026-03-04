import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { WorkflowMode, WorkflowSession } from '../types/workflowKernel';
import {
  cancelActiveWorkflow,
  startModeWithCompensation,
  submitWorkflowInputWithTracking,
} from './simpleWorkflowCoordinator';
import { useTaskModeStore } from './taskMode';
import { usePlanModeStore } from './planMode';
import { useWorkflowOrchestratorStore } from './workflowOrchestrator';
import { usePlanOrchestratorStore } from './planOrchestrator';
import { useWorkflowKernelStore } from './workflowKernel';

function createKernelSession(mode: WorkflowMode): WorkflowSession {
  return {
    sessionId: `kernel-${mode}`,
    status: 'active',
    activeMode: mode,
    modeSnapshots: {
      chat: null,
      task: null,
      plan: null,
    },
    handoffContext: {
      conversationContext: [],
      artifactRefs: [],
      contextSources: ['simple_mode'],
      metadata: {},
    },
    linkedModeSessions: {},
    lastError: null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    lastCheckpointId: null,
  };
}

describe('simpleWorkflowCoordinator', () => {
  beforeEach(() => {
    useWorkflowKernelStore.getState().reset();
    useTaskModeStore.getState().reset();
    usePlanModeStore.getState().reset();
    useWorkflowOrchestratorStore.getState().resetWorkflow();
    usePlanOrchestratorStore.getState().resetWorkflow();
  });

  it('does not start mode runtime when kernel submit fails', async () => {
    const transitionAndSubmitInput = vi.fn().mockResolvedValue(null);
    const startTaskWorkflow = vi.fn();
    const linkModeSession = vi.fn();
    const cancelKernelOperation = vi.fn();

    const result = await startModeWithCompensation({
      mode: 'task',
      prompt: 'ship task',
      source: 'composer',
      handoff: {
        conversationContext: [],
        artifactRefs: [],
        contextSources: ['simple_mode'],
        metadata: {},
      },
      transitionAndSubmitInput,
      linkModeSession,
      cancelKernelOperation,
      startChat: vi.fn(),
      startTaskWorkflow,
      startPlanWorkflow: vi.fn(),
    });

    expect(result).toEqual({
      ok: false,
      errorCode: 'kernel_submit_failed',
      session: null,
    });
    expect(startTaskWorkflow).not.toHaveBeenCalled();
    expect(linkModeSession).not.toHaveBeenCalled();
    expect(cancelKernelOperation).not.toHaveBeenCalled();
  });

  it('compensates with mode_start_failed when task start throws', async () => {
    const kernelSession = createKernelSession('task');
    const transitionAndSubmitInput = vi.fn().mockResolvedValueOnce(kernelSession).mockResolvedValueOnce(kernelSession);
    const cancelKernelOperation = vi.fn().mockResolvedValue(kernelSession);
    const taskCancel = vi.fn().mockResolvedValue(undefined);
    const taskExit = vi.fn().mockResolvedValue(undefined);

    useTaskModeStore.setState({
      sessionId: 'task-runtime-1',
      cancelOperation: taskCancel,
      exitTaskMode: taskExit,
    } as unknown as ReturnType<typeof useTaskModeStore.getState>);

    const result = await startModeWithCompensation({
      mode: 'task',
      prompt: 'ship task',
      source: 'composer',
      handoff: {
        conversationContext: [],
        artifactRefs: [],
        contextSources: ['simple_mode'],
        metadata: {},
      },
      transitionAndSubmitInput,
      linkModeSession: vi.fn(),
      cancelKernelOperation,
      startChat: vi.fn(),
      startTaskWorkflow: vi.fn().mockRejectedValue(new Error('start failed')),
      startPlanWorkflow: vi.fn(),
    });

    expect(result.ok).toBe(false);
    expect(result.errorCode).toBe('mode_start_failed');
    expect(taskCancel).toHaveBeenCalledTimes(1);
    expect(taskExit).toHaveBeenCalledTimes(1);
    expect(cancelKernelOperation).toHaveBeenCalledWith('mode_start_failed');
    expect(transitionAndSubmitInput).toHaveBeenNthCalledWith(
      2,
      'task',
      expect.objectContaining({
        type: 'system_phase_update',
        metadata: expect.objectContaining({
          reasonCode: 'mode_start_failed',
        }),
      }),
      undefined,
    );
  });

  it('compensates with mode_session_link_failed when link fails', async () => {
    const kernelSession = createKernelSession('task');
    const transitionAndSubmitInput = vi.fn().mockResolvedValueOnce(kernelSession).mockResolvedValueOnce(kernelSession);
    const cancelKernelOperation = vi.fn().mockResolvedValue(kernelSession);
    const taskCancel = vi.fn().mockResolvedValue(undefined);
    const taskExit = vi.fn().mockResolvedValue(undefined);
    const linkModeSession = vi.fn().mockResolvedValue(null);

    useTaskModeStore.setState({
      sessionId: 'task-runtime-2',
      cancelOperation: taskCancel,
      exitTaskMode: taskExit,
    } as unknown as ReturnType<typeof useTaskModeStore.getState>);

    const startTaskWorkflow = vi.fn().mockImplementation(async () => {
      useWorkflowOrchestratorStore.setState({ sessionId: 'task-mode-session-2' });
    });

    const result = await startModeWithCompensation({
      mode: 'task',
      prompt: 'ship task',
      source: 'queue_or_external',
      handoff: {
        conversationContext: [],
        artifactRefs: [],
        contextSources: ['simple_mode'],
        metadata: {},
      },
      transitionAndSubmitInput,
      linkModeSession,
      cancelKernelOperation,
      startChat: vi.fn(),
      startTaskWorkflow,
      startPlanWorkflow: vi.fn(),
    });

    expect(result.ok).toBe(false);
    expect(result.errorCode).toBe('mode_session_link_failed');
    expect(linkModeSession).toHaveBeenCalledWith('task', 'task-runtime-2');
    expect(taskCancel).toHaveBeenCalledTimes(1);
    expect(taskExit).toHaveBeenCalledTimes(1);
    expect(cancelKernelOperation).toHaveBeenCalledWith('mode_session_link_failed');
    expect(transitionAndSubmitInput).toHaveBeenNthCalledWith(
      2,
      'task',
      expect.objectContaining({
        type: 'system_phase_update',
        metadata: expect.objectContaining({
          reasonCode: 'mode_session_link_failed',
        }),
      }),
      undefined,
    );
  });

  it('injects clientRequestId metadata on tracked workflow input', async () => {
    const transitionAndSubmitInput = vi.fn().mockResolvedValue(createKernelSession('chat'));

    await submitWorkflowInputWithTracking({
      transitionAndSubmitInput,
      targetMode: 'chat',
      intent: {
        type: 'chat_message',
        content: 'hello',
        metadata: {
          source: 'composer',
        },
      },
    });

    expect(transitionAndSubmitInput).toHaveBeenCalledWith(
      'chat',
      expect.objectContaining({
        metadata: expect.objectContaining({
          source: 'composer',
          clientRequestId: expect.any(String),
        }),
      }),
      undefined,
    );
  });

  it('cancels task execution by runtime first and skips immediate kernel cancel in executing phase', async () => {
    const callOrder: string[] = [];
    const cancelTaskWorkflow = vi.fn().mockImplementation(async () => {
      callOrder.push('runtime');
    });
    const cancelKernelOperation = vi.fn().mockImplementation(async () => {
      callOrder.push('kernel');
      return createKernelSession('task');
    });

    await cancelActiveWorkflow({
      workflowMode: 'task',
      taskWorkflowCancelling: false,
      planWorkflowCancelling: false,
      isTaskExecuting: true,
      isPlanExecuting: false,
      cancelKernelOperation,
      cancelTaskWorkflow,
      cancelPlanWorkflow: vi.fn(),
    });

    expect(callOrder).toEqual(['runtime']);
    expect(cancelKernelOperation).not.toHaveBeenCalled();
  });

  it('cancels task and plan non-executing workflows in runtime-then-kernel order', async () => {
    const taskOrder: string[] = [];
    await cancelActiveWorkflow({
      workflowMode: 'task',
      taskWorkflowCancelling: false,
      planWorkflowCancelling: false,
      isTaskExecuting: false,
      isPlanExecuting: false,
      cancelKernelOperation: vi.fn().mockImplementation(async () => {
        taskOrder.push('kernel');
        return createKernelSession('task');
      }),
      cancelTaskWorkflow: vi.fn().mockImplementation(async () => {
        taskOrder.push('runtime');
      }),
      cancelPlanWorkflow: vi.fn(),
    });
    expect(taskOrder).toEqual(['runtime', 'kernel']);

    const planOrder: string[] = [];
    await cancelActiveWorkflow({
      workflowMode: 'plan',
      taskWorkflowCancelling: false,
      planWorkflowCancelling: false,
      isTaskExecuting: false,
      isPlanExecuting: false,
      cancelKernelOperation: vi.fn().mockImplementation(async () => {
        planOrder.push('kernel');
        return createKernelSession('plan');
      }),
      cancelTaskWorkflow: vi.fn(),
      cancelPlanWorkflow: vi.fn().mockImplementation(async () => {
        planOrder.push('runtime');
      }),
    });
    expect(planOrder).toEqual(['runtime', 'kernel']);
  });

  it('falls back to kernel cancellation with runtime_cancel_failed when runtime cancel fails', async () => {
    const cancelKernelOperation = vi.fn().mockResolvedValue(createKernelSession('task'));
    const cancelTaskWorkflow = vi.fn().mockRejectedValue(new Error('runtime cancel failed'));

    await expect(
      cancelActiveWorkflow({
        workflowMode: 'task',
        taskWorkflowCancelling: false,
        planWorkflowCancelling: false,
        isTaskExecuting: true,
        isPlanExecuting: false,
        cancelKernelOperation,
        cancelTaskWorkflow,
        cancelPlanWorkflow: vi.fn(),
      }),
    ).rejects.toThrow('runtime cancel failed');

    expect(cancelKernelOperation).toHaveBeenCalledWith('runtime_cancel_failed');
  });
});
