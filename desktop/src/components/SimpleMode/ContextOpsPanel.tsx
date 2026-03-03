import { useCallback, useEffect, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useContextOpsStore } from '../../store/contextOps';
import { useContextSelectionStore } from '../../store/contextSelection';
import { useSettingsStore } from '../../store/settings';

type ContextOpsTab = 'inspector' | 'trace' | 'artifacts' | 'ops';

function fmtPct(value: number, digits = 1): string {
  return `${(value * 100).toFixed(digits)}%`;
}

function fmtTime(value: string | null | undefined): string {
  if (!value) return '-';
  const dt = new Date(value);
  if (Number.isNaN(dt.getTime())) return value;
  return dt.toLocaleString();
}

function StatCard({
  label,
  value,
  tone = 'neutral',
}: {
  label: string;
  value: string;
  tone?: 'neutral' | 'good' | 'warn' | 'bad';
}) {
  const toneClass =
    tone === 'good'
      ? 'border-emerald-200 bg-emerald-50 dark:border-emerald-800 dark:bg-emerald-900/20'
      : tone === 'warn'
        ? 'border-amber-200 bg-amber-50 dark:border-amber-800 dark:bg-amber-900/20'
        : tone === 'bad'
          ? 'border-red-200 bg-red-50 dark:border-red-800 dark:bg-red-900/20'
          : 'border-gray-200 bg-white dark:border-gray-700 dark:bg-gray-900';

  return (
    <div className={clsx('rounded-md border px-3 py-2', toneClass)}>
      <div className="text-2xs uppercase tracking-wide text-gray-500 dark:text-gray-400">{label}</div>
      <div className="mt-1 text-sm font-semibold text-gray-900 dark:text-gray-100">{value}</div>
    </div>
  );
}

