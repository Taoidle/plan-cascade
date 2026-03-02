/**
 * SimpleMode Component
 *
 * IM-style layout with:
 * - Left (optional): WorkspaceTreeSidebar
 * - Middle: ChatTranscript + ChatToolbar + InputBox
 * - Right (optional): TabbedRightPanel (Output + Git tabs)
 * - Bottom: Status bar (connection, project, model, permission, index, tokens)
 * - Edge collapse buttons for left/right panels
 */

import { useEffect, useMemo, useRef, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { InputBox, type InputBoxHandle } from './InputBox';
import { WorkspaceTreeSidebar } from './WorkspaceTreeSidebar';
import { EdgeCollapseButton } from './EdgeCollapseButton';
import { BottomStatusBar } from './BottomStatusBar';
import { ChatToolbar } from './ChatToolbar';
import { TabbedRightPanel, type RightPanelTab } from './TabbedRightPanel';
import { useExecutionStore } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';
import { useWorkflowOrchestratorStore } from '../../store/workflowOrchestrator';
import { usePlanOrchestratorStore } from '../../store/planOrchestrator';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import { useGitStore } from '../../store/git';
import { useFileChangesStore } from '../../store/fileChanges';
import { InterviewInputPanel } from './InterviewInputPanel';
import { ToolPermissionOverlay } from './ToolPermissionOverlay';
import { useToolPermissionStore } from '../../store/toolPermission';
import { useAgentsStore } from '../../store/agents';
import { createFileChangeCardBridge } from '../../lib/fileChangeCardBridge';
import { listenOpenAIChanges } from '../../lib/simpleModeNavigation';
import {
  captureElementToBlob,
  blobToBase64,
  saveBinaryWithDialog,
  localTimestampForFilename,
} from '../../lib/exportUtils';
import { useToast } from '../shared/Toast';
import { useContextSourcesStore } from '../../store/contextSources';
import { buildConversationHistory } from '../../lib/contextBridge';
import { ChatTranscript } from './ChatTranscript';
import {
  clearPersistedSimpleChatQueue,
  loadPersistedSimpleChatQueue,
  persistSimpleChatQueue,
  type QueuedChatMessage,
} from './queuePersistence';
import {
  DEFAULT_PROMPT_TOKEN_BUDGET,
  estimatePromptTokensFallback,
  toAttachmentTokenEstimateInput,
  type PromptTokenEstimateResult,
} from './tokenBudget';
import { resolvePromptTokenBudget } from '../../lib/promptTokenBudget';

type WorkflowMode = 'chat' | 'plan' | 'task';
interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

const MAX_QUEUED_CHAT_MESSAGES = 3;
const TOKEN_ESTIMATE_DEBOUNCE_MS = 180;
const WORKFLOW_KERNEL_SESSION_STORAGE_PREFIX = 'simple_mode_workflow_kernel_session_v2:';

function workflowKernelSessionStorageKey(workspacePath: string | null): string {
  return `${WORKFLOW_KERNEL_SESSION_STORAGE_PREFIX}${workspacePath || '__default_workspace__'}`;
}

export function SimpleMode() {
  const { t } = useTranslation('simpleMode');
  const { showToast } = useToast();
  const {
    status,
    isCancelling: executionIsCancelling,
    connectionStatus,
    isSubmitting,
    apiError,
    start,
    sendFollowUp,
    pause,
    resume,
    cancel,
    reset,
    initialize,
    cleanup,
    isAnalyzingStrategy,
    clearStrategyAnalysis,
    isChatSession,
    streamingOutput,
    analysisCoverage,
    logs,
    history,
    clearHistory,
    deleteHistory,
    renameHistory,
    restoreFromHistory,
    sessionUsageTotals,
    turnUsageTotals,
    taskId,
    standaloneSessionId,
    attachments,
    addAttachment,
    removeAttachment,
    backgroundSessions,
    switchToSession,
    removeBackgroundSession,
    foregroundParentSessionId,
    foregroundBgId,
  } = useExecutionStore();
  const activeAgentName = useExecutionStore((s) => s.activeAgentName);
  const backend = useSettingsStore((s) => s.backend);
  const provider = useSettingsStore((s) => s.provider);
  const model = useSettingsStore((s) => s.model);
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const sidebarCollapsed = useSettingsStore((s) => s.sidebarCollapsed);
  const setSidebarCollapsed = useSettingsStore((s) => s.setSidebarCollapsed);
  const autoPanelHoverEnabled = useSettingsStore((s) => s.autoPanelHoverEnabled);

  const [description, setDescription] = useState('');
  const [leftPanelHoverExpanded, setLeftPanelHoverExpanded] = useState(false);
  const [rightPanelHoverExpanded, setRightPanelHoverExpanded] = useState(false);
  const [rightPanelOpen, setRightPanelOpen] = useState(false);
  const [rightPanelTab, setRightPanelTab] = useState<RightPanelTab>('output');
  const [workflowMode, setWorkflowMode] = useState<WorkflowMode>('chat');
  const [supportsPointerHover, setSupportsPointerHover] = useState(false);
  const [queuedChatMessages, setQueuedChatMessages] = useState<QueuedChatMessage[]>([]);
  const [tokenEstimate, setTokenEstimate] = useState<PromptTokenEstimateResult | null>(null);
  const [isEstimatingTokenBudget, setIsEstimatingTokenBudget] = useState(false);
  const [promptTokenBudget, setPromptTokenBudget] = useState(DEFAULT_PROMPT_TOKEN_BUDGET);

  // Ref for InputBox to call pickFile externally
  const inputBoxRef = useRef<InputBoxHandle>(null);
  // Ref for ChatTranscript scroll container (used for image export)
  const chatScrollRef = useRef<HTMLDivElement>(null);
  const [isCapturing, setIsCapturing] = useState(false);
  const leftHoverTimerRef = useRef<number | null>(null);
  const rightHoverTimerRef = useRef<number | null>(null);
  const queueIdRef = useRef(0);
  const queueDispatchInFlightRef = useRef(false);
  const hasHydratedQueueRef = useRef(false);

  const workflowKernelSessionId = useWorkflowKernelStore((s) => s.sessionId);
  const workflowKernelSession = useWorkflowKernelStore((s) => s.session);
  const openWorkflowKernelSession = useWorkflowKernelStore((s) => s.openSession);
  const recoverWorkflowKernelSession = useWorkflowKernelStore((s) => s.recoverSession);
  const transitionWorkflowKernelMode = useWorkflowKernelStore((s) => s.transitionMode);
  const transitionAndSubmitWorkflowKernelInput = useWorkflowKernelStore((s) => s.transitionAndSubmitInput);
  const syncWorkflowKernelPhase = useWorkflowKernelStore((s) => s.syncModePhase);
  const refreshWorkflowKernelState = useWorkflowKernelStore((s) => s.refreshSessionState);
  const cancelWorkflowKernelOperation = useWorkflowKernelStore((s) => s.cancelOperation);
  const resetWorkflowKernel = useWorkflowKernelStore((s) => s.reset);
  const kernelBootstrapInFlightRef = useRef(false);
  const lastSyncedKernelPhasesRef = useRef<{ chat: string; plan: string; task: string }>({
    chat: '',
    plan: '',
    task: '',
  });

  const isRunning = status === 'running' || status === 'paused';

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

  // Handle workflow mode changes with context inheritance notifications
  const handleWorkflowModeChange = useCallback(
    (newMode: WorkflowMode) => {
      if (newMode === workflowMode) return;
      if (isRunning) {
        const canConfirm = typeof window !== 'undefined' && typeof window.confirm === 'function';
        const confirmed = !canConfirm
          ? true
          : window.confirm(
              t('workflow.modeSwitchConfirm', {
                defaultValue:
                  'An execution is still running. Switching modes now may change your active workflow context. Continue?',
              }),
            );
        if (!confirmed) return;
      }

      // Check for context inheritance
      const hasChatHistory = streamingOutput.length > 0;
      const hasPendingTaskContext = useExecutionStore.getState()._pendingTaskContext;

      // Show notification about context inheritance
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

      if (workflowMode === 'chat' && newMode !== 'chat' && queuedChatMessages.length > 0) {
        setQueuedChatMessages([]);
        showToast(
          t('workflow.clearQueuedMessages', {
            defaultValue: 'Cleared queued chat messages when leaving Chat mode.',
          }),
          'info',
        );
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
    [workflowMode, isRunning, streamingOutput, queuedChatMessages.length, showToast, t, transitionWorkflowKernelMode],
  );

  useEffect(() => {
    initialize();
    return () => {
      cleanup();
    };
  }, [initialize, cleanup]);

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
    if (!workflowKernelSessionId) return;
    void refreshWorkflowKernelState();
    const timer = window.setInterval(() => {
      void refreshWorkflowKernelState();
    }, 1500);
    return () => window.clearInterval(timer);
  }, [workflowKernelSessionId, refreshWorkflowKernelState]);

  useEffect(() => {
    const activeMode = workflowKernelSession?.activeMode;
    if (!activeMode || activeMode === workflowMode) return;
    setWorkflowMode(activeMode);
  }, [workflowKernelSession?.activeMode, workflowMode]);

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return;
    const media = window.matchMedia('(hover: hover) and (pointer: fine)');
    const handleChange = () => setSupportsPointerHover(media.matches);
    handleChange();
    media.addEventListener('change', handleChange);
    return () => media.removeEventListener('change', handleChange);
  }, []);

  useEffect(() => {
    if (hasHydratedQueueRef.current) return;
    hasHydratedQueueRef.current = true;

    if (typeof localStorage === 'undefined') return;
    const restored = loadPersistedSimpleChatQueue(localStorage, workspacePath, MAX_QUEUED_CHAT_MESSAGES);
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
  }, [workspacePath, showToast, t]);

  useEffect(() => {
    if (!hasHydratedQueueRef.current || typeof localStorage === 'undefined') return;

    if (queuedChatMessages.length === 0) {
      clearPersistedSimpleChatQueue(localStorage);
      return;
    }

    persistSimpleChatQueue(localStorage, queuedChatMessages, workspacePath);
  }, [queuedChatMessages, workspacePath]);

  // Handle navigation requests coming from chat cards.
  useEffect(() => {
    return listenOpenAIChanges(({ turnIndex }) => {
      setRightPanelOpen(true);
      setRightPanelHoverExpanded(false);
      setRightPanelTab('git');
      useGitStore.getState().setSelectedTab('ai-changes');
      if (typeof turnIndex === 'number') {
        useFileChangesStore.getState().selectTurn(turnIndex);
      }
    });
  }, []);

  const prevPathRef = useRef(workspacePath);
  useEffect(() => {
    if (prevPathRef.current !== workspacePath && isChatSession) {
      reset();
      clearStrategyAnalysis();
      setDescription('');
      // Reset knowledge auto-association so fresh workspace triggers re-association
      useContextSourcesStore.getState().resetAutoAssociation();
    }
    prevPathRef.current = workspacePath;
  }, [workspacePath, isChatSession, reset, clearStrategyAnalysis]);

  // File change card bridge: converts file-change events into inline chat cards
  // Both backends emit `file-change-recorded` events keyed by session ID:
  //   - Claude Code backend uses `taskId`
  //   - Standalone/multi-LLM backend uses `standaloneSessionId`
  const bridgeSessionId = taskId || standaloneSessionId;
  useEffect(() => {
    if (!bridgeSessionId || !workspacePath) return;
    const bridge = createFileChangeCardBridge(bridgeSessionId, workspacePath);
    const unlistenPromise = bridge.startListening();

    // Listen for turn end (status transitions from running to something else)
    let prevStatus = useExecutionStore.getState().status;
    const unsub = useExecutionStore.subscribe((state) => {
      if (prevStatus === 'running' && state.status !== 'running') {
        const currentTurn = state.streamingOutput.filter((l) => l.type === 'info').length - 1;
        if (currentTurn >= 0) bridge.onTurnEnd(currentTurn);
      }
      prevStatus = state.status;
    });

    return () => {
      unlistenPromise.then((fn) => fn());
      unsub();
      bridge.reset();
    };
  }, [bridgeSessionId, workspacePath]);

  const workflowPhaseLegacy = useWorkflowOrchestratorStore((s) => s.phase);
  const pendingQuestion = useWorkflowOrchestratorStore((s) => s.pendingQuestion);
  const startWorkflow = useWorkflowOrchestratorStore((s) => s.startWorkflow);
  const submitInterviewAnswer = useWorkflowOrchestratorStore((s) => s.submitInterviewAnswer);
  const skipInterviewQuestion = useWorkflowOrchestratorStore((s) => s.skipInterviewQuestion);
  const overrideConfigNatural = useWorkflowOrchestratorStore((s) => s.overrideConfigNatural);
  const addPrdFeedback = useWorkflowOrchestratorStore((s) => s.addPrdFeedback);
  const cancelWorkflow = useWorkflowOrchestratorStore((s) => s.cancelWorkflow);
  const taskWorkflowCancelling = useWorkflowOrchestratorStore((s) => s.isCancelling);
  const resetWorkflow = useWorkflowOrchestratorStore((s) => s.resetWorkflow);
  const isInterviewSubmitting =
    useWorkflowOrchestratorStore((s) => s.phase === 'interviewing') && pendingQuestion === null;

  // Plan mode orchestrator
  const planPhaseLegacy = usePlanOrchestratorStore((s) => s.phase);
  const pendingClarifyQuestion = usePlanOrchestratorStore((s) => s.pendingClarifyQuestion);
  const planIsBusy = usePlanOrchestratorStore((s) => s.isBusy);
  const startPlanWorkflow = usePlanOrchestratorStore((s) => s.startPlanWorkflow);
  const submitPlanClarification = usePlanOrchestratorStore((s) => s.submitClarification);
  const skipPlanClarification = usePlanOrchestratorStore((s) => s.skipClarification);
  const cancelPlanWorkflow = usePlanOrchestratorStore((s) => s.cancelWorkflow);
  const planWorkflowCancelling = usePlanOrchestratorStore((s) => s.isCancelling);
  const resetPlanWorkflow = usePlanOrchestratorStore((s) => s.resetWorkflow);

  const workflowPhase = workflowKernelSession?.modeSnapshots.task?.phase ?? workflowPhaseLegacy;
  const planPhase = workflowKernelSession?.modeSnapshots.plan?.phase ?? planPhaseLegacy;
  const chatPhase =
    workflowKernelSession?.modeSnapshots.chat?.phase ??
    (status === 'running'
      ? 'running'
      : status === 'paused'
        ? 'paused'
        : status === 'completed'
          ? 'completed'
          : status === 'failed'
            ? 'failed'
            : 'ready');
  const rightPanelPhase = workflowMode === 'task' ? workflowPhase : workflowMode === 'plan' ? planPhase : chatPhase;

  useEffect(() => {
    if (workflowMode !== 'task') return;
    if (!workflowPhaseLegacy) return;
    if (lastSyncedKernelPhasesRef.current.task === workflowPhaseLegacy) return;
    lastSyncedKernelPhasesRef.current.task = workflowPhaseLegacy;
    void syncWorkflowKernelPhase('task', workflowPhaseLegacy, 'task_orchestrator');
  }, [workflowMode, workflowPhaseLegacy, syncWorkflowKernelPhase]);

  useEffect(() => {
    if (workflowMode !== 'plan') return;
    if (!planPhaseLegacy) return;
    if (lastSyncedKernelPhasesRef.current.plan === planPhaseLegacy) return;
    lastSyncedKernelPhasesRef.current.plan = planPhaseLegacy;
    void syncWorkflowKernelPhase('plan', planPhaseLegacy, 'plan_orchestrator');
  }, [workflowMode, planPhaseLegacy, syncWorkflowKernelPhase]);

  useEffect(() => {
    if (workflowMode !== 'chat') return;
    const phase =
      status === 'running'
        ? 'running'
        : status === 'paused'
          ? 'paused'
          : status === 'completed'
            ? 'completed'
            : status === 'failed'
              ? 'failed'
              : 'ready';
    if (lastSyncedKernelPhasesRef.current.chat === phase) return;
    lastSyncedKernelPhasesRef.current.chat = phase;
    void syncWorkflowKernelPhase('chat', phase, 'chat_execution_status');
  }, [workflowMode, status, syncWorkflowKernelPhase]);

  const hasStructuredInterviewQuestion =
    workflowMode === 'task' &&
    workflowPhase === 'interviewing' &&
    !!pendingQuestion &&
    (pendingQuestion.inputType === 'boolean' ||
      pendingQuestion.inputType === 'single_select' ||
      pendingQuestion.inputType === 'multi_select');
  const hasTextInterviewQuestion =
    workflowMode === 'task' && workflowPhase === 'interviewing' && !!pendingQuestion && !hasStructuredInterviewQuestion;
  const hasPlanClarifyQuestion = workflowMode === 'plan' && planPhase === 'clarifying' && !!pendingClarifyQuestion;

  // Tool permission state
  const permissionRequest = useToolPermissionStore((s) => s.pendingRequest);
  const permissionQueueSize = useToolPermissionStore((s) => s.requestQueue.length);
  const isPermissionResponding = useToolPermissionStore((s) => s.isResponding);
  const respondPermission = useToolPermissionStore((s) => s.respond);
  const permissionLevel = useToolPermissionStore((s) => s.sessionLevel);
  const setPermissionLevel = useToolPermissionStore((s) => s.setSessionLevel);
  const permissionSessionId = taskId || standaloneSessionId || '';
  const contextSessionId = taskId
    ? `claude:${taskId}`
    : standaloneSessionId
      ? `standalone:${standaloneSessionId}`
      : null;

  useEffect(() => {
    if (!permissionSessionId) return;
    void setPermissionLevel(permissionSessionId, permissionLevel);
  }, [permissionSessionId, permissionLevel, setPermissionLevel]);

  const handleStart = useCallback(
    async (inputPrompt?: string) => {
      const prompt = (inputPrompt ?? description).trim();
      if (!prompt || isSubmitting || isAnalyzingStrategy) return;
      if (inputPrompt === undefined) {
        setDescription('');
      }

      const conversationContext = buildConversationHistory().map((turn) => ({
        user: turn.user,
        assistant: turn.assistant,
      }));
      await transitionAndSubmitWorkflowKernelInput(
        workflowMode,
        {
          type: 'mode_entry_prompt',
          content: prompt,
          metadata: {
            mode: workflowMode,
            source: inputPrompt === undefined ? 'composer' : 'queue_or_external',
          },
        },
        {
          conversationContext,
          artifactRefs: [],
          contextSources: ['simple_mode'],
          metadata: {
            source: 'start',
            mode: workflowMode,
          },
        },
      );

      if (workflowMode === 'task') {
        // Route Task mode through the workflow orchestrator
        await startWorkflow(prompt);
        return;
      }

      if (workflowMode === 'plan') {
        // Route Plan mode through the plan orchestrator
        await startPlanWorkflow(prompt);
        return;
      }

      await start(prompt, 'simple');
    },
    [
      description,
      isAnalyzingStrategy,
      isSubmitting,
      start,
      startWorkflow,
      startPlanWorkflow,
      transitionAndSubmitWorkflowKernelInput,
      workflowMode,
    ],
  );

  const handleFollowUp = useCallback(
    async (inputPrompt?: string) => {
      const prompt = (inputPrompt ?? description).trim();
      if (!prompt || isSubmitting) return;
      if (inputPrompt === undefined) {
        setDescription('');
      }

      // Route through orchestrator if in active Task workflow phase
      if (workflowMode === 'task' && workflowPhase !== 'idle') {
        if (workflowPhase === 'configuring') {
          await transitionAndSubmitWorkflowKernelInput(workflowMode, {
            type: 'task_configuration',
            content: prompt,
            metadata: { mode: workflowMode, phase: workflowPhase },
          });
          overrideConfigNatural(prompt);
        } else if (workflowPhase === 'reviewing_prd') {
          await transitionAndSubmitWorkflowKernelInput(workflowMode, {
            type: 'task_prd_feedback',
            content: prompt,
            metadata: { mode: workflowMode, phase: workflowPhase },
          });
          addPrdFeedback(prompt);
        } else if (workflowPhase === 'interviewing' && pendingQuestion && !hasStructuredInterviewQuestion) {
          await transitionAndSubmitWorkflowKernelInput(workflowMode, {
            type: 'task_interview_answer',
            content: prompt,
            metadata: {
              mode: workflowMode,
              phase: workflowPhase,
              questionId: pendingQuestion.questionId,
            },
          });
          await submitInterviewAnswer(prompt);
        }
        return;
      }

      // Route plan clarification through plan orchestrator
      if (workflowMode === 'plan' && planPhase === 'clarifying' && pendingClarifyQuestion) {
        await transitionAndSubmitWorkflowKernelInput(workflowMode, {
          type: 'plan_clarification',
          content: prompt,
          metadata: {
            mode: workflowMode,
            phase: planPhase,
            questionId: pendingClarifyQuestion.questionId,
          },
        });
        await submitPlanClarification({
          questionId: pendingClarifyQuestion.questionId,
          answer: prompt,
          skipped: false,
        });
        return;
      }

      await transitionAndSubmitWorkflowKernelInput(workflowMode, {
        type: 'chat_message',
        content: prompt,
        metadata: {
          mode: workflowMode,
        },
      });
      await sendFollowUp(prompt);
    },
    [
      description,
      isSubmitting,
      sendFollowUp,
      workflowMode,
      workflowPhase,
      pendingQuestion,
      planPhase,
      pendingClarifyQuestion,
      hasStructuredInterviewQuestion,
      overrideConfigNatural,
      addPrdFeedback,
      submitPlanClarification,
      submitInterviewAnswer,
      transitionAndSubmitWorkflowKernelInput,
    ],
  );

  const handleStructuredInterviewSubmit = useCallback(
    async (answer: string) => {
      const normalized = answer.trim();
      if (!normalized) return;
      const questionId = pendingQuestion?.questionId;
      await transitionAndSubmitWorkflowKernelInput('task', {
        type: 'task_interview_answer',
        content: normalized,
        metadata: {
          mode: 'task',
          phase: workflowPhase,
          source: 'structured_interview_panel',
          questionId: questionId ?? null,
        },
      });
      await submitInterviewAnswer(normalized);
    },
    [pendingQuestion?.questionId, submitInterviewAnswer, transitionAndSubmitWorkflowKernelInput, workflowPhase],
  );

  const handleSkipInterviewQuestion = useCallback(async () => {
    const questionId = pendingQuestion?.questionId;
    await transitionAndSubmitWorkflowKernelInput('task', {
      type: 'task_interview_answer',
      content: '[skip]',
      metadata: {
        mode: 'task',
        phase: workflowPhase,
        source: 'interview_skip',
        questionId: questionId ?? null,
        skipped: true,
      },
    });
    await skipInterviewQuestion();
  }, [pendingQuestion?.questionId, skipInterviewQuestion, transitionAndSubmitWorkflowKernelInput, workflowPhase]);

  const handleSkipPlanClarifyQuestion = useCallback(async () => {
    const questionId = pendingClarifyQuestion?.questionId;
    await transitionAndSubmitWorkflowKernelInput('plan', {
      type: 'plan_clarification',
      content: '[skip]',
      metadata: {
        mode: 'plan',
        phase: planPhase,
        source: 'plan_clarify_skip_question',
        questionId: questionId ?? null,
        skipped: true,
      },
    });
    if (!pendingClarifyQuestion) return;
    await submitPlanClarification({
      questionId: pendingClarifyQuestion.questionId,
      answer: '',
      skipped: true,
    });
  }, [pendingClarifyQuestion, planPhase, submitPlanClarification, transitionAndSubmitWorkflowKernelInput]);

  const handleSkipPlanClarification = useCallback(async () => {
    await transitionAndSubmitWorkflowKernelInput('plan', {
      type: 'plan_clarification',
      content: '[skip_all]',
      metadata: {
        mode: 'plan',
        phase: planPhase,
        source: 'plan_clarify_skip_all',
        questionId: pendingClarifyQuestion?.questionId ?? null,
        skippedAll: true,
      },
    });
    await skipPlanClarification();
  }, [pendingClarifyQuestion?.questionId, planPhase, skipPlanClarification, transitionAndSubmitWorkflowKernelInput]);

  const removeQueuedChatMessage = useCallback((id: string) => {
    setQueuedChatMessages((prev) => prev.filter((msg) => msg.id !== id));
  }, []);

  const queueChatMessage = useCallback(
    (prompt: string, submitAsFollowUp: boolean) => {
      setQueuedChatMessages((prev) => {
        if (prev.length >= MAX_QUEUED_CHAT_MESSAGES) {
          showToast(
            t('workflow.queueLimitReached', {
              max: MAX_QUEUED_CHAT_MESSAGES,
              defaultValue: `Queue is full (max ${MAX_QUEUED_CHAT_MESSAGES} messages).`,
            }),
            'info',
          );
          return prev;
        }

        const nextId = `queued-${Date.now()}-${queueIdRef.current++}`;
        return [...prev, { id: nextId, prompt, submitAsFollowUp }];
      });
    },
    [showToast, t],
  );

  const handleComposerSubmit = useCallback(async () => {
    const prompt = description.trim();
    if (!prompt) return;
    const taskWorkflowActive =
      workflowPhase !== 'idle' &&
      workflowPhase !== 'completed' &&
      workflowPhase !== 'failed' &&
      workflowPhase !== 'cancelled';
    const planWorkflowActive =
      planPhase !== 'idle' && planPhase !== 'completed' && planPhase !== 'failed' && planPhase !== 'cancelled';

    const submitAsFollowUp =
      isChatSession ||
      (workflowMode === 'task' && taskWorkflowActive) ||
      (workflowMode === 'plan' && planWorkflowActive);

    if (workflowMode === 'chat' && isRunning) {
      if (attachments.length > 0) {
        showToast(
          t('workflow.queueAttachmentsNotSupported', {
            defaultValue: 'Queued chat messages with new attachments are not supported yet.',
          }),
          'info',
        );
        return;
      }
      queueChatMessage(prompt, submitAsFollowUp);
      setDescription('');
      return;
    }

    if (submitAsFollowUp) {
      await handleFollowUp();
    } else {
      await handleStart();
    }
  }, [
    description,
    isChatSession,
    workflowMode,
    workflowPhase,
    planPhase,
    isRunning,
    attachments.length,
    showToast,
    t,
    queueChatMessage,
    handleFollowUp,
    handleStart,
  ]);

  const handleNewTask = useCallback(() => {
    const hasContext = streamingOutput.length > 0 || useExecutionStore.getState()._pendingTaskContext;

    clearPersistedWorkflowKernelSessionId();
    resetWorkflowKernel();
    resetWorkflow();
    resetPlanWorkflow();
    reset();
    clearStrategyAnalysis();
    setDescription('');
    setQueuedChatMessages([]);
    void openWorkflowKernelSession('chat', {
      conversationContext: [],
      artifactRefs: [],
      contextSources: ['simple_mode'],
      metadata: {
        entry: 'new_task',
      },
    });

    if (hasContext) {
      showToast(t('contextBridge.contextReset', { defaultValue: 'Context cleared for new task' }), 'info');
    }
  }, [
    clearPersistedWorkflowKernelSessionId,
    reset,
    clearStrategyAnalysis,
    resetWorkflow,
    resetPlanWorkflow,
    resetWorkflowKernel,
    openWorkflowKernelSession,
    streamingOutput,
    showToast,
    t,
  ]);

  const handleRestoreHistory = useCallback(
    (historyId: string) => {
      clearPersistedWorkflowKernelSessionId();
      resetWorkflowKernel();
      resetWorkflow();
      resetPlanWorkflow();
      restoreFromHistory(historyId);
      setRightPanelOpen(false);
      setWorkflowMode('chat');
      setDescription('');
      setQueuedChatMessages([]);
      void openWorkflowKernelSession('chat', {
        conversationContext: [],
        artifactRefs: [],
        contextSources: ['simple_mode'],
        metadata: {
          entry: 'restore_history',
          historyId,
        },
      });
    },
    [
      clearPersistedWorkflowKernelSessionId,
      restoreFromHistory,
      resetWorkflow,
      resetPlanWorkflow,
      resetWorkflowKernel,
      openWorkflowKernelSession,
    ],
  );

  const handleSwitchSession = useCallback(
    (sessionId: string) => {
      // Keep workflow/orchestrator state scoped to the foreground session.
      clearPersistedWorkflowKernelSessionId();
      resetWorkflowKernel();
      resetWorkflow();
      resetPlanWorkflow();
      switchToSession(sessionId);
      setWorkflowMode('chat');
      setDescription('');
      setQueuedChatMessages([]);
      void openWorkflowKernelSession('chat', {
        conversationContext: [],
        artifactRefs: [],
        contextSources: ['simple_mode'],
        metadata: {
          entry: 'switch_session',
          externalSessionId: sessionId,
        },
      });
    },
    [
      clearPersistedWorkflowKernelSessionId,
      resetWorkflow,
      resetPlanWorkflow,
      resetWorkflowKernel,
      openWorkflowKernelSession,
      switchToSession,
    ],
  );

  const handleCancelStructuredWorkflow = useCallback(async () => {
    if (taskWorkflowCancelling || planWorkflowCancelling) return;
    await cancelWorkflowKernelOperation('cancelled_by_user');
    if (workflowMode === 'plan') {
      await cancelPlanWorkflow();
      return;
    }
    if (workflowMode === 'task') {
      await cancelWorkflow();
    }
  }, [
    cancelWorkflowKernelOperation,
    workflowMode,
    cancelPlanWorkflow,
    cancelWorkflow,
    taskWorkflowCancelling,
    planWorkflowCancelling,
  ]);

  const handleExportImage = useCallback(async () => {
    const el = chatScrollRef.current;
    if (!el) return;

    setIsCapturing(true);
    try {
      const isDark = document.documentElement.classList.contains('dark');
      const blob = await captureElementToBlob(el, 'png', {
        backgroundColor: isDark ? '#111827' : '#ffffff',
      });
      const base64 = await blobToBase64(blob);
      const ts = localTimestampForFilename();
      const saved = await saveBinaryWithDialog(`chat-export-${ts}.png`, base64);
      if (saved) {
        showToast(t('chatToolbar.exportImageSuccess', { defaultValue: 'Image exported successfully' }), 'success');
      }
    } catch (err) {
      console.error('Export image failed:', err);
      showToast(t('chatToolbar.exportImageFailed', { defaultValue: 'Failed to export image' }), 'error');
    } finally {
      setIsCapturing(false);
    }
  }, [showToast, t]);

  const kernelStatus = workflowKernelSession?.status ?? 'active';
  const kernelSessionMode = workflowKernelSession?.activeMode ?? workflowMode;
  const hasActiveKernelSession = kernelStatus === 'active';
  const isTaskWorkflowActive =
    workflowMode === 'task' &&
    kernelSessionMode === 'task' &&
    hasActiveKernelSession &&
    workflowPhase !== 'idle' &&
    workflowPhase !== 'completed' &&
    workflowPhase !== 'failed' &&
    workflowPhase !== 'cancelled';
  const isPlanWorkflowActive =
    workflowMode === 'plan' &&
    kernelSessionMode === 'plan' &&
    hasActiveKernelSession &&
    planPhase !== 'idle' &&
    planPhase !== 'completed' &&
    planPhase !== 'failed' &&
    planPhase !== 'cancelled';
  const isTaskWorkflowBusy =
    workflowMode === 'task' &&
    (workflowPhase === 'analyzing' ||
      workflowPhase === 'exploring' ||
      workflowPhase === 'requirement_analysis' ||
      workflowPhase === 'generating_prd' ||
      workflowPhase === 'generating_design_doc' ||
      workflowPhase === 'executing' ||
      (workflowPhase === 'interviewing' && pendingQuestion === null));
  const isPlanWorkflowBusy =
    workflowMode === 'plan' &&
    (planIsBusy ||
      planPhase === 'analyzing' ||
      planPhase === 'planning' ||
      planPhase === 'executing' ||
      (planPhase === 'clarifying' && pendingClarifyQuestion === null));
  const isStructuredWorkflowCancelling =
    (workflowMode === 'task' && taskWorkflowCancelling) || (workflowMode === 'plan' && planWorkflowCancelling);
  const canQueueWhileRunning =
    workflowMode === 'chat' &&
    isRunning &&
    !executionIsCancelling &&
    !isAnalyzingStrategy &&
    !hasStructuredInterviewQuestion;
  const inputBusy =
    executionIsCancelling ||
    isAnalyzingStrategy ||
    isTaskWorkflowBusy ||
    isPlanWorkflowBusy ||
    (isSubmitting && !canQueueWhileRunning);
  const inputDisabled =
    inputBusy ||
    isStructuredWorkflowCancelling ||
    hasStructuredInterviewQuestion ||
    (workflowMode !== 'chat' && isRunning) ||
    (workflowMode === 'task' && workflowPhase === 'interviewing' && pendingQuestion === null) ||
    (workflowMode === 'plan' && planPhase === 'clarifying' && pendingClarifyQuestion === null);
  const inputLoading =
    (inputBusy ||
      (workflowMode !== 'chat' && isRunning) ||
      (workflowMode === 'task' && workflowPhase === 'interviewing' && pendingQuestion === null) ||
      (workflowMode === 'plan' && planPhase === 'clarifying' && pendingClarifyQuestion === null)) &&
    !(workflowMode === 'chat' && isRunning);
  const hoverPanelsEnabled = autoPanelHoverEnabled && supportsPointerHover;
  const isLeftPanelOpen = !sidebarCollapsed || leftPanelHoverExpanded;
  const isRightPanelOpen = rightPanelOpen || rightPanelHoverExpanded;

  const detailLineCount = useMemo(
    () => streamingOutput.filter((line) => line.type !== 'text' && line.type !== 'info').length,
    [streamingOutput],
  );

  const clearLeftHoverTimer = useCallback(() => {
    if (leftHoverTimerRef.current !== null) {
      window.clearTimeout(leftHoverTimerRef.current);
      leftHoverTimerRef.current = null;
    }
  }, []);

  const clearRightHoverTimer = useCallback(() => {
    if (rightHoverTimerRef.current !== null) {
      window.clearTimeout(rightHoverTimerRef.current);
      rightHoverTimerRef.current = null;
    }
  }, []);

  const openLeftHoverPanel = useCallback(() => {
    if (!hoverPanelsEnabled || !sidebarCollapsed) return;
    clearLeftHoverTimer();
    setLeftPanelHoverExpanded(true);
  }, [hoverPanelsEnabled, sidebarCollapsed, clearLeftHoverTimer]);

  const scheduleCloseLeftHoverPanel = useCallback(() => {
    if (!hoverPanelsEnabled || !sidebarCollapsed) return;
    clearLeftHoverTimer();
    leftHoverTimerRef.current = window.setTimeout(() => {
      setLeftPanelHoverExpanded(false);
      leftHoverTimerRef.current = null;
    }, 180);
  }, [hoverPanelsEnabled, sidebarCollapsed, clearLeftHoverTimer]);

  const openRightHoverPanel = useCallback(() => {
    if (!hoverPanelsEnabled || rightPanelOpen) return;
    clearRightHoverTimer();
    setRightPanelHoverExpanded(true);
  }, [hoverPanelsEnabled, rightPanelOpen, clearRightHoverTimer]);

  const scheduleCloseRightHoverPanel = useCallback(() => {
    if (!hoverPanelsEnabled || rightPanelOpen) return;
    clearRightHoverTimer();
    rightHoverTimerRef.current = window.setTimeout(() => {
      setRightPanelHoverExpanded(false);
      rightHoverTimerRef.current = null;
    }, 180);
  }, [hoverPanelsEnabled, rightPanelOpen, clearRightHoverTimer]);

  useEffect(() => {
    if (hoverPanelsEnabled) return;
    clearLeftHoverTimer();
    clearRightHoverTimer();
    setLeftPanelHoverExpanded(false);
    setRightPanelHoverExpanded(false);
  }, [hoverPanelsEnabled, clearLeftHoverTimer, clearRightHoverTimer]);

  useEffect(
    () => () => {
      clearLeftHoverTimer();
      clearRightHoverTimer();
    },
    [clearLeftHoverTimer, clearRightHoverTimer],
  );

  // Output button toggle logic
  const handleToggleOutput = useCallback(() => {
    if (!isRightPanelOpen) {
      setRightPanelOpen(true);
      setRightPanelHoverExpanded(false);
      setRightPanelTab('output');
    } else if (!rightPanelOpen) {
      // Hover-opened panel: convert to pinned panel on explicit toggle.
      setRightPanelOpen(true);
      setRightPanelHoverExpanded(false);
      setRightPanelTab('output');
    } else if (rightPanelTab === 'output') {
      setRightPanelOpen(false);
      setRightPanelHoverExpanded(false);
    } else {
      setRightPanelTab('output');
    }
  }, [isRightPanelOpen, rightPanelOpen, rightPanelTab]);

  useEffect(() => {
    if (workflowMode !== 'chat' || queuedChatMessages.length === 0) return;
    if (isRunning || isSubmitting || isAnalyzingStrategy || permissionRequest) return;
    if (queueDispatchInFlightRef.current) return;

    const [nextMessage] = queuedChatMessages;
    if (!nextMessage) return;

    queueDispatchInFlightRef.current = true;
    setQueuedChatMessages((prev) => prev.slice(1));
    const run = nextMessage.submitAsFollowUp ? handleFollowUp(nextMessage.prompt) : handleStart(nextMessage.prompt);
    void Promise.resolve(run).finally(() => {
      queueDispatchInFlightRef.current = false;
    });
  }, [
    workflowMode,
    queuedChatMessages,
    isRunning,
    isSubmitting,
    isAnalyzingStrategy,
    permissionRequest,
    handleFollowUp,
    handleStart,
  ]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const budget = await resolvePromptTokenBudget({
        backend,
        provider,
        model,
        fallbackBudget: DEFAULT_PROMPT_TOKEN_BUDGET,
      });
      if (!cancelled) {
        setPromptTokenBudget(budget);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [backend, provider, model]);

  useEffect(() => {
    const hasPrompt = description.trim().length > 0;
    const hasAttachments = attachments.length > 0;
    if (!hasPrompt && !hasAttachments) {
      setTokenEstimate(null);
      setIsEstimatingTokenBudget(false);
      return;
    }

    let cancelled = false;
    const timer = window.setTimeout(async () => {
      setIsEstimatingTokenBudget(true);
      try {
        const result = await invoke<CommandResponse<PromptTokenEstimateResult>>('estimate_prompt_tokens', {
          prompt: description,
          attachments: toAttachmentTokenEstimateInput(attachments),
          budgetTokens: promptTokenBudget,
        });

        if (cancelled) return;
        if (result.success && result.data) {
          setTokenEstimate(result.data);
          return;
        }
      } catch {
        // Fallback below.
      } finally {
        if (!cancelled) {
          setIsEstimatingTokenBudget(false);
        }
      }

      if (!cancelled) {
        setTokenEstimate(estimatePromptTokensFallback(description, attachments, promptTokenBudget));
      }
    }, TOKEN_ESTIMATE_DEBOUNCE_MS);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [description, attachments, promptTokenBudget]);

  return (
    <div className="h-full flex flex-col">
      {/* Main content area */}
      <div className="flex-1 min-h-0 px-4 py-2">
        <div className="relative h-full max-w-[2200px] mx-auto w-full flex">
          {hoverPanelsEnabled && (
            <>
              <div
                className="absolute left-0 top-0 bottom-0 w-2 z-20"
                onMouseEnter={openLeftHoverPanel}
                onMouseLeave={scheduleCloseLeftHoverPanel}
              />
              <div
                className="absolute right-0 top-0 bottom-0 w-2 z-20"
                onMouseEnter={openRightHoverPanel}
                onMouseLeave={scheduleCloseRightHoverPanel}
              />
            </>
          )}

          {/* Left panel: WorkspaceTreeSidebar */}
          <div
            className={clsx(
              'shrink-0 transition-all duration-200 ease-out overflow-hidden',
              isLeftPanelOpen ? 'w-[280px] opacity-100 mr-3' : 'w-0 opacity-0',
            )}
            onMouseEnter={openLeftHoverPanel}
            onMouseLeave={scheduleCloseLeftHoverPanel}
          >
            <div className="w-[280px] h-full">
              <WorkspaceTreeSidebar
                history={history}
                onRestore={handleRestoreHistory}
                onDelete={deleteHistory}
                onRename={renameHistory}
                onClear={clearHistory}
                onNewTask={handleNewTask}
                currentTask={isChatSession ? streamingOutput[0]?.content || null : null}
                backgroundSessions={backgroundSessions}
                onSwitchSession={handleSwitchSession}
                onRemoveSession={removeBackgroundSession}
                foregroundParentSessionId={foregroundParentSessionId}
                foregroundBgId={foregroundBgId}
              />
            </div>
          </div>

          {/* Middle column: conversation + toolbar + input */}
          <div className="relative flex-1 min-w-0 flex flex-col rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-hidden">
            {/* Edge collapse buttons — absolute overlay inside chat area */}
            <EdgeCollapseButton
              side="left"
              expanded={isLeftPanelOpen}
              onToggle={() => {
                if (sidebarCollapsed && leftPanelHoverExpanded) {
                  setSidebarCollapsed(false);
                } else {
                  setSidebarCollapsed(!sidebarCollapsed);
                }
                setLeftPanelHoverExpanded(false);
              }}
            />
            <EdgeCollapseButton
              side="right"
              expanded={isRightPanelOpen}
              onToggle={() => {
                if (!rightPanelOpen && rightPanelHoverExpanded) {
                  setRightPanelOpen(true);
                } else {
                  setRightPanelOpen(!rightPanelOpen);
                }
                setRightPanelHoverExpanded(false);
              }}
            />

            {/* Chat transcript */}
            <div className="flex-1 min-h-0">
              <ChatTranscript lines={streamingOutput} status={status} scrollRef={chatScrollRef} />
            </div>

            {/* Chat toolbar */}
            <ChatToolbar
              workflowMode={workflowMode}
              onWorkflowModeChange={handleWorkflowModeChange}
              onFilePick={() => inputBoxRef.current?.pickFile()}
              isFilePickDisabled={inputBusy || isRunning || !!permissionRequest}
              executionStatus={status}
              isCancelling={executionIsCancelling}
              onPause={pause}
              onResume={resume}
              onCancel={cancel}
              taskWorkflowActive={isTaskWorkflowActive}
              planWorkflowActive={isPlanWorkflowActive}
              isWorkflowCancelling={isStructuredWorkflowCancelling}
              onCancelWorkflow={handleCancelStructuredWorkflow}
              onExportImage={handleExportImage}
              isExportDisabled={streamingOutput.length === 0}
              isCapturing={isCapturing}
              rightPanelOpen={isRightPanelOpen}
              rightPanelTab={rightPanelTab}
              onToggleOutput={handleToggleOutput}
              detailLineCount={detailLineCount}
            />

            {/* Input area */}
            <div className="shrink-0 border-t border-gray-200 dark:border-gray-700">
              {/* Priority 1: Tool permission approval overlay */}
              {permissionRequest ? (
                <ToolPermissionOverlay
                  request={permissionRequest}
                  onRespond={respondPermission}
                  loading={isPermissionResponding}
                  queueSize={permissionQueueSize}
                />
              ) : (
                <div className="p-4 space-y-3">
                  {/* Structured interview keeps dedicated controls, while the composer stays mounted. */}
                  {hasStructuredInterviewQuestion && pendingQuestion && (
                    <InterviewInputPanel
                      question={pendingQuestion}
                      onSubmit={handleStructuredInterviewSubmit}
                      onSkip={handleSkipInterviewQuestion}
                      loading={isInterviewSubmitting}
                    />
                  )}

                  {/* Text interview question prompt */}
                  {hasTextInterviewQuestion && pendingQuestion && (
                    <div className="rounded-lg border border-violet-200 dark:border-violet-800 bg-violet-50/40 dark:bg-violet-900/20 px-3 py-2">
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0">
                          <p className="text-xs font-medium uppercase tracking-wide text-violet-600 dark:text-violet-400">
                            {t('workflow.interview.questionTitle', { defaultValue: 'Interview Question' })}
                          </p>
                          <p className="mt-1 text-sm font-medium text-violet-800 dark:text-violet-200">
                            {pendingQuestion.question}
                          </p>
                          {pendingQuestion.hint && (
                            <p className="mt-1 text-xs text-violet-600/80 dark:text-violet-300/80">
                              {pendingQuestion.hint}
                            </p>
                          )}
                        </div>
                        {!pendingQuestion.required && (
                          <button
                            onClick={() => {
                              void handleSkipInterviewQuestion();
                            }}
                            className="shrink-0 px-2 py-1 rounded text-xs text-violet-600 dark:text-violet-300 hover:bg-violet-100 dark:hover:bg-violet-800/50 transition-colors"
                          >
                            {t('workflow.interview.skipBtn', { defaultValue: 'Skip' })}
                          </button>
                        )}
                      </div>
                    </div>
                  )}

                  {/* Plan clarify prompt while still using the shared composer */}
                  {hasPlanClarifyQuestion && pendingClarifyQuestion && (
                    <div className="rounded-lg border border-amber-200 dark:border-amber-800 bg-amber-50/40 dark:bg-amber-900/20 px-3 py-2">
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0">
                          <p className="text-xs font-medium uppercase tracking-wide text-amber-600 dark:text-amber-400">
                            {t('planMode:clarify.title', { defaultValue: 'Clarification Needed' })}
                          </p>
                          <p className="mt-1 text-sm font-medium text-amber-800 dark:text-amber-200">
                            {pendingClarifyQuestion.question}
                          </p>
                          {pendingClarifyQuestion.hint && (
                            <p className="mt-1 text-xs text-amber-700/80 dark:text-amber-300/80">
                              {pendingClarifyQuestion.hint}
                            </p>
                          )}
                        </div>
                        <div className="shrink-0 flex items-center gap-1">
                          <button
                            onClick={() => {
                              void handleSkipPlanClarifyQuestion();
                            }}
                            className="px-2 py-1 rounded text-xs text-amber-700 dark:text-amber-300 hover:bg-amber-100 dark:hover:bg-amber-800/50 transition-colors"
                          >
                            {t('planMode:clarify.skipQuestion', { defaultValue: 'Skip' })}
                          </button>
                          <button
                            onClick={() => {
                              void handleSkipPlanClarification();
                            }}
                            className="px-2 py-1 rounded text-xs text-amber-700 dark:text-amber-300 hover:bg-amber-100 dark:hover:bg-amber-800/50 transition-colors"
                          >
                            {t('planMode:clarify.skipAll', { defaultValue: 'Skip All' })}
                          </button>
                        </div>
                      </div>
                    </div>
                  )}

                  {/* Question generation/loading hints */}
                  {workflowMode === 'task' && workflowPhase === 'interviewing' && !pendingQuestion && (
                    <div className="px-3 py-2 flex items-center gap-2 text-sm text-violet-600 dark:text-violet-400">
                      <svg
                        className="animate-spin h-4 w-4"
                        xmlns="http://www.w3.org/2000/svg"
                        fill="none"
                        viewBox="0 0 24 24"
                      >
                        <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                        <path
                          className="opacity-75"
                          fill="currentColor"
                          d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                        />
                      </svg>
                      <span>{t('workflow.interview.generating', { defaultValue: 'Generating next question...' })}</span>
                    </div>
                  )}
                  {workflowMode === 'plan' && planPhase === 'clarifying' && !pendingClarifyQuestion && (
                    <div className="px-3 py-2 flex items-center gap-2 text-sm text-amber-600 dark:text-amber-400">
                      <svg
                        className="animate-spin h-4 w-4"
                        xmlns="http://www.w3.org/2000/svg"
                        fill="none"
                        viewBox="0 0 24 24"
                      >
                        <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                        <path
                          className="opacity-75"
                          fill="currentColor"
                          d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                        />
                      </svg>
                      <span>
                        {t('planMode:clarify.generatingQuestion', {
                          defaultValue: 'Generating clarification question...',
                        })}
                      </span>
                    </div>
                  )}

                  <InputBox
                    ref={inputBoxRef}
                    value={description}
                    onChange={setDescription}
                    onSubmit={handleComposerSubmit}
                    disabled={inputDisabled}
                    enterSubmits={false}
                    placeholder={
                      inputDisabled && !canQueueWhileRunning
                        ? t('workflow.input.waitingPlaceholder', { defaultValue: 'Waiting for response...' })
                        : workflowMode === 'task' && workflowPhase === 'configuring'
                          ? t('workflow.input.configuringPlaceholder', {
                              defaultValue:
                                'Type config overrides (e.g. "6 parallel, enable TDD") or click Continue above...',
                            })
                          : workflowMode === 'task' && workflowPhase === 'reviewing_prd'
                            ? t('workflow.input.prdFeedbackPlaceholder', {
                                defaultValue: 'Add feedback or press Approve on the PRD card...',
                              })
                            : hasTextInterviewQuestion
                              ? t('workflow.input.interviewPlaceholder', {
                                  defaultValue: 'Type your answer to the interview question...',
                                })
                              : workflowMode === 'plan' && planPhase === 'clarifying' && pendingClarifyQuestion
                                ? t('planMode:clarify.inputPlaceholder', { defaultValue: 'Type your clarification...' })
                                : workflowMode === 'task'
                                  ? t('workflow.input.taskPlaceholder', {
                                      defaultValue: 'Describe a task (implementation / analysis / refactor)...',
                                    })
                                  : workflowMode === 'plan'
                                    ? t('workflow.input.planPlaceholder', {
                                        defaultValue:
                                          'Describe a task to decompose and execute (writing, research, etc.)...',
                                      })
                                    : isRunning
                                      ? t('workflow.queue.placeholder', {
                                          defaultValue: 'Execution in progress. Your message will be queued...',
                                        })
                                      : t('input.followUpPlaceholder', {
                                          defaultValue: 'Type a normal chat message...',
                                        })
                    }
                    isLoading={inputLoading}
                    allowSubmitWhileLoading={canQueueWhileRunning}
                    attachments={attachments}
                    onAttach={addAttachment}
                    onRemoveAttachment={removeAttachment}
                    workspacePath={workspacePath}
                    activeAgentName={activeAgentName}
                    onClearAgent={() => {
                      useAgentsStore.getState().clearActiveAgent();
                      useExecutionStore.setState({ activeAgentId: null, activeAgentName: null });
                    }}
                  />

                  {workflowMode === 'chat' && queuedChatMessages.length > 0 && (
                    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/60 px-3 py-2">
                      <p className="text-xs font-medium text-gray-600 dark:text-gray-300">
                        {t('workflow.queue.title', {
                          count: queuedChatMessages.length,
                          max: MAX_QUEUED_CHAT_MESSAGES,
                          defaultValue: `Queued messages (${queuedChatMessages.length}/${MAX_QUEUED_CHAT_MESSAGES})`,
                        })}
                      </p>
                      <div className="mt-2 space-y-1">
                        {queuedChatMessages.map((message, index) => (
                          <div
                            key={message.id}
                            className="flex items-center gap-2 rounded bg-white dark:bg-gray-900 px-2 py-1 border border-gray-200 dark:border-gray-700"
                          >
                            <span className="text-2xs text-gray-500 dark:text-gray-400 shrink-0">#{index + 1}</span>
                            <span className="text-xs text-gray-700 dark:text-gray-200 truncate flex-1">
                              {message.prompt}
                            </span>
                            <button
                              onClick={() => removeQueuedChatMessage(message.id)}
                              className="text-2xs text-red-500 hover:text-red-600 dark:text-red-400 dark:hover:text-red-300 transition-colors"
                              title={t('workflow.queue.remove', { defaultValue: 'Remove queued message' })}
                            >
                              {t('workflow.queue.removeShort', { defaultValue: 'Remove' })}
                            </button>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              )}
              {apiError && (
                <div className="mx-4 mb-3 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                  <p className="text-sm text-red-600 dark:text-red-400">{apiError}</p>
                </div>
              )}
            </div>
          </div>

          {/* Right panel: Output + Git tabs */}
          <div
            className={clsx(
              'shrink-0 transition-all duration-200 ease-out overflow-hidden',
              isRightPanelOpen ? 'w-[620px] opacity-100 ml-3' : 'w-0 opacity-0',
            )}
            onMouseEnter={openRightHoverPanel}
            onMouseLeave={scheduleCloseRightHoverPanel}
          >
            <div className="w-[620px] h-full">
              <TabbedRightPanel
                activeTab={rightPanelTab}
                onTabChange={setRightPanelTab}
                workflowMode={workflowMode}
                workflowPhase={rightPanelPhase}
                logs={logs}
                analysisCoverage={analysisCoverage}
                streamingOutput={streamingOutput}
                workspacePath={workspacePath}
                contextSessionId={contextSessionId}
              />
            </div>
          </div>
        </div>
      </div>

      {/* Bottom status bar */}
      <BottomStatusBar
        connectionStatus={connectionStatus}
        workspacePath={workspacePath}
        permissionLevel={permissionLevel}
        onPermissionLevelChange={(level) => setPermissionLevel(permissionSessionId, level)}
        sessionId={permissionSessionId}
        turnUsage={turnUsageTotals}
        sessionUsage={sessionUsageTotals}
        tokenEstimate={tokenEstimate}
        isEstimatingTokenBudget={isEstimatingTokenBudget}
      />
    </div>
  );
}

export default SimpleMode;
