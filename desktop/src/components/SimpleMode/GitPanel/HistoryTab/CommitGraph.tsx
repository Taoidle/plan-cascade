/**
 * CommitGraph Component
 *
 * SVG+HTML hybrid commit history graph with virtual scrolling.
 * Left side: SVG canvas for DAG lines and nodes.
 * Right side: HTML commit info rows (author, message, date, ref badges).
 *
 * Virtual scrolling renders only visible rows + 10-row buffer for performance.
 * Fixed 36px row height ensures perfect SVG-HTML alignment.
 *
 * Feature-003: Commit History Graph with SVG Visualization
 */

import { useRef, useMemo, useCallback, useEffect, useState } from 'react';
import type { GraphLayout, CommitNode } from '../../../../types/git';
import CommitRow from './CommitRow';
import {
  ROW_HEIGHT,
  graphWidth,
  getLaneColor,
  renderEdgesForRange,
  renderNodesForRange,
} from './graphRenderer';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Number of buffer rows above and below the visible area */
const BUFFER_ROWS = 10;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface CommitGraphProps {
  /** Graph layout computed by the Rust backend */
  graphLayout: GraphLayout;
  /** Commit nodes matching the graph layout */
  commits: CommitNode[];
  /** Currently selected commit SHA */
  selectedCommitSha: string | null;
  /** Second selected commit for compare mode */
  compareCommitSha: string | null;
  /** Callback when a commit is selected */
  onSelectCommit: (sha: string) => void;
  /** Callback when a commit is shift-clicked for compare */
  onCompareCommit: (sha: string) => void;
  /** Callback when user right-clicks a commit */
  onContextMenu: (sha: string, event: React.MouseEvent) => void;
  /** Callback when scrolled to the bottom (for infinite scroll) */
  onScrollToBottom?: () => void;
  /** SHA of the current HEAD commit */
  headSha: string | null;
  /** Search query for highlighting matches */
  searchQuery: string;
}

// ---------------------------------------------------------------------------
// CommitGraph Component
// ---------------------------------------------------------------------------

