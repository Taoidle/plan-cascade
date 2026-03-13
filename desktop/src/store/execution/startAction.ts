import { invoke } from '@tauri-apps/api/core';
import type { FileAttachmentData, WorkspaceFileReferenceData } from '../../types/attachment';
import type { ContextSourceConfig } from '../../types/contextSources';
import { useAgentsStore } from '../agents';
import { useContextSourcesStore } from '../contextSources';
import { useSettingsStore } from '../settings';
import { useToolPermissionStore } from '../toolPermission';
import {
  DEFAULT_MODEL_BY_PROVIDER,
  isClaudeCodeBackend,
  parseMemoryReviewAgentProvider,
  resolveProviderBaseUrl,
  resolveStandaloneProvider,
} from './providerUtils';
import { ensurePromptContent, extractPluginInvocationsFromPrompt } from './messageDispatch';
import { createStandaloneExecutionId, createStandaloneSessionId } from './sessionLifecycle';
import { clearPendingDeltas } from './streamDeltas';
import {
  appendToBackgroundSession,
  findBackgroundSessionByTaskId,
  updateBackgroundSessionByTaskId,
} from './sessionRouting';
import { inferInjectedSourceKinds } from './contextAssembly';
import { getActiveKernelChatTranscriptForPrompt } from './kernelTranscript';
import { buildActiveChatRuntimeRegistryPatch } from './runtimeRegistryActions';
import type { ExecutionState, ExecutionStatus, StandaloneTurn, StreamLine } from './types';

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

interface LegacyTaskStartData {
  task_id: string;
}

interface BackendStandaloneExecutionResult {
  response?: string | null;
  usage: Record<string, unknown>;
  iterations: number;
  success: boolean;
  error?: string | null;
}

type SessionSource = 'claude' | 'standalone';

async function markKernelChatTurnFailed(sessionId: string | null, error: string): Promise<void> {
  const normalizedSessionId = sessionId?.trim() ?? '';
  const normalizedError = error.trim();
  if (!normalizedSessionId || !normalizedError) return;
  try {
    await invoke<CommandResponse<unknown>>('workflow_mark_chat_turn_failed', {
      sessionId: normalizedSessionId,
      error: normalizedError,
    });
  } catch {
    // best effort kernel sync
  }
}

