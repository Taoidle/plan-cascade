import { useCallback, useEffect, useRef, useState } from 'react';
import type { TFunction } from 'i18next';
import type { FileAttachmentData } from '../../types/attachment';
import type { WorkflowMode } from '../../types/workflowKernel';
import {
  clearPersistedSimpleChatQueue,
  loadPersistedSimpleChatQueue,
  persistSimpleChatQueue,
  snapshotQueueAttachments,
  type QueuedChatMessage,
} from './queuePersistence';

type ToastLevel = 'info' | 'success' | 'error';

interface UseQueuedChatMessagesParams {
  workspacePath: string;
  workflowMode: WorkflowMode;
  maxQueuedChatMessages: number;
  isRunning: boolean;
  isSubmitting: boolean;
  isAnalyzingStrategy: boolean;
  permissionRequest: unknown;
  isTaskWorkflowBusy: boolean;
  isPlanWorkflowBusy: boolean;
  attachments: FileAttachmentData[];
  addAttachment: (attachment: FileAttachmentData) => void;
  clearAttachments: () => void;
  handleFollowUp: (inputPrompt?: string) => Promise<void>;
  handleStart: (inputPrompt?: string) => Promise<void>;
  showToast: (message: string, level?: ToastLevel) => void;
  t: TFunction<'simpleMode'>;
}

interface UseQueuedChatMessagesResult {
  queuedChatMessages: QueuedChatMessage[];
  queueChatMessage: (
    prompt: string,
    submitAsFollowUp: boolean,
    mode: WorkflowMode,
    queuedAttachments: FileAttachmentData[],
  ) => void;
  removeQueuedChatMessage: (id: string) => void;
  clearQueuedChatMessages: () => void;
}

