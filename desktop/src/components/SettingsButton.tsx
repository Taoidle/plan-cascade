/**
 * SettingsButton Component
 *
 * Button that opens the settings dialog.
 * Displays a gear icon with tooltip.
 */

import * as Tooltip from '@radix-ui/react-tooltip';
import * as Dialog from '@radix-ui/react-dialog';
import { GearIcon, Cross2Icon } from '@radix-ui/react-icons';
import { clsx } from 'clsx';
import { useState } from 'react';

interface SettingsButtonProps {
  className?: string;
}

export function SettingsButton({ className }: SettingsButtonProps) {
  const [open, setOpen] = useState(false);

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Tooltip.Provider>
        <Tooltip.Root>
          <Tooltip.Trigger asChild>
            <Dialog.Trigger asChild>
              <button
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
            </Dialog.Trigger>
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

      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-lg max-h-[85vh] overflow-auto',
            'bg-white dark:bg-gray-900 rounded-xl shadow-xl',
            'p-6',
            'focus:outline-none'
          )}
        >
          <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
            Settings
          </Dialog.Title>

          <Dialog.Description className="mt-2 text-sm text-gray-500 dark:text-gray-400">
            Configure Plan Cascade preferences
          </Dialog.Description>

          <div className="mt-6 space-y-6">
            {/* Backend Selection */}
            <section>
              <h3 className="text-sm font-medium text-gray-900 dark:text-white mb-3">
                Backend
              </h3>
              <div className="space-y-2">
                <label className="flex items-center gap-3 p-3 rounded-lg border border-gray-200 dark:border-gray-700 cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800">
                  <input
                    type="radio"
                    name="backend"
                    value="claude-code"
                    defaultChecked
                    className="text-primary-600"
                  />
                  <div>
                    <div className="font-medium text-gray-900 dark:text-white">
                      Claude Code (Recommended)
                    </div>
                    <div className="text-sm text-gray-500 dark:text-gray-400">
                      No API key required
                    </div>
                  </div>
                </label>
                <label className="flex items-center gap-3 p-3 rounded-lg border border-gray-200 dark:border-gray-700 cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800">
                  <input
                    type="radio"
                    name="backend"
                    value="claude-api"
                    className="text-primary-600"
                  />
                  <div>
                    <div className="font-medium text-gray-900 dark:text-white">
                      Claude API
                    </div>
                    <div className="text-sm text-gray-500 dark:text-gray-400">
                      Direct API access
                    </div>
                  </div>
                </label>
              </div>
            </section>

            {/* Theme Selection */}
            <section>
              <h3 className="text-sm font-medium text-gray-900 dark:text-white mb-3">
                Theme
              </h3>
              <select
                className={clsx(
                  'w-full px-3 py-2 rounded-lg border',
                  'border-gray-200 dark:border-gray-700',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500'
                )}
                defaultValue="system"
              >
                <option value="system">System</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
            </section>
          </div>

          <div className="mt-6 flex justify-end gap-3">
            <Dialog.Close asChild>
              <button
                className={clsx(
                  'px-4 py-2 rounded-lg',
                  'bg-gray-100 dark:bg-gray-800',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500'
                )}
              >
                Cancel
              </button>
            </Dialog.Close>
            <button
              className={clsx(
                'px-4 py-2 rounded-lg',
                'bg-primary-600 text-white',
                'hover:bg-primary-700',
                'focus:outline-none focus:ring-2 focus:ring-primary-500'
              )}
              onClick={() => setOpen(false)}
            >
              Save
            </button>
          </div>

          <Dialog.Close asChild>
            <button
              className={clsx(
                'absolute top-4 right-4 p-1 rounded-lg',
                'hover:bg-gray-100 dark:hover:bg-gray-800',
                'focus:outline-none focus:ring-2 focus:ring-primary-500'
              )}
              aria-label="Close"
            >
              <Cross2Icon className="w-4 h-4 text-gray-500" />
            </button>
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default SettingsButton;
