/**
 * GeneralSection Component
 *
 * General settings including working mode and UI preferences.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ResetIcon, RocketIcon } from '@radix-ui/react-icons';
import { useSettingsStore } from '../../store/settings';
import { useOnboardingStore } from '../../store/onboarding';
import { LanguageSelector } from './LanguageSelector';

interface GeneralSectionProps {
  /** Callback to close the parent Settings dialog */
  onCloseDialog?: () => void;
}

export function GeneralSection({ onCloseDialog }: GeneralSectionProps = {}) {
  const { t } = useTranslation('settings');
  const { t: tWizard } = useTranslation('wizard');
  const {
    defaultMode,
    setDefaultMode,
    theme,
    setTheme
  } = useSettingsStore();
  const { triggerWizard, startTour } = useOnboardingStore();

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">
          {t('general.title')}
        </h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {t('general.description')}
        </p>
      </div>

      {/* Working Mode Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {t('general.workingMode.title')}
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
                {t('general.workingMode.simple.name')}
              </div>
              <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                {t('general.workingMode.simple.description')}
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
                {t('general.workingMode.expert.name')}
              </div>
              <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                {t('general.workingMode.expert.description')}
              </div>
            </div>
          </label>
        </div>
      </section>

      {/* Language Section */}
      <LanguageSelector />

      {/* Theme Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {t('general.theme.title')}
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
          <option value="system">{t('general.theme.system')}</option>
          <option value="light">{t('general.theme.light')}</option>
          <option value="dark">{t('general.theme.dark')}</option>
        </select>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {t('general.theme.description')}
        </p>
      </section>

      {/* Execution Limits */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {t('general.executionLimits.title')}
        </h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div>
            <label className="block text-sm text-gray-600 dark:text-gray-400 mb-1">
              {t('general.executionLimits.maxParallelStories')}
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
              {t('general.executionLimits.maxIterations')}
            </label>
            <input
              type="number"
              min={10}
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
              {t('general.executionLimits.maxTotalTokens')}
            </label>
            <input
              type="number"
              min={100000}
              max={5000000}
              step={100000}
              value={useSettingsStore.getState().maxTotalTokens}
              onChange={(e) => {
                const value = parseInt(e.target.value, 10);
                if (!isNaN(value)) {
                  useSettingsStore.setState({ maxTotalTokens: value });
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
              {t('general.executionLimits.timeout')}
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

      {/* Onboarding & Tour Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {tWizard('tour.title')}
        </h3>
        <div className="space-y-3">
          <div
            className={clsx(
              'flex items-center justify-between p-4 rounded-lg border',
              'border-gray-200 dark:border-gray-700',
              'bg-white dark:bg-gray-800'
            )}
          >
            <div>
              <div className="font-medium text-gray-900 dark:text-white text-sm">
                {tWizard('settings.rerunWizard')}
              </div>
              <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                {tWizard('settings.rerunWizardDescription')}
              </div>
            </div>
            <button
              onClick={() => {
                onCloseDialog?.();
                // Small delay to let dialog close before opening wizard
                setTimeout(() => triggerWizard(), 200);
              }}
              className={clsx(
                'inline-flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium',
                'bg-gray-100 dark:bg-gray-700',
                'text-gray-700 dark:text-gray-300',
                'hover:bg-gray-200 dark:hover:bg-gray-600',
                'transition-colors'
              )}
            >
              <ResetIcon className="w-4 h-4" />
              {tWizard('settings.rerunWizard')}
            </button>
          </div>

          <div
            className={clsx(
              'flex items-center justify-between p-4 rounded-lg border',
              'border-gray-200 dark:border-gray-700',
              'bg-white dark:bg-gray-800'
            )}
          >
            <div>
              <div className="font-medium text-gray-900 dark:text-white text-sm">
                {tWizard('settings.replayTour')}
              </div>
              <div className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                {tWizard('settings.replayTourDescription')}
              </div>
            </div>
            <button
              onClick={() => {
                onCloseDialog?.();
                // Small delay to let dialog close before starting tour
                setTimeout(() => startTour(), 200);
              }}
              className={clsx(
                'inline-flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium',
                'bg-primary-50 dark:bg-primary-900/30',
                'text-primary-700 dark:text-primary-300',
                'hover:bg-primary-100 dark:hover:bg-primary-900/50',
                'transition-colors'
              )}
            >
              <RocketIcon className="w-4 h-4" />
              {tWizard('settings.replayTour')}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}

export default GeneralSection;
