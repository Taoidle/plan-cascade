/**
 * Graph Renderer Utilities
 *
 * SVG path generation for the commit history graph.
 * Renders nodes (circles/diamonds) and edges (straight lines/Bezier curves)
 * with an 8-color cycling palette for branch coloring.
 *
 * Feature-003: Commit History Graph with SVG Visualization
 */

import type { GraphNode, GraphEdge } from '../../../../types/git';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Width of each lane in pixels */
export const LANE_WIDTH = 20;

/** Height of each row in pixels (must match CommitRow height) */
export const ROW_HEIGHT = 36;

/** Radius of regular commit node circles */
export const NODE_RADIUS = 5;

/** Radius of HEAD commit node circles */
export const HEAD_NODE_RADIUS = 7;

/** Size of merge commit diamonds (half-diagonal) */
export const MERGE_DIAMOND_SIZE = 5;

/** Left padding before the first lane */
export const GRAPH_LEFT_PADDING = 14;

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
 * Generate an SVG path `d` attribute for an edge between two nodes.
 *
 * - Same-lane connections: straight vertical line.
 * - Cross-lane connections: Bezier curve to smoothly connect different lanes.
 */
export function edgePath(edge: GraphEdge): string {
  const x1 = laneX(edge.from_lane);
  const y1 = rowY(edge.from_row);
  const x2 = laneX(edge.to_lane);
  const y2 = rowY(edge.to_row);

  // Same lane: straight line
  if (edge.from_lane === edge.to_lane) {
    return `M ${x1} ${y1} L ${x2} ${y2}`;
  }

  // Cross-lane: Bezier curve
  // Control points create a smooth S-curve:
  // - First control point drops down vertically from the source
  // - Second control point rises up vertically to the target
  const midY = (y1 + y2) / 2;
  return `M ${x1} ${y1} C ${x1} ${midY}, ${x2} ${midY}, ${x2} ${y2}`;
}

/**
 * Get the edge color based on the source lane.
 * The edge inherits the color of the lane it originates from.
 */
export function edgeColor(edge: GraphEdge): string {
  // Use the "to" lane color for the edge so that merge lines
  // adopt the color of the branch being merged
  return getLaneColor(edge.from_lane === edge.to_lane ? edge.from_lane : edge.to_lane);
}

// ---------------------------------------------------------------------------
// Node Rendering Data
// ---------------------------------------------------------------------------

export interface NodeRenderData {
  /** Center X coordinate */
  cx: number;
  /** Center Y coordinate */
  cy: number;
  /** Lane color */
  color: string;
  /** Whether this is a merge commit (diamond shape) */
  isMerge: boolean;
  /** Whether this is the HEAD commit */
  isHead: boolean;
  /** SVG element string for the node */
  shapePath: string;
}

/**
 * Compute render data for a graph node.
 *
 * @param node - The graph node with lane/row assignments.
 * @param isMerge - Whether this commit has multiple parents.
 * @param isHead - Whether this is the current HEAD commit.
 */
export function nodeRenderData(
  node: GraphNode,
  isMerge: boolean,
  isHead: boolean,
): NodeRenderData {
  const cx = laneX(node.lane);
  const cy = rowY(node.row);
  const color = getLaneColor(node.lane);

  let shapePath: string;

  if (isMerge) {
    // Diamond shape for merge commits
    const s = MERGE_DIAMOND_SIZE;
    shapePath = `M ${cx} ${cy - s} L ${cx + s} ${cy} L ${cx} ${cy + s} L ${cx - s} ${cy} Z`;
  } else if (isHead) {
    // Larger circle for HEAD, rendered as a circle element (use cx, cy, r)
    // Represent as a circular path for consistency
    const r = HEAD_NODE_RADIUS;
    shapePath = `M ${cx - r} ${cy} A ${r} ${r} 0 1 1 ${cx + r} ${cy} A ${r} ${r} 0 1 1 ${cx - r} ${cy}`;
  } else {
    // Regular circle
    const r = NODE_RADIUS;
    shapePath = `M ${cx - r} ${cy} A ${r} ${r} 0 1 1 ${cx + r} ${cy} A ${r} ${r} 0 1 1 ${cx - r} ${cy}`;
  }

  return { cx, cy, color, isMerge, isHead, shapePath };
}

// ---------------------------------------------------------------------------
// SVG Rendering Helpers for Batch Operations
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
 * Pre-compute all edges for a range of rows (for virtual scrolling).
 * Only includes edges that are at least partially visible in the range.
 */
export function renderEdgesForRange(
  edges: GraphEdge[],
  startRow: number,
  endRow: number,
): RenderedEdge[] {
  return edges
    .filter((edge) => {
      // Include edge if either endpoint is in the visible range,
      // or if the edge spans across the visible range
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
 * Pre-compute all nodes for a range of rows (for virtual scrolling).
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
      data: nodeRenderData(
        node,
        (commitParentCounts.get(node.sha) ?? 0) > 1,
        node.sha === headSha,
      ),
    }));
}
