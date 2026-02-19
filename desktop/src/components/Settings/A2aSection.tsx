/**
 * A2A Remote Agents Section
 *
 * Settings section for managing remote A2A (Agent-to-Agent) agents.
 * Allows users to discover, register, and remove remote agents by URL.
 */

import { useState, useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  discoverA2aAgent,
  listA2aAgents,
  registerA2aAgent,
  removeA2aAgent,
} from '../../lib/a2aApi';
import type { RegisteredRemoteAgent, DiscoveredAgent } from '../../lib/a2aApi';

export function A2aSection() {
  const { t } = useTranslation('settings');

  const [agents, setAgents] = useState<RegisteredRemoteAgent[]>([]);
  const [url, setUrl] = useState('');
  const [discovering, setDiscovering] = useState(false);
  const [registering, setRegistering] = useState(false);
  const [discovered, setDiscovered] = useState<DiscoveredAgent | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchAgents = useCallback(async () => {
    try {
      const list = await listA2aAgents();
      setAgents(list);
    } catch (err) {
      console.error('Failed to load remote agents:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchAgents();
  }, [fetchAgents]);

  const handleDiscover = async () => {
    if (!url.trim()) return;
    setDiscovering(true);
    setError(null);
    setSuccessMessage(null);
    setDiscovered(null);

    try {
      const result = await discoverA2aAgent(url.trim());
      setDiscovered(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDiscovering(false);
    }
  };

  const handleRegister = async () => {
    if (!discovered) return;
    setRegistering(true);
    setError(null);

    try {
      await registerA2aAgent(discovered.base_url, discovered.agent_card);
      setSuccessMessage(t('a2a.registerSuccess', { name: discovered.agent_card.name }));
      setDiscovered(null);
      setUrl('');
      await fetchAgents();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setRegistering(false);
    }
  };

  const handleRemove = async (id: string, name: string) => {
    setError(null);
    setSuccessMessage(null);

    try {
      await removeA2aAgent(id);
      setSuccessMessage(t('a2a.removeSuccess', { name }));
      await fetchAgents();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleDismissMessage = () => {
    setError(null);
    setSuccessMessage(null);
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div>
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
          {t('a2a.title')}
        </h3>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
          {t('a2a.description')}
        </p>
      </div>

      {/* Status messages */}
      {error && (
        <div className="flex items-start gap-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-sm text-red-700 dark:text-red-300">
          <span className="flex-1">{error}</span>
          <button
            onClick={handleDismissMessage}
            className="text-red-500 hover:text-red-700 dark:hover:text-red-300 text-xs font-medium"
          >
            {t('a2a.dismiss', 'Dismiss')}
          </button>
        </div>
      )}

      {successMessage && (
        <div className="flex items-start gap-2 p-3 rounded-lg bg-green-50 dark:bg-green-900/20 text-sm text-green-700 dark:text-green-300">
          <span className="flex-1">{successMessage}</span>
          <button
            onClick={handleDismissMessage}
            className="text-green-500 hover:text-green-700 dark:hover:text-green-300 text-xs font-medium"
          >
            {t('a2a.dismiss', 'Dismiss')}
          </button>
        </div>
      )}

      {/* Add Remote Agent */}
      <div className="space-y-3">
        <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">
          {t('a2a.addAgent')}
        </h4>
        <div className="flex gap-2">
          <input
            type="text"
            value={url}
            onChange={(e) => {
              setUrl(e.target.value);
              setDiscovered(null);
              setError(null);
            }}
            placeholder={t('a2a.urlPlaceholder')}
            className={clsx(
              'flex-1 px-3 py-2 text-sm rounded-lg border',
              'bg-white dark:bg-gray-800',
              'border-gray-300 dark:border-gray-600',
              'text-gray-900 dark:text-white',
              'placeholder-gray-400 dark:placeholder-gray-500',
              'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent'
            )}
          />
          <button
            onClick={handleDiscover}
            disabled={discovering || !url.trim()}
            className={clsx(
              'px-4 py-2 text-sm font-medium rounded-lg transition-colors',
              'bg-primary-600 text-white hover:bg-primary-700',
              'disabled:opacity-50 disabled:cursor-not-allowed'
            )}
          >
            {discovering ? t('a2a.discovering') : t('a2a.discover')}
          </button>
        </div>
      </div>

      {/* Discovered Agent Card */}
      {discovered && (
        <div
          className={clsx(
            'p-4 rounded-lg border-2',
            'border-primary-300 dark:border-primary-700',
            'bg-primary-50 dark:bg-primary-900/20'
          )}
        >
          <div className="flex items-start justify-between">
            <div className="min-w-0 flex-1">
              <h4 className="text-sm font-semibold text-gray-900 dark:text-white">
                {discovered.agent_card.name}
              </h4>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                {discovered.agent_card.description}
              </p>
              <div className="flex flex-wrap gap-1 mt-2">
                {discovered.agent_card.capabilities.map((cap) => (
                  <span
                    key={cap}
                    className="px-2 py-0.5 text-xs rounded-full bg-primary-100 dark:bg-primary-800 text-primary-700 dark:text-primary-300"
                  >
                    {cap}
                  </span>
                ))}
              </div>
              <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
                v{discovered.agent_card.version} &middot; {discovered.base_url}
              </p>
            </div>
            <button
              onClick={handleRegister}
              disabled={registering}
              className={clsx(
                'ml-3 px-4 py-1.5 text-xs font-medium rounded-lg transition-colors',
                'bg-green-600 text-white hover:bg-green-700',
                'disabled:opacity-50 disabled:cursor-not-allowed'
              )}
            >
              {registering ? t('a2a.registering') : t('a2a.register')}
            </button>
          </div>
        </div>
      )}

      {/* Registered Agents List */}
      <div className="space-y-3">
        <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300">
          {t('a2a.registeredAgents')}
        </h4>

        {loading ? (
          <p className="text-sm text-gray-400 dark:text-gray-500">
            {t('a2a.loading')}
          </p>
        ) : agents.length === 0 ? (
          <p className="text-sm text-gray-400 dark:text-gray-500 italic">
            {t('a2a.noAgents')}
          </p>
        ) : (
          <div className="space-y-2">
            {agents.map((agent) => (
              <div
                key={agent.id}
                className={clsx(
                  'p-3 rounded-lg border',
                  'border-gray-200 dark:border-gray-700',
                  'bg-gray-50 dark:bg-gray-800/50'
                )}
              >
                <div className="flex items-start justify-between">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-gray-900 dark:text-white">
                        {agent.name}
                      </span>
                      <span className="text-xs text-gray-400 dark:text-gray-500">
                        v{agent.version}
                      </span>
                    </div>
                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                      {agent.description}
                    </p>
                    <div className="flex flex-wrap gap-1 mt-1.5">
                      {agent.capabilities.map((cap) => (
                        <span
                          key={cap}
                          className="px-1.5 py-0.5 text-xs rounded bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300"
                        >
                          {cap}
                        </span>
                      ))}
                    </div>
                    <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
                      {agent.base_url}
                    </p>
                  </div>
                  <button
                    onClick={() => handleRemove(agent.id, agent.name)}
                    className={clsx(
                      'ml-3 px-3 py-1 text-xs font-medium rounded-lg transition-colors',
                      'text-red-600 dark:text-red-400',
                      'hover:bg-red-50 dark:hover:bg-red-900/20',
                      'border border-red-300 dark:border-red-700'
                    )}
                  >
                    {t('a2a.remove')}
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export default A2aSection;
