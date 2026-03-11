/**
 * Token Breakdown Component
 *
 * Displays analytics usage breakdown across multiple dimensions.
 */

import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { AnalyticsBreakdownRow, ModelUsage, ProjectUsage } from '../../store/analytics';
import { formatCost, formatTokens } from '../../store/analytics';
import { phaseLabel, scopeLabel, workflowLabel } from './analyticsLabels';

type BreakdownTab = 'model' | 'project' | 'workflow' | 'phase' | 'scope';

interface TokenBreakdownProps {
  byModel: ModelUsage[];
  byProject: ProjectUsage[];
  byWorkflow: AnalyticsBreakdownRow[];
  byPhase: AnalyticsBreakdownRow[];
  byScope: AnalyticsBreakdownRow[];
}

const COLORS = ['#3B82F6', '#10B981', '#F59E0B', '#EF4444', '#8B5CF6', '#EC4899', '#06B6D4', '#84CC16'];

export function TokenBreakdown({ byModel, byProject, byWorkflow, byPhase, byScope }: TokenBreakdownProps) {
  const { t } = useTranslation('analytics');
  const [activeTab, setActiveTab] = useState<BreakdownTab>('model');

  const tabs: Array<{ id: BreakdownTab; label: string }> = [
    { id: 'model', label: t('breakdown.byModel', 'By Model') },
    { id: 'project', label: t('breakdown.byProject', 'By Project') },
    { id: 'workflow', label: t('breakdown.byWorkflow', 'By Workflow') },
    { id: 'phase', label: t('breakdown.byPhase', 'By Phase') },
    { id: 'scope', label: t('breakdown.byScope', 'By Scope') },
  ];

  return (
    <div>
      <div className="flex flex-wrap gap-2 mb-4">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={clsx(
              'px-3 py-1.5 text-sm rounded-lg transition-colors',
              activeTab === tab.id
                ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {activeTab === 'model' && <ModelPieChart data={byModel} />}
      {activeTab === 'project' && (
        <BreakdownBarList
          rows={byProject.map((item) => ({
            key: item.project_id,
            label: item.project_name || item.project_id,
            stats: item.stats,
          }))}
          emptyLabel={t('breakdown.noProjectData', 'No project data available')}
        />
      )}
      {activeTab === 'workflow' && (
        <BreakdownBarList
          rows={byWorkflow}
          emptyLabel={t('breakdown.noWorkflowData', 'No workflow data available')}
          dimension="workflow"
        />
      )}
      {activeTab === 'phase' && (
        <BreakdownBarList
          rows={byPhase}
          emptyLabel={t('breakdown.noPhaseData', 'No phase data available')}
          dimension="phase"
        />
      )}
      {activeTab === 'scope' && (
        <BreakdownBarList
          rows={byScope}
          emptyLabel={t('breakdown.noScopeData', 'No scope data available')}
          dimension="scope"
        />
      )}
    </div>
  );
}

interface ModelPieChartProps {
  data: ModelUsage[];
}

function ModelPieChart({ data }: ModelPieChartProps) {
  const { t } = useTranslation('analytics');
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);

  const chartData = useMemo(() => {
    if (!data || data.length === 0) return { slices: [], total: 0 };

    const total = data.reduce((sum, d) => sum + d.stats.total_cost_microdollars, 0);
    if (total === 0) return { slices: [], total: 0 };

    let currentAngle = 0;
    const slices = data.slice(0, 8).map((d, i) => {
      const percentage = d.stats.total_cost_microdollars / total;
      const angle = percentage * 360;
      const startAngle = currentAngle;
      const endAngle = currentAngle + angle;
      currentAngle = endAngle;

      return {
        ...d,
        percentage,
        startAngle,
        endAngle,
        color: COLORS[i % COLORS.length],
      };
    });

    return { slices, total };
  }, [data]);

  if (chartData.slices.length === 0) {
    return (
      <div className="flex items-center justify-center h-[200px] text-gray-400 dark:text-gray-600">
        {t('breakdown.noData', 'No data available')}
      </div>
    );
  }

  const describeArc = (startAngle: number, endAngle: number, radius: number) => {
    const start = polarToCartesian(50, 50, radius, endAngle);
    const end = polarToCartesian(50, 50, radius, startAngle);
    const largeArcFlag = endAngle - startAngle <= 180 ? '0' : '1';

    return ['M', 50, 50, 'L', start.x, start.y, 'A', radius, radius, 0, largeArcFlag, 0, end.x, end.y, 'Z'].join(' ');
  };

  const polarToCartesian = (cx: number, cy: number, r: number, angle: number) => {
    const angleRad = ((angle - 90) * Math.PI) / 180;
    return {
      x: cx + r * Math.cos(angleRad),
      y: cy + r * Math.sin(angleRad),
    };
  };

  return (
    <div className="flex items-start gap-6">
      <div className="relative w-[180px] h-[180px] shrink-0">
        <svg viewBox="0 0 100 100" className="w-full h-full">
          {chartData.slices.map((slice, i) => (
            <path
              key={slice.model_name}
              d={describeArc(slice.startAngle, slice.endAngle, 40)}
              fill={slice.color}
              className={clsx(
                'transition-opacity cursor-pointer',
                hoveredIndex !== null && hoveredIndex !== i && 'opacity-50',
              )}
              onMouseEnter={() => setHoveredIndex(i)}
              onMouseLeave={() => setHoveredIndex(null)}
            />
          ))}
          <text x="50" y="48" textAnchor="middle" className="fill-gray-900 dark:fill-white text-[6px] font-semibold">
            {formatCost(chartData.total)}
          </text>
          <text x="50" y="56" textAnchor="middle" className="fill-gray-500 dark:fill-gray-400 text-[4px]">
            {t('breakdown.total', 'Total')}
          </text>
        </svg>
      </div>

      <div className="flex-1 space-y-2">
        {chartData.slices.map((slice, i) => (
          <div
            key={slice.model_name}
            className={clsx(
              'flex items-center gap-2 text-sm cursor-pointer rounded px-2 py-1 -mx-2 transition-colors',
              hoveredIndex === i && 'bg-gray-100 dark:bg-gray-800',
            )}
            onMouseEnter={() => setHoveredIndex(i)}
            onMouseLeave={() => setHoveredIndex(null)}
          >
            <span className="w-3 h-3 rounded-sm shrink-0" style={{ backgroundColor: slice.color }} />
            <span className="flex-1 text-gray-900 dark:text-white truncate">{slice.model_name}</span>
            <span className="text-gray-500 dark:text-gray-400 text-xs">{(slice.percentage * 100).toFixed(1)}%</span>
          </div>
        ))}
      </div>
    </div>
  );
}

interface BreakdownBarListProps {
  rows: AnalyticsBreakdownRow[];
  emptyLabel: string;
  dimension?: BreakdownTab;
}

function BreakdownBarList({ rows, emptyLabel, dimension }: BreakdownBarListProps) {
  const { t } = useTranslation('analytics');
  const chartData = useMemo(() => {
    if (!rows.length) return { rows: [], maxCost: 0 };
    const maxCost = Math.max(...rows.map((row) => row.stats.total_cost_microdollars), 1);
    const localizedLabel = (row: AnalyticsBreakdownRow) => {
      if (dimension === 'workflow') {
        return workflowLabel(t, row.key as Parameters<typeof workflowLabel>[1]);
      }
      if (dimension === 'phase') {
        return phaseLabel(t, row.key);
      }
      if (dimension === 'scope') {
        return scopeLabel(t, row.key as Parameters<typeof scopeLabel>[1]);
      }
      return row.label;
    };
    return {
      maxCost,
      rows: rows.slice(0, 10).map((row, index) => ({
        ...row,
        label: localizedLabel(row),
        percentage: row.stats.total_cost_microdollars / maxCost,
        color: COLORS[index % COLORS.length],
      })),
    };
  }, [dimension, rows, t]);

  if (!chartData.rows.length) {
    return (
      <div className="flex items-center justify-center h-[200px] text-gray-400 dark:text-gray-600">{emptyLabel}</div>
    );
  }

  return (
    <div className="space-y-3">
      {chartData.rows.map((row) => (
        <div
          key={row.key}
          className="rounded px-2 py-2 -mx-2 transition-colors hover:bg-gray-100 dark:hover:bg-gray-800"
        >
          <div className="flex items-center justify-between mb-1 gap-3">
            <div className="min-w-0">
              <div className="text-sm text-gray-900 dark:text-white truncate">{row.label}</div>
              <div className="text-xs text-gray-500 dark:text-gray-400">
                {row.stats.request_count.toLocaleString()} {t('labels.requests', 'requests')} ·{' '}
                {formatTokens(row.stats.total_input_tokens + row.stats.total_output_tokens)}
              </div>
            </div>
            <span className="text-sm text-gray-500 dark:text-gray-400">
              {formatCost(row.stats.total_cost_microdollars)}
            </span>
          </div>
          <div className="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
            <div
              className="h-full rounded-full transition-all"
              style={{ width: `${row.percentage * 100}%`, backgroundColor: row.color }}
            />
          </div>
        </div>
      ))}
    </div>
  );
}

export default TokenBreakdown;
