/**
 * ConfigCard
 *
 * Three-layer config UI:
 * - Layer 1 (default): Read-only summary + "Continue" + "Customize" link
 * - Layer 2: Expanded form with editable fields
 * - Layer 3: Natural language override text input
 */

import { useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { ConfigCardData } from '../../../types/workflowCard';
import { useWorkflowOrchestratorStore } from '../../../store/workflowOrchestrator';

export function ConfigCard({ data, interactive }: { data: ConfigCardData; interactive: boolean }) {
  const { t } = useTranslation('simpleMode');
  const [layer, setLayer] = useState<1 | 2 | 3>(1);
  const [localConfig, setLocalConfig] = useState(data);
  const [nlOverride, setNlOverride] = useState('');
  const updateConfig = useWorkflowOrchestratorStore((s) => s.updateConfig);
  const confirmConfig = useWorkflowOrchestratorStore((s) => s.confirmConfig);
  const overrideConfigNatural = useWorkflowOrchestratorStore((s) => s.overrideConfigNatural);
  const phase = useWorkflowOrchestratorStore((s) => s.phase);

  const isActive = interactive && phase === 'configuring';

  const handleConfirm = useCallback(() => {
    if (layer === 2) {
      updateConfig(localConfig);
    }
    confirmConfig();
  }, [layer, localConfig, updateConfig, confirmConfig]);

  const handleNlSubmit = useCallback(() => {
    if (nlOverride.trim()) {
      overrideConfigNatural(nlOverride.trim());
      setNlOverride('');
      setLayer(1);
    }
  }, [nlOverride, overrideConfigNatural]);

  return (
    <div className="rounded-lg border border-sky-200 dark:border-sky-800 bg-sky-50 dark:bg-sky-900/20 overflow-hidden">
      {/* Header */}
      <div className="px-3 py-2 bg-sky-100/50 dark:bg-sky-900/30 border-b border-sky-200 dark:border-sky-800 flex items-center justify-between">
        <span className="text-xs font-semibold text-sky-700 dark:text-sky-300 uppercase tracking-wide">
          {t('workflow.config.title')}
        </span>
        {data.isOverridden && (
          <span className="text-2xs px-1.5 py-0.5 rounded bg-sky-200 dark:bg-sky-800 text-sky-600 dark:text-sky-400">
            {t('workflow.config.customized')}
          </span>
        )}
      </div>

      <div className="px-3 py-2 space-y-2">
        {/* Layer 1: Summary */}
        {layer === 1 && (
          <>
            <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
              <ConfigRow label={t('workflow.config.flow')} value={data.flowLevel} />
              <ConfigRow label={t('workflow.config.tdd')} value={data.tddMode} />
              <ConfigRow label={t('workflow.config.maxParallel')} value={String(data.maxParallel)} />
              <ConfigRow label={t('workflow.config.qualityGates')} value={data.qualityGatesEnabled ? t('workflow.config.on') : t('workflow.config.off')} />
              <ConfigRow label={t('workflow.config.interview')} value={data.specInterviewEnabled ? t('workflow.config.enabled') : t('workflow.config.skip')} />
            </div>

            {isActive && (
              <div className="flex items-center gap-2 pt-1">
                <button
                  onClick={handleConfirm}
                  className="px-3 py-1 text-xs font-medium rounded-md bg-sky-600 text-white hover:bg-sky-700 transition-colors"
                >
                  {t('workflow.config.continue')}
                </button>
                <button
                  onClick={() => setLayer(2)}
                  className="text-xs text-sky-600 dark:text-sky-400 hover:underline"
                >
                  {t('workflow.config.customize')}
                </button>
              </div>
            )}
          </>
        )}

        {/* Layer 2: Edit form */}
        {layer === 2 && (
          <>
            <div className="space-y-2">
              <SelectField
                label={t('workflow.config.flowLevel')}
                value={localConfig.flowLevel}
                options={[
                  { value: 'quick', label: t('workflow.config.quick') },
                  { value: 'standard', label: t('workflow.config.standard') },
                  { value: 'full', label: t('workflow.config.full') },
                ]}
                onChange={(v) => setLocalConfig({ ...localConfig, flowLevel: v as ConfigCardData['flowLevel'] })}
              />
              <SelectField
                label={t('workflow.config.tddMode')}
                value={localConfig.tddMode}
                options={[
                  { value: 'off', label: t('workflow.config.off') },
                  { value: 'flexible', label: t('workflow.config.flexible') },
                  { value: 'strict', label: t('workflow.config.strict') },
                ]}
                onChange={(v) => setLocalConfig({ ...localConfig, tddMode: v as ConfigCardData['tddMode'] })}
              />
              <div className="flex items-center gap-2">
                <label className="text-xs text-sky-700 dark:text-sky-300 w-24 shrink-0">{t('workflow.config.maxParallel')}</label>
                <input
                  type="range"
                  min={1}
                  max={16}
                  value={localConfig.maxParallel}
                  onChange={(e) => setLocalConfig({ ...localConfig, maxParallel: parseInt(e.target.value, 10) })}
                  className="flex-1 h-1.5 accent-sky-600"
                />
                <span className="text-xs text-sky-600 dark:text-sky-400 w-6 text-right">{localConfig.maxParallel}</span>
              </div>
              <ToggleField
                label={t('workflow.config.qualityGates')}
                value={localConfig.qualityGatesEnabled}
                onChange={(v) => setLocalConfig({ ...localConfig, qualityGatesEnabled: v })}
              />
              <ToggleField
                label={t('workflow.config.specInterview')}
                value={localConfig.specInterviewEnabled}
                onChange={(v) => setLocalConfig({ ...localConfig, specInterviewEnabled: v })}
              />
            </div>

            <div className="flex items-center gap-2 pt-1">
              <button
                onClick={handleConfirm}
                className="px-3 py-1 text-xs font-medium rounded-md bg-sky-600 text-white hover:bg-sky-700 transition-colors"
              >
                {t('workflow.config.applyAndContinue')}
              </button>
              <button
                onClick={() => setLayer(1)}
                className="text-xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
              >
                {t('workflow.config.back')}
              </button>
              <button
                onClick={() => setLayer(3)}
                className="text-xs text-sky-600 dark:text-sky-400 hover:underline ml-auto"
              >
                {t('workflow.config.useNaturalLanguage')}
              </button>
            </div>
          </>
        )}

        {/* Layer 3: NL override */}
        {layer === 3 && (
          <>
            <p className="text-xs text-sky-600 dark:text-sky-400">
              {t('workflow.config.nlDescription')}
            </p>
            <div className="flex gap-2">
              <input
                type="text"
                value={nlOverride}
                onChange={(e) => setNlOverride(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleNlSubmit()}
                placeholder={t('workflow.config.nlPlaceholder')}
                className="flex-1 px-2 py-1 text-xs rounded border border-sky-300 dark:border-sky-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-1 focus:ring-sky-500"
              />
              <button
                onClick={handleNlSubmit}
                className="px-3 py-1 text-xs font-medium rounded-md bg-sky-600 text-white hover:bg-sky-700 transition-colors"
              >
                {t('workflow.config.apply')}
              </button>
            </div>
            <button
              onClick={() => setLayer(2)}
              className="text-xs text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
            >
              {t('workflow.config.backToForm')}
            </button>
          </>
        )}
      </div>
    </div>
  );
}

function ConfigRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-sky-600/80 dark:text-sky-400/80">{label}</span>
      <span className="font-medium text-sky-700 dark:text-sky-300">{value}</span>
    </div>
  );
}

