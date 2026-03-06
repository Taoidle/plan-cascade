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
import { TabbedRightPanel } from './TabbedRightPanel';
import { useExecutionStore } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';
import { useWorkflowOrchestratorStore } from '../../store/workflowOrchestrator';
import { usePlanOrchestratorStore } from '../../store/planOrchestrator';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import { useGitStore } from '../../store/git';
import { useFileChangesStore } from '../../store/fileChanges';
import { useToolPermissionStore } from '../../store/toolPermission';
import { useAgentsStore } from '../../store/agents';
import { useWorkflowObservabilityStore } from '../../store/workflowObservability';
import { createFileChangeCardBridge } from '../../lib/fileChangeCardBridge';
import { listenOpenAIChanges } from '../../lib/simpleModeNavigation';
import { useToast } from '../shared/Toast';
import { useContextSourcesStore } from '../../store/contextSources';
import { ChatTranscript } from './ChatTranscript';
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
import { useSimpleInputRouting } from './useSimpleInputRouting';
import { useSimpleExport } from './useSimpleExport';
import { useSimplePanelState } from './useSimplePanelState';
import { useSimpleModeSwitch } from './useSimpleModeSwitch';
import { useSimpleKernelSession } from './useSimpleKernelSession';
import { useSimpleQueueRuntime } from './useSimpleQueueRuntime';
import { useWorkflowQuestionSpecs } from './useWorkflowQuestionSpecs';
import { buildConversationHistory } from '../../lib/contextBridge';
import {
  cancelActiveWorkflow,
  submitWorkflowInputWithTracking,
  switchModeSafely,
} from '../../store/simpleWorkflowCoordinator';
import type { WorkflowMode } from '../../types/workflowKernel';
import {
  selectKernelChatRuntime,
  selectKernelPlanRuntime,
  selectKernelRuntimeStatus,
  selectKernelTaskRuntime,
} from '../../store/workflowKernelSelectors';
import {
  isPlanPhaseBusy,
  isTaskPhaseBusy,
  isWorkflowModeActive,
  markUnknownPhaseForReporting,
} from '../../store/workflowPhaseModel';

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

const MAX_QUEUED_CHAT_MESSAGES = 20;
const TOKEN_ESTIMATE_DEBOUNCE_MS = 180;
const MIN_RIGHT_PANEL_WIDTH = 420;
const MAX_RIGHT_PANEL_WIDTH = 960;

