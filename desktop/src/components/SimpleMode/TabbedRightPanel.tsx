/**
 * TabbedRightPanel Component
 *
 * Combined right panel with Output and Git tabs.
 * Output tab shows workflow progress, execution logs, and streaming output.
 * Git tab shows the GitPanel component.
 */

import { clsx } from 'clsx';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { GitPanel } from './GitPanel';
import { ContextOpsPanel } from './ContextOpsPanel';
import { DebugArtifactsPanel } from './DebugArtifactsPanel';
import { WorkflowKernelProgressPanel } from './WorkflowKernelProgressPanel';
import { StreamingOutput, ErrorState } from '../shared';
import type { ExecutionStatus, StreamLine } from '../../store/execution';
import type { AnalysisCoverageSnapshot } from '../../store/execution';
import { useContextOpsStore } from '../../store/contextOps';
import { useWorkflowObservabilityStore } from '../../store/workflowObservability';
import { useSettingsStore } from '../../store/settings';

export type RightPanelTab = 'output' | 'git' | 'context' | 'artifacts';

interface TabbedRightPanelProps {
  activeTab: RightPanelTab;
  onTabChange: (tab: RightPanelTab) => void;
  // Output tab props
  workflowMode: 'chat' | 'plan' | 'task' | 'debug';
  workflowPhase: string;
  logs: string[];
  analysisCoverage: AnalysisCoverageSnapshot | null;
  executionStatus: ExecutionStatus;
  modeTranscriptLines: StreamLine[];
  // Git tab props
  workspacePath: string | null;
  rootSessionId: string | null;
  contextSessionId: string | null;
  debugSessionId?: string | null;
}

