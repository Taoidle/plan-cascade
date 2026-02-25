/**
 * AgentLibrary Component
 *
 * Main component for browsing and managing agents.
 */

import { useEffect, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { PlusIcon, MagnifyingGlassIcon, ReloadIcon, DownloadIcon, UploadIcon } from '@radix-ui/react-icons';
import { useAgentsStore, getFilteredAgents } from '../../store/agents';
import type { Agent, AgentWithStats } from '../../types/agent';
import { AgentCard, AgentCardSkeleton } from './AgentCard';
import { AgentEditor } from './AgentEditor';
import { AgentRunner } from './AgentRunner';

export function AgentLibrary() {
  const { t } = useTranslation(['agents', 'common']);
  const {
    agents,
    selectedAgent,
    searchQuery,
    loading,
    error,
    fetchAgents,
    setSearchQuery,
    exportAgents,
    importAgents,
    clearError,
  } = useAgentsStore();

  const [editorOpen, setEditorOpen] = useState(false);
  const [editingAgent, setEditingAgent] = useState<Agent | null>(null);
  const [runnerOpen, setRunnerOpen] = useState(false);
  const [runningAgent, setRunningAgent] = useState<Agent | null>(null);

  // Fetch agents on mount
  useEffect(() => {
    fetchAgents();
  }, [fetchAgents]);

  // Filter agents based on search query
  const filteredAgents = getFilteredAgents(agents, searchQuery);

  // Handlers
  const handleCreateAgent = () => {
    setEditingAgent(null);
    setEditorOpen(true);
  };

  const handleEditAgent = (agent: AgentWithStats) => {
    setEditingAgent(agent);
    setEditorOpen(true);
  };

  const handleRunAgent = (agent: AgentWithStats) => {
    setRunningAgent(agent);
    setRunnerOpen(true);
  };

  const handleDeleteAgent = (agent: AgentWithStats) => {
    // Delete is handled in the editor with confirmation
    setEditingAgent(agent);
    setEditorOpen(true);
  };

  const handleExport = async () => {
    const json = await exportAgents();
    if (json) {
      // Create download
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `agents-${new Date().toISOString().split('T')[0]}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    }
  };

  const handleImport = async () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = async (e) => {
      const file = (e.target as HTMLInputElement).files?.[0];
      if (file) {
        const text = await file.text();
        await importAgents(text);
      }
    };
    input.click();
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">{t('title')}</h2>

          <div className="flex items-center gap-2">
            {/* Refresh */}
            <button
              onClick={() => fetchAgents()}
              disabled={loading.agents}
              className={clsx(
                'p-2 rounded-md',
                'bg-gray-100 dark:bg-gray-800',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'disabled:opacity-50',
                'transition-colors',
              )}
              title={t('common:refresh')}
            >
              <ReloadIcon className={clsx('w-4 h-4', loading.agents && 'animate-spin')} />
            </button>

            {/* Export */}
            <button
              onClick={handleExport}
              disabled={agents.length === 0}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
                'bg-gray-100 dark:bg-gray-800',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'text-sm font-medium',
                'disabled:opacity-50',
                'transition-colors',
              )}
            >
              <DownloadIcon className="w-4 h-4" />
              <span className="hidden sm:inline">{t('export')}</span>
            </button>

            {/* Import */}
            <button
              onClick={handleImport}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
                'bg-gray-100 dark:bg-gray-800',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'text-sm font-medium',
                'transition-colors',
              )}
            >
              <UploadIcon className="w-4 h-4" />
              <span className="hidden sm:inline">{t('import')}</span>
            </button>

            {/* Create */}
            <button
              onClick={handleCreateAgent}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
                'bg-primary-600 hover:bg-primary-700',
                'text-white text-sm font-medium',
                'transition-colors',
              )}
            >
              <PlusIcon className="w-4 h-4" />
              <span>{t('createAgent')}</span>
            </button>
          </div>
        </div>

        {/* Search */}
        <div className="relative">
          <MagnifyingGlassIcon className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t('searchPlaceholder')}
            className={clsx(
              'w-full pl-10 pr-4 py-2 rounded-lg border',
              'border-gray-200 dark:border-gray-700',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'placeholder-gray-400 dark:placeholder-gray-500',
              'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent',
            )}
          />
        </div>

        <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">{t('description')}</p>
      </div>

      {/* Error */}
      {error && (
        <div className="mx-4 mt-4 p-3 rounded-md bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400">
          <div className="flex items-center justify-between">
            <span className="text-sm">{error}</span>
            <button onClick={clearError} className="text-sm underline hover:no-underline">
              {t('common:dismiss')}
            </button>
          </div>
        </div>
      )}

      {/* Agent Grid */}
      <div className="flex-1 overflow-y-auto p-4">
        {loading.agents && agents.length === 0 ? (
          // Loading state
          <div className="grid gap-4 grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 3xl:grid-cols-4">
            <AgentCardSkeleton />
            <AgentCardSkeleton />
            <AgentCardSkeleton />
          </div>
        ) : filteredAgents.length === 0 ? (
          // Empty state
          <div className="text-center py-12">
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-gray-100 dark:bg-gray-800 flex items-center justify-center">
              <PlusIcon className="w-8 h-8 text-gray-400" />
            </div>
            <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-2">
              {searchQuery ? t('noResults') : t('noAgents')}
            </h3>
            <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
              {searchQuery ? t('noResultsDescription') : t('noAgentsDescription')}
            </p>
            {!searchQuery && (
              <button
                onClick={handleCreateAgent}
                className={clsx(
                  'inline-flex items-center gap-1.5 px-4 py-2 rounded-md',
                  'bg-primary-600 hover:bg-primary-700',
                  'text-white text-sm font-medium',
                  'transition-colors',
                )}
              >
                <PlusIcon className="w-4 h-4" />
                {t('createFirstAgent')}
              </button>
            )}
          </div>
        ) : (
          // Agent cards
          <div className="grid gap-4 grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 3xl:grid-cols-4">
            {filteredAgents.map((agent) => (
              <AgentCard
                key={agent.id}
                agent={agent}
                isSelected={selectedAgent?.id === agent.id}
                onRun={() => handleRunAgent(agent)}
                onEdit={() => handleEditAgent(agent)}
                onDelete={() => handleDeleteAgent(agent)}
              />
            ))}
          </div>
        )}
      </div>

      {/* Editor Dialog */}
      <AgentEditor
        agent={editingAgent}
        open={editorOpen}
        onOpenChange={setEditorOpen}
        onSaved={() => {
          setEditorOpen(false);
          fetchAgents();
        }}
      />

      {/* Runner Dialog */}
      <AgentRunner agent={runningAgent} open={runnerOpen} onOpenChange={setRunnerOpen} />
    </div>
  );
}

export default AgentLibrary;
