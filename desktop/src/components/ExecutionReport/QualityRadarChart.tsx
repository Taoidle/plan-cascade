/**
 * QualityRadarChart Component
 *
 * Renders a 5-dimension quality radar chart using SVG.
 * Displays code review scores per story overlaid on a shared radar.
 *
 * Story 004: Execution Report visualization components
 */

import { useMemo } from 'react';
import { clsx } from 'clsx';
import type { RadarDimension } from '../../store/executionReport';

// ============================================================================
// Helpers
// ============================================================================

const CHART_SIZE = 200;
const CENTER = CHART_SIZE / 2;
const RADIUS = 80;
const LEVELS = 5;

/** Convert polar coordinates to SVG x,y */
function polarToCartesian(
  angle: number,
  radius: number,
  cx: number = CENTER,
  cy: number = CENTER,
): { x: number; y: number } {
  // Start from top (-90 degrees)
  const rad = ((angle - 90) * Math.PI) / 180;
  return {
    x: cx + radius * Math.cos(rad),
    y: cy + radius * Math.sin(rad),
  };
}

/** Build a polygon points string from values */
function buildPolygonPoints(values: number[], maxValues: number[], count: number): string {
  return values
    .map((val, i) => {
      const max = maxValues[i] || 1;
      const ratio = Math.min(val / max, 1);
      const angle = (360 / count) * i;
      const { x, y } = polarToCartesian(angle, RADIUS * ratio);
      return `${x},${y}`;
    })
    .join(' ');
}

const storyColors = [
  'rgba(59, 130, 246, 0.6)', // blue
  'rgba(239, 68, 68, 0.6)', // red
  'rgba(34, 197, 94, 0.6)', // green
  'rgba(168, 85, 247, 0.6)', // purple
  'rgba(245, 158, 11, 0.6)', // amber
  'rgba(6, 182, 212, 0.6)', // cyan
];

// ============================================================================
// Component
// ============================================================================

interface QualityRadarChartProps {
  dimensions: RadarDimension[];
}

export function QualityRadarChart({ dimensions }: QualityRadarChartProps) {
  const count = dimensions.length;

  // Get unique story IDs
  const storyIds = useMemo(() => {
    const ids = new Set<string>();
    for (const dim of dimensions) {
      for (const id of Object.keys(dim.storyScores)) {
        ids.add(id);
      }
    }
    return Array.from(ids);
  }, [dimensions]);

  if (count === 0) {
    return (
      <div className="text-xs text-gray-400 dark:text-gray-500 italic" data-testid="quality-radar-chart">
        No quality data available
      </div>
    );
  }

  // Grid lines (concentric polygons)
  const gridLines = Array.from({ length: LEVELS }, (_, level) => {
    const ratio = (level + 1) / LEVELS;
    const points = Array.from({ length: count }, (_, i) => {
      const angle = (360 / count) * i;
      const { x, y } = polarToCartesian(angle, RADIUS * ratio);
      return `${x},${y}`;
    }).join(' ');
    return points;
  });

  // Axis lines
  const axisLines = dimensions.map((_, i) => {
    const angle = (360 / count) * i;
    return polarToCartesian(angle, RADIUS);
  });

  // Labels
  const labels = dimensions.map((dim, i) => {
    const angle = (360 / count) * i;
    const { x, y } = polarToCartesian(angle, RADIUS + 20);
    return { text: dim.dimension, x, y };
  });

  // Average polygon
  const avgValues = dimensions.map((dim) => dim.averageScore);
  const maxValues = dimensions.map((dim) => dim.maxScore);
  const avgPoints = buildPolygonPoints(avgValues, maxValues, count);

  return (
    <div className="space-y-3" data-testid="quality-radar-chart">
      <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">Quality Radar</h4>

      <div className="flex justify-center">
        <svg width={CHART_SIZE} height={CHART_SIZE} viewBox={`0 0 ${CHART_SIZE} ${CHART_SIZE}`}>
          {/* Grid */}
          {gridLines.map((points, i) => (
            <polygon
              key={`grid-${i}`}
              points={points}
              fill="none"
              stroke="currentColor"
              className="text-gray-200 dark:text-gray-700"
              strokeWidth={0.5}
            />
          ))}

          {/* Axes */}
          {axisLines.map((point, i) => (
            <line
              key={`axis-${i}`}
              x1={CENTER}
              y1={CENTER}
              x2={point.x}
              y2={point.y}
              stroke="currentColor"
              className="text-gray-200 dark:text-gray-700"
              strokeWidth={0.5}
            />
          ))}

          {/* Per-story polygons */}
          {storyIds.map((storyId, si) => {
            const storyValues = dimensions.map((dim) => dim.storyScores[storyId] ?? 0);
            const points = buildPolygonPoints(storyValues, maxValues, count);
            const color = storyColors[si % storyColors.length];
            return (
              <polygon
                key={`story-${storyId}`}
                points={points}
                fill={color}
                stroke={color.replace('0.6', '1')}
                strokeWidth={1}
                opacity={0.3}
              />
            );
          })}

          {/* Average polygon (bold) */}
          <polygon
            points={avgPoints}
            fill="rgba(59, 130, 246, 0.15)"
            stroke="rgba(59, 130, 246, 0.8)"
            strokeWidth={2}
          />

          {/* Labels */}
          {labels.map((label, i) => (
            <text
              key={`label-${i}`}
              x={label.x}
              y={label.y}
              textAnchor="middle"
              dominantBaseline="middle"
              className="fill-gray-600 dark:fill-gray-400 text-[9px]"
            >
              {label.text}
            </text>
          ))}
        </svg>
      </div>

      {/* Legend */}
      <div className="flex flex-wrap gap-3 justify-center">
        {storyIds.map((storyId, si) => (
          <div key={storyId} className="flex items-center gap-1.5 text-xs">
            <div className="w-3 h-3 rounded-sm" style={{ backgroundColor: storyColors[si % storyColors.length] }} />
            <span className="text-gray-600 dark:text-gray-400">{storyId}</span>
          </div>
        ))}
        <div className="flex items-center gap-1.5 text-xs">
          <div className="w-3 h-3 rounded-sm bg-blue-500/50" />
          <span className="text-gray-600 dark:text-gray-400 font-medium">Average</span>
        </div>
      </div>
    </div>
  );
}

export default QualityRadarChart;
