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
import { useWorkflowKernelStore } from '../../../store/workflowKernel';
import { submitWorkflowActionIntentViaCoordinator } from '../../../store/simpleWorkflowCoordinator';
import { reportInteractiveActionFailure } from '../../../lib/workflowObservability';

export function ConfigCard({ data, interactive }: { data: ConfigCardData; interactive: boolean }) {
  const { t } = useTranslation('simpleMode');
  const [layer, setLayer] = useState<1 | 2 | 3>(1);
  const [localConfig, setLocalConfig] = useState(data);
  const [nlOverride, setNlOverride] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const updateConfig = useWorkflowOrchestratorStore((s) => s.updateConfig);
  const confirmConfig = useWorkflowOrchestratorStore((s) => s.confirmConfig);
  const overrideConfigNatural = useWorkflowOrchestratorStore((s) => s.overrideConfigNatural);
  const workflowSession = useWorkflowKernelStore((s) => s.session);
  const displayFlowLevel = localizeFlowLevel(t, localConfig.flowLevel);
  const displayTddMode = localizeTddMode(t, localConfig.tddMode);

  const isKernelTaskActive = workflowSession?.status === 'active' && workflowSession.activeMode === 'task';
  const kernelTaskPhase = workflowSession?.modeSnapshots.task?.phase ?? 'idle';
  const isActive = interactive && kernelTaskPhase === 'configuring' && isKernelTaskActive;

  const handleConfirm = useCallback(async () => {
    if (!isActive || isSubmitting) return;
    setIsSubmitting(true);
    setSubmitError(null);
    const workflowConfig = {
      flowLevel: localConfig.flowLevel,
      tddMode: localConfig.tddMode,
      maxParallel: localConfig.maxParallel,
      qualityGatesEnabled: localConfig.qualityGatesEnabled,
      specInterviewEnabled: localConfig.specInterviewEnabled,
      skipVerification: localConfig.skipVerification,
      skipReview: localConfig.skipReview,
      globalAgentOverride: localConfig.globalAgentOverride,
      implAgentOverride: localConfig.implAgentOverride,
    };
    if (layer === 2) {
      updateConfig(workflowConfig);
    }
    const summary = [
      `flow=${localConfig.flowLevel}`,
      `tdd=${localConfig.tddMode}`,
      `maxParallel=${localConfig.maxParallel}`,
      `qualityGates=${localConfig.qualityGatesEnabled}`,
      `specInterview=${localConfig.specInterviewEnabled}`,
    ].join(';');
    try {
      await submitWorkflowActionIntentViaCoordinator({
        mode: 'task',
        type: 'task_configuration',
        source: 'config_card_confirm',
        action: layer === 2 ? 'confirm_custom_config' : 'confirm_default_config',
        content: summary,
        metadata: {
          layer,
          flowLevel: localConfig.flowLevel,
          tddMode: localConfig.tddMode,
          maxParallel: localConfig.maxParallel,
          qualityGatesEnabled: localConfig.qualityGatesEnabled,
          specInterviewEnabled: localConfig.specInterviewEnabled,
        },
      });
    } catch {
      // Keep orchestration available even if kernel logging fails.
    }
    try {
      const result = await confirmConfig(workflowConfig);
      if (!result.ok) {
        const message = result.message || 'Failed to confirm workflow configuration';
        setSubmitError(message);
        await reportInteractiveActionFailure({
          card: 'config_card',
          action: layer === 2 ? 'confirm_custom_config' : 'confirm_default_config',
          errorCode: result.errorCode || 'config_confirm_failed',
          message,
          session: workflowSession,
        });
        return;
      }
      setLayer(1);
    } finally {
      setIsSubmitting(false);
    }
  }, [confirmConfig, isActive, isSubmitting, layer, localConfig, updateConfig, workflowSession]);

  const handleNlSubmit = useCallback(async () => {
    if (!isActive || isSubmitting) return;
    if (nlOverride.trim()) {
      setIsSubmitting(true);
      setSubmitError(null);
      try {
        await submitWorkflowActionIntentViaCoordinator({
          mode: 'task',
          type: 'task_configuration',
          source: 'config_card_nl_override',
          action: 'apply_natural_language_override',
          content: nlOverride.trim(),
          metadata: {
            isNaturalLanguageOverride: true,
          },
        });
      } catch {
        // Keep orchestration available even if kernel logging fails.
      } finally {
        setIsSubmitting(false);
      }
      overrideConfigNatural(nlOverride.trim());
      setNlOverride('');
      setLayer(1);
    }
  }, [isActive, isSubmitting, nlOverride, overrideConfigNatural]);

  return (
    <div className="rounded-lg border border-sky-200 dark:border-sky-800 bg-sky-50 dark:bg-sky-900/20 overflow-hidden">
      {/* Header */}
      <div className="px-3 py-2 bg-sky-100/50 dark:bg-sky-900/30 border-b border-sky-200 dark:border-sky-800 flex items-center justify-between">
        <span className="text-xs font-semibold text-sky-700 dark:text-sky-300 uppercase tracking-wide">
          {t('workflow.config.title')}
        </span>
        <div className="flex items-center gap-1.5">
          {data.recommendationSource === 'llm_enhanced' && (
            <span className="text-2xs px-1.5 py-0.5 rounded bg-violet-200 dark:bg-violet-800 text-violet-700 dark:text-violet-300">
              {t('workflow.config.aiRecommended')}
            </span>
          )}
          {(data.isOverridden ||
            (kernelTaskPhase !== 'configuring' &&
              (localConfig.flowLevel !== data.flowLevel ||
                localConfig.tddMode !== data.tddMode ||
                localConfig.maxParallel !== data.maxParallel ||
                localConfig.qualityGatesEnabled !== data.qualityGatesEnabled ||
                localConfig.specInterviewEnabled !== data.specInterviewEnabled))) && (
            <span className="text-2xs px-1.5 py-0.5 rounded bg-sky-200 dark:bg-sky-800 text-sky-600 dark:text-sky-400">
              {t('workflow.config.customized')}
            </span>
          )}
          {!isActive && kernelTaskPhase !== 'configuring' && (
            <span className="text-2xs px-1.5 py-0.5 rounded bg-green-200 dark:bg-green-800 text-green-600 dark:text-green-400">
              {t('workflow.config.applied')}
            </span>
          )}
        </div>
      </div>

      <div className="px-3 py-2 space-y-2 relative">
        {/* Layer 1: Summary */}
        <div
          className={clsx(
            'transition-opacity duration-150',
            layer === 1 ? 'opacity-100' : 'opacity-0 h-0 overflow-hidden',
          )}
        >
          <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
            <ConfigRow label={t('workflow.config.flow')} value={displayFlowLevel} />
            <ConfigRow label={t('workflow.config.tdd')} value={displayTddMode} />
            <ConfigRow label={t('workflow.config.maxParallel')} value={String(localConfig.maxParallel)} />
            <ConfigRow
              label={t('workflow.config.qualityGates')}
              value={localConfig.qualityGatesEnabled ? t('workflow.config.on') : t('workflow.config.off')}
            />
            <ConfigRow
              label={t('workflow.config.interview')}
              value={localConfig.specInterviewEnabled ? t('workflow.config.enabled') : t('workflow.config.skip')}
            />
          </div>

          {isActive && (
            <div className="flex items-center gap-2 pt-1">
              <button
                onClick={() => {
                  void handleConfirm();
                }}
                disabled={isSubmitting}
                className="px-3 py-1 text-xs font-medium rounded-md bg-sky-600 text-white hover:bg-sky-700 disabled:opacity-60 disabled:cursor-not-allowed transition-colors"
              >
                {t('workflow.config.continue')}
              </button>
              <button onClick={() => setLayer(2)} className="text-xs text-sky-600 dark:text-sky-400 hover:underline">
                {t('workflow.config.customize')}
              </button>
              {submitError && <span className="text-2xs text-rose-600 dark:text-rose-300">{submitError}</span>}
            </div>
          )}
        </div>

        {/* Layer 2: Edit form */}
        <div
          className={clsx(
            'transition-opacity duration-150',
            layer === 2 ? 'opacity-100' : 'opacity-0 h-0 overflow-hidden',
          )}
        >
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
              <label className="text-xs text-sky-700 dark:text-sky-300 w-24 shrink-0">
                {t('workflow.config.maxParallel')}
              </label>
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
              onClick={() => {
                void handleConfirm();
              }}
              disabled={isSubmitting}
              className="px-3 py-1 text-xs font-medium rounded-md bg-sky-600 text-white hover:bg-sky-700 disabled:opacity-60 disabled:cursor-not-allowed transition-colors"
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
        </div>

        {/* Layer 3: NL override */}
        <div
          className={clsx(
            'transition-opacity duration-150',
            layer === 3 ? 'opacity-100' : 'opacity-0 h-0 overflow-hidden',
          )}
        >
          <p className="text-xs text-sky-600 dark:text-sky-400">{t('workflow.config.nlDescription')}</p>
          <div className="flex gap-2">
            <input
              type="text"
              value={nlOverride}
              onChange={(e) => setNlOverride(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  void handleNlSubmit();
                }
              }}
              disabled={isSubmitting}
              placeholder={t('workflow.config.nlPlaceholder')}
              className="flex-1 px-2 py-1 text-xs rounded border border-sky-300 dark:border-sky-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 focus:outline-none focus:ring-1 focus:ring-sky-500"
            />
            <button
              onClick={() => {
                void handleNlSubmit();
              }}
              disabled={isSubmitting}
              className="px-3 py-1 text-xs font-medium rounded-md bg-sky-600 text-white hover:bg-sky-700 disabled:opacity-60 disabled:cursor-not-allowed transition-colors"
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
        </div>
      </div>
    </div>
  );
}

