/**
 * ModelSwitcher Component
 *
 * Compact dropdown for quick model/provider switching in the SimpleMode header.
 * Shows current provider/model as a badge; opens grouped dropdown on click.
 *
 * - Claude Code CLI: no model selection, just show "Claude Code"
 * - Standalone backends: grouped by provider, shows API key status
 * - Disabled while execution is running
 */

import { useState, useEffect, useRef, useCallback, useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useSettingsStore, type Backend } from '../../store/settings';
import { useExecutionStore } from '../../store/execution';
import {
  BACKEND_OPTIONS,
  FALLBACK_MODELS_BY_PROVIDER,
  DEFAULT_MODEL_BY_PROVIDER,
  normalizeProvider,
  getLocalProviderApiKeyStatuses,
  getCustomModelsByProvider,
  type BackendOption,
  type ApiKeyStatus,
} from '../../lib/providers';

// ============================================================================
// Types
// ============================================================================

interface ProviderGroup {
  option: BackendOption;
  displayName: string;
  models: string[];
  hasApiKey: boolean;
  requiresApiKey: boolean;
  isCli: boolean;
}

// ============================================================================
// Helper: determine transition type and apply switch
// ============================================================================

type TransitionKind = 'standalone_to_standalone' | 'standalone_to_cli' | 'cli_to_standalone' | 'cli_to_cli';

function classifyTransition(currentBackend: Backend, targetBackend: Backend): TransitionKind {
  const currentIsCli = currentBackend === 'claude-code';
  const targetIsCli = targetBackend === 'claude-code';
  if (currentIsCli && targetIsCli) return 'cli_to_cli';
  if (currentIsCli && !targetIsCli) return 'cli_to_standalone';
  if (!currentIsCli && targetIsCli) return 'standalone_to_cli';
  return 'standalone_to_standalone';
}

// ============================================================================
// Component
// ============================================================================

interface ModelSwitcherProps {
  dropdownDirection?: 'up' | 'down';
}

