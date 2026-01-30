/**
 * AgentEditor Component
 *
 * Form for creating and editing agents.
 */

import { useState, useEffect } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import * as Dialog from '@radix-ui/react-dialog';
import { Cross2Icon, CheckIcon, TrashIcon } from '@radix-ui/react-icons';
import type { Agent, AgentCreateRequest, AgentUpdateRequest } from '../../types/agent';
import { CLAUDE_MODELS, DEFAULT_MODEL, AVAILABLE_TOOLS, getToolsByCategory } from '../../types/agent';
import { useAgentsStore } from '../../store/agents';

interface AgentEditorProps {
  /** Agent to edit (null for create mode) */
  agent: Agent | null;
  /** Whether the dialog is open */
  open: boolean;
  /** Callback when open state changes */
  onOpenChange: (open: boolean) => void;
  /** Callback when agent is saved */
  onSaved?: (agent: Agent) => void;
}

export function AgentEditor({ agent, open, onOpenChange, onSaved }: AgentEditorProps) {
  const { t } = useTranslation(['agents', 'common']);
  const { createAgent, updateAgent, deleteAgent, loading } = useAgentsStore();

  const isEditMode = agent !== null;

  // Form state
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [systemPrompt, setSystemPrompt] = useState('');
  const [model, setModel] = useState(DEFAULT_MODEL);
  const [allowedTools, setAllowedTools] = useState<string[]>([]);

  // Validation state
  const [errors, setErrors] = useState<Record<string, string>>({});

  // Reset form when agent changes
  useEffect(() => {
    if (agent) {
      setName(agent.name);
      setDescription(agent.description || '');
      setSystemPrompt(agent.system_prompt);
      setModel(agent.model);
      setAllowedTools(agent.allowed_tools);
    } else {
      setName('');
      setDescription('');
      setSystemPrompt('');
      setModel(DEFAULT_MODEL);
      setAllowedTools([]);
    }
    setErrors({});
  }, [agent, open]);

  const validate = (): boolean => {
    const newErrors: Record<string, string> = {};

    if (!name.trim()) {
      newErrors.name = t('errors.nameRequired');
    } else if (name.length > 100) {
      newErrors.name = t('errors.nameTooLong');
    }

    if (!systemPrompt.trim()) {
      newErrors.systemPrompt = t('errors.promptRequired');
    } else if (systemPrompt.length > 100000) {
      newErrors.systemPrompt = t('errors.promptTooLong');
    }

    if (!model) {
      newErrors.model = t('errors.modelRequired');
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSave = async () => {
    if (!validate()) return;

    let result: Agent | null = null;

    if (isEditMode && agent) {
      const request: AgentUpdateRequest = {
        name: name !== agent.name ? name : undefined,
        description: description !== (agent.description || '') ? description || null : undefined,
        system_prompt: systemPrompt !== agent.system_prompt ? systemPrompt : undefined,
        model: model !== agent.model ? model : undefined,
        allowed_tools: JSON.stringify(allowedTools) !== JSON.stringify(agent.allowed_tools) ? allowedTools : undefined,
      };

      result = await updateAgent(agent.id, request);
    } else {
      const request: AgentCreateRequest = {
        name,
        description: description || null,
        system_prompt: systemPrompt,
        model,
        allowed_tools: allowedTools,
      };

      result = await createAgent(request);
    }

    if (result) {
      onOpenChange(false);
      onSaved?.(result);
    }
  };

  const handleDelete = async () => {
    if (!agent) return;

    if (!confirm(t('confirmDelete', { name: agent.name }))) {
      return;
    }

    const success = await deleteAgent(agent.id);
    if (success) {
      onOpenChange(false);
    }
  };

  const toggleTool = (toolName: string) => {
    setAllowedTools((prev) =>
      prev.includes(toolName)
        ? prev.filter((t) => t !== toolName)
        : [...prev, toolName]
    );
  };

  const toggleCategory = (category: string) => {
    const categoryTools: string[] = AVAILABLE_TOOLS.filter((t) => t.category === category).map(
      (t) => t.name
    );
    const allSelected = categoryTools.every((t) => allowedTools.includes(t));

    if (allSelected) {
      setAllowedTools((prev) => prev.filter((t) => !categoryTools.includes(t)));
    } else {
      setAllowedTools((prev) => [...new Set([...prev, ...categoryTools])]);
    }
  };

  const toolsByCategory = getToolsByCategory();

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm z-50" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-2xl max-h-[85vh] overflow-y-auto',
            'bg-white dark:bg-gray-900 rounded-lg shadow-xl z-50',
            'p-6'
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between mb-6">
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              {isEditMode ? t('editAgent') : t('createAgent')}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button
                className="p-1 rounded-md hover:bg-gray-100 dark:hover:bg-gray-800"
                aria-label={t('common:close')}
              >
                <Cross2Icon className="w-5 h-5" />
              </button>
            </Dialog.Close>
          </div>

          {/* Form */}
          <div className="space-y-4">
            {/* Name */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                {t('name')} *
              </label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t('namePlaceholder')}
                className={clsx(
                  'w-full px-3 py-2 rounded-md border',
                  errors.name
                    ? 'border-red-500 focus:ring-red-500'
                    : 'border-gray-300 dark:border-gray-600 focus:ring-primary-500',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'focus:outline-none focus:ring-2'
                )}
              />
              {errors.name && (
                <p className="mt-1 text-sm text-red-500">{errors.name}</p>
              )}
            </div>

            {/* Description */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                {t('description')}
              </label>
              <input
                type="text"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={t('descriptionPlaceholder')}
                className={clsx(
                  'w-full px-3 py-2 rounded-md border',
                  'border-gray-300 dark:border-gray-600 focus:ring-primary-500',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'focus:outline-none focus:ring-2'
                )}
              />
            </div>

            {/* Model */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                {t('model')} *
              </label>
              <select
                value={model}
                onChange={(e) => setModel(e.target.value)}
                className={clsx(
                  'w-full px-3 py-2 rounded-md border',
                  errors.model
                    ? 'border-red-500 focus:ring-red-500'
                    : 'border-gray-300 dark:border-gray-600 focus:ring-primary-500',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'focus:outline-none focus:ring-2'
                )}
              >
                {CLAUDE_MODELS.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name} - {m.description}
                  </option>
                ))}
              </select>
              {errors.model && (
                <p className="mt-1 text-sm text-red-500">{errors.model}</p>
              )}
            </div>

            {/* System Prompt */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                {t('systemPrompt')} *
              </label>
              <textarea
                value={systemPrompt}
                onChange={(e) => setSystemPrompt(e.target.value)}
                placeholder={t('systemPromptPlaceholder')}
                rows={6}
                className={clsx(
                  'w-full px-3 py-2 rounded-md border font-mono text-sm',
                  errors.systemPrompt
                    ? 'border-red-500 focus:ring-red-500'
                    : 'border-gray-300 dark:border-gray-600 focus:ring-primary-500',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'focus:outline-none focus:ring-2'
                )}
              />
              {errors.systemPrompt && (
                <p className="mt-1 text-sm text-red-500">{errors.systemPrompt}</p>
              )}
              <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                {systemPrompt.length.toLocaleString()} / 100,000 {t('characters')}
              </p>
            </div>

            {/* Allowed Tools */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                {t('allowedTools')}
                <span className="ml-2 text-xs text-gray-500">
                  ({allowedTools.length === 0
                    ? t('allToolsAllowed')
                    : t('toolsSelected', { count: allowedTools.length })})
                </span>
              </label>

              <div className="space-y-3 max-h-48 overflow-y-auto border border-gray-200 dark:border-gray-700 rounded-md p-3">
                {Object.entries(toolsByCategory).map(([category, tools]) => {
                  const categorySelected = tools.filter((t) =>
                    allowedTools.includes(t.name)
                  ).length;
                  const allSelected = categorySelected === tools.length;
                  const someSelected = categorySelected > 0 && !allSelected;

                  return (
                    <div key={category}>
                      {/* Category Header */}
                      <button
                        type="button"
                        onClick={() => toggleCategory(category)}
                        className={clsx(
                          'flex items-center gap-2 w-full text-left',
                          'text-sm font-medium text-gray-700 dark:text-gray-300',
                          'hover:text-gray-900 dark:hover:text-white'
                        )}
                      >
                        <span
                          className={clsx(
                            'w-4 h-4 rounded flex items-center justify-center',
                            allSelected
                              ? 'bg-primary-600 text-white'
                              : someSelected
                              ? 'bg-primary-200 dark:bg-primary-800'
                              : 'border border-gray-300 dark:border-gray-600'
                          )}
                        >
                          {(allSelected || someSelected) && (
                            <CheckIcon className="w-3 h-3" />
                          )}
                        </span>
                        {category}
                      </button>

                      {/* Tools in Category */}
                      <div className="ml-6 mt-1 space-y-1">
                        {tools.map((tool) => (
                          <label
                            key={tool.name}
                            className="flex items-center gap-2 text-sm cursor-pointer"
                          >
                            <input
                              type="checkbox"
                              checked={allowedTools.includes(tool.name)}
                              onChange={() => toggleTool(tool.name)}
                              className="rounded border-gray-300 dark:border-gray-600 text-primary-600 focus:ring-primary-500"
                            />
                            <span className="text-gray-600 dark:text-gray-400">
                              {tool.name}
                            </span>
                            <span className="text-xs text-gray-400 dark:text-gray-500">
                              - {tool.description}
                            </span>
                          </label>
                        ))}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between mt-6 pt-4 border-t border-gray-200 dark:border-gray-700">
            {isEditMode ? (
              <button
                onClick={handleDelete}
                disabled={loading.deleting}
                className={clsx(
                  'flex items-center gap-1 px-3 py-2 rounded-md',
                  'bg-red-100 dark:bg-red-900/50',
                  'text-red-700 dark:text-red-300',
                  'hover:bg-red-200 dark:hover:bg-red-800',
                  'disabled:opacity-50',
                  'text-sm font-medium transition-colors'
                )}
              >
                <TrashIcon className="w-4 h-4" />
                {t('common:delete')}
              </button>
            ) : (
              <div />
            )}

            <div className="flex items-center gap-2">
              <Dialog.Close asChild>
                <button
                  className={clsx(
                    'px-4 py-2 rounded-md',
                    'bg-gray-100 dark:bg-gray-800',
                    'text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-200 dark:hover:bg-gray-700',
                    'text-sm font-medium transition-colors'
                  )}
                >
                  {t('common:cancel')}
                </button>
              </Dialog.Close>

              <button
                onClick={handleSave}
                disabled={loading.creating || loading.updating}
                className={clsx(
                  'px-4 py-2 rounded-md',
                  'bg-primary-600 hover:bg-primary-700',
                  'text-white text-sm font-medium',
                  'disabled:opacity-50',
                  'transition-colors'
                )}
              >
                {loading.creating || loading.updating
                  ? t('common:saving')
                  : isEditMode
                  ? t('common:save')
                  : t('common:create')}
              </button>
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default AgentEditor;
