/**
 * NetworkSection Component
 *
 * Settings section for HTTP proxy configuration.
 * Supports global proxy, per-provider proxy strategy, and connectivity testing.
 */

import { useState, useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { CheckCircledIcon, CrossCircledIcon } from '@radix-ui/react-icons';
import { useProxyStore } from '../../store/proxy';
import type { ProxyConfig, ProxyProtocol, ProxyStrategy } from '../../lib/proxyApi';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const LLM_PROVIDERS = [
  { id: 'anthropic', defaultStrategy: 'use_global' as ProxyStrategy },
  { id: 'openai', defaultStrategy: 'use_global' as ProxyStrategy },
  { id: 'deepseek', defaultStrategy: 'no_proxy' as ProxyStrategy },
  { id: 'qwen', defaultStrategy: 'no_proxy' as ProxyStrategy },
  { id: 'glm', defaultStrategy: 'no_proxy' as ProxyStrategy },
  { id: 'minimax', defaultStrategy: 'no_proxy' as ProxyStrategy },
  { id: 'ollama', defaultStrategy: 'no_proxy' as ProxyStrategy },
];

const EMBEDDING_PROVIDERS = [
  { id: 'embedding_openai', defaultStrategy: 'use_global' as ProxyStrategy },
  { id: 'embedding_qwen', defaultStrategy: 'no_proxy' as ProxyStrategy },
  { id: 'embedding_glm', defaultStrategy: 'no_proxy' as ProxyStrategy },
  { id: 'embedding_ollama', defaultStrategy: 'no_proxy' as ProxyStrategy },
];

const WEBHOOK_PROVIDERS = [
  { id: 'webhook_slack', defaultStrategy: 'use_global' as ProxyStrategy },
  { id: 'webhook_feishu', defaultStrategy: 'no_proxy' as ProxyStrategy },
  { id: 'webhook_telegram', defaultStrategy: 'use_global' as ProxyStrategy },
  { id: 'webhook_discord', defaultStrategy: 'use_global' as ProxyStrategy },
  { id: 'webhook_custom', defaultStrategy: 'use_global' as ProxyStrategy },
];

const CLAUDE_CODE_PROVIDER = {
  id: 'claude_code',
  defaultStrategy: 'use_global' as ProxyStrategy,
};

const PROTOCOLS: ProxyProtocol[] = ['http', 'https', 'socks5'];

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function NetworkSection() {
  const { t } = useTranslation('settings');

  const {
    globalProxy,
    providerStrategies,
    loading,
    saving,
    testing,
    testResult,
    error,
    fetchProxyConfig,
    setGlobalProxy,
    setProviderStrategy,
    testProxyConnection,
    clearTestResult,
    clearError,
  } = useProxyStore();

  // Local form state for global proxy
  const [enabled, setEnabled] = useState(false);
  const [protocol, setProtocol] = useState<ProxyProtocol>('http');
  const [host, setHost] = useState('');
  const [port, setPort] = useState(8080);
  const [authEnabled, setAuthEnabled] = useState(false);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [statusMessage, setStatusMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  // Load config on mount
  useEffect(() => {
    fetchProxyConfig();
  }, [fetchProxyConfig]);

  // Sync global proxy to form state when loaded
  useEffect(() => {
    if (globalProxy) {
      setEnabled(true);
      setProtocol(globalProxy.protocol);
      setHost(globalProxy.host);
      setPort(globalProxy.port);
      if (globalProxy.username) {
        setAuthEnabled(true);
        setUsername(globalProxy.username);
      }
    } else {
      setEnabled(false);
    }
  }, [globalProxy]);

  // Auto-dismiss status message
  useEffect(() => {
    if (statusMessage) {
      const timer = setTimeout(() => setStatusMessage(null), 3000);
      return () => clearTimeout(timer);
    }
  }, [statusMessage]);

  const buildProxyConfig = useCallback(
    (): ProxyConfig => ({
      protocol,
      host,
      port,
      username: authEnabled && username ? username : undefined,
    }),
    [protocol, host, port, authEnabled, username],
  );

  const handleSaveGlobal = useCallback(async () => {
    if (!enabled) {
      const success = await setGlobalProxy(null);
      if (success) setStatusMessage({ type: 'success', text: t('network.globalProxy.saveSuccess') });
      else setStatusMessage({ type: 'error', text: t('network.globalProxy.saveError') });
      return;
    }
    if (!host.trim()) return;
    const proxy = buildProxyConfig();
    const success = await setGlobalProxy(proxy, authEnabled && password ? password : undefined);
    if (success) setStatusMessage({ type: 'success', text: t('network.globalProxy.saveSuccess') });
    else setStatusMessage({ type: 'error', text: t('network.globalProxy.saveError') });
  }, [enabled, host, buildProxyConfig, setGlobalProxy, authEnabled, password, t]);

  const handleTest = useCallback(() => {
    if (!host.trim()) return;
    const proxy = buildProxyConfig();
    testProxyConnection(proxy, authEnabled && password ? password : undefined);
  }, [host, buildProxyConfig, testProxyConnection, authEnabled, password]);

  const handleStrategyChange = useCallback(
    (provider: string, strategy: ProxyStrategy) => {
      setProviderStrategy(provider, strategy);
    },
    [setProviderStrategy],
  );

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <div className="animate-spin w-6 h-6 border-2 border-primary-500 border-t-transparent rounded-full" />
      </div>
    );
  }

  return (
    <div className="space-y-8">
      {/* Title */}
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white">{t('network.title')}</h2>
        <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">{t('network.description')}</p>
      </div>

      {/* Error banner */}
      {error && (
        <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-sm text-red-700 dark:text-red-300 flex items-center justify-between">
          <span>{error}</span>
          <button onClick={clearError} className="text-red-500 hover:text-red-700 ml-2">
            &times;
          </button>
        </div>
      )}

      {/* Status message */}
      {statusMessage && (
        <div
          className={clsx(
            'p-3 rounded-lg text-sm flex items-center gap-2',
            statusMessage.type === 'success'
              ? 'bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 text-green-700 dark:text-green-300'
              : 'bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300',
          )}
        >
          {statusMessage.type === 'success' ? <CheckCircledIcon /> : <CrossCircledIcon />}
          {statusMessage.text}
        </div>
      )}

      {/* ─── Global Proxy ─── */}
      <div className="border border-gray-200 dark:border-gray-700 rounded-lg p-4 space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-base font-medium text-gray-900 dark:text-white">{t('network.globalProxy.title')}</h3>
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
              className="w-4 h-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
            />
            <span className="text-sm text-gray-600 dark:text-gray-400">{t('network.globalProxy.enable')}</span>
          </label>
        </div>

        {enabled && (
          <div className="space-y-4">
            {/* Protocol + Host + Port */}
            <div className="grid grid-cols-[120px_1fr_100px] gap-3">
              <div>
                <label className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                  {t('network.globalProxy.protocol')}
                </label>
                <select
                  value={protocol}
                  onChange={(e) => setProtocol(e.target.value as ProxyProtocol)}
                  className={clsx(
                    'w-full px-3 py-2 rounded-lg border text-sm',
                    'border-gray-300 dark:border-gray-600',
                    'bg-white dark:bg-gray-800',
                    'text-gray-900 dark:text-white',
                    'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                  )}
                >
                  {PROTOCOLS.map((p) => (
                    <option key={p} value={p}>
                      {p.toUpperCase()}
                    </option>
                  ))}
                </select>
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                  {t('network.globalProxy.host')}
                </label>
                <input
                  type="text"
                  value={host}
                  onChange={(e) => setHost(e.target.value)}
                  placeholder={t('network.globalProxy.hostPlaceholder')}
                  className={clsx(
                    'w-full px-3 py-2 rounded-lg border text-sm',
                    'border-gray-300 dark:border-gray-600',
                    'bg-white dark:bg-gray-800',
                    'text-gray-900 dark:text-white',
                    'placeholder:text-gray-400',
                    'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                  )}
                />
              </div>
              <div>
                <label className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                  {t('network.globalProxy.port')}
                </label>
                <input
                  type="number"
                  value={port}
                  onChange={(e) => setPort(parseInt(e.target.value) || 0)}
                  min={1}
                  max={65535}
                  className={clsx(
                    'w-full px-3 py-2 rounded-lg border text-sm',
                    'border-gray-300 dark:border-gray-600',
                    'bg-white dark:bg-gray-800',
                    'text-gray-900 dark:text-white',
                    'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                  )}
                />
              </div>
            </div>

            {/* Auth */}
            <div className="space-y-3">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={authEnabled}
                  onChange={(e) => setAuthEnabled(e.target.checked)}
                  className="w-4 h-4 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                />
                <span className="text-sm text-gray-600 dark:text-gray-400">{t('network.globalProxy.auth')}</span>
              </label>

              {authEnabled && (
                <div className="grid grid-cols-2 gap-3 pl-6">
                  <div>
                    <label className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                      {t('network.globalProxy.username')}
                    </label>
                    <input
                      type="text"
                      value={username}
                      onChange={(e) => setUsername(e.target.value)}
                      placeholder={t('network.globalProxy.usernamePlaceholder')}
                      className={clsx(
                        'w-full px-3 py-2 rounded-lg border text-sm',
                        'border-gray-300 dark:border-gray-600',
                        'bg-white dark:bg-gray-800',
                        'text-gray-900 dark:text-white',
                        'placeholder:text-gray-400',
                        'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                      )}
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                      {t('network.globalProxy.password')}
                    </label>
                    <input
                      type="password"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      placeholder={t('network.globalProxy.passwordPlaceholder')}
                      className={clsx(
                        'w-full px-3 py-2 rounded-lg border text-sm',
                        'border-gray-300 dark:border-gray-600',
                        'bg-white dark:bg-gray-800',
                        'text-gray-900 dark:text-white',
                        'placeholder:text-gray-400',
                        'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                      )}
                    />
                  </div>
                </div>
              )}
            </div>

            {/* Actions */}
            <div className="flex items-center gap-3">
              <button
                onClick={handleTest}
                disabled={testing || !host.trim()}
                className={clsx(
                  'px-4 py-2 rounded-lg text-sm font-medium',
                  'border border-gray-300 dark:border-gray-600',
                  'bg-white dark:bg-gray-800',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-50 dark:hover:bg-gray-700',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                )}
              >
                {testing ? t('network.globalProxy.testing') : t('network.globalProxy.testConnection')}
              </button>
              <button
                onClick={handleSaveGlobal}
                disabled={saving || !host.trim()}
                className={clsx(
                  'px-4 py-2 rounded-lg text-sm font-medium',
                  'bg-primary-600 text-white',
                  'hover:bg-primary-700',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                )}
              >
                {saving ? t('network.globalProxy.saving') : t('network.globalProxy.save')}
              </button>
            </div>

            {/* Test result */}
            {testResult && (
              <div
                className={clsx(
                  'p-3 rounded-lg text-sm flex items-center gap-2',
                  testResult.success
                    ? 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300'
                    : 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300',
                )}
              >
                {testResult.success ? <CheckCircledIcon /> : <CrossCircledIcon />}
                <span>
                  {testResult.success
                    ? `${t('network.testResult.success')}${testResult.latency_ms ? ` — ${t('network.testResult.latency', { ms: testResult.latency_ms })}` : ''}`
                    : `${t('network.testResult.failed')}${testResult.error ? `: ${testResult.error}` : ''}`}
                </span>
                <button onClick={clearTestResult} className="ml-auto text-gray-400 hover:text-gray-600">
                  &times;
                </button>
              </div>
            )}
          </div>
        )}

        {!enabled && (
          <p className="text-sm text-gray-400 dark:text-gray-500 italic">{t('network.globalProxy.disabled')}</p>
        )}
      </div>

      {/* ─── Provider Proxy Settings ─── */}
      <div className="border border-gray-200 dark:border-gray-700 rounded-lg p-4 space-y-6">
        <h3 className="text-base font-medium text-gray-900 dark:text-white">{t('network.providerProxy.title')}</h3>

        {/* LLM Providers */}
        <div>
          <h4 className="text-sm font-medium text-gray-600 dark:text-gray-400 mb-3">
            {t('network.providerProxy.llmProviders')}
          </h4>
          <div className="space-y-2">
            {LLM_PROVIDERS.map(({ id }) => (
              <ProviderStrategyRow
                key={id}
                providerId={id}
                strategy={providerStrategies[id]}
                onStrategyChange={handleStrategyChange}
                t={t}
              />
            ))}
          </div>
        </div>

        {/* Embedding Providers */}
        <div>
          <h4 className="text-sm font-medium text-gray-600 dark:text-gray-400 mb-3">
            {t('network.providerProxy.embeddingProviders')}
          </h4>
          <div className="space-y-2">
            {EMBEDDING_PROVIDERS.map(({ id }) => (
              <ProviderStrategyRow
                key={id}
                providerId={id}
                strategy={providerStrategies[id]}
                onStrategyChange={handleStrategyChange}
                t={t}
              />
            ))}
            {/* TF-IDF - always local */}
            <div className="flex items-center justify-between py-2 px-3 rounded-lg bg-gray-50 dark:bg-gray-800/50">
              <span className="text-sm text-gray-600 dark:text-gray-400">{t('network.providers.embedding_tfidf')}</span>
              <span className="text-xs text-gray-400 dark:text-gray-500 italic">
                {t('network.providerProxy.localNote')}
              </span>
            </div>
          </div>
        </div>

        {/* Claude Code */}
        <div>
          <h4 className="text-sm font-medium text-gray-600 dark:text-gray-400 mb-3">
            {t('network.providerProxy.claudeCode')}
          </h4>
          <div className="space-y-2">
            <ProviderStrategyRow
              providerId={CLAUDE_CODE_PROVIDER.id}
              strategy={providerStrategies[CLAUDE_CODE_PROVIDER.id]}
              onStrategyChange={handleStrategyChange}
              t={t}
            />
          </div>
        </div>

        {/* Webhook Providers */}
        <div>
          <h4 className="text-sm font-medium text-gray-600 dark:text-gray-400 mb-3">
            {t('network.providerProxy.webhookProviders', 'Webhook Channels')}
          </h4>
          <div className="space-y-2">
            {WEBHOOK_PROVIDERS.map(({ id }) => (
              <ProviderStrategyRow
                key={id}
                providerId={id}
                strategy={providerStrategies[id]}
                onStrategyChange={handleStrategyChange}
                t={t}
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface ProviderStrategyRowProps {
  providerId: string;
  strategy?: ProxyStrategy;
  onStrategyChange: (provider: string, strategy: ProxyStrategy) => void;
  t: (key: string, options?: Record<string, unknown>) => string;
}

function ProviderStrategyRow({ providerId, strategy, onStrategyChange, t }: ProviderStrategyRowProps) {
  const currentStrategy = strategy ?? 'no_proxy';

  return (
    <div className="flex items-center justify-between py-2 px-3 rounded-lg bg-gray-50 dark:bg-gray-800/50">
      <span className="text-sm text-gray-700 dark:text-gray-300">{t(`network.providers.${providerId}`)}</span>
      <select
        value={currentStrategy}
        onChange={(e) => onStrategyChange(providerId, e.target.value as ProxyStrategy)}
        className={clsx(
          'px-3 py-1.5 rounded-lg border text-sm',
          'border-gray-300 dark:border-gray-600',
          'bg-white dark:bg-gray-800',
          'text-gray-700 dark:text-gray-300',
          'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
        )}
      >
        <option value="use_global">{t('network.providerProxy.useGlobal')}</option>
        <option value="no_proxy">{t('network.providerProxy.noProxy')}</option>
        <option value="custom">{t('network.providerProxy.custom')}</option>
      </select>
    </div>
  );
}

export default NetworkSection;
