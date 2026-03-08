import { useEffect, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { FALLBACK_MODELS_BY_PROVIDER } from '../../lib/providers';
import type { CommandResponse } from '../../lib/tauri';
import { useSettingsStore } from '../../store/settings';

const PROVIDER_DISPLAY_NAMES: Record<string, string> = {
  anthropic: 'Anthropic',
  openai: 'OpenAI',
  deepseek: 'DeepSeek',
  glm: 'GLM (Zhipu)',
  qwen: 'Qwen (Alibaba)',
  minimax: 'MiniMax',
  ollama: 'Ollama',
};

function ReviewAgentOptions({
  inheritLabel,
  configuredProviders,
}: {
  inheritLabel: string;
  configuredProviders: string[];
}) {
  return (
    <>
      <option value="">{inheritLabel}</option>
      {configuredProviders.map((provider) => {
        const models = FALLBACK_MODELS_BY_PROVIDER[provider];
        if (!models || models.length === 0) return null;
        return (
          <optgroup key={provider} label={PROVIDER_DISPLAY_NAMES[provider] ?? provider}>
            {models.map((model) => (
              <option key={`llm:${provider}:${model}`} value={`llm:${provider}:${model}`}>
                {model}
              </option>
            ))}
          </optgroup>
        );
      })}
    </>
  );
}

export function MemorySection() {
  const { t } = useTranslation('settings');
  const memorySettings = useSettingsStore((s) => s.memorySettings);
  const updateMemorySettings = useSettingsStore((s) => s.updateMemorySettings);
  const [configuredProviders, setConfiguredProviders] = useState<string[]>([]);

  useEffect(() => {
    let cancelled = false;
    async function fetchProviders() {
      const providers = new Set<string>(['ollama']);
      try {
        const result = await invoke<CommandResponse<string[]>>('list_configured_api_key_providers');
        if (result.success && result.data) {
          for (const provider of result.data) {
            providers.add(provider);
          }
        }
      } catch {
        // No-op: keep whatever options are available locally.
      }
      if (!cancelled) {
        setConfiguredProviders(Array.from(providers));
      }
    }
    void fetchProviders();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">{t('memory.title')}</h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('memory.description')}</p>
      </div>

      <section className="space-y-4">
        <div>
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('memory.extraction.title')}</h3>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">{t('memory.extraction.description')}</p>
        </div>

        <label
          className={clsx(
            'flex items-start gap-4 p-4 rounded-lg border cursor-pointer',
            'transition-colors border-gray-200 dark:border-gray-700',
            'hover:bg-gray-50 dark:hover:bg-gray-800',
          )}
        >
          <input
            type="checkbox"
            checked={memorySettings.autoExtractEnabled}
            onChange={(event) => updateMemorySettings({ autoExtractEnabled: event.target.checked })}
            className="mt-1 text-primary-600"
          />
          <div>
            <div className="font-medium text-gray-900 dark:text-white text-sm">
              {t('memory.extraction.autoExtract')}
            </div>
            <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
              {t('memory.extraction.autoExtractDescription')}
            </div>
          </div>
        </label>

        <div className="rounded-lg border border-gray-200 dark:border-gray-700 px-4 py-3">
          <div className="text-sm font-medium text-gray-900 dark:text-white">{t('memory.extraction.successOnly')}</div>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
            {t('memory.extraction.successOnlyDescription')}
          </p>
        </div>
      </section>

      <section className="space-y-4">
        <div>
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('memory.review.title')}</h3>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">{t('memory.review.description')}</p>
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium text-gray-900 dark:text-white" htmlFor="memory-review-mode">
            {t('memory.review.modeLabel')}
          </label>
          <select
            id="memory-review-mode"
            value={memorySettings.reviewMode}
            onChange={(event) =>
              updateMemorySettings({
                reviewMode: event.target.value as 'llm_review' | 'auto_approve' | 'manual_only',
              })
            }
            className={clsx(
              'w-full max-w-md px-3 py-2 rounded-lg border',
              'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary-500',
            )}
          >
            <option value="llm_review">{t('memory.review.modes.llm_review')}</option>
            <option value="auto_approve">{t('memory.review.modes.auto_approve')}</option>
            <option value="manual_only">{t('memory.review.modes.manual_only')}</option>
          </select>
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('memory.review.modeDescription')}</p>
        </div>

        <div className="space-y-2">
          <label className="text-sm font-medium text-gray-900 dark:text-white" htmlFor="memory-review-agent">
            {t('memory.review.agentLabel')}
          </label>
          <select
            id="memory-review-agent"
            value={memorySettings.reviewAgentRef}
            onChange={(event) => updateMemorySettings({ reviewAgentRef: event.target.value })}
            disabled={memorySettings.reviewMode !== 'llm_review'}
            className={clsx(
              'w-full max-w-md px-3 py-2 rounded-lg border',
              'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white focus:outline-none focus:ring-2 focus:ring-primary-500',
              memorySettings.reviewMode !== 'llm_review' && 'opacity-60 cursor-not-allowed',
            )}
          >
            <ReviewAgentOptions
              inheritLabel={t('memory.review.inheritGlobal')}
              configuredProviders={configuredProviders}
            />
          </select>
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('memory.review.agentDescription')}</p>
        </div>
      </section>

      <section className="space-y-4">
        <div>
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('memory.injection.title')}</h3>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">{t('memory.injection.description')}</p>
        </div>
        <div className="rounded-lg border border-gray-200 dark:border-gray-700 px-4 py-3">
          <div className="text-sm font-medium text-gray-900 dark:text-white">{t('memory.injection.activeOnly')}</div>
          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">{t('memory.injection.activeOnlyDescription')}</p>
        </div>
      </section>
    </div>
  );
}

export default MemorySection;
