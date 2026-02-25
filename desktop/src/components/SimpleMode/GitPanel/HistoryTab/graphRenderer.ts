/**
 * Graph Renderer Utilities
 *
 * SVG path generation for the commit history graph.
 * Renders nodes (circles/diamonds) and edges with professional
 * "railroad track" style: vertical lines + compact S-curve transitions.
 *
 * Feature-003: Commit History Graph with SVG Visualization
 */

import type { GraphNode, GraphEdge } from '../../../../types/git';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Width of each lane in pixels */
export const LANE_WIDTH = 16;

/** Height of each row in pixels (must match CommitRow height) */
export const ROW_HEIGHT = 36;

/** Radius of regular commit node circles */
export const NODE_RADIUS = 4;

/** Radius of HEAD commit node circles */
export const HEAD_NODE_RADIUS = 5;

/** Size of merge commit diamonds (half-diagonal) */
export const MERGE_DIAMOND_SIZE = 4;

/** Left padding before the first lane */
export const GRAPH_LEFT_PADDING = 12;

/**
 * 8-color cycling palette for branch coloring.
 * Colors chosen for good contrast in both light and dark modes.
 */
export const LANE_COLORS = [
  '#4C9AFF', // blue
  '#36B37E', // green
  '#FF5630', // red
  '#FFAB00', // amber
  '#6554C0', // purple
  '#00B8D9', // cyan
  '#FF8B00', // orange
  '#E774BB', // pink
] as const;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Get the color for a given lane index (cycles through the palette). */
export function getLaneColor(lane: number): string {
  return LANE_COLORS[lane % LANE_COLORS.length];
}

/** Get the X center position for a given lane. */
export function laneX(lane: number): number {
  return GRAPH_LEFT_PADDING + lane * LANE_WIDTH + LANE_WIDTH / 2;
}

/** Get the Y center position for a given row. */
export function rowY(row: number): number {
  return row * ROW_HEIGHT + ROW_HEIGHT / 2;
}

/**
 * Total SVG graph width for a given max_lane value.
 * Adds padding on both sides.
 */
export function graphWidth(maxLane: number): number {
  return GRAPH_LEFT_PADDING + (maxLane + 1) * LANE_WIDTH + GRAPH_LEFT_PADDING;
}

// ---------------------------------------------------------------------------
// SVG Path Generation
// ---------------------------------------------------------------------------

/**
 * Generate an SVG path for an edge between two nodes.
 *
 * Railroad-track style:
 * - Same lane → straight vertical line
 * - Cross-lane → compact S-curve near the child, then straight vertical
 *
 * The S-curve `C x1 (y1+h), x2 (y1+h), x2 (y1+2h)` smoothly transitions
 * from moving vertically at x1 to moving vertically at x2 in just 2h pixels
 * of vertical space. The rest of the edge is a clean straight line.
 */
export function edgePath(edge: GraphEdge): string {
  const x1 = laneX(edge.from_lane);
  const y1 = rowY(edge.from_row);
  const x2 = laneX(edge.to_lane);
  const y2 = rowY(edge.to_row);

  // Same lane: straight vertical line
  if (edge.from_lane === edge.to_lane) {
    return `M ${x1} ${y1} L ${x2} ${y2}`;
  }

  const totalDy = y2 - y1;

  // The S-curve transition height — adapts to available space.
  // Uses ~1 row height, clamped so it never exceeds half the total distance.
  const curveH = Math.min(ROW_HEIGHT, totalDy / 2);

  // S-curve starts right at the child node and transitions to the parent lane.
  // After the curve, a straight vertical line reaches the parent node.
  const curveEnd = y1 + curveH * 2;

  let d = `M ${x1} ${y1} C ${x1} ${y1 + curveH}, ${x2} ${y1 + curveH}, ${x2} ${curveEnd}`;

  if (curveEnd < y2 - 0.5) {
    d += ` L ${x2} ${y2}`;
  }

  return d;
}

/**
 * Get the edge color based on the branch lane.
 * Merge-source edges use the source branch color (to_lane).
 * Same-lane edges use their own lane color.
 */
export function edgeColor(edge: GraphEdge): string {
  return getLaneColor(edge.from_lane === edge.to_lane ? edge.from_lane : edge.to_lane);
}

// ---------------------------------------------------------------------------
// Node Rendering Data
// ---------------------------------------------------------------------------

export interface NodeRenderData {
  cx: number;
  cy: number;
  color: string;
  isMerge: boolean;
  isHead: boolean;
  shapePath: string;
}

/**
 * Compute render data for a graph node.
 */
export function nodeRenderData(node: GraphNode, isMerge: boolean, isHead: boolean): NodeRenderData {
  const cx = laneX(node.lane);
  const cy = rowY(node.row);
  const color = getLaneColor(node.lane);

  let shapePath: string;

  if (isMerge) {
    const s = MERGE_DIAMOND_SIZE;
    shapePath = `M ${cx} ${cy - s} L ${cx + s} ${cy} L ${cx} ${cy + s} L ${cx - s} ${cy} Z`;
  } else {
    const r = isHead ? HEAD_NODE_RADIUS : NODE_RADIUS;
    shapePath = `M ${cx - r} ${cy} A ${r} ${r} 0 1 1 ${cx + r} ${cy} A ${r} ${r} 0 1 1 ${cx - r} ${cy}`;
  }

  return { cx, cy, color, isMerge, isHead, shapePath };
}

// ---------------------------------------------------------------------------
// Batch Rendering for Virtual Scrolling
// ---------------------------------------------------------------------------

export interface RenderedEdge {
  key: string;
  d: string;
  color: string;
}

export interface RenderedNode {
  key: string;
  data: NodeRenderData;
}

/**
 * Pre-compute edges that are at least partially visible in the row range.
 */
export function renderEdgesForRange(edges: GraphEdge[], startRow: number, endRow: number): RenderedEdge[] {
  return edges
    .filter((edge) => {
      const minRow = Math.min(edge.from_row, edge.to_row);
      const maxRow = Math.max(edge.from_row, edge.to_row);
      return maxRow >= startRow && minRow <= endRow;
    })
    .map((edge) => ({
      key: `${edge.from_sha}-${edge.to_sha}`,
      d: edgePath(edge),
      color: edgeColor(edge),
    }));
}

/**
 * Pre-compute nodes visible in the row range.
 */
export function renderNodesForRange(
  nodes: GraphNode[],
  startRow: number,
  endRow: number,
  commitParentCounts: Map<string, number>,
  headSha: string | null,
): RenderedNode[] {
  return nodes
    .filter((node) => node.row >= startRow && node.row <= endRow)
    .map((node) => ({
      key: node.sha,
      data: nodeRenderData(node, (commitParentCounts.get(node.sha) ?? 0) > 1, node.sha === headSha),
    }));
}
