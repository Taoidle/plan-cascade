/**
 * QualityGatesSection Component
 *
 * Quality gate configuration with toggles for typecheck, test, lint,
 * custom scripts, and retry settings.
 */

import * as Switch from '@radix-ui/react-switch';
import { clsx } from 'clsx';
import { useSettingsStore } from '../../store/settings';

interface QualityGateInfo {
  id: 'typecheck' | 'test' | 'lint' | 'custom';
  name: string;
  description: string;
  icon: string;
}

const qualityGates: QualityGateInfo[] = [
  {
    id: 'typecheck',
    name: 'Type Check',
    description: 'Run type checking (mypy, pyright, tsc) before considering a story complete.',
    icon: 'T',
  },
  {
    id: 'test',
    name: 'Tests',
    description: 'Run tests (pytest, jest, npm test) to verify functionality.',
    icon: 'TS',
  },
  {
    id: 'lint',
    name: 'Lint',
    description: 'Run linting (ruff, eslint) to ensure code quality.',
    icon: 'L',
  },
  {
    id: 'custom',
    name: 'Custom Script',
    description: 'Run a custom quality check script.',
    icon: 'C',
  },
];

export function QualityGatesSection() {
  const { qualityGates: gateSettings, updateQualityGates } = useSettingsStore();

  const handleToggle = (gateId: 'typecheck' | 'test' | 'lint' | 'custom', checked: boolean) => {
    updateQualityGates({ [gateId]: checked });
  };

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">
          Quality Gates
        </h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Configure quality checks that must pass before a story is marked complete.
        </p>
      </div>

      {/* Quality Gate Toggles */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          Enabled Gates
        </h3>
        <div className="space-y-3">
          {qualityGates.map((gate) => (
            <div
              key={gate.id}
              className={clsx(
                'flex items-start gap-4 p-4 rounded-lg border',
                'border-gray-200 dark:border-gray-700',
                'bg-white dark:bg-gray-800'
              )}
            >
              <div
                className={clsx(
                  'w-10 h-10 rounded-lg flex items-center justify-center shrink-0',
                  'text-sm font-bold',
                  gateSettings[gate.id]
                    ? 'bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-400'
                    : 'bg-gray-100 text-gray-400 dark:bg-gray-700 dark:text-gray-500'
                )}
              >
                {gate.icon}
              </div>

              <div className="flex-1 min-w-0">
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <div className="font-medium text-gray-900 dark:text-white">
                      {gate.name}
                    </div>
                    <div className="text-sm text-gray-500 dark:text-gray-400 mt-0.5">
                      {gate.description}
                    </div>
                  </div>

                  <Switch.Root
                    checked={gateSettings[gate.id]}
                    onCheckedChange={(checked) => handleToggle(gate.id, checked)}
                    className={clsx(
                      'w-10 h-6 rounded-full relative shrink-0',
                      'bg-gray-200 dark:bg-gray-700',
                      'data-[state=checked]:bg-primary-600',
                      'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
                      'dark:focus:ring-offset-gray-800'
                    )}
                  >
                    <Switch.Thumb
                      className={clsx(
                        'block w-5 h-5 bg-white rounded-full shadow',
                        'transition-transform',
                        'data-[state=checked]:translate-x-[18px]',
                        'data-[state=unchecked]:translate-x-[2px]'
                      )}
                    />
                  </Switch.Root>
                </div>
              </div>
            </div>
          ))}
        </div>
      </section>

      {/* Custom Script Input (shown when custom is enabled) */}
      {gateSettings.custom && (
        <section className="space-y-4">
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">
            Custom Script
          </h3>
          <div>
            <input
              type="text"
              value={gateSettings.customScript}
              onChange={(e) => updateQualityGates({ customScript: e.target.value })}
              placeholder="e.g., ./scripts/quality-check.sh"
              className={clsx(
                'w-full px-3 py-2 rounded-lg border',
                'border-gray-200 dark:border-gray-700',
                'bg-white dark:bg-gray-800',
                'text-gray-900 dark:text-white font-mono text-sm',
                'focus:outline-none focus:ring-2 focus:ring-primary-500'
              )}
            />
            <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">
              Enter a script or command to run. Exit code 0 means success.
            </p>
          </div>
        </section>
      )}

      {/* Retry Settings */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          Retry Settings
        </h3>
        <div className="max-w-xs">
          <label className="block text-sm text-gray-600 dark:text-gray-400 mb-1">
            Maximum Retries
          </label>
          <input
            type="number"
            min={0}
            max={10}
            value={gateSettings.maxRetries}
            onChange={(e) => {
              const value = parseInt(e.target.value, 10);
              if (!isNaN(value) && value >= 0 && value <= 10) {
                updateQualityGates({ maxRetries: value });
              }
            }}
            className={clsx(
              'w-full px-3 py-2 rounded-lg border',
              'border-gray-200 dark:border-gray-700',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'focus:outline-none focus:ring-2 focus:ring-primary-500'
            )}
          />
          <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">
            Number of times to retry a story if quality gates fail (0-10).
          </p>
        </div>
      </section>

      {/* Summary */}
      <section className="space-y-2">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          Summary
        </h3>
        <div
          className={clsx(
            'p-4 rounded-lg',
            'bg-gray-50 dark:bg-gray-800/50',
            'border border-gray-200 dark:border-gray-700'
          )}
        >
          <p className="text-sm text-gray-600 dark:text-gray-400">
            {getGateSummary(gateSettings)}
          </p>
        </div>
      </section>
    </div>
  );
}

function getGateSummary(gates: {
  typecheck: boolean;
  test: boolean;
  lint: boolean;
  custom: boolean;
  maxRetries: number;
}): string {
  const enabled = [];
  if (gates.typecheck) enabled.push('type checking');
  if (gates.test) enabled.push('tests');
  if (gates.lint) enabled.push('linting');
  if (gates.custom) enabled.push('custom script');

  if (enabled.length === 0) {
    return 'No quality gates enabled. Stories will be marked complete without validation.';
  }

  const gateList = enabled.length === 1
    ? enabled[0]
    : enabled.slice(0, -1).join(', ') + ' and ' + enabled[enabled.length - 1];

  return `Stories must pass ${gateList} to be considered complete. ${
    gates.maxRetries > 0
      ? `If gates fail, the agent will retry up to ${gates.maxRetries} time${gates.maxRetries === 1 ? '' : 's'}.`
      : 'Retries are disabled.'
  }`;
}

export default QualityGatesSection;
