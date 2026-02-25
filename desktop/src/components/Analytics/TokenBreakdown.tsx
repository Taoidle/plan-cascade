/**
 * Token Breakdown Component
 *
 * Displays usage breakdown by model (pie chart) and by project (bar chart).
 * Uses simple SVG-based visualizations.
 */

import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { ModelUsage, ProjectUsage } from '../../store/analytics';
import { formatCost, formatTokens } from '../../store/analytics';

interface TokenBreakdownProps {
  byModel: ModelUsage[];
  byProject: ProjectUsage[];
}

// Color palette for charts
const COLORS = [
  '#3B82F6', // blue-500
  '#10B981', // emerald-500
  '#F59E0B', // amber-500
  '#EF4444', // red-500
  '#8B5CF6', // violet-500
  '#EC4899', // pink-500
  '#06B6D4', // cyan-500
  '#84CC16', // lime-500
];

export function TokenBreakdown({ byModel, byProject }: TokenBreakdownProps) {
  const { t } = useTranslation('analytics');
  const [activeTab, setActiveTab] = useState<'model' | 'project'>('model');

  return (
    <div>
      {/* Tab Switcher */}
      <div className="flex gap-2 mb-4">
        <button
          onClick={() => setActiveTab('model')}
          className={clsx(
            'px-3 py-1.5 text-sm rounded-lg transition-colors',
            activeTab === 'model'
              ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
              : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
          )}
        >
          {t('breakdown.byModel', 'By Model')}
        </button>
        <button
          onClick={() => setActiveTab('project')}
          className={clsx(
            'px-3 py-1.5 text-sm rounded-lg transition-colors',
            activeTab === 'project'
              ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
              : 'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
          )}
        >
          {t('breakdown.byProject', 'By Project')}
        </button>
      </div>

      {/* Content */}
      {activeTab === 'model' ? <ModelPieChart data={byModel} /> : <ProjectBarChart data={byProject} />}
    </div>
  );
}

// Pie chart for model usage
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

  // Convert angle to SVG path
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
      {/* Pie Chart */}
      <div className="relative w-[180px] h-[180px] shrink-0">
        <svg viewBox="0 0 100 100" className="w-full h-full">
          {chartData.slices.map((slice, i) => (
            <path
              key={i}
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
          {/* Center text */}
          <text x="50" y="48" textAnchor="middle" className="fill-gray-900 dark:fill-white text-[6px] font-semibold">
            {formatCost(chartData.total)}
          </text>
          <text x="50" y="56" textAnchor="middle" className="fill-gray-500 dark:fill-gray-400 text-[4px]">
            {t('breakdown.total', 'Total')}
          </text>
        </svg>
      </div>

      {/* Legend */}
      <div className="flex-1 space-y-2">
        {chartData.slices.map((slice, i) => (
          <div
            key={i}
            className={clsx(
              'flex items-center gap-2 text-sm',
              'cursor-pointer rounded px-2 py-1 -mx-2',
              'transition-colors',
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

// Bar chart for project usage
interface ProjectBarChartProps {
  data: ProjectUsage[];
}

function ProjectBarChart({ data }: ProjectBarChartProps) {
  const { t } = useTranslation('analytics');
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);

  const chartData = useMemo(() => {
    if (!data || data.length === 0) return { bars: [], maxCost: 0 };

    const maxCost = Math.max(...data.map((d) => d.stats.total_cost_microdollars), 1);

    const bars = data.slice(0, 8).map((d, i) => ({
      ...d,
      percentage: d.stats.total_cost_microdollars / maxCost,
      color: COLORS[i % COLORS.length],
      displayName: d.project_name || d.project_id.substring(0, 8) + '...',
    }));

    return { bars, maxCost };
  }, [data]);

  if (chartData.bars.length === 0) {
    return (
      <div className="flex items-center justify-center h-[200px] text-gray-400 dark:text-gray-600">
        {t('breakdown.noProjectData', 'No project data available')}
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {chartData.bars.map((bar, i) => (
        <div
          key={bar.project_id}
          className={clsx(
            'cursor-pointer rounded px-2 py-2 -mx-2',
            'transition-colors',
            hoveredIndex === i && 'bg-gray-100 dark:bg-gray-800',
          )}
          onMouseEnter={() => setHoveredIndex(i)}
          onMouseLeave={() => setHoveredIndex(null)}
        >
          <div className="flex items-center justify-between mb-1">
            <span className="text-sm text-gray-900 dark:text-white truncate">{bar.displayName}</span>
            <span className="text-sm text-gray-500 dark:text-gray-400 ml-2">
              {formatCost(bar.stats.total_cost_microdollars)}
            </span>
          </div>
          <div className="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
            <div
              className="h-full rounded-full transition-all"
              style={{
                width: `${bar.percentage * 100}%`,
                backgroundColor: bar.color,
              }}
            />
          </div>
          {hoveredIndex === i && (
            <div className="mt-2 text-xs text-gray-500 dark:text-gray-400 flex gap-4">
              <span>{formatTokens(bar.stats.total_input_tokens + bar.stats.total_output_tokens)} tokens</span>
              <span>{bar.stats.request_count} requests</span>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

export default TokenBreakdown;
