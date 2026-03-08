import { useCallback, useEffect, useRef } from 'react';
import type {
  HandoffContextBundle,
  WorkflowMode,
  WorkflowSession,
  WorkflowSessionState,
} from '../../types/workflowKernel';
import i18n from '../../i18n';
import { useExecutionStore } from '../../store/execution';
import { useSimpleSessionStore } from '../../store/simpleSessionStore';

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
  getWorkflowKernelCatalogState: () => Promise<{
    activeSessionId: string | null;
    sessions: Array<{ sessionId: string }>;
  } | null>;
  activateWorkflowKernelSession: (sessionId: string) => Promise<WorkflowSessionState | null>;
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
  getWorkflowKernelCatalogState,
  activateWorkflowKernelSession,
}: UseWorkflowKernelSessionBridgeParams): UseWorkflowKernelSessionBridgeResult {
  const kernelBootstrapInFlightRef = useRef(false);
  const interruptedNoticeSessionIdRef = useRef<string | null>(null);

  const clearPersistedWorkflowKernelSessionId = useCallback(() => {
    useSimpleSessionStore.getState().setActiveRootSessionId(null);
  }, []);

  useEffect(() => {
    if (workflowKernelSessionId) return;
    if (kernelBootstrapInFlightRef.current) return;

    kernelBootstrapInFlightRef.current = true;
    const bootstrap = async () => {
      const catalogState = await getWorkflowKernelCatalogState();
      const persistedSessionId =
        useSimpleSessionStore.getState().activeRootSessionId || catalogState?.activeSessionId || null;

      if (persistedSessionId) {
        const activated = await activateWorkflowKernelSession(persistedSessionId);
        const recovered = activated ?? (await recoverWorkflowKernelSession(persistedSessionId));
        if (recovered?.session?.sessionId) {
          useSimpleSessionStore.getState().setActiveRootSessionId(recovered.session.sessionId);
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
      }

      const opened = await openWorkflowKernelSession('chat', {
        conversationContext: [],
        summaryItems: [],
        artifactRefs: [],
        contextSources: ['simple_mode'],
        metadata: {
          entry: 'simple_mode_mount',
          workspacePath,
        },
      });
      if (opened?.sessionId) {
        useSimpleSessionStore.getState().setActiveRootSessionId(opened.sessionId);
      }
      kernelBootstrapInFlightRef.current = false;
    };

    void bootstrap().finally(() => {
      kernelBootstrapInFlightRef.current = false;
    });
  }, [
    activateWorkflowKernelSession,
    getWorkflowKernelCatalogState,
    openWorkflowKernelSession,
    recoverWorkflowKernelSession,
    workflowKernelSessionId,
    workspacePath,
  ]);

  useEffect(() => {
    if (!workflowKernelSessionId) return;
    useSimpleSessionStore.getState().setActiveRootSessionId(workflowKernelSessionId);
  }, [workflowKernelSessionId]);

  useEffect(() => {
    if (!workflowKernelSessionActiveMode || workflowKernelSessionActiveMode === workflowMode) return;
    setWorkflowMode(workflowKernelSessionActiveMode);
  }, [workflowKernelSessionActiveMode, workflowMode, setWorkflowMode]);

  return {
    clearPersistedWorkflowKernelSessionId,
  };
}