export function ModelSwitcher({ dropdownDirection = 'down' }: ModelSwitcherProps = {}) {
  const { t } = useTranslation('simpleMode');
  const { t: tSettings } = useTranslation('settings');

  // Settings store
  const backend = useSettingsStore((s) => s.backend);
  const provider = useSettingsStore((s) => s.provider);
  const model = useSettingsStore((s) => s.model);
  const setBackend = useSettingsStore((s) => s.setBackend);
  const setProvider = useSettingsStore((s) => s.setProvider);
  const setModel = useSettingsStore((s) => s.setModel);

  // Execution store
  const status = useExecutionStore((s) => s.status);
  const appendStreamLine = useExecutionStore((s) => s.appendStreamLine);

  // Dropdown state
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // API key statuses (refreshed on open)
  const [apiKeyStatuses, setApiKeyStatuses] = useState<ApiKeyStatus>(() => getLocalProviderApiKeyStatuses());

  const isRunning = status === 'running' || status === 'paused';

  // Refresh API key statuses when dropdown opens
  useEffect(() => {
    if (open) {
      setApiKeyStatuses(getLocalProviderApiKeyStatuses());
    }
  }, [open]);

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handleClick = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, [open]);

  // Close on Escape key
  useEffect(() => {
    if (!open) return;
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setOpen(false);
      }
    };
    document.addEventListener('keydown', handleKey);
    return () => document.removeEventListener('keydown', handleKey);
  }, [open]);

  // Build provider groups with models
  const providerGroups = useMemo((): ProviderGroup[] => {
    const customModels = getCustomModelsByProvider();
    return BACKEND_OPTIONS.map((option) => {
      const normalizedProvider = normalizeProvider(option.provider);
      const isCli = option.id === 'claude-code';
      const fallbackModels = FALLBACK_MODELS_BY_PROVIDER[normalizedProvider] || [];
      const custom = customModels[normalizedProvider] || [];
      const allModels = Array.from(new Set([...fallbackModels, ...custom]));

      return {
        option,
        displayName: tSettings(`llm.providers.${option.i18nKey}.name`),
        models: isCli ? [] : allModels,
        hasApiKey: !option.requiresApiKey || !!apiKeyStatuses[normalizedProvider],
        requiresApiKey: option.requiresApiKey,
        isCli,
      };
    });
  }, [apiKeyStatuses, tSettings]);

  // Current display label
  const currentLabel = useMemo(() => {
    if (backend === 'claude-code') {
      return 'Claude Code';
    }
    const normalizedProvider = normalizeProvider(provider);
    const option = BACKEND_OPTIONS.find((o) => o.id === backend);
    const providerName = option ? tSettings(`llm.providers.${option.i18nKey}.name`) : normalizedProvider;
    const modelName = model || DEFAULT_MODEL_BY_PROVIDER[normalizedProvider] || '';
    return modelName ? `${providerName} / ${modelName}` : providerName;
  }, [backend, provider, model, tSettings]);

  // Handle selecting a specific model within a provider
  const handleSelectModel = useCallback(
    (group: ProviderGroup, selectedModel: string) => {
      if (isRunning) return;

      // For providers requiring API key but none configured, warn and abort
      if (group.requiresApiKey && !group.hasApiKey) {
        appendStreamLine(`Cannot switch to ${group.displayName}: ${t('modelSwitcher.noApiKey')}.`, 'warning');
        setOpen(false);
        return;
      }

      const targetBackend = group.option.id;
      const transition = classifyTransition(backend, targetBackend);

      // Apply settings
      setBackend(targetBackend);
      setProvider(group.option.provider);
      setModel(selectedModel);

      // Handle transitions with informational messages
      switch (transition) {
        case 'standalone_to_standalone':
          appendStreamLine(
            `Switched to ${group.displayName} / ${selectedModel}. ${t('modelSwitcher.contextPreserved')}.`,
            'warning',
          );
          break;

        case 'standalone_to_cli':
          // Should not reach here (CLI has no models), but handle gracefully
          appendStreamLine(`Switched to Claude Code CLI. ${t('modelSwitcher.sessionRestart')}`, 'warning');
          break;

        case 'cli_to_standalone': {
          appendStreamLine(`Switched to ${group.displayName} / ${selectedModel}.`, 'warning');
          // Reset Claude Code session state
          useExecutionStore.setState({ taskId: null, isChatSession: false });
          break;
        }

        case 'cli_to_cli':
          // Should not reach here, but handle
          break;
      }

      setOpen(false);
    },
    [backend, isRunning, appendStreamLine, t, setBackend, setProvider, setModel],
  );

  // Handle selecting a provider (for Claude Code, no model)
  const handleSelectProvider = useCallback(
    (group: ProviderGroup) => {
      if (isRunning) return;

      // For providers requiring API key but none configured, warn and abort
      if (group.requiresApiKey && !group.hasApiKey) {
        appendStreamLine(`Cannot switch to ${group.displayName}: ${t('modelSwitcher.noApiKey')}.`, 'warning');
        setOpen(false);
        return;
      }

      const targetBackend = group.option.id;

      // Claude Code selected (no model)
      if (group.isCli) {
        const transition = classifyTransition(backend, targetBackend);
        setBackend(targetBackend);
        setProvider(group.option.provider);

        if (transition === 'standalone_to_cli') {
          appendStreamLine(`Switched to Claude Code CLI. ${t('modelSwitcher.sessionRestart')}`, 'warning');
        } else if (transition === 'cli_to_cli') {
          // no-op, already on Claude Code
        }

        setOpen(false);
        return;
      }

      // For non-CLI providers, select the default model
      const normalizedProv = normalizeProvider(group.option.provider);
      const defaultModel = DEFAULT_MODEL_BY_PROVIDER[normalizedProv] || group.models[0] || '';

      handleSelectModel(group, defaultModel);
    },
    [backend, isRunning, appendStreamLine, t, setBackend, setProvider, handleSelectModel],
  );

  // Check if a specific model in a provider is the currently selected one
  const isSelectedModel = useCallback(
    (optionId: Backend, modelId: string): boolean => {
      if (backend !== optionId) return false;
      if (!model && modelId === (DEFAULT_MODEL_BY_PROVIDER[normalizeProvider(provider)] || '')) {
        return true;
      }
      return model === modelId;
    },
    [backend, model, provider],
  );

  const isSelectedProvider = useCallback((optionId: Backend): boolean => backend === optionId, [backend]);

  return (
    <div ref={containerRef} className="relative">
      {/* Badge / trigger button */}
      <button
        onClick={() => {
          if (!isRunning) setOpen((v) => !v);
        }}
        disabled={isRunning}
        className={clsx(
          'flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-medium transition-colors max-w-[220px]',
          'border',
          isRunning
            ? 'opacity-50 cursor-not-allowed border-gray-300 dark:border-gray-700 text-gray-400 dark:text-gray-500'
            : open
              ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300'
              : 'border-gray-300 dark:border-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
        )}
        title={t('modelSwitcher.current')}
      >
        <svg className="w-3.5 h-3.5 shrink-0" fill="none" viewBox="0 0 24 24" strokeWidth={1.5} stroke="currentColor">
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M9.813 15.904 9 18.75l-.813-2.846a4.5 4.5 0 0 0-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 0 0 3.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 0 0 3.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 0 0-3.09 3.09ZM18.259 8.715 18 9.75l-.259-1.035a3.375 3.375 0 0 0-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 0 0 2.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 0 0 2.455 2.456L21.75 6l-1.036.259a3.375 3.375 0 0 0-2.455 2.456Z"
          />
        </svg>
        <span className="truncate">{currentLabel}</span>
        <svg
          className={clsx('w-3 h-3 shrink-0 transition-transform', open && 'rotate-180')}
          fill="none"
          viewBox="0 0 24 24"
          strokeWidth={2}
          stroke="currentColor"
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="m19.5 8.25-7.5 7.5-7.5-7.5" />
        </svg>
      </button>

      {/* Dropdown panel */}
      {open && (
        <div
          className={clsx(
            dropdownDirection === 'up' ? 'absolute bottom-full left-0 mb-1 z-50' : 'absolute top-full left-0 mt-1 z-50',
            'w-[320px] max-h-[420px] overflow-y-auto',
            'rounded-lg border border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-900',
            'shadow-lg',
            'py-1',
            'animate-in fade-in-0 zoom-in-95 duration-150',
          )}
        >
          {providerGroups.map((group, groupIdx) => (
            <div key={group.option.id}>
              {/* Divider between groups */}
              {groupIdx > 0 && <div className="mx-2 my-1 border-t border-gray-100 dark:border-gray-800" />}

              {/* Provider header */}
              {group.isCli ? (
                /* Claude Code: clickable provider row, no sub-models */
                <button
                  onClick={() => handleSelectProvider(group)}
                  className={clsx(
                    'w-full text-left px-3 py-2 flex items-center justify-between gap-2 transition-colors',
                    isSelectedProvider(group.option.id)
                      ? 'bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300'
                      : 'hover:bg-gray-50 dark:hover:bg-gray-800 text-gray-900 dark:text-white',
                  )}
                >
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium">{group.displayName}</span>
                    <span className="text-2xs text-gray-400 dark:text-gray-500">(local CLI)</span>
                  </div>
                  {isSelectedProvider(group.option.id) && (
                    <svg
                      className="w-4 h-4 text-primary-600 dark:text-primary-400 shrink-0"
                      fill="none"
                      viewBox="0 0 24 24"
                      strokeWidth={2}
                      stroke="currentColor"
                    >
                      <path strokeLinecap="round" strokeLinejoin="round" d="m4.5 12.75 6 6 9-13.5" />
                    </svg>
                  )}
                </button>
              ) : (
                /* Non-CLI provider header */
                <div className="px-3 pt-2 pb-1 flex items-center justify-between gap-2">
                  <span
                    className={clsx(
                      'text-xs font-semibold',
                      group.hasApiKey ? 'text-gray-700 dark:text-gray-300' : 'text-gray-400 dark:text-gray-600',
                    )}
                  >
                    {group.displayName}
                  </span>
                  {group.requiresApiKey && (
                    <span
                      className={clsx(
                        'text-2xs px-1.5 py-0.5 rounded-full',
                        group.hasApiKey
                          ? 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400'
                          : 'bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400',
                      )}
                    >
                      {group.hasApiKey ? 'API key \u2713' : t('modelSwitcher.noApiKey')}
                    </span>
                  )}
                </div>
              )}

              {/* Model list for non-CLI providers */}
              {!group.isCli && group.models.length > 0 && (
                <div className="py-0.5">
                  {group.models.map((modelId) => {
                    const selected = isSelectedModel(group.option.id, modelId);
                    const disabled = group.requiresApiKey && !group.hasApiKey;

                    return (
                      <button
                        key={`${group.option.id}-${modelId}`}
                        onClick={() => {
                          if (!disabled) handleSelectModel(group, modelId);
                        }}
                        disabled={disabled}
                        className={clsx(
                          'w-full text-left pl-6 pr-3 py-1.5 text-xs transition-colors flex items-center justify-between gap-2',
                          disabled
                            ? 'text-gray-400 dark:text-gray-600 cursor-not-allowed'
                            : selected
                              ? 'bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300'
                              : 'text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
                        )}
                      >
                        <span className="truncate font-mono">{modelId}</span>
                        {selected && (
                          <svg
                            className="w-3.5 h-3.5 text-primary-600 dark:text-primary-400 shrink-0"
                            fill="none"
                            viewBox="0 0 24 24"
                            strokeWidth={2}
                            stroke="currentColor"
                          >
                            <path strokeLinecap="round" strokeLinejoin="round" d="m4.5 12.75 6 6 9-13.5" />
                          </svg>
                        )}
                      </button>
                    );
                  })}
                </div>
              )}

              {/* Empty model list hint for non-CLI */}
              {!group.isCli && group.models.length === 0 && (
                <div className="pl-6 pr-3 py-1.5 text-2xs text-gray-400 dark:text-gray-600 italic">
                  No models available
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default ModelSwitcher;
