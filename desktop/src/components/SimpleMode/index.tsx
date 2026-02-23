/**
 * SimpleMode Component
 *
 * Three-panel layout:
 * - Left: conversation/session manager
 * - Middle: normal chat view
 * - Right (optional): detailed output/tool activity
 */

import { Fragment, useEffect, useMemo, useRef, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { MarkdownRenderer } from '../ClaudeCodeMode/MarkdownRenderer';
import { InputBox } from './InputBox';
import { ConnectionStatus } from './ConnectionStatus';
import { MessageActions, EditMode } from './MessageActions';
import { ModelSwitcher } from './ModelSwitcher';
import { WorkspaceTreeSidebar } from './WorkspaceTreeSidebar';
import { GitPanel } from './GitPanel';
import { useExecutionStore, type StreamLine } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';
import { StreamingOutput, ErrorState, ProjectSelector, IndexStatus } from '../shared';
import {
  buildDisplayBlocks,
  ToolCallLine,
  SubAgentLine,
  AnalysisLine,
  ToolResultLine,
  SubAgentGroupPanel,
  EventGroupLine,
} from '../shared/StreamingOutput';
import { ContextualActions } from '../shared/ContextualActions';
import { useWorkflowOrchestratorStore } from '../../store/workflowOrchestrator';
import { WorkflowCardRenderer } from './WorkflowCards/WorkflowCardRenderer';
import { StructuredInputOverlay } from './StructuredInputOverlay';
import { ToolPermissionOverlay } from './ToolPermissionOverlay';
import { PermissionSelector } from './PermissionSelector';
import { WorkflowProgressPanel } from './WorkflowProgressPanel';
import { useToolPermissionStore } from '../../store/toolPermission';
import type { CardPayload } from '../../types/workflowCard';

type WorkflowMode = 'chat' | 'task';

function formatNumber(value: number | null | undefined): string {
  if (typeof value !== 'number' || Number.isNaN(value)) return '0';
  return value.toLocaleString();
}

export function SimpleMode() {
  const { t } = useTranslation('simpleMode');
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
  } = useExecutionStore();
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const sidebarCollapsed = useSettingsStore((s) => s.sidebarCollapsed);
  const setSidebarCollapsed = useSettingsStore((s) => s.setSidebarCollapsed);

  const [description, setDescription] = useState('');
  const [showOutputPanel, setShowOutputPanel] = useState(false);
  const [showDiffPanel, setShowDiffPanel] = useState(false);
  const [workflowMode, setWorkflowMode] = useState<WorkflowMode>('chat');

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

  const workflowPhase = useWorkflowOrchestratorStore((s) => s.phase);
  const pendingQuestion = useWorkflowOrchestratorStore((s) => s.pendingQuestion);
  const startWorkflow = useWorkflowOrchestratorStore((s) => s.startWorkflow);
  const submitInterviewAnswer = useWorkflowOrchestratorStore((s) => s.submitInterviewAnswer);
  const skipInterviewQuestion = useWorkflowOrchestratorStore((s) => s.skipInterviewQuestion);
  const overrideConfigNatural = useWorkflowOrchestratorStore((s) => s.overrideConfigNatural);
  const addPrdFeedback = useWorkflowOrchestratorStore((s) => s.addPrdFeedback);
  const cancelWorkflow = useWorkflowOrchestratorStore((s) => s.cancelWorkflow);
  const resetWorkflow = useWorkflowOrchestratorStore((s) => s.resetWorkflow);
  const isInterviewSubmitting = useWorkflowOrchestratorStore((s) => s.phase === 'interviewing') &&
    pendingQuestion === null;

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

    await start(description, 'simple');
    setDescription('');
  }, [
    description,
    isAnalyzingStrategy,
    isSubmitting,
    start,
    startWorkflow,
    workflowMode,
  ]);

  const handleFollowUp = useCallback(async () => {
    if (!description.trim() || isSubmitting) return;
    const prompt = description;
    setDescription('');

    // Route through orchestrator if in active Task workflow phase
    if (workflowMode === 'task' && workflowPhase !== 'idle') {
      if (workflowPhase === 'interviewing') {
        await submitInterviewAnswer(prompt);
      } else if (workflowPhase === 'configuring') {
        overrideConfigNatural(prompt);
      } else if (workflowPhase === 'reviewing_prd') {
        addPrdFeedback(prompt);
      }
      return;
    }

    await sendFollowUp(prompt);
  }, [description, isSubmitting, sendFollowUp, workflowMode, workflowPhase, submitInterviewAnswer, overrideConfigNatural, addPrdFeedback]);

  const handleNewTask = useCallback(() => {
    resetWorkflow();
    reset();
    clearStrategyAnalysis();
    setDescription('');
  }, [reset, clearStrategyAnalysis, resetWorkflow]);

  const handleRestoreHistory = useCallback(
    (historyId: string) => {
      restoreFromHistory(historyId);
      setShowOutputPanel(false);
      setShowDiffPanel(false);
      setWorkflowMode('chat');
    },
    [restoreFromHistory]
  );

  const isRunning = status === 'running' || status === 'paused';
  const isDisabled = isRunning || isSubmitting || isAnalyzingStrategy;

  const detailLineCount = useMemo(
    () => streamingOutput.filter((line) => line.type !== 'text' && line.type !== 'info').length,
    [streamingOutput]
  );

  // Whether any right panel is visible (for grid layout calculation)
  const showRightPanel = showOutputPanel || showDiffPanel;

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between px-6 py-3 shrink-0 max-w-[2200px] mx-auto w-full">
        <div className="flex items-center gap-2">
          <ConnectionStatus status={connectionStatus} />
          <ProjectSelector compact />
          <ModelSwitcher />
          <PermissionSelector
            level={permissionLevel}
            onLevelChange={(level) => setPermissionLevel('current-session', level)}
            sessionId="current-session"
          />
          {workspacePath && <IndexStatus compact />}
          <TokenUsageInline turnUsage={turnUsageTotals} totals={sessionUsageTotals} />
        </div>

        <div className="flex items-center gap-2">
          <ContextualActions
            className="hidden lg:flex"
            onPauseExecution={pause}
            onResumeExecution={resume}
            onCancelExecution={cancel}
            onResetExecution={reset}
          />

          <button
            onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
            className={clsx(
              'text-sm px-3 py-1.5 rounded-lg transition-colors',
              'text-gray-600 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
              !sidebarCollapsed && 'bg-gray-100 dark:bg-gray-800'
            )}
            title={t('sidebar.toggleSidebar', { defaultValue: 'Toggle sidebar' })}
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h7" />
            </svg>
          </button>

          <div className="flex items-center rounded-lg border border-gray-300 dark:border-gray-700 overflow-hidden">
            <button
              onClick={() => setWorkflowMode('chat')}
              className={clsx(
                'px-3 py-1.5 text-xs font-medium transition-colors',
                workflowMode === 'chat'
                  ? 'bg-primary-600 text-white'
                  : 'bg-white dark:bg-gray-900 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800'
              )}
            >
              {t('workflowMode.chat', { defaultValue: 'Chat' })}
            </button>
            <button
              onClick={() => setWorkflowMode('task')}
              className={clsx(
                'px-3 py-1.5 text-xs font-medium transition-colors',
                workflowMode === 'task'
                  ? 'bg-primary-600 text-white'
                  : 'bg-white dark:bg-gray-900 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800'
              )}
            >
              {t('workflowMode.task', { defaultValue: 'Task' })}
            </button>
          </div>

          <button
            onClick={() => {
              setShowOutputPanel((v) => {
                const next = !v;
                if (next) setShowDiffPanel(false);
                return next;
              });
            }}
            className={clsx(
              'text-sm px-3 py-1.5 rounded-lg transition-colors',
              'text-gray-600 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
              showOutputPanel && 'bg-gray-100 dark:bg-gray-800'
            )}
            title={t('output.toggleTitle', { defaultValue: 'Toggle detailed output panel' })}
          >
            {t('output.label', { defaultValue: 'Output' })}{detailLineCount > 0 ? ` (${detailLineCount})` : ''}
          </button>

          <button
            onClick={() => {
              setShowDiffPanel((v) => {
                const next = !v;
                if (next) setShowOutputPanel(false);
                return next;
              });
            }}
            className={clsx(
              'text-sm px-3 py-1.5 rounded-lg transition-colors',
              'text-gray-600 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
              showDiffPanel && 'bg-gray-100 dark:bg-gray-800'
            )}
            title={t('diffPanel.title', { defaultValue: 'Changes' })}
          >
            {t('diffPanel.title', { defaultValue: 'Diffs' })}
          </button>
        </div>
      </div>

      <div className="flex-1 min-h-0 px-6 pb-4">
        <div
          className={clsx(
            'h-full max-w-[2200px] mx-auto w-full grid gap-4',
            sidebarCollapsed
              ? (showRightPanel
                  ? 'grid-cols-1 xl:grid-cols-[minmax(480px,1fr)_minmax(520px,0.95fr)]'
                  : 'grid-cols-1 xl:grid-cols-[minmax(640px,1fr)]')
              : (showRightPanel
                  ? 'grid-cols-1 xl:grid-cols-[280px_minmax(480px,1fr)_minmax(520px,0.95fr)]'
                  : 'grid-cols-1 xl:grid-cols-[280px_minmax(640px,1fr)]')
          )}
        >
          {!sidebarCollapsed && (
            <WorkspaceTreeSidebar
              history={history}
              onRestore={handleRestoreHistory}
              onDelete={deleteHistory}
              onRename={renameHistory}
              onClear={clearHistory}
              onNewTask={handleNewTask}
              currentTask={isChatSession ? (streamingOutput[0]?.content || null) : null}
              backgroundSessions={backgroundSessions}
              onSwitchSession={switchToSession}
              onRemoveSession={removeBackgroundSession}
            />
          )}

          <div className="min-h-0 flex flex-col rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
            <div className="flex-1 min-h-0">
              <ChatTranscript lines={streamingOutput} status={status} />
            </div>

            <div className="shrink-0 border-t border-gray-200 dark:border-gray-700">
              {/* Priority 1: Tool permission approval overlay */}
              {permissionRequest ? (
                <ToolPermissionOverlay
                  request={permissionRequest}
                  onRespond={respondPermission}
                  loading={isPermissionResponding}
                  queueSize={permissionQueueSize}
                />
              ) : /* Priority 2: Structured input overlay for interview boolean/select questions */
              workflowMode === 'task' && pendingQuestion && pendingQuestion.inputType !== 'text' && pendingQuestion.inputType !== 'textarea' ? (
                <StructuredInputOverlay
                  question={pendingQuestion}
                  onSubmit={submitInterviewAnswer}
                  onSkip={skipInterviewQuestion}
                  loading={isInterviewSubmitting}
                />
              ) : (
                <div className="p-4">
                  <InputBox
                    value={description}
                    onChange={setDescription}
                    onSubmit={isChatSession || (workflowMode === 'task' && workflowPhase !== 'idle') ? handleFollowUp : handleStart}
                    disabled={isDisabled}
                    placeholder={
                      isRunning
                        ? t('workflow.input.waitingPlaceholder', { defaultValue: 'Waiting for response...' })
                        : workflowMode === 'task' && workflowPhase === 'interviewing'
                          ? t('workflow.input.interviewPlaceholder', { defaultValue: 'Type your answer...' })
                          : workflowMode === 'task' && workflowPhase === 'reviewing_prd'
                            ? t('workflow.input.prdFeedbackPlaceholder', { defaultValue: 'Add feedback or press Approve on the PRD card...' })
                            : workflowMode === 'task'
                              ? t('workflow.input.taskPlaceholder', {
                                  defaultValue: 'Describe a task (implementation / analysis / refactor)...',
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
                  />
                </div>
              )}
              {apiError && (
                <div className="mx-4 mb-3 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                  <p className="text-sm text-red-600 dark:text-red-400">{apiError}</p>
                </div>
              )}
              {/* Cancel button during active workflow */}
              {workflowMode === 'task' && workflowPhase !== 'idle' && workflowPhase !== 'completed' && workflowPhase !== 'failed' && workflowPhase !== 'cancelled' && (
                <div className="px-4 pb-3">
                  <button
                    onClick={cancelWorkflow}
                    className="text-xs text-gray-500 dark:text-gray-400 hover:text-red-600 dark:hover:text-red-400 transition-colors"
                  >
                    {t('workflow.cancelWorkflow')}
                  </button>
                </div>
              )}
            </div>
          </div>

          {showOutputPanel && (
            <div className="min-h-0 flex flex-col">
              <div className="shrink-0 space-y-2 mb-2">
                {workflowMode === 'task' ? (
                  <>
                    {/* Task mode: show workflow progress panel */}
                    {workflowPhase !== 'idle' && <WorkflowProgressPanel />}
                    <ExecutionLogsCard logs={logs} />
                    <ErrorState maxErrors={3} />
                  </>
                ) : (
                  <>
                    {/* Chat mode: no progress section, only analysis coverage */}
                    {analysisCoverage && <AnalysisCoveragePanel coverage={analysisCoverage} />}
                    <ExecutionLogsCard logs={logs} />
                    <ErrorState maxErrors={3} />
                  </>
                )}
              </div>
              <StreamingOutput maxHeight="none" compact={false} showClear className="flex-1 min-h-0" />
            </div>
          )}

          {showDiffPanel && (
            <GitPanel
              streamingOutput={streamingOutput}
              workspacePath={workspacePath}
            />
          )}
        </div>
      </div>
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
}: {
  lines: StreamLine[];
  status: 'idle' | 'running' | 'paused' | 'completed' | 'failed';
}) {
  const { t } = useTranslation('simpleMode');
  const containerRef = useRef<HTMLDivElement>(null);
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
    if (result.length === 0 && lines.length > 0 &&
        lines.some((l) => l.type !== 'info')) {
      const syntheticUserLine: StreamLine = {
        id: -1, content: '(continued)', type: 'info', timestamp: lines[0].timestamp,
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

  // Auto-scroll
  useEffect(() => {
    if (!containerRef.current) return;
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
    <div ref={containerRef} className="h-full overflow-y-auto px-4 py-4 space-y-4">
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
}: {
  lines: StreamLine[];
  isLastTurn: boolean;
  userLineId: number;
  disabled: boolean;
  isClaudeCodeBackend: boolean;
  onEdit: (lineId: number, newContent: string) => void;
  onCopy: (content: string) => void;
}) {
  const showReasoning = useSettingsStore((s) => s.showReasoningOutput);

  // Separate thinking from other lines
  const thinkingLines = useMemo(() => lines.filter((l) => l.type === 'thinking'), [lines]);
  const contentLines = useMemo(() => lines.filter((l) => l.type !== 'thinking'), [lines]);

  // Build display blocks for content (always grouped, like compact mode)
  const blocks = useMemo(() => buildDisplayBlocks(contentLines, true), [contentLines]);

  // Collect text content for copy
  const textContent = useMemo(
    () => lines.filter((l) => l.type === 'text').map((l) => l.content).join(''),
    [lines]
  );

  // Find last text line for MessageActions
  const lastTextLine = useMemo(
    () => [...lines].reverse().find((l) => l.type === 'text'),
    [lines]
  );

  // Check if there's rich content (tools, sub-agents, etc.)
  const hasRichContent = contentLines.some(
    (l) => l.type === 'tool' || l.type === 'tool_result' || l.type === 'sub_agent' || l.type === 'analysis' || l.subAgentId
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
        {showReasoning && thinkingLines.length > 0 && (
          <ChatThinkingSection lines={thinkingLines} />
        )}

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
              return <WorkflowCardRenderer key={line.id} payload={payload} />;
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
      {expanded && (
        <div className="px-3 py-2 border-t border-gray-200 dark:border-gray-600 text-xs text-gray-500 dark:text-gray-400 italic font-mono whitespace-pre-wrap max-h-64 overflow-y-auto">
          {content}
        </div>
      )}
    </div>
  );
}

function TokenUsageInline({
  turnUsage,
  totals,
}: {
  turnUsage: ReturnType<typeof useExecutionStore.getState>['turnUsageTotals'];
  totals: ReturnType<typeof useExecutionStore.getState>['sessionUsageTotals'];
}) {
  if (!turnUsage && !totals) return null;

  const turn = turnUsage || {
    input_tokens: 0,
    output_tokens: 0,
    thinking_tokens: 0,
    cache_read_tokens: 0,
    cache_creation_tokens: 0,
  };
  const total = totals || turn;

  return (
    <div className="hidden xl:flex items-center gap-2 ml-1">
      <span className="px-2 py-1 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 text-2xs">
        turn in {formatNumber(turn.input_tokens)} / out {formatNumber(turn.output_tokens)}
      </span>
      <span className="px-2 py-1 rounded bg-sky-50 dark:bg-sky-900/20 text-sky-700 dark:text-sky-300 text-2xs">
        session in {formatNumber(total.input_tokens)} / out {formatNumber(total.output_tokens)}
      </span>
    </div>
  );
}


function pct(value: number | undefined): string {
  if (typeof value !== 'number' || !Number.isFinite(value)) return '-';
  return `${(value * 100).toFixed(1)}%`;
}

function AnalysisCoveragePanel({
  coverage,
}: {
  coverage: NonNullable<ReturnType<typeof useExecutionStore.getState>['analysisCoverage']>;
}) {
  const coverageProgress = Math.max(0, Math.min(100, (coverage.coverageRatio || 0) * 100));
  const sampledProgress = Math.max(0, Math.min(100, (coverage.sampledReadRatio || 0) * 100));
  const testsProgress = Math.max(0, Math.min(100, (coverage.testCoverageRatio || 0) * 100));

  return (
    <div className="p-3 rounded-lg bg-sky-50 dark:bg-sky-900/20 border border-sky-200 dark:border-sky-800">
      <div className="flex items-center justify-between">
        <p className="text-sm font-medium text-sky-700 dark:text-sky-300">Analysis Coverage</p>
        <span className="text-xs text-sky-600 dark:text-sky-400">{coverage.status}</span>
      </div>

      <div className="mt-2 grid grid-cols-1 sm:grid-cols-3 gap-2 text-xs">
        <MetricBar label="Observed" value={pct(coverage.coverageRatio)} progress={coverageProgress} />
        <MetricBar label="Read Depth" value={pct(coverage.sampledReadRatio)} progress={sampledProgress} />
        <MetricBar label="Tests Read" value={pct(coverage.testCoverageRatio)} progress={testsProgress} />
      </div>

      <div className="mt-2 text-xs text-sky-700/90 dark:text-sky-300/90">
        files {coverage.observedPaths}/{coverage.inventoryTotalFiles} | sampled {coverage.sampledReadFiles} | tests {coverage.testFilesRead}/{coverage.testFilesTotal}
      </div>

      {(coverage.coverageTargetRatio || coverage.sampledReadTargetRatio || coverage.testCoverageTargetRatio) && (
        <div className="mt-1 text-xs text-sky-600 dark:text-sky-400">
          targets: observed {pct(coverage.coverageTargetRatio)} | read depth {pct(coverage.sampledReadTargetRatio)} | tests {pct(coverage.testCoverageTargetRatio)}
        </div>
      )}

      {coverage.validationIssues.length > 0 && (
        <div className="mt-2 text-xs text-amber-700 dark:text-amber-300">
          validation: {coverage.validationIssues.slice(0, 2).join(' ; ')}
        </div>
      )}
    </div>
  );
}

function MetricBar({
  label,
  value,
  progress,
}: {
  label: string;
  value: string;
  progress: number;
}) {
  return (
    <div>
      <div className="flex items-center justify-between text-sky-700 dark:text-sky-300">
        <span>{label}</span>
        <span>{value}</span>
      </div>
      <div className="mt-1 h-1.5 rounded bg-sky-100 dark:bg-sky-900/50 overflow-hidden">
        <div className="h-full bg-sky-500" style={{ width: `${progress}%` }} />
      </div>
    </div>
  );
}

function ExecutionLogsCard({ logs }: { logs: string[] }) {
  const recent = logs.slice(-20).reverse();
  if (recent.length === 0) return null;

  return (
    <div className="p-3 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
      <div className="flex items-center justify-between">
        <p className="text-sm font-medium text-gray-800 dark:text-gray-200">Execution Logs</p>
        <span className="text-2xs text-gray-500 dark:text-gray-400">{recent.length}</span>
      </div>
      <div className="mt-2 max-h-32 overflow-y-auto rounded border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-950 p-2 font-mono text-2xs text-gray-700 dark:text-gray-300 space-y-1">
        {recent.map((line, idx) => (
          <div key={`${idx}-${line.slice(0, 16)}`} className="whitespace-pre-wrap break-words">
            {line}
          </div>
        ))}
      </div>
    </div>
  );
}

export default SimpleMode;
