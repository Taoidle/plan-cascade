import { useCallback, useEffect, useMemo, useRef } from 'react';
import type { TFunction } from 'i18next';
import type { FileAttachmentData, WorkspaceFileReferenceData } from '../../types/attachment';
import type { WorkflowMode } from '../../types/workflowKernel';
import { useSimpleQueueStore, selectNextQueueDispatchItem } from '../../store/simpleQueue';
import {
  clearPersistedSimpleChatQueue,
  loadPersistedSimpleChatQueueWithMeta,
  persistSimpleChatQueue,
  snapshotQueueAttachments,
  snapshotQueueReferences,
  type QueuePriority,
  type QueuedChatMessage,
} from './queuePersistence';

type ToastLevel = 'info' | 'success' | 'error';

interface UseQueuedChatMessagesParams {
  workspacePath: string;
  sessionId: string;
  workflowMode: WorkflowMode;
  maxQueuedChatMessages: number;
  isRunning: boolean;
  isAnalyzingStrategy: boolean;
  permissionRequest: unknown;
  isTaskWorkflowBusy: boolean;
  isPlanWorkflowBusy: boolean;
  attachments: FileAttachmentData[];
  references: WorkspaceFileReferenceData[];
  addAttachment: (attachment: FileAttachmentData) => void;
  clearAttachments: () => void;
  setWorkspaceReferences: (references: WorkspaceFileReferenceData[]) => void;
  handleFollowUp: (inputPrompt?: string) => Promise<void>;
  handleStart: (inputPrompt?: string) => Promise<void>;
  switchWorkflowModeForQueue: (targetMode: WorkflowMode) => Promise<boolean>;
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
    queuedReferences: WorkspaceFileReferenceData[],
    priority?: QueuePriority,
  ) => void;
  removeQueuedChatMessage: (id: string) => void;
  clearQueuedChatMessages: () => void;
  moveQueuedChatMessage: (id: string, direction: 'up' | 'down' | 'top' | 'bottom') => void;
  setQueuedChatMessagePriority: (id: string, priority: QueuePriority) => void;
  retryQueuedChatMessage: (id: string) => void;
}