export function TabbedRightPanel({
  activeTab,
  onTabChange,
  workflowMode,
  workflowPhase,
  logs,
  analysisCoverage,
  executionStatus,
  modeTranscriptLines,
  workspacePath,
  rootSessionId,
  contextSessionId,
  debugSessionId,
}: TabbedRightPanelProps) {
  const { t } = useTranslation('simpleMode');
  const contextInspectorEnabled = useContextOpsStore((s) => s.policy.context_inspector_ui);
  const refreshPolicy = useContextOpsStore((s) => s.refreshPolicy);
  const observabilitySnapshot = useWorkflowObservabilityStore((s) => s.snapshot);
  const refreshObservability = useWorkflowObservabilityStore((s) => s.refreshSnapshot);
  const developerModeEnabled = useSettingsStore((s) => s.developerModeEnabled);
  const developerPanels = useSettingsStore((s) => s.developerPanels);
  const showContextTab = contextInspectorEnabled && developerModeEnabled && developerPanels.contextInspector;
  const showDebugArtifactsTab = workflowMode === 'debug' && !!debugSessionId;
  const effectiveActiveTab: RightPanelTab =
    !showContextTab && activeTab === 'context'
      ? 'output'
      : !showDebugArtifactsTab && activeTab === 'artifacts'
        ? 'output'
        : activeTab;
  const showWorkflowReliability = developerModeEnabled && developerPanels.workflowReliability;
  const showExecutionLogs = developerModeEnabled && developerPanels.executionLogs;
  const showStreamingOutput = developerModeEnabled && developerPanels.streamingOutput;
  const showAnalysisCoverage = workflowMode !== 'task' && analysisCoverage !== null;

  useEffect(() => {
    void refreshPolicy();
  }, [refreshPolicy]);

  useEffect(() => {
    if (effectiveActiveTab !== 'output' || !showWorkflowReliability) return;
    void refreshObservability();
    const timer = window.setInterval(() => {
      void refreshObservability();
    }, 12000);
    return () => window.clearInterval(timer);
  }, [effectiveActiveTab, refreshObservability, showWorkflowReliability, workflowMode, workflowPhase]);

  const allTabs: { id: RightPanelTab; label: string }[] = [
    { id: 'output', label: t('rightPanel.outputTab', { defaultValue: 'Output' }) },
    { id: 'git', label: t('rightPanel.gitTab', { defaultValue: 'Git' }) },
    { id: 'context', label: t('rightPanel.contextTab', { defaultValue: 'Context' }) },
    { id: 'artifacts', label: t('rightPanel.artifactsTab', { defaultValue: 'Artifacts' }) },
  ];
  const tabs = allTabs.filter((tab) => {
    if (tab.id === 'context') return showContextTab;
    if (tab.id === 'artifacts') return showDebugArtifactsTab;
    return true;
  });

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
              effectiveActiveTab === tab.id
                ? 'text-primary-600 dark:text-primary-400'
                : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
            )}
          >
            {tab.label}
            {/* Active indicator */}
            {effectiveActiveTab === tab.id && (
              <span className="absolute bottom-0 left-1 right-1 h-0.5 bg-primary-600 dark:bg-primary-400 rounded-full" />
            )}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div
        key={effectiveActiveTab}
        className={clsx(
          'flex-1 min-h-0 animate-fade-in',
          effectiveActiveTab === 'output' ? 'overflow-hidden' : 'overflow-y-auto',
        )}
      >
        {effectiveActiveTab === 'output' ? (
          <div className="min-h-0 flex flex-col h-full">
            <div className="shrink-0 space-y-2 p-2 overflow-y-auto border-b border-gray-200 dark:border-gray-700 max-h-[60%]">
              <WorkflowKernelProgressPanel workflowMode={workflowMode} workflowPhase={workflowPhase} />
              {showAnalysisCoverage && analysisCoverage && <AnalysisCoveragePanel coverage={analysisCoverage} />}
              {showWorkflowReliability && <WorkflowFailureSummaryPanel snapshot={observabilitySnapshot} />}
              <ErrorState maxErrors={8} />
            </div>
            <div className="min-h-0 flex-1 flex flex-col overflow-hidden">
              {showExecutionLogs && <ExecutionLogsCard logs={logs} />}
              {showStreamingOutput && (
                <StreamingOutput
                  maxHeight="none"
                  compact={false}
                  showClear={false}
                  className="flex-1 min-h-0 px-2 pb-2"
                  lines={modeTranscriptLines}
                  statusOverride={executionStatus}
                />
              )}
            </div>
          </div>
        ) : effectiveActiveTab === 'git' ? (
          <GitPanel streamingOutput={modeTranscriptLines} workspacePath={workspacePath} rootSessionId={rootSessionId} />
        ) : effectiveActiveTab === 'artifacts' ? (
          <DebugArtifactsPanel sessionId={debugSessionId ?? null} />
        ) : (
          <ContextOpsPanel projectPath={workspacePath} sessionId={contextSessionId} />
        )}
      </div>
    </div>
  );
}

