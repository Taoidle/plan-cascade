/**
 * Pricing Rules Panel
 *
 * Manual provider/model pricing management for analytics v2.
 */

import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { dedupeModels, FALLBACK_MODELS_BY_PROVIDER, normalizeProvider } from '../../lib/providers';
import { useSettingsStore } from '../../store/settings';
import { useAnalyticsStore, type PricingRule } from '../../store/analytics';

const BUILTIN_PROVIDER_ORDER = ['anthropic', 'openai', 'deepseek', 'glm', 'qwen', 'minimax', 'ollama'] as const;
const CUSTOM_MODEL_VALUE = '__custom_model__';
const COMMON_CURRENCIES: Array<{ code: string; name: string }> = [
  { code: 'USD', name: 'US Dollar' },
  { code: 'EUR', name: 'Euro' },
  { code: 'GBP', name: 'Pound Sterling' },
  { code: 'CNY', name: 'Chinese Yuan' },
  { code: 'JPY', name: 'Japanese Yen' },
  { code: 'HKD', name: 'Hong Kong Dollar' },
  { code: 'SGD', name: 'Singapore Dollar' },
  { code: 'KRW', name: 'South Korean Won' },
  { code: 'AUD', name: 'Australian Dollar' },
  { code: 'CAD', name: 'Canadian Dollar' },
  { code: 'CHF', name: 'Swiss Franc' },
  { code: 'INR', name: 'Indian Rupee' },
  { code: 'MXN', name: 'Mexican Peso' },
  { code: 'BRL', name: 'Brazilian Real' },
  { code: 'AED', name: 'UAE Dirham' },
];

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

interface ProviderCatalogModel {
  id: string;
}

interface ProviderCatalog {
  provider_type: string;
  models: ProviderCatalogModel[];
}

const EMPTY_RULE: PricingRule = {
  id: '',
  provider: 'anthropic',
  model_pattern: '*',
  currency: 'USD',
  input_per_million: 0,
  output_per_million: 0,
  cache_read_per_million: 0,
  cache_write_per_million: 0,
  thinking_per_million: 0,
  effective_from: Math.floor(Date.now() / 1000),
  effective_to: null,
  status: 'active',
  created_at: 0,
  updated_at: 0,
  note: null,
};

function providerDisplayName(provider: string): string {
  switch (provider) {
    case 'anthropic':
      return 'Anthropic';
    case 'openai':
      return 'OpenAI';
    case 'deepseek':
      return 'DeepSeek';
    case 'glm':
      return 'GLM';
    case 'qwen':
      return 'Qwen';
    case 'minimax':
      return 'MiniMax';
    case 'ollama':
      return 'Ollama';
    default:
      return provider;
  }
}