function SelectField({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: { value: string; label: string }[];
  onChange: (v: string) => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <label className="text-xs text-sky-700 dark:text-sky-300 w-24 shrink-0">{label}</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="flex-1 px-2 py-1 text-xs rounded border border-sky-300 dark:border-sky-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-1 focus:ring-sky-500"
      >
        {options.map((opt) => (
          <option key={opt.value} value={opt.value}>{opt.label}</option>
        ))}
      </select>
    </div>
  );
}

function ToggleField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: boolean;
  onChange: (v: boolean) => void;
}) {
  const { t } = useTranslation('simpleMode');
  return (
    <div className="flex items-center gap-2">
      <label className="text-xs text-sky-700 dark:text-sky-300 w-24 shrink-0">{label}</label>
      <button
        onClick={() => onChange(!value)}
        className={clsx(
          'relative w-8 h-4 rounded-full transition-colors',
          value ? 'bg-sky-600' : 'bg-gray-300 dark:bg-gray-600'
        )}
      >
        <span
          className={clsx(
            'absolute top-0.5 left-0.5 w-3 h-3 rounded-full bg-white transition-transform',
            value && 'translate-x-4'
          )}
        />
      </button>
      <span className="text-xs text-sky-600/80 dark:text-sky-400/80">{value ? t('workflow.config.on') : t('workflow.config.off')}</span>
    </div>
  );
}
