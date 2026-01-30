/**
 * ImportExportSection Component
 *
 * Configuration import/export functionality.
 */

import { useState, useRef } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { clsx } from 'clsx';
import { DownloadIcon, UploadIcon, Cross2Icon, CheckCircledIcon, ExclamationTriangleIcon } from '@radix-ui/react-icons';
import { useSettingsStore } from '../../store/settings';

type MessageType = 'success' | 'error' | 'warning';

interface StatusMessage {
  type: MessageType;
  text: string;
}

export function ImportExportSection() {
  const [isExporting, setIsExporting] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const [message, setMessage] = useState<StatusMessage | null>(null);
  const [importPreview, setImportPreview] = useState<object | null>(null);
  const [showConfirmDialog, setShowConfirmDialog] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleExport = async () => {
    setIsExporting(true);
    setMessage(null);

    try {
      // Fetch settings from backend to get the complete config
      const response = await fetch('http://127.0.0.1:8765/api/settings/export');
      if (!response.ok) {
        throw new Error('Failed to export settings');
      }

      const data = await response.json();

      // Create blob and download
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = `plan-cascade-settings-${new Date().toISOString().split('T')[0]}.json`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);

      setMessage({ type: 'success', text: 'Settings exported successfully' });
    } catch (error) {
      console.error('Export failed:', error);
      setMessage({ type: 'error', text: 'Failed to export settings. Please try again.' });
    } finally {
      setIsExporting(false);
    }
  };

  const handleFileSelect = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (e) => {
      try {
        const content = e.target?.result as string;
        const parsed = JSON.parse(content);

        // Basic validation
        if (!parsed.version || !parsed.settings) {
          setMessage({
            type: 'error',
            text: 'Invalid settings file format. Expected "version" and "settings" fields.',
          });
          return;
        }

        setImportPreview(parsed);
        setShowConfirmDialog(true);
      } catch (error) {
        setMessage({ type: 'error', text: 'Failed to parse settings file. Please ensure it is valid JSON.' });
      }
    };
    reader.readAsText(file);

    // Reset file input
    if (fileInputRef.current) {
      fileInputRef.current.value = '';
    }
  };

  const handleImportConfirm = async () => {
    if (!importPreview) return;

    setIsImporting(true);
    setMessage(null);

    try {
      const response = await fetch('http://127.0.0.1:8765/api/settings/import', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(importPreview),
      });

      if (!response.ok) {
        throw new Error('Failed to import settings');
      }

      await response.json();

      // Update frontend store with imported settings
      const { settings } = importPreview as { settings: Record<string, unknown> };
      syncSettingsToStore(settings);

      setMessage({ type: 'success', text: 'Settings imported successfully' });
      setShowConfirmDialog(false);
      setImportPreview(null);
    } catch (error) {
      console.error('Import failed:', error);
      setMessage({ type: 'error', text: 'Failed to import settings. Please try again.' });
    } finally {
      setIsImporting(false);
    }
  };

  const handleReset = async () => {
    setMessage(null);

    try {
      const response = await fetch('http://127.0.0.1:8765/api/settings/reset', {
        method: 'POST',
      });

      if (!response.ok) {
        throw new Error('Failed to reset settings');
      }

      // Reset frontend store
      useSettingsStore.getState().resetToDefaults();

      setMessage({ type: 'success', text: 'Settings reset to defaults' });
    } catch (error) {
      console.error('Reset failed:', error);
      setMessage({ type: 'error', text: 'Failed to reset settings. Please try again.' });
    }
  };

  return (
    <div className="space-y-8">
      <div>
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-1">
          Import & Export
        </h2>
        <p className="text-sm text-gray-500 dark:text-gray-400">
          Backup your settings or transfer them to another installation.
        </p>
      </div>

      {/* Status Message */}
      {message && (
        <div
          className={clsx(
            'flex items-center gap-3 p-4 rounded-lg',
            message.type === 'success' && 'bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800',
            message.type === 'error' && 'bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800',
            message.type === 'warning' && 'bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800'
          )}
        >
          {message.type === 'success' && <CheckCircledIcon className="w-5 h-5 text-green-600 dark:text-green-400" />}
          {message.type === 'error' && <Cross2Icon className="w-5 h-5 text-red-600 dark:text-red-400" />}
          {message.type === 'warning' && <ExclamationTriangleIcon className="w-5 h-5 text-yellow-600 dark:text-yellow-400" />}
          <span
            className={clsx(
              'text-sm',
              message.type === 'success' && 'text-green-700 dark:text-green-300',
              message.type === 'error' && 'text-red-700 dark:text-red-300',
              message.type === 'warning' && 'text-yellow-700 dark:text-yellow-300'
            )}
          >
            {message.text}
          </span>
        </div>
      )}

      {/* Export Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          Export Settings
        </h3>
        <div
          className={clsx(
            'p-6 rounded-lg border-2 border-dashed',
            'border-gray-200 dark:border-gray-700',
            'text-center'
          )}
        >
          <DownloadIcon className="w-12 h-12 mx-auto text-gray-400 dark:text-gray-500 mb-4" />
          <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
            Download your current settings as a JSON file for backup or transfer.
          </p>
          <button
            onClick={handleExport}
            disabled={isExporting}
            className={clsx(
              'inline-flex items-center gap-2 px-4 py-2 rounded-lg',
              'bg-primary-600 text-white',
              'hover:bg-primary-700',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
              'disabled:opacity-50 disabled:cursor-not-allowed'
            )}
          >
            <DownloadIcon className="w-4 h-4" />
            {isExporting ? 'Exporting...' : 'Export Settings'}
          </button>
        </div>
      </section>

      {/* Import Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          Import Settings
        </h3>
        <div
          className={clsx(
            'p-6 rounded-lg border-2 border-dashed',
            'border-gray-200 dark:border-gray-700',
            'text-center'
          )}
        >
          <UploadIcon className="w-12 h-12 mx-auto text-gray-400 dark:text-gray-500 mb-4" />
          <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
            Import settings from a previously exported JSON file.
          </p>
          <input
            ref={fileInputRef}
            type="file"
            accept=".json,application/json"
            onChange={handleFileSelect}
            className="hidden"
            id="import-file"
          />
          <label
            htmlFor="import-file"
            className={clsx(
              'inline-flex items-center gap-2 px-4 py-2 rounded-lg cursor-pointer',
              'bg-gray-100 dark:bg-gray-800',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
              'focus:outline-none focus:ring-2 focus:ring-primary-500'
            )}
          >
            <UploadIcon className="w-4 h-4" />
            Choose File
          </label>
        </div>
      </section>

      {/* Reset Section */}
      <section className="space-y-4">
        <h3 className="text-sm font-medium text-gray-900 dark:text-white">
          Reset to Defaults
        </h3>
        <div
          className={clsx(
            'p-4 rounded-lg',
            'bg-yellow-50 dark:bg-yellow-900/10',
            'border border-yellow-200 dark:border-yellow-800'
          )}
        >
          <div className="flex items-start gap-3">
            <ExclamationTriangleIcon className="w-5 h-5 text-yellow-600 dark:text-yellow-500 shrink-0 mt-0.5" />
            <div className="flex-1">
              <p className="text-sm text-yellow-800 dark:text-yellow-200 mb-3">
                This will reset all settings to their default values. This action cannot be undone.
              </p>
              <button
                onClick={handleReset}
                className={clsx(
                  'px-3 py-1.5 rounded-lg text-sm',
                  'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300',
                  'hover:bg-yellow-200 dark:hover:bg-yellow-900/50',
                  'focus:outline-none focus:ring-2 focus:ring-yellow-500'
                )}
              >
                Reset All Settings
              </button>
            </div>
          </div>
        </div>
      </section>

      {/* Import Confirmation Dialog */}
      <Dialog.Root open={showConfirmDialog} onOpenChange={setShowConfirmDialog}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm" />
          <Dialog.Content
            className={clsx(
              'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
              'w-full max-w-lg max-h-[85vh] overflow-auto',
              'bg-white dark:bg-gray-900 rounded-xl shadow-xl',
              'p-6',
              'focus:outline-none'
            )}
          >
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              Import Settings
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-gray-500 dark:text-gray-400">
              This will overwrite your current settings. Are you sure you want to continue?
            </Dialog.Description>

            {/* Preview */}
            {importPreview && (
              <div className="mt-4">
                <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                  Settings Preview
                </h4>
                <div
                  className={clsx(
                    'max-h-48 overflow-auto p-3 rounded-lg',
                    'bg-gray-50 dark:bg-gray-800',
                    'border border-gray-200 dark:border-gray-700',
                    'text-xs font-mono text-gray-600 dark:text-gray-400'
                  )}
                >
                  <pre>{JSON.stringify((importPreview as { settings: object }).settings, null, 2)}</pre>
                </div>
              </div>
            )}

            <div className="mt-6 flex justify-end gap-3">
              <Dialog.Close asChild>
                <button
                  className={clsx(
                    'px-4 py-2 rounded-lg',
                    'bg-gray-100 dark:bg-gray-800',
                    'text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-200 dark:hover:bg-gray-700'
                  )}
                >
                  Cancel
                </button>
              </Dialog.Close>
              <button
                onClick={handleImportConfirm}
                disabled={isImporting}
                className={clsx(
                  'px-4 py-2 rounded-lg',
                  'bg-primary-600 text-white',
                  'hover:bg-primary-700',
                  'disabled:opacity-50 disabled:cursor-not-allowed'
                )}
              >
                {isImporting ? 'Importing...' : 'Import'}
              </button>
            </div>

            <Dialog.Close asChild>
              <button
                className={clsx(
                  'absolute top-4 right-4 p-1 rounded-lg',
                  'hover:bg-gray-100 dark:hover:bg-gray-800'
                )}
                aria-label="Close"
              >
                <Cross2Icon className="w-4 h-4 text-gray-500" />
              </button>
            </Dialog.Close>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </div>
  );
}

