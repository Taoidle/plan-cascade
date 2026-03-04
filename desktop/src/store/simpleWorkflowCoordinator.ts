import type { HandoffContextBundle, UserInputIntent, WorkflowMode, WorkflowSession } from '../types/workflowKernel';
import type { PlanEditOperation } from '../types/workflowKernel';
import { withWorkflowClientRequestMetadata } from '../lib/workflowClientRequest';
import { submitWorkflowKernelActionIntent, type SubmitKernelIntentSpec } from '../lib/workflowKernelIntent';
import { useTaskModeStore } from './taskMode';
import { usePlanModeStore } from './planMode';
import { useWorkflowOrchestratorStore } from './workflowOrchestrator';
import { usePlanOrchestratorStore } from './planOrchestrator';
import { useWorkflowKernelStore } from './workflowKernel';

export interface SubmitWorkflowInputParams {
  transitionAndSubmitInput: (
    targetMode: WorkflowMode,
    intent: UserInputIntent,
    handoff?: HandoffContextBundle,
  ) => Promise<WorkflowSession | null>;
  targetMode: WorkflowMode;
  intent: UserInputIntent;
  handoff?: HandoffContextBundle;
}

export interface StartModeParams {
  mode: WorkflowMode;
  prompt: string;
  source: 'composer' | 'queue_or_external';
  handoff: HandoffContextBundle;
  transitionAndSubmitInput: (
    targetMode: WorkflowMode,
    intent: UserInputIntent,
    handoff?: HandoffContextBundle,
  ) => Promise<WorkflowSession | null>;
  linkModeSession: (mode: WorkflowMode, modeSessionId: string) => Promise<WorkflowSession | null>;
  cancelKernelOperation: (reason?: string) => Promise<WorkflowSession | null>;
  startChat: (prompt: string, source: 'simple') => Promise<void>;
  startTaskWorkflow: (description: string) => Promise<void>;
  startPlanWorkflow: (description: string) => Promise<void>;
}

export interface StartModeResult {
  ok: boolean;
  errorCode: 'kernel_submit_failed' | 'mode_start_failed' | 'mode_session_link_failed' | null;
  session: WorkflowSession | null;
}

export async function submitWorkflowInputWithTracking({
  transitionAndSubmitInput,
  targetMode,
  intent,
  handoff,
}: SubmitWorkflowInputParams): Promise<WorkflowSession | null> {
  const metadata = withWorkflowClientRequestMetadata(
    (intent.metadata ?? {}) as Record<string, unknown>,
    targetMode,
    intent.type,
  );

  return transitionAndSubmitInput(
    targetMode,
    {
      ...intent,
      metadata,
    },
    handoff,
  );
}

async function rollbackModeRuntime(mode: WorkflowMode): Promise<void> {
  if (mode === 'task') {
    const taskStore = useTaskModeStore.getState();
    if (taskStore.sessionId) {
      try {
        await taskStore.cancelOperation();
      } catch {
        // best effort cleanup
      }
      try {
        await taskStore.exitTaskMode();
      } catch {
        // best effort cleanup
      }
    }
    useWorkflowOrchestratorStore.getState().resetWorkflow();
    return;
  }

  if (mode === 'plan') {
    const planStore = usePlanModeStore.getState();
    if (planStore.sessionId) {
      try {
        await planStore.cancelOperation();
      } catch {
        // best effort cleanup
      }
      try {
        await planStore.exitPlanMode();
      } catch {
        // best effort cleanup
      }
    }
    usePlanOrchestratorStore.getState().resetWorkflow();
  }
}

async function compensateStartFailure(
  mode: WorkflowMode,
  reasonCode: 'mode_start_failed' | 'mode_session_link_failed',
  transitionAndSubmitInput: SubmitWorkflowInputParams['transitionAndSubmitInput'],
  cancelKernelOperation: StartModeParams['cancelKernelOperation'],
): Promise<void> {
  await rollbackModeRuntime(mode);

  try {
    await submitWorkflowInputWithTracking({
      transitionAndSubmitInput,
      targetMode: mode,
      intent: {
        type: 'system_phase_update',
        content: 'phase:failed',
        metadata: {
          mode,
          phase: 'failed',
          reasonCode,
        },
      },
    });
  } catch {
    // best effort kernel trace
  }

  try {
    await cancelKernelOperation(reasonCode);
  } catch {
    // best effort kernel cancellation
  }
}