function WorkflowFailureSummaryPanel({
  snapshot,
}: {
  snapshot: ReturnType<typeof useWorkflowObservabilityStore.getState>['snapshot'];
}) {
  const { t } = useTranslation('simpleMode');
  if (!snapshot) return null;
  const latestFailure = snapshot.latestFailure;
  const breakdownPreview = snapshot.interactiveActionFailBreakdown.slice(0, 3);

  return (
    <div className="p-3 rounded-lg bg-rose-50 dark:bg-rose-900/20 border border-rose-200 dark:border-rose-800">
      <p className="text-sm font-medium text-rose-700 dark:text-rose-300">
        {t('rightPanel.workflowFailures.title', { defaultValue: 'Workflow Reliability' })}
      </p>
      <div className="mt-1 text-xs text-rose-700/90 dark:text-rose-300/90">
        {t('rightPanel.workflowFailures.metrics.linkRehydrate', {
          success: snapshot.metrics.workflowLinkRehydrateSuccess,
          total: snapshot.metrics.workflowLinkRehydrateTotal,
          defaultValue: 'Link rehydrate {{success}}/{{total}}',
        })}{' '}
        |{' '}
        {t('rightPanel.workflowFailures.metrics.actionFailures', {
          total: snapshot.metrics.interactiveActionFailTotal,
          defaultValue: 'Action failures {{total}}',
        })}{' '}
        |{' '}
        {t('rightPanel.workflowFailures.metrics.prdFeedback', {
          success: snapshot.metrics.prdFeedbackApplySuccess,
          total: snapshot.metrics.prdFeedbackApplyTotal,
          defaultValue: 'PRD feedback {{success}}/{{total}}',
        })}
      </div>
      {latestFailure ? (
        <details className="mt-2 rounded border border-rose-200 dark:border-rose-800 bg-white/70 dark:bg-gray-950/40">
          <summary className="cursor-pointer list-none px-2 py-1.5 text-xs font-medium text-rose-700 dark:text-rose-300">
            {t('rightPanel.workflowFailures.latest.title', {
              action: latestFailure.action,
              defaultValue: 'Latest: {{action}}',
            })}
            {latestFailure.errorCode ? ` (${latestFailure.errorCode})` : ''}
          </summary>
          <div className="px-2 pb-2 text-2xs text-rose-700/90 dark:text-rose-300/90 space-y-1">
            {latestFailure.message && <div>{latestFailure.message}</div>}
            <div>
              {t('rightPanel.workflowFailures.latest.mode', {
                value: latestFailure.mode ?? '-',
                defaultValue: 'Mode {{value}}',
              })}{' '}
              |{' '}
              {t('rightPanel.workflowFailures.latest.before', {
                value: latestFailure.phaseBefore ?? '-',
                defaultValue: 'Before {{value}}',
              })}{' '}
              |{' '}
              {t('rightPanel.workflowFailures.latest.after', {
                value: latestFailure.phaseAfter ?? '-',
                defaultValue: 'After {{value}}',
              })}
            </div>
            <div>{latestFailure.timestamp}</div>
          </div>
        </details>
      ) : (
        <div className="mt-2 text-2xs text-rose-700/80 dark:text-rose-300/80">
          {t('rightPanel.workflowFailures.none', { defaultValue: 'No recent workflow failures.' })}
        </div>
      )}
      {breakdownPreview.length > 0 && (
        <div className="mt-2 text-2xs text-rose-700/90 dark:text-rose-300/90 space-y-0.5">
          {breakdownPreview.map((item) => (
            <div key={`${item.card}-${item.action}-${item.errorCode}`}>
              {t('rightPanel.workflowFailures.breakdownItem', {
                card: item.card,
                action: item.action,
                errorCode: item.errorCode,
                total: item.total,
                defaultValue: '{{card}} / {{action}} / {{errorCode}}: {{total}}',
              })}
            </div>
          ))}
        </div>
      )}
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
  const [expanded, setExpanded] = useState(false);
  const recent = logs.slice(-40).reverse();
  if (recent.length === 0) return null;
  const visible = expanded ? recent : recent.slice(0, 3);

  return (
    <div className="px-2 pt-2 pb-1">
      <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-2">
        <button
          onClick={() => setExpanded((value) => !value)}
          className="w-full flex items-center justify-between gap-2 text-left"
        >
          <p className="text-xs font-medium text-gray-800 dark:text-gray-200">{t('executionLogs.title')}</p>
          <div className="flex items-center gap-2 text-2xs text-gray-500 dark:text-gray-400">
            <span>{recent.length}</span>
            <span>
              {expanded
                ? t('common.collapse', { defaultValue: 'Collapse' })
                : t('common.expand', { defaultValue: 'Expand' })}
            </span>
          </div>
        </button>
        <div
          className={clsx(
            'mt-2 overflow-y-auto rounded border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-950 p-2 font-mono text-2xs text-gray-700 dark:text-gray-300 space-y-1',
            expanded ? 'max-h-36' : 'max-h-16',
          )}
        >
          {visible.map((line, idx) => (
            <div key={`${idx}-${line.slice(0, 16)}`} className="whitespace-pre-wrap break-words">
              {line}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