export function useQueuedChatMessages({
  workspacePath,
  sessionId,
  workflowMode,
  maxQueuedChatMessages,
  isRunning,
  isAnalyzingStrategy,
  permissionRequest,
  isTaskWorkflowBusy,
  isPlanWorkflowBusy,
  addAttachment,
  clearAttachments,
  setWorkspaceReferences,
  handleFollowUp,
  handleStart,
  switchWorkflowModeForQueue,
  showToast,
  t,
}: UseQueuedChatMessagesParams): UseQueuedChatMessagesResult {
  const items = useSimpleQueueStore((s) => s.items);
  const hydrate = useSimpleQueueStore((s) => s.hydrate);
  const enqueue = useSimpleQueueStore((s) => s.enqueue);
  const remove = useSimpleQueueStore((s) => s.remove);
  const clearSession = useSimpleQueueStore((s) => s.clearSession);
  const move = useSimpleQueueStore((s) => s.move);
  const setPriority = useSimpleQueueStore((s) => s.setPriority);
  const markStatus = useSimpleQueueStore((s) => s.markStatus);
  const incrementAttempts = useSimpleQueueStore((s) => s.incrementAttempts);
  const resetForRetry = useSimpleQueueStore((s) => s.resetForRetry);
  const consume = useSimpleQueueStore((s) => s.consume);

  const queueDispatchInFlightRef = useRef(false);
  const hasHydratedQueueKeyRef = useRef<string>('');
  const hasWarnedQueuePersistenceFailureRef = useRef(false);

  const queuedChatMessages = useMemo(() => items.filter((item) => item.sessionId === sessionId), [items, sessionId]);

  const clearQueuedChatMessages = useCallback(() => {
    if (!sessionId) return;
    clearSession(sessionId);
    showToast(
      t('workflow.queue.cleared', {
        defaultValue: 'Queued messages cleared.',
      }),
      'info',
    );
  }, [clearSession, sessionId, showToast, t]);

  const removeQueuedChatMessage = useCallback(
    (id: string) => {
      remove(id);
    },
    [remove],
  );

  const moveQueuedChatMessage = useCallback(
    (id: string, direction: 'up' | 'down' | 'top' | 'bottom') => {
      move(id, direction);
    },
    [move],
  );

  const setQueuedChatMessagePriority = useCallback(
    (id: string, priority: QueuePriority) => {
      setPriority(id, priority);
    },
    [setPriority],
  );

  const retryQueuedChatMessage = useCallback(
    (id: string) => {
      resetForRetry(id);
      showToast(
        t('workflow.queue.retryQueuedManually', {
          defaultValue: 'Queued message marked for retry.',
        }),
        'info',
      );
    },
    [resetForRetry, showToast, t],
  );

  const queueChatMessage = useCallback(
    (
      prompt: string,
      submitAsFollowUp: boolean,
      mode: WorkflowMode,
      queuedAttachments: FileAttachmentData[],
      queuedReferences: WorkspaceFileReferenceData[],
      priority: QueuePriority = 'normal',
    ) => {
      if (!sessionId) {
        showToast(
          t('workflow.queue.sessionUnavailable', {
            defaultValue: 'Queue is unavailable before a workflow session is ready.',
          }),
          'error',
        );
        return;
      }

      const { attachments, droppedCount } = snapshotQueueAttachments(queuedAttachments);
      const references = snapshotQueueReferences(queuedReferences);
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

      const result = enqueue(
        {
          sessionId,
          prompt,
          submitAsFollowUp,
          mode,
          attachments,
          references,
          priority,
        },
        maxQueuedChatMessages,
      );

      if (!result.ok && result.reason === 'limit_reached') {
        showToast(
          t('workflow.queueLimitReached', {
            max: maxQueuedChatMessages,
            defaultValue: `Queue is full (max ${maxQueuedChatMessages} messages).`,
          }),
          'info',
        );
      }
    },
    [enqueue, maxQueuedChatMessages, sessionId, showToast, t],
  );

  useEffect(() => {
    const hydrationKey = `${workspacePath}::${sessionId}`;
    if (!sessionId) return;
    if (hasHydratedQueueKeyRef.current === hydrationKey) return;
    hasHydratedQueueKeyRef.current = hydrationKey;

    if (typeof localStorage === 'undefined') return;
    const restored = loadPersistedSimpleChatQueueWithMeta(
      localStorage,
      workspacePath,
      maxQueuedChatMessages,
      sessionId,
    );
    if (restored.queue.length === 0) return;

    hydrate(restored.queue);
    const restoredForSession = restored.queue.filter((item) => item.sessionId === sessionId).length;
    if (restoredForSession > 0) {
      showToast(
        t('workflow.queue.recovered', {
          count: restoredForSession,
          defaultValue: `Recovered ${restoredForSession} queued chat message(s).`,
        }),
        'info',
      );
    }

    if (restored.migratedFromVersion !== null) {
      showToast(
        t('workflow.queue.migratedToV4', {
          fromVersion: restored.migratedFromVersion,
          defaultValue: `Queue migrated from V${restored.migratedFromVersion} to V5.`,
        }),
        'info',
      );
    }

    if (restored.crossSessionCount > 0) {
      showToast(
        t('workflow.queue.crossSessionRecovered', {
          count: restored.crossSessionCount,
          defaultValue: `Recovered ${restored.crossSessionCount} queued message(s) from other sessions.`,
        }),
        'info',
      );
    }
  }, [workspacePath, maxQueuedChatMessages, sessionId, hydrate, showToast, t]);

  useEffect(() => {
    if (typeof localStorage === 'undefined') return;

    if (items.length === 0) {
      clearPersistedSimpleChatQueue(localStorage);
      hasWarnedQueuePersistenceFailureRef.current = false;
      return;
    }

    const persisted = persistSimpleChatQueue(localStorage, items, workspacePath);
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
  }, [items, workspacePath, showToast, t]);

  useEffect(() => {
    if (!sessionId || queuedChatMessages.length === 0) return;
    if (isRunning || isAnalyzingStrategy || permissionRequest || isTaskWorkflowBusy || isPlanWorkflowBusy) {
      return;
    }
    if (queueDispatchInFlightRef.current) return;

    const nextMessage = selectNextQueueDispatchItem(items, sessionId);
    if (!nextMessage) return;

    if (nextMessage.mode !== workflowMode) {
      queueDispatchInFlightRef.current = true;
      showToast(
        t('workflow.queue.autoSwitchingMode', {
          mode: nextMessage.mode,
          defaultValue: `Switching to ${nextMessage.mode} mode for queued message...`,
        }),
        'info',
      );
      void switchWorkflowModeForQueue(nextMessage.mode)
        .then((switched) => {
          if (switched) {
            showToast(
              t('workflow.queue.autoSwitchSuccess', {
                mode: nextMessage.mode,
                defaultValue: `Switched to ${nextMessage.mode} mode for queued message.`,
              }),
              'success',
            );
            return;
          }

          const blockedReason = t('workflow.queue.autoSwitchFailed', {
            mode: nextMessage.mode,
            defaultValue: `Failed to switch to ${nextMessage.mode} mode.`,
          });
          markStatus(nextMessage.id, 'blocked', blockedReason);
          showToast(blockedReason, 'error');
        })
        .catch((error) => {
          const message = error instanceof Error ? error.message : String(error);
          markStatus(nextMessage.id, 'blocked', message);
          showToast(
            t('workflow.queue.autoSwitchFailed', {
              mode: nextMessage.mode,
              defaultValue: `Failed to switch to ${nextMessage.mode} mode.`,
            }),
            'error',
          );
        })
        .finally(() => {
          queueDispatchInFlightRef.current = false;
        });
      return;
    }

    queueDispatchInFlightRef.current = true;
    markStatus(nextMessage.id, 'running', null);

    const run = (async () => {
      clearAttachments();
      setWorkspaceReferences(nextMessage.references);
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
        markStatus(nextMessage.id, 'succeeded', null);
        consume(nextMessage.id);
        showToast(
          t('workflow.queue.consumed', {
            defaultValue: 'Queued follow-up consumed.',
          }),
          'success',
        );
      })
      .catch((error) => {
        const message = error instanceof Error ? error.message : String(error);
        const retryCount = incrementAttempts(nextMessage.id, message);

        if (retryCount <= 2) {
          showToast(
            t('workflow.queue.retrying', {
              attempt: retryCount,
              defaultValue: `Queued follow-up failed, retrying (${retryCount}/2).`,
            }),
            'info',
          );
          return;
        }

        markStatus(nextMessage.id, 'failed', message);
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
    sessionId,
    queuedChatMessages.length,
    items,
    workflowMode,
    isRunning,
    isAnalyzingStrategy,
    permissionRequest,
    isTaskWorkflowBusy,
    isPlanWorkflowBusy,
    handleFollowUp,
    handleStart,
    clearAttachments,
    addAttachment,
    setWorkspaceReferences,
    showToast,
    t,
    markStatus,
    switchWorkflowModeForQueue,
    incrementAttempts,
    consume,
  ]);

  return {
    queuedChatMessages,
    queueChatMessage,
    removeQueuedChatMessage,
    clearQueuedChatMessages,
    moveQueuedChatMessage,
    setQueuedChatMessagePriority,
    retryQueuedChatMessage,
  };
}
