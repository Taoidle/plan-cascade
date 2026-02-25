/**
 * ExecutionTimeline Component
 *
 * Visual timeline showing tool execution order, duration, and parallelism.
 * Displays temporal relationships between tool calls and identifies performance bottlenecks.
 *
 * Story-006: Tool execution timeline visualization
 */

import { useState, useMemo, useRef, useCallback, useEffect } from 'react';
import { clsx } from 'clsx';
import { ZoomInIcon, ZoomOutIcon, ResetIcon, DownloadIcon, InfoCircledIcon, PlayIcon } from '@radix-ui/react-icons';
import type { ToolCall, ToolType } from '../../store/claudeCode';

// ============================================================================
// Types
// ============================================================================

interface ExecutionTimelineProps {
  /** List of tool calls to visualize */
  toolCalls: ToolCall[];
  /** Callback when a tool bar is clicked */
  onToolClick?: (toolCallId: string) => void;
  /** Height of the timeline in pixels */
  height?: number;
  /** Additional CSS classes */
  className?: string;
}

interface TimelineBar {
  toolCall: ToolCall;
  startMs: number;
  endMs: number;
  duration: number;
  lane: number;
}

interface TooltipData {
  toolCall: ToolCall;
  x: number;
  y: number;
}

// ============================================================================
// Constants
// ============================================================================

const TOOL_COLORS: Record<ToolType | 'Unknown', string> = {
  Read: '#3b82f6', // blue-500
  Write: '#22c55e', // green-500
  Edit: '#eab308', // yellow-500
  Bash: '#a855f7', // purple-500
  Glob: '#f97316', // orange-500
  Grep: '#ec4899', // pink-500
  WebFetch: '#06b6d4', // cyan-500
  WebSearch: '#06b6d4', // cyan-500
  Unknown: '#6b7280', // gray-500
};

const LANE_HEIGHT = 32;
const BAR_HEIGHT = 24;
const PADDING_TOP = 40;
const PADDING_LEFT = 60;
const PADDING_RIGHT = 20;
const PADDING_BOTTOM = 30;
const MIN_BAR_WIDTH = 4;

// ============================================================================
// Helper Functions
// ============================================================================

function parseTimestamp(timestamp: string | undefined): number {
  if (!timestamp) return 0;
  return new Date(timestamp).getTime();
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
}

function formatTime(ms: number): string {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);

  if (minutes > 0) {
    return `${minutes}m ${seconds % 60}s`;
  }
  return `${seconds}.${Math.floor((ms % 1000) / 100)}s`;
}

// ============================================================================
// Timeline Calculation
// ============================================================================

function calculateTimeline(toolCalls: ToolCall[]): {
  bars: TimelineBar[];
  startTime: number;
  endTime: number;
  duration: number;
  laneCount: number;
} {
  if (toolCalls.length === 0) {
    return { bars: [], startTime: 0, endTime: 0, duration: 0, laneCount: 0 };
  }

  // Calculate start and end times for each tool call
  const toolCallTimes = toolCalls
    .map((tc) => {
      const startMs = parseTimestamp(tc.startedAt);
      let endMs = parseTimestamp(tc.completedAt);

      // If not completed, use current time or duration
      if (!endMs) {
        endMs = tc.duration ? startMs + tc.duration : Date.now();
      }

      return {
        toolCall: tc,
        startMs,
        endMs,
        duration: endMs - startMs,
      };
    })
    .filter((t) => t.startMs > 0); // Filter out invalid entries

  if (toolCallTimes.length === 0) {
    return { bars: [], startTime: 0, endTime: 0, duration: 0, laneCount: 0 };
  }

  // Find global time range
  const startTime = Math.min(...toolCallTimes.map((t) => t.startMs));
  const endTime = Math.max(...toolCallTimes.map((t) => t.endMs));
  const duration = endTime - startTime;

  // Assign lanes using a greedy algorithm (avoid overlapping bars)
  const lanes: { endMs: number }[] = [];
  const bars: TimelineBar[] = [];

  // Sort by start time
  const sortedToolCalls = [...toolCallTimes].sort((a, b) => a.startMs - b.startMs);

  sortedToolCalls.forEach(({ toolCall, startMs, endMs, duration }) => {
    // Find the first lane that's free at this start time
    let assignedLane = -1;
    for (let i = 0; i < lanes.length; i++) {
      if (lanes[i].endMs <= startMs) {
        assignedLane = i;
        break;
      }
    }

    // If no free lane, create a new one
    if (assignedLane === -1) {
      assignedLane = lanes.length;
      lanes.push({ endMs: 0 });
    }

    // Update lane end time
    lanes[assignedLane].endMs = endMs;

    bars.push({
      toolCall,
      startMs: startMs - startTime, // Relative to timeline start
      endMs: endMs - startTime,
      duration,
      lane: assignedLane,
    });
  });

  return {
    bars,
    startTime,
    endTime,
    duration,
    laneCount: lanes.length,
  };
}

