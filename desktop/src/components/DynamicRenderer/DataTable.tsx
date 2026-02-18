/**
 * DataTable Component
 *
 * Renders a sortable data table from structured JSON data.
 * Used by DynamicRenderer for 'table' component type.
 *
 * Story 002: DynamicRenderer frontend component
 */

import { useState, useMemo, useCallback } from 'react';
import { clsx } from 'clsx';
import { CaretSortIcon, CaretUpIcon, CaretDownIcon } from '@radix-ui/react-icons';
import type { TableData, TableColumn } from '../../types/richContent';

// ============================================================================
// Types
// ============================================================================

type SortDirection = 'asc' | 'desc' | null;

interface SortState {
  column: string | null;
  direction: SortDirection;
}

interface DataTableProps {
  data: TableData;
}

// ============================================================================
// Component
// ============================================================================

export function DataTable({ data }: DataTableProps) {
  const [sort, setSort] = useState<SortState>({ column: null, direction: null });

  const handleSort = useCallback(
    (column: TableColumn) => {
      if (!column.sortable) return;

      setSort((prev) => {
        if (prev.column === column.key) {
          // Cycle: asc -> desc -> null
          if (prev.direction === 'asc') return { column: column.key, direction: 'desc' };
          if (prev.direction === 'desc') return { column: null, direction: null };
        }
        return { column: column.key, direction: 'asc' };
      });
    },
    []
  );

  const sortedRows = useMemo(() => {
    if (!sort.column || !sort.direction) return data.rows;

    const key = sort.column;
    const dir = sort.direction === 'asc' ? 1 : -1;

    return [...data.rows].sort((a, b) => {
      const valA = a[key];
      const valB = b[key];

      if (valA == null && valB == null) return 0;
      if (valA == null) return dir;
      if (valB == null) return -dir;

      if (typeof valA === 'number' && typeof valB === 'number') {
        return (valA - valB) * dir;
      }

      return String(valA).localeCompare(String(valB)) * dir;
    });
  }, [data.rows, sort]);

  const sortIcon = (column: TableColumn) => {
    if (!column.sortable) return null;
    if (sort.column !== column.key) return <CaretSortIcon className="w-3.5 h-3.5 text-gray-400" />;
    if (sort.direction === 'asc') return <CaretUpIcon className="w-3.5 h-3.5 text-blue-500" />;
    if (sort.direction === 'desc') return <CaretDownIcon className="w-3.5 h-3.5 text-blue-500" />;
    return <CaretSortIcon className="w-3.5 h-3.5 text-gray-400" />;
  };

  return (
    <div className="overflow-x-auto" data-testid="data-table">
      {data.title && (
        <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
          {data.title}
        </h4>
      )}
      <table className="w-full text-xs border-collapse">
        <thead>
          <tr className="border-b border-gray-200 dark:border-gray-700">
            {data.columns.map((col) => (
              <th
                key={col.key}
                className={clsx(
                  'px-3 py-2 text-left font-medium text-gray-500 dark:text-gray-400',
                  col.sortable && 'cursor-pointer hover:text-gray-700 dark:hover:text-gray-200'
                )}
                style={col.width ? { width: col.width } : undefined}
                onClick={() => handleSort(col)}
              >
                <div className="flex items-center gap-1">
                  {col.label}
                  {sortIcon(col)}
                </div>
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sortedRows.map((row, idx) => (
            <tr
              key={idx}
              className={clsx(
                'border-b border-gray-100 dark:border-gray-800',
                'hover:bg-gray-50 dark:hover:bg-gray-800/50',
                'transition-colors'
              )}
            >
              {data.columns.map((col) => (
                <td
                  key={col.key}
                  className="px-3 py-2 text-gray-700 dark:text-gray-300"
                >
                  {row[col.key] != null ? String(row[col.key]) : ''}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {sortedRows.length === 0 && (
        <p className="text-center text-xs text-gray-400 dark:text-gray-500 py-4">
          No data
        </p>
      )}
    </div>
  );
}

export default DataTable;
