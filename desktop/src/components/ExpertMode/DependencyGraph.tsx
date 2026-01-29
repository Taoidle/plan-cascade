/**
 * Dependency Graph Component
 *
 * Visual representation of story dependencies using SVG.
 * Shows nodes for each story with arrows indicating dependencies.
 */

import { useMemo, useRef, useEffect, useState } from 'react';
import { clsx } from 'clsx';
import { usePRDStore, PRDStory, StoryStatus } from '../../store/prd';

interface NodePosition {
  x: number;
  y: number;
  story: PRDStory;
}

const NODE_WIDTH = 180;
const NODE_HEIGHT = 60;
const NODE_MARGIN_X = 50;
const NODE_MARGIN_Y = 30;
const PADDING = 40;

const statusColors: Record<StoryStatus, { bg: string; border: string; text: string }> = {
  pending: {
    bg: '#f3f4f6',
    border: '#9ca3af',
    text: '#374151',
  },
  in_progress: {
    bg: '#dbeafe',
    border: '#3b82f6',
    text: '#1d4ed8',
  },
  completed: {
    bg: '#dcfce7',
    border: '#22c55e',
    text: '#166534',
  },
  failed: {
    bg: '#fee2e2',
    border: '#ef4444',
    text: '#dc2626',
  },
};

