import { useCallback, useEffect, useRef } from 'react';
import type {
  HandoffContextBundle,
  WorkflowMode,
  WorkflowSession,
  WorkflowSessionState,
} from '../../types/workflowKernel';
import i18n from '../../i18n';
import { useExecutionStore } from '../../store/execution';

const WORKFLOW_KERNEL_SESSION_STORAGE_PREFIX = 'simple_mode_workflow_kernel_session_v2:';

function workflowKernelSessionStorageKey(workspacePath: string | null): string {
  return `${WORKFLOW_KERNEL_SESSION_STORAGE_PREFIX}${workspacePath || '__default_workspace__'}`;
}

interface UseWorkflowKernelSessionBridgeParams {
  workspacePath: string;
  workflowMode: WorkflowMode;
  workflowKernelSessionId: string | null;
  workflowKernelSessionActiveMode: WorkflowMode | null;
  setWorkflowMode: (mode: WorkflowMode) => void;
  openWorkflowKernelSession: (
    initialMode?: WorkflowMode,
    initialContext?: HandoffContextBundle,
  ) => Promise<WorkflowSession | null>;
  recoverWorkflowKernelSession: (sessionId: string) => Promise<WorkflowSessionState | null>;
}

interface UseWorkflowKernelSessionBridgeResult {
  clearPersistedWorkflowKernelSessionId: () => void;
}

export function useWorkflowKernelSessionBridge({
  workspacePath,
  workflowMode,
  workflowKernelSessionId,
  workflowKernelSessionActiveMode,
  setWorkflowMode,
  openWorkflowKernelSession,
  recoverWorkflowKernelSession,
}: UseWorkflowKernelSessionBridgeParams): UseWorkflowKernelSessionBridgeResult {
  const kernelBootstrapInFlightRef = useRef(false);
  const interruptedNoticeSessionIdRef = useRef<string | null>(null);

  const persistWorkflowKernelSessionId = useCallback(
    (sessionId: string) => {
      if (typeof localStorage === 'undefined') return;
      localStorage.setItem(workflowKernelSessionStorageKey(workspacePath), sessionId);
    },
    [workspacePath],
  );

  const clearPersistedWorkflowKernelSessionId = useCallback(() => {
    if (typeof localStorage === 'undefined') return;
    localStorage.removeItem(workflowKernelSessionStorageKey(workspacePath));
  }, [workspacePath]);

  useEffect(() => {
    if (workflowKernelSessionId) return;
    if (kernelBootstrapInFlightRef.current) return;

    kernelBootstrapInFlightRef.current = true;
    const bootstrap = async () => {
      if (typeof localStorage !== 'undefined') {
        const persistedSessionId = localStorage.getItem(workflowKernelSessionStorageKey(workspacePath));
        if (persistedSessionId) {
          const recovered = await recoverWorkflowKernelSession(persistedSessionId);
          if (recovered?.session?.sessionId) {
            const interruptedByRestart = recovered.events.some((event) => {
              if (event.kind !== 'input_submitted') return false;
              const payload = event.payload as Record<string, unknown> | null;
              const metadata = (payload?.metadata as Record<string, unknown> | undefined) ?? undefined;
              return metadata?.reasonCode === 'interrupted_by_restart';
            });
            if (interruptedByRestart && interruptedNoticeSessionIdRef.current !== recovered.session.sessionId) {
              interruptedNoticeSessionIdRef.current = recovered.session.sessionId;
              useExecutionStore.getState().appendCard({
                cardType: 'workflow_info',
                cardId: `workflow-restart-interrupted-${Date.now()}`,
                data: {
                  level: 'warning',
                  message: i18n.t('simpleMode:workflow.recovered.interruptedByRestart', {
                    defaultValue:
                      'Execution was interrupted by app restart. Please retry from the current plan/task state.',
                  }),
                },
                interactive: false,
              });
            }
            kernelBootstrapInFlightRef.current = false;
            return;
          }
          localStorage.removeItem(workflowKernelSessionStorageKey(workspacePath));
        }
      }

      await openWorkflowKernelSession('chat', {
        conversationContext: [],
        artifactRefs: [],
        contextSources: ['simple_mode'],
        metadata: {
          entry: 'simple_mode_mount',
        },
      });
      kernelBootstrapInFlightRef.current = false;
    };

    void bootstrap().finally(() => {
      kernelBootstrapInFlightRef.current = false;
    });
  }, [workflowKernelSessionId, workspacePath, openWorkflowKernelSession, recoverWorkflowKernelSession]);

  useEffect(() => {
    if (!workflowKernelSessionId) return;
    persistWorkflowKernelSessionId(workflowKernelSessionId);
  }, [workflowKernelSessionId, persistWorkflowKernelSessionId]);

  useEffect(() => {
    if (!workflowKernelSessionActiveMode || workflowKernelSessionActiveMode === workflowMode) return;
    setWorkflowMode(workflowKernelSessionActiveMode);
  }, [workflowKernelSessionActiveMode, workflowMode, setWorkflowMode]);

  return {
    clearPersistedWorkflowKernelSessionId,
  };
}
