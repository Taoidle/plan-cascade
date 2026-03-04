import { useCallback, useMemo, useState } from 'react';
import type { TFunction } from 'i18next';
import { buildConversationHistory } from '../../lib/contextBridge';
import { useExecutionStore } from '../../store/execution';
import { switchModeSafely } from '../../store/simpleWorkflowCoordinator';
import type { HandoffContextBundle, WorkflowMode, WorkflowSession } from '../../types/workflowKernel';

type ToastLevel = 'info' | 'success' | 'error';

type ModeSwitchBlockReason =
  | 'running_execution'
  | 'task_workflow_active'
  | 'plan_workflow_active'
  | 'structured_question_pending'
  | null;

interface UseWorkflowModeSwitchGuardParams {
  workflowMode: WorkflowMode;
  isRunning: boolean;
  workflowPhase: string;
  planPhase: string;
  isTaskWorkflowActive: boolean;
  isPlanWorkflowActive: boolean;
  hasStructuredInterviewQuestion: boolean;
  hasPlanClarifyQuestion: boolean;
  setWorkflowMode: (mode: WorkflowMode) => void;
  transitionWorkflowKernelMode: (
    targetMode: WorkflowMode,
    handoff: HandoffContextBundle,
  ) => Promise<WorkflowSession | null>;
  showToast: (message: string, level?: ToastLevel) => void;
  t: TFunction<'simpleMode'>;
}

interface UseWorkflowModeSwitchGuardResult {
  modeSwitchConfirmOpen: boolean;
  modeSwitchBlockReason: ModeSwitchBlockReason;
  handleWorkflowModeChange: (newMode: WorkflowMode) => void;
  handleConfirmModeSwitch: () => void;
  handleModeSwitchDialogOpenChange: (open: boolean) => void;
}

const ACTIVE_TASK_PHASES = new Set([
  'analyzing',
  'configuring',
  'interviewing',
  'exploring',
  'requirement_analysis',
  'generating_prd',
  'reviewing_prd',
  'architecture_review',
  'generating_design_doc',
  'executing',
]);

const ACTIVE_PLAN_PHASES = new Set([
  'analyzing',
  'clarifying',
  'clarification_error',
  'planning',
  'reviewing_plan',
  'executing',
]);

export function resolveModeSwitchBlockReason(params: {
  isRunning: boolean;
  workflowMode: WorkflowMode;
  workflowPhase: string;
  planPhase: string;
  isTaskWorkflowActive: boolean;
  isPlanWorkflowActive: boolean;
  hasStructuredInterviewQuestion: boolean;
  hasPlanClarifyQuestion: boolean;
}): ModeSwitchBlockReason {
  if (params.hasStructuredInterviewQuestion || params.hasPlanClarifyQuestion) {
    return 'structured_question_pending';
  }
  if (params.isRunning) {
    return 'running_execution';
  }

  const taskActive =
    params.isTaskWorkflowActive ||
    (params.workflowMode === 'task' && ACTIVE_TASK_PHASES.has((params.workflowPhase || '').toLowerCase()));
  if (taskActive) {
    return 'task_workflow_active';
  }

  const planActive =
    params.isPlanWorkflowActive ||
    (params.workflowMode === 'plan' && ACTIVE_PLAN_PHASES.has((params.planPhase || '').toLowerCase()));
  if (planActive) {
    return 'plan_workflow_active';
  }

  return null;
}

