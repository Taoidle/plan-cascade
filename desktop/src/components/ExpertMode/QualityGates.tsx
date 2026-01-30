/**
 * Quality Gates Component
 *
 * Checkbox group for quality gates (TypeCheck, Test, Lint, Custom)
 * with toggle all and custom gate support.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { usePRDStore } from '../../store/prd';
import * as Checkbox from '@radix-ui/react-checkbox';
import {
  CheckIcon,
  PlusIcon,
  Cross2Icon,
  GearIcon,
  CodeIcon,
  CheckCircledIcon,
  LightningBoltIcon,
} from '@radix-ui/react-icons';

const gateIcons: Record<string, React.ReactNode> = {
  typecheck: <CodeIcon className="w-4 h-4" />,
  test: <CheckCircledIcon className="w-4 h-4" />,
  lint: <LightningBoltIcon className="w-4 h-4" />,
};

export function QualityGates() {
  const { prd, setQualityGate, addCustomQualityGate, removeQualityGate } = usePRDStore();
  const [isAddingCustom, setIsAddingCustom] = useState(false);
  const [customName, setCustomName] = useState('');
  const [customCommand, setCustomCommand] = useState('');

  const allEnabled = prd.qualityGates.every((g) => g.enabled);

  const handleToggleAll = () => {
    const newState = !allEnabled;
    prd.qualityGates.forEach((gate) => {
      setQualityGate(gate.id, newState);
    });
  };

  const handleAddCustomGate = (e: React.FormEvent) => {
    e.preventDefault();
    if (customName.trim() && customCommand.trim()) {
      addCustomQualityGate(customName.trim(), customCommand.trim());
      setCustomName('');
      setCustomCommand('');
      setIsAddingCustom(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <label className="block text-sm font-medium text-gray-700 dark:text-gray-300">
          Quality Gates
        </label>
        <button
          onClick={handleToggleAll}
          className={clsx(
            'text-xs px-2 py-1 rounded',
            'text-gray-600 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-700',
            'transition-colors'
          )}
        >
          {allEnabled ? 'Disable All' : 'Enable All'}
        </button>
      </div>

      <div className="space-y-2">
        {prd.qualityGates.map((gate) => {
          const isCustom = gate.id.startsWith('custom-');
          const icon = gateIcons[gate.id] || <GearIcon className="w-4 h-4" />;

          return (
            <div
              key={gate.id}
              className={clsx(
                'flex items-center justify-between p-3 rounded-lg',
                'bg-white dark:bg-gray-800',
                'border border-gray-200 dark:border-gray-700'
              )}
            >
              <label className="flex items-center gap-3 cursor-pointer flex-1">
                <Checkbox.Root
                  checked={gate.enabled}
                  onCheckedChange={(checked) => setQualityGate(gate.id, !!checked)}
                  className={clsx(
                    'w-5 h-5 rounded flex items-center justify-center',
                    'border-2 transition-colors',
                    gate.enabled
                      ? 'bg-primary-600 border-primary-600'
                      : 'bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600'
                  )}
                >
                  <Checkbox.Indicator>
                    <CheckIcon className="w-3.5 h-3.5 text-white" />
                  </Checkbox.Indicator>
                </Checkbox.Root>

                <span
                  className={clsx(
                    'text-gray-500 dark:text-gray-400',
                    gate.enabled && 'text-primary-600 dark:text-primary-400'
                  )}
                >
                  {icon}
                </span>

                <div className="flex-1">
                  <span
                    className={clsx(
                      'font-medium',
                      gate.enabled
                        ? 'text-gray-900 dark:text-white'
                        : 'text-gray-500 dark:text-gray-400'
                    )}
                  >
                    {gate.name}
                  </span>
                  {gate.command && (
                    <span className="ml-2 text-xs text-gray-400 dark:text-gray-500 font-mono">
                      {gate.command}
                    </span>
                  )}
                </div>
              </label>

              {isCustom && (
                <button
                  onClick={() => removeQualityGate(gate.id)}
                  className={clsx(
                    'p-1.5 rounded',
                    'text-gray-400 hover:text-red-500',
                    'hover:bg-red-50 dark:hover:bg-red-900/20',
                    'transition-colors'
                  )}
                  title="Remove gate"
                >
                  <Cross2Icon className="w-4 h-4" />
                </button>
              )}
            </div>
          );
        })}

        {/* Add custom gate form */}
        {isAddingCustom ? (
          <form
            onSubmit={handleAddCustomGate}
            className={clsx(
              'p-3 rounded-lg',
              'bg-primary-50 dark:bg-primary-900/20',
              'border-2 border-dashed border-primary-300 dark:border-primary-700'
            )}
          >
            <div className="space-y-3">
              <div>
                <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
                  Gate Name
                </label>
                <input
                  type="text"
                  value={customName}
                  onChange={(e) => setCustomName(e.target.value)}
                  placeholder="e.g., Build Check"
                  className={clsx(
                    'w-full px-3 py-2 rounded-lg text-sm',
                    'bg-white dark:bg-gray-800',
                    'border border-gray-300 dark:border-gray-600',
                    'focus:outline-none focus:ring-2 focus:ring-primary-500'
                  )}
                  autoFocus
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-600 dark:text-gray-400 mb-1">
                  Command
                </label>
                <input
                  type="text"
                  value={customCommand}
                  onChange={(e) => setCustomCommand(e.target.value)}
                  placeholder="e.g., npm run build"
                  className={clsx(
                    'w-full px-3 py-2 rounded-lg text-sm font-mono',
                    'bg-white dark:bg-gray-800',
                    'border border-gray-300 dark:border-gray-600',
                    'focus:outline-none focus:ring-2 focus:ring-primary-500'
                  )}
                />
              </div>
              <div className="flex justify-end gap-2">
                <button
                  type="button"
                  onClick={() => {
                    setIsAddingCustom(false);
                    setCustomName('');
                    setCustomCommand('');
                  }}
                  className={clsx(
                    'px-3 py-1.5 text-sm rounded-lg',
                    'bg-gray-100 dark:bg-gray-700',
                    'text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-200 dark:hover:bg-gray-600'
                  )}
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={!customName.trim() || !customCommand.trim()}
                  className={clsx(
                    'px-3 py-1.5 text-sm rounded-lg',
                    'bg-primary-600 text-white',
                    'hover:bg-primary-700',
                    'disabled:opacity-50 disabled:cursor-not-allowed'
                  )}
                >
                  Add Gate
                </button>
              </div>
            </div>
          </form>
        ) : (
          <button
            onClick={() => setIsAddingCustom(true)}
            className={clsx(
              'w-full flex items-center justify-center gap-2 p-3 rounded-lg',
              'border-2 border-dashed border-gray-300 dark:border-gray-600',
              'text-gray-500 dark:text-gray-400 text-sm',
              'hover:border-primary-500 hover:text-primary-600 dark:hover:text-primary-400',
              'transition-colors'
            )}
          >
            <PlusIcon className="w-4 h-4" />
            Add Custom Gate
          </button>
        )}
      </div>
    </div>
  );
}

export default QualityGates;
