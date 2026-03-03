/**
 * AddServerDialog Component
 *
 * Dialog for adding or editing an MCP server.
 */

import { useEffect, useState } from 'react';
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
  onServerUpdated?: (server: McpServer) => void;
  server?: McpServer | null;
}

export function AddServerDialog({ open, onOpenChange, onServerAdded, onServerUpdated, server }: AddServerDialogProps) {
  const { t } = useTranslation();
  const [serverType, setServerType] = useState<McpServerType>('stdio');
  const [name, setName] = useState('');
  const [command, setCommand] = useState('');
  const [args, setArgs] = useState('');
  const [url, setUrl] = useState('');
  const [autoConnect, setAutoConnect] = useState(true);
  const [envVars, setEnvVars] = useState<Array<{ key: string; value: string }>>([]);
  const [headerVars, setHeaderVars] = useState<Array<{ key: string; value: string }>>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isEditMode = !!server;

  const resetForm = () => {
    setServerType('stdio');
    setName('');
    setCommand('');
    setArgs('');
    setUrl('');
    setAutoConnect(true);
    setEnvVars([]);
    setHeaderVars([]);
    setError(null);
  };

  useEffect(() => {
    if (!open) return;

    if (server) {
      setServerType(server.server_type);
      setName(server.name);
      setCommand(server.command ?? '');
      setArgs(server.args.join(' '));
      setUrl(server.url ?? '');
      setAutoConnect(server.auto_connect ?? true);
      setEnvVars(Object.entries(server.env ?? {}).map(([key, value]) => ({ key, value })));
      setHeaderVars(Object.entries(server.headers ?? {}).map(([key, value]) => ({ key, value })));
      setError(null);
    } else {
      resetForm();
    }
  }, [open, server]);

  const parseArgs = (raw: string): string[] => {
    const tokens = raw.match(/[^\s"']+|"([^"]*)"|'([^']*)'/g) || [];
    return tokens.map((token) => {
      if ((token.startsWith('"') && token.endsWith('"')) || (token.startsWith("'") && token.endsWith("'"))) {
        return token.slice(1, -1);
      }
      return token;
    });
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);

    try {
      const env: Record<string, string> = {};
      envVars.forEach((v) => {
        if (v.key.trim()) {
          env[v.key.trim()] = v.value;
        }
      });

      const headers: Record<string, string> = {};
      headerVars.forEach((v) => {
        if (v.key.trim()) {
          headers[v.key.trim()] = v.value;
        }
      });

      if (isEditMode && server) {
        const response = await invoke<CommandResponse<McpServer>>('update_mcp_server', {
          id: server.id,
          name,
          serverType,
          command: serverType === 'stdio' ? command : null,
          clearCommand: serverType !== 'stdio',
          args: serverType === 'stdio' ? parseArgs(args) : [],
          env: serverType === 'stdio' ? env : {},
          url: serverType === 'stream_http' ? url : null,
          clearUrl: serverType !== 'stream_http',
          headers: serverType === 'stream_http' ? headers : {},
          autoConnect,
        });

        if (response.success && response.data) {
          onServerUpdated?.(response.data);
          onOpenChange(false);
        } else {
          setError(response.error || t('mcp.errors.addServer'));
        }
      } else {
        const response = await invoke<CommandResponse<McpServer>>('add_mcp_server', {
          name,
          serverType,
          command: serverType === 'stdio' ? command : null,
          args: serverType === 'stdio' ? parseArgs(args) : null,
          env: serverType === 'stdio' && Object.keys(env).length > 0 ? env : null,
          url: serverType === 'stream_http' ? url : null,
          headers: serverType === 'stream_http' && Object.keys(headers).length > 0 ? headers : null,
          autoConnect,
        });

        if (response.success && response.data) {
          onServerAdded(response.data);
          resetForm();
        } else {
          setError(response.error || t('mcp.errors.addServer'));
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : t('mcp.errors.addServer'));
    } finally {
      setLoading(false);
    }
  };

  const addEnvVar = () => {
    setEnvVars([...envVars, { key: '', value: '' }]);
  };

  const addHeaderVar = () => {
    setHeaderVars([...headerVars, { key: '', value: '' }]);
  };

  const removeEnvVar = (index: number) => {
    setEnvVars(envVars.filter((_, i) => i !== index));
  };

  const removeHeaderVar = (index: number) => {
    setHeaderVars(headerVars.filter((_, i) => i !== index));
  };

  const updateEnvVar = (index: number, field: 'key' | 'value', value: string) => {
    const updated = [...envVars];
    updated[index][field] = value;
    setEnvVars(updated);
  };

  const updateHeaderVar = (index: number, field: 'key' | 'value', value: string) => {
    const updated = [...headerVars];
    updated[index][field] = value;
    setHeaderVars(updated);
  };

  const handleOpenChange = (nextOpen: boolean) => {
    if (!nextOpen && !isEditMode) {
      resetForm();
    }
    onOpenChange(nextOpen);
  };

  return (
    <Dialog.Root open={open} onOpenChange={handleOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 animate-in fade-in-0" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-md max-h-[85vh] overflow-y-auto',
            'bg-white dark:bg-gray-900 rounded-lg shadow-xl',
            'animate-in fade-in-0 zoom-in-95',
            'focus:outline-none',
          )}
        >
          <form onSubmit={handleSubmit}>
            <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
              <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
                {isEditMode ? t('mcp.editServerTitle', 'Edit MCP Server') : t('mcp.addServerTitle')}
              </Dialog.Title>
              <Dialog.Close asChild>
                <button type="button" className="p-1 rounded-md hover:bg-gray-100 dark:hover:bg-gray-800">
                  <Cross2Icon className="w-5 h-5 text-gray-500 dark:text-gray-400" />
                </button>
              </Dialog.Close>
            </div>

            <div className="p-4 space-y-4">
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
                        : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 border-2 border-transparent',
                    )}
                  >
                    {t('mcp.serverTypeStdio')}
                  </button>
                  <button
                    type="button"
                    onClick={() => setServerType('stream_http')}
                    className={clsx(
                      'flex-1 py-2 px-3 rounded-md text-sm font-medium transition-colors',
                      serverType === 'stream_http'
                        ? 'bg-primary-100 dark:bg-primary-900/50 text-primary-700 dark:text-primary-300 border-2 border-primary-500'
                        : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 border-2 border-transparent',
                    )}
                  >
                    {t('mcp.serverTypeStreamHttp')}
                  </button>
                </div>
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  {t('mcp.serverName')}
                </label>
                <input
                  type="text"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder={t('mcp.placeholders.serverName')}
                  required
                  className={clsx(
                    'w-full px-3 py-2 rounded-md',
                    'bg-gray-100 dark:bg-gray-800',
                    'border border-gray-200 dark:border-gray-700',
                    'text-gray-900 dark:text-white',
                    'focus:outline-none focus:ring-2 focus:ring-primary-500',
                  )}
                />
              </div>

              <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
                <input
                  type="checkbox"
                  checked={autoConnect}
                  onChange={(e) => setAutoConnect(e.target.checked)}
                  className="rounded border-gray-300 text-primary-600 focus:ring-primary-500"
                />
                <span>{t('mcp.autoConnect', 'Auto-connect on startup')}</span>
              </label>

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
                      placeholder={t('mcp.placeholders.command')}
                      required
                      className={clsx(
                        'w-full px-3 py-2 rounded-md font-mono text-sm',
                        'bg-gray-100 dark:bg-gray-800',
                        'border border-gray-200 dark:border-gray-700',
                        'text-gray-900 dark:text-white',
                        'focus:outline-none focus:ring-2 focus:ring-primary-500',
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
                      placeholder={t('mcp.placeholders.arguments')}
                      className={clsx(
                        'w-full px-3 py-2 rounded-md font-mono text-sm',
                        'bg-gray-100 dark:bg-gray-800',
                        'border border-gray-200 dark:border-gray-700',
                        'text-gray-900 dark:text-white',
                        'focus:outline-none focus:ring-2 focus:ring-primary-500',
                      )}
                    />
                  </div>

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
                            placeholder={t('mcp.placeholders.envKey')}
                            className={clsx(
                              'flex-1 px-2 py-1.5 rounded-md font-mono text-xs',
                              'bg-gray-100 dark:bg-gray-800',
                              'text-gray-900 dark:text-white',
                              'border border-gray-200 dark:border-gray-700',
                            )}
                          />
                          <input
                            type="text"
                            value={env.value}
                            onChange={(e) => updateEnvVar(i, 'value', e.target.value)}
                            placeholder={t('mcp.placeholders.envValue')}
                            className={clsx(
                              'flex-1 px-2 py-1.5 rounded-md font-mono text-xs',
                              'bg-gray-100 dark:bg-gray-800',
                              'text-gray-900 dark:text-white',
                              'border border-gray-200 dark:border-gray-700',
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

              {serverType === 'stream_http' && (
                <>
                  <div>
                    <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('mcp.serverUrl')}
                    </label>
                    <input
                      type="url"
                      value={url}
                      onChange={(e) => setUrl(e.target.value)}
                      placeholder={t('mcp.placeholders.serverUrl')}
                      required
                      className={clsx(
                        'w-full px-3 py-2 rounded-md font-mono text-sm',
                        'bg-gray-100 dark:bg-gray-800',
                        'border border-gray-200 dark:border-gray-700',
                        'text-gray-900 dark:text-white',
                        'focus:outline-none focus:ring-2 focus:ring-primary-500',
                      )}
                    />
                  </div>

                  <div>
                    <div className="flex items-center justify-between mb-1">
                      <label className="text-sm font-medium text-gray-700 dark:text-gray-300">{t('mcp.headers')}</label>
                      <button
                        type="button"
                        onClick={addHeaderVar}
                        className="text-xs text-primary-600 dark:text-primary-400 hover:underline"
                      >
                        <PlusIcon className="w-3 h-3 inline mr-1" />
                        {t('mcp.addHeader')}
                      </button>
                    </div>
                    <div className="space-y-2">
                      {headerVars.map((header, i) => (
                        <div key={i} className="flex gap-2">
                          <input
                            type="text"
                            value={header.key}
                            onChange={(e) => updateHeaderVar(i, 'key', e.target.value)}
                            placeholder={t('mcp.placeholders.headerKey')}
                            className={clsx(
                              'flex-1 px-2 py-1.5 rounded-md font-mono text-xs',
                              'bg-gray-100 dark:bg-gray-800',
                              'text-gray-900 dark:text-white',
                              'border border-gray-200 dark:border-gray-700',
                            )}
                          />
                          <input
                            type="text"
                            value={header.value}
                            onChange={(e) => updateHeaderVar(i, 'value', e.target.value)}
                            placeholder={t('mcp.placeholders.headerValue')}
                            className={clsx(
                              'flex-1 px-2 py-1.5 rounded-md font-mono text-xs',
                              'bg-gray-100 dark:bg-gray-800',
                              'text-gray-900 dark:text-white',
                              'border border-gray-200 dark:border-gray-700',
                            )}
                          />
                          <button
                            type="button"
                            onClick={() => removeHeaderVar(i)}
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

              {error && (
                <div className="p-3 rounded-md bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 text-sm">
                  {error}
                </div>
              )}
            </div>

            <div className="flex justify-end gap-2 p-4 border-t border-gray-200 dark:border-gray-700">
              <Dialog.Close asChild>
                <button
                  type="button"
                  className={clsx(
                    'px-4 py-2 rounded-md',
                    'bg-gray-100 dark:bg-gray-800',
                    'text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-200 dark:hover:bg-gray-700',
                    'text-sm font-medium',
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
                  'disabled:opacity-50',
                )}
              >
                {loading
                  ? isEditMode
                    ? t('common.saving')
                    : t('common.adding')
                  : isEditMode
                    ? t('common.save')
                    : t('mcp.addServer')}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default AddServerDialog;
