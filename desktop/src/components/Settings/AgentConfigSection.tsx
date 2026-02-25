/**
 * AgentConfigSection Component
 *
 * Agent configuration panel for managing execution agents.
 */

import { useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import * as Switch from '@radix-ui/react-switch';
import { clsx } from 'clsx';
import { PlusIcon, Pencil1Icon, TrashIcon, Cross2Icon, StarFilledIcon } from '@radix-ui/react-icons';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../../store/settings';

interface Agent {
  name: string;
  enabled: boolean;
  command: string;
  isDefault: boolean;
}

interface AgentFormData {
  name: string;
  command: string;
  enabled: boolean;
}

export function AgentConfigSection() {
  const { t } = useTranslation('settings');
  const { agents, defaultAgent, agentSelection, updateAgent } = useSettingsStore();
  const [isAddDialogOpen, setIsAddDialogOpen] = useState(false);
  const [editingAgent, setEditingAgent] = useState<Agent | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);

  const handleSetDefault = (agentName: string) => {
    // Update all agents, setting only this one as default
    agents.forEach((agent) => {
      updateAgent(agent.name, { isDefault: agent.name === agentName });
    });
    useSettingsStore.setState({ defaultAgent: agentName });
  };

  const handleToggleEnabled = (agentName: string, enabled: boolean) => {
    updateAgent(agentName, { enabled });
  };

  const handleAddAgent = (data: AgentFormData) => {
    const newAgent: Agent = {
      name: data.name,
      command: data.command,
      enabled: data.enabled,
      isDefault: false,
    };
    useSettingsStore.setState({
      agents: [...agents, newAgent],
    });
    setIsAddDialogOpen(false);
  };

  const handleEditAgent = (data: AgentFormData) => {
    if (!editingAgent) return;
    updateAgent(editingAgent.name, {
      command: data.command,
      enabled: data.enabled,
    });
    setEditingAgent(null);
  };

  const handleDeleteAgent = (agentName: string) => {
    useSettingsStore.setState({
      agents: agents.filter((a) => a.name !== agentName),
    });
    // If deleting default, set first enabled as default
    if (defaultAgent === agentName) {
      const firstEnabled = agents.find((a) => a.name !== agentName && a.enabled);
      if (firstEnabled) {
        handleSetDefault(firstEnabled.name);
      }
    }
    setDeleteConfirm(null);
  };

  const handleAgentSelectionChange = (value: string) => {
    useSettingsStore.setState({
      agentSelection: value as 'smart' | 'prefer_default' | 'manual',
    });
  };

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">{t('agents.title')}</h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('agents.description')}</p>
      </div>

      {/* Agent Selection Strategy */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('agents.selection.title')}</h3>
        <select
          value={agentSelection}
          onChange={(e) => handleAgentSelectionChange(e.target.value)}
          className={clsx(
            'w-full max-w-xs px-3 py-2 rounded-lg border',
            'border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800',
            'text-gray-900 dark:text-white',
            'focus:outline-none focus:ring-2 focus:ring-primary-500',
          )}
        >
          <option value="smart">{t('agents.selection.smart')}</option>
          <option value="prefer_default">{t('agents.selection.preferDefault')}</option>
          <option value="manual">{t('agents.selection.manual')}</option>
        </select>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          {agentSelection === 'smart' && t('agents.selection.smartDescription')}
          {agentSelection === 'prefer_default' && t('agents.selection.preferDefaultDescription')}
          {agentSelection === 'manual' && t('agents.selection.manualDescription')}
        </p>
      </section>

      {/* Agent List */}
      <section className="space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-medium text-gray-900 dark:text-white">{t('agents.configuredAgents')}</h3>
          <button
            onClick={() => setIsAddDialogOpen(true)}
            className={clsx(
              'inline-flex items-center gap-1 px-3 py-1.5 rounded-lg text-sm',
              'bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-400',
              'hover:bg-primary-200 dark:hover:bg-primary-900/50',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
            )}
          >
            <PlusIcon className="w-4 h-4" />
            {t('agents.addAgent')}
          </button>
        </div>

        <div className="space-y-2">
          {agents.map((agent) => (
            <div
              key={agent.name}
              className={clsx(
                'flex items-center justify-between p-4 rounded-lg border',
                'border-gray-200 dark:border-gray-700',
                'bg-white dark:bg-gray-800',
              )}
            >
              <div className="flex items-center gap-4">
                <Switch.Root
                  checked={agent.enabled}
                  onCheckedChange={(checked) => handleToggleEnabled(agent.name, checked)}
                  className={clsx(
                    'w-10 h-6 rounded-full relative',
                    'bg-gray-200 dark:bg-gray-700',
                    'data-[state=checked]:bg-primary-600',
                    'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
                    'dark:focus:ring-offset-gray-800',
                  )}
                >
                  <Switch.Thumb
                    className={clsx(
                      'block w-5 h-5 bg-white rounded-full shadow',
                      'transition-transform',
                      'data-[state=checked]:translate-x-[18px]',
                      'data-[state=unchecked]:translate-x-[2px]',
                    )}
                  />
                </Switch.Root>

                <div>
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-gray-900 dark:text-white">{agent.name}</span>
                    {agent.isDefault && (
                      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400">
                        <StarFilledIcon className="w-3 h-3" /> {t('agents.default')}
                      </span>
                    )}
                    {!agent.enabled && (
                      <span className="text-xs text-gray-400 dark:text-gray-500">{t('agents.disabled')}</span>
                    )}
                  </div>
                  <div className="text-sm text-gray-500 dark:text-gray-400">
                    {t('agents.commandLabel')}{' '}
                    <code className="text-xs bg-gray-100 dark:bg-gray-700 px-1 py-0.5 rounded">{agent.command}</code>
                  </div>
                </div>
              </div>

              <div className="flex items-center gap-2">
                {!agent.isDefault && agent.enabled && (
                  <button
                    onClick={() => handleSetDefault(agent.name)}
                    className={clsx(
                      'px-2 py-1 rounded text-xs',
                      'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-400',
                      'hover:bg-gray-200 dark:hover:bg-gray-600',
                    )}
                  >
                    {t('agents.setDefault')}
                  </button>
                )}
                <button
                  onClick={() => setEditingAgent(agent)}
                  className={clsx(
                    'p-2 rounded-lg',
                    'text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200',
                    'hover:bg-gray-100 dark:hover:bg-gray-700',
                  )}
                  aria-label={`Edit ${agent.name}`}
                >
                  <Pencil1Icon className="w-4 h-4" />
                </button>
                <button
                  onClick={() => setDeleteConfirm(agent.name)}
                  className={clsx(
                    'p-2 rounded-lg',
                    'text-gray-500 hover:text-red-600 dark:text-gray-400 dark:hover:text-red-400',
                    'hover:bg-gray-100 dark:hover:bg-gray-700',
                  )}
                  aria-label={`Delete ${agent.name}`}
                >
                  <TrashIcon className="w-4 h-4" />
                </button>
              </div>
            </div>
          ))}

          {agents.length === 0 && (
            <div className="text-center py-8 text-gray-500 dark:text-gray-400">{t('agents.emptyState')}</div>
          )}
        </div>
      </section>

      {/* Add Agent Dialog */}
      <AgentFormDialog
        open={isAddDialogOpen}
        onOpenChange={setIsAddDialogOpen}
        title={t('agents.dialog.addTitle')}
        onSubmit={handleAddAgent}
        t={t}
      />

      {/* Edit Agent Dialog */}
      <AgentFormDialog
        open={!!editingAgent}
        onOpenChange={(open) => !open && setEditingAgent(null)}
        title={t('agents.dialog.editTitle')}
        initialData={editingAgent || undefined}
        onSubmit={handleEditAgent}
        isEdit
        t={t}
      />

      {/* Delete Confirmation Dialog */}
      <Dialog.Root open={!!deleteConfirm} onOpenChange={(open) => !open && setDeleteConfirm(null)}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm" />
          <Dialog.Content
            className={clsx(
              'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
              'w-full max-w-md p-6',
              'bg-white dark:bg-gray-900 rounded-xl shadow-xl',
              'focus:outline-none',
            )}
          >
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              {t('agents.dialog.deleteTitle')}
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-gray-500 dark:text-gray-400">
              {t('agents.dialog.deleteConfirm', { name: deleteConfirm })}
            </Dialog.Description>
            <div className="mt-6 flex justify-end gap-3">
              <Dialog.Close asChild>
                <button
                  className={clsx(
                    'px-4 py-2 rounded-lg',
                    'bg-gray-100 dark:bg-gray-800',
                    'text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-200 dark:hover:bg-gray-700',
                  )}
                >
                  {t('agents.dialog.cancel')}
                </button>
              </Dialog.Close>
              <button
                onClick={() => deleteConfirm && handleDeleteAgent(deleteConfirm)}
                className={clsx('px-4 py-2 rounded-lg', 'bg-red-600 text-white', 'hover:bg-red-700')}
              >
                {t('agents.dialog.delete')}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </div>
  );
}

