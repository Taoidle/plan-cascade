/**
 * TabbedRightPanel Component
 *
 * Combined right panel with Output and Git tabs.
 * Output tab shows workflow progress, execution logs, and streaming output.
 * Git tab shows the GitPanel component.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { GitPanel } from './GitPanel';
import { WorkflowProgressPanel } from './WorkflowProgressPanel';
import { StreamingOutput, ErrorState } from '../shared';
import type { StreamLine } from '../../store/execution';
import type { AnalysisCoverageSnapshot } from '../../store/execution';

export type RightPanelTab = 'output' | 'git';

interface TabbedRightPanelProps {
  activeTab: RightPanelTab;
  onTabChange: (tab: RightPanelTab) => void;
  // Output tab props
  workflowMode: 'chat' | 'plan' | 'task';
  workflowPhase: string;
  logs: string[];
  analysisCoverage: AnalysisCoverageSnapshot | null;
  // Git tab props
  streamingOutput: StreamLine[];
  workspacePath: string | null;
}

export function TabbedRightPanel({
  activeTab,
  onTabChange,
  workflowMode,
  workflowPhase,
  logs,
  analysisCoverage,
  streamingOutput,
  workspacePath,
}: TabbedRightPanelProps) {
  const { t } = useTranslation('simpleMode');

  const tabs: { id: RightPanelTab; label: string }[] = [
    { id: 'output', label: t('rightPanel.outputTab', { defaultValue: 'Output' }) },
    { id: 'git', label: t('rightPanel.gitTab', { defaultValue: 'Git' }) },
  ];

  return (
    <div className="h-full flex flex-col rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
      {/* Tab bar */}
      <div className="shrink-0 flex items-center border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900/50 px-2">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => onTabChange(tab.id)}
            className={clsx(
              'px-3 py-2 text-xs font-medium transition-colors relative',
              activeTab === tab.id
                ? 'text-primary-600 dark:text-primary-400'
                : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
            )}
          >
            {tab.label}
            {/* Active indicator */}
            {activeTab === tab.id && (
              <span className="absolute bottom-0 left-1 right-1 h-0.5 bg-primary-600 dark:bg-primary-400 rounded-full" />
            )}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div key={activeTab} className="flex-1 min-h-0 overflow-y-auto animate-fade-in">
        {activeTab === 'output' ? (
          <div className="min-h-0 flex flex-col h-full">
            <div className="shrink-0 space-y-2 p-2">
              {workflowMode === 'task' ? (
                <>
                  {workflowPhase !== 'idle' && <WorkflowProgressPanel />}
                  <ExecutionLogsCard logs={logs} />
                  <ErrorState maxErrors={3} />
                </>
              ) : (
                <>
                  {analysisCoverage && <AnalysisCoveragePanel coverage={analysisCoverage} />}
                  <ExecutionLogsCard logs={logs} />
                  <ErrorState maxErrors={3} />
                </>
              )}
            </div>
            <StreamingOutput maxHeight="none" compact={false} showClear className="flex-1 min-h-0 px-2 pb-2" />
          </div>
        ) : (
          <GitPanel streamingOutput={streamingOutput} workspacePath={workspacePath} />
        )}
      </div>
    </div>
  );
}

// ============================================================================
// Internal helper components (moved from index.tsx)
// ============================================================================

function pct(value: number | undefined): string {
  if (typeof value !== 'number' || !Number.isFinite(value)) return '-';
  return `${(value * 100).toFixed(1)}%`;
}

function AnalysisCoveragePanel({ coverage }: { coverage: AnalysisCoverageSnapshot }) {
  const { t } = useTranslation('simpleMode');
  const coverageProgress = Math.max(0, Math.min(100, (coverage.coverageRatio || 0) * 100));
  const sampledProgress = Math.max(0, Math.min(100, (coverage.sampledReadRatio || 0) * 100));
  const testsProgress = Math.max(0, Math.min(100, (coverage.testCoverageRatio || 0) * 100));

  return (
    <div className="p-3 rounded-lg bg-sky-50 dark:bg-sky-900/20 border border-sky-200 dark:border-sky-800">
      <div className="flex items-center justify-between">
        <p className="text-sm font-medium text-sky-700 dark:text-sky-300">{t('analysisCoverage.title')}</p>
        <span className="text-xs text-sky-600 dark:text-sky-400">{coverage.status}</span>
      </div>

      <div className="mt-2 grid grid-cols-1 sm:grid-cols-3 gap-2 text-xs">
        <MetricBar
          label={t('analysisCoverage.observed')}
          value={pct(coverage.coverageRatio)}
          progress={coverageProgress}
        />
        <MetricBar
          label={t('analysisCoverage.readDepth')}
          value={pct(coverage.sampledReadRatio)}
          progress={sampledProgress}
        />
        <MetricBar
          label={t('analysisCoverage.testsRead')}
          value={pct(coverage.testCoverageRatio)}
          progress={testsProgress}
        />
      </div>

      <div className="mt-2 text-xs text-sky-700/90 dark:text-sky-300/90">
        files {coverage.observedPaths}/{coverage.inventoryTotalFiles} | sampled {coverage.sampledReadFiles} | tests{' '}
        {coverage.testFilesRead}/{coverage.testFilesTotal}
      </div>

      {(coverage.coverageTargetRatio || coverage.sampledReadTargetRatio || coverage.testCoverageTargetRatio) && (
        <div className="mt-1 text-xs text-sky-600 dark:text-sky-400">
          targets: observed {pct(coverage.coverageTargetRatio)} | read depth {pct(coverage.sampledReadTargetRatio)} |
          tests {pct(coverage.testCoverageTargetRatio)}
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

function MetricBar({ label, value, progress }: { label: string; value: string; progress: number }) {
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
  const { t } = useTranslation('simpleMode');
  const recent = logs.slice(-20).reverse();
  if (recent.length === 0) return null;

  return (
    <div className="p-3 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
      <div className="flex items-center justify-between">
        <p className="text-sm font-medium text-gray-800 dark:text-gray-200">{t('executionLogs.title')}</p>
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