export function DependencyGraph() {
  const { prd } = usePRDStore();
  const svgRef = useRef<SVGSVGElement>(null);
  const [selectedStory, setSelectedStory] = useState<string | null>(null);

  // Calculate node positions using a simple layered layout
  const { nodes, edges, width, height } = useMemo(() => {
    if (prd.stories.length === 0) {
      return { nodes: [], edges: [], width: 400, height: 200 };
    }

    // Build dependency layers
    const layers: string[][] = [];
    const assigned = new Set<string>();
    const storyMap = new Map(prd.stories.map((s) => [s.id, s]));

    // First pass: find stories with no dependencies
    const noDeps = prd.stories.filter((s) => s.dependencies.length === 0);
    if (noDeps.length > 0) {
      layers.push(noDeps.map((s) => s.id));
      noDeps.forEach((s) => assigned.add(s.id));
    }

    // Subsequent passes: add stories whose deps are all assigned
    let maxIterations = prd.stories.length;
    while (assigned.size < prd.stories.length && maxIterations > 0) {
      const layer: string[] = [];
      for (const story of prd.stories) {
        if (assigned.has(story.id)) continue;
        const allDepsAssigned = story.dependencies.every((d) => assigned.has(d));
        if (allDepsAssigned) {
          layer.push(story.id);
        }
      }
      if (layer.length === 0) {
        // Circular dependency or orphan - add remaining
        const remaining = prd.stories
          .filter((s) => !assigned.has(s.id))
          .map((s) => s.id);
        layers.push(remaining);
        remaining.forEach((id) => assigned.add(id));
        break;
      }
      layers.push(layer);
      layer.forEach((id) => assigned.add(id));
      maxIterations--;
    }

    // Calculate positions
    const nodePositions: NodePosition[] = [];
    const nodeMap = new Map<string, NodePosition>();

    let maxLayerWidth = 0;
    layers.forEach((layer, layerIndex) => {
      maxLayerWidth = Math.max(maxLayerWidth, layer.length);
    });

    const totalWidth = maxLayerWidth * (NODE_WIDTH + NODE_MARGIN_X) - NODE_MARGIN_X + PADDING * 2;
    const totalHeight = layers.length * (NODE_HEIGHT + NODE_MARGIN_Y) - NODE_MARGIN_Y + PADDING * 2;

    layers.forEach((layer, layerIndex) => {
      const layerWidth = layer.length * (NODE_WIDTH + NODE_MARGIN_X) - NODE_MARGIN_X;
      const startX = (totalWidth - layerWidth) / 2;
      const y = PADDING + layerIndex * (NODE_HEIGHT + NODE_MARGIN_Y);

      layer.forEach((storyId, nodeIndex) => {
        const x = startX + nodeIndex * (NODE_WIDTH + NODE_MARGIN_X);
        const story = storyMap.get(storyId)!;
        const pos: NodePosition = { x, y, story };
        nodePositions.push(pos);
        nodeMap.set(storyId, pos);
      });
    });

    // Calculate edges
    const edgeList: { from: NodePosition; to: NodePosition }[] = [];
    for (const story of prd.stories) {
      const toNode = nodeMap.get(story.id);
      if (!toNode) continue;
      for (const depId of story.dependencies) {
        const fromNode = nodeMap.get(depId);
        if (fromNode) {
          edgeList.push({ from: fromNode, to: toNode });
        }
      }
    }

    return {
      nodes: nodePositions,
      edges: edgeList,
      width: Math.max(totalWidth, 400),
      height: Math.max(totalHeight, 200),
    };
  }, [prd.stories]);

  if (prd.stories.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500 dark:text-gray-400">
        <div className="text-center">
          <p className="text-lg mb-2">No stories to visualize</p>
          <p className="text-sm">Add stories to see the dependency graph</p>
        </div>
      </div>
    );
  }

  return (
    <div className="w-full h-full overflow-auto bg-gray-50 dark:bg-gray-900 rounded-lg">
      <svg
        ref={svgRef}
        width={width}
        height={height}
        className="min-w-full"
      >
        {/* Arrow marker definition */}
        <defs>
          <marker
            id="arrowhead"
            markerWidth="10"
            markerHeight="7"
            refX="9"
            refY="3.5"
            orient="auto"
          >
            <polygon
              points="0 0, 10 3.5, 0 7"
              fill="#9ca3af"
            />
          </marker>
          <marker
            id="arrowhead-selected"
            markerWidth="10"
            markerHeight="7"
            refX="9"
            refY="3.5"
            orient="auto"
          >
            <polygon
              points="0 0, 10 3.5, 0 7"
              fill="#6366f1"
            />
          </marker>
        </defs>

        {/* Edges */}
        {edges.map(({ from, to }, index) => {
          const isSelected = selectedStory === from.story.id || selectedStory === to.story.id;
          const startX = from.x + NODE_WIDTH / 2;
          const startY = from.y + NODE_HEIGHT;
          const endX = to.x + NODE_WIDTH / 2;
          const endY = to.y;

          // Curved path
          const midY = (startY + endY) / 2;
          const path = `M ${startX} ${startY} C ${startX} ${midY}, ${endX} ${midY}, ${endX} ${endY}`;

          return (
            <path
              key={index}
              d={path}
              fill="none"
              stroke={isSelected ? '#6366f1' : '#9ca3af'}
              strokeWidth={isSelected ? 2 : 1.5}
              strokeDasharray={isSelected ? 'none' : '4'}
              markerEnd={isSelected ? 'url(#arrowhead-selected)' : 'url(#arrowhead)'}
              className="transition-all"
            />
          );
        })}

        {/* Nodes */}
        {nodes.map(({ x, y, story }) => {
          const colors = statusColors[story.status];
          const isSelected = selectedStory === story.id;
          const storyIndex = prd.stories.findIndex((s) => s.id === story.id) + 1;

          return (
            <g
              key={story.id}
              transform={`translate(${x}, ${y})`}
              onClick={() => setSelectedStory(isSelected ? null : story.id)}
              className="cursor-pointer"
            >
              {/* Node background */}
              <rect
                x={0}
                y={0}
                width={NODE_WIDTH}
                height={NODE_HEIGHT}
                rx={8}
                fill={colors.bg}
                stroke={isSelected ? '#6366f1' : colors.border}
                strokeWidth={isSelected ? 2 : 1.5}
                className="transition-all"
              />

              {/* Story number badge */}
              <circle
                cx={20}
                cy={NODE_HEIGHT / 2}
                r={12}
                fill={colors.border}
              />
              <text
                x={20}
                y={NODE_HEIGHT / 2 + 4}
                textAnchor="middle"
                fontSize={10}
                fontWeight="bold"
                fill="white"
              >
                {storyIndex}
              </text>

              {/* Story title */}
              <text
                x={40}
                y={NODE_HEIGHT / 2 - 4}
                fontSize={12}
                fontWeight="500"
                fill={colors.text}
                className="truncate"
              >
                <tspan>
                  {story.title.length > 16
                    ? story.title.substring(0, 16) + '...'
                    : story.title}
                </tspan>
              </text>

              {/* Status label */}
              <text
                x={40}
                y={NODE_HEIGHT / 2 + 12}
                fontSize={10}
                fill={colors.border}
              >
                {story.status.replace('_', ' ')}
              </text>

              {/* Dependency count badge */}
              {story.dependencies.length > 0 && (
                <>
                  <circle
                    cx={NODE_WIDTH - 15}
                    cy={15}
                    r={10}
                    fill="#fbbf24"
                  />
                  <text
                    x={NODE_WIDTH - 15}
                    y={19}
                    textAnchor="middle"
                    fontSize={10}
                    fontWeight="bold"
                    fill="white"
                  >
                    {story.dependencies.length}
                  </text>
                </>
              )}
            </g>
          );
        })}
      </svg>
    </div>
  );
}

