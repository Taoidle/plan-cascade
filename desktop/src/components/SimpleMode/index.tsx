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
import { ChatTranscript } from './ChatTranscript';
import { PlanClarifyInputArea } from './PlanClarifyInputArea';

type WorkflowMode = 'chat' | 'plan' | 'task';

export function SimpleMode() {
  const { t } = useTranslation('simpleMode');
  const { showToast } = useToast();
  const {
    status,
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

  // Ref for InputBox to call pickFile externally
  const inputBoxRef = useRef<InputBoxHandle>(null);
  // Ref for ChatTranscript scroll container (used for image export)
  const chatScrollRef = useRef<HTMLDivElement>(null);
  const [isCapturing, setIsCapturing] = useState(false);
  const leftHoverTimerRef = useRef<number | null>(null);
  const rightHoverTimerRef = useRef<number | null>(null);

  // Handle workflow mode changes with context inheritance notifications
  const handleWorkflowModeChange = useCallback(
    (newMode: WorkflowMode) => {
      if (newMode === workflowMode) return;

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

      setWorkflowMode(newMode);
    },
    [workflowMode, streamingOutput, showToast, t],
  );

  useEffect(() => {
    initialize();
    return () => {
      cleanup();
    };
  }, [initialize, cleanup]);

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
  const taskId = useExecutionStore((s) => s.taskId);
  const standaloneSessionId = useExecutionStore((s) => s.standaloneSessionId);
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

  const workflowPhase = useWorkflowOrchestratorStore((s) => s.phase);
  const pendingQuestion = useWorkflowOrchestratorStore((s) => s.pendingQuestion);
  const startWorkflow = useWorkflowOrchestratorStore((s) => s.startWorkflow);
  const submitInterviewAnswer = useWorkflowOrchestratorStore((s) => s.submitInterviewAnswer);
  const skipInterviewQuestion = useWorkflowOrchestratorStore((s) => s.skipInterviewQuestion);
  const overrideConfigNatural = useWorkflowOrchestratorStore((s) => s.overrideConfigNatural);
  const addPrdFeedback = useWorkflowOrchestratorStore((s) => s.addPrdFeedback);
  const cancelWorkflow = useWorkflowOrchestratorStore((s) => s.cancelWorkflow);
  const resetWorkflow = useWorkflowOrchestratorStore((s) => s.resetWorkflow);
  const isInterviewSubmitting =
    useWorkflowOrchestratorStore((s) => s.phase === 'interviewing') && pendingQuestion === null;

  // Plan mode orchestrator
  const planPhase = usePlanOrchestratorStore((s) => s.phase);
  const pendingClarifyQuestion = usePlanOrchestratorStore((s) => s.pendingClarifyQuestion);
  const planIsBusy = usePlanOrchestratorStore((s) => s.isBusy);
  const startPlanWorkflow = usePlanOrchestratorStore((s) => s.startPlanWorkflow);
  const submitPlanClarification = usePlanOrchestratorStore((s) => s.submitClarification);
  const skipPlanClarification = usePlanOrchestratorStore((s) => s.skipClarification);
  const cancelPlanWorkflow = usePlanOrchestratorStore((s) => s.cancelWorkflow);
  const resetPlanWorkflow = usePlanOrchestratorStore((s) => s.resetWorkflow);

  // Tool permission state
  const permissionRequest = useToolPermissionStore((s) => s.pendingRequest);
  const permissionQueueSize = useToolPermissionStore((s) => s.requestQueue.length);
  const isPermissionResponding = useToolPermissionStore((s) => s.isResponding);
  const respondPermission = useToolPermissionStore((s) => s.respond);
  const permissionLevel = useToolPermissionStore((s) => s.sessionLevel);
  const setPermissionLevel = useToolPermissionStore((s) => s.setSessionLevel);

  const handleStart = useCallback(async () => {
    if (!description.trim() || isSubmitting || isAnalyzingStrategy) return;
    const prompt = description;
    setDescription('');

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
  }, [description, isAnalyzingStrategy, isSubmitting, start, startWorkflow, startPlanWorkflow, workflowMode]);

  const handleFollowUp = useCallback(async () => {
    if (!description.trim() || isSubmitting) return;
    const prompt = description;
    setDescription('');

    // Route through orchestrator if in active Task workflow phase
    // Note: interviewing phase is handled by InterviewInputPanel, not InputBox
    if (workflowMode === 'task' && workflowPhase !== 'idle') {
      if (workflowPhase === 'configuring') {
        overrideConfigNatural(prompt);
      } else if (workflowPhase === 'reviewing_prd') {
        addPrdFeedback(prompt);
      }
      return;
    }

    // Route plan clarification through plan orchestrator
    if (workflowMode === 'plan' && planPhase === 'clarifying' && pendingClarifyQuestion) {
      await submitPlanClarification({
        questionId: pendingClarifyQuestion.questionId,
        answer: prompt,
        skipped: false,
      });
      return;
    }

    await sendFollowUp(prompt);
  }, [
    description,
    isSubmitting,
    sendFollowUp,
    workflowMode,
    workflowPhase,
    planPhase,
    pendingClarifyQuestion,
    overrideConfigNatural,
    addPrdFeedback,
    submitPlanClarification,
  ]);

  const handleNewTask = useCallback(() => {
    const hasContext = streamingOutput.length > 0 || useExecutionStore.getState()._pendingTaskContext;

    resetWorkflow();
    resetPlanWorkflow();
    reset();
    clearStrategyAnalysis();
    setDescription('');

    if (hasContext) {
      showToast(t('contextBridge.contextReset', { defaultValue: 'Context cleared for new task' }), 'info');
    }
  }, [reset, clearStrategyAnalysis, resetWorkflow, resetPlanWorkflow, streamingOutput, showToast, t]);

  const handleRestoreHistory = useCallback(
    (historyId: string) => {
      resetWorkflow();
      resetPlanWorkflow();
      restoreFromHistory(historyId);
      setRightPanelOpen(false);
      handleWorkflowModeChange('chat');
      setDescription('');
    },
    [restoreFromHistory, handleWorkflowModeChange, resetWorkflow, resetPlanWorkflow],
  );

  const handleSwitchSession = useCallback(
    (sessionId: string) => {
      // Keep workflow/orchestrator state scoped to the foreground session.
      resetWorkflow();
      resetPlanWorkflow();
      switchToSession(sessionId);
      setWorkflowMode('chat');
      setDescription('');
    },
    [resetWorkflow, resetPlanWorkflow, switchToSession],
  );

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

  const isRunning = status === 'running' || status === 'paused';
  const isTaskWorkflowActive =
    workflowPhase !== 'idle' &&
    workflowPhase !== 'completed' &&
    workflowPhase !== 'failed' &&
    workflowPhase !== 'cancelled';
  const isPlanWorkflowActive =
    planPhase !== 'idle' && planPhase !== 'completed' && planPhase !== 'failed' && planPhase !== 'cancelled';
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
  const inputBusy = isRunning || isSubmitting || isAnalyzingStrategy || isTaskWorkflowBusy || isPlanWorkflowBusy;
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
              isFilePickDisabled={inputBusy}
              executionStatus={status}
              onPause={pause}
              onResume={resume}
              onCancel={cancel}
              taskWorkflowActive={workflowMode === 'task' && isTaskWorkflowActive}
              planWorkflowActive={workflowMode === 'plan' && isPlanWorkflowActive}
              onCancelWorkflow={workflowMode === 'plan' ? cancelPlanWorkflow : cancelWorkflow}
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
              ) : /* Priority 2: Interview input panel (replaces InputBox during interviews) */
              workflowMode === 'task' && workflowPhase === 'interviewing' && pendingQuestion ? (
                <InterviewInputPanel
                  question={pendingQuestion}
                  onSubmit={submitInterviewAnswer}
                  onSkip={skipInterviewQuestion}
                  loading={isInterviewSubmitting}
                />
              ) : /* Priority 3: Interview loading state (LLM generating next question) */
              workflowMode === 'task' && workflowPhase === 'interviewing' && !pendingQuestion ? (
                <div className="px-4 py-3 flex items-center gap-2 text-sm text-violet-600 dark:text-violet-400">
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
              ) : /* Priority 4: Plan clarification input (with question + hint) */
              workflowMode === 'plan' && planPhase === 'clarifying' && pendingClarifyQuestion ? (
                <PlanClarifyInputArea
                  question={pendingClarifyQuestion}
                  onSubmit={(text) =>
                    submitPlanClarification({
                      questionId: pendingClarifyQuestion.questionId,
                      answer: text,
                      skipped: false,
                    })
                  }
                  onSkip={() =>
                    submitPlanClarification({
                      questionId: pendingClarifyQuestion.questionId,
                      answer: '',
                      skipped: true,
                    })
                  }
                  onSkipAll={skipPlanClarification}
                  loading={planIsBusy}
                />
              ) : /* Priority 5: Plan clarification loading state */
              workflowMode === 'plan' && planPhase === 'clarifying' && !pendingClarifyQuestion ? (
                <div className="px-4 py-3 flex items-center gap-2 text-sm text-amber-600 dark:text-amber-400">
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
                    {t('planMode:clarify.generatingQuestion', { defaultValue: 'Generating clarification question...' })}
                  </span>
                </div>
              ) : (
                <div className="p-4">
                  <InputBox
                    ref={inputBoxRef}
                    value={description}
                    onChange={setDescription}
                    onSubmit={
                      isChatSession ||
                      (workflowMode === 'task' && isTaskWorkflowActive) ||
                      (workflowMode === 'plan' && isPlanWorkflowActive)
                        ? handleFollowUp
                        : handleStart
                    }
                    disabled={inputBusy}
                    enterSubmits={false}
                    placeholder={
                      inputBusy
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
                            : workflowMode === 'task'
                              ? t('workflow.input.taskPlaceholder', {
                                  defaultValue: 'Describe a task (implementation / analysis / refactor)...',
                                })
                              : workflowMode === 'plan'
                                ? t('workflow.input.planPlaceholder', {
                                    defaultValue:
                                      'Describe a task to decompose and execute (writing, research, etc.)...',
                                  })
                                : t('input.followUpPlaceholder', {
                                    defaultValue: 'Type a normal chat message...',
                                  })
                    }
                    isLoading={inputBusy}
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
              isRightPanelOpen ? 'w-[520px] opacity-100 ml-3' : 'w-0 opacity-0',
            )}
            onMouseEnter={openRightHoverPanel}
            onMouseLeave={scheduleCloseRightHoverPanel}
          >
            <div className="w-[520px] h-full">
              <TabbedRightPanel
                activeTab={rightPanelTab}
                onTabChange={setRightPanelTab}
                workflowMode={workflowMode}
                workflowPhase={workflowPhase}
                logs={logs}
                analysisCoverage={analysisCoverage}
                streamingOutput={streamingOutput}
                workspacePath={workspacePath}
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
        onPermissionLevelChange={(level) => setPermissionLevel('current-session', level)}
        sessionId="current-session"
        turnUsage={turnUsageTotals}
        sessionUsage={sessionUsageTotals}
      />
    </div>
  );
}

export default SimpleMode;
