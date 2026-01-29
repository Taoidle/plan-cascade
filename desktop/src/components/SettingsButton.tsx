/**
 * SettingsButton Component
 *
 * Button that opens the settings dialog.
 * Displays a gear icon with tooltip.
 */

import * as Tooltip from '@radix-ui/react-tooltip';
import { GearIcon } from '@radix-ui/react-icons';
import { clsx } from 'clsx';
import { useState } from 'react';
import { SettingsDialog } from './Settings';

interface SettingsButtonProps {
  className?: string;
}

export function SettingsButton({ className }: SettingsButtonProps) {
  const [open, setOpen] = useState(false);

  return (
    <>
      <Tooltip.Provider>
        <Tooltip.Root>
          <Tooltip.Trigger asChild>
            <button
              onClick={() => setOpen(true)}
              className={clsx(
                'p-2 rounded-lg transition-colors',
                'hover:bg-gray-100 dark:hover:bg-gray-800',
                'focus:outline-none focus:ring-2 focus:ring-primary-500',
                className
              )}
              aria-label="Settings"
            >
              <GearIcon className="w-5 h-5 text-gray-600 dark:text-gray-300" />
            </button>
          </Tooltip.Trigger>

          <Tooltip.Portal>
            <Tooltip.Content
              className={clsx(
                'px-3 py-1.5 rounded-md text-sm',
                'bg-gray-900 dark:bg-gray-100',
                'text-white dark:text-gray-900',
                'shadow-lg'
              )}
              sideOffset={5}
            >
              Settings
              <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
            </Tooltip.Content>
          </Tooltip.Portal>
        </Tooltip.Root>
      </Tooltip.Provider>

      <SettingsDialog open={open} onOpenChange={setOpen} />
    </>
  );
}

export default SettingsButton;
