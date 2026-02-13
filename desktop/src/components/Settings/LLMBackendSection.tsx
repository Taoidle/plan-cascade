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
import { useSettingsStore, Backend, StandaloneContextTurns } from '../../store/settings';

interface BackendOption {
  id: Backend;
  i18nKey: string;
  requiresApiKey: boolean;
  provider?: string;
}

const backendOptions: BackendOption[] = [
  {
    id: 'claude-code',
    i18nKey: 'claude-code',
    requiresApiKey: false,
    provider: 'anthropic',
  },
  {
    id: 'claude-api',
    i18nKey: 'claude-api',
    requiresApiKey: true,
    provider: 'anthropic',
  },
  {
    id: 'openai',
    i18nKey: 'openai',
    requiresApiKey: true,
    provider: 'openai',
  },
  {
    id: 'deepseek',
    i18nKey: 'deepseek',
    requiresApiKey: true,
    provider: 'deepseek',
  },
  {
    id: 'glm',
    i18nKey: 'glm',
    requiresApiKey: true,
    provider: 'glm',
  },
  {
    id: 'qwen',
    i18nKey: 'qwen',
    requiresApiKey: true,
    provider: 'qwen',
  },
  {
    id: 'ollama',
    i18nKey: 'ollama',
    requiresApiKey: false,
    provider: 'ollama',
  },
];

const standaloneContextTurnValues: StandaloneContextTurns[] = [2, 4, 6, 8, 10, 20, 50, 100, 200, 500, -1];

interface ApiKeyStatus {
  [provider: string]: boolean;
}

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

const PROVIDER_ALIASES: Record<string, string> = {
  anthropic: 'anthropic',
  claude: 'anthropic',
  'claude-api': 'anthropic',
  openai: 'openai',
  deepseek: 'deepseek',
  glm: 'glm',
  'glm-api': 'glm',
  zhipu: 'glm',
  zhipuai: 'glm',
  qwen: 'qwen',
  'qwen-api': 'qwen',
  dashscope: 'qwen',
  alibaba: 'qwen',
  aliyun: 'qwen',
  ollama: 'ollama',
};

const CUSTOM_MODELS_STORAGE_KEY = 'plan-cascade-custom-models';
const LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY = 'plan-cascade-provider-api-key-cache';
const MODEL_DEFAULT_VALUE = '__provider_default__';
const MODEL_CUSTOM_VALUE = '__custom__';

const FALLBACK_MODELS_BY_PROVIDER: Record<string, string[]> = {
  anthropic: ['claude-3-5-sonnet-20241022', 'claude-3-opus-20240229'],
  openai: ['gpt-4o', 'o1-preview', 'o3-mini'],
  deepseek: ['deepseek-chat', 'deepseek-r1'],
  glm: [
    'glm-4.7',
    'glm-4.6',
    'glm-4.6v',
    'glm-4.6v-flash',
    'glm-4.6v-flashx',
    'glm-4.5',
    'glm-4.5-air',
    'glm-4.5-x',
    'glm-4.5-flash',
    'glm-4.5v',
    'glm-4.1v-thinking-flashx',
    'glm-4.1v-thinking-flash',
    'glm-4-air-250414',
    'glm-4-flash-250414',
    'glm-4-plus',
    'glm-4-air',
    'glm-4-airx',
    'glm-4-flash',
    'glm-4-flashx',
    'glm-4v-plus-0111',
    'glm-4v-flash',
  ],
  qwen: ['qwen3-plus', 'qwq-plus', 'qwen-plus', 'qwen-turbo'],
  ollama: ['llama3.2', 'deepseek-r1:14b', 'qwq:32b'],
};
const API_KEY_REQUIRED_PROVIDERS = dedupeModels(
  backendOptions
    .filter((option) => option.requiresApiKey)
    .map((option) => normalizeProvider(option.provider || option.id))
);

function normalizeProvider(provider: string): string {
  const normalized = provider.trim().toLowerCase();
  return PROVIDER_ALIASES[normalized] || normalized;
}

function dedupeModels(models: string[]): string[] {
  return Array.from(new Set(models.map((m) => m.trim()).filter(Boolean)));
}

function readLocalProviderApiKeyCache(): Record<string, string> {
  try {
    const raw = localStorage.getItem(LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    const normalized: Record<string, string> = {};
    Object.entries(parsed).forEach(([provider, value]) => {
      if (typeof value !== 'string') return;
      const trimmed = value.trim();
      if (!trimmed) return;
      normalized[normalizeProvider(provider)] = trimmed;
    });
    return normalized;
  } catch {
    return {};
  }
}

function writeLocalProviderApiKeyCache(nextValue: Record<string, string>): void {
  localStorage.setItem(LOCAL_PROVIDER_API_KEY_CACHE_STORAGE_KEY, JSON.stringify(nextValue));
}

function getLocalProviderApiKey(provider: string): string {
  const cache = readLocalProviderApiKeyCache();
  return cache[normalizeProvider(provider)] || '';
}

function setLocalProviderApiKey(provider: string, apiKey: string): void {
  const normalizedProvider = normalizeProvider(provider);
  const cache = readLocalProviderApiKeyCache();
  const trimmed = apiKey.trim();
  if (trimmed) {
    cache[normalizedProvider] = trimmed;
  } else {
    delete cache[normalizedProvider];
  }
  writeLocalProviderApiKeyCache(cache);
}

function getLocalProviderApiKeyStatuses(): ApiKeyStatus {
  const cache = readLocalProviderApiKeyCache();
  const statuses: ApiKeyStatus = {};
  Object.entries(cache).forEach(([provider, value]) => {
    if (value.trim()) {
      statuses[provider] = true;
    }
  });
  return statuses;
}

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

    // Preferred path: directly verify each provider by attempting to read stored key.
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
    } catch {
      // Fall through to compatibility path.
    }

    // Compatibility fallback for older backend versions.
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
    } catch (error) {
      console.error('Failed to fetch API key statuses:', error);
      // Final fallback: local cache.
      const stored = localStorage.getItem('plan-cascade-api-keys');
      if (stored) {
        const parsed = JSON.parse(stored) as ApiKeyStatus;
        const normalized: ApiKeyStatus = {};
        Object.entries(parsed).forEach(([provider, configured]) => {
          if (configured) {
            normalized[normalizeProvider(provider)] = true;
          }
        });
        setApiKeyStatuses({ ...normalized, ...localFallbackStatuses });
      } else {
        setApiKeyStatuses(localFallbackStatuses);
      }
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
