/**
 * ServerRegistry Component
 *
 * Main component for displaying and managing MCP servers.
 */

import { useEffect, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { PlusIcon, DownloadIcon, ReloadIcon } from '@radix-ui/react-icons';
import type { McpServer, CommandResponse, HealthCheckResult } from '../../types/mcp';
import { ServerCard } from './ServerCard';
import { AddServerDialog } from './AddServerDialog';
import { ImportDialog } from './ImportDialog';

export function ServerRegistry() {
  const { t } = useTranslation();
  const [servers, setServers] = useState<McpServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [testingServerId, setTestingServerId] = useState<string | null>(null);
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [importDialogOpen, setImportDialogOpen] = useState(false);

  // Fetch servers
  const fetchServers = useCallback(async () => {
    setLoading(true);
    setError(null);

    try {
      const response = await invoke<CommandResponse<McpServer[]>>('list_mcp_servers');
      if (response.success && response.data) {
        setServers(response.data);
      } else {
        setError(response.error || 'Failed to fetch servers');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to fetch servers');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchServers();
  }, [fetchServers]);

  // Test server connection
  const handleTest = async (serverId: string) => {
    setTestingServerId(serverId);

    try {
      const response = await invoke<CommandResponse<HealthCheckResult>>('test_mcp_server', {
        id: serverId,
      });

      if (response.success && response.data) {
        // Update server status in local state
        setServers((prev) =>
          prev.map((s) =>
            s.id === serverId
              ? { ...s, status: response.data!.status, last_checked: response.data!.checked_at }
              : s
          )
        );
      }
    } catch (err) {
      console.error('Test failed:', err);
    } finally {
      setTestingServerId(null);
    }
  };

  // Toggle server enabled
  const handleToggle = async (serverId: string, enabled: boolean) => {
    try {
      const response = await invoke<CommandResponse<McpServer>>('toggle_mcp_server', {
        id: serverId,
        enabled,
      });

      if (response.success && response.data) {
        setServers((prev) =>
          prev.map((s) => (s.id === serverId ? response.data! : s))
        );
      }
    } catch (err) {
      console.error('Toggle failed:', err);
    }
  };

  // Delete server
  const handleDelete = async (serverId: string) => {
    if (!confirm(t('mcp.confirmDelete'))) return;

    try {
      const response = await invoke<CommandResponse<void>>('remove_mcp_server', {
        id: serverId,
      });

      if (response.success) {
        setServers((prev) => prev.filter((s) => s.id !== serverId));
      }
    } catch (err) {
      console.error('Delete failed:', err);
    }
  };

  // Handle server added
  const handleServerAdded = (server: McpServer) => {
    setServers((prev) => [...prev, server]);
    setAddDialogOpen(false);
  };

  // Handle import complete
  const handleImportComplete = () => {
    fetchServers();
    setImportDialogOpen(false);
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between mb-2">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
            {t('mcp.title')}
          </h2>

          <div className="flex items-center gap-2">
            {/* Refresh Button */}
            <button
              onClick={fetchServers}
              disabled={loading}
              className={clsx(
                'p-2 rounded-md',
                'bg-gray-100 dark:bg-gray-800',
                'text-gray-600 dark:text-gray-400',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'disabled:opacity-50',
                'transition-colors'
              )}
              title={t('mcp.refresh')}
            >
              <ReloadIcon className={clsx('w-4 h-4', loading && 'animate-spin')} />
            </button>

            {/* Import Button */}
            <button
              onClick={() => setImportDialogOpen(true)}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
                'bg-gray-100 dark:bg-gray-800',
                'text-gray-700 dark:text-gray-300',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'text-sm font-medium',
                'transition-colors'
              )}
            >
              <DownloadIcon className="w-4 h-4" />
              <span>{t('mcp.import')}</span>
            </button>

            {/* Add Button */}
            <button
              onClick={() => setAddDialogOpen(true)}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
                'bg-primary-600 hover:bg-primary-700',
                'text-white text-sm font-medium',
                'transition-colors'
              )}
            >
              <PlusIcon className="w-4 h-4" />
              <span>{t('mcp.addServer')}</span>
            </button>
          </div>
        </div>

        <p className="text-sm text-gray-500 dark:text-gray-400">
          {t('mcp.description')}
        </p>
      </div>

      {/* Server List */}
      <div className="flex-1 overflow-y-auto p-4">
        {loading && servers.length === 0 ? (
          // Loading state
          <div className="space-y-4">
            <ServerSkeleton />
            <ServerSkeleton />
          </div>
        ) : error ? (
          // Error state
          <div className="text-center py-8">
            <p className="text-sm text-red-500 dark:text-red-400">{error}</p>
            <button
              onClick={fetchServers}
              className="mt-2 text-sm text-primary-600 dark:text-primary-400 hover:underline"
            >
              {t('common.retry')}
            </button>
          </div>
        ) : servers.length === 0 ? (
          // Empty state
          <div className="text-center py-12">
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-gray-100 dark:bg-gray-800 flex items-center justify-center">
              <PlusIcon className="w-8 h-8 text-gray-400" />
            </div>
            <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-2">
              {t('mcp.noServers')}
            </h3>
            <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">
              {t('mcp.noServersDescription')}
            </p>
            <div className="flex items-center justify-center gap-2">
              <button
                onClick={() => setImportDialogOpen(true)}
                className={clsx(
                  'flex items-center gap-1.5 px-3 py-2 rounded-md',
                  'bg-gray-100 dark:bg-gray-800',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                  'text-sm font-medium',
                  'transition-colors'
                )}
              >
                <DownloadIcon className="w-4 h-4" />
                <span>{t('mcp.importFromClaude')}</span>
              </button>
              <button
                onClick={() => setAddDialogOpen(true)}
                className={clsx(
                  'flex items-center gap-1.5 px-3 py-2 rounded-md',
                  'bg-primary-600 hover:bg-primary-700',
                  'text-white text-sm font-medium',
                  'transition-colors'
                )}
              >
                <PlusIcon className="w-4 h-4" />
                <span>{t('mcp.addManually')}</span>
              </button>
            </div>
          </div>
        ) : (
          // Server cards
          <div className="grid gap-4 grid-cols-1 lg:grid-cols-2 3xl:grid-cols-3">
            {servers.map((server) => (
              <ServerCard
                key={server.id}
                server={server}
                onTest={() => handleTest(server.id)}
                onToggle={(enabled) => handleToggle(server.id, enabled)}
                onEdit={() => {
                  // TODO: Open edit dialog
                  console.log('Edit:', server.id);
                }}
                onDelete={() => handleDelete(server.id)}
                isLoading={testingServerId === server.id}
              />
            ))}
          </div>
        )}
      </div>

      {/* Dialogs */}
      <AddServerDialog
        open={addDialogOpen}
        onOpenChange={setAddDialogOpen}
        onServerAdded={handleServerAdded}
      />

      <ImportDialog
        open={importDialogOpen}
        onOpenChange={setImportDialogOpen}
        onImportComplete={handleImportComplete}
      />
    </div>
  );
}

function ServerSkeleton() {
  return (
    <div className="p-4 rounded-lg border border-gray-200 dark:border-gray-700" aria-hidden="true">
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-3">
          <div className="w-2.5 h-2.5 rounded-full bg-gray-200 dark:bg-gray-700 animate-skeleton" />
          <div>
            <div className="h-5 w-32 bg-gray-200 dark:bg-gray-700 rounded mb-1 animate-skeleton" />
            <div className="h-4 w-20 bg-gray-100 dark:bg-gray-800 rounded animate-skeleton" />
          </div>
        </div>
        <div className="h-5 w-9 bg-gray-200 dark:bg-gray-700 rounded-full animate-skeleton" />
      </div>
      <div className="h-4 w-48 bg-gray-100 dark:bg-gray-800 rounded mb-3 animate-skeleton" />
      <div className="flex gap-2">
        <div className="h-7 w-16 bg-gray-100 dark:bg-gray-800 rounded animate-skeleton" />
        <div className="h-7 w-14 bg-gray-100 dark:bg-gray-800 rounded animate-skeleton" />
      </div>
    </div>
  );
}

export default ServerRegistry;