export async function startModeWithCompensation({
  mode,
  prompt,
  source,
  handoff,
  transitionAndSubmitInput,
  linkModeSession,
  cancelKernelOperation,
  startChat,
  startTaskWorkflow,
  startPlanWorkflow,
}: StartModeParams): Promise<StartModeResult> {
  const kernelSession = await submitWorkflowInputWithTracking({
    transitionAndSubmitInput,
    targetMode: mode,
    intent: {
      type: 'mode_entry_prompt',
      content: prompt,
      metadata: {
        mode,
        source,
      },
    },
    handoff,
  });

  if (!kernelSession) {
    return { ok: false, errorCode: 'kernel_submit_failed', session: null };
  }

  try {
    if (mode === 'task') {
      await startTaskWorkflow(prompt);
      const taskModeSessionId = useWorkflowOrchestratorStore.getState().sessionId;
      if (!taskModeSessionId) {
        await compensateStartFailure(mode, 'mode_start_failed', transitionAndSubmitInput, cancelKernelOperation);
        return { ok: false, errorCode: 'mode_start_failed', session: kernelSession };
      }

      const linked = await linkModeSession('task', taskModeSessionId);
      if (!linked) {
        await compensateStartFailure(mode, 'mode_session_link_failed', transitionAndSubmitInput, cancelKernelOperation);
        return { ok: false, errorCode: 'mode_session_link_failed', session: kernelSession };
      }

      return { ok: true, errorCode: null, session: linked };
    }

    if (mode === 'plan') {
      await startPlanWorkflow(prompt);
      const planModeSessionId = usePlanOrchestratorStore.getState().sessionId;
      if (!planModeSessionId) {
        await compensateStartFailure(mode, 'mode_start_failed', transitionAndSubmitInput, cancelKernelOperation);
        return { ok: false, errorCode: 'mode_start_failed', session: kernelSession };
      }

      const linked = await linkModeSession('plan', planModeSessionId);
      if (!linked) {
        await compensateStartFailure(mode, 'mode_session_link_failed', transitionAndSubmitInput, cancelKernelOperation);
        return { ok: false, errorCode: 'mode_session_link_failed', session: kernelSession };
      }

      return { ok: true, errorCode: null, session: linked };
    }

    await startChat(prompt, 'simple');
    return { ok: true, errorCode: null, session: kernelSession };
  } catch {
    if (mode === 'task' || mode === 'plan') {
      await compensateStartFailure(mode, 'mode_start_failed', transitionAndSubmitInput, cancelKernelOperation);
    } else {
      try {
        await cancelKernelOperation('mode_start_failed');
      } catch {
        // best effort cleanup
      }
    }
    return { ok: false, errorCode: 'mode_start_failed', session: kernelSession };
  }
}

export async function switchModeSafely(params: {
  targetMode: WorkflowMode;
  handoff: HandoffContextBundle;
  transitionWorkflowKernelMode: (
    targetMode: WorkflowMode,
    handoff: HandoffContextBundle,
  ) => Promise<WorkflowSession | null>;
}): Promise<WorkflowSession | null> {
  const handoff = {
    ...params.handoff,
    metadata: withWorkflowClientRequestMetadata(
      (params.handoff.metadata ?? {}) as Record<string, unknown>,
      params.targetMode,
      'mode_transition',
    ),
  };

  return params.transitionWorkflowKernelMode(params.targetMode, handoff);
}

export async function cancelActiveWorkflow(params: {
  workflowMode: WorkflowMode;
  taskWorkflowCancelling: boolean;
  planWorkflowCancelling: boolean;
  isTaskExecuting: boolean;
  isPlanExecuting: boolean;
  cancelKernelOperation: (reason?: string) => Promise<WorkflowSession | null>;
  cancelTaskWorkflow: () => Promise<void>;
  cancelPlanWorkflow: () => Promise<void>;
}): Promise<void> {
  if (params.taskWorkflowCancelling || params.planWorkflowCancelling) return;

  if (params.workflowMode === 'chat') {
    await params.cancelKernelOperation('cancelled_by_user');
    return;
  }

  if (params.workflowMode === 'plan') {
    try {
      await params.cancelPlanWorkflow();
    } catch (error) {
      await params.cancelKernelOperation('runtime_cancel_failed');
      throw error;
    }
    if (!params.isPlanExecuting) {
      await params.cancelKernelOperation('cancelled_by_user');
    }
    return;
  }
  if (params.workflowMode === 'task') {
    try {
      await params.cancelTaskWorkflow();
    } catch (error) {
      await params.cancelKernelOperation('runtime_cancel_failed');
      throw error;
    }
    if (!params.isTaskExecuting) {
      await params.cancelKernelOperation('cancelled_by_user');
    }
    return;
  }

  await params.cancelKernelOperation('cancelled_by_user');
}

export async function submitWorkflowActionIntentViaCoordinator(
  spec: Omit<SubmitKernelIntentSpec, 'transitionAndSubmitInput'>,
): Promise<unknown> {
  return submitWorkflowKernelActionIntent({
    ...spec,
    transitionAndSubmitInput: useWorkflowKernelStore.getState().transitionAndSubmitInput,
  });
}

export async function applyPlanEditViaCoordinator(operation: PlanEditOperation): Promise<WorkflowSession | null> {
  return useWorkflowKernelStore.getState().applyPlanEdit(operation);
}

export async function executePlanViaCoordinator(): Promise<WorkflowSession | null> {
  return useWorkflowKernelStore.getState().executePlan();
}

export async function retryPlanStepViaCoordinator(stepId: string): Promise<WorkflowSession | null> {
  return useWorkflowKernelStore.getState().retryStep(stepId);
}
