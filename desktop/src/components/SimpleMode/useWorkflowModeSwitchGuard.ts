import { useCallback, useMemo, useState } from 'react';
import type { TFunction } from 'i18next';
import { buildConversationHistory } from '../../lib/contextBridge';
import { switchModeSafely } from '../../store/simpleWorkflowCoordinator';
import type { HandoffContextBundle, WorkflowMode, WorkflowSession } from '../../types/workflowKernel';
import { resolveModeSwitchBlockReasonFromKernel, type ModeSwitchBlockReason } from '../../store/workflowPhaseModel';

type ToastLevel = 'info' | 'success' | 'error';

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
  appendWorkflowKernelContextItems: (
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
  return resolveModeSwitchBlockReasonFromKernel(params);
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
  appendWorkflowKernelContextItems,
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

      const conversationContext = buildConversationHistory().map((turn) => ({
        user: turn.user,
        assistant: turn.assistant,
      }));
      const hasConversationContext = conversationContext.length > 0;

      if (newMode === 'task' && hasConversationContext) {
        showToast(
          t('contextBridge.switchToTaskWithContext', { defaultValue: 'Switching to Task mode with chat context' }),
          'info',
        );
      } else if (newMode === 'plan' && hasConversationContext) {
        showToast(
          t('contextBridge.switchToPlanWithContext', { defaultValue: 'Switching to Plan mode with chat context' }),
          'info',
        );
      } else if (newMode === 'chat' && hasConversationContext) {
        showToast(
          t('contextBridge.switchToChatWithTaskContext', { defaultValue: 'Switching to Chat mode with task context' }),
          'info',
        );
      }

      void (async () => {
        if (workflowMode === 'chat' && hasConversationContext) {
          await appendWorkflowKernelContextItems('chat', {
            conversationContext,
            artifactRefs: [],
            contextSources: ['mode_switch_sync'],
            metadata: {
              source: 'simple_mode_switch_guard',
              sourceMode: workflowMode,
              targetMode: newMode,
            },
          });
        }

        const transitioned = await switchModeSafely({
          targetMode: newMode,
          handoff: {
            conversationContext: [],
            artifactRefs: [],
            contextSources: ['simple_mode'],
            metadata: {
              sourceMode: workflowMode,
              targetMode: newMode,
              hasConversationContext,
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
    [appendWorkflowKernelContextItems, setWorkflowMode, showToast, t, transitionWorkflowKernelMode, workflowMode],
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