interface AgentFormDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  initialData?: AgentFormData;
  onSubmit: (data: AgentFormData) => void;
  isEdit?: boolean;
  t: (key: string) => string;
}

function AgentFormDialog({
  open,
  onOpenChange,
  title,
  initialData,
  onSubmit,
  isEdit = false,
  t,
}: AgentFormDialogProps) {
  const [name, setName] = useState(initialData?.name || '');
  const [command, setCommand] = useState(initialData?.command || '');
  const [enabled, setEnabled] = useState(initialData?.enabled ?? true);

  const resetForm = () => {
    setName(initialData?.name || '');
    setCommand(initialData?.command || '');
    setEnabled(initialData?.enabled ?? true);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !command.trim()) return;
    onSubmit({ name, command, enabled });
    resetForm();
  };

  return (
    <Dialog.Root
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) resetForm();
        onOpenChange(isOpen);
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-md p-6',
            'bg-white dark:bg-gray-900 rounded-xl shadow-xl',
            'focus:outline-none',
          )}
        >
          <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">{title}</Dialog.Title>

          <form onSubmit={handleSubmit} className="mt-4 space-y-4">
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                {t('agents.name')}
              </label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={t('agents.namePlaceholder')}
                disabled={isEdit}
                className={clsx(
                  'w-full px-3 py-2 rounded-lg border',
                  'border-gray-200 dark:border-gray-700',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                )}
              />
            </div>

            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                {t('agents.command')}
              </label>
              <input
                type="text"
                value={command}
                onChange={(e) => setCommand(e.target.value)}
                placeholder={t('agents.commandPlaceholder')}
                className={clsx(
                  'w-full px-3 py-2 rounded-lg border',
                  'border-gray-200 dark:border-gray-700',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
              />
              <p className="mt-1 text-xs text-gray-500 dark:text-gray-400">{t('agents.commandHelp')}</p>
            </div>

            <div className="flex items-center gap-3">
              <Switch.Root
                checked={enabled}
                onCheckedChange={setEnabled}
                className={clsx(
                  'w-10 h-6 rounded-full relative',
                  'bg-gray-200 dark:bg-gray-700',
                  'data-[state=checked]:bg-primary-600',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
                )}
              >
                <Switch.Thumb
                  className={clsx(
                    'block w-5 h-5 bg-white rounded-full shadow',
                    'transition-transform',
                    'data-[state=checked]:translate-x-[18px]',
                    'data-[state=unchecked]:translate-x-[2px]',
                  )}
                />
              </Switch.Root>
              <span className="text-sm text-gray-700 dark:text-gray-300">{t('agents.enabled')}</span>
            </div>

            <div className="flex justify-end gap-3 pt-4">
              <Dialog.Close asChild>
                <button
                  type="button"
                  className={clsx(
                    'px-4 py-2 rounded-lg',
                    'bg-gray-100 dark:bg-gray-800',
                    'text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-200 dark:hover:bg-gray-700',
                  )}
                >
                  {t('agents.dialog.cancel')}
                </button>
              </Dialog.Close>
              <button
                type="submit"
                disabled={!name.trim() || !command.trim()}
                className={clsx(
                  'px-4 py-2 rounded-lg',
                  'bg-primary-600 text-white',
                  'hover:bg-primary-700',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                )}
              >
                {isEdit ? t('agents.dialog.save') : t('agents.dialog.add')}
              </button>
            </div>
          </form>

          <Dialog.Close asChild>
            <button
              className={clsx('absolute top-4 right-4 p-1 rounded-lg', 'hover:bg-gray-100 dark:hover:bg-gray-800')}
              aria-label={t('agents.dialog.close')}
            >
              <Cross2Icon className="w-4 h-4 text-gray-500 dark:text-gray-400" />
            </button>
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default AgentConfigSection;
