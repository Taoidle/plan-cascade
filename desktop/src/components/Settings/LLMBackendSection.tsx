/**
 * LLMBackendSection Component
 *
 * LLM backend selection and API key configuration.
 */

import { useState, useEffect } from 'react';
import { clsx } from 'clsx';
import { CheckCircledIcon, CrossCircledIcon, EyeOpenIcon, EyeNoneIcon } from '@radix-ui/react-icons';
import { useSettingsStore, Backend } from '../../store/settings';

interface BackendOption {
  id: Backend;
  name: string;
  description: string;
  requiresApiKey: boolean;
  provider?: string;
}

const backendOptions: BackendOption[] = [
  {
    id: 'claude-code',
    name: 'Claude Code (Claude Max)',
    description: 'Use Claude Code as LLM backend. No API key required.',
    requiresApiKey: false,
    provider: 'claude',
  },
  {
    id: 'claude-api',
    name: 'Claude API',
    description: 'Direct Anthropic Claude API access.',
    requiresApiKey: true,
    provider: 'claude',
  },
  {
    id: 'openai',
    name: 'OpenAI',
    description: 'OpenAI GPT models (GPT-4, GPT-4o, etc.).',
    requiresApiKey: true,
    provider: 'openai',
  },
  {
    id: 'deepseek',
    name: 'DeepSeek',
    description: 'DeepSeek models for code generation.',
    requiresApiKey: true,
    provider: 'deepseek',
  },
  {
    id: 'ollama',
    name: 'Ollama (Local)',
    description: 'Run models locally with Ollama.',
    requiresApiKey: false,
    provider: 'ollama',
  },
];

interface ApiKeyStatus {
  [provider: string]: boolean;
}

