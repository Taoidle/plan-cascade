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

import { Fragment, useEffect, useMemo, useRef, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { MarkdownRenderer } from '../ClaudeCodeMode/MarkdownRenderer';
import { Collapsible } from './Collapsible';
import { InputBox, type InputBoxHandle } from './InputBox';
import { MessageActions, EditMode } from './MessageActions';
import { WorkspaceTreeSidebar } from './WorkspaceTreeSidebar';
import { EdgeCollapseButton } from './EdgeCollapseButton';
import { BottomStatusBar } from './BottomStatusBar';
import { ChatToolbar } from './ChatToolbar';
import { TabbedRightPanel, type RightPanelTab } from './TabbedRightPanel';
import { useExecutionStore, type StreamLine } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';
import {
  buildDisplayBlocks,
  ToolCallLine,
  SubAgentLine,
  AnalysisLine,
  ToolResultLine,
  SubAgentGroupPanel,
  EventGroupLine,
} from '../shared/StreamingOutput';
import { useWorkflowOrchestratorStore } from '../../store/workflowOrchestrator';
import { usePlanOrchestratorStore } from '../../store/planOrchestrator';
import { WorkflowCardRenderer } from './WorkflowCards/WorkflowCardRenderer';
import { InterviewInputPanel } from './InterviewInputPanel';
import { ToolPermissionOverlay } from './ToolPermissionOverlay';
import { useToolPermissionStore } from '../../store/toolPermission';
import { useAgentsStore } from '../../store/agents';
import { createFileChangeCardBridge } from '../../lib/fileChangeCardBridge';
import {
  captureElementToBlob,
  blobToBase64,
  saveBinaryWithDialog,
  localTimestampForFilename,
} from '../../lib/exportUtils';
import type { CardPayload } from '../../types/workflowCard';
import { useToast } from '../shared/Toast';

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

  const [description, setDescription] = useState('');
  const [rightPanelOpen, setRightPanelOpen] = useState(false);
  const [rightPanelTab, setRightPanelTab] = useState<RightPanelTab>('output');
  const [workflowMode, setWorkflowMode] = useState<WorkflowMode>('chat');

  // Ref for InputBox to call pickFile externally
  const inputBoxRef = useRef<InputBoxHandle>(null);
  // Ref for ChatTranscript scroll container (used for image export)
  const chatScrollRef = useRef<HTMLDivElement>(null);
  const [isCapturing, setIsCapturing] = useState(false);

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

  const prevPathRef = useRef(workspacePath);
  useEffect(() => {
    if (prevPathRef.current !== workspacePath && isChatSession) {
      reset();
      clearStrategyAnalysis();
      setDescription('');
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

    if (workflowMode === 'task') {
      // Route Task mode through the workflow orchestrator
      await startWorkflow(description);
      setDescription('');
      return;
    }

    if (workflowMode === 'plan') {
      // Route Plan mode through the plan orchestrator
      await startPlanWorkflow(description);
      setDescription('');
      return;
    }

    await start(description, 'simple');
    setDescription('');
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
      restoreFromHistory(historyId);
      setRightPanelOpen(false);
      handleWorkflowModeChange('chat');
    },
    [restoreFromHistory, handleWorkflowModeChange],
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
  const isDisabled = isRunning || isSubmitting || isAnalyzingStrategy;

  const detailLineCount = useMemo(
    () => streamingOutput.filter((line) => line.type !== 'text' && line.type !== 'info').length,
    [streamingOutput],
  );

  // Output button toggle logic
  const handleToggleOutput = useCallback(() => {
    if (!rightPanelOpen) {
      setRightPanelOpen(true);
      setRightPanelTab('output');
    } else if (rightPanelTab === 'output') {
      setRightPanelOpen(false);
    } else {
      setRightPanelTab('output');
    }
  }, [rightPanelOpen, rightPanelTab]);

  return (
    <div className="h-full flex flex-col">
      {/* Main content area */}
      <div className="flex-1 min-h-0 px-4 py-2">
        <div className="h-full max-w-[2200px] mx-auto w-full flex">
          {/* Left panel: WorkspaceTreeSidebar */}
          <div
            className={clsx(
              'shrink-0 transition-all duration-200 ease-out overflow-hidden',
              sidebarCollapsed ? 'w-0 opacity-0' : 'w-[280px] opacity-100 mr-3',
            )}
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
                onSwitchSession={switchToSession}
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
              expanded={!sidebarCollapsed}
              onToggle={() => setSidebarCollapsed(!sidebarCollapsed)}
            />
            <EdgeCollapseButton
              side="right"
              expanded={rightPanelOpen}
              onToggle={() => setRightPanelOpen(!rightPanelOpen)}
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
              isFilePickDisabled={isDisabled}
              executionStatus={status}
              onPause={pause}
              onResume={resume}
              onCancel={cancel}
              taskWorkflowActive={
                workflowMode === 'task' &&
                workflowPhase !== 'idle' &&
                workflowPhase !== 'completed' &&
                workflowPhase !== 'failed' &&
                workflowPhase !== 'cancelled'
              }
              planWorkflowActive={
                workflowMode === 'plan' &&
                planPhase !== 'idle' &&
                planPhase !== 'completed' &&
                planPhase !== 'failed' &&
                planPhase !== 'cancelled'
              }
              onCancelWorkflow={workflowMode === 'plan' ? cancelPlanWorkflow : cancelWorkflow}
              onExportImage={handleExportImage}
              isExportDisabled={streamingOutput.length === 0}
              isCapturing={isCapturing}
              rightPanelOpen={rightPanelOpen}
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
                      (workflowMode === 'task' && workflowPhase !== 'idle') ||
                      (workflowMode === 'plan' && planPhase !== 'idle')
                        ? handleFollowUp
                        : handleStart
                    }
                    disabled={isDisabled}
                    enterSubmits={false}
                    placeholder={
                      isRunning
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
                    isLoading={isRunning}
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
              rightPanelOpen ? 'w-[520px] opacity-100 ml-3' : 'w-0 opacity-0',
            )}
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

interface RichTurn {
  turnIndex: number;
  userLine: StreamLine;
  assistantLines: StreamLine[];
}

function ChatTranscript({
  lines,
  status,
  scrollRef,
}: {
  lines: StreamLine[];
  status: 'idle' | 'running' | 'paused' | 'completed' | 'failed';
  scrollRef?: React.RefObject<HTMLDivElement | null>;
}) {
  const { t } = useTranslation('simpleMode');
  const containerRef = useRef<HTMLDivElement>(null);

  // Sync containerRef to the external scrollRef so the parent can access it
  const setRef = useCallback(
    (node: HTMLDivElement | null) => {
      (containerRef as React.MutableRefObject<HTMLDivElement | null>).current = node;
      if (scrollRef) {
        (scrollRef as React.MutableRefObject<HTMLDivElement | null>).current = node;
      }
    },
    [scrollRef],
  );
  const [editingLineId, setEditingLineId] = useState<number | null>(null);

  // Derive rich conversation turns from ALL lines (not just text)
  const richTurns = useMemo((): RichTurn[] => {
    const result: RichTurn[] = [];
    let turnIndex = 0;
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].type !== 'info') continue;

      let endIndex = lines.length - 1;
      for (let j = i + 1; j < lines.length; j++) {
        if (lines[j].type === 'info') {
          endIndex = j - 1;
          break;
        }
      }

      const assistantLines: StreamLine[] = [];
      for (let j = i + 1; j <= endIndex; j++) {
        assistantLines.push(lines[j]);
      }

      result.push({ turnIndex: turnIndex++, userLine: lines[i], assistantLines });
    }
    // Fallback: if no info lines but content exists, synthesize a turn so the panel isn't empty
    if (result.length === 0 && lines.length > 0 && lines.some((l) => l.type !== 'info')) {
      const syntheticUserLine: StreamLine = {
        id: -1,
        content: '(continued)',
        type: 'info',
        timestamp: lines[0].timestamp,
      };
      result.push({
        turnIndex: 0,
        userLine: syntheticUserLine,
        assistantLines: lines.filter((l) => l.type !== 'info'),
      });
    }
    return result;
  }, [lines]);

  const backend = useSettingsStore((s) => s.backend);
  const isClaudeCodeBackendValue = backend === 'claude-code';
  const isActionsDisabled = status === 'running' || status === 'paused';
  const lastTurnIndex = richTurns.length > 0 ? richTurns.length - 1 : -1;

  // Clear editing state when lines change
  useEffect(() => {
    if (editingLineId !== null) {
      const lineStillExists = lines.some((l) => l.id === editingLineId);
      if (!lineStillExists) setEditingLineId(null);
    }
  }, [lines, editingLineId]);

  // Action handlers
  const handleEdit = useCallback((lineId: number, newContent: string) => {
    setEditingLineId(null);
    useExecutionStore.getState().editAndResend(lineId, newContent);
  }, []);

  const handleEditStart = useCallback((lineId: number) => {
    setEditingLineId(lineId);
  }, []);

  const handleEditCancel = useCallback(() => {
    setEditingLineId(null);
  }, []);

  const handleCopy = useCallback((content: string) => {
    navigator.clipboard.writeText(content).catch(() => {});
  }, []);

  // Sticky-bottom auto-scroll: only scroll if user is near the bottom
  const isNearBottom = useRef(true);
  const [showScrollBtn, setShowScrollBtn] = useState(false);

  const handleScroll = useCallback(() => {
    if (!containerRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = containerRef.current;
    const nearBottom = scrollHeight - scrollTop - clientHeight < 50;
    isNearBottom.current = nearBottom;
    setShowScrollBtn(!nearBottom);
  }, []);

  const scrollToBottom = useCallback(() => {
    containerRef.current?.scrollTo({ top: containerRef.current.scrollHeight, behavior: 'smooth' });
  }, []);

  useEffect(() => {
    if (!containerRef.current || !isNearBottom.current) return;
    containerRef.current.scrollTop = containerRef.current.scrollHeight;
  }, [lines]);

  const hasContent = lines.length > 0 && lines.some((l) => l.type !== 'info');
  if (richTurns.length === 0 && !hasContent) {
    return (
      <div className="h-full flex items-center justify-center text-sm text-gray-500 dark:text-gray-400">
        {status === 'running'
          ? t('emptyChat.thinking', { defaultValue: 'Thinking...' })
          : t('emptyChat.startConversation', { defaultValue: 'Start a conversation on the right input box.' })}
      </div>
    );
  }

  return (
    <div className="relative h-full">
      <div ref={setRef} onScroll={handleScroll} className="h-full overflow-y-auto px-4 py-4 space-y-4">
        {richTurns.map((turn) => {
          const isLastTurn = turn.turnIndex === lastTurnIndex;

          return (
            <Fragment key={turn.userLine.id}>
              {/* User message bubble */}
              {editingLineId === turn.userLine.id ? (
                <div className="flex justify-end">
                  <EditMode
                    content={turn.userLine.content}
                    onSave={(newContent) => handleEdit(turn.userLine.id, newContent)}
                    onCancel={handleEditCancel}
                    isClaudeCodeBackend={isClaudeCodeBackendValue}
                  />
                </div>
              ) : (
                <div className="group relative flex justify-end">
                  <div className="max-w-[82%] px-4 py-2 rounded-2xl rounded-br-sm bg-primary-600 text-white text-sm whitespace-pre-wrap">
                    {turn.userLine.content}
                  </div>
                  <MessageActions
                    line={turn.userLine}
                    isUserMessage={true}
                    isLastTurn={isLastTurn}
                    isClaudeCodeBackend={isClaudeCodeBackendValue}
                    disabled={isActionsDisabled}
                    onEdit={handleEdit}
                    onRegenerate={() => useExecutionStore.getState().regenerateResponse(turn.userLine.id)}
                    onRollback={() => useExecutionStore.getState().rollbackToTurn(turn.userLine.id)}
                    onCopy={handleCopy}
                    onEditStart={handleEditStart}
                    onEditCancel={handleEditCancel}
                  />
                </div>
              )}

              {/* Assistant response section */}
              {turn.assistantLines.length > 0 ? (
                <ChatAssistantSection
                  lines={turn.assistantLines}
                  isLastTurn={isLastTurn}
                  userLineId={turn.userLine.id}
                  disabled={isActionsDisabled}
                  isClaudeCodeBackend={isClaudeCodeBackendValue}
                  onEdit={handleEdit}
                  onCopy={handleCopy}
                  onFork={() => useExecutionStore.getState().forkSessionAtTurn(turn.userLine.id)}
                />
              ) : status === 'running' && isLastTurn ? (
                <div className="flex justify-start">
                  <div className="px-4 py-2 rounded-2xl rounded-bl-sm bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 text-sm italic flex items-center gap-2">
                    <span className="w-1.5 h-1.5 rounded-full bg-primary-400 animate-pulse" />
                    {t('emptyChat.thinking', { defaultValue: 'Thinking...' })}
                  </div>
                </div>
              ) : null}
            </Fragment>
          );
        })}
      </div>

      {/* Scroll to bottom button */}
      {showScrollBtn && (
        <button
          onClick={scrollToBottom}
          className={clsx(
            'absolute bottom-4 left-1/2 -translate-x-1/2 z-10',
            'flex items-center justify-center',
            'w-8 h-8 rounded-full',
            'bg-white dark:bg-gray-800',
            'border border-gray-200 dark:border-gray-700',
            'shadow-md',
            'text-gray-500 dark:text-gray-400',
            'hover:bg-gray-50 dark:hover:bg-gray-700',
            'transition-colors',
            'animate-in fade-in-0 zoom-in-75 duration-150',
          )}
          title={t('chat.scrollToBottom', { defaultValue: 'Scroll to bottom' })}
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M4 6l4 4 4-4" />
          </svg>
        </button>
      )}
    </div>
  );
}

