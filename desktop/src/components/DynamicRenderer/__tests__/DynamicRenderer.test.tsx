/**
 * DynamicRenderer Component Tests
 *
 * Story 002: DynamicRenderer frontend component
 */

import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DynamicRenderer } from '../DynamicRenderer';
import { DataTable } from '../DataTable';
import { ProgressChart } from '../ProgressChart';
import { DiffViewer } from '../DiffViewer';
import { ActionGroup } from '../ActionGroup';
import { MarkdownFallback } from '../MarkdownFallback';
import type {
  RichContentEvent,
  TableData,
  ChartData,
  DiffData,
  ActionButtonsData,
} from '../../../types/richContent';

// ============================================================================
// DataTable Tests
// ============================================================================

describe('DataTable', () => {
  const tableData: TableData = {
    columns: [
      { key: 'name', label: 'Name', sortable: true },
      { key: 'status', label: 'Status', sortable: true },
      { key: 'score', label: 'Score', sortable: true },
    ],
    rows: [
      { name: 'Story 1', status: 'completed', score: 95 },
      { name: 'Story 2', status: 'failed', score: 40 },
      { name: 'Story 3', status: 'running', score: 70 },
    ],
    title: 'Test Table',
  };

  it('renders table with correct columns', () => {
    render(<DataTable data={tableData} />);
    expect(screen.getByTestId('data-table')).toBeDefined();
    expect(screen.getByText('Name')).toBeDefined();
    expect(screen.getByText('Status')).toBeDefined();
    expect(screen.getByText('Score')).toBeDefined();
  });

  it('renders all rows', () => {
    render(<DataTable data={tableData} />);
    expect(screen.getByText('Story 1')).toBeDefined();
    expect(screen.getByText('Story 2')).toBeDefined();
    expect(screen.getByText('Story 3')).toBeDefined();
  });

  it('renders title', () => {
    render(<DataTable data={tableData} />);
    expect(screen.getByText('Test Table')).toBeDefined();
  });

  it('sorts columns when clicked', () => {
    render(<DataTable data={tableData} />);
    const nameHeader = screen.getByText('Name');

    // Click to sort ascending
    fireEvent.click(nameHeader);
    const cells = screen.getAllByRole('cell');
    // After asc sort, first row should be Story 1 (alphabetically)
    expect(cells[0].textContent).toBe('Story 1');
  });

  it('handles empty rows', () => {
    const emptyData: TableData = {
      columns: [{ key: 'name', label: 'Name' }],
      rows: [],
    };
    render(<DataTable data={emptyData} />);
    expect(screen.getByText('No data')).toBeDefined();
  });
});

// ============================================================================
// ProgressChart Tests
// ============================================================================

describe('ProgressChart', () => {
  const chartData: ChartData = {
    title: 'Story Progress',
    items: [
      { label: 'Completed', value: 3, status: 'success' },
      { label: 'Failed', value: 1, status: 'error' },
      { label: 'Pending', value: 2, status: 'neutral' },
    ],
    total: 6,
  };

  it('renders chart with title', () => {
    render(<ProgressChart data={chartData} />);
    expect(screen.getByTestId('progress-chart')).toBeDefined();
    expect(screen.getByText('Story Progress')).toBeDefined();
  });

  it('renders legend items', () => {
    render(<ProgressChart data={chartData} />);
    expect(screen.getByText('Completed:')).toBeDefined();
    expect(screen.getByText('Failed:')).toBeDefined();
    expect(screen.getByText('Pending:')).toBeDefined();
  });

  it('renders values', () => {
    render(<ProgressChart data={chartData} />);
    expect(screen.getByText('3')).toBeDefined();
    expect(screen.getByText('1')).toBeDefined();
    expect(screen.getByText('2')).toBeDefined();
  });

  it('handles auto-calculated total', () => {
    const autoData: ChartData = {
      items: [
        { label: 'A', value: 10 },
        { label: 'B', value: 20 },
      ],
    };
    render(<ProgressChart data={autoData} />);
    // Total should be 30, A = 33.3%, B = 66.7%
    expect(screen.getByText('(33.3%)')).toBeDefined();
    expect(screen.getByText('(66.7%)')).toBeDefined();
  });
});