/**
 * PRD Preview Panel
 *
 * Shows PRD summary alongside the dependency graph.
 */
export function PRDPreviewPanel() {
  const { prd } = usePRDStore();

  const stats = useMemo(() => {
    const total = prd.stories.length;
    const completed = prd.stories.filter((s) => s.status === 'completed').length;
    const inProgress = prd.stories.filter((s) => s.status === 'in_progress').length;
    const pending = prd.stories.filter((s) => s.status === 'pending').length;
    const failed = prd.stories.filter((s) => s.status === 'failed').length;
    const withDeps = prd.stories.filter((s) => s.dependencies.length > 0).length;

    return { total, completed, inProgress, pending, failed, withDeps };
  }, [prd.stories]);

  return (
    <div className="h-full flex flex-col">
      {/* Summary header */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-3">
          {prd.title || 'Untitled PRD'}
        </h3>

        {/* Stats */}
        <div className="grid grid-cols-3 gap-3 text-center">
          <div className="p-2 rounded-lg bg-gray-100 dark:bg-gray-800">
            <p className="text-2xl font-bold text-gray-900 dark:text-white">
              {stats.total}
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400">Stories</p>
          </div>
          <div className="p-2 rounded-lg bg-green-100 dark:bg-green-900/30">
            <p className="text-2xl font-bold text-green-600 dark:text-green-400">
              {stats.completed}
            </p>
            <p className="text-xs text-green-600 dark:text-green-400">Complete</p>
          </div>
          <div className="p-2 rounded-lg bg-amber-100 dark:bg-amber-900/30">
            <p className="text-2xl font-bold text-amber-600 dark:text-amber-400">
              {stats.withDeps}
            </p>
            <p className="text-xs text-amber-600 dark:text-amber-400">With Deps</p>
          </div>
        </div>

        {/* Progress bar */}
        {stats.total > 0 && (
          <div className="mt-3">
            <div className="flex justify-between text-xs text-gray-500 dark:text-gray-400 mb-1">
              <span>Progress</span>
              <span>{Math.round((stats.completed / stats.total) * 100)}%</span>
            </div>
            <div className="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
              <div
                className="h-full bg-green-500 transition-all duration-300"
                style={{ width: `${(stats.completed / stats.total) * 100}%` }}
              />
            </div>
          </div>
        )}
      </div>

      {/* Dependency graph */}
      <div className="flex-1 min-h-0">
        <DependencyGraph />
      </div>
    </div>
  );
}

export default DependencyGraph;