/** Assistant response section: renders rich content (text, tools, sub-agents, thinking) within a chat bubble */
function ChatAssistantSection({
  lines,
  isLastTurn,
  userLineId,
  disabled,
  isClaudeCodeBackend,
  onEdit,
  onCopy,
  onFork,
}: {
  lines: StreamLine[];
  isLastTurn: boolean;
  userLineId: number;
  disabled: boolean;
  isClaudeCodeBackend: boolean;
  onEdit: (lineId: number, newContent: string) => void;
  onCopy: (content: string) => void;
  onFork: (userLineId: number) => void;
}) {
  const showReasoning = useSettingsStore((s) => s.showReasoningOutput);

  // Separate thinking from other lines
  const thinkingLines = useMemo(() => lines.filter((l) => l.type === 'thinking'), [lines]);
  const contentLines = useMemo(() => lines.filter((l) => l.type !== 'thinking'), [lines]);

  // Build display blocks for content (always grouped, like compact mode)
  const blocks = useMemo(() => buildDisplayBlocks(contentLines, true), [contentLines]);

  // Collect text content for copy
  const textContent = useMemo(
    () =>
      lines
        .filter((l) => l.type === 'text')
        .map((l) => l.content)
        .join(''),
    [lines],
  );

  // Find last text line for MessageActions
  const lastTextLine = useMemo(() => [...lines].reverse().find((l) => l.type === 'text'), [lines]);

  // Check if there's rich content (tools, sub-agents, etc.)
  const hasRichContent = contentLines.some(
    (l) =>
      l.type === 'tool' || l.type === 'tool_result' || l.type === 'sub_agent' || l.type === 'analysis' || l.subAgentId,
  );

  return (
    <div className="group relative flex justify-start">
      <div
        className={clsx(
          'max-w-[88%] rounded-2xl rounded-bl-sm bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100',
          hasRichContent ? 'px-3 py-2 space-y-2' : 'px-4 py-2',
        )}
      >
        {/* Thinking section (collapsed by default) */}
        {showReasoning && thinkingLines.length > 0 && <ChatThinkingSection lines={thinkingLines} />}

        {/* Content blocks */}
        {blocks.map((block, idx) => {
          if (block.kind === 'sub_agent_group') {
            return (
              <SubAgentGroupPanel
                key={`sa-${block.subAgentId}-${block.lines[0]?.id}`}
                subAgentId={block.subAgentId}
                lines={block.lines}
                depth={block.depth}
                compact
              />
            );
          }
          if (block.kind === 'group') {
            return (
              <EventGroupLine
                key={block.groupId}
                groupId={block.groupId}
                kind={block.groupKind}
                lines={block.lines}
                compact
              />
            );
          }
          // Single line block
          const line = block.line;
          if (line.type === 'card') {
            try {
              const payload = JSON.parse(line.content) as CardPayload;
              return (
                <div key={line.id} className="my-1">
                  <WorkflowCardRenderer payload={payload} />
                </div>
              );
            } catch {
              return null;
            }
          }
          if (line.type === 'text') {
            return (
              <div key={line.id}>
                <MarkdownRenderer content={line.content} className="text-sm" />
              </div>
            );
          }
          if (line.type === 'tool') {
            return <ToolCallLine key={line.id} content={line.content} compact />;
          }
          if (line.type === 'tool_result') {
            return <ToolResultLine key={line.id} content={line.content} compact />;
          }
          if (line.type === 'sub_agent') {
            return <SubAgentLine key={line.id} content={line.content} compact />;
          }
          if (line.type === 'analysis') {
            return <AnalysisLine key={line.id} content={line.content} compact />;
          }
          // error, warning, success
          if (line.type === 'error' || line.type === 'warning' || line.type === 'success') {
            const toneClass =
              line.type === 'error'
                ? 'border-red-300 bg-red-50 text-red-700 dark:border-red-800 dark:bg-red-900/20 dark:text-red-300'
                : line.type === 'warning'
                  ? 'border-amber-300 bg-amber-50 text-amber-700 dark:border-amber-800 dark:bg-amber-900/20 dark:text-amber-300'
                  : 'border-green-300 bg-green-50 text-green-700 dark:border-green-800 dark:bg-green-900/20 dark:text-green-300';
            return (
              <div key={line.id} className={clsx('text-xs px-3 py-2 rounded border', toneClass)}>
                {line.content}
              </div>
            );
          }
          return <div key={`block-${idx}`} />;
        })}
      </div>

      {/* MessageActions on the assistant section */}
      {lastTextLine && (
        <MessageActions
          line={lastTextLine}
          isUserMessage={false}
          isLastTurn={isLastTurn}
          isClaudeCodeBackend={isClaudeCodeBackend}
          disabled={disabled}
          onEdit={onEdit}
          onRegenerate={() => useExecutionStore.getState().regenerateResponse(userLineId)}
          onRollback={() => useExecutionStore.getState().rollbackToTurn(userLineId)}
          onCopy={() => onCopy(textContent)}
          onFork={() => onFork(userLineId)}
        />
      )}
    </div>
  );
}

