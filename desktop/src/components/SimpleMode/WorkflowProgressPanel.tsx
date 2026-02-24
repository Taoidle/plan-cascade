/**
 * WorkflowProgressPanel
 *
 * Full workflow progress panel for the output sidebar in Task mode.
 * Shows phase timeline, batch progress, story status grid, and quality gate summary.
 */

import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { useWorkflowOrchestratorStore } from '../../store/workflowOrchestrator';
import { useTaskModeStore, type GateStatus } from '../../store/taskMode';
import type { WorkflowPhase } from '../../types/workflowCard';

const PHASE_STEPS: { phase: WorkflowPhase; labelKey: string }[] = [
  { phase: 'analyzing', labelKey: 'workflow.progress.phaseAnalyze' },
  { phase: 'configuring', labelKey: 'workflow.progress.phaseConfig' },
  { phase: 'interviewing', labelKey: 'workflow.progress.phaseInterview' },
  { phase: 'generating_prd', labelKey: 'workflow.progress.phasePrd' },
  { phase: 'reviewing_prd', labelKey: 'workflow.progress.phaseReview' },
  { phase: 'generating_design_doc', labelKey: 'workflow.progress.phaseDesignDoc' },
  { phase: 'executing', labelKey: 'workflow.progress.phaseExecute' },
  { phase: 'completed', labelKey: 'workflow.progress.phaseDone' },
];

const PHASE_ORDER: WorkflowPhase[] = PHASE_STEPS.map((s) => s.phase);

export function WorkflowProgressPanel() {
  const phase = useWorkflowOrchestratorStore((s) => s.phase);
  const config = useWorkflowOrchestratorStore((s) => s.config);
  const storyStatuses = useTaskModeStore((s) => s.storyStatuses);
  const qualityGateResults = useTaskModeStore((s) => s.qualityGateResults);
  const currentBatch = useTaskModeStore((s) => s.currentBatch);
  const totalBatches = useTaskModeStore((s) => s.totalBatches);
  const prd = useTaskModeStore((s) => s.prd);

  // Filter out interview step if not enabled
  const visibleSteps = config.specInterviewEnabled
    ? PHASE_STEPS
    : PHASE_STEPS.filter((s) => s.phase !== 'interviewing');

  return (
    <div className="p-3 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 space-y-3">
      {/* Phase Timeline */}
      <WorkflowPhaseTimeline currentPhase={phase} steps={visibleSteps} />

      {/* Batch Progress (only during/after execution) */}
      {(phase === 'executing' || phase === 'completed' || phase === 'failed') && (
        <BatchProgressSection
          currentBatch={currentBatch}
          totalBatches={totalBatches}
          storyStatuses={storyStatuses}
          prd={prd}
        />
      )}

      {/* Quality Gate Summary */}
      {Object.keys(qualityGateResults).length > 0 && (
        <QualityGateSummary results={qualityGateResults} />
      )}
    </div>
  );
}

function WorkflowPhaseTimeline({
  currentPhase,
  steps,
}: {
  currentPhase: WorkflowPhase;
  steps: { phase: WorkflowPhase; labelKey: string }[];
}) {
  const { t } = useTranslation('simpleMode');
  const currentIndex = PHASE_ORDER.indexOf(currentPhase);

  return (
    <div>
      <p className="text-xs font-medium text-gray-700 dark:text-gray-300 mb-1.5">{t('workflow.progress.title')}</p>
      <div className="flex items-center gap-0.5">
        {steps.map((step, i) => {
          const stepIndex = PHASE_ORDER.indexOf(step.phase);
          const isActive = step.phase === currentPhase;
          const isCompleted = stepIndex < currentIndex;
          const isFailed = (currentPhase === 'failed' || currentPhase === 'cancelled') && isActive;

          return (
            <div key={step.phase} className="flex items-center flex-1">
              <div className="flex flex-col items-center flex-1">
                <div
                  className={clsx(
                    'w-full h-1.5 rounded-full transition-colors',
                    isCompleted
                      ? 'bg-green-500'
                      : isActive
                        ? isFailed
                          ? 'bg-red-500'
                          : 'bg-blue-500 animate-pulse'
                        : 'bg-gray-200 dark:bg-gray-700'
                  )}
                />
                <span
                  className={clsx(
                    'text-2xs mt-1 transition-colors',
                    isActive
                      ? isFailed
                        ? 'text-red-600 dark:text-red-400 font-medium'
                        : 'text-blue-600 dark:text-blue-400 font-medium'
                      : isCompleted
                        ? 'text-green-600 dark:text-green-400'
                        : 'text-gray-400 dark:text-gray-500'
                  )}
                >
                  {t(step.labelKey)}
                </span>
              </div>
              {i < steps.length - 1 && <div className="w-0.5" />}
            </div>
          );
        })}
      </div>
    </div>
  );
}

