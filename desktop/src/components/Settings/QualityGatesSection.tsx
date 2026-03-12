import { useState } from 'react';
import * as Switch from '@radix-ui/react-switch';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../../store/settings';
import { getQualityGateDefinitionsForMode, type QualityBehavior } from '../../types/workflowQuality';
import type { WorkflowMode } from '../../types/workflowKernel';

const MODES: WorkflowMode[] = ['chat', 'plan', 'task', 'debug'];

const BEHAVIOR_OPTIONS: QualityBehavior[] = ['manual_review', 'auto_retry_if_retryable', 'warn_and_continue'];

export function QualityGatesSection() {
  const { t } = useTranslation('settings');
  const { quality, updateQualitySettings } = useSettingsStore();
  const [editingGateId, setEditingGateId] = useState<string | null>(null);
  const [customName, setCustomName] = useState('');
  const [customCommand, setCustomCommand] = useState('');
  const [customModes, setCustomModes] = useState<WorkflowMode[]>(['task']);
  const [customBlocking, setCustomBlocking] = useState(true);
  const [customError, setCustomError] = useState<string | null>(null);

  const resetCustomForm = () => {
    setEditingGateId(null);
    setCustomName('');
    setCustomCommand('');
    setCustomModes(['task']);
    setCustomBlocking(true);
    setCustomError(null);
  };

  const startEditingGate = (gateId: string) => {
    const gate = quality.customGates.find((candidate) => candidate.id === gateId);
    if (!gate) {
      return;
    }
    setEditingGateId(gate.id);
    setCustomName(gate.name);
    setCustomCommand(gate.command);
    setCustomModes(gate.modes);
    setCustomBlocking(gate.blocking);
    setCustomError(null);
  };

  const addCustomGate = () => {
    if (!customName.trim() || !customCommand.trim() || customModes.length === 0) {
      setCustomError(t('quality.custom.validation.required'));
      return;
    }
    const normalizedName = customName.trim();
    const normalizedCommand = customCommand.trim();
    const duplicateName = quality.customGates.find(
      (gate) => gate.id !== editingGateId && gate.name.toLowerCase() === normalizedName.toLowerCase(),
    );
    if (duplicateName) {
      setCustomError(t('quality.custom.validation.duplicateName'));
      return;
    }
    const gateId = editingGateId ?? `custom-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
    updateQualitySettings({
      customGates: [
        ...quality.customGates.filter((gate) => gate.id !== gateId),
        {
          id: gateId,
          name: normalizedName,
          command: normalizedCommand,
          modes: customModes,
          blocking: customBlocking,
        },
      ],
    });
    resetCustomForm();
  };

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">{t('quality.title')}</h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('quality.description')}</p>
      </div>

      <section className="rounded-xl border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('quality.enabled')}</h3>
            <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('quality.enabledHelp')}</p>
          </div>
          <Switch.Root
            checked={quality.enabled}
            onCheckedChange={(checked) => updateQualitySettings({ enabled: checked })}
            className={clsx(
              'w-10 h-6 rounded-full relative shrink-0',
              'bg-gray-200 dark:bg-gray-700',
              'data-[state=checked]:bg-primary-600',
            )}
          >
            <Switch.Thumb
              className={clsx(
                'block w-5 h-5 bg-white rounded-full shadow',
                'transition-transform',
                'data-[state=checked]:translate-x-[18px]',
                'data-[state=unchecked]:translate-x-[2px]',
              )}
            />
          </Switch.Root>
        </div>
      </section>

      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('quality.modePolicies')}</h3>
        <div className="space-y-4">
          {MODES.map((mode) => {
            const retryPolicy = quality.retryPolicyByMode[mode];
            const profileOverride = quality.profileOverridesByMode[mode];
            const definitions = getQualityGateDefinitionsForMode(mode);
            const selectedGateIds = profileOverride.defaultGateIds ?? [];
            return (
              <div
                key={mode}
                className="rounded-xl border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-800"
              >
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <h4 className="text-sm font-semibold text-gray-900 dark:text-white">
                      {t(`quality.modes.${mode}`)}
                    </h4>
                    <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
                      {selectedGateIds.length > 0
                        ? t('quality.profile.selectedCount', { count: selectedGateIds.length })
                        : t('quality.profile.noOverrides')}
                    </p>
                  </div>
                  <select
                    value={quality.defaultBehaviorByMode[mode]}
                    onChange={(event) =>
                      updateQualitySettings({
                        defaultBehaviorByMode: {
                          [mode]: event.target.value as QualityBehavior,
                        } as Record<WorkflowMode, QualityBehavior>,
                      })
                    }
                    className={clsx(
                      'rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-900',
                      'dark:border-gray-700 dark:bg-gray-900 dark:text-white',
                    )}
                  >
                    {BEHAVIOR_OPTIONS.map((behavior) => (
                      <option key={behavior} value={behavior}>
                        {t(`quality.behaviors.${behavior}`)}
                      </option>
                    ))}
                  </select>
                </div>

                <div className="mt-4 grid gap-3 sm:grid-cols-2">
                  <label className="block">
                    <span className="mb-1 block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                      {t('quality.retry.maxAttempts')}
                    </span>
                    <input
                      type="number"
                      min={0}
                      max={10}
                      value={retryPolicy.maxAttempts}
                      onChange={(event) => {
                        const value = Number.parseInt(event.target.value, 10);
                        if (Number.isNaN(value)) return;
                        updateQualitySettings({
                          retryPolicyByMode: {
                            [mode]: {
                              ...retryPolicy,
                              maxAttempts: Math.max(0, Math.min(10, value)),
                            },
                          } as typeof quality.retryPolicyByMode,
                        });
                      }}
                      className={clsx(
                        'w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-900',
                        'dark:border-gray-700 dark:bg-gray-900 dark:text-white',
                      )}
                    />
                  </label>
                </div>

                <div className="mt-4">
                  <span className="mb-2 block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                    {t('quality.profile.defaultGates')}
                  </span>
                  <div className="grid gap-3 sm:grid-cols-2">
                    {definitions.map((definition) => {
                      const checked = selectedGateIds.includes(definition.id);
                      return (
                        <label
                          key={definition.id}
                          className={clsx(
                            'flex cursor-pointer items-start gap-3 rounded-lg border px-3 py-3',
                            checked
                              ? 'border-primary-500 bg-primary-50/60 dark:border-primary-500 dark:bg-primary-950/20'
                              : 'border-gray-200 bg-gray-50 dark:border-gray-700 dark:bg-gray-900/40',
                          )}
                        >
                          <input
                            type="checkbox"
                            checked={checked}
                            onChange={(event) => {
                              const defaultGateIds = event.target.checked
                                ? [...selectedGateIds, definition.id]
                                : selectedGateIds.filter((gateId) => gateId !== definition.id);
                              updateQualitySettings({
                                profileOverridesByMode: {
                                  [mode]: {
                                    ...profileOverride,
                                    defaultGateIds,
                                  },
                                } as typeof quality.profileOverridesByMode,
                              });
                            }}
                            className="mt-1 h-4 w-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                          />
                          <span className="min-w-0">
                            <span className="block text-sm font-medium text-gray-900 dark:text-white">
                              {t(definition.labelKey)}
                            </span>
                            <span className="mt-1 block text-xs leading-5 text-gray-500 dark:text-gray-400">
                              {t(definition.descriptionKey)}
                            </span>
                          </span>
                        </label>
                      );
                    })}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </section>

      <section className="rounded-xl border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-800">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('quality.plugins.title')}</h3>
            <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('quality.plugins.description')}</p>
          </div>
          <Switch.Root
            checked={quality.pluginPolicy.allowPluginGates}
            onCheckedChange={(checked) =>
              updateQualitySettings({
                pluginPolicy: {
                  ...quality.pluginPolicy,
                  allowPluginGates: checked,
                },
              })
            }
            className={clsx(
              'w-10 h-6 rounded-full relative shrink-0',
              'bg-gray-200 dark:bg-gray-700',
              'data-[state=checked]:bg-primary-600',
            )}
          >
            <Switch.Thumb
              className={clsx(
                'block w-5 h-5 bg-white rounded-full shadow',
                'transition-transform',
                'data-[state=checked]:translate-x-[18px]',
                'data-[state=unchecked]:translate-x-[2px]',
              )}
            />
          </Switch.Root>
        </div>
      </section>

      <section className="rounded-xl border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-800">
        <div className="mb-4">
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('quality.custom.title')}</h3>
          <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('quality.custom.description')}</p>
          <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">{t('quality.custom.envHelp')}</p>
        </div>

        <div className="space-y-3">
          {quality.customGates.length === 0 ? (
            <div className="rounded-lg border border-dashed border-gray-300 px-4 py-3 text-sm text-gray-500 dark:border-gray-700 dark:text-gray-400">
              {t('quality.custom.empty')}
            </div>
          ) : null}
          {quality.customGates.map((gate) => (
            <div
              key={gate.id}
              className="rounded-lg border border-gray-200 bg-gray-50 p-3 dark:border-gray-700 dark:bg-gray-900/40"
            >
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0">
                  <div className="text-sm font-medium text-gray-900 dark:text-white">{gate.name}</div>
                  <div className="mt-1 break-all font-mono text-xs text-gray-500 dark:text-gray-400">
                    {gate.command}
                  </div>
                  <div className="mt-2 flex flex-wrap gap-2">
                    {gate.modes.map((mode) => (
                      <span
                        key={`${gate.id}-${mode}`}
                        className="rounded-full bg-gray-200 px-2 py-1 text-[11px] font-medium text-gray-700 dark:bg-gray-700 dark:text-gray-200"
                      >
                        {t(`quality.modes.${mode}`)}
                      </span>
                    ))}
                    <span
                      className={clsx(
                        'rounded-full px-2 py-1 text-[11px] font-medium',
                        gate.blocking
                          ? 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300'
                          : 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300',
                      )}
                    >
                      {gate.blocking ? t('quality.custom.blocking') : t('quality.custom.nonBlocking')}
                    </span>
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  <button
                    type="button"
                    onClick={() => startEditingGate(gate.id)}
                    className="rounded-lg border border-gray-200 px-3 py-1.5 text-xs font-medium text-gray-600 hover:bg-gray-100 dark:border-gray-700 dark:text-gray-300 dark:hover:bg-gray-700"
                  >
                    {t('quality.custom.edit')}
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      if (editingGateId === gate.id) {
                        resetCustomForm();
                      }
                      updateQualitySettings({
                        customGates: quality.customGates.filter((candidate) => candidate.id !== gate.id),
                      });
                    }}
                    className="rounded-lg border border-gray-200 px-3 py-1.5 text-xs font-medium text-gray-600 hover:bg-gray-100 dark:border-gray-700 dark:text-gray-300 dark:hover:bg-gray-700"
                  >
                    {t('quality.custom.remove')}
                  </button>
                </div>
              </div>
            </div>
          ))}

          <div className="rounded-lg border border-dashed border-gray-300 p-4 dark:border-gray-700">
            <div className="mb-3 flex items-center justify-between gap-3">
              <h4 className="text-sm font-medium text-gray-900 dark:text-white">
                {editingGateId ? t('quality.custom.editTitle') : t('quality.custom.addTitle')}
              </h4>
              {editingGateId ? (
                <button
                  type="button"
                  onClick={resetCustomForm}
                  className="rounded-lg border border-gray-200 px-3 py-1.5 text-xs font-medium text-gray-600 hover:bg-gray-100 dark:border-gray-700 dark:text-gray-300 dark:hover:bg-gray-700"
                >
                  {t('quality.custom.cancel')}
                </button>
              ) : null}
            </div>
            <div className="grid gap-3">
              <label className="block">
                <span className="mb-1 block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                  {t('quality.custom.name')}
                </span>
                <input
                  type="text"
                  value={customName}
                  onChange={(event) => setCustomName(event.target.value)}
                  className={clsx(
                    'w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-900',
                    'dark:border-gray-700 dark:bg-gray-900 dark:text-white',
                  )}
                  placeholder={t('quality.custom.namePlaceholder')}
                />
              </label>

              <label className="block">
                <span className="mb-1 block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                  {t('quality.custom.command')}
                </span>
                <input
                  type="text"
                  value={customCommand}
                  onChange={(event) => setCustomCommand(event.target.value)}
                  className={clsx(
                    'w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-900',
                    'dark:border-gray-700 dark:bg-gray-900 dark:text-white',
                  )}
                  placeholder={t('quality.custom.commandPlaceholder')}
                />
                <span className="mt-1 block text-xs text-gray-500 dark:text-gray-400">
                  {t('quality.custom.commandHelp')}
                </span>
              </label>

              <div>
                <span className="mb-2 block text-xs font-medium uppercase tracking-wide text-gray-500 dark:text-gray-400">
                  {t('quality.custom.modes')}
                </span>
                <div className="flex flex-wrap gap-2">
                  {MODES.map((mode) => {
                    const checked = customModes.includes(mode);
                    return (
                      <label
                        key={`custom-mode-${mode}`}
                        className={clsx(
                          'flex cursor-pointer items-center gap-2 rounded-lg border px-3 py-2 text-sm',
                          checked
                            ? 'border-primary-500 bg-primary-50 dark:border-primary-500 dark:bg-primary-950/20'
                            : 'border-gray-200 bg-gray-50 dark:border-gray-700 dark:bg-gray-900/40',
                        )}
                      >
                        <input
                          type="checkbox"
                          checked={checked}
                          onChange={(event) =>
                            setCustomModes((current) =>
                              event.target.checked ? [...current, mode] : current.filter((value) => value !== mode),
                            )
                          }
                          className="h-4 w-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                        />
                        <span className="text-gray-900 dark:text-white">{t(`quality.modes.${mode}`)}</span>
                      </label>
                    );
                  })}
                </div>
              </div>

              <label className="flex items-center gap-3 rounded-lg border border-gray-200 bg-gray-50 px-3 py-3 dark:border-gray-700 dark:bg-gray-900/40">
                <input
                  type="checkbox"
                  checked={customBlocking}
                  onChange={(event) => setCustomBlocking(event.target.checked)}
                  className="h-4 w-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                />
                <span className="text-sm text-gray-900 dark:text-white">{t('quality.custom.blockingLabel')}</span>
              </label>

              <div className="flex justify-end">
                {customError ? (
                  <p className="mr-auto self-center text-sm text-red-600 dark:text-red-400">{customError}</p>
                ) : null}
                <button
                  type="button"
                  onClick={addCustomGate}
                  disabled={!customName.trim() || !customCommand.trim() || customModes.length === 0}
                  className={clsx(
                    'rounded-lg px-3 py-2 text-sm font-medium text-white',
                    'bg-primary-600 hover:bg-primary-700 disabled:cursor-not-allowed disabled:opacity-50',
                  )}
                >
                  {editingGateId ? t('quality.custom.save') : t('quality.custom.add')}
                </button>
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="rounded-xl border border-gray-200 bg-gray-50 p-4 dark:border-gray-700 dark:bg-gray-800/50">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('quality.summary.label')}</h3>
        <p className="mt-2 text-sm text-gray-600 dark:text-gray-400">
          {quality.enabled
            ? t('quality.summary.configured', { count: quality.customGates.length })
            : t('quality.summary.disabled')}
        </p>
      </section>
    </div>
  );
}

export default QualityGatesSection;
