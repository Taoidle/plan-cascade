/**
 * QualityGatesSection Component
 *
 * Quality gate configuration with toggles for typecheck, test, lint,
 * custom scripts, and retry settings.
 */

import * as Switch from '@radix-ui/react-switch';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../../store/settings';

type GateId = 'typecheck' | 'test' | 'lint' | 'custom';

interface QualityGateInfo {
  id: GateId;
  i18nKey: string;
  icon: string;
}

const qualityGates: QualityGateInfo[] = [
  { id: 'typecheck', i18nKey: 'typecheck', icon: 'T' },
  { id: 'test', i18nKey: 'test', icon: 'TS' },
  { id: 'lint', i18nKey: 'lint', icon: 'L' },
  { id: 'custom', i18nKey: 'custom', icon: 'C' },
];

export function QualityGatesSection() {
  const { t } = useTranslation('settings');
  const { qualityGates: gateSettings, updateQualityGates } = useSettingsStore();

  const handleToggle = (gateId: GateId, checked: boolean) => {
    updateQualityGates({ [gateId]: checked });
  };

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">
          {t('quality.title')}
        </h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {t('quality.description')}
        </p>
      </div>

      {/* Quality Gate Toggles */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {t('quality.enabledGates')}
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
                      {t(`quality.${gate.i18nKey}.label`)}
                    </div>
                    <div className="text-sm text-gray-500 dark:text-gray-400 mt-0.5">
                      {t(`quality.${gate.i18nKey}.description`)}
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
            {t('quality.customScript.label')}
          </h3>
          <div>
            <input
              type="text"
              value={gateSettings.customScript}
              onChange={(e) => updateQualityGates({ customScript: e.target.value })}
              placeholder={t('quality.customScript.placeholder')}
              className={clsx(
                'w-full px-3 py-2 rounded-lg border',
                'border-gray-200 dark:border-gray-700',
                'bg-white dark:bg-gray-800',
                'text-gray-900 dark:text-white font-mono text-sm',
                'focus:outline-none focus:ring-2 focus:ring-primary-500'
              )}
            />
            <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">
              {t('quality.customScript.help')}
            </p>
          </div>
        </section>
      )}

      {/* Retry Settings */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {t('quality.retry.label')}
        </h3>
        <div className="max-w-xs">
          <label className="block text-sm text-gray-600 dark:text-gray-400 mb-1">
            {t('quality.retry.maxRetries')}
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
            {t('quality.retry.help')}
          </p>
        </div>
      </section>

      {/* Summary */}
      <section className="space-y-2">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {t('quality.summary.label')}
        </h3>
        <div
          className={clsx(
            'p-4 rounded-lg',
            'bg-gray-50 dark:bg-gray-800/50',
            'border border-gray-200 dark:border-gray-700'
          )}
        >
          <p className="text-sm text-gray-600 dark:text-gray-400">
            {getGateSummary(gateSettings, t)}
          </p>
        </div>
      </section>
    </div>
  );
}

function getGateSummary(
  gates: {
    typecheck: boolean;
    test: boolean;
    lint: boolean;
    custom: boolean;
    maxRetries: number;
  },
  t: (key: string, options?: Record<string, unknown>) => string
): string {
  const enabled = [];
  if (gates.typecheck) enabled.push(t('quality.summary.typecheck'));
  if (gates.test) enabled.push(t('quality.summary.test'));
  if (gates.lint) enabled.push(t('quality.summary.lint'));
  if (gates.custom) enabled.push(t('quality.summary.custom'));

  if (enabled.length === 0) {
    return t('quality.summary.noGates');
  }

  const gateList = enabled.length === 1
    ? enabled[0]
    : enabled.slice(0, -1).join(', ') + ' and ' + enabled[enabled.length - 1];

  const retryMessage = gates.maxRetries > 0
    ? t(gates.maxRetries === 1 ? 'quality.summary.retryMessage' : 'quality.summary.retryMessage_plural', { count: gates.maxRetries })
    : t('quality.summary.noRetries');

  return t('quality.summary.mustPass', { gates: gateList, retryMessage });
}

export default QualityGatesSection;
