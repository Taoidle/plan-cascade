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
import { useSettingsStore } from '../../store/settings';

// Section components (to be implemented)
import { GeneralSection } from './GeneralSection';
import { LLMBackendSection } from './LLMBackendSection';
import { AgentConfigSection } from './AgentConfigSection';
import { QualityGatesSection } from './QualityGatesSection';
import { PhaseAgentSection } from './PhaseAgentSection';
import { ImportExportSection } from './ImportExportSection';

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type SettingsTab = 'general' | 'llm' | 'agents' | 'quality' | 'phases' | 'import-export';

const tabs: { id: SettingsTab; label: string }[] = [
  { id: 'general', label: 'General' },
  { id: 'llm', label: 'LLM Backend' },
  { id: 'agents', label: 'Agents' },
  { id: 'quality', label: 'Quality Gates' },
  { id: 'phases', label: 'Phase Agents' },
  { id: 'import-export', label: 'Import/Export' },
];

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>('general');
  const [isSaving, setIsSaving] = useState(false);

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
        >
          {/* Header */}
          <div className="flex items-center justify-between p-6 border-b border-gray-200 dark:border-gray-800">
            <Dialog.Title className="text-xl font-semibold text-gray-900 dark:text-white">
              Settings
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
                <Cross2Icon className="w-5 h-5 text-gray-500" />
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
                <GeneralSection />
              </Tabs.Content>
              <Tabs.Content value="llm" className="outline-none">
                <LLMBackendSection />
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
                Cancel
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
              {isSaving ? 'Saving...' : 'Save'}
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

  // Prepare payload for backend
  const payload = {
    backend: settings.backend,
    provider: settings.provider,
    model: settings.model,
    agents: settings.agents.map((a) => ({
      name: a.name,
      enabled: a.enabled,
      command: a.command,
      is_default: a.isDefault,
    })),
    agent_selection: settings.agentSelection,
    default_agent: settings.defaultAgent,
    quality_gates: {
      typecheck: settings.qualityGates.typecheck,
      test: settings.qualityGates.test,
      lint: settings.qualityGates.lint,
      custom: settings.qualityGates.custom,
      custom_script: settings.qualityGates.customScript,
      max_retries: settings.qualityGates.maxRetries,
    },
    max_parallel_stories: settings.maxParallelStories,
    max_iterations: settings.maxIterations,
    timeout_seconds: settings.timeoutSeconds,
    default_mode: settings.defaultMode,
    theme: settings.theme,
  };

  const response = await fetch('http://127.0.0.1:8765/api/settings', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });

  if (!response.ok) {
    throw new Error('Failed to save settings');
  }

  return response.json();
}

export default SettingsDialog;
