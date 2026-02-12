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
import { useExecutionStore, type ExecutionHistoryItem, type StreamLine } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';
import { StreamingOutput, GlobalProgressBar, ErrorState, ProjectSelector, IndexStatus } from '../shared';

type WorkflowMode = 'chat' | 'task';

function formatNumber(value: number | null | undefined): string {
  if (typeof value !== 'number' || Number.isNaN(value)) return '0';
  return value.toLocaleString();
}

function normalizeWorkspacePath(path: string | null | undefined): string | null {
  const value = (path || '').trim();
  if (!value) return null;
  return value.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
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
    reset,
    initialize,
    cleanup,
    analyzeStrategy,
    strategyAnalysis,
    isAnalyzingStrategy,
    clearStrategyAnalysis,
    isChatSession,
    streamingOutput,
    standaloneTurns,
    analysisCoverage,
    logs,
    history,
    clearHistory,
    renameHistory,
    restoreFromHistory,
    sessionUsageTotals,
    latestUsage,
  } = useExecutionStore();
  const workspacePath = useSettingsStore((s) => s.workspacePath);

  const [description, setDescription] = useState('');
  const [showOutputPanel, setShowOutputPanel] = useState(false);
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

  return (
    <div className="h-full flex flex-col">
      <div className="flex items-center justify-between px-6 py-3 shrink-0 max-w-[2200px] mx-auto w-full">
        <div className="flex items-center gap-2">
          <ConnectionStatus status={connectionStatus} />
          <ProjectSelector compact />
          {workspacePath && <IndexStatus compact />}
          <TokenUsageInline latestUsage={latestUsage} totals={sessionUsageTotals} />
        </div>

        <div className="flex items-center gap-2">
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
            onClick={() => setShowOutputPanel((v) => !v)}
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

          {(isChatSession || standaloneTurns.length > 0) && (
            <button
              onClick={handleNewTask}
              className={clsx(
                'text-sm px-3 py-1.5 rounded-lg transition-colors',
                'text-gray-600 dark:text-gray-400',
                'hover:bg-gray-100 dark:hover:bg-gray-800'
              )}
            >
              {t('buttons.startNewTask', { ns: 'common', defaultValue: 'New Chat' })}
            </button>
          )}
        </div>
      </div>

      <div className="flex-1 min-h-0 px-6 pb-4">
        <div
          className={clsx(
            'h-full max-w-[2200px] mx-auto w-full grid gap-4',
            showOutputPanel
              ? 'grid-cols-1 xl:grid-cols-[280px_minmax(480px,1fr)_minmax(520px,0.95fr)]'
              : 'grid-cols-1 xl:grid-cols-[280px_minmax(640px,1fr)]'
          )}
        >
          <SessionSidebar
            history={history}
            workspacePath={workspacePath}
            onClear={clearHistory}
            onRename={renameHistory}
            onRestore={handleRestoreHistory}
            currentTask={isChatSession ? (streamingOutput[0]?.content || null) : null}
          />

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
        </div>
      </div>
    </div>
  );
}

