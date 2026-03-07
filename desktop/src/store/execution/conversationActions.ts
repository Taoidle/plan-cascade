import { invoke } from '@tauri-apps/api/core';
import { deriveConversationTurns, getNextTurnId, rebuildStandaloneTurns } from '../../lib/conversationUtils';
import { reportNonFatal } from '../../lib/nonFatal';
import type { FileAttachmentData } from '../../types/attachment';
import type { ContextSourceConfig } from '../../types/contextSources';
import { useContextSourcesStore } from '../contextSources';
import { useSettingsStore } from '../settings';
import { useToolPermissionStore } from '../toolPermission';
import { useWorkflowKernelStore } from '../workflowKernel';
import {
  DEFAULT_MODEL_BY_PROVIDER,
  isClaudeCodeBackend,
  resolveProviderBaseUrl,
  resolveStandaloneProvider,
} from './providerUtils';
import { ensurePromptContent, extractPluginInvocationsFromPrompt } from './messageDispatch';
import { createStandaloneExecutionId, createStandaloneSessionId } from './sessionLifecycle';
import { clearPendingDeltas } from './streamDeltas';
import type { ExecutionState, NonCardStreamLineType, StandaloneTurn, StreamLine } from './types';

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

interface ClaudeSendMessageResponse {
  execution_id?: string;
}

interface ClaudeCancelExecutionResponse {
  cancelled: boolean;
  session_id: string;
  execution_id?: string | null;
  reason?: string | null;
}

interface BackendStandaloneExecutionResult {
  response?: string | null;
  usage: Record<string, unknown>;
  iterations: number;
  success: boolean;
  error?: string | null;
}

type SessionSource = 'claude' | 'standalone';

async function appendActiveChatLinesToKernel(lines: StreamLine[]): Promise<void> {
  const kernel = useWorkflowKernelStore.getState();
  const rootSessionId = kernel.activeRootSessionId ?? kernel.sessionId;
  if (!rootSessionId || lines.length === 0) return;
  await kernel.appendModeTranscript(
    rootSessionId,
    'chat',
    lines.map((line) => ({ ...line })),
  );
}

async function storeActiveChatTranscriptInKernel(lines: StreamLine[]): Promise<void> {
  const kernel = useWorkflowKernelStore.getState();
  const rootSessionId = kernel.activeRootSessionId ?? kernel.sessionId;
  if (!rootSessionId) return;
  await kernel.storeModeTranscript(
    rootSessionId,
    'chat',
    lines.map((line) => ({ ...line })),
  );
}

function resolveSessionScopedContext(sessionId: string | null, source: SessionSource): ContextSourceConfig | null {
  const scopedSessionId = sessionId?.trim() ? `${source}:${sessionId.trim()}` : null;
  const store = useContextSourcesStore.getState();
  store.setMemorySessionId(scopedSessionId);
  return store.buildConfig() ?? null;
}

type ExecutionSetState = (
  partial: Partial<ExecutionState> | ((state: ExecutionState) => Partial<ExecutionState>),
) => void;

interface BuildClaudePromptParams {
  query: string;
  lines: StreamLine[];
  projectPath: string;
  sessionId: string | null;
  contextSources: ContextSourceConfig | null;
  addLog: (message: string) => void;
}

interface BuildStandaloneMessageParams {
  query: string;
  turns: StandaloneTurn[];
  contextTurnsLimit: number;
  projectPath: string;
  sessionId: string | null;
  contextSources: ContextSourceConfig | null;
  addLog: (message: string) => void;
}

interface BuildStandaloneMessageResult {
  message: string;
  injectedSourceKinds: string[];
  externalContextInjected: boolean;
}