async function cancelKernelChatTurn(sessionId: string | null, reason = 'cancelled_by_user'): Promise<void> {
  const normalizedSessionId = sessionId?.trim() ?? '';
  if (!normalizedSessionId) return;
  try {
    await invoke<CommandResponse<unknown>>('workflow_cancel_operation', {
      sessionId: normalizedSessionId,
      reason,
    });
  } catch {
    // best effort kernel sync
  }
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

interface StartActionDeps {
  set: ExecutionSetState;
  get: () => ExecutionState;
  buildClaudePromptWithContextEnvelope: (params: BuildClaudePromptParams) => Promise<string>;
  buildStandaloneMessageWithContextEnvelope: (
    params: BuildStandaloneMessageParams,
  ) => Promise<BuildStandaloneMessageResult>;
  preparePromptWithAttachmentContext: (
    prompt: string,
    attachments: FileAttachmentData[],
    workspaceReferences: WorkspaceFileReferenceData[],
    addLog: (message: string) => void,
  ) => Promise<string>;
  getStandaloneContextTurnsLimit: () => number;
  trimStandaloneTurns: (turns: StandaloneTurn[], limit: number) => StandaloneTurn[];
  collectAssistantTextSince: (lines: StreamLine[], minExclusiveLineId: number) => string;
  isBackendStandaloneExecutionResult: (data: unknown) => boolean;
  standaloneContextUnlimited: number;
}

function isLegacyTaskStartData(data: unknown): data is LegacyTaskStartData {
  return (
    !!data && typeof data === 'object' && 'task_id' in data && typeof (data as LegacyTaskStartData).task_id === 'string'
  );
}

export function createStartAction(
  deps: StartActionDeps,
): (description: string, mode: 'simple' | 'expert') => Promise<void> {
  const {
    set,
    get,
    buildClaudePromptWithContextEnvelope,
    buildStandaloneMessageWithContextEnvelope,
    preparePromptWithAttachmentContext,
    getStandaloneContextTurnsLimit,
    trimStandaloneTurns,
    collectAssistantTextSince,
    isBackendStandaloneExecutionResult,
    standaloneContextUnlimited,
  } = deps;

  return async (description: string, mode: 'simple' | 'expert') => {
    if (get().status === 'running') {
      if (get().isChatSession) {
        get().parkForegroundRuntime();
      } else {
        get().backgroundCurrentSession();
      }
    }

    const agentStore = useAgentsStore.getState();
    const activeAgent = agentStore.activeAgentForSession;

    const settingsSnapshot = useSettingsStore.getState();
    const backendSnapshot = String((settingsSnapshot as { backend?: unknown }).backend || '');
    const isClaudeBackend = isClaudeCodeBackend(backendSnapshot);
    const kernelTranscriptSnapshot = getActiveKernelChatTranscriptForPrompt(description);
    const activeKernelRootSessionId = kernelTranscriptSnapshot.rootSessionId;
    const kernelChatLines = kernelTranscriptSnapshot.lines;
    const existingStandaloneTurns = get().standaloneTurns;
    const existingStandaloneSessionId = get().standaloneSessionId;
    const preserveSimpleConversation = mode === 'simple' && kernelChatLines.length > 0;
    const nextStandaloneSessionId =
      mode === 'simple' && !isClaudeBackend
        ? preserveSimpleConversation && existingStandaloneSessionId
          ? existingStandaloneSessionId
          : createStandaloneSessionId()
        : null;

    set({
      isSubmitting: true,
      apiError: null,
      status: 'running',
      isCancelling: false,
      pendingCancelBeforeSessionReady: false,
      activeExecutionId: null,
      taskDescription: description,
      startedAt: Date.now(),
      result: null,
      taskId: isClaudeBackend ? get().taskId : null,
      isChatSession: isClaudeBackend ? get().isChatSession : false,
      logs: [],
      stories: [],
      batches: [],
      currentBatch: 0,
      currentStoryId: null,
      progress: 0,
      streamingOutput: preserveSimpleConversation ? kernelChatLines : [],
      streamLineCounter: preserveSimpleConversation
        ? kernelChatLines.reduce((max, line) => Math.max(max, line.id), 0)
        : 0,
      analysisCoverage: null,
      qualityGateResults: [],
      executionErrors: [],
      estimatedTimeRemaining: null,
      standaloneSessionId: nextStandaloneSessionId,
      latestUsage: preserveSimpleConversation ? get().latestUsage : null,
      sessionUsageTotals: preserveSimpleConversation ? get().sessionUsageTotals : null,
      turnUsageTotals: null,
      foregroundParentSessionId: preserveSimpleConversation ? get().foregroundParentSessionId : null,
      foregroundBgId: null,
      foregroundOriginHistoryId: preserveSimpleConversation ? get().foregroundOriginHistoryId : null,
      foregroundOriginSessionId: preserveSimpleConversation ? get().foregroundOriginSessionId : null,
      foregroundDirty: true,
      activeAgentId: activeAgent?.id ?? null,
      activeAgentName: activeAgent?.name ?? null,
    });

    const capturedStartedAt = get().startedAt;

    get().toolCallFilter.reset();

    get().addLog(`Starting execution in ${mode} mode...`);
    get().addLog(`Task: ${description}`);

    try {
      const settings = settingsSnapshot;
      const backendValue = String((settings as { backend?: unknown }).backend || '');
      const providerValue = String((settings as { provider?: unknown }).provider || '');
      const modelValue = String((settings as { model?: unknown }).model || '');

      if (isClaudeCodeBackend(backendValue)) {
        const projectPath = settings.workspacePath || '.';
        const startResult = await invoke<CommandResponse<{ session_id: string }>>('start_chat', {
          request: { project_path: projectPath },
        });

        if (!startResult.success || !startResult.data) {
          throw new Error(startResult.error || 'Failed to start Claude Code session');
        }

        const sessionId = startResult.data.session_id;
        set({ taskId: sessionId, isSubmitting: false, isChatSession: true });
        set((state: ExecutionState) =>
          buildActiveChatRuntimeRegistryPatch(state, {
            source: 'claude',
            rawSessionId: sessionId,
            rootSessionId: activeKernelRootSessionId,
            workspacePath: settings.workspacePath || null,
            llmBackend: settings.backend,
            llmProvider: settings.provider,
            llmModel: settings.model,
          }),
        );
        get().addLog(`Claude Code session started: ${sessionId}`);

        if (get().pendingCancelBeforeSessionReady) {
          get().addLog('Cancellation was requested before session became ready; aborting send_message.');
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
            await cancelKernelChatTurn(activeKernelRootSessionId);
            get().addLog('Execution cancelled before first message dispatch.');
          } else {
            set({
              status: 'idle',
              isCancelling: false,
              pendingCancelBeforeSessionReady: false,
              activeExecutionId: null,
              apiError: null,
            });
            await cancelKernelChatTurn(activeKernelRootSessionId);
            get().addLog(
              `Cancelled before dispatch${cancelResult.data?.reason ? `: ${cancelResult.data.reason}` : ''}.`,
            );
          }
          return;
        }

        const claudeContextSources = resolveSessionScopedContext(sessionId, 'claude');
        const assembledPrompt = await buildClaudePromptWithContextEnvelope({
          query: description,
          lines: kernelChatLines,
          projectPath,
          sessionId,
          contextSources: claudeContextSources,
          addLog: get().addLog,
        });

        const claudeAttachments = get().attachments;
        const claudePrompt = await preparePromptWithAttachmentContext(
          assembledPrompt,
          claudeAttachments,
          get().workspaceReferences,
          get().addLog,
        );
        get().clearAttachments();
        get().clearWorkspaceReferences();

        const sendResult = await invoke<CommandResponse<ClaudeSendMessageResponse | boolean>>('send_message', {
          request: {
            session_id: sessionId,
            prompt: claudePrompt,
            kernel_session_id: activeKernelRootSessionId,
          },
        });
        if (!sendResult.success) {
          throw new Error(sendResult.error || 'Failed to send message');
        }
        const sendPayload =
          sendResult.data && typeof sendResult.data === 'object'
            ? (sendResult.data as ClaudeSendMessageResponse)
            : null;
        if (get().taskId === sessionId && !get().isCancelling) {
          set({
            activeExecutionId: sendPayload?.execution_id ?? null,
          });
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
            await cancelKernelChatTurn(activeKernelRootSessionId);
            get().addLog('Execution cancelled after dispatch ACK.');
            return;
          }
          set({
            status: 'idle',
            isCancelling: false,
            pendingCancelBeforeSessionReady: false,
            activeExecutionId: null,
            apiError: cancelResult.error || cancelResult.data?.reason || 'Failed to cancel execution',
          });
          await cancelKernelChatTurn(activeKernelRootSessionId);
          return;
        }
      } else {
        const provider = resolveStandaloneProvider(backendValue, providerValue, modelValue);
        const effectiveModel =
          activeAgent?.model || settings.model || DEFAULT_MODEL_BY_PROVIDER[provider] || 'claude-sonnet-4-6-20260219';
        const model = effectiveModel;
        const isSimpleStandalone = mode === 'simple';
        const turnStartLineId = get().streamLineCounter;
        set({ currentTurnStartLineId: turnStartLineId });
        const standaloneSessionId = get().standaloneSessionId;
        if (standaloneSessionId) {
          set((state: ExecutionState) =>
            buildActiveChatRuntimeRegistryPatch(state, {
              source: 'standalone',
              rawSessionId: standaloneSessionId,
              rootSessionId: activeKernelRootSessionId,
              workspacePath: settings.workspacePath || null,
              llmBackend: settings.backend,
              llmProvider: settings.provider,
              llmModel: settings.model,
            }),
          );
        }
        const permissionLevel = useToolPermissionStore.getState().sessionLevel;
        const contextTurnsLimit = getStandaloneContextTurnsLimit();
        const recentStandaloneTurns = trimStandaloneTurns(existingStandaloneTurns, contextTurnsLimit);
        const contextSources = resolveSessionScopedContext(standaloneSessionId, 'standalone');
        const { cleanedPrompt, pluginInvocations } = extractPluginInvocationsFromPrompt(description);
        const normalizedPrompt = ensurePromptContent(cleanedPrompt, pluginInvocations.length);
        const assembledContext = isSimpleStandalone
          ? await buildStandaloneMessageWithContextEnvelope({
              query: normalizedPrompt,
              turns: existingStandaloneTurns,
              contextTurnsLimit,
              projectPath: settings.workspacePath || '.',
              sessionId: standaloneSessionId,
              contextSources,
              addLog: get().addLog,
            })
          : {
              message: normalizedPrompt,
              injectedSourceKinds: inferInjectedSourceKinds({
                hasHistory: recentStandaloneTurns.length > 0,
                contextSources,
              }),
              externalContextInjected: false,
            };
        const messageToSend = assembledContext.message;
        get().addLog(
          `Resolved provider: ${provider} (backend=${backendValue || 'empty'}, setting=${providerValue || 'empty'}, model=${modelValue || 'empty'})`,
        );
        if (pluginInvocations.length > 0) {
          get().addLog(`Applying ${pluginInvocations.length} plugin invocation(s)`);
        }
        if (isSimpleStandalone && recentStandaloneTurns.length > 0) {
          const contextLabel =
            contextTurnsLimit === standaloneContextUnlimited ? 'unlimited' : String(contextTurnsLimit);
          get().addLog(
            `Using standalone conversation context (${recentStandaloneTurns.length}/${contextLabel} turns, v2)`,
          );
        }

        const standaloneAttachments = get().attachments;
        const enrichedMessage = await preparePromptWithAttachmentContext(
          messageToSend,
          standaloneAttachments,
          get().workspaceReferences,
          get().addLog,
        );
        get().clearAttachments();
        get().clearWorkspaceReferences();

        const baseUrl = resolveProviderBaseUrl(provider, settings);
        const memoryReviewProvider = parseMemoryReviewAgentProvider(settings.memorySettings.reviewAgentRef);
        const memoryReviewBaseUrl = memoryReviewProvider
          ? resolveProviderBaseUrl(memoryReviewProvider, settings)
          : undefined;
        const standaloneExecutionId = createStandaloneExecutionId();
        set({ activeExecutionId: standaloneExecutionId });

        const result = await invoke<CommandResponse<unknown>>('execute_standalone', {
          message: enrichedMessage,
          provider,
          model,
          projectPath: settings.workspacePath || '.',
          enableTools: true,
          baseUrl,
          analysisSessionId: standaloneSessionId,
          permissionLevel,
          enableCompaction: settings.enableContextCompaction ?? true,
          enableThinking: settings.enableThinking ?? false,
          maxConcurrentSubagents: settings.maxConcurrentSubagents || undefined,
          executionId: standaloneExecutionId,
          kernelSessionId: activeKernelRootSessionId,
          memoryAutoExtractEnabled: settings.memorySettings.autoExtractEnabled,
          memoryReviewMode: settings.memorySettings.reviewMode,
          memoryReviewAgentRef: settings.memorySettings.reviewAgentRef || null,
          memoryReviewBaseUrl: memoryReviewBaseUrl || null,
          pluginInvocations: pluginInvocations.length > 0 ? pluginInvocations : null,
          systemPrompt: activeAgent?.system_prompt ?? null,
          contextSources,
          externalContextInjected: assembledContext.externalContextInjected,
          injectedSourceKinds: assembledContext.injectedSourceKinds,
        });

        if (!result.success || !result.data) {
          throw new Error(result.error || 'Failed to start execution');
        }

        if (isLegacyTaskStartData(result.data)) {
          set({
            taskId: result.data.task_id,
            isSubmitting: false,
          });
          get().addLog(`Task started with ID: ${result.data.task_id}`);
          return;
        }

        if (isBackendStandaloneExecutionResult(result.data)) {
          const execution = result.data as BackendStandaloneExecutionResult;

          const sessionWasBackgrounded =
            (standaloneSessionId && get().standaloneSessionId !== standaloneSessionId) ||
            get().startedAt !== capturedStartedAt;

          if (sessionWasBackgrounded && standaloneSessionId) {
            useToolPermissionStore.getState().clearSessionRequests(standaloneSessionId);
            const bgMatch = findBackgroundSessionByTaskId(get(), standaloneSessionId);
            if (bgMatch) {
              if (mode === 'simple') {
                const bgAssistantResponse = execution.response?.trim() || '';
                const bgStreamedText = collectAssistantTextSince(bgMatch.snapshot.streamingOutput, turnStartLineId);
                const bgAssistantText = bgAssistantResponse || bgStreamedText;
                if (bgAssistantText) {
                  const retentionLimit = getStandaloneContextTurnsLimit();
                  set(
                    updateBackgroundSessionByTaskId(get(), standaloneSessionId, (snap) => ({
                      standaloneTurns: trimStandaloneTurns(
                        [
                          ...snap.standaloneTurns,
                          { user: description, assistant: bgAssistantText, createdAt: Date.now() },
                        ],
                        retentionLimit,
                      ),
                    })),
                  );
                }
              }

              const bgAfter = findBackgroundSessionByTaskId(get(), standaloneSessionId);
              if (bgAfter && bgAfter.snapshot.status === 'running') {
                const succeeded = execution.success;
                const duration = Date.now() - (bgAfter.snapshot.startedAt || Date.now());
                const durationStr =
                  duration >= 60000
                    ? `${Math.floor(duration / 60000)}m ${Math.round((duration % 60000) / 1000)}s`
                    : `${Math.round(duration / 1000)}s`;
                set(
                  appendToBackgroundSession(
                    get(),
                    standaloneSessionId,
                    succeeded
                      ? `Completed (${durationStr})`
                      : `Execution finished with failures.${execution.error ? ` ${execution.error}` : ''}`,
                    succeeded ? 'success' : 'error',
                  ),
                );
                set(
                  updateBackgroundSessionByTaskId(get(), standaloneSessionId, () => ({
                    status: (succeeded ? 'completed' : 'failed') as ExecutionStatus,
                  })),
                );
              }
            }
            return;
          }

          const assistantResponse = execution.response?.trim() || '';
          const streamedAssistantText = collectAssistantTextSince(get().streamingOutput, turnStartLineId);
          const assistantTurnText = assistantResponse || streamedAssistantText;

          if (mode === 'simple' && assistantTurnText) {
            const retentionLimit = getStandaloneContextTurnsLimit();
            set((state: ExecutionState) => ({
              standaloneTurns: trimStandaloneTurns(
                [...state.standaloneTurns, { user: description, assistant: assistantTurnText, createdAt: Date.now() }],
                retentionLimit,
              ),
            }));
          }

          set({ isSubmitting: false });

          const finalizeFromInvoke = async () => {
            const bgCheck =
              (standaloneSessionId && get().standaloneSessionId !== standaloneSessionId) ||
              get().startedAt !== capturedStartedAt;
            if (bgCheck) return;

            if (get().status !== 'running') {
              if (get().status === 'completed' || get().status === 'failed') {
                get().saveToHistory();
              }
              return;
            }
            const succeeded = execution.success;
            if (standaloneSessionId) {
              useToolPermissionStore.getState().clearSessionRequests(standaloneSessionId);
            }

            const duration = Date.now() - (get().startedAt || Date.now());
            const durationStr =
              duration >= 60000
                ? `${Math.floor(duration / 60000)}m ${Math.round((duration % 60000) / 1000)}s`
                : `${Math.round(duration / 1000)}s`;

            set({
              taskId: null,
              status: succeeded ? 'completed' : 'failed',
              activeExecutionId: null,
              progress: succeeded ? 100 : get().progress,
              estimatedTimeRemaining: 0,
              apiError: succeeded ? null : execution.error || 'Execution failed',
              result: {
                success: succeeded,
                message: succeeded ? 'Execution completed' : 'Execution failed',
                completedStories: succeeded ? 1 : 0,
                totalStories: 1,
                duration,
                error: execution.error || undefined,
              },
            });

            if (succeeded) {
              get().appendStreamLine(`Completed (${durationStr})`, 'success');
            } else {
              get().appendStreamLine(
                `Execution finished with failures.${execution.error ? ` ${execution.error}` : ''}`,
                'error',
              );
            }
            if (!succeeded && execution.error) {
              get().addExecutionError({
                severity: 'error',
                title: 'Execution Failed',
                description: execution.error,
                suggestedFix: 'Check API key/model settings and retry.',
              });
            }

            get().addLog(
              succeeded
                ? `Execution completed (iterations: ${execution.iterations})`
                : `Execution failed: ${execution.error || 'Unknown error'}`,
            );
            get().saveToHistory();
          };

          if (get().status === 'running') {
            if (get().streamLineCounter > turnStartLineId) {
              globalThis.setTimeout(() => {
                void finalizeFromInvoke();
              }, 3000);
            } else {
              void finalizeFromInvoke();
            }
          } else if (get().status === 'completed') {
            void finalizeFromInvoke();
          }
          return;
        }

        throw new Error('Unexpected execute_standalone response shape');
      }
    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Unknown error';

      const errSessionGone =
        (nextStandaloneSessionId && get().standaloneSessionId !== nextStandaloneSessionId) ||
        get().startedAt !== capturedStartedAt;
      if (errSessionGone && nextStandaloneSessionId) {
        useToolPermissionStore.getState().clearSessionRequests(nextStandaloneSessionId);
        set(appendToBackgroundSession(get(), nextStandaloneSessionId, `Error: ${errorMessage}`, 'error'));
        set(
          updateBackgroundSessionByTaskId(get(), nextStandaloneSessionId, () => ({
            status: 'failed' as ExecutionStatus,
          })),
        );
        return;
      }

      set({
        status: 'failed',
        isSubmitting: false,
        isCancelling: false,
        pendingCancelBeforeSessionReady: false,
        activeExecutionId: null,
        apiError: errorMessage,
        result: {
          success: false,
          message: 'Failed to start execution',
          completedStories: 0,
          totalStories: 0,
          duration: Date.now() - (get().startedAt || Date.now()),
          error: errorMessage,
        },
      });
      if (nextStandaloneSessionId) {
        useToolPermissionStore.getState().clearSessionRequests(nextStandaloneSessionId);
      }

      await markKernelChatTurnFailed(activeKernelRootSessionId, errorMessage);
      get().addLog(`Error: ${errorMessage}`);
      get().saveToHistory();
    }
  };
}