export function ContextOpsPanel({ projectPath, sessionId }: { projectPath: string | null; sessionId: string | null }) {
  const { t } = useTranslation('simpleMode');
  const [activeTab, setActiveTab] = useState<ContextOpsTab>('inspector');
  const [traceInput, setTraceInput] = useState('');
  const [artifactName, setArtifactName] = useState('');
  const [chaosIterations, setChaosIterations] = useState(20);
  const [chaosProbability, setChaosProbability] = useState(0.15);

  const latestEnvelope = useContextOpsStore((s) => s.latestEnvelope);
  const unifiedContextSelectionEnabled = useSettingsStore((s) => s.simpleContextUnifiedStore);
  const selectionMismatchCount = useContextSelectionStore((s) => s.uiMeta.mismatchCount);
  const selectionBuildCount = useContextSelectionStore((s) => s.uiMeta.buildCount);
  const selectionDailyStats = useContextSelectionStore((s) => s.uiMeta.dailyStats);
  const selectedTraceId = useContextOpsStore((s) => s.selectedTraceId);
  const traces = useContextOpsStore((s) => s.traces);
  const policy = useContextOpsStore((s) => s.policy);
  const rollout = useContextOpsStore((s) => s.rollout);
  const artifacts = useContextOpsStore((s) => s.artifacts);
  const dashboard = useContextOpsStore((s) => s.dashboard);
  const chaosRuns = useContextOpsStore((s) => s.chaosRuns);
  const lastChaosReport = useContextOpsStore((s) => s.lastChaosReport);
  const isBusy = useContextOpsStore((s) => s.isBusy);
  const error = useContextOpsStore((s) => s.error);

  const selectTrace = useContextOpsStore((s) => s.selectTrace);
  const loadTrace = useContextOpsStore((s) => s.loadTrace);
  const refreshPolicy = useContextOpsStore((s) => s.refreshPolicy);
  const savePolicy = useContextOpsStore((s) => s.savePolicy);
  const refreshRollout = useContextOpsStore((s) => s.refreshRollout);
  const saveRollout = useContextOpsStore((s) => s.saveRollout);
  const loadArtifacts = useContextOpsStore((s) => s.loadArtifacts);
  const saveCurrentEnvelopeAsArtifact = useContextOpsStore((s) => s.saveCurrentEnvelopeAsArtifact);
  const applyArtifact = useContextOpsStore((s) => s.applyArtifact);
  const deleteArtifact = useContextOpsStore((s) => s.deleteArtifact);
  const loadDashboard = useContextOpsStore((s) => s.loadDashboard);
  const runChaosProbe = useContextOpsStore((s) => s.runChaosProbe);
  const loadChaosRuns = useContextOpsStore((s) => s.loadChaosRuns);
  const clearError = useContextOpsStore((s) => s.clearError);

  const effectiveProjectPath = projectPath?.trim() ?? '';

  useEffect(() => {
    void refreshPolicy();
    void refreshRollout();
  }, [refreshPolicy, refreshRollout]);

  useEffect(() => {
    if (!effectiveProjectPath) return;
    void loadArtifacts(effectiveProjectPath, sessionId);
    void loadDashboard(effectiveProjectPath, 24);
    void loadChaosRuns(effectiveProjectPath, 20);
  }, [effectiveProjectPath, sessionId, loadArtifacts, loadDashboard, loadChaosRuns]);

  useEffect(() => {
    if (!latestEnvelope?.trace_id) return;
    setTraceInput(latestEnvelope.trace_id);
    selectTrace(latestEnvelope.trace_id);
    void loadTrace(latestEnvelope.trace_id);
  }, [latestEnvelope?.trace_id, selectTrace, loadTrace]);

  const activeTrace = selectedTraceId ? (traces[selectedTraceId] ?? null) : null;

  const tabButtonClass = (tab: ContextOpsTab) =>
    clsx(
      'px-2.5 py-1.5 text-xs rounded-md transition-colors',
      activeTab === tab
        ? 'bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-300'
        : 'text-gray-600 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-800',
    );

  const togglePinned = useCallback(
    async (sourceId: string) => {
      const selector = `id:${sourceId}`;
      const isPinned = policy.pinned_sources.includes(selector);
      const next = isPinned
        ? policy.pinned_sources.filter((s) => s !== selector)
        : [...policy.pinned_sources, selector];
      await savePolicy({ pinned_sources: next });
    },
    [policy.pinned_sources, savePolicy],
  );

  const toggleExcluded = useCallback(
    async (sourceId: string) => {
      const selector = `id:${sourceId}`;
      const isExcluded = policy.excluded_sources.includes(selector);
      const next = isExcluded
        ? policy.excluded_sources.filter((s) => s !== selector)
        : [...policy.excluded_sources, selector];
      await savePolicy({ excluded_sources: next });
    },
    [policy.excluded_sources, savePolicy],
  );

  const sourceRows = latestEnvelope?.sources ?? [];
  const mismatchRate = selectionBuildCount > 0 ? (selectionMismatchCount / selectionBuildCount) * 100 : 0;
  const now = new Date();
  const sevenDaysAgo = new Date(now);
  sevenDaysAgo.setDate(now.getDate() - 6);
  const recentSevenDayStats = selectionDailyStats.filter((item) => {
    const date = new Date(`${item.date}T00:00:00`);
    return !Number.isNaN(date.getTime()) && date >= sevenDaysAgo && date <= now;
  });
  const sevenDayBuildCount = recentSevenDayStats.reduce((sum, item) => sum + item.buildCount, 0);
  const sevenDayMismatchCount = recentSevenDayStats.reduce((sum, item) => sum + item.mismatchCount, 0);
  const sevenDayMismatchRate = sevenDayBuildCount > 0 ? (sevenDayMismatchCount / sevenDayBuildCount) * 100 : 0;

  const inspectorContent = (
    <div className="space-y-3">
      {!latestEnvelope ? (
        <div className="text-xs text-gray-500 dark:text-gray-400">
          {t('rightPanel.contextOps.empty.envelope', {
            defaultValue: 'No context envelope captured yet. Send a message to generate one.',
          })}
        </div>
      ) : (
        <>
          <div className="grid grid-cols-2 gap-2">
            <StatCard
              label={t('rightPanel.contextOps.stats.trace', { defaultValue: 'Trace' })}
              value={latestEnvelope.trace_id.slice(0, 8)}
              tone={latestEnvelope.budget.over_budget ? 'bad' : 'good'}
            />
            <StatCard
              label={t('rightPanel.contextOps.stats.budget', { defaultValue: 'Budget' })}
              value={`${latestEnvelope.budget.used_input_tokens}/${latestEnvelope.budget.input_token_budget}`}
              tone={latestEnvelope.budget.over_budget ? 'warn' : 'good'}
            />
            <StatCard
              label={t('rightPanel.contextOps.stats.compaction', { defaultValue: 'Compaction' })}
              value={
                latestEnvelope.compaction.triggered
                  ? `${latestEnvelope.compaction.strategy} (${latestEnvelope.compaction.net_saving})`
                  : t('rightPanel.contextOps.common.none', { defaultValue: 'none' })
              }
              tone={latestEnvelope.compaction.triggered ? 'warn' : 'neutral'}
            />
            <StatCard
              label={t('rightPanel.contextOps.stats.mode', { defaultValue: 'Mode' })}
              value={latestEnvelope.request_meta.mode}
            />
            <StatCard
              label={t('rightPanel.contextOps.stats.selectionStore', { defaultValue: 'Selection Store' })}
              value={unifiedContextSelectionEnabled ? 'unified' : 'legacy'}
              tone={unifiedContextSelectionEnabled ? 'good' : 'warn'}
            />
            <StatCard
              label={t('rightPanel.contextOps.stats.selectionOrigin', { defaultValue: 'Selection Origin' })}
              value={latestEnvelope.diagnostics?.selection_origin || '-'}
            />
            <StatCard
              label={t('rightPanel.contextOps.stats.selectionMismatch', { defaultValue: 'Selection Mismatch' })}
              value={String(selectionMismatchCount)}
              tone={selectionMismatchCount > 0 ? 'warn' : 'good'}
            />
            <StatCard
              label={t('rightPanel.contextOps.stats.selectionMismatchRate', { defaultValue: 'Mismatch Rate' })}
              value={`${mismatchRate.toFixed(2)}% (${selectionMismatchCount}/${selectionBuildCount})`}
              tone={mismatchRate > 0.5 ? 'warn' : 'good'}
            />
            <StatCard
              label={t('rightPanel.contextOps.stats.selectionMismatchRate7d', { defaultValue: 'Mismatch Rate (7d)' })}
              value={`${sevenDayMismatchRate.toFixed(2)}% (${sevenDayMismatchCount}/${sevenDayBuildCount})`}
              tone={sevenDayMismatchRate > 0.5 ? 'warn' : 'good'}
            />
          </div>

          <div className="rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden">
            <div className="px-3 py-2 text-xs font-medium bg-gray-50 dark:bg-gray-800/70 border-b border-gray-200 dark:border-gray-700">
              {t('rightPanel.contextOps.sources.title', {
                defaultValue: 'Sources ({{count}})',
                count: sourceRows.length,
              })}
            </div>
            <div className="max-h-60 overflow-y-auto divide-y divide-gray-200 dark:divide-gray-700">
              {sourceRows.map((source) => {
                const pinSelector = `id:${source.id}`;
                const pinned = policy.pinned_sources.includes(pinSelector);
                const excluded = policy.excluded_sources.includes(pinSelector);
                return (
                  <div key={source.id} className="px-3 py-2 text-xs">
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <div className="font-medium text-gray-900 dark:text-gray-100 truncate">{source.label}</div>
                        <div className="text-gray-500 dark:text-gray-400">
                          {source.kind} · {source.token_cost}{' '}
                          {t('rightPanel.contextOps.sources.tokenShort', { defaultValue: 'tok' })} · {source.reason}
                        </div>
                      </div>
                      <div className="flex items-center gap-1 shrink-0">
                        <button
                          onClick={() => void togglePinned(source.id)}
                          className={clsx(
                            'px-2 py-0.5 rounded border',
                            pinned
                              ? 'border-primary-300 text-primary-700 dark:border-primary-700 dark:text-primary-300'
                              : 'border-gray-300 text-gray-600 dark:border-gray-600 dark:text-gray-300',
                          )}
                        >
                          {pinned
                            ? t('rightPanel.contextOps.actions.pinned', { defaultValue: 'Pinned' })
                            : t('rightPanel.contextOps.actions.pin', { defaultValue: 'Pin' })}
                        </button>
                        <button
                          onClick={() => void toggleExcluded(source.id)}
                          className={clsx(
                            'px-2 py-0.5 rounded border',
                            excluded
                              ? 'border-red-300 text-red-700 dark:border-red-700 dark:text-red-300'
                              : 'border-gray-300 text-gray-600 dark:border-gray-600 dark:text-gray-300',
                          )}
                        >
                          {excluded
                            ? t('rightPanel.contextOps.actions.excluded', { defaultValue: 'Excluded' })
                            : t('rightPanel.contextOps.actions.exclude', { defaultValue: 'Exclude' })}
                        </button>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </>
      )}
    </div>
  );

  const traceContent = (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <input
          value={traceInput}
          onChange={(e) => setTraceInput(e.target.value)}
          placeholder={t('rightPanel.contextOps.trace.placeholder', { defaultValue: 'trace_id' })}
          className="flex-1 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 px-2 py-1 text-xs"
        />
        <button
          onClick={() => {
            const id = traceInput.trim();
            if (!id) return;
            selectTrace(id);
            void loadTrace(id);
          }}
          className="px-2 py-1 text-xs rounded bg-primary-600 text-white hover:bg-primary-700"
        >
          {t('rightPanel.contextOps.actions.load', { defaultValue: 'Load' })}
        </button>
      </div>

      {!activeTrace ? (
        <div className="text-xs text-gray-500 dark:text-gray-400">
          {t('rightPanel.contextOps.empty.trace', { defaultValue: 'No trace loaded.' })}
        </div>
      ) : (
        <div className="rounded-md border border-gray-200 dark:border-gray-700 max-h-[420px] overflow-y-auto">
          {activeTrace.events.map((event, idx) => (
            <div
              key={`${event.trace_id}-${event.created_at}-${idx}`}
              className="px-3 py-2 border-b border-gray-200 dark:border-gray-700 last:border-b-0"
            >
              <div className="text-2xs text-gray-500 dark:text-gray-400">
                {fmtTime(event.created_at)} · {event.event_type}
                {event.source_kind ? ` · ${event.source_kind}` : ''}
                {event.source_id ? ` · ${event.source_id}` : ''}
              </div>
              <div className="mt-1 text-xs text-gray-900 dark:text-gray-100">{event.message}</div>
              {event.metadata && (
                <pre className="mt-1 text-2xs text-gray-600 dark:text-gray-300 whitespace-pre-wrap break-words bg-gray-50 dark:bg-gray-900/70 rounded p-2">
                  {JSON.stringify(event.metadata, null, 2)}
                </pre>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );

  const artifactsContent = (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <input
          value={artifactName}
          onChange={(e) => setArtifactName(e.target.value)}
          placeholder={t('rightPanel.contextOps.artifacts.namePlaceholder', { defaultValue: 'artifact name' })}
          className="flex-1 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 px-2 py-1 text-xs"
        />
        <button
          onClick={() => {
            if (!effectiveProjectPath) return;
            void saveCurrentEnvelopeAsArtifact(artifactName, effectiveProjectPath, sessionId).then((ok) => {
              if (ok) setArtifactName('');
            });
          }}
          className="px-2 py-1 text-xs rounded bg-primary-600 text-white hover:bg-primary-700 disabled:opacity-60"
          disabled={!latestEnvelope || !artifactName.trim() || !effectiveProjectPath}
        >
          {t('rightPanel.contextOps.actions.save', { defaultValue: 'Save' })}
        </button>
        <button
          onClick={() => {
            if (!effectiveProjectPath) return;
            void loadArtifacts(effectiveProjectPath, sessionId);
          }}
          className="px-2 py-1 text-xs rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300"
          disabled={!effectiveProjectPath}
        >
          {t('rightPanel.contextOps.actions.refresh', { defaultValue: 'Refresh' })}
        </button>
      </div>

      <div className="rounded-md border border-gray-200 dark:border-gray-700 max-h-[420px] overflow-y-auto divide-y divide-gray-200 dark:divide-gray-700">
        {artifacts.length === 0 ? (
          <div className="px-3 py-3 text-xs text-gray-500 dark:text-gray-400">
            {t('rightPanel.contextOps.empty.artifacts', { defaultValue: 'No artifacts.' })}
          </div>
        ) : (
          artifacts.map((artifact) => (
            <div key={artifact.id} className="px-3 py-2 text-xs">
              <div className="font-medium text-gray-900 dark:text-gray-100">{artifact.name}</div>
              <div className="text-gray-500 dark:text-gray-400">
                {artifact.id.slice(0, 8)} · {fmtTime(artifact.updated_at)}
              </div>
              <div className="mt-2 flex items-center gap-2">
                <button
                  onClick={() => void applyArtifact(artifact.id, sessionId)}
                  className="px-2 py-0.5 rounded border border-primary-300 text-primary-700 dark:border-primary-700 dark:text-primary-300"
                >
                  {t('rightPanel.contextOps.actions.apply', { defaultValue: 'Apply' })}
                </button>
                <button
                  onClick={() => void deleteArtifact(artifact.id, effectiveProjectPath, sessionId)}
                  className="px-2 py-0.5 rounded border border-red-300 text-red-700 dark:border-red-700 dark:text-red-300"
                >
                  {t('rightPanel.contextOps.actions.delete', { defaultValue: 'Delete' })}
                </button>
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );

  const opsContent = (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-2">
        <StatCard
          label={t('rightPanel.contextOps.stats.availability', { defaultValue: 'Availability' })}
          value={dashboard ? fmtPct(dashboard.availability, 2) : '-'}
          tone={dashboard && dashboard.availability >= 0.999 ? 'good' : 'warn'}
        />
        <StatCard
          label={t('rightPanel.contextOps.stats.degradedRate', { defaultValue: 'Degraded Rate' })}
          value={dashboard ? fmtPct(dashboard.degraded_rate, 2) : '-'}
          tone={dashboard && dashboard.degraded_rate <= 0.05 ? 'good' : 'bad'}
        />
        <StatCard
          label={t('rightPanel.contextOps.stats.p95Latency', { defaultValue: 'P95 Latency' })}
          value={dashboard ? `${dashboard.prepare_context_p95_ms.toFixed(1)}ms` : '-'}
          tone={dashboard && dashboard.prepare_context_p95_ms <= 300 ? 'good' : 'bad'}
        />
        <StatCard
          label={t('rightPanel.contextOps.stats.traces', { defaultValue: 'Traces' })}
          value={dashboard ? String(dashboard.total_traces) : '-'}
        />
        <StatCard
          label={t('rightPanel.contextOps.stats.memoryP95', { defaultValue: 'Memory P95' })}
          value={dashboard ? `${dashboard.memory_query_p95_ms.toFixed(1)}ms` : '-'}
          tone={dashboard && dashboard.memory_query_p95_ms <= 300 ? 'good' : 'bad'}
        />
        <StatCard
          label={t('rightPanel.contextOps.stats.emptyHitRate', { defaultValue: 'Empty Hit Rate' })}
          value={dashboard ? fmtPct(dashboard.empty_hit_rate, 2) : '-'}
          tone={dashboard && dashboard.empty_hit_rate <= 0.2 ? 'good' : 'warn'}
        />
        <StatCard
          label={t('rightPanel.contextOps.stats.candidateCount', { defaultValue: 'Candidate Count' })}
          value={dashboard ? dashboard.candidate_count.toFixed(1) : '-'}
        />
        <StatCard
          label={t('rightPanel.contextOps.stats.reviewBacklog', { defaultValue: 'Review Backlog' })}
          value={dashboard ? String(dashboard.review_backlog) : '-'}
          tone={dashboard && dashboard.review_backlog <= 200 ? 'good' : 'bad'}
        />
        <StatCard
          label={t('rightPanel.contextOps.stats.approveRate', { defaultValue: 'Approve Rate' })}
          value={dashboard ? fmtPct(dashboard.approve_rate, 2) : '-'}
          tone={dashboard && dashboard.approve_rate >= 0.7 ? 'good' : 'warn'}
        />
        <StatCard
          label={t('rightPanel.contextOps.stats.rejectRate', { defaultValue: 'Reject Rate' })}
          value={dashboard ? fmtPct(dashboard.reject_rate, 2) : '-'}
          tone={dashboard && dashboard.reject_rate <= 0.3 ? 'good' : 'warn'}
        />
      </div>

      {dashboard?.alerts && dashboard.alerts.length > 0 && (
        <div className="rounded-md border border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/20 px-3 py-2">
          <div className="text-xs font-medium text-amber-800 dark:text-amber-200">
            {t('rightPanel.contextOps.ops.alerts', { defaultValue: 'Alerts' })}
          </div>
          <ul className="mt-1 space-y-1 text-xs text-amber-700 dark:text-amber-300">
            {dashboard.alerts.map((alert) => (
              <li key={alert.code}>
                [{alert.severity}] {alert.message} ({alert.value.toFixed(3)} / {alert.threshold})
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="rounded-md border border-gray-200 dark:border-gray-700 px-3 py-2 space-y-2">
        <div className="text-xs font-medium text-gray-800 dark:text-gray-200">
          {t('rightPanel.contextOps.ops.featureFlags', { defaultValue: 'Feature Flags' })}
        </div>
        <label className="flex items-center justify-between text-xs">
          <span>context_v2_pipeline</span>
          <input
            type="checkbox"
            checked={policy.context_v2_pipeline}
            onChange={(e) => void savePolicy({ context_v2_pipeline: e.target.checked })}
          />
        </label>
        <label className="flex items-center justify-between text-xs">
          <span>memory_v2_ranker</span>
          <input
            type="checkbox"
            checked={policy.memory_v2_ranker}
            onChange={(e) => void savePolicy({ memory_v2_ranker: e.target.checked })}
          />
        </label>
        <label className="flex items-center justify-between text-xs">
          <span>context_inspector_ui</span>
          <input
            type="checkbox"
            checked={policy.context_inspector_ui}
            onChange={(e) => void savePolicy({ context_inspector_ui: e.target.checked })}
          />
        </label>
      </div>

      <div className="rounded-md border border-gray-200 dark:border-gray-700 px-3 py-2 space-y-2">
        <div className="text-xs font-medium text-gray-800 dark:text-gray-200">
          {t('rightPanel.contextOps.ops.rolloutTitle', { defaultValue: 'Rollout / A-B' })}
        </div>
        <label className="flex items-center justify-between text-xs">
          <span>{t('rightPanel.contextOps.ops.enabled', { defaultValue: 'Enabled' })}</span>
          <input
            type="checkbox"
            checked={rollout.enabled}
            onChange={(e) => void saveRollout({ enabled: e.target.checked })}
          />
        </label>
        <label className="block text-xs">
          <span className="text-gray-600 dark:text-gray-300">
            {t('rightPanel.contextOps.ops.rolloutPercent', {
              defaultValue: 'Rollout %: {{value}}',
              value: rollout.rollout_percentage,
            })}
          </span>
          <input
            type="range"
            min={0}
            max={100}
            value={rollout.rollout_percentage}
            onChange={(e) => void saveRollout({ rollout_percentage: Number(e.target.value) })}
            className="w-full"
          />
        </label>
        <label className="block text-xs">
          <span className="text-gray-600 dark:text-gray-300">
            {t('rightPanel.contextOps.ops.abMode', { defaultValue: 'A/B mode' })}
          </span>
          <select
            value={rollout.ab_mode}
            onChange={(e) => void saveRollout({ ab_mode: e.target.value })}
            className="mt-1 w-full rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 px-2 py-1 text-xs"
          >
            <option value="off">{t('rightPanel.contextOps.ops.abModes.off', { defaultValue: 'off' })}</option>
            <option value="shadow">{t('rightPanel.contextOps.ops.abModes.shadow', { defaultValue: 'shadow' })}</option>
            <option value="split">{t('rightPanel.contextOps.ops.abModes.split', { defaultValue: 'split' })}</option>
          </select>
        </label>
      </div>

      <div className="rounded-md border border-gray-200 dark:border-gray-700 px-3 py-2 space-y-2">
        <div className="text-xs font-medium text-gray-800 dark:text-gray-200">
          {t('rightPanel.contextOps.ops.chaosProbe', { defaultValue: 'Chaos Probe' })}
        </div>
        <div className="flex items-center gap-2">
          <label className="text-xs text-gray-600 dark:text-gray-300">
            {t('rightPanel.contextOps.ops.iterations', { defaultValue: 'Iterations' })}
            <input
              type="number"
              min={1}
              max={500}
              value={chaosIterations}
              onChange={(e) => setChaosIterations(Math.max(1, Number(e.target.value) || 1))}
              className="ml-2 w-20 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 px-2 py-1 text-xs"
            />
          </label>
          <label className="text-xs text-gray-600 dark:text-gray-300">
            {t('rightPanel.contextOps.ops.failureProbability', { defaultValue: 'Failure p' })}
            <input
              type="number"
              min={0}
              max={1}
              step={0.01}
              value={chaosProbability}
              onChange={(e) => setChaosProbability(Math.max(0, Math.min(1, Number(e.target.value) || 0)))}
              className="ml-2 w-20 rounded border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 px-2 py-1 text-xs"
            />
          </label>
          <button
            onClick={() => {
              if (!effectiveProjectPath) return;
              void runChaosProbe(effectiveProjectPath, sessionId, chaosIterations, chaosProbability);
            }}
            disabled={!effectiveProjectPath || isBusy}
            className="px-2 py-1 text-xs rounded bg-primary-600 text-white hover:bg-primary-700 disabled:opacity-60"
          >
            {isBusy
              ? t('rightPanel.contextOps.actions.running', { defaultValue: 'Running...' })
              : t('rightPanel.contextOps.actions.run', { defaultValue: 'Run' })}
          </button>
        </div>
        {lastChaosReport && (
          <div className="text-xs text-gray-700 dark:text-gray-300">
            {t('rightPanel.contextOps.ops.lastRun', {
              defaultValue: 'last: {{runId}} · injected={{injected}} · fallback={{fallback}}',
              runId: lastChaosReport.run_id.slice(0, 8),
              injected: lastChaosReport.injected_faults,
              fallback: fmtPct(lastChaosReport.fallback_success_rate),
            })}
          </div>
        )}
      </div>

      <div className="rounded-md border border-gray-200 dark:border-gray-700 px-3 py-2">
        <div className="text-xs font-medium text-gray-800 dark:text-gray-200">
          {t('rightPanel.contextOps.ops.runbook', { defaultValue: 'Runbook' })}
        </div>
        <div className="mt-1 text-xs text-gray-600 dark:text-gray-300 break-all">
          {dashboard?.runbook_path ?? 'docs/Context-V2-Incident-Runbook.md'}
        </div>
      </div>

      {chaosRuns.length > 0 && (
        <div className="rounded-md border border-gray-200 dark:border-gray-700 px-3 py-2">
          <div className="text-xs font-medium text-gray-800 dark:text-gray-200">
            {t('rightPanel.contextOps.ops.recentChaosRuns', { defaultValue: 'Recent Chaos Runs' })}
          </div>
          <div className="mt-2 space-y-1 max-h-36 overflow-y-auto">
            {chaosRuns.map((run) => (
              <div key={run.run_id} className="text-xs text-gray-700 dark:text-gray-300">
                {t('rightPanel.contextOps.ops.chaosRunItem', {
                  defaultValue: '{{time}} · {{runId}} · iter={{iterations}} · fallback={{fallback}}',
                  time: fmtTime(run.created_at),
                  runId: run.run_id.slice(0, 8),
                  iterations: run.iterations,
                  fallback: fmtPct(run.fallback_success_rate),
                })}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );

  let visibleContent = inspectorContent;
  if (activeTab === 'trace') visibleContent = traceContent;
  else if (activeTab === 'artifacts') visibleContent = artifactsContent;
  else if (activeTab === 'ops') visibleContent = opsContent;

  return (
    <div className="h-full flex flex-col">
      <div className="shrink-0 border-b border-gray-200 dark:border-gray-700 p-2 space-y-2">
        <div className="flex items-center gap-1">
          <button className={tabButtonClass('inspector')} onClick={() => setActiveTab('inspector')}>
            {t('rightPanel.contextOps.tabs.inspector', { defaultValue: 'Inspector' })}
          </button>
          <button className={tabButtonClass('trace')} onClick={() => setActiveTab('trace')}>
            {t('rightPanel.contextOps.tabs.trace', { defaultValue: 'Trace' })}
          </button>
          <button className={tabButtonClass('artifacts')} onClick={() => setActiveTab('artifacts')}>
            {t('rightPanel.contextOps.tabs.artifacts', { defaultValue: 'Artifacts' })}
          </button>
          <button className={tabButtonClass('ops')} onClick={() => setActiveTab('ops')}>
            {t('rightPanel.contextOps.tabs.ops', { defaultValue: 'Ops' })}
          </button>
          <button
            className="ml-auto px-2 py-1 text-xs rounded border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300"
            onClick={() => {
              if (!effectiveProjectPath) return;
              void loadDashboard(effectiveProjectPath, dashboard?.window_hours ?? 24);
              void loadArtifacts(effectiveProjectPath, sessionId);
              if (selectedTraceId) void loadTrace(selectedTraceId);
            }}
            disabled={!effectiveProjectPath}
          >
            {t('rightPanel.contextOps.actions.refresh', { defaultValue: 'Refresh' })}
          </button>
        </div>
        <div className="text-2xs text-gray-500 dark:text-gray-400 truncate">
          {t('rightPanel.contextOps.meta.projectSession', {
            defaultValue: 'project: {{project}} | session: {{session}}',
            project: effectiveProjectPath || '-',
            session: sessionId || '-',
          })}
        </div>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto p-2">{visibleContent}</div>

      {error && (
        <div className="shrink-0 border-t border-red-200 dark:border-red-800 bg-red-50 dark:bg-red-900/20 px-3 py-2 flex items-center gap-2">
          <div className="text-xs text-red-700 dark:text-red-300 flex-1">{error}</div>
          <button className="text-xs text-red-700 dark:text-red-300 underline" onClick={clearError}>
            {t('rightPanel.contextOps.actions.dismiss', { defaultValue: 'dismiss' })}
          </button>
        </div>
      )}
    </div>
  );
}

export default ContextOpsPanel;
