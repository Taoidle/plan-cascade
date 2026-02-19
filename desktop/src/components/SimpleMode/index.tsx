/**
 * SimpleMode Component
 *
 * Three-panel layout:
 * - Left: conversation/session manager
 * - Middle: normal chat view
 * - Right (optional): detailed output/tool activity
 */

import { useEffect, useMemo, useRef, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { MarkdownRenderer } from '../ClaudeCodeMode/MarkdownRenderer';
import { InputBox } from './InputBox';
import { ProgressView } from './ProgressView';
import { ConnectionStatus } from './ConnectionStatus';
import { MessageActions, EditMode } from './MessageActions';
import { ModelSwitcher } from './ModelSwitcher';
import { WorkspaceTreeSidebar } from './WorkspaceTreeSidebar';
import { GitPanel } from './GitPanel';
import { useExecutionStore, type StreamLine } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';
import { deriveConversationTurns } from '../../lib/conversationUtils';
import { StreamingOutput, GlobalProgressBar, ErrorState, ProjectSelector, IndexStatus } from '../shared';
import { ContextualActions } from '../shared/ContextualActions';

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
    analyzeStrategy,
    strategyAnalysis,
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
    latestUsage,
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

  const handleStart = useCallback(async () => {
    if (!description.trim() || isSubmitting || isAnalyzingStrategy) return;

    if (workflowMode === 'task') {
      await analyzeStrategy(description);
    }

    await start(description, 'simple');
    setDescription('');
  }, [
    analyzeStrategy,
    description,
    isAnalyzingStrategy,
    isSubmitting,
    start,
    workflowMode,
  ]);

  const handleFollowUp = useCallback(async () => {
    if (!description.trim() || isSubmitting) return;
    const prompt = description;
    setDescription('');
    await sendFollowUp(prompt);
  }, [description, isSubmitting, sendFollowUp]);

  const handleNewTask = useCallback(() => {
    reset();
    clearStrategyAnalysis();
    setDescription('');
  }, [reset, clearStrategyAnalysis]);

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
          {workspacePath && <IndexStatus compact />}
          <TokenUsageInline latestUsage={latestUsage} totals={sessionUsageTotals} />
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
              Chat
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
              Task
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
            title="Toggle detailed output panel"
          >
            Output{detailLineCount > 0 ? ` (${detailLineCount})` : ''}
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

            <div className="shrink-0 p-4 border-t border-gray-200 dark:border-gray-700">
              <InputBox
                value={description}
                onChange={setDescription}
                onSubmit={isChatSession ? handleFollowUp : handleStart}
                disabled={isDisabled}
                placeholder={
                  isRunning
                    ? t('input.waitingPlaceholder', { defaultValue: 'Waiting for response...' })
                    : workflowMode === 'task'
                      ? t('input.placeholder', {
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
              {apiError && (
                <div className="mt-3 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
                  <p className="text-sm text-red-600 dark:text-red-400">{apiError}</p>
                </div>
              )}
            </div>
          </div>

          {showOutputPanel && (
            <div className="min-h-0 flex flex-col">
              <div className="shrink-0 space-y-2 mb-2">
                {(isAnalyzingStrategy || strategyAnalysis) && workflowMode === 'task' && (
                  <StrategyBanner
                    isAnalyzing={isAnalyzingStrategy}
                    analysis={strategyAnalysis}
                    t={t}
                  />
                )}
                {analysisCoverage && <AnalysisCoveragePanel coverage={analysisCoverage} />}
                {isRunning && !isChatSession && (
                  <div className="p-3 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
                    <GlobalProgressBar compact showStoryLabels={false} />
                    <div className="mt-2">
                      <ProgressView />
                    </div>
                  </div>
                )}
                <ExecutionLogsCard logs={logs} />
                <ErrorState maxErrors={3} />
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

function ChatTranscript({
  lines,
  status,
}: {
  lines: StreamLine[];
  status: 'idle' | 'running' | 'paused' | 'completed' | 'failed';
}) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [editingLineId, setEditingLineId] = useState<number | null>(null);

  const visibleLines = useMemo(
    () =>
      lines.filter((line) =>
        line.type === 'info' ||
        line.type === 'text' ||
        line.type === 'error' ||
        line.type === 'success' ||
        line.type === 'warning'
      ),
    [lines]
  );

  // Derive conversation turns for MessageActions
  const turns = useMemo(() => deriveConversationTurns(visibleLines), [visibleLines]);

  // Determine backend type and disabled state
  const backend = useSettingsStore((s) => s.backend);
  const isClaudeCodeBackendValue = backend === 'claude-code';
  const isActionsDisabled = status === 'running' || status === 'paused';

  // Build a map from line.id to the turn's userLineId (for assistant lines to find their parent turn)
  const lineToUserLineId = useMemo(() => {
    const map = new Map<number, number>();
    for (const turn of turns) {
      // Map the user info line
      map.set(turn.userLineId, turn.userLineId);
      // Map all visible lines in the assistant range to this turn's userLineId
      for (const vl of visibleLines) {
        if (vl.id > turn.userLineId) {
          const nextTurn = turns.find((t) => t.turnIndex === turn.turnIndex + 1);
          if (!nextTurn || vl.id < nextTurn.userLineId) {
            map.set(vl.id, turn.userLineId);
          }
        }
      }
    }
    return map;
  }, [turns, visibleLines]);

  // Determine the last turn's userLineId
  const lastTurnUserLineId = turns.length > 0 ? turns[turns.length - 1].userLineId : -1;

  // Clear editing state when lines change (e.g., after edit submission)
  useEffect(() => {
    if (editingLineId !== null) {
      const lineStillExists = visibleLines.some((l) => l.id === editingLineId);
      if (!lineStillExists) {
        setEditingLineId(null);
      }
    }
  }, [visibleLines, editingLineId]);

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

  const handleRegenerate = useCallback((lineId: number) => {
    // For assistant messages, find the parent turn's userLineId
    const userLineId = lineToUserLineId.get(lineId) ?? lineId;
    useExecutionStore.getState().regenerateResponse(userLineId);
  }, [lineToUserLineId]);

  const handleRollback = useCallback((lineId: number) => {
    useExecutionStore.getState().rollbackToTurn(lineId);
  }, []);

  const handleCopy = useCallback((content: string) => {
    navigator.clipboard.writeText(content).catch(() => {
      // Fallback: silently fail
    });
  }, []);

  useEffect(() => {
    if (!containerRef.current) return;
    containerRef.current.scrollTop = containerRef.current.scrollHeight;
  }, [visibleLines]);

  if (visibleLines.length === 0) {
    return (
      <div className="h-full flex items-center justify-center text-sm text-gray-500 dark:text-gray-400">
        {status === 'running' ? 'Thinking...' : 'Start a conversation on the right input box.'}
      </div>
    );
  }

  return (
    <div ref={containerRef} className="h-full overflow-y-auto px-4 py-4 space-y-4">
      {visibleLines.map((line) => {
        const userLineId = lineToUserLineId.get(line.id);
        const isLastTurn = userLineId === lastTurnUserLineId;

        if (line.type === 'info') {
          // User message: show edit mode or normal message bubble
          if (editingLineId === line.id) {
            return (
              <div key={line.id} className="flex justify-end">
                <EditMode
                  content={line.content}
                  onSave={(newContent) => handleEdit(line.id, newContent)}
                  onCancel={handleEditCancel}
                  isClaudeCodeBackend={isClaudeCodeBackendValue}
                />
              </div>
            );
          }
          return (
            <div key={line.id} className="group relative flex justify-end">
              <div className="max-w-[82%] px-4 py-2 rounded-2xl rounded-br-sm bg-primary-600 text-white text-sm whitespace-pre-wrap">
                {line.content}
              </div>
              <MessageActions
                line={line}
                isUserMessage={true}
                isLastTurn={isLastTurn}
                isClaudeCodeBackend={isClaudeCodeBackendValue}
                disabled={isActionsDisabled}
                onEdit={handleEdit}
                onRegenerate={handleRegenerate}
                onRollback={handleRollback}
                onCopy={handleCopy}
                onEditStart={handleEditStart}
                onEditCancel={handleEditCancel}
              />
            </div>
          );
        }
        if (line.type === 'text') {
          return (
            <div key={line.id} className="group relative flex justify-start">
              <div className="max-w-[88%] px-4 py-2 rounded-2xl rounded-bl-sm bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100">
                <MarkdownRenderer content={line.content} className="text-sm" />
              </div>
              <MessageActions
                line={line}
                isUserMessage={false}
                isLastTurn={isLastTurn}
                isClaudeCodeBackend={isClaudeCodeBackendValue}
                disabled={isActionsDisabled}
                onEdit={handleEdit}
                onRegenerate={handleRegenerate}
                onRollback={handleRollback}
                onCopy={handleCopy}
              />
            </div>
          );
        }
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
      })}
    </div>
  );
}

function TokenUsageInline({
  latestUsage,
  totals,
}: {
  latestUsage: ReturnType<typeof useExecutionStore.getState>['latestUsage'];
  totals: ReturnType<typeof useExecutionStore.getState>['sessionUsageTotals'];
}) {
  if (!latestUsage && !totals) return null;

  const latest = latestUsage || {
    input_tokens: 0,
    output_tokens: 0,
    thinking_tokens: 0,
    cache_read_tokens: 0,
    cache_creation_tokens: 0,
  };
  const total = totals || latest;

  return (
    <div className="hidden xl:flex items-center gap-2 ml-1">
      <span className="px-2 py-1 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 text-2xs">
        turn in {formatNumber(latest.input_tokens)} / out {formatNumber(latest.output_tokens)}
      </span>
      <span className="px-2 py-1 rounded bg-sky-50 dark:bg-sky-900/20 text-sky-700 dark:text-sky-300 text-2xs">
        session in {formatNumber(total.input_tokens)} / out {formatNumber(total.output_tokens)}
      </span>
    </div>
  );
}

function StrategyBanner({
  isAnalyzing,
  analysis,
  t,
}: {
  isAnalyzing: boolean;
  analysis: ReturnType<typeof useExecutionStore.getState>['strategyAnalysis'];
  t: ReturnType<typeof useTranslation>['t'];
}) {
  if (isAnalyzing) {
    return (
      <div className="p-3 rounded-lg bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 flex items-center gap-2">
        <div className="animate-spin h-4 w-4 border-2 border-blue-500 border-t-transparent rounded-full" />
        <p className="text-sm text-blue-600 dark:text-blue-400">
          {t('strategy.analyzing', { defaultValue: 'Analyzing task complexity...' })}
        </p>
      </div>
    );
  }

  if (!analysis) return null;

  return (
    <div className="p-3 rounded-lg bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800">
      <div className="flex items-center justify-between">
        <p className="text-sm font-medium text-green-700 dark:text-green-300">
          {t('strategy.selected', { defaultValue: 'Strategy' })}:{' '}
          <span className="font-semibold">{analysis.strategy.replace(/_/g, ' ')}</span>
          <span className="ml-2 text-xs text-green-600 dark:text-green-400">
            ({(analysis.confidence * 100).toFixed(0)}% confidence)
          </span>
        </p>
      </div>
      <p className="text-xs text-green-600 dark:text-green-400 mt-1">{analysis.reasoning}</p>
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