export function CommitGraph({
  graphLayout,
  commits,
  selectedCommitSha,
  compareCommitSha,
  onSelectCommit,
  onCompareCommit,
  onContextMenu,
  onScrollToBottom,
  headSha,
  searchQuery,
}: CommitGraphProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [containerHeight, setContainerHeight] = useState(0);

  // ---------------------------------------------------------------------------
  // SHA to commit lookup
  // ---------------------------------------------------------------------------

  const shaToCommit = useMemo(() => {
    const map = new Map<string, CommitNode>();
    for (const commit of commits) {
      map.set(commit.sha, commit);
    }
    return map;
  }, [commits]);

  // ---------------------------------------------------------------------------
  // Commit parent counts (for merge detection)
  // ---------------------------------------------------------------------------

  const commitParentCounts = useMemo(() => {
    const map = new Map<string, number>();
    for (const commit of commits) {
      map.set(commit.sha, commit.parents.length);
    }
    return map;
  }, [commits]);

  // ---------------------------------------------------------------------------
  // Computed dimensions
  // ---------------------------------------------------------------------------

  const totalRows = graphLayout.nodes.length;
  const totalHeight = totalRows * ROW_HEIGHT;
  const svgWidth = graphWidth(graphLayout.max_lane);

  // ---------------------------------------------------------------------------
  // Virtual scrolling range
  // ---------------------------------------------------------------------------

  const { startRow, endRow } = useMemo(() => {
    const start = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - BUFFER_ROWS);
    const visibleRows = Math.ceil(containerHeight / ROW_HEIGHT);
    const end = Math.min(totalRows - 1, start + visibleRows + 2 * BUFFER_ROWS);
    return { startRow: start, endRow: end };
  }, [scrollTop, containerHeight, totalRows]);

  // ---------------------------------------------------------------------------
  // Pre-compute rendered edges and nodes for visible range
  // ---------------------------------------------------------------------------

  const renderedEdges = useMemo(
    () => renderEdgesForRange(graphLayout.edges, startRow, endRow),
    [graphLayout.edges, startRow, endRow]
  );

  const renderedNodes = useMemo(
    () =>
      renderNodesForRange(
        graphLayout.nodes,
        startRow,
        endRow,
        commitParentCounts,
        headSha,
      ),
    [graphLayout.nodes, startRow, endRow, commitParentCounts, headSha]
  );

  // ---------------------------------------------------------------------------
  // Visible commits (ordered by row)
  // ---------------------------------------------------------------------------

  const visibleRows = useMemo(() => {
    const rows: { node: (typeof graphLayout.nodes)[0]; commit: CommitNode }[] = [];
    for (const node of graphLayout.nodes) {
      if (node.row >= startRow && node.row <= endRow) {
        const commit = shaToCommit.get(node.sha);
        if (commit) {
          rows.push({ node, commit });
        }
      }
    }
    rows.sort((a, b) => a.node.row - b.node.row);
    return rows;
  }, [graphLayout.nodes, startRow, endRow, shaToCommit]);

  // ---------------------------------------------------------------------------
  // Scroll handler
  // ---------------------------------------------------------------------------

  const handleScroll = useCallback(
    (e: React.UIEvent<HTMLDivElement>) => {
      const target = e.currentTarget;
      setScrollTop(target.scrollTop);

      // Detect scroll to bottom for infinite scroll
      if (
        onScrollToBottom &&
        target.scrollHeight - target.scrollTop - target.clientHeight < ROW_HEIGHT * 3
      ) {
        onScrollToBottom();
      }
    },
    [onScrollToBottom]
  );

  // ---------------------------------------------------------------------------
  // Resize observer for container height
  // ---------------------------------------------------------------------------

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setContainerHeight(entry.contentRect.height);
      }
    });

    observer.observe(container);
    setContainerHeight(container.clientHeight);

    return () => observer.disconnect();
  }, []);

  // ---------------------------------------------------------------------------
  // Click handler with shift detection for compare mode
  // ---------------------------------------------------------------------------

  const handleRowClick = useCallback(
    (sha: string, e: React.MouseEvent) => {
      if (e.shiftKey && selectedCommitSha) {
        onCompareCommit(sha);
      } else {
        onSelectCommit(sha);
      }
    },
    [selectedCommitSha, onSelectCommit, onCompareCommit]
  );

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  if (totalRows === 0) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-gray-500 dark:text-gray-400">
        No commits to display
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className="flex-1 min-h-0 overflow-auto"
      onScroll={handleScroll}
    >
      {/* Virtual scroll container */}
      <div
        style={{ height: totalHeight, position: 'relative' }}
        className="flex"
      >
        {/* SVG graph column (left) */}
        <div
          className="shrink-0 sticky left-0 z-10 bg-white dark:bg-gray-900"
          style={{ width: svgWidth }}
        >
          <svg
            width={svgWidth}
            height={totalHeight}
            className="absolute top-0 left-0"
          >
            {/* Edges */}
            {renderedEdges.map((edge) => (
              <path
                key={edge.key}
                d={edge.d}
                stroke={edge.color}
                strokeWidth={1.5}
                fill="none"
                strokeLinecap="round"
              />
            ))}

            {/* Nodes */}
            {renderedNodes.map((node) => (
              <g key={node.key}>
                {/* Glow effect for HEAD */}
                {node.data.isHead && (
                  <circle
                    cx={node.data.cx}
                    cy={node.data.cy}
                    r={10}
                    fill="none"
                    stroke={node.data.color}
                    strokeWidth={1}
                    opacity={0.4}
                  />
                )}
                <path
                  d={node.data.shapePath}
                  fill={node.data.color}
                  stroke="none"
                />
              </g>
            ))}
          </svg>
        </div>

        {/* HTML commit rows (right) */}
        <div className="flex-1 min-w-0">
          {/* Spacer for rows above the visible range */}
          <div style={{ height: startRow * ROW_HEIGHT }} />

          {/* Visible rows */}
          {visibleRows.map(({ node, commit }) => (
            <CommitRow
              key={commit.sha}
              commit={commit}
              isSelected={commit.sha === selectedCommitSha}
              isCompareTarget={commit.sha === compareCommitSha}
              isHead={commit.sha === headSha}
              laneColor={getLaneColor(node.lane)}
              searchQuery={searchQuery}
              onClick={(e) => handleRowClick(commit.sha, e)}
              onContextMenu={(e) => onContextMenu(commit.sha, e)}
            />
          ))}

          {/* Spacer for rows below the visible range */}
          <div
            style={{
              height: Math.max(0, (totalRows - endRow - 1) * ROW_HEIGHT),
            }}
          />
        </div>
      </div>
    </div>
  );
}

export default CommitGraph;
