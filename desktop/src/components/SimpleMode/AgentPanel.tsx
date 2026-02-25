/**
 * AgentPanel Component
 *
 * Collapsible sidebar panel showing agents with quick actions.
 * Includes a "Manage All..." button to open the AgentDialog.
 */

import { useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { GearIcon, PlayIcon, Pencil1Icon } from '@radix-ui/react-icons';
import { useAgentsStore, getFilteredAgents } from '../../store/agents';
import { useExecutionStore } from '../../store/execution';
import { Collapsible } from './Collapsible';

// ============================================================================
// AgentPanel
// ============================================================================

export function AgentPanel() {
  const { t } = useTranslation('simpleMode');

  const agents = useAgentsStore((s) => s.agents);
  const searchQuery = useAgentsStore((s) => s.searchQuery);
  const loading = useAgentsStore((s) => s.loading.agents);
  const panelOpen = useAgentsStore((s) => s.panelOpen);
  const fetchAgents = useAgentsStore((s) => s.fetchAgents);
  const openDialog = useAgentsStore((s) => s.openDialog);
  const setActiveAgentForSession = useAgentsStore((s) => s.setActiveAgentForSession);

  // Load data when panel opens
  useEffect(() => {
    if (panelOpen && agents.length === 0) {
      fetchAgents();
    }
  }, [panelOpen, agents.length, fetchAgents]);

  const filteredAgents = getFilteredAgents(agents, searchQuery);

  const handleStartChat = useCallback(
    (agent: (typeof agents)[0]) => {
      setActiveAgentForSession(agent);
      useExecutionStore.getState().reset();
    },
    [setActiveAgentForSession],
  );

  const handleEdit = useCallback(
    (agent: (typeof agents)[0]) => {
      openDialog(agent);
    },
    [openDialog],
  );

  const handleManageAll = useCallback(() => {
    openDialog();
  }, [openDialog]);

  return (
    <Collapsible open={panelOpen}>
      <div data-testid="agent-panel" className="border-t border-gray-200 dark:border-gray-700">
        {/* Header */}
        <div className="flex items-center justify-between px-3 py-2">
          <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
            {t('agentPanel.title', { defaultValue: 'Agents' })}
          </span>
          <button
            onClick={handleManageAll}
            className={clsx(
              'p-1 rounded-md transition-colors',
              'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
            title={t('agentPanel.manageAll', { defaultValue: 'Manage All...' })}
          >
            <GearIcon className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Content */}
        <div className="px-2 pb-2 space-y-1 max-h-[300px] overflow-y-auto">
          {/* Loading state */}
          {loading && agents.length === 0 && (
            <div className="text-center py-4">
              <span className="text-xs text-gray-400 dark:text-gray-500">
                {t('agentPanel.loading', { defaultValue: 'Loading agents...' })}
              </span>
            </div>
          )}

          {/* Empty state */}
          {!loading && agents.length === 0 && (
            <div className="text-center py-4">
              <span className="text-xs text-gray-400 dark:text-gray-500">
                {t('agentPanel.noAgents', { defaultValue: 'No agents yet' })}
              </span>
            </div>
          )}

          {/* Agent list */}
          {filteredAgents.map((agent) => (
            <div
              key={agent.id}
              className={clsx(
                'flex items-center gap-2 px-2 py-1.5 rounded-md',
                'hover:bg-gray-50 dark:hover:bg-gray-800',
                'transition-colors',
              )}
            >
              {/* Info */}
              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1">
                  <span className="text-xs text-gray-900 dark:text-white truncate">{agent.name}</span>
                  {agent.model && (
                    <span className="text-2xs px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400 shrink-0">
                      {agent.model.replace('claude-', '').split('-')[0]}
                    </span>
                  )}
                </div>
                {agent.description && (
                  <p className="text-2xs text-gray-400 dark:text-gray-500 truncate">{agent.description}</p>
                )}
              </div>

              {/* Actions */}
              <div className="flex items-center gap-0.5 shrink-0">
                <button
                  onClick={() => handleStartChat(agent)}
                  className={clsx(
                    'p-1 rounded-md transition-colors',
                    'text-primary-500 hover:text-primary-700 dark:hover:text-primary-300',
                    'hover:bg-primary-50 dark:hover:bg-primary-900/20',
                  )}
                  title={t('agentPanel.startChat', { defaultValue: 'Start Chat' })}
                >
                  <PlayIcon className="w-3 h-3" />
                </button>
                <button
                  onClick={() => handleEdit(agent)}
                  className={clsx(
                    'p-1 rounded-md transition-colors',
                    'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                    'hover:bg-gray-100 dark:hover:bg-gray-800',
                  )}
                  title={t('agentPanel.edit', { defaultValue: 'Edit' })}
                >
                  <Pencil1Icon className="w-3 h-3" />
                </button>
              </div>
            </div>
          ))}
        </div>

        {/* Manage All button */}
        <div className="px-3 pb-2">
          <button
            onClick={handleManageAll}
            className={clsx(
              'w-full px-2 py-1.5 rounded-md text-xs font-medium transition-colors',
              'text-primary-600 dark:text-primary-400',
              'hover:bg-primary-50 dark:hover:bg-primary-900/20',
              'border border-primary-200 dark:border-primary-800',
            )}
          >
            {t('agentPanel.manageAll', { defaultValue: 'Manage All...' })}
          </button>
        </div>
      </div>
    </Collapsible>
  );
}

export default AgentPanel;
