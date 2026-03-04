import { useCallback, useState } from 'react';
import type { TFunction } from 'i18next';
import { buildConversationHistory } from '../../lib/contextBridge';
import { useExecutionStore } from '../../store/execution';
import type { StreamLine } from '../../store/execution/types';
import type { HandoffContextBundle, WorkflowMode, WorkflowSession } from '../../types/workflowKernel';

type ToastLevel = 'info' | 'success' | 'error';

interface UseWorkflowModeSwitchGuardParams {
  workflowMode: WorkflowMode;
  isRunning: boolean;
  streamingOutput: StreamLine[];
  queuedChatMessagesLength: number;
  clearQueuedChatMessages: () => void;
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
  handleWorkflowModeChange: (newMode: WorkflowMode) => void;
  handleConfirmModeSwitch: () => void;
  handleModeSwitchDialogOpenChange: (open: boolean) => void;
}

export function useWorkflowModeSwitchGuard({
  workflowMode,
  isRunning,
  streamingOutput,
  queuedChatMessagesLength,
  clearQueuedChatMessages,
  setWorkflowMode,
  transitionWorkflowKernelMode,
  showToast,
  t,
}: UseWorkflowModeSwitchGuardParams): UseWorkflowModeSwitchGuardResult {
  const [pendingModeSwitch, setPendingModeSwitch] = useState<WorkflowMode | null>(null);
  const [modeSwitchConfirmOpen, setModeSwitchConfirmOpen] = useState(false);

  const applyWorkflowModeChange = useCallback(
    (newMode: WorkflowMode) => {
      if (newMode === workflowMode) return;

      const hasChatHistory = streamingOutput.length > 0;
      const hasPendingTaskContext = useExecutionStore.getState()._pendingTaskContext;

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

      if (queuedChatMessagesLength > 0) {
        clearQueuedChatMessages();
        showToast(
          t('workflow.clearQueuedMessages', {
            defaultValue: 'Cleared queued follow-up messages when switching workflow mode.',
          }),
          'info',
        );
      }

      if (workflowMode === 'chat' && newMode === 'task' && hasChatHistory) {
        const contextSummary = streamingOutput
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
        const transitioned = await transitionWorkflowKernelMode(newMode, {
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
    [
      clearQueuedChatMessages,
      queuedChatMessagesLength,
      setWorkflowMode,
      showToast,
      streamingOutput,
      t,
      transitionWorkflowKernelMode,
      workflowMode,
    ],
  );

  const handleWorkflowModeChange = useCallback(
    (newMode: WorkflowMode) => {
      if (newMode === workflowMode) return;
      if (isRunning) {
        setPendingModeSwitch(newMode);
        setModeSwitchConfirmOpen(true);
        return;
      }
      applyWorkflowModeChange(newMode);
    },
    [applyWorkflowModeChange, isRunning, workflowMode],
  );

  const handleConfirmModeSwitch = useCallback(() => {
    const nextMode = pendingModeSwitch;
    setPendingModeSwitch(null);
    setModeSwitchConfirmOpen(false);
    if (!nextMode) return;
    applyWorkflowModeChange(nextMode);
  }, [applyWorkflowModeChange, pendingModeSwitch]);

  const handleModeSwitchDialogOpenChange = useCallback((open: boolean) => {
    setModeSwitchConfirmOpen(open);
    if (!open) {
      setPendingModeSwitch(null);
    }
  }, []);

  return {
    modeSwitchConfirmOpen,
    handleWorkflowModeChange,
    handleConfirmModeSwitch,
    handleModeSwitchDialogOpenChange,
  };
}