export function SimpleModeShell() {
  const { t } = useTranslation('simpleMode');
  const { showToast } = useToast();
  const simpleController = useSimpleModeController();
  const status = useExecutionStore((s) => s.status);
  const executionIsCancelling = useExecutionStore((s) => s.isCancelling);
  const connectionStatus = useExecutionStore((s) => s.connectionStatus);
  const isSubmitting = useExecutionStore((s) => s.isSubmitting);
  const apiError = useExecutionStore((s) => s.apiError);
  const start = useExecutionStore((s) => s.start);
  const sendFollowUp = useExecutionStore((s) => s.sendFollowUp);
  const pause = useExecutionStore((s) => s.pause);
  const resume = useExecutionStore((s) => s.resume);
  const cancel = useExecutionStore((s) => s.cancel);
  const reset = useExecutionStore((s) => s.reset);
  const initialize = useExecutionStore((s) => s.initialize);
  const cleanup = useExecutionStore((s) => s.cleanup);
  const isAnalyzingStrategy = useExecutionStore((s) => s.isAnalyzingStrategy);
  const clearStrategyAnalysis = useExecutionStore((s) => s.clearStrategyAnalysis);
  const isChatSession = useExecutionStore((s) => s.isChatSession);
  const streamingOutput = useExecutionStore((s) => s.streamingOutput);
  const analysisCoverage = useExecutionStore((s) => s.analysisCoverage);
  const logs = useExecutionStore((s) => s.logs);
  const history = useExecutionStore((s) => s.history);
  const clearHistory = useExecutionStore((s) => s.clearHistory);
  const deleteHistory = useExecutionStore((s) => s.deleteHistory);
  const renameHistory = useExecutionStore((s) => s.renameHistory);
  const restoreFromHistory = useExecutionStore((s) => s.restoreFromHistory);
  const sessionUsageTotals = useExecutionStore((s) => s.sessionUsageTotals);
  const turnUsageTotals = useExecutionStore((s) => s.turnUsageTotals);
  const taskId = useExecutionStore((s) => s.taskId);
  const standaloneSessionId = useExecutionStore((s) => s.standaloneSessionId);
  const attachments = useExecutionStore((s) => s.attachments);
  const addAttachment = useExecutionStore((s) => s.addAttachment);
  const removeAttachment = useExecutionStore((s) => s.removeAttachment);
  const clearAttachments = useExecutionStore((s) => s.clearAttachments);
  const backgroundSessions = useExecutionStore((s) => s.backgroundSessions);
  const switchToSession = useExecutionStore((s) => s.switchToSession);
  const removeBackgroundSession = useExecutionStore((s) => s.removeBackgroundSession);
  const foregroundParentSessionId = useExecutionStore((s) => s.foregroundParentSessionId);
  const foregroundBgId = useExecutionStore((s) => s.foregroundBgId);
  const activeAgentName = useExecutionStore((s) => s.activeAgentName);
  const backend = useSettingsStore((s) => s.backend);
  const provider = useSettingsStore((s) => s.provider);
  const model = useSettingsStore((s) => s.model);
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const sidebarCollapsed = useSettingsStore((s) => s.sidebarCollapsed);
  const setSidebarCollapsed = useSettingsStore((s) => s.setSidebarCollapsed);
  const autoPanelHoverEnabled = useSettingsStore((s) => s.autoPanelHoverEnabled);

  const [description, setDescription] = useState('');
  const [workflowMode, setWorkflowMode] = useState<WorkflowMode>('chat');
  const [tokenEstimate, setTokenEstimate] = useState<PromptTokenEstimateResult | null>(null);
  const [isEstimatingTokenBudget, setIsEstimatingTokenBudget] = useState(false);
  const [promptTokenBudget, setPromptTokenBudget] = useState(DEFAULT_PROMPT_TOKEN_BUDGET);

  // Ref for InputBox to call pickFile externally
  const inputBoxRef = useRef<InputBoxHandle>(null);
  // Ref for ChatTranscript scroll container (used for image export)
  const chatScrollRef = useRef<HTMLDivElement>(null);
  const leftHoverTimerRef = useRef<number | null>(null);
  const rightHoverTimerRef = useRef<number | null>(null);
  const rightPanelResizeRef = useRef<{ startX: number; startWidth: number } | null>(null);
  const {
    leftPanelHoverExpanded,
    rightPanelHoverExpanded,
    rightPanelOpen,
    rightPanelWidth,
    rightPanelTab,
    supportsPointerHover,
    setLeftPanelHoverExpanded,
    setRightPanelHoverExpanded,
    setRightPanelOpen,
    setRightPanelWidth,
    setRightPanelTab,
  } = useSimplePanelState(workspacePath);
  const { isCapturing, handleExportImage } = useSimpleExport({
    chatScrollRef,
    showToast,
    t,
  });

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

  const { clearPersistedWorkflowKernelSessionId } = useSimpleKernelSession({
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
  }, [setRightPanelHoverExpanded, setRightPanelOpen, setRightPanelTab]);

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

  // Plan mode orchestrator
  const startPlanWorkflow = usePlanOrchestratorStore((s) => s.startPlanWorkflow);
  const submitPlanClarification = usePlanOrchestratorStore((s) => s.submitClarification);
  const skipPlanClarification = usePlanOrchestratorStore((s) => s.skipClarification);
  const cancelPlanWorkflow = usePlanOrchestratorStore((s) => s.cancelWorkflow);
  const ensurePlanTerminalCompletionCard = usePlanOrchestratorStore((s) => s.ensureTerminalCompletionCardFromKernel);
  const planWorkflowCancelling = usePlanOrchestratorStore((s) => s.isCancelling);
  const resetPlanWorkflow = usePlanOrchestratorStore((s) => s.resetWorkflow);
  const kernelChatRuntime = useMemo(() => selectKernelChatRuntime(workflowKernelSession), [workflowKernelSession]);
  const kernelTaskRuntime = useMemo(() => selectKernelTaskRuntime(workflowKernelSession), [workflowKernelSession]);
  const kernelPlanRuntime = useMemo(() => selectKernelPlanRuntime(workflowKernelSession), [workflowKernelSession]);
  const kernelRuntimeStatus = useMemo(() => selectKernelRuntimeStatus(workflowKernelSession), [workflowKernelSession]);
  const workflowKernelTaskPhase = kernelTaskRuntime.phase;
  const workflowKernelPlanPhase = kernelPlanRuntime.phase;
  const workflowKernelChatPhase = kernelChatRuntime.phase;
  const workflowKernelPendingInterview = kernelTaskRuntime.pendingInterview;
  const workflowKernelPendingClarification = kernelPlanRuntime.pendingClarification;

  const { taskPendingQuestion, planPendingQuestion } = useWorkflowQuestionSpecs(
    workflowKernelPendingInterview,
    workflowKernelPendingClarification,
  );
  const workflowPhase = workflowKernelTaskPhase;
  const planPhase = workflowKernelPlanPhase;
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
  const hasStructuredPlanClarifyQuestion = planClarifyingPhase && !!planPendingQuestion;
  const hasPlanClarifyQuestion = planClarifyingPhase && !!planPendingQuestion;
  const isTaskWorkflowActiveForSwitchGuard = workflowMode === 'task' && kernelRuntimeStatus.isTaskActive;
  const isPlanWorkflowActiveForSwitchGuard = workflowMode === 'plan' && kernelRuntimeStatus.isPlanActive;
  const effectiveTaskPhaseForInput = taskInterviewingPhase ? 'interviewing' : workflowPhase;
  const effectivePlanPhaseForInput = planClarifyingPhase ? 'clarifying' : planPhase;
  const isInterviewSubmitting = taskInterviewingPhase && taskPendingQuestion === null;
  const recordInteractiveActionFailure = useWorkflowObservabilityStore((s) => s.recordInteractiveActionFailure);

  useEffect(() => {
    if (!workflowKernelSession) return;
    if (
      markUnknownPhaseForReporting('task', workflowPhase) &&
      isWorkflowModeActive({
        mode: 'task',
        currentMode: workflowKernelSession.activeMode,
        isKernelSessionActive: workflowKernelSession.status === 'active',
        phase: workflowPhase,
      })
    ) {
      void recordInteractiveActionFailure({
        card: 'workflow_phase',
        action: 'unknown_phase_detected',
        errorCode: 'unknown_task_phase',
        message: `Unknown task phase: ${workflowPhase}`,
        mode: 'task',
        kernelSessionId: workflowKernelSession.sessionId,
        modeSessionId: kernelTaskRuntime.linkedSessionId,
        phaseBefore: workflowPhase,
        phaseAfter: workflowPhase,
      });
    }
    if (
      markUnknownPhaseForReporting('plan', planPhase) &&
      isWorkflowModeActive({
        mode: 'plan',
        currentMode: workflowKernelSession.activeMode,
        isKernelSessionActive: workflowKernelSession.status === 'active',
        phase: planPhase,
      })
    ) {
      void recordInteractiveActionFailure({
        card: 'workflow_phase',
        action: 'unknown_phase_detected',
        errorCode: 'unknown_plan_phase',
        message: `Unknown plan phase: ${planPhase}`,
        mode: 'plan',
        kernelSessionId: workflowKernelSession.sessionId,
        modeSessionId: kernelPlanRuntime.linkedSessionId,
        phaseBefore: planPhase,
        phaseAfter: planPhase,
      });
    }
  }, [
    workflowKernelSession,
    workflowPhase,
    planPhase,
    kernelTaskRuntime.linkedSessionId,
    kernelPlanRuntime.linkedSessionId,
    recordInteractiveActionFailure,
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
    handleStructuredPlanClarifySubmit,
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
    hasStructuredPlanClarifyQuestion,
    linkWorkflowKernelModeSession,
    cancelWorkflowKernelOperation,
    transitionAndSubmitWorkflowKernelInput,
  });

  const queueSessionId = workflowKernelSession?.sessionId ?? workflowKernelSessionId;
  const switchWorkflowModeForQueue = useCallback(
    async (targetMode: WorkflowMode): Promise<boolean> => {
      if (targetMode === workflowMode) return true;
      const conversationContext = buildConversationHistory().map((turn) => ({
        user: turn.user,
        assistant: turn.assistant,
      }));
      const transitioned = await switchModeSafely({
        targetMode,
        handoff: {
          conversationContext,
          artifactRefs: [],
          contextSources: ['simple_mode'],
          metadata: {
            source: 'queued_message_dispatch',
            sourceMode: workflowMode,
            targetMode,
          },
        },
        transitionWorkflowKernelMode,
      });
      if (!transitioned) return false;
      setWorkflowMode(targetMode);
      return true;
    },
    [workflowMode, transitionWorkflowKernelMode],
  );

  const {
    queuedChatMessages,
    queueChatMessage,
    removeQueuedChatMessage,
    clearQueuedChatMessages,
    moveQueuedChatMessage,
    setQueuedChatMessagePriority,
    retryQueuedChatMessage,
  } = useSimpleQueueRuntime({
    workspacePath,
    sessionId: queueSessionId ?? '',
    workflowMode,
    maxQueuedChatMessages: MAX_QUEUED_CHAT_MESSAGES,
    isRunning,
    isSubmitting,
    isAnalyzingStrategy,
    permissionRequest,
    isTaskWorkflowBusy:
      workflowMode === 'task' && kernelRuntimeStatus.isTaskActive && isTaskPhaseBusy(effectiveTaskPhaseForInput),
    isPlanWorkflowBusy:
      workflowMode === 'plan' && kernelRuntimeStatus.isPlanActive && isPlanPhaseBusy(effectivePlanPhaseForInput),
    attachments,
    addAttachment,
    clearAttachments,
    handleFollowUp,
    handleStart,
    switchWorkflowModeForQueue,
    showToast,
    t,
  });

  const {
    modeSwitchConfirmOpen,
    modeSwitchBlockReason,
    handleWorkflowModeChange,
    handleConfirmModeSwitch,
    handleModeSwitchDialogOpenChange,
  } = useSimpleModeSwitch({
    workflowMode,
    isRunning,
    workflowPhase,
    planPhase,
    isTaskWorkflowActive: isTaskWorkflowActiveForSwitchGuard,
    isPlanWorkflowActive: isPlanWorkflowActiveForSwitchGuard,
    hasStructuredInterviewQuestion,
    hasPlanClarifyQuestion,
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
      !hasStructuredPlanClarifyQuestion &&
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
    hasStructuredPlanClarifyQuestion,
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
      setRightPanelOpen,
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
    try {
      await cancelActiveWorkflow({
        workflowMode,
        taskWorkflowCancelling,
        planWorkflowCancelling,
        isTaskExecuting: workflowKernelTaskPhase === 'executing',
        isPlanExecuting: workflowKernelPlanPhase === 'executing',
        cancelKernelOperation: cancelWorkflowKernelOperation,
        cancelTaskWorkflow: cancelWorkflow,
        cancelPlanWorkflow: cancelPlanWorkflow,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      showToast(message || t('workflow.cancelFailed', { defaultValue: 'Cancel failed' }), 'error');
    }
  }, [
    cancelWorkflowKernelOperation,
    workflowMode,
    cancelPlanWorkflow,
    cancelWorkflow,
    taskWorkflowCancelling,
    planWorkflowCancelling,
    workflowKernelTaskPhase,
    workflowKernelPlanPhase,
    showToast,
    t,
  ]);

  const kernelStatus = workflowKernelSession?.status ?? 'active';
  const kernelSessionMode = workflowKernelSession?.activeMode ?? workflowMode;
  const hasActiveKernelSession = kernelStatus === 'active';
  const isTaskWorkflowActive = isWorkflowModeActive({
    mode: 'task',
    currentMode: kernelSessionMode,
    isKernelSessionActive: hasActiveKernelSession,
    phase: workflowPhase,
  });
  const isPlanWorkflowActive = isWorkflowModeActive({
    mode: 'plan',
    currentMode: kernelSessionMode,
    isKernelSessionActive: hasActiveKernelSession,
    phase: planPhase,
  });
  const isTaskWorkflowBusy =
    workflowMode === 'task' && isTaskWorkflowActive && isTaskPhaseBusy(effectiveTaskPhaseForInput);
  const isPlanWorkflowBusy =
    workflowMode === 'plan' && isPlanWorkflowActive && isPlanPhaseBusy(effectivePlanPhaseForInput);
  const isStructuredWorkflowCancelling =
    (workflowMode === 'task' && taskWorkflowCancelling) || (workflowMode === 'plan' && planWorkflowCancelling);
  const canQueueWhileRunning =
    !executionIsCancelling &&
    !isAnalyzingStrategy &&
    !isStructuredWorkflowCancelling &&
    !hasStructuredInterviewQuestion &&
    !hasStructuredPlanClarifyQuestion &&
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
    hasStructuredPlanClarifyQuestion ||
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
  }, [hoverPanelsEnabled, sidebarCollapsed, clearLeftHoverTimer, setLeftPanelHoverExpanded]);

  const scheduleCloseLeftHoverPanel = useCallback(() => {
    if (!hoverPanelsEnabled || !sidebarCollapsed) return;
    clearLeftHoverTimer();
    leftHoverTimerRef.current = window.setTimeout(() => {
      setLeftPanelHoverExpanded(false);
      leftHoverTimerRef.current = null;
    }, 180);
  }, [hoverPanelsEnabled, sidebarCollapsed, clearLeftHoverTimer, setLeftPanelHoverExpanded]);

  const openRightHoverPanel = useCallback(() => {
    if (!hoverPanelsEnabled || rightPanelOpen) return;
    clearRightHoverTimer();
    setRightPanelHoverExpanded(true);
  }, [hoverPanelsEnabled, rightPanelOpen, clearRightHoverTimer, setRightPanelHoverExpanded]);

  const scheduleCloseRightHoverPanel = useCallback(() => {
    if (!hoverPanelsEnabled || rightPanelOpen) return;
    clearRightHoverTimer();
    rightHoverTimerRef.current = window.setTimeout(() => {
      setRightPanelHoverExpanded(false);
      rightHoverTimerRef.current = null;
    }, 180);
  }, [hoverPanelsEnabled, rightPanelOpen, clearRightHoverTimer, setRightPanelHoverExpanded]);

  useEffect(() => {
    if (workflowMode !== 'plan') return;
    if (planPhase !== 'completed' && planPhase !== 'failed' && planPhase !== 'cancelled') return;
    void ensurePlanTerminalCompletionCard();
  }, [workflowMode, planPhase, ensurePlanTerminalCompletionCard]);

  useEffect(() => {
    if (hoverPanelsEnabled) return;
    clearLeftHoverTimer();
    clearRightHoverTimer();
    setLeftPanelHoverExpanded(false);
    setRightPanelHoverExpanded(false);
  }, [
    hoverPanelsEnabled,
    clearLeftHoverTimer,
    clearRightHoverTimer,
    setLeftPanelHoverExpanded,
    setRightPanelHoverExpanded,
  ]);

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
  }, [
    isRightPanelOpen,
    rightPanelOpen,
    rightPanelTab,
    setRightPanelHoverExpanded,
    setRightPanelOpen,
    setRightPanelTab,
  ]);

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
    [clampRightPanelWidth, isRightPanelOpen, rightPanelWidth, setRightPanelWidth],
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
              <ChatTranscript
                lines={streamingOutput}
                status={status}
                scrollRef={chatScrollRef}
                forceFullRender={isCapturing}
              />
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
                hasStructuredPlanClarifyQuestion={hasStructuredPlanClarifyQuestion}
                hasTextInterviewQuestion={hasTextInterviewQuestion}
                taskPendingQuestion={taskPendingQuestion}
                planPendingQuestion={planPendingQuestion}
                onStructuredInterviewSubmit={handleStructuredInterviewSubmit}
                onStructuredPlanClarifySubmit={handleStructuredPlanClarifySubmit}
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
                onMoveQueuedChatMessage={moveQueuedChatMessage}
                onSetQueuedChatMessagePriority={setQueuedChatMessagePriority}
                onRetryQueuedChatMessage={retryQueuedChatMessage}
                onClearQueuedChatMessages={clearQueuedChatMessages}
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