// ============================================================================
// Statistics Calculation
// ============================================================================

function calculateStatistics(
  bars: TimelineBar[],
  totalDuration: number,
): {
  totalExecutionTime: number;
  parallelEfficiency: number;
  averageDuration: number;
  longestOperation: TimelineBar | null;
  idleTime: number;
} {
  if (bars.length === 0) {
    return {
      totalExecutionTime: 0,
      parallelEfficiency: 0,
      averageDuration: 0,
      longestOperation: null,
      idleTime: 0,
    };
  }

  const totalExecutionTime = bars.reduce((sum, bar) => sum + bar.duration, 0);
  const parallelEfficiency = totalDuration > 0 ? totalExecutionTime / totalDuration : 0;
  const averageDuration = totalExecutionTime / bars.length;
  const longestOperation = bars.reduce(
    (longest, bar) => (bar.duration > (longest?.duration || 0) ? bar : longest),
    bars[0],
  );

  // Calculate idle time (gaps between operations)
  const sortedBars = [...bars].sort((a, b) => a.startMs - b.startMs);
  let idleTime = 0;
  let currentEnd = 0;

  sortedBars.forEach((bar) => {
    if (bar.startMs > currentEnd) {
      idleTime += bar.startMs - currentEnd;
    }
    currentEnd = Math.max(currentEnd, bar.endMs);
  });

  return {
    totalExecutionTime,
    parallelEfficiency,
    averageDuration,
    longestOperation,
    idleTime,
  };
}

// ============================================================================
// ExecutionTimeline Component
// ============================================================================