export function useQueuedChatMessages({
  workspacePath,
  workflowMode,
  maxQueuedChatMessages,
  isRunning,
  isSubmitting,
  isAnalyzingStrategy,
  permissionRequest,
  isTaskWorkflowBusy,
  isPlanWorkflowBusy,
  addAttachment,
  clearAttachments,
  handleFollowUp,
  handleStart,
  showToast,
  t,
}: UseQueuedChatMessagesParams): UseQueuedChatMessagesResult {
  const [queuedChatMessages, setQueuedChatMessages] = useState<QueuedChatMessage[]>([]);
  const queueIdRef = useRef(0);
  const queueDispatchInFlightRef = useRef(false);
  const hasHydratedQueueRef = useRef(false);
  const hasWarnedQueuePersistenceFailureRef = useRef(false);

  const clearQueuedChatMessages = useCallback(() => {
    setQueuedChatMessages([]);
  }, []);

  const removeQueuedChatMessage = useCallback((id: string) => {
    setQueuedChatMessages((prev) => prev.filter((msg) => msg.id !== id));
  }, []);

  const queueChatMessage = useCallback(
    (prompt: string, submitAsFollowUp: boolean, mode: WorkflowMode, queuedAttachments: FileAttachmentData[]) => {
      setQueuedChatMessages((prev) => {
        if (prev.length >= maxQueuedChatMessages) {
          showToast(
            t('workflow.queueLimitReached', {
              max: maxQueuedChatMessages,
              defaultValue: `Queue is full (max ${maxQueuedChatMessages} messages).`,
            }),
            'info',
          );
          return prev;
        }

        const { attachments, droppedCount } = snapshotQueueAttachments(queuedAttachments);
        if (droppedCount > 0) {
          showToast(
            t('workflow.queue.attachmentsDropped', {
              count: droppedCount,
              defaultValue:
                'Some queued attachments could not be saved. The message was queued without them; reattach after this run.',
            }),
            'info',
          );
        }

        const nextId = `queued-${Date.now()}-${queueIdRef.current++}`;
        return [...prev, { id: nextId, prompt, submitAsFollowUp, mode, attempts: 0, attachments }];
      });
    },
    [maxQueuedChatMessages, showToast, t],
  );

  useEffect(() => {
    if (hasHydratedQueueRef.current) return;
    hasHydratedQueueRef.current = true;

    if (typeof localStorage === 'undefined') return;
    const restored = loadPersistedSimpleChatQueue(localStorage, workspacePath, maxQueuedChatMessages);
    if (restored.length === 0) return;

    setQueuedChatMessages(restored);
    queueIdRef.current = restored.length;
    showToast(
      t('workflow.queue.recovered', {
        count: restored.length,
        defaultValue: `Recovered ${restored.length} queued chat message(s).`,
      }),
      'info',
    );
  }, [workspacePath, maxQueuedChatMessages, showToast, t]);

  useEffect(() => {
    if (!hasHydratedQueueRef.current || typeof localStorage === 'undefined') return;

    if (queuedChatMessages.length === 0) {
      clearPersistedSimpleChatQueue(localStorage);
      hasWarnedQueuePersistenceFailureRef.current = false;
      return;
    }

    const persisted = persistSimpleChatQueue(localStorage, queuedChatMessages, workspacePath);
    if (persisted) {
      hasWarnedQueuePersistenceFailureRef.current = false;
      return;
    }
    if (!hasWarnedQueuePersistenceFailureRef.current) {
      hasWarnedQueuePersistenceFailureRef.current = true;
      showToast(
        t('workflow.queue.persistenceFailed', {
          defaultValue:
            'Queued messages could not be persisted locally. Keep this window open, or remove large attachments and retry.',
        }),
        'error',
      );
    }
  }, [queuedChatMessages, workspacePath, showToast, t]);

  useEffect(() => {
    if (queuedChatMessages.length === 0) return;
    if (
      isRunning ||
      isSubmitting ||
      isAnalyzingStrategy ||
      permissionRequest ||
      isTaskWorkflowBusy ||
      isPlanWorkflowBusy
    ) {
      return;
    }
    if (queueDispatchInFlightRef.current) return;

    const [nextMessage] = queuedChatMessages;
    if (!nextMessage) return;
    if (nextMessage.mode !== workflowMode) return;

    queueDispatchInFlightRef.current = true;
    setQueuedChatMessages((prev) => prev.slice(1));
    const run = (async () => {
      clearAttachments();
      for (const queuedAttachment of nextMessage.attachments) {
        addAttachment(queuedAttachment);
      }
      if (nextMessage.attachments.length > 0) {
        showToast(
          t('workflow.queue.replayingAttachments', {
            count: nextMessage.attachments.length,
            defaultValue: `Restored ${nextMessage.attachments.length} queued attachment(s).`,
          }),
          'info',
        );
      }
      return nextMessage.submitAsFollowUp ? handleFollowUp(nextMessage.prompt) : handleStart(nextMessage.prompt);
    })();
    void Promise.resolve(run)
      .then(() => {
        showToast(
          t('workflow.queue.consumed', {
            defaultValue: 'Queued follow-up consumed.',
          }),
          'success',
        );
      })
      .catch((error) => {
        const retryCount = nextMessage.attempts + 1;
        if (retryCount <= 2) {
          setQueuedChatMessages((prev) => [
            {
              ...nextMessage,
              attempts: retryCount,
            },
            ...prev,
          ]);
          showToast(
            t('workflow.queue.retrying', {
              attempt: retryCount,
              defaultValue: `Queued follow-up failed, retrying (${retryCount}/2).`,
            }),
            'info',
          );
          return;
        }

        showToast(
          t('workflow.queue.failed', {
            defaultValue: 'Queued follow-up failed and was dropped.',
          }),
          'error',
        );
        if (error) {
          console.error('[simple-mode] queued follow-up failed', error);
        }
      })
      .finally(() => {
        queueDispatchInFlightRef.current = false;
      });
  }, [
    queuedChatMessages,
    workflowMode,
    isRunning,
    isSubmitting,
    isAnalyzingStrategy,
    permissionRequest,
    isTaskWorkflowBusy,
    isPlanWorkflowBusy,
    handleFollowUp,
    handleStart,
    clearAttachments,
    addAttachment,
    showToast,
    t,
  ]);

  return {
    queuedChatMessages,
    queueChatMessage,
    removeQueuedChatMessage,
    clearQueuedChatMessages,
  };
}
