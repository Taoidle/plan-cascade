/**
 * GraphEdgeComponent
 *
 * Renders an SVG line/arrow between two nodes.
 * Dashed lines for conditional edges, solid for direct edges.
 */

import type { Edge, GraphNode } from '../../types/graphWorkflow';

interface GraphEdgeComponentProps {
  edge: Edge;
  index: number;
  nodes: Record<string, GraphNode>;
  isSelected: boolean;
  onClick: () => void;
}

/** Default node size for calculating edge endpoints */
const NODE_WIDTH = 140;
const NODE_HEIGHT = 60;

export function GraphEdgeComponent({ edge, index: _index, nodes, isSelected, onClick }: GraphEdgeComponentProps) {
  const fromNode = nodes[edge.from];
  if (!fromNode?.position) return null;

  // Get target node(s)
  let toNodeId: string | null = null;
  if (edge.edge_type === 'direct') {
    toNodeId = edge.to;
  } else {
    // For conditional edges, draw to default_branch if set
    toNodeId = edge.default_branch ?? null;
  }

  if (!toNodeId) return null;
  const toNode = nodes[toNodeId];
  if (!toNode?.position) return null;

  const fromX = fromNode.position.x + NODE_WIDTH / 2;
  const fromY = fromNode.position.y + NODE_HEIGHT;
  const toX = toNode.position.x + NODE_WIDTH / 2;
  const toY = toNode.position.y;

  const isConditional = edge.edge_type === 'conditional';

  // Calculate arrowhead
  const angle = Math.atan2(toY - fromY, toX - fromX);
  const arrowLen = 10;
  const ax1 = toX - arrowLen * Math.cos(angle - Math.PI / 6);
  const ay1 = toY - arrowLen * Math.sin(angle - Math.PI / 6);
  const ax2 = toX - arrowLen * Math.cos(angle + Math.PI / 6);
  const ay2 = toY - arrowLen * Math.sin(angle + Math.PI / 6);

  return (
    <g
      className="pointer-events-auto cursor-pointer"
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
    >
      {/* Invisible wider line for easier clicking */}
      <line x1={fromX} y1={fromY} x2={toX} y2={toY} stroke="transparent" strokeWidth={12} />

      {/* Visible line */}
      <line
        x1={fromX}
        y1={fromY}
        x2={toX}
        y2={toY}
        stroke={isSelected ? '#3b82f6' : isConditional ? '#f59e0b' : '#6b7280'}
        strokeWidth={isSelected ? 2.5 : 2}
        strokeDasharray={isConditional ? '6,4' : 'none'}
      />

      {/* Arrowhead */}
      <polygon
        points={`${toX},${toY} ${ax1},${ay1} ${ax2},${ay2}`}
        fill={isSelected ? '#3b82f6' : isConditional ? '#f59e0b' : '#6b7280'}
      />

      {/* Conditional label */}
      {isConditional && edge.edge_type === 'conditional' && (
        <text
          x={(fromX + toX) / 2}
          y={(fromY + toY) / 2 - 8}
          textAnchor="middle"
          className="text-[10px] fill-amber-600 dark:fill-amber-400"
        >
          {edge.condition.condition_key}
        </text>
      )}
    </g>
  );
}
