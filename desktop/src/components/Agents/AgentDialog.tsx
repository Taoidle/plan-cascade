/**
 * AgentDialog Component
 *
 * Full-screen dialog for managing agents with a two-column layout:
 * left sidebar for agent list, right panel for editing.
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import * as Dialog from '@radix-ui/react-dialog';
import { Cross2Icon, PlusIcon, MagnifyingGlassIcon, TrashIcon } from '@radix-ui/react-icons';
import { useAgentsStore, getFilteredAgents } from '../../store/agents';
import { CLAUDE_MODELS, getToolsByCategory } from '../../types/agent';
import type { Agent, AgentCreateRequest, AgentUpdateRequest, AgentWithStats } from '../../types/agent';

// ============================================================================
// AgentDialog
// ============================================================================

export function AgentDialog() {
  const { t } = useTranslation('simpleMode');

  const agents = useAgentsStore((s) => s.agents);
  const loading = useAgentsStore((s) => s.loading);
  const dialogOpen = useAgentsStore((s) => s.dialogOpen);
  const editingAgent = useAgentsStore((s) => s.editingAgent);
  const fetchAgents = useAgentsStore((s) => s.fetchAgents);
  const createAgent = useAgentsStore((s) => s.createAgent);
  const updateAgent = useAgentsStore((s) => s.updateAgent);
  const deleteAgent = useAgentsStore((s) => s.deleteAgent);
  const closeDialog = useAgentsStore((s) => s.closeDialog);
  const exportAgents = useAgentsStore((s) => s.exportAgents);
  const importAgents = useAgentsStore((s) => s.importAgents);

  const [searchQuery, setSearchQuery] = useState('');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [isNew, setIsNew] = useState(false);

  // Form state
  const [formName, setFormName] = useState('');
  const [formDescription, setFormDescription] = useState('');
  const [formModel, setFormModel] = useState('claude-sonnet-4-20250514');
  const [formSystemPrompt, setFormSystemPrompt] = useState('');
  const [formAllowedTools, setFormAllowedTools] = useState<string[]>([]);

  const filteredAgents = useMemo(() => getFilteredAgents(agents, searchQuery), [agents, searchQuery]);

  // Load agents when dialog opens
  useEffect(() => {
    if (dialogOpen) {
      fetchAgents();
    }
  }, [dialogOpen, fetchAgents]);

  // Select editing agent if provided
  useEffect(() => {
    if (editingAgent && dialogOpen) {
      setSelectedId(editingAgent.id);
      populateForm(editingAgent);
      setIsNew(false);
    }
  }, [editingAgent, dialogOpen]);

  const populateForm = useCallback((agent: Agent | AgentWithStats) => {
    setFormName(agent.name);
    setFormDescription(agent.description || '');
    setFormModel(agent.model || 'claude-sonnet-4-20250514');
    setFormSystemPrompt(agent.system_prompt || '');
    const tools: string[] =
      typeof agent.allowed_tools === 'string' ? JSON.parse(agent.allowed_tools || '[]') : agent.allowed_tools || [];
    setFormAllowedTools(tools);
  }, []);

  const resetForm = useCallback(() => {
    setFormName('');
    setFormDescription('');
    setFormModel('claude-sonnet-4-20250514');
    setFormSystemPrompt('');
    setFormAllowedTools([]);
  }, []);

  const handleSelectAgent = useCallback(
    (agent: AgentWithStats) => {
      setSelectedId(agent.id);
      populateForm(agent);
      setIsNew(false);
    },
    [populateForm],
  );

  const handleNewAgent = useCallback(() => {
    setSelectedId(null);
    setIsNew(true);
    resetForm();
  }, [resetForm]);

  const handleSave = useCallback(async () => {
    if (!formName.trim()) return;

    if (isNew) {
      const req: AgentCreateRequest = {
        name: formName.trim(),
        description: formDescription.trim() || null,
        system_prompt: formSystemPrompt,
        model: formModel,
        allowed_tools: formAllowedTools,
      };
      const result = await createAgent(req);
      if (result) {
        setSelectedId(result.id);
        setIsNew(false);
      }
    } else if (selectedId) {
      const req: AgentUpdateRequest = {
        name: formName.trim(),
        description: formDescription.trim() || null,
        system_prompt: formSystemPrompt,
        model: formModel,
        allowed_tools: formAllowedTools,
      };
      await updateAgent(selectedId, req);
    }
  }, [
    isNew,
    selectedId,
    formName,
    formDescription,
    formModel,
    formSystemPrompt,
    formAllowedTools,
    createAgent,
    updateAgent,
  ]);

  const handleDelete = useCallback(async () => {
    if (!selectedId) return;
    const confirmed = window.confirm(t('agentDialog.deleteConfirm', { defaultValue: 'Delete this agent?' }));
    if (!confirmed) return;

    const success = await deleteAgent(selectedId);
    if (success) {
      setSelectedId(null);
      resetForm();
      setIsNew(false);
    }
  }, [selectedId, deleteAgent, resetForm, t]);

  const handleExport = useCallback(async () => {
    const json = await exportAgents();
    if (json) {
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'agents-export.json';
      a.click();
      URL.revokeObjectURL(url);
    }
  }, [exportAgents]);

  const handleImport = useCallback(async () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const text = await file.text();
      await importAgents(text);
    };
    input.click();
  }, [importAgents]);

  const handleToolToggle = useCallback((toolName: string) => {
    setFormAllowedTools((prev) => (prev.includes(toolName) ? prev.filter((t) => t !== toolName) : [...prev, toolName]));
  }, []);

  const toolCategories = useMemo(() => getToolsByCategory(), []);

  return (
    <Dialog.Root open={dialogOpen} onOpenChange={(open) => !open && closeDialog()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 z-50" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-[720px] h-[580px] z-50',
            'bg-white dark:bg-gray-900 rounded-xl shadow-xl',
            'flex flex-col overflow-hidden',
            'focus:outline-none',
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
            <Dialog.Title className="text-sm font-semibold text-gray-900 dark:text-white">
              {t('agentDialog.title', { defaultValue: 'Manage Agents' })}
            </Dialog.Title>
            <div className="flex items-center gap-2">
              <button
                onClick={handleExport}
                className="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 px-2 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
              >
                {t('agentDialog.export', { defaultValue: 'Export' })}
              </button>
              <button
                onClick={handleImport}
                className="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 px-2 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
              >
                {t('agentDialog.import', { defaultValue: 'Import' })}
              </button>
              <Dialog.Close asChild>
                <button className="p-1 rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800">
                  <Cross2Icon className="w-4 h-4" />
                </button>
              </Dialog.Close>
            </div>
          </div>

          {/* Body */}
          <div className="flex flex-1 min-h-0">
            {/* Left: Agent list */}
            <div className="w-[220px] border-r border-gray-200 dark:border-gray-700 flex flex-col">
              <div className="p-2">
                <div className="relative mb-2">
                  <MagnifyingGlassIcon className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
                  <input
                    type="text"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder={t('agentDialog.searchPlaceholder', { defaultValue: 'Search agents...' })}
                    className="w-full pl-7 pr-2 py-1.5 text-xs rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-1 focus:ring-primary-500"
                  />
                </div>
                <button
                  onClick={handleNewAgent}
                  className={clsx(
                    'w-full flex items-center gap-1.5 px-2 py-1.5 rounded-md text-xs',
                    'text-primary-600 dark:text-primary-400',
                    'hover:bg-primary-50 dark:hover:bg-primary-900/20',
                    'transition-colors',
                  )}
                >
                  <PlusIcon className="w-3.5 h-3.5" />
                  {t('agentDialog.newAgent', { defaultValue: 'New Agent' })}
                </button>
              </div>

              <div className="flex-1 overflow-y-auto px-2 pb-2 space-y-0.5">
                {filteredAgents.map((agent) => (
                  <button
                    key={agent.id}
                    onClick={() => handleSelectAgent(agent)}
                    className={clsx(
                      'w-full text-left px-2 py-1.5 rounded-md text-xs transition-colors',
                      selectedId === agent.id
                        ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-900 dark:text-primary-100'
                        : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800',
                    )}
                  >
                    <div className="truncate font-medium">{agent.name}</div>
                    {agent.description && (
                      <div className="truncate text-2xs text-gray-400 dark:text-gray-500">{agent.description}</div>
                    )}
                  </button>
                ))}
              </div>
            </div>

            {/* Right: Edit form */}
            <div className="flex-1 overflow-y-auto p-4 space-y-3">
              {!selectedId && !isNew ? (
                <div className="h-full flex items-center justify-center text-xs text-gray-400">
                  {t('agentDialog.selectOrCreate', { defaultValue: 'Select or create an agent' })}
                </div>
              ) : (
                <>
                  {/* Name */}
                  <div>
                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('agentDialog.name', { defaultValue: 'Name' })}
                    </label>
                    <input
                      type="text"
                      value={formName}
                      onChange={(e) => setFormName(e.target.value)}
                      placeholder={t('agentDialog.namePlaceholder', { defaultValue: 'Agent name' })}
                      className="w-full px-3 py-1.5 text-sm rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500"
                    />
                  </div>

                  {/* Description */}
                  <div>
                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('agentDialog.description', { defaultValue: 'Description' })}
                    </label>
                    <input
                      type="text"
                      value={formDescription}
                      onChange={(e) => setFormDescription(e.target.value)}
                      placeholder={t('agentDialog.descriptionPlaceholder', {
                        defaultValue: 'What does this agent do?',
                      })}
                      className="w-full px-3 py-1.5 text-sm rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500"
                    />
                  </div>

                  {/* Model */}
                  <div>
                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('agentDialog.model', { defaultValue: 'Model' })}
                    </label>
                    <select
                      value={formModel}
                      onChange={(e) => setFormModel(e.target.value)}
                      className="w-full px-3 py-1.5 text-sm rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500"
                    >
                      {CLAUDE_MODELS.map((m) => (
                        <option key={m.id} value={m.id}>
                          {m.name}
                        </option>
                      ))}
                    </select>
                  </div>

                  {/* System Prompt */}
                  <div>
                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('agentDialog.systemPrompt', { defaultValue: 'System Prompt' })}
                    </label>
                    <textarea
                      value={formSystemPrompt}
                      onChange={(e) => setFormSystemPrompt(e.target.value)}
                      placeholder={t('agentDialog.systemPromptPlaceholder', {
                        defaultValue: "Define the agent's behavior...",
                      })}
                      rows={8}
                      className="w-full px-3 py-1.5 text-sm rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-primary-500 resize-none min-h-[200px]"
                    />
                  </div>

                  {/* Allowed Tools */}
                  <div>
                    <label className="block text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('agentDialog.allowedTools', { defaultValue: 'Allowed Tools' })}
                    </label>
                    <p className="text-2xs text-gray-400 dark:text-gray-500 mb-2">
                      {formAllowedTools.length === 0
                        ? t('agentDialog.allTools', { defaultValue: 'All tools allowed' })
                        : `${formAllowedTools.length} tools selected`}
                    </p>
                    <div className="space-y-2 max-h-[140px] overflow-y-auto">
                      {Object.entries(toolCategories).map(([category, tools]) => (
                        <div key={category}>
                          <div className="text-2xs font-medium text-gray-500 dark:text-gray-400 mb-1">{category}</div>
                          <div className="flex flex-wrap gap-1">
                            {tools.map((tool) => (
                              <button
                                key={tool.name}
                                onClick={() => handleToolToggle(tool.name)}
                                className={clsx(
                                  'px-1.5 py-0.5 text-2xs rounded-md border transition-colors',
                                  formAllowedTools.includes(tool.name)
                                    ? 'bg-primary-100 dark:bg-primary-900/30 border-primary-300 dark:border-primary-700 text-primary-700 dark:text-primary-300'
                                    : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-400 hover:border-primary-300 dark:hover:border-primary-700',
                                )}
                              >
                                {tool.name}
                              </button>
                            ))}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>

                  {/* Actions */}
                  <div className="flex items-center gap-2 pt-2 border-t border-gray-200 dark:border-gray-700">
                    <button
                      onClick={handleSave}
                      disabled={!formName.trim() || loading.creating || loading.updating}
                      className={clsx(
                        'px-4 py-1.5 text-xs font-medium rounded-md transition-colors',
                        'bg-primary-600 text-white hover:bg-primary-700',
                        'disabled:opacity-50 disabled:cursor-not-allowed',
                      )}
                    >
                      {t('agentDialog.save', { defaultValue: 'Save' })}
                    </button>
                    {selectedId && !isNew && (
                      <button
                        onClick={handleDelete}
                        disabled={loading.deleting}
                        className={clsx(
                          'px-3 py-1.5 text-xs font-medium rounded-md transition-colors',
                          'text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20',
                          'disabled:opacity-50',
                        )}
                      >
                        <TrashIcon className="w-3.5 h-3.5 inline mr-1" />
                        {t('agentDialog.delete', { defaultValue: 'Delete' })}
                      </button>
                    )}
                  </div>
                </>
              )}
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default AgentDialog;