export function useWorkflowModeSwitchGuard({
  workflowMode,
  isRunning,
  workflowPhase,
  planPhase,
  isTaskWorkflowActive,
  isPlanWorkflowActive,
  hasStructuredInterviewQuestion,
  hasPlanClarifyQuestion,
  setWorkflowMode,
  transitionWorkflowKernelMode,
  showToast,
  t,
}: UseWorkflowModeSwitchGuardParams): UseWorkflowModeSwitchGuardResult {
  const [pendingModeSwitch, setPendingModeSwitch] = useState<WorkflowMode | null>(null);
  const [modeSwitchConfirmOpen, setModeSwitchConfirmOpen] = useState(false);
  const [modeSwitchBlockReason, setModeSwitchBlockReason] = useState<ModeSwitchBlockReason>(null);

  const effectiveBlockReason = useMemo(
    () =>
      resolveModeSwitchBlockReason({
        isRunning,
        workflowMode,
        workflowPhase,
        planPhase,
        isTaskWorkflowActive,
        isPlanWorkflowActive,
        hasStructuredInterviewQuestion,
        hasPlanClarifyQuestion,
      }),
    [
      hasPlanClarifyQuestion,
      hasStructuredInterviewQuestion,
      isPlanWorkflowActive,
      isRunning,
      isTaskWorkflowActive,
      planPhase,
      workflowMode,
      workflowPhase,
    ],
  );

  const applyWorkflowModeChange = useCallback(
    (newMode: WorkflowMode) => {
      if (newMode === workflowMode) return;

      const executionSnapshot = useExecutionStore.getState();
      const latestStreamingOutput = executionSnapshot.streamingOutput;
      const hasChatHistory = latestStreamingOutput.length > 0;
      const hasPendingTaskContext = executionSnapshot._pendingTaskContext;

      if (newMode === 'task' && hasChatHistory) {
        showToast(
          t('contextBridge.switchToTaskWithContext', { defaultValue: 'Switching to Task mode with chat context' }),
          'info',
        );
      } else if (newMode === 'plan' && hasChatHistory) {
        showToast(
          t('contextBridge.switchToPlanWithContext', { defaultValue: 'Switching to Plan mode with chat context' }),
          'info',
        );
      } else if (newMode === 'chat' && hasPendingTaskContext) {
        showToast(
          t('contextBridge.switchToChatWithTaskContext', { defaultValue: 'Switching to Chat mode with task context' }),
          'info',
        );
      }

      if (workflowMode === 'chat' && newMode === 'task' && hasChatHistory) {
        const contextSummary = latestStreamingOutput
          .slice(-20)
          .map((line) => line.content)
          .join('\n');
        useExecutionStore.getState().setPendingTaskContext(contextSummary);
      }

      const conversationContext = buildConversationHistory().map((turn) => ({
        user: turn.user,
        assistant: turn.assistant,
      }));

      void (async () => {
        const transitioned = await switchModeSafely({
          targetMode: newMode,
          handoff: {
            conversationContext,
            artifactRefs: [],
            contextSources: ['simple_mode'],
            metadata: {
              sourceMode: workflowMode,
              targetMode: newMode,
              hasChatHistory,
              hasPendingTaskContext: !!hasPendingTaskContext,
              switchedAt: new Date().toISOString(),
            },
          },
          transitionWorkflowKernelMode,
        });

        if (!transitioned) {
          showToast(
            t('workflow.modeSwitchFailed', {
              defaultValue: 'Failed to switch workflow mode. Please retry.',
            }),
            'error',
          );
          return;
        }

        setWorkflowMode(transitioned.activeMode);
      })();
    },
    [setWorkflowMode, showToast, t, transitionWorkflowKernelMode, workflowMode],
  );

  const handleWorkflowModeChange = useCallback(
    (newMode: WorkflowMode) => {
      if (newMode === workflowMode) return;
      if (effectiveBlockReason) {
        setPendingModeSwitch(newMode);
        setModeSwitchBlockReason(effectiveBlockReason);
        setModeSwitchConfirmOpen(true);
        return;
      }
      setModeSwitchBlockReason(null);
      applyWorkflowModeChange(newMode);
    },
    [applyWorkflowModeChange, effectiveBlockReason, workflowMode],
  );

  const handleConfirmModeSwitch = useCallback(() => {
    const nextMode = pendingModeSwitch;
    setPendingModeSwitch(null);
    setModeSwitchConfirmOpen(false);
    setModeSwitchBlockReason(null);
    if (!nextMode) return;
    applyWorkflowModeChange(nextMode);
  }, [applyWorkflowModeChange, pendingModeSwitch]);

  const handleModeSwitchDialogOpenChange = useCallback((open: boolean) => {
    setModeSwitchConfirmOpen(open);
    if (!open) {
      setPendingModeSwitch(null);
      setModeSwitchBlockReason(null);
    }
  }, []);

  return {
    modeSwitchConfirmOpen,
    modeSwitchBlockReason,
    handleWorkflowModeChange,
    handleConfirmModeSwitch,
    handleModeSwitchDialogOpenChange,
  };
}
