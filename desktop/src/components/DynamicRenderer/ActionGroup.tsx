/**
 * ActionGroup Component
 *
 * Renders approve/retry/skip action buttons with IPC callback support.
 * Used by DynamicRenderer for 'action_buttons' component type.
 *
 * Story 002: DynamicRenderer frontend component
 */

import { useCallback } from 'react';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import type { ActionButtonsData, ActionButton, ActionButtonVariant } from '../../types/richContent';

// ============================================================================
// Helpers
// ============================================================================

const variantStyles: Record<ActionButtonVariant, string> = {
  primary: 'bg-blue-600 hover:bg-blue-700 text-white',
  secondary: 'bg-gray-200 dark:bg-gray-700 hover:bg-gray-300 dark:hover:bg-gray-600 text-gray-800 dark:text-gray-200',
  ghost: 'bg-transparent hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-600 dark:text-gray-400',
  danger: 'bg-red-600 hover:bg-red-700 text-white',
};

// ============================================================================
// Component
// ============================================================================

interface ActionGroupProps {
  data: ActionButtonsData;
  /** Optional callback when an action is triggered */
  onAction?: (actionId: string) => void;
}

export function ActionGroup({ data, onAction }: ActionGroupProps) {
  const handleClick = useCallback(
    async (action: ActionButton) => {
      if (action.disabled) return;

      // Notify via callback if provided
      if (onAction) {
        onAction(action.id);
      }

      // Also try to invoke Tauri IPC command for backend handling
      try {
        await invoke('handle_rich_content_action', { actionId: action.id });
      } catch {
        // IPC command may not exist yet -- non-fatal
      }
    },
    [onAction],
  );

  return (
    <div className="space-y-2" data-testid="action-group">
      {data.label && <span className="text-xs font-medium text-gray-500 dark:text-gray-400">{data.label}</span>}
      <div className="flex flex-wrap items-center gap-2">
        {data.actions.map((action) => {
          const variant = action.variant ?? 'secondary';
          return (
            <button
              key={action.id}
              onClick={() => handleClick(action)}
              disabled={action.disabled}
              className={clsx(
                'px-3 py-1.5 rounded-md text-sm font-medium',
                'transition-colors',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                variantStyles[variant],
              )}
              data-action-id={action.id}
            >
              {action.label}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export default ActionGroup;