// ============================================================================
// DiffViewer Tests
// ============================================================================

describe('DiffViewer', () => {
  const diffData: DiffData = {
    old: 'fn foo() {\n  println!("hello");\n}',
    new: 'fn foo() -> i32 {\n  println!("hello");\n  42\n}',
    file: 'src/main.rs',
    language: 'rust',
  };

  it('renders diff viewer', () => {
    render(<DiffViewer data={diffData} />);
    expect(screen.getByTestId('diff-viewer')).toBeDefined();
  });

  it('shows file name', () => {
    render(<DiffViewer data={diffData} />);
    expect(screen.getByText('src/main.rs')).toBeDefined();
  });

  it('shows language badge', () => {
    render(<DiffViewer data={diffData} />);
    expect(screen.getByText('rust')).toBeDefined();
  });

  it('renders diff lines', () => {
    render(<DiffViewer data={diffData} />);
    // Should have some rows
    const rows = screen.getByTestId('diff-viewer').querySelectorAll('tr');
    expect(rows.length).toBeGreaterThan(0);
  });
});

// ============================================================================
// ActionGroup Tests
// ============================================================================

describe('ActionGroup', () => {
  const actionsData: ActionButtonsData = {
    actions: [
      { id: 'approve', label: 'Approve', variant: 'primary' },
      { id: 'retry', label: 'Retry', variant: 'secondary' },
      { id: 'skip', label: 'Skip', variant: 'ghost' },
    ],
    label: 'Review Actions',
  };

  it('renders all buttons', () => {
    render(<ActionGroup data={actionsData} />);
    expect(screen.getByTestId('action-group')).toBeDefined();
    expect(screen.getByText('Approve')).toBeDefined();
    expect(screen.getByText('Retry')).toBeDefined();
    expect(screen.getByText('Skip')).toBeDefined();
  });

  it('renders group label', () => {
    render(<ActionGroup data={actionsData} />);
    expect(screen.getByText('Review Actions')).toBeDefined();
  });

  it('calls onAction callback when clicked', () => {
    const onAction = vi.fn();
    render(<ActionGroup data={actionsData} onAction={onAction} />);
    fireEvent.click(screen.getByText('Approve'));
    expect(onAction).toHaveBeenCalledWith('approve');
  });

  it('renders disabled buttons', () => {
    const disabledData: ActionButtonsData = {
      actions: [
        { id: 'save', label: 'Save', variant: 'primary', disabled: true },
      ],
    };
    render(<ActionGroup data={disabledData} />);
    const btn = screen.getByText('Save');
    expect(btn.closest('button')).toHaveProperty('disabled', true);
  });
});

// ============================================================================
// MarkdownFallback Tests
// ============================================================================

describe('MarkdownFallback', () => {
  it('renders unknown component type badge', () => {
    render(<MarkdownFallback componentType="custom_widget" data={{ foo: 'bar' }} />);
    expect(screen.getByTestId('markdown-fallback')).toBeDefined();
    expect(screen.getByText('custom_widget')).toBeDefined();
    expect(screen.getByText('(unknown component type)')).toBeDefined();
  });

  it('renders JSON data', () => {
    render(<MarkdownFallback componentType="unknown" data={{ key: 'value' }} />);
    const pre = screen.getByTestId('markdown-fallback').querySelector('pre');
    expect(pre?.textContent).toContain('"key"');
    expect(pre?.textContent).toContain('"value"');
  });
});

// ============================================================================
// DynamicRenderer Integration Tests
// ============================================================================