function toDateInput(ts?: number | null): string {
  if (!ts) return '';
  const d = new Date(ts * 1000);
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, '0');
  const day = String(d.getUTCDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

function fromDateInput(value: string, endOfDay = false): number {
  const [y, m, d] = value.split('-').map((v) => Number(v));
  if (endOfDay) {
    // Stored as end-exclusive boundary in UTC.
    return Math.floor(Date.UTC(y, m - 1, d + 1, 0, 0, 0, 0) / 1000);
  }
  return Math.floor(Date.UTC(y, m - 1, d, 0, 0, 0, 0) / 1000);
}

interface PricingRulesPanelProps {
  className?: string;
}

export function PricingRulesPanel({ className }: PricingRulesPanelProps) {
  const { t } = useTranslation('analytics');
  const settingsProvider = useSettingsStore((state) => state.provider);
  const settingsModel = useSettingsStore((state) => state.model);
  const { pricingRules, pricingLoading, upsertPricingRule, deletePricingRule, recomputeCosts, error, clearError } =
    useAnalyticsStore();

  const [providerModels, setProviderModels] = useState<Record<string, string[]>>(FALLBACK_MODELS_BY_PROVIDER);
  const [editing, setEditing] = useState<PricingRule>(() => {
    const provider = normalizeProvider(useSettingsStore.getState().provider || EMPTY_RULE.provider);
    const model = useSettingsStore.getState().model?.trim() || EMPTY_RULE.model_pattern;
    return {
      ...EMPTY_RULE,
      provider,
      model_pattern: model,
      effective_from: Math.floor(Date.now() / 1000),
    };
  });
  const [saving, setSaving] = useState(false);
  const [recomputing, setRecomputing] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [currencySearch, setCurrencySearch] = useState('');

  useEffect(() => {
    const loadProviderCatalog = async () => {
      try {
        const response = await invoke<CommandResponse<ProviderCatalog[]>>('list_providers');
        if (!response.success || !response.data) return;

        const nextMap: Record<string, string[]> = { ...FALLBACK_MODELS_BY_PROVIDER };
        response.data.forEach((provider) => {
          const key = normalizeProvider(provider.provider_type || '');
          if (!key) return;
          const dynamicModels = dedupeModels((provider.models || []).map((m) => m.id || ''));
          nextMap[key] = dedupeModels([...(nextMap[key] || []), ...dynamicModels]);
        });
        setProviderModels(nextMap);
      } catch {
        // Fallback list is good enough when provider catalog is unavailable.
      }
    };
    void loadProviderCatalog();
  }, []);

  const sortedRules = useMemo(() => {
    return [...pricingRules].sort((a, b) => {
      if (a.provider !== b.provider) return a.provider.localeCompare(b.provider);
      if (a.model_pattern !== b.model_pattern) return a.model_pattern.localeCompare(b.model_pattern);
      return b.effective_from - a.effective_from;
    });
  }, [pricingRules]);

  const availableProviders = useMemo(() => {
    const fromRules = pricingRules.map((rule) => normalizeProvider(rule.provider));
    const fromCatalog = Object.keys(providerModels);
    const all = new Set<string>([
      ...BUILTIN_PROVIDER_ORDER,
      ...fromRules,
      ...fromCatalog,
      normalizeProvider(settingsProvider || ''),
      normalizeProvider(editing.provider || ''),
    ]);
    const providers = Array.from(all).filter(Boolean);
    providers.sort((a, b) => {
      const ai = BUILTIN_PROVIDER_ORDER.indexOf(a as (typeof BUILTIN_PROVIDER_ORDER)[number]);
      const bi = BUILTIN_PROVIDER_ORDER.indexOf(b as (typeof BUILTIN_PROVIDER_ORDER)[number]);
      if (ai !== -1 && bi !== -1) return ai - bi;
      if (ai !== -1) return -1;
      if (bi !== -1) return 1;
      return a.localeCompare(b);
    });
    return providers;
  }, [pricingRules, providerModels, settingsProvider, editing.provider]);

  const selectedProvider = normalizeProvider(editing.provider || availableProviders[0] || EMPTY_RULE.provider);

  const knownModels = useMemo(() => {
    const catalogModels = providerModels[selectedProvider] || [];
    const ruleModels = pricingRules
      .filter((rule) => normalizeProvider(rule.provider) === selectedProvider)
      .map((rule) => rule.model_pattern)
      .filter((pattern) => pattern && !pattern.includes('*'));
    const currentSettingModel =
      normalizeProvider(settingsProvider || '') === selectedProvider && settingsModel?.trim() ? [settingsModel] : [];
    return dedupeModels([...catalogModels, ...ruleModels, ...currentSettingModel]);
  }, [providerModels, pricingRules, selectedProvider, settingsProvider, settingsModel]);

  const modelSelectValue =
    editing.model_pattern === '*' || knownModels.includes(editing.model_pattern)
      ? editing.model_pattern
      : CUSTOM_MODEL_VALUE;

  const currencyOptions = useMemo(() => {
    const currencyNameMap = new Map(COMMON_CURRENCIES.map((item) => [item.code, item.name]));
    const knownCodes = dedupeModels([
      ...COMMON_CURRENCIES.map((item) => item.code),
      ...pricingRules.map((rule) => (rule.currency || '').trim().toUpperCase()),
      (editing.currency || 'USD').trim().toUpperCase(),
    ]);
    return knownCodes.map((code) => ({ code, name: currencyNameMap.get(code) || code }));
  }, [pricingRules, editing.currency]);

  const filteredCurrencyOptions = useMemo(() => {
    const query = currencySearch.trim().toLowerCase();
    let filtered = currencyOptions.filter((item) => {
      if (!query) return true;
      return item.code.toLowerCase().includes(query) || item.name.toLowerCase().includes(query);
    });

    const selectedCode = (editing.currency || 'USD').trim().toUpperCase();
    if (!filtered.some((item) => item.code === selectedCode)) {
      const selected = currencyOptions.find((item) => item.code === selectedCode);
      if (selected) {
        filtered = [selected, ...filtered];
      }
    }
    return filtered;
  }, [currencyOptions, currencySearch, editing.currency]);

  const resetForm = () => {
    const provider = normalizeProvider(settingsProvider || EMPTY_RULE.provider);
    const modelPattern = settingsModel?.trim() || '*';
    setEditing({
      ...EMPTY_RULE,
      provider,
      model_pattern: modelPattern,
      effective_from: Math.floor(Date.now() / 1000),
    });
    setShowAdvanced(false);
    setFormError(null);
    setCurrencySearch('');
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setFormError(null);
    clearError();

    const provider = normalizeProvider(editing.provider.trim());
    const modelPattern = editing.model_pattern.trim();
    if (!provider || !modelPattern) {
      setFormError(t('pricing.validationProviderModel', 'Provider and model pattern are required'));
      setSaving(false);
      return;
    }
    const currency = (editing.currency || '').trim().toUpperCase();
    if (!currencyOptions.some((item) => item.code === currency)) {
      setFormError(t('pricing.validationCurrencyRequired', 'Please select a valid currency from the list'));
      setSaving(false);
      return;
    }

    const saved = await upsertPricingRule({
      ...editing,
      provider,
      model_pattern: modelPattern,
      currency,
      note: editing.note?.trim() ? editing.note.trim() : null,
    });

    setSaving(false);
    if (saved) {
      resetForm();
    }
  };

  const handleEdit = (rule: PricingRule) => {
    setEditing({
      ...rule,
      currency: (rule.currency || 'USD').trim().toUpperCase(),
    });
    setShowAdvanced(true);
    setFormError(null);
    setCurrencySearch('');
  };

  const handleDelete = async (ruleId: string) => {
    setFormError(null);
    clearError();
    await deletePricingRule(ruleId);
    if (editing.id === ruleId) {
      resetForm();
    }
  };

  const handleRecompute = async () => {
    setRecomputing(true);
    setFormError(null);
    clearError();
    await recomputeCosts();
    setRecomputing(false);
  };

  const handleUseCurrentModel = () => {
    const provider = normalizeProvider(settingsProvider || editing.provider || EMPTY_RULE.provider);
    const modelPattern = settingsModel?.trim() || '*';
    setEditing((prev) => ({
      ...prev,
      provider,
      model_pattern: modelPattern,
    }));
  };

  const handleSelectProvider = (provider: string) => {
    const normalized = normalizeProvider(provider);
    const models = providerModels[normalized] || [];

    setEditing((prev) => {
      const currentPattern = prev.model_pattern.trim();
      const keepCurrent =
        prev.provider === normalized &&
        currentPattern.length > 0 &&
        (currentPattern === '*' || models.includes(currentPattern));
      return {
        ...prev,
        provider: normalized,
        model_pattern: keepCurrent ? currentPattern : models[0] || '*',
      };
    });
  };

  return (
    <div
      className={clsx(
        'bg-white dark:bg-gray-900 rounded-xl',
        'border border-gray-200 dark:border-gray-800',
        'p-5',
        className,
      )}
    >
      <div className="flex items-center justify-between gap-4 mb-4">
        <div>
          <h3 className="text-base font-semibold text-gray-900 dark:text-white">
            {t('pricing.title', 'Pricing Rules')}
          </h3>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
            {t('pricing.subtitle', 'Manual provider/model pricing with effective time windows')}
          </p>
        </div>
        <button
          onClick={handleRecompute}
          disabled={recomputing}
          className={clsx(
            'px-3 py-1.5 rounded-lg text-sm',
            'bg-gray-100 dark:bg-gray-800',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'disabled:opacity-50 disabled:cursor-not-allowed',
          )}
        >
          {recomputing ? t('pricing.recomputing', 'Recomputing...') : t('pricing.recompute', 'Recompute Costs')}
        </button>
      </div>

      {(formError || error) && (
        <div className="mb-4 px-3 py-2 rounded-lg border border-red-200 dark:border-red-900 bg-red-50 dark:bg-red-950/30 text-red-700 dark:text-red-300 text-sm">
          {formError || error}
        </div>
      )}

      <form onSubmit={handleSave} className="space-y-4 mb-5">
        <div className="rounded-lg border border-gray-200 dark:border-gray-800 p-3">
          <div className="flex flex-wrap items-center gap-2 mb-2">
            <span className="text-xs font-medium text-gray-600 dark:text-gray-300">
              {t('pricing.quickTitle', 'Quick Provider Selection')}
            </span>
            <button
              type="button"
              onClick={handleUseCurrentModel}
              className="px-2 py-1 rounded text-xs bg-primary-50 text-primary-700 dark:bg-primary-900/30 dark:text-primary-300 hover:bg-primary-100 dark:hover:bg-primary-900/50"
            >
              {t('pricing.useCurrent', 'Use Current Provider/Model')}
            </button>
          </div>
          <p className="text-xs text-gray-500 dark:text-gray-400 mb-3">
            {t(
              'pricing.quickDescription',
              'Choose a built-in provider first, then select a model and fill only input/output price.',
            )}
          </p>
          <div className="flex flex-wrap gap-2">
            {BUILTIN_PROVIDER_ORDER.map((provider) => {
              const active = selectedProvider === provider;
              return (
                <button
                  key={provider}
                  type="button"
                  onClick={() => handleSelectProvider(provider)}
                  className={clsx(
                    'px-2.5 py-1 rounded-md text-xs border transition-colors',
                    active
                      ? 'bg-primary-600 border-primary-600 text-white'
                      : 'bg-white dark:bg-gray-900 border-gray-300 dark:border-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
                  )}
                >
                  {providerDisplayName(provider)}
                </button>
              );
            })}
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-3">
          <div className="space-y-1">
            <label className="text-xs text-gray-600 dark:text-gray-300">{t('pricing.provider', 'Provider')}</label>
            <input
              list="analytics-pricing-providers"
              value={editing.provider}
              onChange={(e) => setEditing((prev) => ({ ...prev, provider: e.target.value }))}
              onBlur={() => setEditing((prev) => ({ ...prev, provider: normalizeProvider(prev.provider) }))}
              placeholder={t('pricing.providerHint', 'provider id, e.g. openai')}
              className="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
              required
            />
            <datalist id="analytics-pricing-providers">
              {availableProviders.map((provider) => (
                <option key={provider} value={provider} />
              ))}
            </datalist>
          </div>

          <div className="space-y-1">
            <label className="text-xs text-gray-600 dark:text-gray-300">
              {t('pricing.modelPattern', 'Model Pattern')}
            </label>
            <select
              value={modelSelectValue}
              onChange={(e) => {
                const value = e.target.value;
                setEditing((prev) => ({
                  ...prev,
                  model_pattern: value === CUSTOM_MODEL_VALUE ? '' : value,
                }));
              }}
              className="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
            >
              <option value="*">{t('pricing.allModels', 'All models (*)')}</option>
              {knownModels.map((model) => (
                <option key={model} value={model}>
                  {model}
                </option>
              ))}
              <option value={CUSTOM_MODEL_VALUE}>{t('pricing.customPattern', 'Custom pattern...')}</option>
            </select>
          </div>

          <div className="space-y-1">
            <label className="text-xs text-gray-600 dark:text-gray-300">
              {t('pricing.inputPrice', 'Input / million')}
            </label>
            <input
              type="number"
              min={0}
              value={editing.input_per_million}
              onChange={(e) => setEditing((prev) => ({ ...prev, input_per_million: Number(e.target.value) }))}
              className="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
            />
          </div>

          <div className="space-y-1">
            <label className="text-xs text-gray-600 dark:text-gray-300">
              {t('pricing.outputPrice', 'Output / million')}
            </label>
            <input
              type="number"
              min={0}
              value={editing.output_per_million}
              onChange={(e) => setEditing((prev) => ({ ...prev, output_per_million: Number(e.target.value) }))}
              className="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
            />
          </div>
        </div>

        {modelSelectValue === CUSTOM_MODEL_VALUE && (
          <input
            value={editing.model_pattern}
            onChange={(e) => setEditing((prev) => ({ ...prev, model_pattern: e.target.value }))}
            placeholder={t('pricing.modelPattern', 'Model Pattern (supports *)')}
            className="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
            required
          />
        )}

        <p className="text-xs text-gray-500 dark:text-gray-400">
          {t('pricing.unitHint', 'Unit: microdollars per million tokens')}
        </p>

        <div>
          <button
            type="button"
            onClick={() => setShowAdvanced((prev) => !prev)}
            className="px-3 py-1.5 rounded-lg text-sm bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700"
          >
            {showAdvanced ? t('pricing.hideAdvanced', 'Hide Advanced') : t('pricing.showAdvanced', 'Show Advanced')}
          </button>
        </div>

        {showAdvanced && (
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-3">
            <input
              type="number"
              min={0}
              value={editing.cache_read_per_million}
              onChange={(e) => setEditing((prev) => ({ ...prev, cache_read_per_million: Number(e.target.value) }))}
              placeholder={t('pricing.cacheRead', 'Cache Read / million')}
              className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
            />
            <input
              type="number"
              min={0}
              value={editing.cache_write_per_million}
              onChange={(e) => setEditing((prev) => ({ ...prev, cache_write_per_million: Number(e.target.value) }))}
              placeholder={t('pricing.cacheWrite', 'Cache Write / million')}
              className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
            />
            <input
              type="number"
              min={0}
              value={editing.thinking_per_million}
              onChange={(e) => setEditing((prev) => ({ ...prev, thinking_per_million: Number(e.target.value) }))}
              placeholder={t('pricing.thinking', 'Thinking / million')}
              className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
            />
            <select
              value={editing.status}
              onChange={(e) => setEditing((prev) => ({ ...prev, status: e.target.value as PricingRule['status'] }))}
              className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
            >
              <option value="active">{t('pricing.active', 'Active')}</option>
              <option value="disabled">{t('pricing.disabled', 'Disabled')}</option>
            </select>

            <input
              type="date"
              value={toDateInput(editing.effective_from)}
              onChange={(e) => setEditing((prev) => ({ ...prev, effective_from: fromDateInput(e.target.value) }))}
              className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
              required
            />
            <input
              type="date"
              value={toDateInput(editing.effective_to)}
              onChange={(e) =>
                setEditing((prev) => ({
                  ...prev,
                  effective_to: e.target.value ? fromDateInput(e.target.value, true) : null,
                }))
              }
              className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
            />
            <div className="space-y-1">
              <label className="text-xs text-gray-600 dark:text-gray-300">
                {t('pricing.currencySearch', 'Search Currency')}
              </label>
              <input
                value={currencySearch}
                onChange={(e) => setCurrencySearch(e.target.value)}
                placeholder={t('pricing.currencySearch', 'Search Currency')}
                className="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
              />
            </div>
            <div className="space-y-1">
              <label className="text-xs text-gray-600 dark:text-gray-300">{t('pricing.currency', 'Currency')}</label>
              <select
                value={(editing.currency || 'USD').trim().toUpperCase()}
                onChange={(e) => setEditing((prev) => ({ ...prev, currency: e.target.value.toUpperCase() }))}
                className="w-full px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
              >
                {filteredCurrencyOptions.length === 0 ? (
                  <option value={(editing.currency || 'USD').trim().toUpperCase()}>
                    {t('pricing.currencyNoMatch', 'No matching currency')}
                  </option>
                ) : (
                  filteredCurrencyOptions.map((item) => (
                    <option key={item.code} value={item.code}>
                      {item.code} - {item.name}
                    </option>
                  ))
                )}
              </select>
            </div>
            <input
              value={editing.note || ''}
              onChange={(e) => setEditing((prev) => ({ ...prev, note: e.target.value }))}
              placeholder={t('pricing.note', 'Note')}
              className="px-3 py-2 text-sm rounded-lg border border-gray-300 dark:border-gray-700 bg-white dark:bg-gray-800"
            />
          </div>
        )}

        <div className="flex items-center gap-2">
          <button
            type="submit"
            disabled={saving}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm text-white',
              'bg-primary-600 hover:bg-primary-700',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {saving ? t('pricing.saving', 'Saving...') : t('pricing.save', 'Save Rule')}
          </button>
          <button
            type="button"
            onClick={resetForm}
            className="px-4 py-2 rounded-lg text-sm bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700"
          >
            {t('pricing.reset', 'Reset')}
          </button>
        </div>
      </form>

      <div className="overflow-auto border border-gray-200 dark:border-gray-800 rounded-lg">
        <table className="w-full text-sm">
          <thead className="bg-gray-50 dark:bg-gray-800/40 text-gray-600 dark:text-gray-300">
            <tr>
              <th className="text-left px-3 py-2 font-medium">{t('pricing.provider', 'Provider')}</th>
              <th className="text-left px-3 py-2 font-medium">{t('pricing.modelPattern', 'Model Pattern')}</th>
              <th className="text-right px-3 py-2 font-medium">{t('pricing.inputPrice', 'Input / million')}</th>
              <th className="text-right px-3 py-2 font-medium">{t('pricing.outputPrice', 'Output / million')}</th>
              <th className="text-left px-3 py-2 font-medium">{t('pricing.window', 'Window')}</th>
              <th className="text-left px-3 py-2 font-medium">{t('pricing.status', 'Status')}</th>
              <th className="text-right px-3 py-2 font-medium">{t('pricing.actions', 'Actions')}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
            {pricingLoading ? (
              <tr>
                <td colSpan={7} className="px-3 py-6 text-center text-gray-500 dark:text-gray-400">
                  {t('pricing.loading', 'Loading pricing rules...')}
                </td>
              </tr>
            ) : sortedRules.length === 0 ? (
              <tr>
                <td colSpan={7} className="px-3 py-6 text-center text-gray-500 dark:text-gray-400">
                  {t('pricing.empty', 'No pricing rules configured')}
                </td>
              </tr>
            ) : (
              sortedRules.map((rule) => (
                <tr key={rule.id} className="text-gray-900 dark:text-white hover:bg-gray-50 dark:hover:bg-gray-800/30">
                  <td className="px-3 py-2">{providerDisplayName(rule.provider)}</td>
                  <td className="px-3 py-2 font-mono text-xs">{rule.model_pattern}</td>
                  <td className="px-3 py-2 text-right">{rule.input_per_million.toLocaleString()}</td>
                  <td className="px-3 py-2 text-right">{rule.output_per_million.toLocaleString()}</td>
                  <td className="px-3 py-2 text-xs text-gray-500 dark:text-gray-400">
                    {toDateInput(rule.effective_from)}
                    {' -> '}
                    {rule.effective_to ? toDateInput(rule.effective_to - 1) : '∞'}
                  </td>
                  <td className="px-3 py-2">
                    <span
                      className={clsx(
                        'px-2 py-0.5 rounded-full text-xs',
                        rule.status === 'active'
                          ? 'bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300'
                          : 'bg-gray-200 text-gray-700 dark:bg-gray-700 dark:text-gray-200',
                      )}
                    >
                      {rule.status}
                    </span>
                  </td>
                  <td className="px-3 py-2 text-right">
                    <div className="inline-flex gap-2">
                      <button
                        onClick={() => handleEdit(rule)}
                        className="px-2 py-1 rounded text-xs bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700"
                      >
                        {t('pricing.edit', 'Edit')}
                      </button>
                      <button
                        onClick={() => handleDelete(rule.id)}
                        className="px-2 py-1 rounded text-xs bg-red-100 dark:bg-red-900/40 text-red-700 dark:text-red-300 hover:bg-red-200 dark:hover:bg-red-900/60"
                      >
                        {t('pricing.delete', 'Delete')}
                      </button>
                    </div>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export default PricingRulesPanel;