function syncSettingsToStore(settings: Record<string, unknown>) {
  const store = useSettingsStore.getState();

  // Map backend field names to store field names and update
  if (settings.backend) store.setBackend(settings.backend as Parameters<typeof store.setBackend>[0]);
  if (settings.provider) store.setProvider(settings.provider as string);
  if (settings.model) store.setModel(settings.model as string);
  if (settings.theme) store.setTheme(settings.theme as 'system' | 'light' | 'dark');
  if (settings.default_mode) store.setDefaultMode(settings.default_mode as 'simple' | 'expert');

  // Handle complex objects
  if (settings.agents && Array.isArray(settings.agents)) {
    useSettingsStore.setState({
      agents: settings.agents.map((a: Record<string, unknown>) => ({
        name: a.name as string,
        enabled: a.enabled as boolean,
        command: a.command as string,
        isDefault: a.is_default as boolean,
      })),
    });
  }

  if (settings.quality_gates && typeof settings.quality_gates === 'object') {
    const qg = settings.quality_gates as Record<string, unknown>;
    store.updateQualityGates({
      typecheck: qg.typecheck as boolean,
      test: qg.test as boolean,
      lint: qg.lint as boolean,
      custom: qg.custom as boolean,
      customScript: qg.custom_script as string,
      maxRetries: qg.max_retries as number,
    });
  }

  if (settings.agent_selection) {
    useSettingsStore.setState({
      agentSelection: settings.agent_selection as 'smart' | 'prefer_default' | 'manual',
    });
  }

  if (settings.default_agent) {
    useSettingsStore.setState({ defaultAgent: settings.default_agent as string });
  }

  if (typeof settings.max_parallel_stories === 'number') {
    useSettingsStore.setState({ maxParallelStories: settings.max_parallel_stories });
  }

  if (typeof settings.max_iterations === 'number') {
    useSettingsStore.setState({ maxIterations: settings.max_iterations });
  }

  if (typeof settings.timeout_seconds === 'number') {
    useSettingsStore.setState({ timeoutSeconds: settings.timeout_seconds });
  }
}

export default ImportExportSection;
