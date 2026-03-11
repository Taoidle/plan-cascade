/**
 * GeneralSection Component
 *
 * General settings including working mode and UI preferences.
 */

import { clsx } from 'clsx';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ResetIcon, RocketIcon } from '@radix-ui/react-icons';
import { useSettingsStore } from '../../store/settings';
import { useUpdateStore } from '../../store/update';
import { useOnboardingStore } from '../../store/onboarding';
import { useToast } from '../shared/Toast';
import { LanguageSelector } from './LanguageSelector';
import { getContextPolicy, setContextPolicy, type ContextPolicy } from '../../lib/contextApi';

interface GeneralSectionProps {
  /** Callback to close the parent Settings dialog */
  onCloseDialog?: () => void;
}

export function GeneralSection({ onCloseDialog }: GeneralSectionProps = {}) {
  const { t } = useTranslation('settings');
  const { t: tCommon } = useTranslation();
  const { t: tWizard } = useTranslation('wizard');
  const { t: tUpdates } = useTranslation('updates');
  const { showToast } = useToast();
  const {
    defaultMode,
    setDefaultMode,
    theme,
    setTheme,
    autoPanelHoverEnabled,
    setAutoPanelHoverEnabled,
    closeToBackgroundEnabled,
    setCloseToBackgroundEnabled,
    knowledgeAutoEnsureDocsCollection,
    setKnowledgeAutoEnsureDocsCollection,
    kbQueryRunsV2,
    setKbQueryRunsV2,
    kbPickerServerSearch,
    setKbPickerServerSearch,
    kbIngestJobScopedProgress,
    setKbIngestJobScopedProgress,
    developerModeEnabled,
    setDeveloperModeEnabled,
    developerPanels,
    setDeveloperPanels,
    developerSettingsInitialized,
    setDeveloperSettingsInitialized,
    updatePreferences,
    setUpdateChannel,
    setAutoCheckForUpdates,
  } = useSettingsStore();
  const { triggerWizard, startTour } = useOnboardingStore();
  const currentVersion = useUpdateStore((s) => s.currentVersion);
  const checkingForUpdates = useUpdateStore((s) => s.checking);
  const hydrateCurrentVersion = useUpdateStore((s) => s.hydrateCurrentVersion);
  const checkForUpdates = useUpdateStore((s) => s.checkForUpdates);
  const [contextPolicy, setContextPolicyState] = useState<ContextPolicy | null>(null);

  const backgroundBehaviorDescriptionKey = (() => {
    const platform = navigator.userAgent.toLowerCase();
    if (platform.includes('mac')) return 'general.backgroundRun.descriptionMac';
    if (platform.includes('win')) return 'general.backgroundRun.descriptionWindows';
    return 'general.backgroundRun.descriptionLinux';
  })();

  useEffect(() => {
    void hydrateCurrentVersion();
  }, [hydrateCurrentVersion]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const response = await getContextPolicy();
      if (cancelled) return;
      if (response.success && response.data) {
        setContextPolicyState(response.data);
        if (!developerSettingsInitialized) {
          setDeveloperPanels({ contextInspector: response.data.context_inspector_ui });
          setDeveloperSettingsInitialized(true);
        }
      } else {
        setContextPolicyState(
          (prev) =>
            prev ?? {
              context_v2_pipeline: true,
              memory_v2_ranker: true,
              context_inspector_ui: false,
              pinned_sources: [],
              excluded_sources: [],
              soft_threshold_ratio: 0.85,
              hard_threshold_ratio: 0.95,
            },
        );
        if (!developerSettingsInitialized) {
          setDeveloperSettingsInitialized(true);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [developerSettingsInitialized, setDeveloperPanels, setDeveloperSettingsInitialized]);

  const handleContextInspectorToggle = async (enabled: boolean) => {
    const basePolicy: ContextPolicy = contextPolicy ?? {
      context_v2_pipeline: true,
      memory_v2_ranker: true,
      context_inspector_ui: false,
      pinned_sources: [],
      excluded_sources: [],
      soft_threshold_ratio: 0.85,
      hard_threshold_ratio: 0.95,
    };
    const nextPolicy: ContextPolicy = { ...basePolicy, context_inspector_ui: enabled };
    setDeveloperPanels({ contextInspector: enabled });
    setContextPolicyState(nextPolicy);
    const response = await setContextPolicy(nextPolicy);
    if (!response.success) {
      setDeveloperPanels({ contextInspector: basePolicy.context_inspector_ui });
      setContextPolicyState(basePolicy);
    }
  };

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">{t('general.title')}</h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('general.description')}</p>
      </div>

      {/* Working Mode Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('general.workingMode.title')}</h3>
        <div className="space-y-3">
          <label
            className={clsx(
              'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
              'transition-colors',
              defaultMode === 'simple'
                ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800',
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
              <div className="font-medium text-gray-900 dark:text-white">{t('general.workingMode.simple.name')}</div>
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
                : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800',
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
              <div className="font-medium text-gray-900 dark:text-white">{t('general.workingMode.expert.name')}</div>
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
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('general.theme.title')}</h3>
        <select
          value={theme}
          onChange={(e) => setTheme(e.target.value as 'system' | 'light' | 'dark')}
          className={clsx(
            'w-full max-w-xs px-3 py-2 rounded-lg border',
            'border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800',
            'text-gray-900 dark:text-white',
            'focus:outline-none focus:ring-2 focus:ring-primary-500',
          )}
        >
          <option value="system">{t('general.theme.system')}</option>
          <option value="light">{t('general.theme.light')}</option>
          <option value="dark">{t('general.theme.dark')}</option>
        </select>
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('general.theme.description')}</p>
      </section>

      {/* Panel Hover Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('general.panelHover.title')}</h3>
        <label
          className={clsx(
            'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
            'transition-colors',
            'border-gray-200 dark:border-gray-700',
            'hover:bg-gray-50 dark:hover:bg-gray-800',
          )}
        >
          <input
            type="checkbox"
            checked={autoPanelHoverEnabled}
            onChange={(e) => setAutoPanelHoverEnabled(e.target.checked)}
            className="mt-1 text-primary-600"
          />
          <div>
            <div className="font-medium text-gray-900 dark:text-white text-sm">{t('general.panelHover.enable')}</div>
            <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">{t('general.panelHover.description')}</div>
          </div>
        </label>
      </section>

      {/* Background Run Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('general.backgroundRun.title')}</h3>
        <label
          className={clsx(
            'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
            'transition-colors',
            'border-gray-200 dark:border-gray-700',
            'hover:bg-gray-50 dark:hover:bg-gray-800',
          )}
        >
          <input
            type="checkbox"
            checked={closeToBackgroundEnabled}
            onChange={(e) => setCloseToBackgroundEnabled(e.target.checked)}
            className="mt-1 text-primary-600"
          />
          <div>
            <div className="font-medium text-gray-900 dark:text-white text-sm">{t('general.backgroundRun.enable')}</div>
            <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">{t(backgroundBehaviorDescriptionKey)}</div>
          </div>
        </label>
      </section>

      {/* Updates Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{tUpdates('settings.title')}</h3>
        <div
          className={clsx(
            'rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-800',
            'space-y-4',
          )}
        >
          <div>
            <div className="text-sm font-medium text-gray-900 dark:text-white">
              {tUpdates('settings.currentVersion')}
            </div>
            <div className="mt-1 text-sm text-gray-500 dark:text-gray-400">{currentVersion ?? tCommon('version')}</div>
          </div>

          <div>
            <label className="mb-1 block text-sm font-medium text-gray-900 dark:text-white">
              {tUpdates('settings.updateChannel')}
            </label>
            <select
              value={updatePreferences.updateChannel}
              onChange={(e) => setUpdateChannel(e.target.value as 'stable' | 'beta' | 'alpha')}
              className={clsx(
                'w-full max-w-xs rounded-lg border px-3 py-2',
                'border-gray-200 bg-white text-gray-900 dark:border-gray-700 dark:bg-gray-900 dark:text-white',
                'focus:outline-none focus:ring-2 focus:ring-primary-500',
              )}
            >
              <option value="stable">{tUpdates('channel.stable')}</option>
              <option value="beta">{tUpdates('channel.beta')}</option>
              <option value="alpha">{tUpdates('channel.alpha')}</option>
            </select>
          </div>

          <label
            className={clsx(
              'flex items-start gap-4 rounded-lg border p-4 transition-colors',
              'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-900/40',
            )}
          >
            <input
              type="checkbox"
              checked={updatePreferences.autoCheckForUpdates}
              onChange={(e) => setAutoCheckForUpdates(e.target.checked)}
              className="mt-1 text-primary-600"
            />
            <div>
              <div className="font-medium text-gray-900 dark:text-white text-sm">{tUpdates('settings.autoCheck')}</div>
              <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                {tUpdates('settings.autoCheckDescription')}
              </div>
            </div>
          </label>

          <div className="flex items-center justify-end">
            <button
              type="button"
              onClick={() =>
                void checkForUpdates(true).then((result) => {
                  if (result && !result.available) {
                    showToast(tUpdates('toasts.upToDate'), 'info');
                  }
                })
              }
              disabled={checkingForUpdates}
              className={clsx(
                'rounded-lg bg-primary-600 px-4 py-2 text-sm font-medium text-white transition-colors',
                'hover:bg-primary-700 disabled:cursor-not-allowed disabled:opacity-60',
              )}
            >
              {checkingForUpdates ? tUpdates('settings.checkNowBusy') : tUpdates('settings.checkNow')}
            </button>
          </div>
        </div>
      </section>

      {/* Knowledge Base Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('general.knowledgeBase.title')}</h3>
        <label
          className={clsx(
            'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
            'transition-colors',
            'border-gray-200 dark:border-gray-700',
            'hover:bg-gray-50 dark:hover:bg-gray-800',
          )}
        >
          <input
            type="checkbox"
            checked={knowledgeAutoEnsureDocsCollection}
            onChange={(e) => setKnowledgeAutoEnsureDocsCollection(e.target.checked)}
            className="mt-1 text-primary-600"
          />
          <div>
            <div className="font-medium text-gray-900 dark:text-white text-sm">
              {t('general.knowledgeBase.autoEnsureDocs')}
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {t('general.knowledgeBase.autoEnsureDocsDescription')}
            </div>
          </div>
        </label>

        <label
          className={clsx(
            'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
            'transition-colors',
            'border-gray-200 dark:border-gray-700',
            'hover:bg-gray-50 dark:hover:bg-gray-800',
          )}
        >
          <input
            type="checkbox"
            checked={kbQueryRunsV2}
            onChange={(e) => setKbQueryRunsV2(e.target.checked)}
            className="mt-1 text-primary-600"
          />
          <div>
            <div className="font-medium text-gray-900 dark:text-white text-sm">
              {t('general.knowledgeBase.kbQueryRunsV2')}
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {t('general.knowledgeBase.kbQueryRunsV2Description')}
            </div>
          </div>
        </label>

        <label
          className={clsx(
            'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
            'transition-colors',
            'border-gray-200 dark:border-gray-700',
            'hover:bg-gray-50 dark:hover:bg-gray-800',
          )}
        >
          <input
            type="checkbox"
            checked={kbPickerServerSearch}
            onChange={(e) => setKbPickerServerSearch(e.target.checked)}
            className="mt-1 text-primary-600"
          />
          <div>
            <div className="font-medium text-gray-900 dark:text-white text-sm">
              {t('general.knowledgeBase.kbPickerServerSearch')}
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {t('general.knowledgeBase.kbPickerServerSearchDescription')}
            </div>
          </div>
        </label>

        <label
          className={clsx(
            'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
            'transition-colors',
            'border-gray-200 dark:border-gray-700',
            'hover:bg-gray-50 dark:hover:bg-gray-800',
          )}
        >
          <input
            type="checkbox"
            checked={kbIngestJobScopedProgress}
            onChange={(e) => setKbIngestJobScopedProgress(e.target.checked)}
            className="mt-1 text-primary-600"
          />
          <div>
            <div className="font-medium text-gray-900 dark:text-white text-sm">
              {t('general.knowledgeBase.kbIngestJobScopedProgress')}
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {t('general.knowledgeBase.kbIngestJobScopedProgressDescription')}
            </div>
          </div>
        </label>
      </section>

      {/* Developer Mode */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('general.developerMode.title')}</h3>
        <label
          className={clsx(
            'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
            'transition-colors',
            'border-gray-200 dark:border-gray-700',
            'hover:bg-gray-50 dark:hover:bg-gray-800',
          )}
        >
          <input
            type="checkbox"
            checked={developerModeEnabled}
            onChange={(e) => setDeveloperModeEnabled(e.target.checked)}
            className="mt-1 text-primary-600"
          />
          <div>
            <div className="font-medium text-gray-900 dark:text-white text-sm">{t('general.developerMode.enable')}</div>
            <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {t('general.developerMode.description')}
            </div>
          </div>
        </label>

        <div
          className={clsx(
            'space-y-3 rounded-lg border border-dashed border-gray-200 dark:border-gray-700 p-4 transition-opacity',
            !developerModeEnabled && 'opacity-60',
          )}
        >
          <label
            className={clsx(
              'flex items-start gap-4 rounded-lg border p-4 transition-colors',
              developerModeEnabled
                ? 'cursor-pointer border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800'
                : 'cursor-not-allowed border-gray-200/70 dark:border-gray-700/70 bg-gray-50/40 dark:bg-gray-800/30',
            )}
          >
            <input
              type="checkbox"
              checked={developerPanels.contextInspector}
              disabled={!developerModeEnabled}
              onChange={(e) => void handleContextInspectorToggle(e.target.checked)}
              className="mt-1 text-primary-600"
            />
            <div>
              <div className="font-medium text-gray-900 dark:text-white text-sm">
                {t('general.developerMode.panels.contextInspector.title')}
              </div>
              <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                {t('general.developerMode.panels.contextInspector.description')}
              </div>
            </div>
          </label>

          {(
            [
              ['workflowReliability', 'general.developerMode.panels.workflowReliability'],
              ['executionLogs', 'general.developerMode.panels.executionLogs'],
              ['streamingOutput', 'general.developerMode.panels.streamingOutput'],
            ] as const
          ).map(([panelKey, keyBase]) => (
            <label
              key={panelKey}
              className={clsx(
                'flex items-start gap-4 rounded-lg border p-4 transition-colors',
                developerModeEnabled
                  ? 'cursor-pointer border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800'
                  : 'cursor-not-allowed border-gray-200/70 dark:border-gray-700/70 bg-gray-50/40 dark:bg-gray-800/30',
              )}
            >
              <input
                type="checkbox"
                checked={developerPanels[panelKey]}
                disabled={!developerModeEnabled}
                onChange={(e) =>
                  setDeveloperPanels({
                    [panelKey]: e.target.checked,
                  } as Record<typeof panelKey, boolean>)
                }
                className="mt-1 text-primary-600"
              />
              <div>
                <div className="font-medium text-gray-900 dark:text-white text-sm">{t(`${keyBase}.title`)}</div>
                <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">{t(`${keyBase}.description`)}</div>
              </div>
            </label>
          ))}
        </div>
      </section>

      {/* Execution Limits */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('general.executionLimits.title')}</h3>
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
                'focus:outline-none focus:ring-2 focus:ring-primary-500',
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
                'focus:outline-none focus:ring-2 focus:ring-primary-500',
              )}
            />
          </div>
          <div>
            <label className="block text-sm text-gray-600 dark:text-gray-400 mb-1">
              {t('general.executionLimits.maxConcurrentSubagents')}
            </label>
            <input
              type="number"
              min={0}
              max={20}
              value={useSettingsStore.getState().maxConcurrentSubagents}
              onChange={(e) => {
                const value = parseInt(e.target.value, 10);
                if (!isNaN(value)) {
                  useSettingsStore.setState({ maxConcurrentSubagents: value });
                }
              }}
              className={clsx(
                'w-full px-3 py-2 rounded-lg border',
                'border-gray-200 dark:border-gray-700',
                'bg-white dark:bg-gray-800',
                'text-gray-900 dark:text-white',
                'focus:outline-none focus:ring-2 focus:ring-primary-500',
              )}
            />
            <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
              {t('general.executionLimits.maxConcurrentSubagentsHelp')}
            </p>
          </div>
        </div>
      </section>

      {/* Onboarding & Tour Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{tWizard('tour.title')}</h3>
        <div className="space-y-3">
          <div
            className={clsx(
              'flex items-center justify-between p-4 rounded-lg border',
              'border-gray-200 dark:border-gray-700',
              'bg-white dark:bg-gray-800',
            )}
          >
            <div>
              <div className="font-medium text-gray-900 dark:text-white text-sm">{tWizard('settings.rerunWizard')}</div>
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
                'transition-colors',
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
              'bg-white dark:bg-gray-800',
            )}
          >
            <div>
              <div className="font-medium text-gray-900 dark:text-white text-sm">{tWizard('settings.replayTour')}</div>
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
                'transition-colors',
              )}
            >
              <RocketIcon className="w-4 h-4" />
              {tWizard('settings.replayTour')}
            </button>
          </div>
        </div>
      </section>

      {/* Version Info */}
      <section className="pt-4 border-t border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between text-sm text-gray-500 dark:text-gray-400">
          <span>{tCommon('appName')}</span>
          <span>{tCommon('version')}</span>
        </div>
      </section>
    </div>
  );
}

export default GeneralSection;