describe('DynamicRenderer', () => {
  it('routes table events to DataTable', () => {
    const events: RichContentEvent[] = [
      {
        componentType: 'table',
        data: {
          columns: [{ key: 'name', label: 'Name' }],
          rows: [{ name: 'Test' }],
        } as TableData,
      },
    ];
    render(<DynamicRenderer events={events} />);
    expect(screen.getByTestId('dynamic-renderer')).toBeDefined();
    expect(screen.getByTestId('data-table')).toBeDefined();
  });

  it('routes chart events to ProgressChart', () => {
    const events: RichContentEvent[] = [
      {
        componentType: 'chart',
        data: {
          items: [{ label: 'Done', value: 5, status: 'success' }],
        } as ChartData,
      },
    ];
    render(<DynamicRenderer events={events} />);
    expect(screen.getByTestId('progress-chart')).toBeDefined();
  });

  it('routes diff events to DiffViewer', () => {
    const events: RichContentEvent[] = [
      {
        componentType: 'diff',
        data: {
          old: 'line1',
          new: 'line2',
        } as DiffData,
      },
    ];
    render(<DynamicRenderer events={events} />);
    expect(screen.getByTestId('diff-viewer')).toBeDefined();
  });

  it('routes action_buttons events to ActionGroup', () => {
    const events: RichContentEvent[] = [
      {
        componentType: 'action_buttons',
        data: {
          actions: [{ id: 'ok', label: 'OK' }],
        } as ActionButtonsData,
      },
    ];
    render(<DynamicRenderer events={events} />);
    expect(screen.getByTestId('action-group')).toBeDefined();
  });

  it('routes unknown types to MarkdownFallback', () => {
    const events: RichContentEvent[] = [
      {
        componentType: 'magic_widget',
        data: { foo: 'bar' },
      },
    ];
    render(<DynamicRenderer events={events} />);
    expect(screen.getByTestId('markdown-fallback')).toBeDefined();
  });

  it('handles surface_id update/replace semantics', () => {
    const events: RichContentEvent[] = [
      {
        componentType: 'chart',
        data: {
          title: 'Old Progress',
          items: [{ label: 'Done', value: 1 }],
        } as ChartData,
        surfaceId: 'progress-1',
      },
      {
        componentType: 'chart',
        data: {
          title: 'Updated Progress',
          items: [{ label: 'Done', value: 5 }],
        } as ChartData,
        surfaceId: 'progress-1',
      },
    ];
    render(<DynamicRenderer events={events} />);
    // Should only render the latest event for surface_id 'progress-1'
    const charts = screen.getAllByTestId('progress-chart');
    expect(charts.length).toBe(1);
    expect(screen.getByText('Updated Progress')).toBeDefined();
    expect(screen.queryByText('Old Progress')).toBeNull();
  });

  it('renders multiple events', () => {
    const events: RichContentEvent[] = [
      {
        componentType: 'table',
        data: {
          columns: [{ key: 'name', label: 'Name' }],
          rows: [{ name: 'Row1' }],
        } as TableData,
      },
      {
        componentType: 'chart',
        data: {
          items: [{ label: 'A', value: 1 }],
        } as ChartData,
      },
    ];
    render(<DynamicRenderer events={events} />);
    expect(screen.getByTestId('data-table')).toBeDefined();
    expect(screen.getByTestId('progress-chart')).toBeDefined();
  });

  it('returns null for empty events', () => {
    const { container } = render(<DynamicRenderer events={[]} />);
    expect(container.querySelector('[data-testid="dynamic-renderer"]')).toBeNull();
  });

  it('passes onAction callback to ActionGroup', () => {
    const onAction = vi.fn();
    const events: RichContentEvent[] = [
      {
        componentType: 'action_buttons',
        data: {
          actions: [{ id: 'approve', label: 'Approve' }],
        } as ActionButtonsData,
      },
    ];
    render(<DynamicRenderer events={events} onAction={onAction} />);
    fireEvent.click(screen.getByText('Approve'));
    expect(onAction).toHaveBeenCalledWith('approve');
  });
});