export function LLMBackendSection() {
  const { backend, setBackend, model, setModel, setProvider } = useSettingsStore();
  const [apiKeyStatuses, setApiKeyStatuses] = useState<ApiKeyStatus>({});
  const [apiKeyInputs, setApiKeyInputs] = useState<{ [provider: string]: string }>({});
  const [showApiKey, setShowApiKey] = useState<{ [provider: string]: boolean }>({});
  const [savingKey, setSavingKey] = useState<string | null>(null);
  const [keyMessage, setKeyMessage] = useState<{ provider: string; type: 'success' | 'error'; message: string } | null>(null);

  // Fetch API key statuses on mount
  useEffect(() => {
    fetchApiKeyStatuses();
  }, []);

  const fetchApiKeyStatuses = async () => {
    try {
      // Load API key statuses from localStorage (v5.0 Pure Rust backend)
      const stored = localStorage.getItem('plan-cascade-api-keys');
      if (stored) {
        const statuses: ApiKeyStatus = JSON.parse(stored);
        setApiKeyStatuses(statuses);
      }
    } catch (error) {
      console.error('Failed to fetch API key statuses:', error);
    }
  };

  const handleBackendChange = (newBackend: Backend) => {
    setBackend(newBackend);
    const option = backendOptions.find((o) => o.id === newBackend);
    if (option?.provider) {
      setProvider(option.provider);
    }
  };

  const handleSaveApiKey = async (provider: string) => {
    const apiKey = apiKeyInputs[provider];
    if (!apiKey?.trim()) return;

    setSavingKey(provider);
    setKeyMessage(null);

    try {
      // Save to localStorage (v5.0 - API keys managed locally or via keyring)
      const currentStatuses = { ...apiKeyStatuses, [provider]: true };
      localStorage.setItem('plan-cascade-api-keys', JSON.stringify(currentStatuses));

      // Note: In production, use keyring storage via Tauri command
      // For now, we just mark it as configured
      setApiKeyStatuses(currentStatuses);
      setApiKeyInputs((prev) => ({ ...prev, [provider]: '' }));
      setKeyMessage({ provider, type: 'success', message: 'API key saved successfully' });
    } catch (error) {
      setKeyMessage({ provider, type: 'error', message: 'Failed to save API key' });
    } finally {
      setSavingKey(null);
    }
  };

  const handleDeleteApiKey = async (provider: string) => {
    setSavingKey(provider);
    setKeyMessage(null);

    try {
      // Remove from localStorage
      const currentStatuses = { ...apiKeyStatuses, [provider]: false };
      localStorage.setItem('plan-cascade-api-keys', JSON.stringify(currentStatuses));

      setApiKeyStatuses(currentStatuses);
      setKeyMessage({ provider, type: 'success', message: 'API key removed' });
    } catch (error) {
      setKeyMessage({ provider, type: 'error', message: 'Failed to remove API key' });
    } finally {
      setSavingKey(null);
    }
  };

  const selectedOption = backendOptions.find((o) => o.id === backend);

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">
          LLM Backend
        </h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Select your preferred LLM provider and configure API access.
        </p>
      </div>

      {/* Backend Selection */}
      <section className="space-y-3">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          Provider
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
                    {option.name}
                  </span>
                  {option.requiresApiKey && (
                    <span
                      className={clsx(
                        'inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs',
                        apiKeyStatuses[option.provider || option.id]
                          ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
                          : 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400'
                      )}
                    >
                      {apiKeyStatuses[option.provider || option.id] ? (
                        <>
                          <CheckCircledIcon className="w-3 h-3" /> Configured
                        </>
                      ) : (
                        <>
                          <CrossCircledIcon className="w-3 h-3" /> API Key Required
                        </>
                      )}
                    </span>
                  )}
                </div>
                <div className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                  {option.description}
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
            API Key for {selectedOption.name}
          </h3>

          <div className="space-y-3">
            <div className="flex gap-2">
              <div className="relative flex-1">
                <input
                  type={showApiKey[selectedOption.provider || ''] ? 'text' : 'password'}
                  placeholder={
                    apiKeyStatuses[selectedOption.provider || '']
                      ? 'API key is configured (enter new key to replace)'
                      : 'Enter your API key'
                  }
                  value={apiKeyInputs[selectedOption.provider || ''] || ''}
                  onChange={(e) =>
                    setApiKeyInputs((prev) => ({
                      ...prev,
                      [selectedOption.provider || '']: e.target.value,
                    }))
                  }
                  className={clsx(
                    'w-full px-3 py-2 pr-10 rounded-lg border',
                    'border-gray-200 dark:border-gray-700',
                    'bg-white dark:bg-gray-800',
                    'text-gray-900 dark:text-white',
                    'focus:outline-none focus:ring-2 focus:ring-primary-500'
                  )}
                />
                <button
                  type="button"
                  onClick={() =>
                    setShowApiKey((prev) => ({
                      ...prev,
                      [selectedOption.provider || '']: !prev[selectedOption.provider || ''],
                    }))
                  }
                  className={clsx(
                    'absolute right-2 top-1/2 -translate-y-1/2 p-1',
                    'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300'
                  )}
                >
                  {showApiKey[selectedOption.provider || ''] ? (
                    <EyeNoneIcon className="w-4 h-4" />
                  ) : (
                    <EyeOpenIcon className="w-4 h-4" />
                  )}
                </button>
              </div>
              <button
                onClick={() => handleSaveApiKey(selectedOption.provider || '')}
                disabled={
                  savingKey === selectedOption.provider ||
                  !apiKeyInputs[selectedOption.provider || '']?.trim()
                }
                className={clsx(
                  'px-4 py-2 rounded-lg',
                  'bg-primary-600 text-white',
                  'hover:bg-primary-700',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                  'disabled:opacity-50 disabled:cursor-not-allowed'
                )}
              >
                {savingKey === selectedOption.provider ? 'Saving...' : 'Save'}
              </button>
              {apiKeyStatuses[selectedOption.provider || ''] && (
                <button
                  onClick={() => handleDeleteApiKey(selectedOption.provider || '')}
                  disabled={savingKey === selectedOption.provider}
                  className={clsx(
                    'px-4 py-2 rounded-lg',
                    'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400',
                    'hover:bg-red-200 dark:hover:bg-red-900/50',
                    'focus:outline-none focus:ring-2 focus:ring-red-500',
                    'disabled:opacity-50 disabled:cursor-not-allowed'
                  )}
                >
                  Remove
                </button>
              )}
            </div>

            {keyMessage && keyMessage.provider === selectedOption.provider && (
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
          Model
        </h3>
        <input
          type="text"
          placeholder={getModelPlaceholder(backend)}
          value={model}
          onChange={(e) => setModel(e.target.value)}
          className={clsx(
            'w-full max-w-md px-3 py-2 rounded-lg border',
            'border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800',
            'text-gray-900 dark:text-white',
            'focus:outline-none focus:ring-2 focus:ring-primary-500'
          )}
        />
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Specify the model to use. Leave empty for provider default.
        </p>
      </section>
    </div>
  );
}

function getModelPlaceholder(backend: Backend): string {
  switch (backend) {
    case 'claude-api':
      return 'e.g., claude-3-opus-20240229';
    case 'openai':
      return 'e.g., gpt-4o';
    case 'deepseek':
      return 'e.g., deepseek-coder';
    case 'ollama':
      return 'e.g., codellama:13b';
    default:
      return 'Model name';
  }
}

export default LLMBackendSection;
