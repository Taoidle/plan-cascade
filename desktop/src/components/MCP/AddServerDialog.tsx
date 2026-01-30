/**
 * AddServerDialog Component
 *
 * Dialog for adding a new MCP server.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import * as Dialog from '@radix-ui/react-dialog';
import { Cross2Icon, PlusIcon, TrashIcon } from '@radix-ui/react-icons';
import type { McpServer, McpServerType, CommandResponse } from '../../types/mcp';

interface AddServerDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onServerAdded: (server: McpServer) => void;
}

export function AddServerDialog({ open, onOpenChange, onServerAdded }: AddServerDialogProps) {
  const { t } = useTranslation();
  const [serverType, setServerType] = useState<McpServerType>('stdio');
  const [name, setName] = useState('');
  const [command, setCommand] = useState('');
  const [args, setArgs] = useState('');
  const [url, setUrl] = useState('');
  const [envVars, setEnvVars] = useState<Array<{ key: string; value: string }>>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const resetForm = () => {
    setServerType('stdio');
    setName('');
    setCommand('');
    setArgs('');
    setUrl('');
    setEnvVars([]);
    setError(null);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);

    try {
      // Build env map
      const env: Record<string, string> = {};
      envVars.forEach((v) => {
        if (v.key.trim()) {
          env[v.key.trim()] = v.value;
        }
      });

      const response = await invoke<CommandResponse<McpServer>>('add_mcp_server', {
        name,
        serverType: serverType,
        command: serverType === 'stdio' ? command : null,
        args: serverType === 'stdio' ? args.split(/\s+/).filter(Boolean) : null,
        env: serverType === 'stdio' && Object.keys(env).length > 0 ? env : null,
        url: serverType === 'sse' ? url : null,
        headers: null,
      });

      if (response.success && response.data) {
        onServerAdded(response.data);
        resetForm();
      } else {
        setError(response.error || 'Failed to add server');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to add server');
    } finally {
      setLoading(false);
    }
  };

  const addEnvVar = () => {
    setEnvVars([...envVars, { key: '', value: '' }]);
  };

  const removeEnvVar = (index: number) => {
    setEnvVars(envVars.filter((_, i) => i !== index));
  };

  const updateEnvVar = (index: number, field: 'key' | 'value', value: string) => {
    const updated = [...envVars];
    updated[index][field] = value;
    setEnvVars(updated);
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 animate-in fade-in-0" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-md max-h-[85vh] overflow-y-auto',
            'bg-white dark:bg-gray-900 rounded-lg shadow-xl',
            'animate-in fade-in-0 zoom-in-95',
            'focus:outline-none'
          )}
        >
          <form onSubmit={handleSubmit}>
            {/* Header */}
            <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
              <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
                {t('mcp.addServerTitle')}
              </Dialog.Title>
              <Dialog.Close asChild>
                <button
                  type="button"
                  className="p-1 rounded-md hover:bg-gray-100 dark:hover:bg-gray-800"
                >
                  <Cross2Icon className="w-5 h-5" />
                </button>
              </Dialog.Close>
            </div>

            {/* Body */}
            <div className="p-4 space-y-4">
              {/* Server Type */}
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  {t('mcp.serverType')}
                </label>
                <div className="flex gap-2">
                  <button
                    type="button"
                    onClick={() => setServerType('stdio')}
                    className={clsx(
                      'flex-1 py-2 px-3 rounded-md text-sm font-medium transition-colors',
                      serverType === 'stdio'
                        ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-700 dark:text-primary-300 border-2 border-primary-500'
                        : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 border-2 border-transparent'
                    )}
                  >
                    Stdio
                  </button>
                  <button
                    type="button"
                    onClick={() => setServerType('sse')}
                    className={clsx(
                      'flex-1 py-2 px-3 rounded-md text-sm font-medium transition-colors',
                      serverType === 'sse'
                        ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-700 dark:text-primary-300 border-2 border-primary-500'
                        : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 border-2 border-transparent'
                    )}
                  >
                    SSE
                  </button>
                </div>
              </div>

              {/* Name */}
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  {t('mcp.serverName')}
                </label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="My MCP Server"
                  required
                  className={clsx(
                    'w-full px-3 py-2 rounded-md',
                    'bg-gray-100 dark:bg-gray-800',
                    'border border-gray-200 dark:border-gray-700',
                    'text-gray-900 dark:text-white',
                    'focus:outline-none focus:ring-2 focus:ring-primary-500'
                  )}
                />
              </div>

              {/* Stdio Fields */}
              {serverType === 'stdio' && (
                <>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('mcp.command')}
                    </label>
                    <input
                      type="text"
                      value={command}
                      onChange={(e) => setCommand(e.target.value)}
                      placeholder="node"
                      required
                      className={clsx(
                        'w-full px-3 py-2 rounded-md font-mono text-sm',
                        'bg-gray-100 dark:bg-gray-800',
                        'border border-gray-200 dark:border-gray-700',
                        'text-gray-900 dark:text-white',
                        'focus:outline-none focus:ring-2 focus:ring-primary-500'
                      )}
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('mcp.arguments')}
                    </label>
                    <input
                      type="text"
                      value={args}
                      onChange={(e) => setArgs(e.target.value)}
                      placeholder="server.js --port 3000"
                      className={clsx(
                        'w-full px-3 py-2 rounded-md font-mono text-sm',
                        'bg-gray-100 dark:bg-gray-800',
                        'border border-gray-200 dark:border-gray-700',
                        'text-gray-900 dark:text-white',
                        'focus:outline-none focus:ring-2 focus:ring-primary-500'
                      )}
                    />
                  </div>

                  {/* Environment Variables */}
                  <div>
                    <div className="flex items-center justify-between mb-1">
                      <label className="text-sm font-medium text-gray-700 dark:text-gray-300">
                        {t('mcp.envVariables')}
                      </label>
                      <button
                        type="button"
                        onClick={addEnvVar}
                        className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
                      >
                        <PlusIcon className="w-3 h-3 inline mr-1" />
                        {t('mcp.addEnvVar')}
                      </button>
                    </div>
                    <div className="space-y-2">
                      {envVars.map((env, i) => (
                        <div key={i} className="flex gap-2">
                          <input
                            type="text"
                            value={env.key}
                            onChange={(e) => updateEnvVar(i, 'key', e.target.value)}
                            placeholder="KEY"
                            className={clsx(
                              'flex-1 px-2 py-1.5 rounded-md font-mono text-xs',
                              'bg-gray-100 dark:bg-gray-800',
                              'border border-gray-200 dark:border-gray-700'
                            )}
                          />
                          <input
                            type="text"
                            value={env.value}
                            onChange={(e) => updateEnvVar(i, 'value', e.target.value)}
                            placeholder="value"
                            className={clsx(
                              'flex-1 px-2 py-1.5 rounded-md font-mono text-xs',
                              'bg-gray-100 dark:bg-gray-800',
                              'border border-gray-200 dark:border-gray-700'
                            )}
                          />
                          <button
                            type="button"
                            onClick={() => removeEnvVar(i)}
                            className="p-1.5 text-red-500 hover:bg-red-100 dark:hover:bg-red-900/30 rounded"
                          >
                            <TrashIcon className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      ))}
                    </div>
                  </div>
                </>
              )}

              {/* SSE Fields */}
              {serverType === 'sse' && (
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                    {t('mcp.serverUrl')}
                  </label>
                  <input
                    type="url"
                    value={url}
                    onChange={(e) => setUrl(e.target.value)}
                    placeholder="http://localhost:8080/sse"
                    required
                    className={clsx(
                      'w-full px-3 py-2 rounded-md font-mono text-sm',
                      'bg-gray-100 dark:bg-gray-800',
                      'border border-gray-200 dark:border-gray-700',
                      'text-gray-900 dark:text-white',
                      'focus:outline-none focus:ring-2 focus:ring-primary-500'
                    )}
                  />
                </div>
              )}

              {/* Error */}
              {error && (
                <div className="p-3 rounded-md bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 text-sm">
                  {error}
                </div>
              )}
            </div>

            {/* Footer */}
            <div className="flex justify-end gap-2 p-4 border-t border-gray-200 dark:border-gray-700">
              <Dialog.Close asChild>
                <button
                  type="button"
                  className={clsx(
                    'px-4 py-2 rounded-md',
                    'bg-gray-100 dark:bg-gray-800',
                    'hover:bg-gray-200 dark:hover:bg-gray-700',
                    'text-sm font-medium'
                  )}
                >
                  {t('common.cancel')}
                </button>
              </Dialog.Close>
              <button
                type="submit"
                disabled={loading}
                className={clsx(
                  'px-4 py-2 rounded-md',
                  'bg-primary-600 hover:bg-primary-700',
                  'text-white text-sm font-medium',
                  'disabled:opacity-50'
                )}
              >
                {loading ? t('common.adding') : t('mcp.addServer')}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default AddServerDialog;
