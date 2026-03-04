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

import { useEffect, useMemo, useRef, useState, useCallback, type MouseEvent as ReactMouseEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { type InputBoxHandle } from './InputBox';
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
import { ChatTranscript } from './ChatTranscript';
import type { PlanClarifyQuestionCardData } from '../../types/planModeCard';
import type { InterviewQuestionCardData } from '../../types/workflowCard';
import {
  DEFAULT_PROMPT_TOKEN_BUDGET,
  estimatePromptTokensFallback,
  toAttachmentTokenEstimateInput,
  type PromptTokenEstimateResult,
} from './tokenBudget';
import { resolvePromptTokenBudget } from '../../lib/promptTokenBudget';
import { useSimpleModeController } from './useSimpleModeController';
import { SimplePanelLayout } from './SimplePanelLayout';
import { SimpleInputSection } from './SimpleInputSection';
import { SimpleInputComposer } from './SimpleInputComposer';
import { WorkflowModeSwitchDialog } from './WorkflowModeSwitchDialog';
import { useWorkflowModeSwitchGuard } from './useWorkflowModeSwitchGuard';
import { useWorkflowKernelSessionBridge } from './useWorkflowKernelSessionBridge';
import { useSimpleInputRouting } from './useSimpleInputRouting';
import { useQueuedChatMessages } from './useQueuedChatMessages';
import { cancelActiveWorkflow, submitWorkflowInputWithTracking } from '../../store/simpleWorkflowCoordinator';
import type { WorkflowMode } from '../../types/workflowKernel';

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

const MAX_QUEUED_CHAT_MESSAGES = 3;
const TOKEN_ESTIMATE_DEBOUNCE_MS = 180;
const RIGHT_PANEL_WIDTH_STORAGE_PREFIX = 'simple_mode_right_panel_width_v1:';
const DEFAULT_RIGHT_PANEL_WIDTH = 620;
const MIN_RIGHT_PANEL_WIDTH = 420;
const MAX_RIGHT_PANEL_WIDTH = 960;

function rightPanelWidthStorageKey(workspacePath: string | null): string {
  return `${RIGHT_PANEL_WIDTH_STORAGE_PREFIX}${workspacePath || '__default_workspace__'}`;
}

export function SimpleModeShell() {
  const { t } = useTranslation('simpleMode');
  const { showToast } = useToast();
  const simpleController = useSimpleModeController();
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
    clearAttachments,
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
  const [rightPanelWidth, setRightPanelWidth] = useState(DEFAULT_RIGHT_PANEL_WIDTH);
  const [rightPanelTab, setRightPanelTab] = useState<RightPanelTab>('output');
  const [workflowMode, setWorkflowMode] = useState<WorkflowMode>('chat');
  const [supportsPointerHover, setSupportsPointerHover] = useState(false);
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
  const rightPanelResizeRef = useRef<{ startX: number; startWidth: number } | null>(null);

  const workflowKernelSessionId = useWorkflowKernelStore((s) => s.sessionId);
  const workflowKernelSession = useWorkflowKernelStore((s) => s.session);
  const openWorkflowKernelSession = useWorkflowKernelStore((s) => s.openSession);
  const recoverWorkflowKernelSession = useWorkflowKernelStore((s) => s.recoverSession);
  const transitionWorkflowKernelMode = useWorkflowKernelStore((s) => s.transitionMode);
  const transitionAndSubmitWorkflowKernelInput = useWorkflowKernelStore((s) => s.transitionAndSubmitInput);
  const linkWorkflowKernelModeSession = useWorkflowKernelStore((s) => s.linkModeSession);
  const cancelWorkflowKernelOperation = useWorkflowKernelStore((s) => s.cancelOperation);
  const resetWorkflowKernel = useWorkflowKernelStore((s) => s.reset);

  const isRunning = simpleController.isRunning;

  const { clearPersistedWorkflowKernelSessionId } = useWorkflowKernelSessionBridge({
    workspacePath,
    workflowMode,
    workflowKernelSessionId,
    workflowKernelSessionActiveMode: workflowKernelSession?.activeMode ?? null,
    setWorkflowMode,
    openWorkflowKernelSession,
    recoverWorkflowKernelSession,
  });

  useEffect(() => {
    initialize();
    return () => {
      cleanup();
    };
  }, [initialize, cleanup]);

  useEffect(() => {
    if (typeof localStorage === 'undefined') return;
    const stored = localStorage.getItem(rightPanelWidthStorageKey(workspacePath));
    if (!stored) return;
    const parsed = Number.parseInt(stored, 10);
    if (!Number.isFinite(parsed)) return;
    setRightPanelWidth(Math.max(MIN_RIGHT_PANEL_WIDTH, Math.min(MAX_RIGHT_PANEL_WIDTH, parsed)));
  }, [workspacePath]);

  useEffect(() => {
    if (typeof localStorage === 'undefined') return;
    localStorage.setItem(rightPanelWidthStorageKey(workspacePath), String(Math.round(rightPanelWidth)));
  }, [workspacePath, rightPanelWidth]);

  useEffect(() => {
    if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return;
    const media = window.matchMedia('(hover: hover) and (pointer: fine)');
    const handleChange = () => setSupportsPointerHover(media.matches);
    handleChange();
    media.addEventListener('change', handleChange);
    return () => media.removeEventListener('change', handleChange);
  }, []);

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

  const startWorkflow = useWorkflowOrchestratorStore((s) => s.startWorkflow);
  const submitInterviewAnswer = useWorkflowOrchestratorStore((s) => s.submitInterviewAnswer);
  const skipInterviewQuestion = useWorkflowOrchestratorStore((s) => s.skipInterviewQuestion);
  const overrideConfigNatural = useWorkflowOrchestratorStore((s) => s.overrideConfigNatural);
  const addPrdFeedback = useWorkflowOrchestratorStore((s) => s.addPrdFeedback);
  const cancelWorkflow = useWorkflowOrchestratorStore((s) => s.cancelWorkflow);
  const taskWorkflowCancelling = useWorkflowOrchestratorStore((s) => s.isCancelling);
  const resetWorkflow = useWorkflowOrchestratorStore((s) => s.resetWorkflow);
  const interviewStorePhase = useWorkflowOrchestratorStore((s) => s.phase);
  const syncTaskRuntimeFromKernel = useWorkflowOrchestratorStore((s) => s.syncRuntimeFromKernel);

  // Plan mode orchestrator
  const planIsBusy = usePlanOrchestratorStore((s) => s.isBusy);
  const startPlanWorkflow = usePlanOrchestratorStore((s) => s.startPlanWorkflow);
  const submitPlanClarification = usePlanOrchestratorStore((s) => s.submitClarification);
  const skipPlanClarification = usePlanOrchestratorStore((s) => s.skipClarification);
  const planOrchestratorPhase = usePlanOrchestratorStore((s) => s.phase);
  const cancelPlanWorkflow = usePlanOrchestratorStore((s) => s.cancelWorkflow);
  const planWorkflowCancelling = usePlanOrchestratorStore((s) => s.isCancelling);
  const resetPlanWorkflow = usePlanOrchestratorStore((s) => s.resetWorkflow);
  const syncPlanRuntimeFromKernel = usePlanOrchestratorStore((s) => s.syncRuntimeFromKernel);

  const workflowKernelTaskPhase = workflowKernelSession?.modeSnapshots.task?.phase ?? 'idle';
  const workflowKernelPlanPhase = workflowKernelSession?.modeSnapshots.plan?.phase ?? 'idle';
  const workflowKernelChatPhase = workflowKernelSession?.modeSnapshots.chat?.phase ?? 'ready';
  const workflowKernelPendingInterview = workflowKernelSession?.modeSnapshots.task?.pendingInterview ?? null;
  const workflowKernelPendingClarification = workflowKernelSession?.modeSnapshots.plan?.pendingClarification ?? null;
  const workflowKernelLinkedTaskSessionId = workflowKernelSession?.linkedModeSessions?.task ?? null;
  const workflowKernelLinkedPlanSessionId = workflowKernelSession?.linkedModeSessions?.plan ?? null;

  const kernelInterviewQuestion = useMemo<InterviewQuestionCardData | null>(() => {
    if (!workflowKernelPendingInterview) return null;
    const inputType: InterviewQuestionCardData['inputType'] = (() => {
      switch (workflowKernelPendingInterview.inputType) {
        case 'boolean':
          return 'boolean';
        case 'single_select':
          return 'single_select';
        case 'multi_select':
          return 'multi_select';
        case 'textarea':
          return 'textarea';
        case 'text':
        case 'list':
        default:
          return 'text';
      }
    })();

    return {
      questionId: workflowKernelPendingInterview.questionId,
      question: workflowKernelPendingInterview.question,
      hint: workflowKernelPendingInterview.hint,
      required: workflowKernelPendingInterview.required,
      inputType,
      options: workflowKernelPendingInterview.options ?? [],
      allowCustom: workflowKernelPendingInterview.allowCustom ?? true,
      questionNumber: workflowKernelPendingInterview.questionNumber ?? 1,
      totalQuestions: workflowKernelPendingInterview.totalQuestions ?? 1,
    };
  }, [workflowKernelPendingInterview]);

  const kernelPlanClarifyQuestion = useMemo<PlanClarifyQuestionCardData | null>(() => {
    if (!workflowKernelPendingClarification) return null;
    const inputType: PlanClarifyQuestionCardData['inputType'] = (() => {
      switch (workflowKernelPendingClarification.inputType) {
        case 'boolean':
          return 'boolean';
        case 'single_select':
          return 'single_select';
        case 'textarea':
          return 'textarea';
        case 'text':
        default:
          return 'text';
      }
    })();

    return {
      questionId: workflowKernelPendingClarification.questionId,
      question: workflowKernelPendingClarification.question,
      hint: workflowKernelPendingClarification.hint,
      inputType,
      options: workflowKernelPendingClarification.options ?? [],
    };
  }, [workflowKernelPendingClarification]);

  const taskPendingQuestion = kernelInterviewQuestion;
  const planPendingQuestion = kernelPlanClarifyQuestion;
  const workflowPhase = workflowKernelTaskPhase;
  const planPhase = planOrchestratorPhase === 'clarification_error' ? 'clarification_error' : workflowKernelPlanPhase;
  const chatPhase = workflowKernelChatPhase;
  const rightPanelPhase = workflowMode === 'task' ? workflowPhase : workflowMode === 'plan' ? planPhase : chatPhase;
  const taskInterviewingPhase = workflowMode === 'task' && workflowPhase === 'interviewing';
  const planClarifyingPhase = workflowMode === 'plan' && planPhase === 'clarifying';

  const hasStructuredInterviewQuestion =
    taskInterviewingPhase &&
    !!taskPendingQuestion &&
    (taskPendingQuestion.inputType === 'boolean' ||
      taskPendingQuestion.inputType === 'single_select' ||
      taskPendingQuestion.inputType === 'multi_select');
  const hasTextInterviewQuestion = taskInterviewingPhase && !!taskPendingQuestion && !hasStructuredInterviewQuestion;
  const hasPlanClarifyQuestion = planClarifyingPhase && !!planPendingQuestion;
  const isTaskWorkflowActiveForSwitchGuard =
    workflowMode === 'task' &&
    workflowPhase !== 'idle' &&
    workflowPhase !== 'completed' &&
    workflowPhase !== 'failed' &&
    workflowPhase !== 'cancelled';
  const isPlanWorkflowActiveForSwitchGuard =
    workflowMode === 'plan' &&
    planPhase !== 'idle' &&
    planPhase !== 'completed' &&
    planPhase !== 'failed' &&
    planPhase !== 'cancelled';
  const effectiveTaskPhaseForInput = taskInterviewingPhase ? 'interviewing' : workflowPhase;
  const effectivePlanPhaseForInput = planClarifyingPhase ? 'clarifying' : planPhase;
  const isInterviewSubmitting =
    taskInterviewingPhase && taskPendingQuestion === null && interviewStorePhase === 'interviewing';

  useEffect(() => {
    syncTaskRuntimeFromKernel({
      sessionId: workflowKernelLinkedTaskSessionId,
      interviewId: workflowKernelPendingInterview?.interviewId ?? null,
      pendingQuestion: kernelInterviewQuestion,
      phase: workflowKernelTaskPhase,
    });
    syncPlanRuntimeFromKernel({
      sessionId: workflowKernelLinkedPlanSessionId,
      phase: workflowKernelPlanPhase,
      pendingClarifyQuestion: kernelPlanClarifyQuestion,
    });
  }, [
    syncTaskRuntimeFromKernel,
    syncPlanRuntimeFromKernel,
    workflowKernelLinkedTaskSessionId,
    workflowKernelLinkedPlanSessionId,
    workflowKernelPendingInterview?.interviewId,
    workflowKernelTaskPhase,
    workflowKernelPlanPhase,
    kernelInterviewQuestion,
    kernelPlanClarifyQuestion,
  ]);

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

  const {
    handleStart,
    handleFollowUp,
    handleStructuredInterviewSubmit,
    handleSkipInterviewQuestion,
    handleSkipPlanClarifyQuestion,
    handleSkipPlanClarification,
  } = useSimpleInputRouting({
    description,
    setDescription,
    workflowMode,
    workflowPhase,
    planPhase,
    isSubmitting,
    isAnalyzingStrategy,
    start,
    sendFollowUp,
    startWorkflow,
    startPlanWorkflow,
    overrideConfigNatural,
    addPrdFeedback,
    submitPlanClarification,
    submitInterviewAnswer,
    skipInterviewQuestion,
    skipPlanClarification,
    taskInterviewingPhase,
    taskPendingQuestion,
    planClarifyingPhase,
    planPendingQuestion,
    hasStructuredInterviewQuestion,
    linkWorkflowKernelModeSession,
    cancelWorkflowKernelOperation,
    transitionAndSubmitWorkflowKernelInput,
  });

  const { queuedChatMessages, queueChatMessage, removeQueuedChatMessage, clearQueuedChatMessages } =
    useQueuedChatMessages({
      workspacePath,
      workflowMode,
      maxQueuedChatMessages: MAX_QUEUED_CHAT_MESSAGES,
      isRunning,
      isSubmitting,
      isAnalyzingStrategy,
      permissionRequest,
      isTaskWorkflowBusy:
        workflowMode === 'task' &&
        (effectiveTaskPhaseForInput === 'analyzing' ||
          effectiveTaskPhaseForInput === 'exploring' ||
          effectiveTaskPhaseForInput === 'requirement_analysis' ||
          effectiveTaskPhaseForInput === 'generating_prd' ||
          effectiveTaskPhaseForInput === 'generating_design_doc' ||
          effectiveTaskPhaseForInput === 'executing'),
      isPlanWorkflowBusy:
        workflowMode === 'plan' &&
        (planIsBusy ||
          effectivePlanPhaseForInput === 'analyzing' ||
          effectivePlanPhaseForInput === 'planning' ||
          effectivePlanPhaseForInput === 'executing'),
      attachments,
      addAttachment,
      clearAttachments,
      handleFollowUp,
      handleStart,
      showToast,
      t,
    });

  const {
    modeSwitchConfirmOpen,
    modeSwitchBlockReason,
    handleWorkflowModeChange,
    handleConfirmModeSwitch,
    handleModeSwitchDialogOpenChange,
  } = useWorkflowModeSwitchGuard({
    workflowMode,
    isRunning,
    workflowPhase,
    planPhase,
    isTaskWorkflowActive: isTaskWorkflowActiveForSwitchGuard,
    isPlanWorkflowActive: isPlanWorkflowActiveForSwitchGuard,
    hasStructuredInterviewQuestion,
    hasPlanClarifyQuestion,
    streamingOutput,
    queuedChatMessagesLength: queuedChatMessages.length,
    clearQueuedChatMessages,
    setWorkflowMode,
    transitionWorkflowKernelMode,
    showToast,
    t,
  });
  const modeSwitchLockReasonText = modeSwitchBlockReason
    ? t(`workflow.modeSwitchLockReason.${modeSwitchBlockReason}`, {
        defaultValue: t('workflow.modeSwitchConfirm', {
          defaultValue:
            'An execution is still running. Switching modes now may change your active workflow context. Continue?',
        }),
      })
    : null;

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

    const queueableExecution =
      !executionIsCancelling &&
      !isAnalyzingStrategy &&
      !taskWorkflowCancelling &&
      !planWorkflowCancelling &&
      !hasStructuredInterviewQuestion &&
      ((workflowMode === 'chat' && isRunning) ||
        (workflowMode === 'task' && workflowPhase === 'executing') ||
        (workflowMode === 'plan' && planPhase === 'executing'));

    if (queueableExecution) {
      const submitted = await submitWorkflowInputWithTracking({
        transitionAndSubmitInput: transitionAndSubmitWorkflowKernelInput,
        targetMode: workflowMode,
        intent: {
          type: 'follow_up_intent',
          content: prompt,
          metadata: {
            mode: workflowMode,
            queued: true,
            source: 'simple_mode_follow_up_queue',
            queueDepthBeforeEnqueue: queuedChatMessages.length,
            phase: workflowMode === 'task' ? workflowPhase : workflowMode === 'plan' ? planPhase : null,
            attachmentCount: attachments.length,
          },
        },
      });
      if (!submitted) return;
      const queuedAttachments = [...attachments];
      queueChatMessage(prompt, submitAsFollowUp, workflowMode, queuedAttachments);
      if (queuedAttachments.length > 0) {
        clearAttachments();
      }
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
    executionIsCancelling,
    isAnalyzingStrategy,
    taskWorkflowCancelling,
    planWorkflowCancelling,
    hasStructuredInterviewQuestion,
    transitionAndSubmitWorkflowKernelInput,
    queuedChatMessages.length,
    attachments,
    clearAttachments,
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
    clearQueuedChatMessages();
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
    clearQueuedChatMessages,
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
      clearQueuedChatMessages();
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
      clearQueuedChatMessages,
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
      clearQueuedChatMessages();
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
      clearQueuedChatMessages,
    ],
  );

  const handleCancelStructuredWorkflow = useCallback(async () => {
    await cancelActiveWorkflow({
      workflowMode,
      taskWorkflowCancelling,
      planWorkflowCancelling,
      cancelKernelOperation: cancelWorkflowKernelOperation,
      cancelTaskWorkflow: cancelWorkflow,
      cancelPlanWorkflow: cancelPlanWorkflow,
    });
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
    (effectiveTaskPhaseForInput === 'analyzing' ||
      effectiveTaskPhaseForInput === 'exploring' ||
      effectiveTaskPhaseForInput === 'requirement_analysis' ||
      effectiveTaskPhaseForInput === 'generating_prd' ||
      effectiveTaskPhaseForInput === 'generating_design_doc' ||
      effectiveTaskPhaseForInput === 'executing');
  const isPlanWorkflowBusy =
    workflowMode === 'plan' &&
    (planIsBusy ||
      effectivePlanPhaseForInput === 'analyzing' ||
      effectivePlanPhaseForInput === 'planning' ||
      effectivePlanPhaseForInput === 'executing');
  const isStructuredWorkflowCancelling =
    (workflowMode === 'task' && taskWorkflowCancelling) || (workflowMode === 'plan' && planWorkflowCancelling);
  const canQueueWhileRunning =
    !executionIsCancelling &&
    !isAnalyzingStrategy &&
    !isStructuredWorkflowCancelling &&
    !hasStructuredInterviewQuestion &&
    ((workflowMode === 'chat' && isRunning) ||
      (workflowMode === 'task' && isTaskWorkflowActive && effectiveTaskPhaseForInput === 'executing') ||
      (workflowMode === 'plan' && isPlanWorkflowActive && effectivePlanPhaseForInput === 'executing'));
  const inputBusy =
    executionIsCancelling ||
    isAnalyzingStrategy ||
    isTaskWorkflowBusy ||
    isPlanWorkflowBusy ||
    (isSubmitting && !canQueueWhileRunning);
  const inputDisabled =
    (inputBusy && !canQueueWhileRunning) ||
    isStructuredWorkflowCancelling ||
    hasStructuredInterviewQuestion ||
    (!canQueueWhileRunning && workflowMode !== 'chat' && isRunning);
  const inputLoading = inputBusy && !canQueueWhileRunning;
  const handleClearActiveAgent = useCallback(() => {
    useAgentsStore.getState().clearActiveAgent();
    useExecutionStore.setState({ activeAgentId: null, activeAgentName: null });
  }, []);
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

  const clampRightPanelWidth = useCallback((value: number) => {
    const viewportLimit =
      typeof window === 'undefined'
        ? MAX_RIGHT_PANEL_WIDTH
        : Math.max(MIN_RIGHT_PANEL_WIDTH, Math.floor(window.innerWidth * 0.75));
    return Math.max(MIN_RIGHT_PANEL_WIDTH, Math.min(Math.min(MAX_RIGHT_PANEL_WIDTH, viewportLimit), value));
  }, []);

  const handleRightPanelResizeStart = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
      if (!isRightPanelOpen) return;
      rightPanelResizeRef.current = {
        startX: event.clientX,
        startWidth: rightPanelWidth,
      };

      const handleMouseMove = (moveEvent: MouseEvent) => {
        const current = rightPanelResizeRef.current;
        if (!current) return;
        const delta = current.startX - moveEvent.clientX;
        setRightPanelWidth(clampRightPanelWidth(current.startWidth + delta));
      };

      const handleMouseUp = () => {
        rightPanelResizeRef.current = null;
        window.removeEventListener('mousemove', handleMouseMove);
        window.removeEventListener('mouseup', handleMouseUp);
      };

      window.addEventListener('mousemove', handleMouseMove);
      window.addEventListener('mouseup', handleMouseUp);
      event.preventDefault();
    },
    [clampRightPanelWidth, isRightPanelOpen, rightPanelWidth],
  );

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
      <SimplePanelLayout
        hoverPanelsEnabled={hoverPanelsEnabled}
        isLeftPanelOpen={isLeftPanelOpen}
        isRightPanelOpen={isRightPanelOpen}
        rightPanelWidth={rightPanelWidth}
        onLeftEdgeEnter={openLeftHoverPanel}
        onLeftEdgeLeave={scheduleCloseLeftHoverPanel}
        onRightEdgeEnter={openRightHoverPanel}
        onRightEdgeLeave={scheduleCloseRightHoverPanel}
        leftPanel={
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
        }
        middlePanel={
          <div className="relative flex-1 min-w-0 flex flex-col rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-hidden">
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

            <div className="flex-1 min-h-0">
              <ChatTranscript lines={streamingOutput} status={status} scrollRef={chatScrollRef} />
            </div>

            <ChatToolbar
              workflowMode={workflowMode}
              onWorkflowModeChange={handleWorkflowModeChange}
              modeSwitchLocked={!!modeSwitchBlockReason}
              modeSwitchLockReason={modeSwitchLockReasonText}
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

            <SimpleInputSection
              permissionRequest={permissionRequest}
              isPermissionResponding={isPermissionResponding}
              permissionQueueSize={permissionQueueSize}
              onRespondPermission={respondPermission}
              apiError={apiError}
            >
              <SimpleInputComposer
                t={t}
                workflowMode={workflowMode}
                workflowPhase={workflowPhase}
                isRunning={isRunning}
                taskInterviewingPhase={taskInterviewingPhase}
                planClarifyingPhase={planClarifyingPhase}
                hasStructuredInterviewQuestion={hasStructuredInterviewQuestion}
                hasTextInterviewQuestion={hasTextInterviewQuestion}
                hasPlanClarifyQuestion={hasPlanClarifyQuestion}
                taskPendingQuestion={taskPendingQuestion}
                planPendingQuestion={planPendingQuestion}
                onStructuredInterviewSubmit={handleStructuredInterviewSubmit}
                onSkipInterviewQuestion={handleSkipInterviewQuestion}
                onSkipPlanClarifyQuestion={handleSkipPlanClarifyQuestion}
                onSkipPlanClarification={handleSkipPlanClarification}
                isInterviewSubmitting={isInterviewSubmitting}
                inputBoxRef={inputBoxRef}
                description={description}
                onDescriptionChange={setDescription}
                onSubmit={handleComposerSubmit}
                inputDisabled={inputDisabled}
                canQueueWhileRunning={canQueueWhileRunning}
                inputLoading={inputLoading}
                attachments={attachments}
                onAttach={addAttachment}
                onRemoveAttachment={removeAttachment}
                workspacePath={workspacePath}
                activeAgentName={activeAgentName}
                onClearAgent={handleClearActiveAgent}
                queuedChatMessages={queuedChatMessages}
                onRemoveQueuedChatMessage={removeQueuedChatMessage}
                maxQueuedChatMessages={MAX_QUEUED_CHAT_MESSAGES}
              />
            </SimpleInputSection>
          </div>
        }
        rightPanel={
          <>
            {isRightPanelOpen && (
              <div
                className="absolute left-0 top-0 z-20 h-full w-1.5 cursor-col-resize bg-transparent hover:bg-primary-200/70 dark:hover:bg-primary-700/50 transition-colors"
                onMouseDown={handleRightPanelResizeStart}
                title={t('rightPanel.resize', { defaultValue: 'Resize panel' })}
              />
            )}
            <div className="h-full" style={{ width: rightPanelWidth }}>
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
          </>
        }
      />

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

      <WorkflowModeSwitchDialog
        open={modeSwitchConfirmOpen}
        onOpenChange={handleModeSwitchDialogOpenChange}
        onConfirm={handleConfirmModeSwitch}
        reason={modeSwitchLockReasonText}
      />
    </div>
  );
}

export default SimpleModeShell;