/** Collapsible thinking section for chat bubbles — collapsed by default */
function ChatThinkingSection({ lines }: { lines: StreamLine[] }) {
  const [expanded, setExpanded] = useState(false);
  const content = lines.map((l) => l.content).join('');

  if (!content.trim()) return null;

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-600 overflow-hidden">
      <button
        onClick={() => setExpanded((v) => !v)}
        className="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-gray-500 dark:text-gray-400 hover:bg-gray-200/50 dark:hover:bg-gray-700/50 transition-colors"
      >
        <svg
          className={clsx('w-3 h-3 transition-transform', expanded && 'rotate-90')}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
        </svg>
        <span className="italic">Thinking</span>
        <span className="text-2xs text-gray-400 dark:text-gray-500">({content.length} chars)</span>
      </button>
      <Collapsible open={expanded}>
        <div className="px-3 py-2 border-t border-gray-200 dark:border-gray-600 text-xs text-gray-500 dark:text-gray-400 italic font-mono whitespace-pre-wrap max-h-64 overflow-y-auto">
          {content}
        </div>
      </Collapsible>
    </div>
  );
}

/** Plan clarification input area — shown during plan clarifying phase */
function PlanClarifyInputArea({
  question,
  onSubmit,
  onSkip,
  onSkipAll,
  loading,
}: {
  question: { questionId: string; question: string; hint: string | null; inputType: string };
  onSubmit: (text: string) => void;
  onSkip: () => void;
  onSkipAll: () => void;
  loading: boolean;
}) {
  const { t } = useTranslation('planMode');
  const [value, setValue] = useState('');
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Auto-focus on question change
  useEffect(() => {
    inputRef.current?.focus();
    setValue('');
  }, [question.questionId]);

  const handleSubmit = useCallback(() => {
    if (!value.trim() || loading) return;
    onSubmit(value.trim());
    setValue('');
  }, [value, loading, onSubmit]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault();
        handleSubmit();
      }
    },
    [handleSubmit],
  );

  return (
    <div className="px-4 py-3 space-y-2">
      {/* Question display */}
      <div className="flex items-start gap-2">
        <span className="text-amber-500 dark:text-amber-400 mt-0.5">&#10067;</span>
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium text-amber-700 dark:text-amber-300">{question.question}</p>
          {question.hint && <p className="text-xs text-amber-500/80 dark:text-amber-400/70 mt-0.5">{question.hint}</p>}
        </div>
      </div>

      {/* Input + buttons */}
      <div className="flex items-end gap-2">
        <textarea
          ref={inputRef}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={loading}
          rows={question.inputType === 'textarea' ? 3 : 1}
          className="flex-1 min-w-0 resize-none rounded-lg border border-amber-300 dark:border-amber-700 bg-white dark:bg-gray-800 px-3 py-2 text-sm text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-amber-400 dark:focus:ring-amber-600 disabled:opacity-50"
          placeholder={t('clarify.inputPlaceholder', { defaultValue: 'Type your answer...' }) as string}
        />
        <button
          onClick={handleSubmit}
          disabled={!value.trim() || loading}
          className="shrink-0 px-3 py-2 rounded-lg bg-amber-500 hover:bg-amber-600 disabled:bg-gray-300 dark:disabled:bg-gray-700 text-white text-sm font-medium transition-colors disabled:cursor-not-allowed"
        >
          {t('clarify.submit', { defaultValue: 'Submit' })}
        </button>
        <button
          onClick={onSkip}
          disabled={loading}
          className="shrink-0 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm transition-colors disabled:opacity-50"
        >
          {t('clarify.skipQuestion', { defaultValue: 'Skip' })}
        </button>
        <button
          onClick={onSkipAll}
          disabled={loading}
          className="shrink-0 px-3 py-2 rounded-lg border border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-700 text-sm transition-colors disabled:opacity-50"
        >
          {t('clarify.skipAll', { defaultValue: 'Skip All' })}
        </button>
      </div>
    </div>
  );
}

export default SimpleMode;