function localizeFlowLevel(t: ReturnType<typeof useTranslation>['t'], value: ConfigCardData['flowLevel']): string {
  return t(`workflow.config.values.flowLevel.${value}`, { defaultValue: t(`workflow.config.${value}`) });
}

function localizeTddMode(t: ReturnType<typeof useTranslation>['t'], value: ConfigCardData['tddMode']): string {
  return t(`workflow.config.values.tddMode.${value}`, { defaultValue: t(`workflow.config.${value}`) });
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
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
    </div>
  );
}

function ToggleField({ label, value, onChange }: { label: string; value: boolean; onChange: (v: boolean) => void }) {
  const { t } = useTranslation('simpleMode');
  return (
    <div className="flex items-center gap-2">
      <label className="text-xs text-sky-700 dark:text-sky-300 w-24 shrink-0">{label}</label>
      <button
        onClick={() => onChange(!value)}
        className={clsx(
          'relative w-8 h-4 rounded-full transition-colors',
          value ? 'bg-sky-600' : 'bg-gray-300 dark:bg-gray-600',
        )}
      >
        <span
          className={clsx(
            'absolute top-0.5 left-0.5 w-3 h-3 rounded-full bg-white transition-transform',
            value && 'translate-x-4',
          )}
        />
      </button>
      <span className="text-xs text-sky-600/80 dark:text-sky-400/80">
        {value ? t('workflow.config.on') : t('workflow.config.off')}
      </span>
    </div>
  );
}
