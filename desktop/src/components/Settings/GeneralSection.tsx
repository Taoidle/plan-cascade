/**
 * GeneralSection Component
 *
 * General settings including working mode and UI preferences.
 */

import * as Switch from '@radix-ui/react-switch';
import { clsx } from 'clsx';
import { useSettingsStore } from '../../store/settings';

export function GeneralSection() {
  const {
    defaultMode,
    setDefaultMode,
    theme,
    setTheme
  } = useSettingsStore();

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">
          General Settings
        </h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Configure general application preferences.
        </p>
      </div>

      {/* Working Mode Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          Working Mode
        </h3>
        <div className="space-y-3">
          <label
            className={clsx(
              'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
              'transition-colors',
              defaultMode === 'simple'
                ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800'
            )}
          >
            <input
              type="radio"
              name="workingMode"
              value="simple"
              checked={defaultMode === 'simple'}
              onChange={() => setDefaultMode('simple')}
              className="mt-1 text-primary-600"
            />
            <div>
              <div className="font-medium text-gray-900 dark:text-white">
                Claude Code GUI Mode
              </div>
              <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                Operates as a graphical interface for Claude Code. Uses Claude Code
                as the execution backend with simplified task input.
              </div>
            </div>
          </label>

          <label
            className={clsx(
              'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
              'transition-colors',
              defaultMode === 'expert'
                ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800'
            )}
          >
            <input
              type="radio"
              name="workingMode"
              value="expert"
              checked={defaultMode === 'expert'}
              onChange={() => setDefaultMode('expert')}
              className="mt-1 text-primary-600"
            />
            <div>
              <div className="font-medium text-gray-900 dark:text-white">
                Standalone Orchestration Mode
              </div>
              <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                Full orchestration with PRD generation, multi-agent coordination,
                and advanced execution strategies. Requires LLM API configuration.
              </div>
            </div>
          </label>
        </div>
      </section>

      {/* Theme Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          Theme
        </h3>
        <select
          value={theme}
          onChange={(e) => setTheme(e.target.value as 'system' | 'light' | 'dark')}
          className={clsx(
            'w-full max-w-xs px-3 py-2 rounded-lg border',
            'border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800',
            'text-gray-900 dark:text-white',
            'focus:outline-none focus:ring-2 focus:ring-primary-500'
          )}
        >
          <option value="system">System</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Choose your preferred color scheme. System follows your OS preference.
        </p>
      </section>

      {/* Execution Limits */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          Execution Limits
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div>
            <label className="block text-sm text-gray-600 dark:text-gray-400 mb-1">
              Max Parallel Stories
            </label>
            <input
              type="number"
              min={1}
              max={10}
              value={useSettingsStore.getState().maxParallelStories}
              onChange={(e) => {
                const value = parseInt(e.target.value, 10);
                if (!isNaN(value)) {
                  useSettingsStore.setState({ maxParallelStories: value });
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
          </div>
          <div>
            <label className="block text-sm text-gray-600 dark:text-gray-400 mb-1">
              Max Iterations
            </label>
            <input
              type="number"
              min={1}
              max={200}
              value={useSettingsStore.getState().maxIterations}
              onChange={(e) => {
                const value = parseInt(e.target.value, 10);
                if (!isNaN(value)) {
                  useSettingsStore.setState({ maxIterations: value });
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
          </div>
          <div>
            <label className="block text-sm text-gray-600 dark:text-gray-400 mb-1">
              Timeout (seconds)
            </label>
            <input
              type="number"
              min={60}
              max={3600}
              value={useSettingsStore.getState().timeoutSeconds}
              onChange={(e) => {
                const value = parseInt(e.target.value, 10);
                if (!isNaN(value)) {
                  useSettingsStore.setState({ timeoutSeconds: value });
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
          </div>
        </div>
      </section>
    </div>
  );
}

export default GeneralSection;
