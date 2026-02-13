/**
 * PhaseAgentSection Component
 *
 * Configure agent assignments for different execution phases.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { ChevronDownIcon, Cross2Icon } from '@radix-ui/react-icons';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../../store/settings';

interface PhaseConfig {
  id: string;
  i18nKey: string;
  defaultAgent: string;
  fallbackChain: string[];
}

const defaultPhases: PhaseConfig[] = [
  {
    id: 'planning',
    i18nKey: 'planning',
    defaultAgent: 'codex',
    fallbackChain: ['claude-code'],
  },
  {
    id: 'implementation',
    i18nKey: 'implementation',
    defaultAgent: 'claude-code',
    fallbackChain: ['codex', 'aider'],
  },
  {
    id: 'retry',
    i18nKey: 'retry',
    defaultAgent: 'claude-code',
    fallbackChain: ['aider'],
  },
  {
    id: 'refactor',
    i18nKey: 'refactor',
    defaultAgent: 'aider',
    fallbackChain: ['claude-code'],
  },
  {
    id: 'review',
    i18nKey: 'review',
    defaultAgent: 'claude-code',
    fallbackChain: ['codex'],
  },
];

export function PhaseAgentSection() {
  const { t } = useTranslation('settings');
  const { agents } = useSettingsStore();
  const [phases, setPhases] = useState<PhaseConfig[]>(defaultPhases);
  const [expandedPhase, setExpandedPhase] = useState<string | null>(null);

  const enabledAgents = agents.filter((a) => a.enabled);

  const handleDefaultAgentChange = (phaseId: string, agentName: string) => {
    setPhases((prev) =>
      prev.map((p) =>
        p.id === phaseId ? { ...p, defaultAgent: agentName } : p
      )
    );
  };

  const handleAddFallback = (phaseId: string, agentName: string) => {
    setPhases((prev) =>
      prev.map((p) =>
        p.id === phaseId && !p.fallbackChain.includes(agentName)
          ? { ...p, fallbackChain: [...p.fallbackChain, agentName] }
          : p
      )
    );
  };

  const handleRemoveFallback = (phaseId: string, agentName: string) => {
    setPhases((prev) =>
      prev.map((p) =>
        p.id === phaseId
          ? { ...p, fallbackChain: p.fallbackChain.filter((a) => a !== agentName) }
          : p
      )
    );
  };

  const handleMoveFallback = (phaseId: string, agentName: string, direction: 'up' | 'down') => {
    setPhases((prev) =>
      prev.map((p) => {
        if (p.id !== phaseId) return p;
        const index = p.fallbackChain.indexOf(agentName);
        if (index === -1) return p;

        const newIndex = direction === 'up' ? index - 1 : index + 1;
        if (newIndex < 0 || newIndex >= p.fallbackChain.length) return p;

        const newChain = [...p.fallbackChain];
        [newChain[index], newChain[newIndex]] = [newChain[newIndex], newChain[index]];
        return { ...p, fallbackChain: newChain };
      })
    );
  };

  const toggleExpanded = (phaseId: string) => {
    setExpandedPhase((prev) => (prev === phaseId ? null : phaseId));
  };

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">
          {t('phases.title')}
        </h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {t('phases.description')}
        </p>
      </div>

      {/* Phase Table */}
      <section className="space-y-4">
        <div className="overflow-hidden rounded-lg border border-gray-200 dark:border-gray-700">
          {/* Header */}
          <div
            className={clsx(
              'grid grid-cols-12 gap-4 px-4 py-3',
              'bg-gray-50 dark:bg-gray-800',
              'text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider'
            )}
          >
            <div className="col-span-3">{t('phases.columns.phase')}</div>
            <div className="col-span-4">{t('phases.columns.defaultAgent')}</div>
            <div className="col-span-5">{t('phases.columns.fallbackChain')}</div>
          </div>

          {/* Rows */}
          <div className="divide-y divide-gray-200 dark:divide-gray-700">
            {phases.map((phase) => (
              <div key={phase.id}>
                {/* Main Row */}
                <div
                  className={clsx(
                    'grid grid-cols-12 gap-4 px-4 py-3 items-center',
                    'bg-white dark:bg-gray-900',
                    'hover:bg-gray-50 dark:hover:bg-gray-800/50'
                  )}
                >
                  {/* Phase Name */}
                  <div className="col-span-3">
                    <button
                      onClick={() => toggleExpanded(phase.id)}
                      className="flex items-center gap-2 text-left"
                    >
                      <ChevronDownIcon
                        className={clsx(
                          'w-4 h-4 text-gray-400 transition-transform',
                          expandedPhase === phase.id && 'rotate-180'
                        )}
                      />
                      <div>
                        <div className="font-medium text-gray-900 dark:text-white">
                          {t(`phases.${phase.i18nKey}.name`)}
                        </div>
                        <div className="text-xs text-gray-500 dark:text-gray-400">
                          {t(`phases.${phase.i18nKey}.description`)}
                        </div>
                      </div>
                    </button>
                  </div>

                  {/* Default Agent */}
                  <div className="col-span-4">
                    <select
                      value={phase.defaultAgent}
                      onChange={(e) => handleDefaultAgentChange(phase.id, e.target.value)}
                      className={clsx(
                        'w-full px-3 py-1.5 rounded-lg border text-sm',
                        'border-gray-200 dark:border-gray-700',
                        'bg-white dark:bg-gray-800',
                        'text-gray-900 dark:text-white',
                        'focus:outline-none focus:ring-2 focus:ring-primary-500'
                      )}
                    >
                      {enabledAgents.map((agent) => (
                        <option key={agent.name} value={agent.name}>
                          {agent.name}
                        </option>
                      ))}
                    </select>
                  </div>

                  {/* Fallback Chain Preview */}
                  <div className="col-span-5">
                    <div className="flex flex-wrap gap-1">
                      {phase.fallbackChain.length > 0 ? (
                        phase.fallbackChain.map((agent, index) => (
                          <span
                            key={agent}
                            className={clsx(
                              'inline-flex items-center px-2 py-0.5 rounded text-xs',
                              'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300'
                            )}
                          >
                            {index + 1}. {agent}
                          </span>
                        ))
                      ) : (
                        <span className="text-sm text-gray-400 dark:text-gray-500">
                          {t('phases.fallback.noFallbacks')}
                        </span>
                      )}
                    </div>
                  </div>
                </div>

                {/* Expanded Details */}
                {expandedPhase === phase.id && (
                  <div
                    className={clsx(
                      'px-4 py-4 border-t',
                      'border-gray-100 dark:border-gray-800',
                      'bg-gray-50 dark:bg-gray-800/30'
                    )}
                  >
                    <div className="ml-6 space-y-4">
                      <div>
                        <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                          {t('phases.fallback.title')}
                        </h4>
                        <p className="text-xs text-gray-500 dark:text-gray-400 mb-3">
                          {t('phases.fallback.help')}
                        </p>

                        {/* Fallback List */}
                        <div className="space-y-2">
                          {phase.fallbackChain.map((agent, index) => (
                            <div
                              key={agent}
                              className={clsx(
                                'flex items-center gap-3 p-2 rounded-lg',
                                'bg-white dark:bg-gray-800',
                                'border border-gray-200 dark:border-gray-700'
                              )}
                            >
                              <span className="text-xs text-gray-400 w-6">
                                #{index + 1}
                              </span>
                              <span className="flex-1 text-sm text-gray-900 dark:text-white">
                                {agent}
                              </span>
                              <div className="flex items-center gap-1">
                                <button
                                  onClick={() => handleMoveFallback(phase.id, agent, 'up')}
                                  disabled={index === 0}
                                  className={clsx(
                                    'p-1 rounded text-xs',
                                    'hover:bg-gray-100 dark:hover:bg-gray-700',
                                    'disabled:opacity-30 disabled:cursor-not-allowed'
                                  )}
                                  title={t('phases.fallback.moveUp')}
                                >
                                  <span className="sr-only">{t('phases.fallback.moveUp')}</span>
                                  <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                    <polyline points="18 15 12 9 6 15" />
                                  </svg>
                                </button>
                                <button
                                  onClick={() => handleMoveFallback(phase.id, agent, 'down')}
                                  disabled={index === phase.fallbackChain.length - 1}
                                  className={clsx(
                                    'p-1 rounded text-xs',
                                    'hover:bg-gray-100 dark:hover:bg-gray-700',
                                    'disabled:opacity-30 disabled:cursor-not-allowed'
                                  )}
                                  title={t('phases.fallback.moveDown')}
                                >
                                  <span className="sr-only">{t('phases.fallback.moveDown')}</span>
                                  <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                                    <polyline points="6 9 12 15 18 9" />
                                  </svg>
                                </button>
                                <button
                                  onClick={() => handleRemoveFallback(phase.id, agent)}
                                  className={clsx(
                                    'p-1 rounded text-red-500 hover:text-red-700',
                                    'hover:bg-red-50 dark:hover:bg-red-900/20'
                                  )}
                                  title={t('phases.fallback.remove')}
                                >
                                  <Cross2Icon className="w-4 h-4" />
                                </button>
                              </div>
                            </div>
                          ))}
                        </div>

                        {/* Add Fallback */}
                        <div className="mt-3">
                          <select
                            value=""
                            onChange={(e) => {
                              if (e.target.value) {
                                handleAddFallback(phase.id, e.target.value);
                              }
                            }}
                            className={clsx(
                              'px-3 py-1.5 rounded-lg border text-sm',
                              'border-gray-200 dark:border-gray-700',
                              'bg-white dark:bg-gray-800',
                              'text-gray-900 dark:text-white',
                              'focus:outline-none focus:ring-2 focus:ring-primary-500'
                            )}
                          >
                            <option value="">{t('phases.fallback.addFallback')}</option>
                            {enabledAgents
                              .filter(
                                (a) =>
                                  a.name !== phase.defaultAgent &&
                                  !phase.fallbackChain.includes(a.name)
                              )
                              .map((agent) => (
                                <option key={agent.name} value={agent.name}>
                                  {agent.name}
                                </option>
                              ))}
                          </select>
                        </div>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* Info Note */}
      <section>
        <div
          className={clsx(
            'p-4 rounded-lg',
            'bg-blue-50 dark:bg-blue-900/20',
            'border border-blue-200 dark:border-blue-800'
          )}
        >
          <h4 className="text-sm font-medium text-blue-800 dark:text-blue-300 mb-1">
            {t('phases.info.title')}
          </h4>
          <p className="text-sm text-blue-700 dark:text-blue-400">
            {t('phases.info.description')}
          </p>
        </div>
      </section>
    </div>
  );
}

export default PhaseAgentSection;
