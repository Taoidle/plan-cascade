/**
 * ModeSwitch Component
 *
 * Toggle between Simple and Expert modes.
 * Simple mode: One-click execution with AI-driven automation
 * Expert mode: Full control over PRD editing, agent selection, and execution
 */

import * as Switch from '@radix-ui/react-switch';
import { clsx } from 'clsx';

export type Mode = 'simple' | 'expert';

interface ModeSwitchProps {
  mode: Mode;
  onChange: (mode: Mode) => void;
  disabled?: boolean;
}

export function ModeSwitch({ mode, onChange, disabled = false }: ModeSwitchProps) {
  const isExpert = mode === 'expert';

  return (
    <div className="flex items-center gap-3">
      <span
        className={clsx(
          'text-sm font-medium transition-colors',
          !isExpert ? 'text-primary-600 dark:text-primary-400' : 'text-gray-500 dark:text-gray-400'
        )}
      >
        Simple
      </span>

      <Switch.Root
        checked={isExpert}
        onCheckedChange={(checked) => onChange(checked ? 'expert' : 'simple')}
        disabled={disabled}
        className={clsx(
          'relative h-6 w-11 rounded-full transition-colors',
          'bg-gray-200 dark:bg-gray-700',
          'data-[state=checked]:bg-primary-600',
          'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
          'disabled:cursor-not-allowed disabled:opacity-50'
        )}
      >
        <Switch.Thumb
          className={clsx(
            'block h-5 w-5 rounded-full bg-white shadow-md transition-transform',
            'data-[state=checked]:translate-x-[22px] data-[state=unchecked]:translate-x-0.5'
          )}
        />
      </Switch.Root>

      <span
        className={clsx(
          'text-sm font-medium transition-colors',
          isExpert ? 'text-primary-600 dark:text-primary-400' : 'text-gray-500 dark:text-gray-400'
        )}
      >
        Expert
      </span>
    </div>
  );
}

export default ModeSwitch;
