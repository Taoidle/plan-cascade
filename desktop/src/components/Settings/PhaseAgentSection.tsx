/**
 * PhaseAgentSection Component
 *
 * Configure agent assignments for different execution phases.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { ChevronDownIcon, Cross2Icon } from '@radix-ui/react-icons';
import { useSettingsStore } from '../../store/settings';

interface PhaseConfig {
  id: string;
  name: string;
  description: string;
  defaultAgent: string;
  fallbackChain: string[];
}

const defaultPhases: PhaseConfig[] = [
  {
    id: 'planning',
    name: 'Planning',
    description: 'PRD generation and story creation',
    defaultAgent: 'codex',
    fallbackChain: ['claude-code'],
  },
  {
    id: 'implementation',
    name: 'Implementation',
    description: 'Code generation and feature development',
    defaultAgent: 'claude-code',
    fallbackChain: ['codex', 'aider'],
  },
  {
    id: 'retry',
    name: 'Retry',
    description: 'Fixing failed quality gates',
    defaultAgent: 'claude-code',
    fallbackChain: ['aider'],
  },
  {
    id: 'refactor',
    name: 'Refactor',
    description: 'Code refactoring and cleanup',
    defaultAgent: 'aider',
    fallbackChain: ['claude-code'],
  },
  {
    id: 'review',
    name: 'Review',
    description: 'Code review and validation',
    defaultAgent: 'claude-code',
    fallbackChain: ['codex'],
  },
];

export function PhaseAgentSection() {
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
          Phase Agent Assignment
        </h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Assign default agents and fallback chains to each execution phase.
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
            <div className="col-span-3">Phase</div>
            <div className="col-span-4">Default Agent</div>
            <div className="col-span-5">Fallback Chain</div>
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
                          {phase.name}
                        </div>
                        <div className="text-xs text-gray-500 dark:text-gray-400">
                          {phase.description}
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
                          No fallbacks configured
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
                          Fallback Chain Configuration
                        </h4>
                        <p className="text-xs text-gray-500 dark:text-gray-400 mb-3">
                          If the default agent fails, agents will be tried in order until one succeeds.
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
                                  title="Move up"
                                >
                                  <span className="sr-only">Move up</span>
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
                                  title="Move down"
                                >
                                  <span className="sr-only">Move down</span>
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
                                  title="Remove"
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
                            <option value="">+ Add fallback agent...</option>
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
            How Phase Assignment Works
          </h4>
          <p className="text-sm text-blue-700 dark:text-blue-400">
            Each execution phase can be assigned a default agent. If that agent fails or is
            unavailable, the system will try agents from the fallback chain in order. This
            allows for robust multi-agent orchestration with graceful degradation.
          </p>
        </div>
      </section>
    </div>
  );
}

export default PhaseAgentSection;
