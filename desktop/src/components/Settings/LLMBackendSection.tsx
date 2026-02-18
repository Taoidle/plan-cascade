/**
 * LLMBackendSection Component
 *
 * LLM backend selection and API key configuration.
 */

import { useState, useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import { CheckCircledIcon, CrossCircledIcon, EyeOpenIcon, EyeNoneIcon } from '@radix-ui/react-icons';
import { useTranslation } from 'react-i18next';
import { useSettingsStore, type Backend, type StandaloneContextTurns, type GlmEndpoint, type MinimaxEndpoint } from '../../store/settings';
import {
  BACKEND_OPTIONS,
  FALLBACK_MODELS_BY_PROVIDER,
  CUSTOM_MODELS_STORAGE_KEY,
  MODEL_DEFAULT_VALUE,
  MODEL_CUSTOM_VALUE,
  normalizeProvider,
  dedupeModels,
  getApiKeyRequiredProviders,
  getLocalProviderApiKey,
  setLocalProviderApiKey,
  getLocalProviderApiKeyStatuses,
  type ApiKeyStatus,
} from '../../lib/providers';

const backendOptions = BACKEND_OPTIONS;

const standaloneContextTurnValues: StandaloneContextTurns[] = [2, 4, 6, 8, 10, 20, 50, 100, 200, 500, -1];

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

const API_KEY_REQUIRED_PROVIDERS = getApiKeyRequiredProviders();

export function LLMBackendSection() {
  const { t } = useTranslation('settings');
  const {
    backend,
    setBackend,
    model,
    setModel,
    setProvider,
    standaloneContextTurns,
    setStandaloneContextTurns,
    enableContextCompaction,
    setEnableContextCompaction,
    showReasoningOutput,
    setShowReasoningOutput,
    enableThinking,
    setEnableThinking,
    showSubAgentEvents,
    setShowSubAgentEvents,
    searchProvider,
    setSearchProvider,
    glmEndpoint,
    setGlmEndpoint,
    minimaxEndpoint,
    setMinimaxEndpoint,
  } = useSettingsStore();
  const [apiKeyStatuses, setApiKeyStatuses] = useState<ApiKeyStatus>(() => getLocalProviderApiKeyStatuses());
  const [apiKeyInputs, setApiKeyInputs] = useState<{ [provider: string]: string }>({});
  const [showApiKey, setShowApiKey] = useState<{ [provider: string]: boolean }>({});
  const [savingKey, setSavingKey] = useState<string | null>(null);
  const [loadingSavedKey, setLoadingSavedKey] = useState<string | null>(null);
  const [keyMessage, setKeyMessage] = useState<{ provider: string; type: 'success' | 'error'; message: string } | null>(null);
  const [modelsByProvider, setModelsByProvider] = useState<Record<string, string[]>>(FALLBACK_MODELS_BY_PROVIDER);
  const [customModelsByProvider, setCustomModelsByProvider] = useState<Record<string, string[]>>({});
  const [customModelInput, setCustomModelInput] = useState('');

  // Load cached custom models and fetch provider metadata on mount.
  useEffect(() => {
    const storedCustomModels = localStorage.getItem(CUSTOM_MODELS_STORAGE_KEY);
    if (storedCustomModels) {
      try {
        const parsed = JSON.parse(storedCustomModels) as Record<string, string[]>;
        const normalized: Record<string, string[]> = {};
        Object.entries(parsed).forEach(([provider, models]) => {
          normalized[normalizeProvider(provider)] = dedupeModels(Array.isArray(models) ? models : []);
        });
        setCustomModelsByProvider(normalized);
      } catch {
        // Ignore malformed localStorage payload
      }
    }

    void fetchProviderModels();
  }, []);

  const fetchProviderModels = async () => {
    try {
      const result = await invoke<CommandResponse<ProviderCatalog[]>>('list_providers');
      if (!result.success || !result.data) return;

      const nextModelsByProvider: Record<string, string[]> = { ...FALLBACK_MODELS_BY_PROVIDER };
      result.data.forEach((provider) => {
        const providerKey = normalizeProvider(provider.provider_type || '');
        const modelIds = dedupeModels((provider.models || []).map((m) => m.id));
        nextModelsByProvider[providerKey] = dedupeModels([
          ...(FALLBACK_MODELS_BY_PROVIDER[providerKey] || []),
          ...modelIds,
        ]);
      });
      setModelsByProvider(nextModelsByProvider);
    } catch {
      // Keep fallback model list when provider metadata fetch fails.
    }
  };

  const fetchApiKeyStatuses = useCallback(async () => {
    const localFallbackStatuses = getLocalProviderApiKeyStatuses();

    // Preferred path: batch query via list_configured_api_key_providers (single keychain access).
    try {
      const result = await invoke<CommandResponse<string[]>>('list_configured_api_key_providers');
      if (result.success) {
        const providers = result.data || [];
        const statuses: ApiKeyStatus = {};
        providers.forEach((provider) => {
          statuses[normalizeProvider(provider)] = true;
        });
        const merged = { ...statuses, ...localFallbackStatuses };
        setApiKeyStatuses(merged);
        localStorage.setItem('plan-cascade-api-keys', JSON.stringify(merged));
        return;
      }
      throw new Error(result.error || 'Failed to fetch API key statuses');
    } catch {
      // Fall through to per-provider fallback path.
    }

    // Fallback path: individually verify each provider by attempting to read stored key.
    try {
      const checks = await Promise.all(
        API_KEY_REQUIRED_PROVIDERS.map(async (provider) => {
          try {
            const result = await invoke<CommandResponse<string | null>>('get_provider_api_key', {
              provider,
            });
            if (!result.success) {
              return { provider, configured: false, checked: true };
            }
            const key = typeof result.data === 'string' ? result.data.trim() : '';
            return { provider, configured: key.length > 0, checked: true };
          } catch {
            return { provider, configured: false, checked: false };
          }
        })
      );

      if (checks.some((item) => item.checked)) {
        const statuses: ApiKeyStatus = {};
        checks.forEach(({ provider, configured }) => {
          if (configured) {
            statuses[provider] = true;
          }
        });
        const merged = { ...statuses, ...localFallbackStatuses };
        setApiKeyStatuses(merged);
        localStorage.setItem('plan-cascade-api-keys', JSON.stringify(merged));
        return;
      }
    } catch (error) {
      console.error('Failed to fetch API key statuses:', error);
    }

    // Final fallback: local cache.
    const stored = localStorage.getItem('plan-cascade-api-keys');
    if (stored) {
      try {
        const parsed = JSON.parse(stored) as ApiKeyStatus;
        const normalized: ApiKeyStatus = {};
        Object.entries(parsed).forEach(([provider, configured]) => {
          if (configured) {
            normalized[normalizeProvider(provider)] = true;
          }
        });
        setApiKeyStatuses({ ...normalized, ...localFallbackStatuses });
      } catch {
        setApiKeyStatuses(localFallbackStatuses);
      }
    } else {
      setApiKeyStatuses(localFallbackStatuses);
    }
  }, []);

  // Refresh status on mount and whenever active backend/local key changes.
  useEffect(() => {
    void fetchApiKeyStatuses();
  }, [fetchApiKeyStatuses]);

  const handleBackendChange = (newBackend: Backend) => {
    setBackend(newBackend);
    const option = backendOptions.find((o) => o.id === newBackend);
    if (option?.provider) {
      setProvider(option.provider);
    }
  };

  const handleSaveApiKey = async (provider: string) => {
    const canonicalProvider = normalizeProvider(provider);
    const apiKey = apiKeyInputs[canonicalProvider];
    if (!apiKey?.trim()) return;

    setSavingKey(canonicalProvider);
    setKeyMessage(null);

    try {
      // Store API key in OS keyring via Tauri command
      const result = await invoke<CommandResponse<boolean>>('configure_provider', {
        provider: canonicalProvider,
        apiKey: apiKey.trim(),
      });

      if (!result.success) {
        throw new Error(result.error || 'Failed to store API key');
      }

      // Update local status tracking
      const currentStatuses = { ...apiKeyStatuses, [canonicalProvider]: true };
      localStorage.setItem('plan-cascade-api-keys', JSON.stringify(currentStatuses));
      setApiKeyStatuses(currentStatuses);
      setApiKeyInputs((prev) => ({ ...prev, [canonicalProvider]: apiKey.trim() }));
      setLocalProviderApiKey(canonicalProvider, apiKey.trim());
      setKeyMessage({ provider: canonicalProvider, type: 'success', message: t('llm.apiKey.saveSuccess') });
      await fetchApiKeyStatuses();
    } catch (error) {
      const msg = error instanceof Error ? error.message : t('llm.apiKey.saveError');
      setKeyMessage({ provider: canonicalProvider, type: 'error', message: msg });
    } finally {
      setSavingKey(null);
    }
  };

  const handleDeleteApiKey = async (provider: string) => {
    const canonicalProvider = normalizeProvider(provider);
    setSavingKey(canonicalProvider);
    setKeyMessage(null);

    try {
      // Delete API key from OS keyring via Tauri command (empty string = delete)
      const result = await invoke<CommandResponse<boolean>>('configure_provider', {
        provider: canonicalProvider,
        apiKey: '',
      });

      if (!result.success) {
        throw new Error(result.error || 'Failed to remove API key');
      }

      // Update local status tracking
      const currentStatuses = { ...apiKeyStatuses, [canonicalProvider]: false };
      localStorage.setItem('plan-cascade-api-keys', JSON.stringify(currentStatuses));
      setApiKeyStatuses(currentStatuses);
      setApiKeyInputs((prev) => ({ ...prev, [canonicalProvider]: '' }));
      setLocalProviderApiKey(canonicalProvider, '');
      setKeyMessage({ provider: canonicalProvider, type: 'success', message: t('llm.apiKey.removeSuccess') });
      await fetchApiKeyStatuses();
    } catch (error) {
      const msg = error instanceof Error ? error.message : t('llm.apiKey.removeError');
      setKeyMessage({ provider: canonicalProvider, type: 'error', message: msg });
    } finally {
      setSavingKey(null);
    }
  };

  const handleToggleApiKeyVisibility = async (provider: string) => {
    const canonicalProvider = normalizeProvider(provider);
    const nextVisible = !showApiKey[canonicalProvider];

    setShowApiKey((prev) => ({
      ...prev,
      [canonicalProvider]: nextVisible,
    }));

    if (!nextVisible) return;
    if ((apiKeyInputs[canonicalProvider] || '').trim().length > 0) return;

    setLoadingSavedKey(canonicalProvider);
    try {
      const result = await invoke<CommandResponse<string | null>>('get_provider_api_key', {
        provider: canonicalProvider,
      });
      if (result.success && typeof result.data === 'string' && result.data.trim().length > 0) {
        setApiKeyInputs((prev) => ({
          ...prev,
          [canonicalProvider]: result.data || '',
        }));
        setApiKeyStatuses((prev) => ({
          ...prev,
          [canonicalProvider]: true,
        }));
      } else {
        const fallbackKey = getLocalProviderApiKey(canonicalProvider);
        if (fallbackKey) {
          setApiKeyInputs((prev) => ({
            ...prev,
            [canonicalProvider]: fallbackKey,
          }));
          setApiKeyStatuses((prev) => ({
            ...prev,
            [canonicalProvider]: true,
          }));
        }
      }
    } catch {
      const fallbackKey = getLocalProviderApiKey(canonicalProvider);
      if (fallbackKey) {
        setApiKeyInputs((prev) => ({
          ...prev,
          [canonicalProvider]: fallbackKey,
        }));
        setApiKeyStatuses((prev) => ({
          ...prev,
          [canonicalProvider]: true,
        }));
      }
    } finally {
      setLoadingSavedKey(null);
    }
  };

  const persistCustomModels = (nextValue: Record<string, string[]>) => {
    setCustomModelsByProvider(nextValue);
    localStorage.setItem(CUSTOM_MODELS_STORAGE_KEY, JSON.stringify(nextValue));
  };

  const selectedOption = backendOptions.find((o) => o.id === backend);
  const selectedProvider = normalizeProvider(selectedOption?.provider || selectedOption?.id || '');
  const builtinModels = dedupeModels(modelsByProvider[selectedProvider] || []);
  const customModels = dedupeModels(customModelsByProvider[selectedProvider] || []);
  const allModels = dedupeModels([...builtinModels, ...customModels]);

  const modelSelectValue = (() => {
    if (!model?.trim()) return MODEL_DEFAULT_VALUE;
    return allModels.includes(model) ? model : MODEL_CUSTOM_VALUE;
  })();

  const addCustomModel = () => {
    const provider = selectedProvider;
    const nextModel = customModelInput.trim();
    if (!provider || !nextModel) return;

    const existing = customModelsByProvider[provider] || [];
    if (existing.includes(nextModel)) {
      setModel(nextModel);
      setCustomModelInput('');
      return;
    }

    const next = {
      ...customModelsByProvider,
      [provider]: dedupeModels([...existing, nextModel]),
    };
    persistCustomModels(next);
    setModel(nextModel);
    setCustomModelInput('');
  };

  const removeCustomModel = (provider: string, modelToRemove: string) => {
    const existing = customModelsByProvider[provider] || [];
    const next = {
      ...customModelsByProvider,
      [provider]: existing.filter((m) => m !== modelToRemove),
    };
    persistCustomModels(next);
  };

  const selectedOptionName = selectedOption ? t(`llm.providers.${selectedOption.i18nKey}.name`) : '';

  const getModelPlaceholder = (b: Backend): string => {
    const key = `llm.model.placeholders.${b}`;
    const result = t(key);
    return result !== key ? result : t('llm.model.placeholders.default');
  };

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">
          {t('llm.title')}
        </h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {t('llm.description')}
        </p>
      </div>

      {/* Backend Selection */}
      <section className="space-y-3">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {t('llm.provider.label')}
        </h3>
        <div className="space-y-2">
          {backendOptions.map((option) => (
            <label
              key={option.id}
              className={clsx(
                'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
                'transition-colors',
                backend === option.id
                  ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                  : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800'
              )}
            >
              <input
                type="radio"
                name="backend"
                value={option.id}
                checked={backend === option.id}
                onChange={() => handleBackendChange(option.id)}
                className="mt-1 text-primary-600"
              />
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-gray-900 dark:text-white">
                    {t(`llm.providers.${option.i18nKey}.name`)}
                  </span>
                  {option.requiresApiKey && (
                    (() => {
                      const provider = normalizeProvider(option.provider || option.id);
                      const configured = !!apiKeyStatuses[provider];
                      return (
                    <span
                      className={clsx(
                        'inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs',
                        configured
                          ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
                          : 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400'
                      )}
                    >
                      {configured ? (
                        <>
                          <CheckCircledIcon className="w-3 h-3" /> {t('llm.apiKey.configured')}
                        </>
                      ) : (
                        <>
                          <CrossCircledIcon className="w-3 h-3" /> {t('llm.apiKey.required')}
                        </>
                      )}
                    </span>
                      );
                    })()
                  )}
                </div>
                <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                  {t(`llm.providers.${option.i18nKey}.description`)}
                </div>
              </div>
            </label>
          ))}
        </div>
      </section>

      {/* API Key Configuration (if required) */}
      {selectedOption?.requiresApiKey && (
        <section className="space-y-4">
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">
            {t('llm.apiKey.title', { name: selectedOptionName })}
          </h3>

          <div className="space-y-3">
            <div className="flex gap-2">
              <div className="relative flex-1">
                {(() => {
                  const provider = normalizeProvider(selectedOption.provider || '');
                  const configured = !!apiKeyStatuses[provider];
                  const inputValue = apiKeyInputs[provider] || '';
                  return (
                <input
                  type={showApiKey[provider] ? 'text' : 'password'}
                  placeholder={
                    configured
                      ? t('llm.apiKey.placeholderConfigured')
                      : t('llm.apiKey.placeholder')
                  }
                  value={inputValue}
                  onChange={(e) => {
                    const value = e.target.value;
                    setApiKeyInputs((prev) => ({
                      ...prev,
                      [provider]: value,
                    }));
                  }}
                  className={clsx(
                    'w-full px-3 py-2 pr-10 rounded-lg border',
                    'border-gray-200 dark:border-gray-700',
                    'bg-white dark:bg-gray-800',
                    'text-gray-900 dark:text-white',
                    'focus:outline-none focus:ring-2 focus:ring-primary-500'
                  )}
                />
                  );
                })()}
                <button
                  type="button"
                  onClick={() => handleToggleApiKeyVisibility(selectedOption.provider || '')}
                  className={clsx(
                    'absolute right-2 top-1/2 -translate-y-1/2 p-1',
                    'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300'
                  )}
                >
                  {showApiKey[normalizeProvider(selectedOption.provider || '')] ? (
                    <EyeNoneIcon className="w-4 h-4" />
                  ) : (
                    <EyeOpenIcon className="w-4 h-4" />
                  )}
                </button>
              </div>
              <button
                onClick={() => handleSaveApiKey(normalizeProvider(selectedOption.provider || ''))}
                disabled={
                  savingKey === normalizeProvider(selectedOption.provider || '') ||
                  !apiKeyInputs[normalizeProvider(selectedOption.provider || '')]?.trim()
                }
                className={clsx(
                  'px-4 py-2 rounded-lg',
                  'bg-primary-600 text-white',
                  'hover:bg-primary-700',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                  'disabled:opacity-50 disabled:cursor-not-allowed'
                )}
              >
                {savingKey === normalizeProvider(selectedOption.provider || '') ? t('llm.apiKey.saving') : t('llm.apiKey.save')}
              </button>
              {apiKeyStatuses[normalizeProvider(selectedOption.provider || '')] && (
                <button
                  onClick={() => handleDeleteApiKey(normalizeProvider(selectedOption.provider || ''))}
                  disabled={savingKey === normalizeProvider(selectedOption.provider || '')}
                  className={clsx(
                    'px-4 py-2 rounded-lg',
                    'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400',
                    'hover:bg-red-200 dark:hover:bg-red-900/50',
                    'focus:outline-none focus:ring-2 focus:ring-red-500',
                    'disabled:opacity-50 disabled:cursor-not-allowed'
                  )}
                >
                  {t('llm.apiKey.remove')}
                </button>
              )}
            </div>
            {loadingSavedKey === normalizeProvider(selectedOption.provider || '') && (
              <p className="text-sm text-gray-500 dark:text-gray-400">
                {t('llm.apiKey.loading')}
              </p>
            )}

            {keyMessage && keyMessage.provider === normalizeProvider(selectedOption.provider || '') && (
              <p
                className={clsx(
                  'text-sm',
                  keyMessage.type === 'success'
                    ? 'text-green-600 dark:text-green-400'
                    : 'text-red-600 dark:text-red-400'
                )}
              >
                {keyMessage.message}
              </p>
            )}
          </div>
        </section>
      )}

      {/* GLM Endpoint Selection (only when GLM is selected) */}
      {selectedProvider === 'glm' && (
        <section className="space-y-3">
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">
            {t('llm.glmEndpoint.label')}
          </h3>
          <div className="space-y-2">
            {(['standard', 'coding'] as GlmEndpoint[]).map((ep) => (
              <label
                key={ep}
                className={clsx(
                  'flex items-start gap-3 p-3 rounded-lg border cursor-pointer',
                  'transition-colors',
                  glmEndpoint === ep
                    ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                    : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800'
                )}
              >
                <input
                  type="radio"
                  name="glmEndpoint"
                  value={ep}
                  checked={glmEndpoint === ep}
                  onChange={() => setGlmEndpoint(ep)}
                  className="mt-0.5 text-primary-600"
                />
                <div>
                  <span className="font-medium text-gray-900 dark:text-white text-sm">
                    {t(`llm.glmEndpoint.options.${ep}.name`)}
                  </span>
                  <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                    {t(`llm.glmEndpoint.options.${ep}.description`)}
                  </p>
                  <code className="text-xs text-gray-400 dark:text-gray-500 mt-0.5 block">
                    {t(`llm.glmEndpoint.options.${ep}.url`)}
                  </code>
                </div>
              </label>
            ))}
          </div>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            {t('llm.glmEndpoint.help')}
          </p>
        </section>
      )}

      {/* MiniMax Endpoint Selection (only when MiniMax is selected) */}
      {selectedProvider === 'minimax' && (
        <section className="space-y-3">
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">
            {t('llm.minimaxEndpoint.label')}
          </h3>
          <div className="space-y-2">
            {(['international', 'china'] as MinimaxEndpoint[]).map((ep) => (
              <label
                key={ep}
                className={clsx(
                  'flex items-start gap-3 p-3 rounded-lg border cursor-pointer',
                  'transition-colors',
                  minimaxEndpoint === ep
                    ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                    : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800'
                )}
              >
                <input
                  type="radio"
                  name="minimaxEndpoint"
                  value={ep}
                  checked={minimaxEndpoint === ep}
                  onChange={() => setMinimaxEndpoint(ep)}
                  className="mt-0.5 text-primary-600"
                />
                <div>
                  <span className="font-medium text-gray-900 dark:text-white text-sm">
                    {t(`llm.minimaxEndpoint.options.${ep}.name`)}
                  </span>
                  <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                    {t(`llm.minimaxEndpoint.options.${ep}.description`)}
                  </p>
                  <code className="text-xs text-gray-400 dark:text-gray-500 mt-0.5 block">
                    {t(`llm.minimaxEndpoint.options.${ep}.url`)}
                  </code>
                </div>
              </label>
            ))}
          </div>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            {t('llm.minimaxEndpoint.help')}
          </p>
        </section>
      )}

      {/* Model Selection */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {t('llm.model.label')}
        </h3>
        <select
          value={modelSelectValue}
          onChange={(e) => {
            const value = e.target.value;
            if (value === MODEL_DEFAULT_VALUE) {
              setModel('');
              return;
            }
            if (value === MODEL_CUSTOM_VALUE) {
              if (!model || allModels.includes(model)) {
                setModel('');
              }
              return;
            }
            setModel(value);
          }}
          className={clsx(
            'w-full max-w-md px-3 py-2 rounded-lg border',
            'border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800',
            'text-gray-900 dark:text-white',
            'focus:outline-none focus:ring-2 focus:ring-primary-500'
          )}
        >
          <option value={MODEL_DEFAULT_VALUE}>{t('llm.model.providerDefault')}</option>
          {allModels.map((modelId) => (
            <option key={modelId} value={modelId}>
              {modelId}
            </option>
          ))}
          <option value={MODEL_CUSTOM_VALUE}>
            {model?.trim() && !allModels.includes(model) ? t('llm.model.customPrefix', { model }) : t('llm.model.customModel')}
          </option>
        </select>
        <div className="flex gap-2 max-w-md">
          <input
            type="text"
            placeholder={getModelPlaceholder(backend)}
            value={customModelInput}
            onChange={(e) => setCustomModelInput(e.target.value)}
            className={clsx(
              'flex-1 px-3 py-2 rounded-lg border',
              'border-gray-200 dark:border-gray-700',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'focus:outline-none focus:ring-2 focus:ring-primary-500'
            )}
          />
          <button
            onClick={addCustomModel}
            disabled={!customModelInput.trim() || !selectedProvider}
            className={clsx(
              'px-4 py-2 rounded-lg',
              'bg-primary-600 text-white',
              'hover:bg-primary-700',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
              'disabled:opacity-50 disabled:cursor-not-allowed'
            )}
          >
            {t('llm.model.add')}
          </button>
        </div>
        {customModels.length > 0 && (
          <div className="flex flex-wrap gap-2 max-w-2xl">
            {customModels.map((customModel) => (
              <button
                key={customModel}
                onClick={() => removeCustomModel(selectedProvider, customModel)}
                className={clsx(
                  'px-2 py-1 rounded border text-xs',
                  'border-gray-300 dark:border-gray-600',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-100 dark:hover:bg-gray-800'
                )}
                title={t('llm.model.removeCustom')}
              >
                {customModel} x
              </button>
            ))}
          </div>
        )}
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {t('llm.model.help')}
        </p>
      </section>

      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {t('llm.contextTurns.label')}
        </h3>
        <select
          value={String(standaloneContextTurns)}
          onChange={(e) => setStandaloneContextTurns(Number(e.target.value) as StandaloneContextTurns)}
          className={clsx(
            'w-full max-w-md px-3 py-2 rounded-lg border',
            'border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800',
            'text-gray-900 dark:text-white',
            'focus:outline-none focus:ring-2 focus:ring-primary-500'
          )}
        >
          {standaloneContextTurnValues.map((value) => (
            <option key={value} value={String(value)}>
              {value === -1 ? t('llm.contextTurns.unlimited') : String(value)}
            </option>
          ))}
        </select>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {t('llm.contextTurns.help')}
        </p>
      </section>

      {/* Context Compaction (only for non-Claude-Code backends) */}
      {backend !== 'claude-code' && (
        <section className="space-y-4">
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">
            {t('llm.contextManagement.label')}
          </h3>
          <label className="flex items-start gap-3 cursor-pointer">
            <input
              type="checkbox"
              checked={enableContextCompaction}
              onChange={(e) => setEnableContextCompaction(e.target.checked)}
              className="mt-1 text-primary-600 rounded"
            />
            <div>
              <span className="text-sm font-medium text-gray-900 dark:text-white">
                {t('llm.contextManagement.compaction')}
              </span>
              <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                {t('llm.contextManagement.compactionHelp')}
              </p>
            </div>
          </label>
        </section>
      )}

      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {t('llm.streaming.label')}
        </h3>
        <label className="flex items-start gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={showSubAgentEvents}
            onChange={(e) => setShowSubAgentEvents(e.target.checked)}
            className="mt-1 text-primary-600 rounded"
          />
          <div>
            <span className="text-sm font-medium text-gray-900 dark:text-white">
              {t('llm.streaming.subAgentEvents')}
            </span>
            <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {t('llm.streaming.subAgentEventsHelp')}
            </p>
          </div>
        </label>
        <label className="flex items-start gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={showReasoningOutput}
            onChange={(e) => setShowReasoningOutput(e.target.checked)}
            className="mt-1 text-primary-600 rounded"
          />
          <div>
            <span className="text-sm font-medium text-gray-900 dark:text-white">
              {t('llm.streaming.reasoningTraces')}
            </span>
            <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {t('llm.streaming.reasoningTracesHelp')}
            </p>
          </div>
        </label>
        <label className="flex items-start gap-3 cursor-pointer">
          <input
            type="checkbox"
            checked={enableThinking}
            onChange={(e) => setEnableThinking(e.target.checked)}
            className="mt-1 text-primary-600 rounded"
          />
          <div>
            <span className="text-sm font-medium text-gray-900 dark:text-white">
              {t('llm.streaming.enableThinking')}
            </span>
            <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {t('llm.streaming.enableThinkingHelp')}
            </p>
          </div>
        </label>
      </section>

      {/* Search Provider */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          {t('llm.searchProvider.label')}
        </h3>
        <select
          value={searchProvider}
          onChange={(e) => setSearchProvider(e.target.value as 'tavily' | 'brave' | 'duckduckgo')}
          className={clsx(
            'w-full max-w-md px-3 py-2 rounded-lg border',
            'border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800',
            'text-gray-900 dark:text-white',
            'focus:outline-none focus:ring-2 focus:ring-primary-500'
          )}
        >
          <option value="duckduckgo">{t('llm.searchProvider.duckduckgo')}</option>
          <option value="tavily">{t('llm.searchProvider.tavily')}</option>
          <option value="brave">{t('llm.searchProvider.brave')}</option>
        </select>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {t('llm.searchProvider.help')}
        </p>
      </section>
    </div>
  );
}

export default LLMBackendSection;
