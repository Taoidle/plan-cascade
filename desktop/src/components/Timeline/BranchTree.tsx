/**
 * BranchTree Component
 *
 * SVG-based tree visualization for timeline branches.
 * Shows branch structure with color-coded branches and node circles.
 */

import { useMemo, useCallback, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { Checkpoint, CheckpointBranch, TimelineMetadata } from '../../types/timeline';

interface BranchTreeProps {
  timeline: TimelineMetadata;
  selectedCheckpointId: string | null;
  onCheckpointSelect: (checkpoint: Checkpoint) => void;
  onCheckpointHover?: (checkpoint: Checkpoint | null) => void;
}

// Branch colors for visualization
const BRANCH_COLORS = [
  '#3B82F6', // blue (main)
  '#8B5CF6', // purple
  '#10B981', // green
  '#F59E0B', // amber
  '#EF4444', // red
  '#EC4899', // pink
  '#14B8A6', // teal
  '#6366F1', // indigo
];

interface TreeNode {
  checkpoint: Checkpoint;
  branch: CheckpointBranch | null;
  x: number;
  y: number;
  branchIndex: number;
  children: TreeNode[];
}

export function BranchTree({ timeline, selectedCheckpointId, onCheckpointSelect, onCheckpointHover }: BranchTreeProps) {
  const { t } = useTranslation();
  const [hoveredNode, setHoveredNode] = useState<TreeNode | null>(null);

  // Build tree structure from checkpoints
  const treeData = useMemo(() => {
    if (!timeline.checkpoints.length) return null;

    // Create a map of branches by ID
    const branchMap = new Map(timeline.branches.map((b) => [b.id, b]));

    // Find root checkpoints (no parent)
    const roots = timeline.checkpoints.filter((cp) => !cp.parent_id);

    // Build tree nodes recursively
    const buildNode = (checkpoint: Checkpoint, branchIndex: number, depth: number, xOffset: number): TreeNode => {
      const branch = checkpoint.branch_id ? branchMap.get(checkpoint.branch_id) || null : null;

      // Find children
      const children = timeline.checkpoints
        .filter((cp) => cp.parent_id === checkpoint.id)
        .map((child, idx) => {
          // Check if child is on a different branch
          const childBranchIndex =
            child.branch_id && child.branch_id !== checkpoint.branch_id
              ? timeline.branches.findIndex((b) => b.id === child.branch_id) % BRANCH_COLORS.length
              : branchIndex;

          return buildNode(child, childBranchIndex, depth + 1, xOffset + idx * 2);
        });

      return {
        checkpoint,
        branch,
        x: xOffset,
        y: depth,
        branchIndex,
        children,
      };
    };

    // Build trees from roots
    const trees = roots.map((root, idx) => {
      const branchIndex = root.branch_id ? timeline.branches.findIndex((b) => b.id === root.branch_id) : 0;
      return buildNode(root, branchIndex, 0, idx * 4);
    });

    return trees;
  }, [timeline]);

  // Calculate SVG dimensions
  const dimensions = useMemo(() => {
    if (!treeData) return { width: 200, height: 100 };

    let maxX = 0;
    let maxY = 0;

    const traverse = (node: TreeNode) => {
      maxX = Math.max(maxX, node.x);
      maxY = Math.max(maxY, node.y);
      node.children.forEach(traverse);
    };

    treeData.forEach(traverse);

    const nodeSpacing = 60;
    const levelSpacing = 80;
    const padding = 40;

    return {
      width: (maxX + 1) * nodeSpacing + padding * 2,
      height: (maxY + 1) * levelSpacing + padding * 2,
    };
  }, [treeData]);

  // Handle node click
  const handleNodeClick = useCallback(
    (node: TreeNode) => {
      onCheckpointSelect(node.checkpoint);
    },
    [onCheckpointSelect],
  );

  // Handle node hover
  const handleNodeHover = useCallback(
    (node: TreeNode | null) => {
      setHoveredNode(node);
      onCheckpointHover?.(node?.checkpoint || null);
    },
    [onCheckpointHover],
  );

  if (!treeData || treeData.length === 0) {
    return <div className="text-center p-4 text-gray-500 dark:text-gray-400">{t('timeline.noCheckpoints')}</div>;
  }

  const nodeRadius = 12;
  const nodeSpacing = 60;
  const levelSpacing = 80;
  const padding = 40;

  // Render a node and its children
  const renderNode = (node: TreeNode): JSX.Element => {
    const x = padding + node.x * nodeSpacing;
    const y = padding + node.y * levelSpacing;
    const color = BRANCH_COLORS[node.branchIndex % BRANCH_COLORS.length];
    const isSelected = node.checkpoint.id === selectedCheckpointId;
    const isCurrent = node.checkpoint.id === timeline.current_checkpoint_id;
    const isHovered = hoveredNode?.checkpoint.id === node.checkpoint.id;

    return (
      <g key={node.checkpoint.id}>
        {/* Lines to children */}
        {node.children.map((child) => {
          const childX = padding + child.x * nodeSpacing;
          const childY = padding + child.y * levelSpacing;
          const childColor = BRANCH_COLORS[child.branchIndex % BRANCH_COLORS.length];

          return (
            <path
              key={`line-${node.checkpoint.id}-${child.checkpoint.id}`}
              d={`M ${x} ${y + nodeRadius}
                  C ${x} ${y + levelSpacing / 2},
                    ${childX} ${y + levelSpacing / 2},
                    ${childX} ${childY - nodeRadius}`}
              fill="none"
              stroke={childColor}
              strokeWidth={2}
              className="transition-opacity"
              opacity={hoveredNode && hoveredNode.checkpoint.id !== child.checkpoint.id ? 0.3 : 1}
            />
          );
        })}

        {/* Node circle */}
        <circle
          cx={x}
          cy={y}
          r={isHovered ? nodeRadius + 2 : nodeRadius}
          fill={isSelected || isCurrent ? color : 'white'}
          stroke={color}
          strokeWidth={2}
          className="cursor-pointer transition-all duration-150"
          onClick={() => handleNodeClick(node)}
          onMouseEnter={() => handleNodeHover(node)}
          onMouseLeave={() => handleNodeHover(null)}
        />

        {/* Current indicator */}
        {isCurrent && (
          <circle
            cx={x}
            cy={y}
            r={nodeRadius + 6}
            fill="none"
            stroke={color}
            strokeWidth={1}
            strokeDasharray="4 2"
            className="animate-spin-slow"
            style={{ animationDuration: '4s' }}
          />
        )}

        {/* Label */}
        <text
          x={x}
          y={y + nodeRadius + 16}
          textAnchor="middle"
          className={clsx(
            'text-xs pointer-events-none',
            isSelected || isHovered ? 'fill-gray-900 dark:fill-white font-medium' : 'fill-gray-500 dark:fill-gray-400',
          )}
        >
          {node.checkpoint.label.length > 12 ? node.checkpoint.label.slice(0, 12) + '...' : node.checkpoint.label}
        </text>

        {/* Recursively render children */}
        {node.children.map(renderNode)}
      </g>
    );
  };

  return (
    <div className="w-full overflow-auto">
      <svg width={dimensions.width} height={dimensions.height} className="min-w-full" style={{ minHeight: '200px' }}>
        <defs>
          {/* Define filters for glow effect */}
          <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur stdDeviation="2" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* Render all tree roots */}
        {treeData.map(renderNode)}
      </svg>

      {/* Tooltip for hovered node */}
      {hoveredNode && (
        <div
          className={clsx(
            'fixed z-50 px-3 py-2 rounded-lg shadow-lg',
            'bg-white dark:bg-gray-800',
            'border border-gray-200 dark:border-gray-700',
            'text-sm',
          )}
          style={{
            left: `${padding + hoveredNode.x * nodeSpacing + nodeRadius + 10}px`,
            top: `${padding + hoveredNode.y * levelSpacing - 10}px`,
            transform: 'translateY(-100%)',
          }}
        >
          <p className="font-medium text-gray-900 dark:text-white">{hoveredNode.checkpoint.label}</p>
          <p className="text-xs text-gray-500 dark:text-gray-400">
            {new Date(hoveredNode.checkpoint.timestamp).toLocaleString()}
          </p>
          <p className="text-xs text-gray-500 dark:text-gray-400">
            {hoveredNode.checkpoint.files_snapshot.length} {t('timeline.files')}
          </p>
          {hoveredNode.branch && (
            <p className="text-xs text-gray-500 dark:text-gray-400">Branch: {hoveredNode.branch.name}</p>
          )}
        </div>
      )}
    </div>
  );
}

export default BranchTree;
