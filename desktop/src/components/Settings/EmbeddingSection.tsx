/**
 * EmbeddingSection Component
 *
 * Settings section for embedding provider selection and configuration.
 * Supports provider dropdown, model/dimension/batch-size fields, base URL,
 * fallback provider, API key management for cloud providers, and health check.
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  CheckCircledIcon,
  CrossCircledIcon,
  EyeOpenIcon,
  EyeNoneIcon,
  ReloadIcon,
  InfoCircledIcon,
  Cross2Icon,
  LockClosedIcon,
  PlusIcon,
} from '@radix-ui/react-icons';
import { useEmbeddingStore } from '../../store/embedding';
import type { EmbeddingProviderType, EmbeddingProviderCapability } from '../../types/embedding';
import { EMBEDDING_KEYRING_ALIASES, CLOUD_EMBEDDING_PROVIDERS } from '../../types/embedding';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Resolve a provider string to its typed form. */
function asProviderType(provider: string): EmbeddingProviderType {
  return provider as EmbeddingProviderType;
}

/** Check whether the provider is a cloud provider that needs an API key. */
function isCloudProvider(provider: string): boolean {
  return CLOUD_EMBEDDING_PROVIDERS.includes(asProviderType(provider));
}

/** Get the keyring alias for a cloud embedding provider. */
function getKeyringAlias(provider: string): string | undefined {
  return EMBEDDING_KEYRING_ALIASES[asProviderType(provider)];
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function EmbeddingSection() {
  const { t } = useTranslation('settings');

  const {
    provider,
    model,
    baseUrl,
    dimension,
    batchSize,
    fallbackProvider,
    providers,
    builtinExcludedDirs,
    builtinExcludedExtensions,
    extraExcludedDirs,
    extraExcludedExtensions,
    exclusionsLoading,
    loading,
    saving,
    healthChecking,
    healthResult,
    error,
    reindexRequired,
    fetchConfig,
    fetchProviders,
    fetchIndexConfig,
    setProvider,
    setModel,
    setBaseUrl,
    setDimension,
    setBatchSize,
    setFallbackProvider,
    saveConfig,
    checkHealth,
    saveApiKey,
    loadApiKey,
    addExcludedDir,
    removeExcludedDir,
    addExcludedExtension,
    removeExcludedExtension,
    saveIndexConfig,
    clearError,
    clearHealthResult,
    clearReindexRequired,
  } = useEmbeddingStore();

  // API key management (local UI state only)
  const [apiKeyInput, setApiKeyInput] = useState('');
  const [showApiKey, setShowApiKey] = useState(false);
  const [savingApiKey, setSavingApiKey] = useState(false);
  const [loadingKey, setLoadingKey] = useState(false);
  const [apiKeyMessage, setApiKeyMessage] = useState<{
    type: 'success' | 'error';
    message: string;
  } | null>(null);

  // Custom model input (when "custom" is selected from the dropdown)
  const [customModelInput, setCustomModelInput] = useState('');

  // Index exclusion inputs
  const [newDirInput, setNewDirInput] = useState('');
  const [newExtInput, setNewExtInput] = useState('');

  // Load config and providers on mount
  useEffect(() => {
    void fetchConfig();
    void fetchProviders();
    void fetchIndexConfig();
  }, [fetchConfig, fetchProviders, fetchIndexConfig]);

  // Clear API key input when provider changes
  useEffect(() => {
    setApiKeyInput('');
    setShowApiKey(false);
    setApiKeyMessage(null);
  }, [provider]);

  // Current provider capability
  const currentCapability: EmbeddingProviderCapability | undefined = providers.find(
    (p) => p.provider_type === provider,
  );

  // Model presets for this provider
  const modelPresets = currentCapability?.models;
  const hasPresets = modelPresets && modelPresets.length > 0;

  // Whether the current model matches a preset
  const isPresetModel = hasPresets && modelPresets.some((m) => m.model_id === model);
  const isCustomModel = hasPresets && !isPresetModel;

  // The current model's preset (if any)
  const currentPreset = hasPresets ? modelPresets.find((m) => m.model_id === model) : undefined;

  // Available dimensions for dimension selector — model-specific or provider-level
  const supportedDimensions = useMemo(() => {
    if (currentPreset?.supported_dimensions) {
      return currentPreset.supported_dimensions;
    }
    return currentCapability?.supported_dimensions;
  }, [currentPreset, currentCapability]);

  // Fallback provider options (exclude current provider)
  const fallbackOptions = providers.filter((p) => p.provider_type !== provider);

  // Handle save
  const handleSave = useCallback(async () => {
    clearError();
    clearHealthResult();
    await saveConfig();
    await saveIndexConfig();
    // On failure, the store sets `error` directly — no additional handling needed.
  }, [saveConfig, saveIndexConfig, clearError, clearHealthResult]);

  // Handle health check
  const handleHealthCheck = useCallback(async () => {
    clearHealthResult();
    await checkHealth();
  }, [checkHealth, clearHealthResult]);

  // Handle API key save
  const handleSaveApiKey = useCallback(async () => {
    if (!apiKeyInput.trim()) return;
    const alias = getKeyringAlias(provider);
    if (!alias) return;

    setSavingApiKey(true);
    setApiKeyMessage(null);
    const success = await saveApiKey(alias, apiKeyInput.trim());
    setSavingApiKey(false);

    if (success) {
      setApiKeyMessage({
        type: 'success',
        message: t('embedding.apiKey.saveSuccess'),
      });
    } else {
      setApiKeyMessage({
        type: 'error',
        message: t('embedding.apiKey.saveError'),
      });
    }
  }, [apiKeyInput, provider, saveApiKey, t]);

  if (loading && providers.length === 0) {
    return (
      <div className="flex items-center justify-center py-12">
        <ReloadIcon className="w-5 h-5 animate-spin text-gray-400" />
        <span className="ml-2 text-sm text-gray-500 dark:text-gray-400">{t('embedding.loading')}</span>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      {/* Section header */}
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">{t('embedding.title')}</h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('embedding.description')}</p>
      </div>

      {/* Provider Selection */}
      <section className="space-y-3">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('embedding.provider.label')}</h3>
        <div className="space-y-2">
          {providers.map((cap) => (
            <label
              key={cap.provider_type}
              className={clsx(
                'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
                'transition-colors',
                provider === cap.provider_type
                  ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                  : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800',
              )}
            >
              <input
                type="radio"
                name="embeddingProvider"
                value={cap.provider_type}
                checked={provider === cap.provider_type}
                onChange={() => setProvider(cap.provider_type)}
                className="mt-1 text-primary-600"
              />
              <div className="flex-1">
                <div className="flex items-center gap-2">
                  <span className="font-medium text-gray-900 dark:text-white">{cap.display_name}</span>
                  {cap.is_local ? (
                    <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400">
                      {t('embedding.provider.local')}
                    </span>
                  ) : (
                    <span className="inline-flex items-center px-2 py-0.5 rounded-full text-xs bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400">
                      {t('embedding.provider.cloud')}
                    </span>
                  )}
                </div>
                <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                  {t(`embedding.providers.${cap.provider_type}.description`)}
                </div>
                <div className="text-xs text-gray-400 dark:text-gray-500 mt-1">
                  {t('embedding.provider.defaultModel')}: {cap.default_model}
                  {cap.default_dimension > 0 && ` | ${t('embedding.provider.dimension')}: ${cap.default_dimension}`}
                </div>
              </div>
            </label>
          ))}
        </div>
      </section>

      {/* API Key (only for cloud providers) */}
      {isCloudProvider(provider) && (
        <section className="space-y-4">
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">
            {t('embedding.apiKey.title', { name: currentCapability?.display_name ?? provider })}
          </h3>
          <div className="flex gap-2">
            <div className="relative flex-1">
              <input
                type={showApiKey ? 'text' : 'password'}
                placeholder={t('embedding.apiKey.placeholder')}
                value={apiKeyInput}
                onChange={(e) => setApiKeyInput(e.target.value)}
                className={clsx(
                  'w-full px-3 py-2 pr-10 rounded-lg border',
                  'border-gray-200 dark:border-gray-700',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
              />
              <button
                type="button"
                onClick={async () => {
                  const nextVisible = !showApiKey;
                  setShowApiKey(nextVisible);
                  if (!nextVisible) return;
                  // If the input is empty and we're toggling to visible,
                  // try to load the saved key from the keyring.
                  if (apiKeyInput.trim().length > 0) return;
                  const alias = getKeyringAlias(provider);
                  if (!alias) return;
                  setLoadingKey(true);
                  const key = await loadApiKey(alias);
                  setLoadingKey(false);
                  if (key) {
                    setApiKeyInput(key);
                  }
                }}
                disabled={loadingKey}
                className={clsx(
                  'absolute right-2 top-1/2 -translate-y-1/2 p-1',
                  'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                )}
              >
                {loadingKey ? (
                  <ReloadIcon className="w-4 h-4 animate-spin" />
                ) : showApiKey ? (
                  <EyeNoneIcon className="w-4 h-4" />
                ) : (
                  <EyeOpenIcon className="w-4 h-4" />
                )}
              </button>
            </div>
            <button
              onClick={handleSaveApiKey}
              disabled={savingApiKey || !apiKeyInput.trim()}
              className={clsx(
                'px-4 py-2 rounded-lg',
                'bg-primary-600 text-white',
                'hover:bg-primary-700',
                'focus:outline-none focus:ring-2 focus:ring-primary-500',
                'disabled:opacity-50 disabled:cursor-not-allowed',
              )}
            >
              {savingApiKey ? t('embedding.apiKey.saving') : t('embedding.apiKey.save')}
            </button>
          </div>
          {apiKeyMessage && (
            <p
              className={clsx(
                'text-sm',
                apiKeyMessage.type === 'success'
                  ? 'text-green-600 dark:text-green-400'
                  : 'text-red-600 dark:text-red-400',
              )}
            >
              {apiKeyMessage.message}
            </p>
          )}
          <p className="text-xs text-gray-400 dark:text-gray-500">
            {t('embedding.apiKey.help', { alias: getKeyringAlias(provider) })}
          </p>
        </section>
      )}

      {/* Model */}
      <section className="space-y-3">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('embedding.model.label')}</h3>
        {hasPresets ? (
          <div className="space-y-2">
            <select
              value={isCustomModel ? '__custom__' : model}
              onChange={(e) => {
                const val = e.target.value;
                if (val === '__custom__') {
                  setCustomModelInput('');
                  setModel('');
                } else {
                  setModel(val);
                }
              }}
              className={clsx(
                'w-full max-w-md px-3 py-2 rounded-lg border',
                'border-gray-200 dark:border-gray-700',
                'bg-white dark:bg-gray-800',
                'text-gray-900 dark:text-white',
                'focus:outline-none focus:ring-2 focus:ring-primary-500',
              )}
            >
              {modelPresets.map((preset) => (
                <option key={preset.model_id} value={preset.model_id}>
                  {preset.display_name} ({preset.model_id})
                </option>
              ))}
              <option value="__custom__">{t('embedding.model.customOption')}</option>
            </select>
            {isCustomModel && (
              <input
                type="text"
                value={customModelInput || model}
                onChange={(e) => {
                  setCustomModelInput(e.target.value);
                  setModel(e.target.value);
                }}
                placeholder={t('embedding.model.customPlaceholder')}
                className={clsx(
                  'w-full max-w-md px-3 py-2 rounded-lg border',
                  'border-gray-200 dark:border-gray-700',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
              />
            )}
          </div>
        ) : (
          <input
            type="text"
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder={currentCapability?.default_model ?? ''}
            className={clsx(
              'w-full max-w-md px-3 py-2 rounded-lg border',
              'border-gray-200 dark:border-gray-700',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
            )}
          />
        )}
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('embedding.model.help')}</p>
      </section>

      {/* Base URL (most useful for Ollama, but available for all) */}
      <section className="space-y-3">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('embedding.baseUrl.label')}</h3>
        <input
          type="text"
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
          placeholder={provider === 'ollama' ? 'http://localhost:11434' : t('embedding.baseUrl.placeholder')}
          className={clsx(
            'w-full max-w-md px-3 py-2 rounded-lg border',
            'border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800',
            'text-gray-900 dark:text-white',
            'focus:outline-none focus:ring-2 focus:ring-primary-500',
          )}
        />
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('embedding.baseUrl.help')}</p>
      </section>

      {/* Dimension & Batch Size */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('embedding.advanced.label')}</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {/* Dimension */}
          <div>
            <label className="block text-sm text-gray-600 dark:text-gray-400 mb-1">
              {t('embedding.dimension.label')}
            </label>
            {supportedDimensions && supportedDimensions.length > 0 ? (
              <select
                value={String(dimension)}
                onChange={(e) => setDimension(Number(e.target.value))}
                className={clsx(
                  'w-full px-3 py-2 rounded-lg border',
                  'border-gray-200 dark:border-gray-700',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
              >
                {supportedDimensions.map((dim) => (
                  <option key={dim} value={String(dim)}>
                    {dim}
                    {dim === currentCapability?.default_dimension ? ` (${t('embedding.dimension.default')})` : ''}
                  </option>
                ))}
              </select>
            ) : (
              <input
                type="number"
                min={0}
                value={dimension}
                onChange={(e) => {
                  const val = parseInt(e.target.value, 10);
                  if (!isNaN(val)) setDimension(val);
                }}
                className={clsx(
                  'w-full px-3 py-2 rounded-lg border',
                  'border-gray-200 dark:border-gray-700',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
              />
            )}
            <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">{t('embedding.dimension.help')}</p>
          </div>

          {/* Batch Size */}
          <div>
            <label className="block text-sm text-gray-600 dark:text-gray-400 mb-1">
              {t('embedding.batchSize.label')}
            </label>
            <input
              type="number"
              min={1}
              max={currentCapability?.max_batch_size ?? 2048}
              value={batchSize}
              onChange={(e) => {
                const val = parseInt(e.target.value, 10);
                if (!isNaN(val)) setBatchSize(val);
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
              {t('embedding.batchSize.help', { max: currentCapability?.max_batch_size ?? '?' })}
            </p>
          </div>
        </div>
      </section>

      {/* Fallback Provider */}
      <section className="space-y-3">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('embedding.fallback.label')}</h3>
        <select
          value={fallbackProvider}
          onChange={(e) => setFallbackProvider(e.target.value)}
          className={clsx(
            'w-full max-w-md px-3 py-2 rounded-lg border',
            'border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800',
            'text-gray-900 dark:text-white',
            'focus:outline-none focus:ring-2 focus:ring-primary-500',
          )}
        >
          <option value="">{t('embedding.fallback.none')}</option>
          {fallbackOptions.map((cap) => (
            <option key={cap.provider_type} value={cap.provider_type}>
              {cap.display_name}
            </option>
          ))}
        </select>
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('embedding.fallback.help')}</p>
      </section>

      {/* Index Exclusions */}
      <section className="space-y-4">
        <div>
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('embedding.indexExclusions.title')}</h3>
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">{t('embedding.indexExclusions.description')}</p>
        </div>

        {exclusionsLoading ? (
          <div className="flex items-center gap-2 py-4">
            <ReloadIcon className="w-4 h-4 animate-spin text-gray-400" />
            <span className="text-sm text-gray-500 dark:text-gray-400">{t('embedding.loading')}</span>
          </div>
        ) : (
          <>
            {/* Excluded Directories */}
            <div className="space-y-2">
              <h4 className="text-xs font-medium text-gray-700 dark:text-gray-300">
                {t('embedding.indexExclusions.dirs.title')}
              </h4>

              {/* Built-in dirs */}
              {builtinExcludedDirs.length > 0 && (
                <div>
                  <p className="text-xs text-gray-400 dark:text-gray-500 mb-1">
                    {t('embedding.indexExclusions.dirs.builtin')}
                  </p>
                  <div className="flex flex-wrap gap-1.5">
                    {builtinExcludedDirs.map((dir) => (
                      <span
                        key={dir}
                        className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-500"
                      >
                        <LockClosedIcon className="w-3 h-3" />
                        {dir}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {/* Custom dirs */}
              <div>
                <p className="text-xs text-gray-400 dark:text-gray-500 mb-1">
                  {t('embedding.indexExclusions.dirs.custom')}
                </p>
                <div className="flex flex-wrap gap-1.5 mb-2">
                  {extraExcludedDirs.map((dir) => (
                    <span
                      key={dir}
                      className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-400"
                    >
                      {dir}
                      <button
                        type="button"
                        onClick={() => removeExcludedDir(dir)}
                        className="hover:text-red-500 dark:hover:text-red-400"
                      >
                        <Cross2Icon className="w-3 h-3" />
                      </button>
                    </span>
                  ))}
                  {extraExcludedDirs.length === 0 && (
                    <span className="text-xs text-gray-400 dark:text-gray-500 italic">—</span>
                  )}
                </div>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={newDirInput}
                    onChange={(e) => setNewDirInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && newDirInput.trim()) {
                        addExcludedDir(newDirInput);
                        setNewDirInput('');
                      }
                    }}
                    placeholder={t('embedding.indexExclusions.dirs.placeholder')}
                    className={clsx(
                      'flex-1 max-w-xs px-3 py-1.5 rounded-lg border text-sm',
                      'border-gray-200 dark:border-gray-700',
                      'bg-white dark:bg-gray-800',
                      'text-gray-900 dark:text-white',
                      'focus:outline-none focus:ring-2 focus:ring-primary-500',
                    )}
                  />
                  <button
                    type="button"
                    onClick={() => {
                      if (newDirInput.trim()) {
                        addExcludedDir(newDirInput);
                        setNewDirInput('');
                      }
                    }}
                    disabled={!newDirInput.trim()}
                    className={clsx(
                      'inline-flex items-center gap-1 px-3 py-1.5 rounded-lg text-sm',
                      'bg-gray-100 dark:bg-gray-800',
                      'text-gray-700 dark:text-gray-300',
                      'hover:bg-gray-200 dark:hover:bg-gray-700',
                      'disabled:opacity-50 disabled:cursor-not-allowed',
                    )}
                  >
                    <PlusIcon className="w-3.5 h-3.5" />
                    {t('embedding.indexExclusions.dirs.add')}
                  </button>
                </div>
              </div>
            </div>

            {/* Excluded Extensions */}
            <div className="space-y-2">
              <h4 className="text-xs font-medium text-gray-700 dark:text-gray-300">
                {t('embedding.indexExclusions.extensions.title')}
              </h4>

              {/* Built-in extensions */}
              {builtinExcludedExtensions.length > 0 && (
                <div>
                  <p className="text-xs text-gray-400 dark:text-gray-500 mb-1">
                    {t('embedding.indexExclusions.extensions.builtin')}
                  </p>
                  <div className="flex flex-wrap gap-1.5">
                    {builtinExcludedExtensions.map((ext) => (
                      <span
                        key={ext}
                        className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-gray-100 text-gray-500 dark:bg-gray-800 dark:text-gray-500"
                      >
                        <LockClosedIcon className="w-3 h-3" />.{ext}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {/* Custom extensions */}
              <div>
                <p className="text-xs text-gray-400 dark:text-gray-500 mb-1">
                  {t('embedding.indexExclusions.extensions.custom')}
                </p>
                <div className="flex flex-wrap gap-1.5 mb-2">
                  {extraExcludedExtensions.map((ext) => (
                    <span
                      key={ext}
                      className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-400"
                    >
                      .{ext}
                      <button
                        type="button"
                        onClick={() => removeExcludedExtension(ext)}
                        className="hover:text-red-500 dark:hover:text-red-400"
                      >
                        <Cross2Icon className="w-3 h-3" />
                      </button>
                    </span>
                  ))}
                  {extraExcludedExtensions.length === 0 && (
                    <span className="text-xs text-gray-400 dark:text-gray-500 italic">—</span>
                  )}
                </div>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={newExtInput}
                    onChange={(e) => setNewExtInput(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter' && newExtInput.trim()) {
                        addExcludedExtension(newExtInput);
                        setNewExtInput('');
                      }
                    }}
                    placeholder={t('embedding.indexExclusions.extensions.placeholder')}
                    className={clsx(
                      'flex-1 max-w-xs px-3 py-1.5 rounded-lg border text-sm',
                      'border-gray-200 dark:border-gray-700',
                      'bg-white dark:bg-gray-800',
                      'text-gray-900 dark:text-white',
                      'focus:outline-none focus:ring-2 focus:ring-primary-500',
                    )}
                  />
                  <button
                    type="button"
                    onClick={() => {
                      if (newExtInput.trim()) {
                        addExcludedExtension(newExtInput);
                        setNewExtInput('');
                      }
                    }}
                    disabled={!newExtInput.trim()}
                    className={clsx(
                      'inline-flex items-center gap-1 px-3 py-1.5 rounded-lg text-sm',
                      'bg-gray-100 dark:bg-gray-800',
                      'text-gray-700 dark:text-gray-300',
                      'hover:bg-gray-200 dark:hover:bg-gray-700',
                      'disabled:opacity-50 disabled:cursor-not-allowed',
                    )}
                  >
                    <PlusIcon className="w-3.5 h-3.5" />
                    {t('embedding.indexExclusions.extensions.add')}
                  </button>
                </div>
              </div>
            </div>
          </>
        )}
      </section>

      {/* Health Check */}
      <section className="space-y-3">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('embedding.health.label')}</h3>
        <button
          onClick={handleHealthCheck}
          disabled={healthChecking}
          className={clsx(
            'inline-flex items-center gap-2 px-4 py-2 rounded-lg',
            'bg-gray-100 dark:bg-gray-800',
            'text-gray-700 dark:text-gray-300',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'focus:outline-none focus:ring-2 focus:ring-primary-500',
            'disabled:opacity-50 disabled:cursor-not-allowed',
          )}
        >
          <ReloadIcon className={clsx('w-4 h-4', healthChecking && 'animate-spin')} />
          {healthChecking ? t('embedding.health.checking') : t('embedding.health.checkButton')}
        </button>
        {healthResult && (
          <div
            className={clsx(
              'flex items-start gap-2 p-3 rounded-lg border',
              healthResult.healthy
                ? 'border-green-200 bg-green-50 dark:border-green-800 dark:bg-green-900/20'
                : 'border-red-200 bg-red-50 dark:border-red-800 dark:bg-red-900/20',
            )}
          >
            {healthResult.healthy ? (
              <CheckCircledIcon className="w-5 h-5 text-green-600 dark:text-green-400 shrink-0 mt-0.5" />
            ) : (
              <CrossCircledIcon className="w-5 h-5 text-red-600 dark:text-red-400 shrink-0 mt-0.5" />
            )}
            <div>
              <p
                className={clsx(
                  'text-sm font-medium',
                  healthResult.healthy ? 'text-green-700 dark:text-green-300' : 'text-red-700 dark:text-red-300',
                )}
              >
                {healthResult.healthy ? t('embedding.health.healthy') : t('embedding.health.unhealthy')}
              </p>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{healthResult.message}</p>
              {healthResult.latency_ms != null && (
                <p className="text-xs text-gray-400 dark:text-gray-500 mt-0.5">
                  {t('embedding.health.latency', { ms: healthResult.latency_ms })}
                </p>
              )}
            </div>
          </div>
        )}
      </section>

      {/* Error display */}
      {error && (
        <div className="flex items-start gap-2 p-3 rounded-lg border border-red-200 bg-red-50 dark:border-red-800 dark:bg-red-900/20">
          <CrossCircledIcon className="w-5 h-5 text-red-600 dark:text-red-400 shrink-0 mt-0.5" />
          <div>
            <p className="text-sm text-red-700 dark:text-red-300">{error}</p>
            <button
              onClick={clearError}
              className="text-xs text-red-500 hover:text-red-700 dark:text-red-400 dark:hover:text-red-300 mt-1 underline"
            >
              {t('embedding.dismiss')}
            </button>
          </div>
        </div>
      )}

      {/* Reindex Required Notice */}
      {reindexRequired && (
        <div className="flex items-start gap-2 p-3 rounded-lg border border-amber-200 bg-amber-50 dark:border-amber-800 dark:bg-amber-900/20">
          <InfoCircledIcon className="w-5 h-5 text-amber-600 dark:text-amber-400 shrink-0 mt-0.5" />
          <div>
            <p className="text-sm font-medium text-amber-700 dark:text-amber-300">{t('embedding.reindex.title')}</p>
            <p className="text-xs text-amber-600 dark:text-amber-400 mt-0.5">{t('embedding.reindex.message')}</p>
            <button
              onClick={clearReindexRequired}
              className="text-xs text-amber-500 hover:text-amber-700 dark:text-amber-400 dark:hover:text-amber-300 mt-1 underline"
            >
              {t('embedding.dismiss')}
            </button>
          </div>
        </div>
      )}

      {/* Save Button */}
      <section>
        <button
          onClick={handleSave}
          disabled={saving}
          className={clsx(
            'px-6 py-2.5 rounded-lg',
            'bg-primary-600 text-white font-medium',
            'hover:bg-primary-700',
            'focus:outline-none focus:ring-2 focus:ring-primary-500',
            'disabled:opacity-50 disabled:cursor-not-allowed',
          )}
        >
          {saving ? t('embedding.save.saving') : t('embedding.save.button')}
        </button>
      </section>
    </div>
  );
}

export default EmbeddingSection;