export function ExecutionTimeline({ toolCalls, onToolClick, height: propHeight, className }: ExecutionTimelineProps) {
  // State
  const [zoom, setZoom] = useState(1);
  const [panOffset, setPanOffset] = useState(0);
  const [tooltip, setTooltip] = useState<TooltipData | null>(null);
  const [showStats, setShowStats] = useState(false);
  const [isPanning, setIsPanning] = useState(false);
  const [panStart, setPanStart] = useState(0);

  // Refs
  const containerRef = useRef<HTMLDivElement>(null);
  const svgRef = useRef<SVGSVGElement>(null);

  // Calculate timeline data
  const { bars, startTime, duration, laneCount } = useMemo(() => calculateTimeline(toolCalls), [toolCalls]);

  // Calculate statistics
  const stats = useMemo(() => calculateStatistics(bars, duration), [bars, duration]);

  // Calculate dimensions
  const height = propHeight || Math.max(200, PADDING_TOP + laneCount * LANE_HEIGHT + PADDING_BOTTOM);
  const [containerWidth, setContainerWidth] = useState(800);

  // Update container width on resize
  useEffect(() => {
    const updateWidth = () => {
      if (containerRef.current) {
        setContainerWidth(containerRef.current.clientWidth);
      }
    };

    updateWidth();
    window.addEventListener('resize', updateWidth);
    return () => window.removeEventListener('resize', updateWidth);
  }, []);

  // Calculate scale
  const contentWidth = containerWidth - PADDING_LEFT - PADDING_RIGHT;
  const timeScale = (contentWidth * zoom) / duration;

  // Zoom handlers
  const handleZoomIn = useCallback(() => {
    setZoom((prev) => Math.min(prev * 1.5, 10));
  }, []);

  const handleZoomOut = useCallback(() => {
    setZoom((prev) => Math.max(prev / 1.5, 0.5));
  }, []);

  const handleResetZoom = useCallback(() => {
    setZoom(1);
    setPanOffset(0);
  }, []);

  // Pan handlers
  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (zoom > 1) {
        setIsPanning(true);
        setPanStart(e.clientX - panOffset);
      }
    },
    [zoom, panOffset],
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (isPanning) {
        const newOffset = e.clientX - panStart;
        const maxOffset = 0;
        const minOffset = -(contentWidth * zoom - contentWidth);
        setPanOffset(Math.max(minOffset, Math.min(maxOffset, newOffset)));
      }
    },
    [isPanning, panStart, contentWidth, zoom],
  );

  const handleMouseUp = useCallback(() => {
    setIsPanning(false);
  }, []);

  // Export as SVG
  const handleExport = useCallback(() => {
    if (!svgRef.current) return;

    const svgData = new XMLSerializer().serializeToString(svgRef.current);
    const blob = new Blob([svgData], { type: 'image/svg+xml' });
    const url = URL.createObjectURL(blob);

    const link = document.createElement('a');
    link.href = url;
    link.download = `execution-timeline-${new Date().toISOString().slice(0, 10)}.svg`;
    link.click();

    URL.revokeObjectURL(url);
  }, []);

  // Calculate bar position and dimensions
  const getBarProps = (bar: TimelineBar) => {
    const x = PADDING_LEFT + bar.startMs * timeScale + panOffset;
    const y = PADDING_TOP + bar.lane * LANE_HEIGHT + (LANE_HEIGHT - BAR_HEIGHT) / 2;
    const width = Math.max(MIN_BAR_WIDTH, bar.duration * timeScale);

    return { x, y, width };
  };

  // Time axis ticks
  const timeTicks = useMemo(() => {
    if (duration === 0) return [];

    const tickCount = Math.min(10, Math.floor(contentWidth / 80));
    const tickInterval = duration / tickCount;
    const ticks: { ms: number; x: number }[] = [];

    for (let i = 0; i <= tickCount; i++) {
      const ms = i * tickInterval;
      const x = PADDING_LEFT + ms * timeScale + panOffset;
      if (x >= PADDING_LEFT && x <= containerWidth - PADDING_RIGHT) {
        ticks.push({ ms, x });
      }
    }

    return ticks;
  }, [duration, contentWidth, timeScale, panOffset, containerWidth]);

  // Current time marker (for running operations)
  const hasRunningOps = bars.some((bar) => bar.toolCall.status === 'executing' || bar.toolCall.status === 'pending');

  const currentTimeX = hasRunningOps ? PADDING_LEFT + (Date.now() - startTime) * timeScale + panOffset : null;

  if (bars.length === 0) {
    return (
      <div
        className={clsx('flex flex-col items-center justify-center p-8', 'text-gray-500 dark:text-gray-400', className)}
      >
        <PlayIcon className="w-8 h-8 mb-2 opacity-50" />
        <p className="text-sm">No tool execution data available</p>
      </div>
    );
  }

  return (
    <div ref={containerRef} className={clsx('relative flex flex-col', className)}>
      {/* Controls */}
      <div
        className={clsx(
          'flex items-center justify-between px-3 py-2',
          'bg-gray-50 dark:bg-gray-800/50',
          'border-b border-gray-200 dark:border-gray-700',
          'rounded-t-lg',
        )}
      >
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Execution Timeline</span>
          <span className="text-xs text-gray-500">{formatDuration(duration)} total</span>
        </div>

        <div className="flex items-center gap-1">
          {/* Stats toggle */}
          <button
            onClick={() => setShowStats(!showStats)}
            className={clsx(
              'p-1.5 rounded transition-colors',
              showStats
                ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-600 dark:text-primary-400'
                : 'text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700',
            )}
            title="Toggle statistics"
          >
            <InfoCircledIcon className="w-4 h-4" />
          </button>

          {/* Zoom controls */}
          <button
            onClick={handleZoomOut}
            disabled={zoom <= 0.5}
            className="p-1.5 rounded text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-50 transition-colors"
            title="Zoom out"
          >
            <ZoomOutIcon className="w-4 h-4" />
          </button>
          <span className="text-xs text-gray-500 w-12 text-center">{Math.round(zoom * 100)}%</span>
          <button
            onClick={handleZoomIn}
            disabled={zoom >= 10}
            className="p-1.5 rounded text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-50 transition-colors"
            title="Zoom in"
          >
            <ZoomInIcon className="w-4 h-4" />
          </button>
          <button
            onClick={handleResetZoom}
            className="p-1.5 rounded text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
            title="Reset zoom"
          >
            <ResetIcon className="w-4 h-4" />
          </button>

          {/* Export */}
          <button
            onClick={handleExport}
            className="p-1.5 rounded text-gray-500 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
            title="Export as SVG"
          >
            <DownloadIcon className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Statistics panel */}
      {showStats && (
        <div
          className={clsx(
            'flex items-center gap-6 px-4 py-2',
            'bg-blue-50 dark:bg-blue-900/20',
            'border-b border-gray-200 dark:border-gray-700',
            'text-sm',
          )}
        >
          <div>
            <span className="text-gray-500 dark:text-gray-400">Total execution: </span>
            <span className="font-medium text-gray-700 dark:text-gray-300">
              {formatDuration(stats.totalExecutionTime)}
            </span>
          </div>
          <div>
            <span className="text-gray-500 dark:text-gray-400">Parallel efficiency: </span>
            <span className="font-medium text-gray-700 dark:text-gray-300">
              {(stats.parallelEfficiency * 100).toFixed(0)}%
            </span>
          </div>
          <div>
            <span className="text-gray-500 dark:text-gray-400">Average: </span>
            <span className="font-medium text-gray-700 dark:text-gray-300">
              {formatDuration(stats.averageDuration)}
            </span>
          </div>
          <div>
            <span className="text-gray-500 dark:text-gray-400">Idle time: </span>
            <span className="font-medium text-gray-700 dark:text-gray-300">{formatDuration(stats.idleTime)}</span>
          </div>
          {stats.longestOperation && (
            <div>
              <span className="text-gray-500 dark:text-gray-400">Longest: </span>
              <span className="font-medium text-gray-700 dark:text-gray-300">
                {stats.longestOperation.toolCall.name} ({formatDuration(stats.longestOperation.duration)})
              </span>
            </div>
          )}
        </div>
      )}

      {/* Timeline SVG */}
      <div
        className={clsx('overflow-hidden', zoom > 1 && 'cursor-grab', isPanning && 'cursor-grabbing')}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
      >
        <svg ref={svgRef} width={containerWidth} height={height} className="bg-white dark:bg-gray-900">
          {/* Lane backgrounds */}
          {Array.from({ length: laneCount }).map((_, i) => (
            <rect
              key={`lane-${i}`}
              x={PADDING_LEFT}
              y={PADDING_TOP + i * LANE_HEIGHT}
              width={contentWidth}
              height={LANE_HEIGHT}
              fill={i % 2 === 0 ? 'transparent' : 'rgba(0,0,0,0.02)'}
              className="dark:fill-white/5"
            />
          ))}

          {/* Time axis */}
          <line
            x1={PADDING_LEFT}
            y1={PADDING_TOP + laneCount * LANE_HEIGHT}
            x2={containerWidth - PADDING_RIGHT}
            y2={PADDING_TOP + laneCount * LANE_HEIGHT}
            stroke="currentColor"
            strokeOpacity={0.2}
          />

          {/* Time ticks */}
          {timeTicks.map(({ ms, x }) => (
            <g key={`tick-${ms}`}>
              <line
                x1={x}
                y1={PADDING_TOP + laneCount * LANE_HEIGHT}
                x2={x}
                y2={PADDING_TOP + laneCount * LANE_HEIGHT + 5}
                stroke="currentColor"
                strokeOpacity={0.3}
              />
              <text
                x={x}
                y={PADDING_TOP + laneCount * LANE_HEIGHT + 18}
                textAnchor="middle"
                fontSize={10}
                fill="currentColor"
                opacity={0.5}
              >
                {formatTime(ms)}
              </text>
            </g>
          ))}

          {/* Lane labels */}
          {Array.from({ length: laneCount }).map((_, i) => (
            <text
              key={`label-${i}`}
              x={PADDING_LEFT - 10}
              y={PADDING_TOP + i * LANE_HEIGHT + LANE_HEIGHT / 2}
              textAnchor="end"
              dominantBaseline="middle"
              fontSize={10}
              fill="currentColor"
              opacity={0.4}
            >
              Lane {i + 1}
            </text>
          ))}

          {/* Tool call bars */}
          {bars.map((bar) => {
            const { x, y, width } = getBarProps(bar);
            const color = TOOL_COLORS[bar.toolCall.name] || TOOL_COLORS.Unknown;
            const isRunning = bar.toolCall.status === 'executing';

            return (
              <g
                key={bar.toolCall.id}
                className="cursor-pointer"
                onClick={() => onToolClick?.(bar.toolCall.id)}
                onMouseEnter={(e) =>
                  setTooltip({
                    toolCall: bar.toolCall,
                    x: e.clientX,
                    y: e.clientY,
                  })
                }
                onMouseLeave={() => setTooltip(null)}
              >
                {/* Bar shadow */}
                <rect x={x + 2} y={y + 2} width={width} height={BAR_HEIGHT} rx={4} fill="black" opacity={0.1} />

                {/* Bar */}
                <rect
                  x={x}
                  y={y}
                  width={width}
                  height={BAR_HEIGHT}
                  rx={4}
                  fill={color}
                  opacity={bar.toolCall.status === 'failed' ? 0.6 : 1}
                  className={clsx('transition-opacity', isRunning && 'animate-pulse')}
                />

                {/* Tool name (if bar is wide enough) */}
                {width > 50 && (
                  <text
                    x={x + 6}
                    y={y + BAR_HEIGHT / 2}
                    dominantBaseline="middle"
                    fontSize={10}
                    fill="white"
                    fontWeight="500"
                  >
                    {bar.toolCall.name}
                  </text>
                )}

                {/* Failed indicator */}
                {bar.toolCall.status === 'failed' && (
                  <line x1={x} y1={y} x2={x + width} y2={y + BAR_HEIGHT} stroke="white" strokeWidth={2} opacity={0.5} />
                )}
              </g>
            );
          })}

          {/* Current time marker */}
          {currentTimeX && currentTimeX >= PADDING_LEFT && currentTimeX <= containerWidth - PADDING_RIGHT && (
            <g>
              <line
                x1={currentTimeX}
                y1={PADDING_TOP - 5}
                x2={currentTimeX}
                y2={PADDING_TOP + laneCount * LANE_HEIGHT}
                stroke="#ef4444"
                strokeWidth={2}
                strokeDasharray="4 2"
              />
              <circle cx={currentTimeX} cy={PADDING_TOP - 5} r={4} fill="#ef4444" />
            </g>
          )}
        </svg>
      </div>

      {/* Tooltip */}
      {tooltip && (
        <div
          className={clsx(
            'fixed z-50 px-3 py-2 rounded-lg shadow-lg',
            'bg-gray-900 text-white text-sm',
            'pointer-events-none',
          )}
          style={{
            left: tooltip.x + 10,
            top: tooltip.y + 10,
          }}
        >
          <div className="font-medium">{tooltip.toolCall.name}</div>
          <div className="text-gray-300 text-xs mt-1">Duration: {formatDuration(tooltip.toolCall.duration || 0)}</div>
          <div className="text-gray-300 text-xs">Status: {tooltip.toolCall.status}</div>
          {tooltip.toolCall.parameters.file_path && (
            <div className="text-gray-400 text-xs mt-1 truncate max-w-[200px]">
              {tooltip.toolCall.parameters.file_path}
            </div>
          )}
        </div>
      )}

      {/* Legend */}
      <div
        className={clsx(
          'flex flex-wrap items-center gap-4 px-4 py-2',
          'bg-gray-50 dark:bg-gray-800/50',
          'border-t border-gray-200 dark:border-gray-700',
          'rounded-b-lg',
        )}
      >
        {Object.entries(TOOL_COLORS)
          .filter(([key]) => key !== 'Unknown')
          .map(([name, color]) => (
            <div key={name} className="flex items-center gap-1.5">
              <div className="w-3 h-3 rounded" style={{ backgroundColor: color }} />
              <span className="text-xs text-gray-600 dark:text-gray-400">{name}</span>
            </div>
          ))}
      </div>
    </div>
  );
}

// ============================================================================
// Exports
// ============================================================================

export { calculateTimeline, calculateStatistics, TOOL_COLORS };
export default ExecutionTimeline;
