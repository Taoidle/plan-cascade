/**
 * SettingsDialog Component
 *
 * Main settings dialog with tabbed navigation for different settings sections.
 */

import * as Dialog from '@radix-ui/react-dialog';
import * as Tabs from '@radix-ui/react-tabs';
import { Cross2Icon } from '@radix-ui/react-icons';
import { clsx } from 'clsx';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../../store/settings';

// Section components (to be implemented)
import { GeneralSection } from './GeneralSection';
import { LLMBackendSection } from './LLMBackendSection';
import { AgentConfigSection } from './AgentConfigSection';
import { QualityGatesSection } from './QualityGatesSection';
import { PhaseAgentSection } from './PhaseAgentSection';
import { ImportExportSection } from './ImportExportSection';
import { EmbeddingSection } from './EmbeddingSection';
import { GuardrailSection } from './GuardrailSection';
import { LspSection } from './LspSection';
import { PluginSection } from './PluginSection';
import { NetworkSection } from './NetworkSection';
import { WebhookSection } from './WebhookSection';
import { RemoteSection } from './RemoteSection';
import { A2aSection } from './A2aSection';

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type SettingsTab = 'general' | 'llm' | 'embedding' | 'lsp' | 'network' | 'notifications' | 'remote' | 'plugins' | 'security' | 'agents' | 'quality' | 'phases' | 'a2a' | 'import-export';

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const { t } = useTranslation('settings');
  const [activeTab, setActiveTab] = useState<SettingsTab>('general');
  const [isSaving, setIsSaving] = useState(false);

  const tabs: { id: SettingsTab; label: string }[] = [
    { id: 'general', label: t('tabs.general') },
    { id: 'llm', label: t('tabs.llm') },
    { id: 'embedding', label: t('tabs.embedding') },
    { id: 'lsp', label: t('tabs.lsp') },
    { id: 'network', label: t('tabs.network') },
    { id: 'notifications', label: t('tabs.notifications', 'Notifications') },
    { id: 'remote', label: t('tabs.remote', 'Remote') },
    { id: 'plugins', label: t('tabs.plugins', 'Plugins') },
    { id: 'security', label: t('tabs.security') },
    { id: 'agents', label: t('tabs.agents') },
    { id: 'quality', label: t('tabs.quality') },
    { id: 'phases', label: t('tabs.phases') },
    { id: 'a2a', label: t('tabs.a2a', 'A2A Agents') },
    { id: 'import-export', label: t('tabs.importExport') },
  ];

  const handleSave = async () => {
    setIsSaving(true);
    try {
      // Save settings to backend
      await saveSettingsToBackend();
      onOpenChange(false);
    } catch (error) {
      console.error('Failed to save settings:', error);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-4xl max-h-[90vh]',
            'bg-white dark:bg-gray-900 rounded-xl shadow-xl',
            'flex flex-col',
            'focus:outline-none'
          )}
          aria-describedby={undefined}
        >
          {/* Header */}
          <div className="flex items-center justify-between p-6 border-b border-gray-200 dark:border-gray-800">
            <Dialog.Title className="text-xl font-semibold text-gray-900 dark:text-white">
              {t('title')}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button
                className={clsx(
                  'p-2 rounded-lg',
                  'hover:bg-gray-100 dark:hover:bg-gray-800',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500'
                )}
                aria-label="Close"
              >
                <Cross2Icon className="w-5 h-5 text-gray-500 dark:text-gray-400" />
              </button>
            </Dialog.Close>
          </div>

          {/* Tabs */}
          <Tabs.Root
            value={activeTab}
            onValueChange={(value) => setActiveTab(value as SettingsTab)}
            className="flex flex-1 overflow-hidden"
          >
            {/* Tab List (Sidebar) */}
            <Tabs.List
              className={clsx(
                'w-48 shrink-0 border-r border-gray-200 dark:border-gray-800',
                'bg-gray-50 dark:bg-gray-950',
                'p-2 space-y-1'
              )}
            >
              {tabs.map((tab) => (
                <Tabs.Trigger
                  key={tab.id}
                  value={tab.id}
                  className={clsx(
                    'w-full px-3 py-2 rounded-lg text-left text-sm',
                    'transition-colors',
                    'data-[state=active]:bg-primary-100 dark:data-[state=active]:bg-primary-900',
                    'data-[state=active]:text-primary-700 dark:data-[state=active]:text-primary-300',
                    'data-[state=inactive]:text-gray-600 dark:data-[state=inactive]:text-gray-400',
                    'data-[state=inactive]:hover:bg-gray-100 dark:data-[state=inactive]:hover:bg-gray-800'
                  )}
                >
                  {tab.label}
                </Tabs.Trigger>
              ))}
            </Tabs.List>

            {/* Tab Content */}
            <div className="flex-1 overflow-auto p-6">
              <Tabs.Content value="general" className="outline-none">
                <GeneralSection onCloseDialog={() => onOpenChange(false)} />
              </Tabs.Content>
              <Tabs.Content value="llm" className="outline-none">
                <LLMBackendSection />
              </Tabs.Content>
              <Tabs.Content value="embedding" className="outline-none">
                <EmbeddingSection />
              </Tabs.Content>
              <Tabs.Content value="lsp" className="outline-none">
                <LspSection />
              </Tabs.Content>
              <Tabs.Content value="network" className="outline-none">
                <NetworkSection />
              </Tabs.Content>
              <Tabs.Content value="notifications" className="outline-none">
                <WebhookSection />
              </Tabs.Content>
              <Tabs.Content value="remote" className="outline-none">
                <RemoteSection />
              </Tabs.Content>
              <Tabs.Content value="plugins" className="outline-none">
                <PluginSection />
              </Tabs.Content>
              <Tabs.Content value="security" className="outline-none">
                <GuardrailSection />
              </Tabs.Content>
              <Tabs.Content value="agents" className="outline-none">
                <AgentConfigSection />
              </Tabs.Content>
              <Tabs.Content value="quality" className="outline-none">
                <QualityGatesSection />
              </Tabs.Content>
              <Tabs.Content value="phases" className="outline-none">
                <PhaseAgentSection />
              </Tabs.Content>
              <Tabs.Content value="a2a" className="outline-none">
                <A2aSection />
              </Tabs.Content>
              <Tabs.Content value="import-export" className="outline-none">
                <ImportExportSection />
              </Tabs.Content>
            </div>
          </Tabs.Root>

          {/* Footer */}
          <div className="flex items-center justify-end gap-3 p-6 border-t border-gray-200 dark:border-gray-800">
            <Dialog.Close asChild>
              <button
                className={clsx(
                  'px-4 py-2 rounded-lg',
                  'bg-gray-100 dark:bg-gray-800',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500'
                )}
              >
                {t('buttons.cancel', { ns: 'common' })}
              </button>
            </Dialog.Close>
            <button
              onClick={handleSave}
              disabled={isSaving}
              className={clsx(
                'px-4 py-2 rounded-lg',
                'bg-primary-600 text-white',
                'hover:bg-primary-700',
                'focus:outline-none focus:ring-2 focus:ring-primary-500',
                'disabled:opacity-50 disabled:cursor-not-allowed'
              )}
            >
              {isSaving ? t('buttons.saving', { ns: 'common' }) : t('buttons.save', { ns: 'common' })}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

async function saveSettingsToBackend() {
  // Get settings from store
  const settings = useSettingsStore.getState();

  // Save to Tauri backend using invoke
  try {
    const { updateSettings, isTauriAvailable } = await import('../../lib/settingsApi');

    if (isTauriAvailable()) {
      await updateSettings({
        theme: settings.theme,
        default_provider: settings.provider,
        default_model: settings.model,
      });
    }
  } catch (error) {
    console.warn('Tauri settings save failed, settings saved locally:', error);
  }

  // Always save to local storage as backup
  localStorage.setItem(
    'plan-cascade-settings',
    JSON.stringify({
      backend: settings.backend,
      provider: settings.provider,
      model: settings.model,
      defaultMode: settings.defaultMode,
      theme: settings.theme,
    })
  );
}

export default SettingsDialog;
