/**
 * DynamicRenderer Component
 *
 * Routes RichContent events to the appropriate sub-component based on
 * component_type. Manages a surface map for update/replace semantics
 * when surface_id is present.
 *
 * Routing:
 * - 'table'          -> DataTable
 * - 'chart'          -> ProgressChart
 * - 'diff'           -> DiffViewer
 * - 'action_buttons' -> ActionGroup
 * - unknown          -> MarkdownFallback
 *
 * Story 002: DynamicRenderer frontend component
 */

import { useCallback, useMemo } from 'react';
import type {
  RichContentEvent,
  SurfaceMap,
  TableData,
  ChartData,
  DiffData,
  ActionButtonsData,
} from '../../types/richContent';
import { DataTable } from './DataTable';
import { ProgressChart } from './ProgressChart';
import { DiffViewer } from './DiffViewer';
import { ActionGroup } from './ActionGroup';
import { MarkdownFallback } from './MarkdownFallback';

// ============================================================================
// Types
// ============================================================================

interface DynamicRendererProps {
  /** Array of RichContent events to render */
  events: RichContentEvent[];
  /** Optional callback when an action button is clicked */
  onAction?: (actionId: string) => void;
}

// ============================================================================
// Component
// ============================================================================

export function DynamicRenderer({ events, onAction }: DynamicRendererProps) {
  // Build surface map: surface_id -> latest event
  const surfaceMap = useMemo<SurfaceMap>(() => {
    const map: SurfaceMap = new Map();
    const unsurfaced: RichContentEvent[] = [];

    for (const event of events) {
      if (event.surfaceId) {
        // Update/replace: last event with this surface_id wins
        map.set(event.surfaceId, event);
      } else {
        unsurfaced.push(event);
      }
    }

    // Add unsurfaced events with auto-generated keys
    for (let i = 0; i < unsurfaced.length; i++) {
      map.set(`__auto_${i}`, unsurfaced[i]);
    }

    return map;
  }, [events]);

  const renderEvent = useCallback(
    (event: RichContentEvent, key: string) => {
      switch (event.componentType) {
        case 'table':
          return <DataTable key={key} data={event.data as TableData} />;

        case 'chart':
          return <ProgressChart key={key} data={event.data as ChartData} />;

        case 'diff':
          return <DiffViewer key={key} data={event.data as DiffData} />;

        case 'action_buttons':
          return <ActionGroup key={key} data={event.data as ActionButtonsData} onAction={onAction} />;

        default:
          return <MarkdownFallback key={key} componentType={event.componentType} data={event.data} />;
      }
    },
    [onAction],
  );

  if (surfaceMap.size === 0) return null;

  return (
    <div className="space-y-4" data-testid="dynamic-renderer">
      {Array.from(surfaceMap.entries()).map(([key, event]) => renderEvent(event, key))}
    </div>
  );
}

export default DynamicRenderer;