function SessionSidebar({
  history,
  workspacePath,
  onClear,
  onRename,
  onRestore,
  currentTask,
}: {
  history: ExecutionHistoryItem[];
  workspacePath?: string | null;
  onClear: () => void;
  onRename: (id: string, title: string) => void;
  onRestore: (id: string) => void;
  currentTask: string | null;
}) {
  const activeWorkspace = normalizeWorkspacePath(workspacePath);
  const visibleHistory = useMemo(() => {
    if (!activeWorkspace) return history;
    return history.filter((item) => {
      const scope = normalizeWorkspacePath(item.workspacePath);
      return !scope || scope === activeWorkspace;
    });
  }, [activeWorkspace, history]);

  const grouped = useMemo(() => {
    const withScope: ExecutionHistoryItem[] = [];
    const noScope: ExecutionHistoryItem[] = [];
    for (const item of visibleHistory) {
      const scope = normalizeWorkspacePath(item.workspacePath);
      if (scope) {
        withScope.push(item);
      } else {
        noScope.push(item);
      }
    }
    return { withScope, noScope };
  }, [visibleHistory]);

  const handleRename = useCallback(
    (item: ExecutionHistoryItem) => {
      const current = item.title || item.taskDescription;
      const next = window.prompt('Rename session', current);
      if (next === null) return;
      onRename(item.id, next);
    },
    [onRename]
  );

  return (
    <div className="min-h-0 flex flex-col rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
        <h3 className="text-sm font-semibold text-gray-900 dark:text-white">Sessions</h3>
        <button
          onClick={onClear}
          className="text-xs px-2 py-1 rounded text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20"
        >
          Clear
        </button>
      </div>

      {currentTask && (
        <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700 text-xs">
          <p className="text-gray-500 dark:text-gray-400">Current</p>
          <p className="text-gray-700 dark:text-gray-200 line-clamp-2">{currentTask}</p>
        </div>
      )}

      <div className="flex-1 min-h-0 overflow-y-auto p-2 space-y-2">
        {visibleHistory.length === 0 ? (
          <div className="h-full flex items-center justify-center text-xs text-gray-500 dark:text-gray-400">
            No saved sessions
          </div>
        ) : (
          <>
            {grouped.withScope.length > 0 && (
              <div className="space-y-2">
                <p className="px-1 text-2xs uppercase tracking-wide text-gray-500 dark:text-gray-400">
                  {activeWorkspace ? 'Current Workspace' : 'Scoped Sessions'}
                </p>
                {grouped.withScope.map((item) => (
                  <div
                    key={item.id}
                    onClick={() => onRestore(item.id)}
                    className={clsx(
                      'w-full text-left p-2 rounded border transition-colors cursor-pointer',
                      'border-gray-200 dark:border-gray-700',
                      'hover:bg-gray-50 dark:hover:bg-gray-800'
                    )}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') onRestore(item.id);
                    }}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <p className="text-xs font-medium text-gray-900 dark:text-white line-clamp-2">
                        {item.title || item.taskDescription}
                      </p>
                      <button
                        className="shrink-0 text-2xs px-1.5 py-0.5 rounded text-gray-500 hover:text-gray-200 hover:bg-gray-700/50"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleRename(item);
                        }}
                        title="Rename session"
                      >
                        rename
                      </button>
                    </div>
                    <p className="text-2xs mt-1 text-gray-500 dark:text-gray-400 line-clamp-1">
                      {item.workspacePath}
                    </p>
                    <p className="text-2xs mt-1 text-gray-500 dark:text-gray-400">
                      {new Date(item.startedAt).toLocaleString()} | {item.success ? 'success' : 'failed'}
                    </p>
                  </div>
                ))}
              </div>
            )}

            {grouped.noScope.length > 0 && (
              <div className="space-y-2 pt-1">
                <p className="px-1 text-2xs uppercase tracking-wide text-gray-500 dark:text-gray-400">
                  No Workspace
                </p>
                {grouped.noScope.map((item) => (
                  <div
                    key={item.id}
                    onClick={() => onRestore(item.id)}
                    className={clsx(
                      'w-full text-left p-2 rounded border transition-colors cursor-pointer',
                      'border-gray-200 dark:border-gray-700',
                      'hover:bg-gray-50 dark:hover:bg-gray-800'
                    )}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' || e.key === ' ') onRestore(item.id);
                    }}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <p className="text-xs font-medium text-gray-900 dark:text-white line-clamp-2">
                        {item.title || item.taskDescription}
                      </p>
                      <button
                        className="shrink-0 text-2xs px-1.5 py-0.5 rounded text-gray-500 hover:text-gray-200 hover:bg-gray-700/50"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleRename(item);
                        }}
                        title="Rename session"
                      >
                        rename
                      </button>
                    </div>
                    <p className="text-2xs mt-1 text-gray-500 dark:text-gray-400">
                      {new Date(item.startedAt).toLocaleString()} | {item.success ? 'success' : 'failed'}
                    </p>
                  </div>
                ))}
              </div>
            )}
          </>
        )}
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
        if (line.type === 'info') {
          return (
            <div key={line.id} className="flex justify-end">
              <div className="max-w-[82%] px-4 py-2 rounded-2xl rounded-br-sm bg-primary-600 text-white text-sm whitespace-pre-wrap">
                {line.content}
              </div>
            </div>
          );
        }
        if (line.type === 'text') {
          return (
            <div key={line.id} className="flex justify-start">
              <div className="max-w-[88%] px-4 py-2 rounded-2xl rounded-bl-sm bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100">
                <MarkdownRenderer content={line.content} className="text-sm" />
              </div>
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
