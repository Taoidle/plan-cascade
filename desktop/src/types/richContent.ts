/**
 * Rich Content Types
 *
 * TypeScript types mirroring the Rust AgentEvent::RichContent variant.
 * Used by DynamicRenderer to render structured content from agent events.
 *
 * Story 001: AgentEvent::RichContent variant and backend types
 */

// ============================================================================
// Core RichContent Type
// ============================================================================

/** Component types supported by DynamicRenderer */
export type RichContentComponentType =
  | 'table'
  | 'chart'
  | 'diff'
  | 'action_buttons';

/** RichContent event payload from agent stream */
export interface RichContentEvent {
  /** Component type determining which renderer to use */
  componentType: RichContentComponentType | string;
  /** Structured JSON data consumed by the component */
  data: unknown;
  /** Optional surface ID for update/replace semantics */
  surfaceId?: string;
}

// ============================================================================
// Table Data
// ============================================================================

/** Column definition for DataTable */
export interface TableColumn {
  /** Column key/accessor */
  key: string;
  /** Display header label */
  label: string;
  /** Whether this column is sortable */
  sortable?: boolean;
  /** Optional width (CSS value) */
  width?: string;
}

/** Data payload for 'table' component type */
export interface TableData {
  /** Column definitions */
  columns: TableColumn[];
  /** Row data (array of key-value objects) */
  rows: Record<string, unknown>[];
  /** Optional table title */
  title?: string;
}

// ============================================================================
// Chart Data
// ============================================================================

/** Data payload for 'chart' component type (story progress) */
export interface ChartData {
  /** Chart title */
  title?: string;
  /** Data points for progress visualization */
  items: ChartDataItem[];
  /** Total value for percentage calculation */
  total?: number;
}

/** Individual chart data item */
export interface ChartDataItem {
  /** Label for this data point */
  label: string;
  /** Numeric value */
  value: number;
  /** Optional color (CSS color value) */
  color?: string;
  /** Optional status for semantic coloring */
  status?: 'success' | 'error' | 'warning' | 'info' | 'neutral';
}

// ============================================================================
// Diff Data
// ============================================================================

/** Data payload for 'diff' component type */
export interface DiffData {
  /** Original content */
  old: string;
  /** Modified content */
  new: string;
  /** File path for context */
  file?: string;
  /** Language for syntax highlighting */
  language?: string;
}

// ============================================================================
// Action Buttons Data
// ============================================================================

/** Button variant for visual styling */
export type ActionButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger';

/** Individual action button definition */
export interface ActionButton {
  /** Unique action identifier */
  id: string;
  /** Display label */
  label: string;
  /** Visual variant */
  variant?: ActionButtonVariant;
  /** Whether the button is disabled */
  disabled?: boolean;
  /** Optional icon name */
  icon?: string;
}

/** Data payload for 'action_buttons' component type */
export interface ActionButtonsData {
  /** Array of action button definitions */
  actions: ActionButton[];
  /** Optional group label */
  label?: string;
}

// ============================================================================
// Surface Map
// ============================================================================

/** Map of surface IDs to their current RichContent */
export type SurfaceMap = Map<string, RichContentEvent>;
