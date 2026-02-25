/**
 * Cost Chart Component
 *
 * Displays cost over time as an area/line chart.
 * Uses a simple SVG-based implementation for lightweight rendering.
 */

import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { TimeSeriesPoint } from '../../store/analytics';
import { formatCost } from '../../store/analytics';

interface CostChartProps {
  data: TimeSeriesPoint[];
  height?: number;
}

export function CostChart({ data, height = 300 }: CostChartProps) {
  const { t } = useTranslation('analytics');
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);

  // Process data for chart
  const chartData = useMemo(() => {
    if (!data || data.length === 0) {
      return { points: [], maxCost: 0, minCost: 0 };
    }

    const costs = data.map((d) => d.stats.total_cost_microdollars);
    const maxCost = Math.max(...costs, 1); // Avoid division by zero
    const minCost = Math.min(...costs);

    const points = data.map((d, i) => ({
      x: (i / (data.length - 1 || 1)) * 100,
      y: 100 - ((d.stats.total_cost_microdollars - minCost) / (maxCost - minCost || 1)) * 100,
      value: d.stats.total_cost_microdollars,
      label: d.timestamp_formatted,
      tokens: d.stats.total_input_tokens + d.stats.total_output_tokens,
      requests: d.stats.request_count,
    }));

    return { points, maxCost, minCost };
  }, [data]);

  if (!data || data.length === 0) {
    return (
      <div className={clsx('flex items-center justify-center', 'h-[300px]', 'text-gray-400 dark:text-gray-600')}>
        {t('charts.noData', 'No data available for this period')}
      </div>
    );
  }

  // Generate SVG path for area
  const areaPath = useMemo(() => {
    if (chartData.points.length === 0) return '';

    const pathPoints = chartData.points.map((p) => `${p.x},${p.y}`).join(' L ');
    return `M ${chartData.points[0].x},100 L ${pathPoints} L ${chartData.points[chartData.points.length - 1].x},100 Z`;
  }, [chartData.points]);

  // Generate SVG path for line
  const linePath = useMemo(() => {
    if (chartData.points.length === 0) return '';
    return chartData.points.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x},${p.y}`).join(' ');
  }, [chartData.points]);

  return (
    <div className="relative" style={{ height }}>
      {/* Y-axis labels */}
      <div className="absolute left-0 top-0 bottom-8 w-16 flex flex-col justify-between text-xs text-gray-500 dark:text-gray-400">
        <span>{formatCost(chartData.maxCost)}</span>
        <span>{formatCost((chartData.maxCost + chartData.minCost) / 2)}</span>
        <span>{formatCost(chartData.minCost)}</span>
      </div>

      {/* Chart area */}
      <div className="absolute left-16 right-0 top-0 bottom-8">
        <svg
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          className="w-full h-full"
          onMouseLeave={() => setHoveredIndex(null)}
        >
          {/* Grid lines */}
          <defs>
            <pattern id="grid" width="20" height="25" patternUnits="userSpaceOnUse">
              <path
                d="M 20 0 L 0 0 0 25"
                fill="none"
                stroke="currentColor"
                strokeWidth="0.1"
                className="text-gray-200 dark:text-gray-800"
              />
            </pattern>
          </defs>
          <rect width="100" height="100" fill="url(#grid)" />

          {/* Area fill */}
          <path d={areaPath} className="fill-primary-100 dark:fill-primary-900/30" />

          {/* Line */}
          <path d={linePath} fill="none" strokeWidth="0.5" className="stroke-primary-500" />

          {/* Data points */}
          {chartData.points.map((point, i) => (
            <g key={i}>
              <circle
                cx={point.x}
                cy={point.y}
                r={hoveredIndex === i ? 1.5 : 0.8}
                className={clsx('fill-primary-500', hoveredIndex === i && 'stroke-white stroke-[0.3]')}
              />
              {/* Invisible larger hit area */}
              <circle
                cx={point.x}
                cy={point.y}
                r="3"
                fill="transparent"
                className="cursor-pointer"
                onMouseEnter={() => setHoveredIndex(i)}
              />
            </g>
          ))}
        </svg>

        {/* Tooltip */}
        {hoveredIndex !== null && chartData.points[hoveredIndex] && (
          <div
            className={clsx(
              'absolute z-10 pointer-events-none',
              'px-3 py-2 rounded-lg shadow-lg',
              'bg-gray-900 dark:bg-gray-800',
              'text-white text-xs',
              'transform -translate-x-1/2',
            )}
            style={{
              left: `${chartData.points[hoveredIndex].x}%`,
              top: `${chartData.points[hoveredIndex].y - 15}%`,
            }}
          >
            <div className="font-medium">{chartData.points[hoveredIndex].label}</div>
            <div className="mt-1 space-y-0.5">
              <div>
                {t('tooltip.cost', 'Cost')}: {formatCost(chartData.points[hoveredIndex].value)}
              </div>
              <div>
                {t('tooltip.tokens', 'Tokens')}: {chartData.points[hoveredIndex].tokens.toLocaleString()}
              </div>
              <div>
                {t('tooltip.requests', 'Requests')}: {chartData.points[hoveredIndex].requests}
              </div>
            </div>
          </div>
        )}
      </div>

      {/* X-axis labels */}
      <div className="absolute left-16 right-0 bottom-0 h-8 flex justify-between items-start text-xs text-gray-500 dark:text-gray-400">
        {data.length > 0 && (
          <>
            <span>{data[0].timestamp_formatted}</span>
            {data.length > 2 && <span>{data[Math.floor(data.length / 2)].timestamp_formatted}</span>}
            <span>{data[data.length - 1].timestamp_formatted}</span>
          </>
        )}
      </div>

      {/* Legend */}
      <div className="absolute top-0 right-0 flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
        <span className="flex items-center gap-1">
          <span className="w-3 h-0.5 bg-primary-500 rounded" />
          {t('legend.cost', 'Cost')}
        </span>
      </div>
    </div>
  );
}

export default CostChart;