interface ConversationActionDeps {
  set: ExecutionSetState;
  get: () => ExecutionState;
  buildClaudePromptWithContextEnvelope: (params: BuildClaudePromptParams) => Promise<string>;
  buildStandaloneMessageWithContextEnvelope: (
    params: BuildStandaloneMessageParams,
  ) => Promise<BuildStandaloneMessageResult>;
  preparePromptWithAttachmentContext: (
    prompt: string,
    attachments: FileAttachmentData[],
    addLog: (message: string) => void,
  ) => Promise<string>;
  getStandaloneContextTurnsLimit: () => number;
  trimStandaloneTurns: (turns: StandaloneTurn[], limit: number) => StandaloneTurn[];
  collectAssistantTextSince: (lines: StreamLine[], minExclusiveLineId: number) => string;
  hasAssistantTextLineSince: (lines: StreamLine[], minExclusiveLineId: number) => boolean;
  appendTextWithTypewriter: (
    append: (chunk: string, type: NonCardStreamLineType) => void,
    text: string,
    chunkSize?: number,
    delayMs?: number,
  ) => Promise<void>;
  isBackendStandaloneExecutionResult: (data: unknown) => boolean;
}

interface ConversationActions {
  sendFollowUp: (prompt: string) => Promise<void>;
  cancel: () => Promise<void>;
  regenerateResponse: (userLineId: number) => Promise<void>;
  editAndResend: (userLineId: number, newContent: string) => Promise<void>;
}