function BatchProgressSection({
  currentBatch,
  totalBatches,
  storyStatuses,
  prd,
}: {
  currentBatch: number;
  totalBatches: number;
  storyStatuses: Record<string, string>;
  prd: ReturnType<typeof useTaskModeStore.getState>['prd'];
}) {
  const { t } = useTranslation('simpleMode');
  const stories = prd?.stories ?? [];
  const statusEntries = Object.entries(storyStatuses ?? {});
  const completed = statusEntries.filter(([, s]) => s === 'completed').length;
  const failed = statusEntries.filter(([, s]) => s === 'failed').length;
  const running = statusEntries.filter(([, s]) => s === 'running' || s === 'executing').length;
  const total = stories.length || statusEntries.length;

  return (
    <div>
      <div className="flex items-center justify-between">
        <p className="text-xs font-medium text-gray-700 dark:text-gray-300">
          {t('workflow.progress.batch', { current: Math.min(currentBatch + 1, totalBatches), total: totalBatches })}
        </p>
        <span className="text-2xs text-gray-500 dark:text-gray-400">
          {t('workflow.progress.storiesDone', { completed, total })}
        </span>
      </div>

      {/* Story status grid */}
      <div className="mt-1.5 grid grid-cols-2 gap-1">
        {stories.map((story) => {
          const status = storyStatuses[story.id] || 'pending';
          return (
            <StoryStatusCard
              key={story.id}
              storyId={story.id}
              storyTitle={story.title}
              status={status}
            />
          );
        })}
      </div>

      {/* Summary bar */}
      <div className="mt-2 flex items-center gap-3 text-2xs">
        {running > 0 && (
          <span className="flex items-center gap-1 text-blue-600 dark:text-blue-400">
            <span className="w-1.5 h-1.5 rounded-full bg-blue-500 animate-pulse" />
            {t('workflow.progress.running', { count: running })}
          </span>
        )}
        {completed > 0 && (
          <span className="text-green-600 dark:text-green-400">{t('workflow.progress.completed', { count: completed })}</span>
        )}
        {failed > 0 && (
          <span className="text-red-600 dark:text-red-400">{t('workflow.progress.failed', { count: failed })}</span>
        )}
      </div>
    </div>
  );
}

function StoryStatusCard({
  storyId,
  storyTitle,
  status,
}: {
  storyId: string;
  storyTitle: string;
  status: string;
}) {
  const statusConfig = {
    pending: { bg: 'bg-gray-50 dark:bg-gray-800', text: 'text-gray-500 dark:text-gray-400', badge: 'bg-gray-200 dark:bg-gray-700' },
    running: { bg: 'bg-blue-50 dark:bg-blue-900/20', text: 'text-blue-700 dark:text-blue-300', badge: 'bg-blue-200 dark:bg-blue-800' },
    executing: { bg: 'bg-blue-50 dark:bg-blue-900/20', text: 'text-blue-700 dark:text-blue-300', badge: 'bg-blue-200 dark:bg-blue-800' },
    completed: { bg: 'bg-green-50 dark:bg-green-900/20', text: 'text-green-700 dark:text-green-300', badge: 'bg-green-200 dark:bg-green-800' },
    failed: { bg: 'bg-red-50 dark:bg-red-900/20', text: 'text-red-700 dark:text-red-300', badge: 'bg-red-200 dark:bg-red-800' },
    cancelled: { bg: 'bg-gray-50 dark:bg-gray-800', text: 'text-gray-500 dark:text-gray-400', badge: 'bg-gray-200 dark:bg-gray-700' },
  }[status] || { bg: 'bg-gray-50 dark:bg-gray-800', text: 'text-gray-500 dark:text-gray-400', badge: 'bg-gray-200 dark:bg-gray-700' };

  return (
    <div className={clsx('px-2 py-1 rounded', statusConfig.bg)}>
      <div className="flex items-center justify-between">
        <span className={clsx('text-2xs truncate flex-1', statusConfig.text)} title={storyTitle}>
          {storyTitle}
        </span>
        <span className={clsx('text-2xs px-1 py-0.5 rounded ml-1 shrink-0', statusConfig.badge, statusConfig.text)}>
          {status}
        </span>
      </div>
      <span className="text-2xs text-gray-400 dark:text-gray-500">{storyId}</span>
    </div>
  );
}

function QualityGateSummary({
  results,
}: {
  results: Record<string, { overallStatus: GateStatus; gates: unknown[] }>;
}) {
  const { t } = useTranslation('simpleMode');
  const entries = Object.values(results);
  const passed = entries.filter((r) => r.overallStatus === 'passed').length;
  const failed = entries.filter((r) => r.overallStatus === 'failed').length;
  const total = entries.length;

  if (total === 0) return null;

  return (
    <div>
      <p className="text-xs font-medium text-gray-700 dark:text-gray-300">{t('workflow.progress.qualityGates')}</p>
      <div className="mt-1 flex items-center gap-3 text-xs">
        <span className="text-green-600 dark:text-green-400">{t('workflow.progress.passed', { count: passed })}</span>
        {failed > 0 && <span className="text-red-600 dark:text-red-400">{t('workflow.progress.failed', { count: failed })}</span>}
        <span className="text-gray-500 dark:text-gray-400">{t('workflow.progress.total', { count: total })}</span>
      </div>

      {/* Pass rate bar */}
      <div className="mt-1 h-1.5 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
        <div
          className="h-full bg-green-500 rounded-full transition-all"
          style={{ width: `${total > 0 ? (passed / total) * 100 : 0}%` }}
        />
      </div>
    </div>
  );
}