export function createConversationActions(deps: ConversationActionDeps): ConversationActions {
  const {
    set,
    get,
    buildClaudePromptWithContextEnvelope,
    buildStandaloneMessageWithContextEnvelope,
    preparePromptWithAttachmentContext,
    getStandaloneContextTurnsLimit,
    trimStandaloneTurns,
    collectAssistantTextSince,
    hasAssistantTextLineSince,
    appendTextWithTypewriter,
    isBackendStandaloneExecutionResult,
  } = deps;

  return {
    sendFollowUp: async (prompt: string) => {
      const sessionId = get().taskId;
      if (!sessionId || !get().isChatSession) {
        return;
      }

      const turnId = getNextTurnId(get().streamingOutput);
      const lineId = get().streamLineCounter + 1;
      get().appendStreamLine(prompt, 'info', undefined, undefined, { turnId, turnBoundary: 'user' });
      void appendActiveChatLinesToKernel([
        {
          id: lineId,
          content: prompt,
          type: 'info',
          timestamp: Date.now(),
          turnId,
          turnBoundary: 'user',
        },
      ]);
      get().toolCallFilter.reset();

      set({
        status: 'running',
        isSubmitting: false,
        isCancelling: false,
        pendingCancelBeforeSessionReady: false,
        activeExecutionId: null,
        apiError: null,
        result: null,
        foregroundDirty: true,
      });

      const followUpContextSources = resolveSessionScopedContext(sessionId, 'claude');
      const assembledPrompt = await buildClaudePromptWithContextEnvelope({
        query: prompt,
        lines: get().streamingOutput,
        projectPath: useSettingsStore.getState().workspacePath || '.',
        sessionId,
        contextSources: followUpContextSources,
        addLog: get().addLog,
      });

      const followUpAttachments = get().attachments;
      const enrichedPrompt = await preparePromptWithAttachmentContext(
        assembledPrompt,
        followUpAttachments,
        get().addLog,
      );
      get().clearAttachments();

      get().addLog(`Follow-up: ${prompt}`);

      try {
        const sendResult = await invoke<CommandResponse<ClaudeSendMessageResponse | boolean>>('send_message', {
          request: { session_id: sessionId, prompt: enrichedPrompt },
        });
        if (!sendResult.success) {
          throw new Error(sendResult.error || 'Failed to send follow-up');
        }
        const sendPayload =
          sendResult.data && typeof sendResult.data === 'object'
            ? (sendResult.data as ClaudeSendMessageResponse)
            : null;
        if (get().taskId === sessionId && !get().isCancelling) {
          set({ activeExecutionId: sendPayload?.execution_id ?? null });
        }
        if (get().pendingCancelBeforeSessionReady) {
          const cancelResult = await invoke<CommandResponse<ClaudeCancelExecutionResponse>>('cancel_execution', {
            session_id: sessionId,
          });
          if (cancelResult.success && cancelResult.data?.cancelled) {
            clearPendingDeltas();
            set({
              status: 'idle',
              isCancelling: false,
              pendingCancelBeforeSessionReady: false,
              activeExecutionId: null,
            });
            get().addLog('Execution cancelled after follow-up dispatch ACK.');
            return;
          }
          set({
            status: 'idle',
            isCancelling: false,
            pendingCancelBeforeSessionReady: false,
            activeExecutionId: null,
            apiError: cancelResult.error || cancelResult.data?.reason || 'Failed to cancel execution',
          });
        }
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({
          status: 'failed',
          isCancelling: false,
          pendingCancelBeforeSessionReady: false,
          activeExecutionId: null,
          apiError: errorMessage,
        });
        get().addLog(`Error: ${errorMessage}`);
      }
    },

    cancel: async () => {
      if (get().isCancelling) return;
      if (get().status !== 'running' && get().status !== 'paused') return;
      set({ isCancelling: true, apiError: null });

      try {
        const { taskId, standaloneSessionId } = get();
        if (standaloneSessionId) {
          const standaloneCancel = await invoke<CommandResponse<boolean>>('cancel_standalone_execution', {
            sessionId: standaloneSessionId,
          });
          if (!standaloneCancel.success) {
            throw new Error(standaloneCancel.error || 'Failed to cancel standalone execution');
          }

          clearPendingDeltas();
          set({
            status: 'idle',
            isCancelling: false,
            pendingCancelBeforeSessionReady: false,
            activeExecutionId: null,
            taskId: null,
            currentStoryId: null,
            result: {
              success: false,
              message: 'Execution cancelled by user',
              completedStories: get().stories.filter((s) => s.status === 'completed').length,
              totalStories: get().stories.length,
              duration: Date.now() - (get().startedAt || Date.now()),
            },
          });
          get().addLog('Execution cancelled');
          get().saveToHistory();
          import('../toolPermission').then(({ useToolPermissionStore }) => {
            useToolPermissionStore.getState().clearAll();
          });
          return;
        }

        if (!taskId) {
          set({ pendingCancelBeforeSessionReady: true });
          get().addLog('Cancel requested before session was ready. Waiting for backend session initialization.');
          return;
        }

        if (!get().activeExecutionId) {
          set({ pendingCancelBeforeSessionReady: true });
          get().addLog(
            'Cancel requested before execution_id was ready. Deferring cancellation until dispatch completes.',
          );
          return;
        }

        const cancelResult = await invoke<CommandResponse<ClaudeCancelExecutionResponse>>('cancel_execution', {
          session_id: taskId,
        });
        if (!cancelResult.success || !cancelResult.data) {
          throw new Error(cancelResult.error || 'Failed to cancel execution');
        }
        if (!cancelResult.data.cancelled) {
          throw new Error(cancelResult.data.reason || 'No active execution to cancel');
        }

        clearPendingDeltas();
        set({
          status: 'idle',
          isCancelling: false,
          pendingCancelBeforeSessionReady: false,
          activeExecutionId: null,
          currentStoryId: null,
          result: {
            success: false,
            message: 'Execution cancelled by user',
            completedStories: get().stories.filter((s) => s.status === 'completed').length,
            totalStories: get().stories.length,
            duration: Date.now() - (get().startedAt || Date.now()),
          },
        });
        get().addLog('Execution cancelled');
        get().saveToHistory();

        import('../toolPermission').then(({ useToolPermissionStore }) => {
          useToolPermissionStore.getState().clearAll();
        });
      } catch (error) {
        const errorMessage = error instanceof Error ? error.message : 'Unknown error';
        set({
          isCancelling: false,
          pendingCancelBeforeSessionReady: false,
          apiError: errorMessage,
        });
        get().addLog(`Failed to cancel: ${errorMessage}`);
      }
    },

    regenerateResponse: async (userLineId: number) => {
      const lines = get().streamingOutput;
      const turns = deriveConversationTurns(lines);
      const targetTurn = turns.find((t) => t.userLineId === userLineId);
      if (!targetTurn) return;

      const userLineIndex = lines.findIndex((l) => l.id === userLineId);
      if (userLineIndex < 0) return;

      const userContent = targetTurn.userContent;
      const truncatedLines = lines.slice(0, userLineIndex + 1);
      const linesForContext = lines.slice(0, userLineIndex);
      const rebuiltTurns = rebuildStandaloneTurns(linesForContext);

      const settingsSnapshot = useSettingsStore.getState();
      const backendValue = String((settingsSnapshot as { backend?: unknown }).backend || '');

      if (isClaudeCodeBackend(backendValue)) {
        const taskId = get().taskId;
        if (taskId) {
          try {
            await invoke('cancel_execution', { session_id: taskId });
          } catch (error) {
            reportNonFatal('execution.regenerateResponse.cancelExecution', error, { taskId });
          }
        }

        set({
          streamingOutput: [
            ...truncatedLines,
            {
              id: (truncatedLines.length > 0 ? truncatedLines[truncatedLines.length - 1].id : 0) + 1,
              content: 'Regenerating response. Previous context will be lost.',
              type: 'warning',
              timestamp: Date.now(),
            },
          ],
          streamLineCounter: (truncatedLines.length > 0 ? truncatedLines[truncatedLines.length - 1].id : 0) + 1,
          standaloneTurns: [],
          status: 'running',
          isCancelling: false,
          pendingCancelBeforeSessionReady: false,
          activeExecutionId: null,
          taskId: null,
          isChatSession: false,
          isSubmitting: false,
          apiError: null,
          result: null,
          foregroundDirty: true,
        });
        void storeActiveChatTranscriptInKernel([
          ...truncatedLines,
          {
            id: (truncatedLines.length > 0 ? truncatedLines[truncatedLines.length - 1].id : 0) + 1,
            content: 'Regenerating response. Previous context will be lost.',
            type: 'warning',
            timestamp: Date.now(),
          },
        ]);

        get().toolCallFilter.reset();

        try {
          const projectPath = settingsSnapshot.workspacePath || '.';
          const startResult = await invoke<CommandResponse<{ session_id: string }>>('start_chat', {
            request: { project_path: projectPath },
          });

          if (!startResult.success || !startResult.data) {
            throw new Error(startResult.error || 'Failed to start Claude Code session');
          }

          const sessionId = startResult.data.session_id;
          set({ taskId: sessionId, isChatSession: true, activeExecutionId: null });

          if (get().pendingCancelBeforeSessionReady) {
            const cancelResult = await invoke<CommandResponse<ClaudeCancelExecutionResponse>>('cancel_execution', {
              session_id: sessionId,
            });
            if (cancelResult.success && cancelResult.data?.cancelled) {
              clearPendingDeltas();
              set({
                status: 'idle',
                isCancelling: false,
                pendingCancelBeforeSessionReady: false,
                activeExecutionId: null,
              });
              get().addLog('Regeneration cancelled before dispatch.');
              return;
            }
            set({
              status: 'idle',
              isCancelling: false,
              pendingCancelBeforeSessionReady: false,
              activeExecutionId: null,
              apiError: cancelResult.error || cancelResult.data?.reason || 'Failed to cancel execution',
            });
            return;
          }

          const regenContextSources = resolveSessionScopedContext(sessionId, 'claude');
          const assembledPrompt = await buildClaudePromptWithContextEnvelope({
            query: userContent,
            lines: get().streamingOutput,
            projectPath,
            sessionId,
            contextSources: regenContextSources,
            addLog: get().addLog,
          });
          const regenAttachments = get().attachments;
          const enrichedPrompt = await preparePromptWithAttachmentContext(
            assembledPrompt,
            regenAttachments,
            get().addLog,
          );
          get().clearAttachments();

          const sendResult = await invoke<CommandResponse<ClaudeSendMessageResponse | boolean>>('send_message', {
            request: { session_id: sessionId, prompt: enrichedPrompt },
          });
          if (!sendResult.success) {
            throw new Error(sendResult.error || 'Failed to send regenerate request');
          }
          const sendPayload =
            sendResult.data && typeof sendResult.data === 'object'
              ? (sendResult.data as ClaudeSendMessageResponse)
              : null;
          set({ activeExecutionId: sendPayload?.execution_id ?? null });
          if (get().pendingCancelBeforeSessionReady) {
            const cancelResult = await invoke<CommandResponse<ClaudeCancelExecutionResponse>>('cancel_execution', {
              session_id: sessionId,
            });
            if (cancelResult.success && cancelResult.data?.cancelled) {
              clearPendingDeltas();
              set({
                status: 'idle',
                isCancelling: false,
                pendingCancelBeforeSessionReady: false,
                activeExecutionId: null,
              });
              get().addLog('Regeneration cancelled after dispatch ACK.');
              return;
            }
            set({
              status: 'idle',
              isCancelling: false,
              pendingCancelBeforeSessionReady: false,
              activeExecutionId: null,
              apiError: cancelResult.error || cancelResult.data?.reason || 'Failed to cancel execution',
            });
            return;
          }
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : 'Unknown error';
          set({
            status: 'failed',
            isCancelling: false,
            pendingCancelBeforeSessionReady: false,
            activeExecutionId: null,
            apiError: errorMessage,
          });
          get().addLog(`Regenerate error: ${errorMessage}`);
        }
      } else {
        set({
          streamingOutput: truncatedLines,
          streamLineCounter: truncatedLines.length > 0 ? truncatedLines[truncatedLines.length - 1].id : 0,
          standaloneTurns: rebuiltTurns,
          status: 'running',
          isCancelling: false,
          pendingCancelBeforeSessionReady: false,
          activeExecutionId: null,
          taskId: null,
          isChatSession: false,
          isSubmitting: false,
          apiError: null,
          result: null,
          foregroundDirty: true,
        });

        get().toolCallFilter.reset();

        const turnStartLineId = get().streamLineCounter;
        set({ currentTurnStartLineId: turnStartLineId });

        const providerValue = String((settingsSnapshot as { provider?: unknown }).provider || '');
        const modelValue = String((settingsSnapshot as { model?: unknown }).model || '');
        const provider = resolveStandaloneProvider(backendValue, providerValue, modelValue);
        const model = settingsSnapshot.model || DEFAULT_MODEL_BY_PROVIDER[provider] || 'claude-sonnet-4-6-20260219';
        const contextTurnsLimit = getStandaloneContextTurnsLimit();
        const existingStandaloneSessionId = get().standaloneSessionId;
        const standaloneSessionId = existingStandaloneSessionId || createStandaloneSessionId();
        if (!existingStandaloneSessionId) {
          set({ standaloneSessionId });
        }
        const regenContextSources = resolveSessionScopedContext(standaloneSessionId, 'standalone');
        const { cleanedPrompt, pluginInvocations } = extractPluginInvocationsFromPrompt(userContent);
        const normalizedPrompt = ensurePromptContent(cleanedPrompt, pluginInvocations.length);
        const assembledContext = await buildStandaloneMessageWithContextEnvelope({
          query: normalizedPrompt,
          turns: rebuiltTurns,
          contextTurnsLimit,
          projectPath: settingsSnapshot.workspacePath || '.',
          sessionId: standaloneSessionId,
          contextSources: regenContextSources,
          addLog: get().addLog,
        });
        const messageToSend = assembledContext.message;
        const baseUrl = resolveProviderBaseUrl(provider, settingsSnapshot);
        const permissionLevel = useToolPermissionStore.getState().sessionLevel;
        const standaloneExecutionId = createStandaloneExecutionId();
        set({ activeExecutionId: standaloneExecutionId });

        try {
          const result = await invoke<CommandResponse<unknown>>('execute_standalone', {
            message: messageToSend,
            provider,
            model,
            projectPath: settingsSnapshot.workspacePath || '.',
            enableTools: true,
            baseUrl,
            analysisSessionId: standaloneSessionId,
            permissionLevel,
            enableCompaction: settingsSnapshot.enableContextCompaction ?? true,
            enableThinking: settingsSnapshot.enableThinking ?? false,
            maxIterations: settingsSnapshot.maxIterations ?? undefined,
            maxConcurrentSubagents: settingsSnapshot.maxConcurrentSubagents || undefined,
            executionId: standaloneExecutionId,
            pluginInvocations: pluginInvocations.length > 0 ? pluginInvocations : null,
            contextSources: regenContextSources,
            externalContextInjected: assembledContext.externalContextInjected,
            injectedSourceKinds: assembledContext.injectedSourceKinds,
          });

          if (!result.success || !result.data) {
            throw new Error(result.error || 'Failed to regenerate');
          }

          if (isBackendStandaloneExecutionResult(result.data)) {
            const execution = result.data as BackendStandaloneExecutionResult;
            const assistantResponse = execution.response?.trim() || '';
            const streamedAssistantText = collectAssistantTextSince(get().streamingOutput, turnStartLineId);
            const assistantTurnText = assistantResponse || streamedAssistantText;

            if (assistantTurnText) {
              const retentionLimit = getStandaloneContextTurnsLimit();
              set((state: ExecutionState) => ({
                standaloneTurns: trimStandaloneTurns(
                  [
                    ...state.standaloneTurns,
                    {
                      user: userContent,
                      assistant: assistantTurnText,
                      createdAt: Date.now(),
                    },
                  ],
                  retentionLimit,
                ),
              }));
            }

            const hasStreamedText = hasAssistantTextLineSince(get().streamingOutput, turnStartLineId);
            if (!hasStreamedText && assistantTurnText) {
              await appendTextWithTypewriter((chunk, type) => get().appendStreamLine(chunk, type), assistantTurnText);
            }

            if (get().status === 'running') {
              set({
                status: execution.success ? 'completed' : 'failed',
                activeExecutionId: null,
                apiError: execution.success ? null : execution.error || 'Regeneration failed',
              });
            }
          }
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : 'Unknown error';
          set({
            status: 'failed',
            isCancelling: false,
            pendingCancelBeforeSessionReady: false,
            activeExecutionId: null,
            apiError: errorMessage,
          });
          get().addLog(`Regenerate error: ${errorMessage}`);
        }
      }

      get().addLog(`Regenerated response for userLineId=${userLineId}`);
    },

    editAndResend: async (userLineId: number, newContent: string) => {
      const lines = get().streamingOutput;
      const turns = deriveConversationTurns(lines);
      const targetTurn = turns.find((t) => t.userLineId === userLineId);
      if (!targetTurn) return;

      const userLineIndex = lines.findIndex((l) => l.id === userLineId);
      if (userLineIndex < 0) return;

      const truncatedLines = lines.slice(0, userLineIndex);
      const newInfoId = (truncatedLines.length > 0 ? truncatedLines[truncatedLines.length - 1].id : 0) + 1;
      const nextTurnId = getNextTurnId(truncatedLines);
      const linesWithEditedMessage: StreamLine[] = [
        ...truncatedLines,
        {
          id: newInfoId,
          content: newContent,
          type: 'info',
          timestamp: Date.now(),
          turnId: nextTurnId,
          turnBoundary: 'user',
        },
      ];

      const rebuiltTurns = rebuildStandaloneTurns(truncatedLines);

      const settingsSnapshot = useSettingsStore.getState();
      const backendValue = String((settingsSnapshot as { backend?: unknown }).backend || '');

      if (isClaudeCodeBackend(backendValue)) {
        const taskId = get().taskId;
        if (taskId) {
          try {
            await invoke('cancel_execution', { session_id: taskId });
          } catch (error) {
            reportNonFatal('execution.editAndResend.cancelExecution', error, { taskId });
          }
        }

        set({
          streamingOutput: [
            ...linesWithEditedMessage,
            {
              id: newInfoId + 1,
              content: 'Regenerating response. Previous context will be lost.',
              type: 'warning',
              timestamp: Date.now(),
            },
          ],
          streamLineCounter: newInfoId + 1,
          standaloneTurns: [],
          status: 'running',
          isCancelling: false,
          pendingCancelBeforeSessionReady: false,
          activeExecutionId: null,
          taskId: null,
          isChatSession: false,
          isSubmitting: false,
          apiError: null,
          result: null,
          foregroundDirty: true,
        });
        void storeActiveChatTranscriptInKernel([
          ...linesWithEditedMessage,
          {
            id: newInfoId + 1,
            content: 'Regenerating response. Previous context will be lost.',
            type: 'warning',
            timestamp: Date.now(),
          },
        ]);

        get().toolCallFilter.reset();

        try {
          const projectPath = settingsSnapshot.workspacePath || '.';
          const startResult = await invoke<CommandResponse<{ session_id: string }>>('start_chat', {
            request: { project_path: projectPath },
          });

          if (!startResult.success || !startResult.data) {
            throw new Error(startResult.error || 'Failed to start Claude Code session');
          }

          const sessionId = startResult.data.session_id;
          set({ taskId: sessionId, isChatSession: true, activeExecutionId: null });

          if (get().pendingCancelBeforeSessionReady) {
            const cancelResult = await invoke<CommandResponse<ClaudeCancelExecutionResponse>>('cancel_execution', {
              session_id: sessionId,
            });
            if (cancelResult.success && cancelResult.data?.cancelled) {
              clearPendingDeltas();
              set({
                status: 'idle',
                isCancelling: false,
                pendingCancelBeforeSessionReady: false,
                activeExecutionId: null,
              });
              get().addLog('Edit-and-resend cancelled before dispatch.');
              return;
            }
            set({
              status: 'idle',
              isCancelling: false,
              pendingCancelBeforeSessionReady: false,
              activeExecutionId: null,
              apiError: cancelResult.error || cancelResult.data?.reason || 'Failed to cancel execution',
            });
            return;
          }

          const editContextSources = resolveSessionScopedContext(sessionId, 'claude');
          const assembledPrompt = await buildClaudePromptWithContextEnvelope({
            query: newContent,
            lines: get().streamingOutput,
            projectPath,
            sessionId,
            contextSources: editContextSources,
            addLog: get().addLog,
          });
          const editAttachments = get().attachments;
          const enrichedPrompt = await preparePromptWithAttachmentContext(
            assembledPrompt,
            editAttachments,
            get().addLog,
          );
          get().clearAttachments();

          const sendResult = await invoke<CommandResponse<ClaudeSendMessageResponse | boolean>>('send_message', {
            request: { session_id: sessionId, prompt: enrichedPrompt },
          });
          if (!sendResult.success) {
            throw new Error(sendResult.error || 'Failed to send edited prompt');
          }
          const sendPayload =
            sendResult.data && typeof sendResult.data === 'object'
              ? (sendResult.data as ClaudeSendMessageResponse)
              : null;
          set({ activeExecutionId: sendPayload?.execution_id ?? null });
          if (get().pendingCancelBeforeSessionReady) {
            const cancelResult = await invoke<CommandResponse<ClaudeCancelExecutionResponse>>('cancel_execution', {
              session_id: sessionId,
            });
            if (cancelResult.success && cancelResult.data?.cancelled) {
              clearPendingDeltas();
              set({
                status: 'idle',
                isCancelling: false,
                pendingCancelBeforeSessionReady: false,
                activeExecutionId: null,
              });
              get().addLog('Edit-and-resend cancelled after dispatch ACK.');
              return;
            }
            set({
              status: 'idle',
              isCancelling: false,
              pendingCancelBeforeSessionReady: false,
              activeExecutionId: null,
              apiError: cancelResult.error || cancelResult.data?.reason || 'Failed to cancel execution',
            });
            return;
          }
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : 'Unknown error';
          set({
            status: 'failed',
            isCancelling: false,
            pendingCancelBeforeSessionReady: false,
            activeExecutionId: null,
            apiError: errorMessage,
          });
          get().addLog(`Edit and resend error: ${errorMessage}`);
        }
      } else {
        set({
          streamingOutput: linesWithEditedMessage,
          streamLineCounter: newInfoId,
          standaloneTurns: rebuiltTurns,
          status: 'running',
          isCancelling: false,
          pendingCancelBeforeSessionReady: false,
          activeExecutionId: null,
          taskId: null,
          isChatSession: false,
          isSubmitting: false,
          apiError: null,
          result: null,
          foregroundDirty: true,
        });

        get().toolCallFilter.reset();

        const turnStartLineId = get().streamLineCounter;
        set({ currentTurnStartLineId: turnStartLineId });

        const providerValue = String((settingsSnapshot as { provider?: unknown }).provider || '');
        const modelValue = String((settingsSnapshot as { model?: unknown }).model || '');
        const provider = resolveStandaloneProvider(backendValue, providerValue, modelValue);
        const model = settingsSnapshot.model || DEFAULT_MODEL_BY_PROVIDER[provider] || 'claude-sonnet-4-6-20260219';
        const contextTurnsLimit = getStandaloneContextTurnsLimit();
        const existingStandaloneSessionId = get().standaloneSessionId;
        const standaloneSessionId = existingStandaloneSessionId || createStandaloneSessionId();
        if (!existingStandaloneSessionId) {
          set({ standaloneSessionId });
        }
        const editContextSources = resolveSessionScopedContext(standaloneSessionId, 'standalone');
        const { cleanedPrompt, pluginInvocations } = extractPluginInvocationsFromPrompt(newContent);
        const normalizedPrompt = ensurePromptContent(cleanedPrompt, pluginInvocations.length);
        const assembledContext = await buildStandaloneMessageWithContextEnvelope({
          query: normalizedPrompt,
          turns: rebuiltTurns,
          contextTurnsLimit,
          projectPath: settingsSnapshot.workspacePath || '.',
          sessionId: standaloneSessionId,
          contextSources: editContextSources,
          addLog: get().addLog,
        });
        const messageToSend = assembledContext.message;
        const baseUrl = resolveProviderBaseUrl(provider, settingsSnapshot);
        const permissionLevel = useToolPermissionStore.getState().sessionLevel;
        const standaloneExecutionId = createStandaloneExecutionId();
        set({ activeExecutionId: standaloneExecutionId });

        try {
          const result = await invoke<CommandResponse<unknown>>('execute_standalone', {
            message: messageToSend,
            provider,
            model,
            projectPath: settingsSnapshot.workspacePath || '.',
            enableTools: true,
            baseUrl,
            analysisSessionId: standaloneSessionId,
            permissionLevel,
            enableCompaction: settingsSnapshot.enableContextCompaction ?? true,
            enableThinking: settingsSnapshot.enableThinking ?? false,
            maxIterations: settingsSnapshot.maxIterations ?? undefined,
            maxConcurrentSubagents: settingsSnapshot.maxConcurrentSubagents || undefined,
            executionId: standaloneExecutionId,
            pluginInvocations: pluginInvocations.length > 0 ? pluginInvocations : null,
            contextSources: editContextSources,
            externalContextInjected: assembledContext.externalContextInjected,
            injectedSourceKinds: assembledContext.injectedSourceKinds,
          });

          if (!result.success || !result.data) {
            throw new Error(result.error || 'Failed to execute edit');
          }

          if (isBackendStandaloneExecutionResult(result.data)) {
            const execution = result.data as BackendStandaloneExecutionResult;
            const assistantResponse = execution.response?.trim() || '';
            const streamedAssistantText = collectAssistantTextSince(get().streamingOutput, turnStartLineId);
            const assistantTurnText = assistantResponse || streamedAssistantText;

            if (assistantTurnText) {
              const retentionLimit = getStandaloneContextTurnsLimit();
              set((state: ExecutionState) => ({
                standaloneTurns: trimStandaloneTurns(
                  [
                    ...state.standaloneTurns,
                    {
                      user: newContent,
                      assistant: assistantTurnText,
                      createdAt: Date.now(),
                    },
                  ],
                  retentionLimit,
                ),
              }));
            }

            const hasStreamedText = hasAssistantTextLineSince(get().streamingOutput, turnStartLineId);
            if (!hasStreamedText && assistantTurnText) {
              await appendTextWithTypewriter((chunk, type) => get().appendStreamLine(chunk, type), assistantTurnText);
            }

            if (get().status === 'running') {
              set({
                status: execution.success ? 'completed' : 'failed',
                activeExecutionId: null,
                apiError: execution.success ? null : execution.error || 'Edit and resend failed',
              });
            }
          }
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : 'Unknown error';
          set({
            status: 'failed',
            isCancelling: false,
            pendingCancelBeforeSessionReady: false,
            activeExecutionId: null,
            apiError: errorMessage,
          });
          get().addLog(`Edit and resend error: ${errorMessage}`);
        }
      }

      get().addLog(`Edited and resent userLineId=${userLineId}`);
    },
  };
}
