/**
 * ServerRegistry Component
 *
 * Main component for displaying and managing MCP servers.
 */

import { useEffect, useState, useCallback, useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { PlusIcon, DownloadIcon, ReloadIcon, UploadIcon, InfoCircledIcon, Cross2Icon } from '@radix-ui/react-icons';
import type {
  McpServer,
  CommandResponse,
  HealthCheckResult,
  ConnectedServerInfo,
  McpExportPayload,
  McpCatalogItem,
  McpInstallResult,
  McpInstallProgressEvent,
  McpInstallLogEvent,
  McpOauthEvent,
  McpRuntimeInfo,
  McpRuntimeRepairResult,
} from '../../types/mcp';
import { ServerCard } from './ServerCard';
import { AddServerDialog } from './AddServerDialog';
import { ImportDialog } from './ImportDialog';
import { DiscoverTab } from './DiscoverTab';
import { InstallCatalogDialog } from './InstallCatalogDialog';
import { useToast } from '../shared/Toast';
import { localTimestampForFilename, saveTextWithDialog } from '../../lib/exportUtils';

type McpCommandAction =
  | 'open-add'
  | 'open-import'
  | 'open-discover'
  | 'install-recommended'
  | 'refresh'
  | 'test-enabled'
  | 'export';
type McpEventStatus = 'success' | 'error' | 'info';

interface McpEventRecord {
  id: string;
  at: string;
  action: string;
  status: McpEventStatus;
  serverId?: string;
  serverName?: string;
  detail?: string;
}

const MCP_COMMAND_EVENT = 'plan-cascade:mcp-command';

function addToSet(prev: Set<string>, id: string) {
  const next = new Set(prev);
  next.add(id);
  return next;
}

function removeFromSet(prev: Set<string>, id: string) {
  const next = new Set(prev);
  next.delete(id);
  return next;
}

export function ServerRegistry() {
  const { t } = useTranslation();
  const { showToast } = useToast();
  const [servers, setServers] = useState<McpServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [serverErrors, setServerErrors] = useState<Record<string, string>>({});
  const [testingIds, setTestingIds] = useState<Set<string>>(new Set());
  const [connectingIds, setConnectingIds] = useState<Set<string>>(new Set());
  const [disconnectingIds, setDisconnectingIds] = useState<Set<string>>(new Set());
  const [togglingIds, setTogglingIds] = useState<Set<string>>(new Set());
  const [deletingIds, setDeletingIds] = useState<Set<string>>(new Set());
  const [connectedServers, setConnectedServers] = useState<Record<string, ConnectedServerInfo>>({});
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [importDialogOpen, setImportDialogOpen] = useState(false);
  const [editingServer, setEditingServer] = useState<McpServer | null>(null);
  const [showDiagnostics, setShowDiagnostics] = useState(false);
  const [eventLog, setEventLog] = useState<McpEventRecord[]>([]);
  const [runtimeInventory, setRuntimeInventory] = useState<McpRuntimeInfo[]>([]);
  const [runtimeLoading, setRuntimeLoading] = useState(false);
  const [repairingRuntimes, setRepairingRuntimes] = useState<Set<string>>(new Set());
  const [selectedToolServerId, setSelectedToolServerId] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'installed' | 'discover'>('installed');
  const [selectedCatalogItem, setSelectedCatalogItem] = useState<McpCatalogItem | null>(null);
  const [installRecommendedNonce, setInstallRecommendedNonce] = useState(0);

  const connectedServerIds = useMemo(() => new Set(Object.keys(connectedServers)), [connectedServers]);
  const selectedToolServer = selectedToolServerId ? connectedServers[selectedToolServerId] : null;
  const duplicateServerNames = useMemo(() => {
    const counts = new Map<string, number>();
    for (const server of servers) {
      counts.set(server.name, (counts.get(server.name) || 0) + 1);
    }
    return new Set(
      Array.from(counts.entries())
        .filter(([, count]) => count > 1)
        .map(([name]) => name),
    );
  }, [servers]);
  const installedCatalogItems = useMemo(() => {
    const map: Record<string, { serverId: string; serverName: string; managed: boolean }[]> = {};
    for (const server of servers) {
      if (!server.catalog_item_id) {
        continue;
      }
      if (!map[server.catalog_item_id]) {
        map[server.catalog_item_id] = [];
      }
      map[server.catalog_item_id].push({
        serverId: server.id,
        serverName: server.name,
        managed: !!server.managed_install,
      });
    }
    return map;
  }, [servers]);

  const setServerError = useCallback((serverId: string, message: string | null) => {
    setServerErrors((prev) => {
      const next = { ...prev };
      if (!message) {
        delete next[serverId];
      } else {
        next[serverId] = message;
      }
      return next;
    });
  }, []);

  const appendEvent = useCallback(
    (
      action: string,
      status: McpEventStatus,
      options?: {
        server?: McpServer;
        serverId?: string;
        detail?: string;
      },
    ) => {
      const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
      const next: McpEventRecord = {
        id,
        at: new Date().toISOString(),
        action,
        status,
        serverId: options?.serverId || options?.server?.id,
        serverName: options?.server?.name,
        detail: options?.detail,
      };
      setEventLog((prev) => [next, ...prev].slice(0, 20));
    },
    [],
  );

  const handleCatalogEvent = useCallback(
    (event: { action: 'catalog_refresh'; status: McpEventStatus; detail?: string }) => {
      appendEvent(event.action, event.status, { detail: event.detail });
    },
    [appendEvent],
  );

  const getEventActionLabel = useCallback(
    (action: string) => {
      const actionKey =
        action === 'test_enabled'
          ? 'testEnabled'
          : action === 'catalog_refresh'
            ? 'catalogRefresh'
            : action === 'install_progress'
              ? 'installProgress'
              : action === 'install_log'
                ? 'installLog'
                : action === 'oauth_state'
                  ? 'oauthState'
                  : action === 'runtime_repair'
                    ? 'runtimeRepair'
                    : action;
      return t(`mcp.eventActions.${actionKey}`, { defaultValue: action });
    },
    [t],
  );

  const fetchServers = useCallback(
    async (silent = false) => {
      if (!silent) {
        setLoading(true);
      }
      if (!silent && servers.length === 0) {
        setError(null);
      }

      try {
        const response = await invoke<CommandResponse<McpServer[]>>('list_mcp_servers');
        if (response.success && response.data) {
          setServers(response.data);
          setError(null);
        } else {
          const message = response.error || t('mcp.errors.fetchServers');
          if (servers.length === 0) {
            setError(message);
          }
          showToast(message, 'error');
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : t('mcp.errors.fetchServers');
        if (servers.length === 0) {
          setError(message);
        }
        showToast(message, 'error');
      } finally {
        if (!silent) {
          setLoading(false);
        }
      }
    },
    [servers.length, showToast, t],
  );

  const fetchConnectedServers = useCallback(async () => {
    try {
      const response = await invoke<CommandResponse<ConnectedServerInfo[]>>('list_connected_mcp_servers');
      if (response.success && response.data) {
        const next: Record<string, ConnectedServerInfo> = {};
        response.data.forEach((info) => {
          next[info.server_id] = info;
        });
        setConnectedServers(next);
      }
    } catch (err) {
      console.warn('Failed to fetch connected MCP servers', err);
    }
  }, []);

  const fetchRuntimeInventory = useCallback(
    async (silent = false) => {
      if (!silent) {
        setRuntimeLoading(true);
      }
      try {
        const response = await invoke<CommandResponse<McpRuntimeInfo[]>>('list_mcp_runtime_inventory');
        if (response.success && response.data) {
          setRuntimeInventory(response.data);
        } else if (!silent) {
          showToast(response.error || t('mcp.errors.fetchServers'), 'error');
        }
      } catch (err) {
        if (!silent) {
          showToast(err instanceof Error ? err.message : t('mcp.errors.fetchServers'), 'error');
        }
      } finally {
        if (!silent) {
          setRuntimeLoading(false);
        }
      }
    },
    [showToast, t],
  );

  useEffect(() => {
    fetchServers();
    fetchConnectedServers();
    fetchRuntimeInventory();
  }, [fetchConnectedServers, fetchRuntimeInventory, fetchServers]);

  useEffect(() => {
    let disposed = false;
    let progressUnlisten: UnlistenFn | null = null;
    let logUnlisten: UnlistenFn | null = null;
    let oauthUnlisten: UnlistenFn | null = null;

    const toJobSuffix = (jobId: string) => {
      if (!jobId) return '';
      return ` [job:${jobId.slice(0, 8)}]`;
    };

    const bindEvents = async () => {
      progressUnlisten = await listen<McpInstallProgressEvent>('mcp:install-progress', (event) => {
        if (disposed) return;
        const payload = event.payload;
        const status: McpEventStatus =
          payload.status === 'success' ? 'success' : payload.status === 'failed' ? 'error' : 'info';
        appendEvent('install_progress', status, {
          serverId: payload.server_id || undefined,
          detail: `${payload.phase} - ${payload.message} (${Math.round(payload.progress * 100)}%)${toJobSuffix(payload.job_id)}`,
        });
        if (
          (payload.status === 'success' || payload.status === 'failed') &&
          (payload.phase === 'COMMIT' || payload.phase === 'ROLLBACK')
        ) {
          void fetchRuntimeInventory(true);
        }
      });

      logUnlisten = await listen<McpInstallLogEvent>('mcp:install-log', (event) => {
        if (disposed) return;
        const payload = event.payload;
        if (payload.level === 'info') {
          return;
        }
        appendEvent('install_log', payload.level === 'error' ? 'error' : 'info', {
          detail: `[${payload.phase}/${payload.level}] ${payload.message}${toJobSuffix(payload.job_id)}`,
        });
      });

      oauthUnlisten = await listen<McpOauthEvent>('mcp:oauth-state', (event) => {
        if (disposed) return;
        const payload = event.payload;
        const state = payload.state.toLowerCase();
        const status: McpEventStatus =
          state.includes('error') || state.includes('failed') || state.includes('denied')
            ? 'error'
            : state.includes('success') || state.includes('authorized') || state.includes('connected')
              ? 'success'
              : 'info';
        appendEvent('oauth_state', status, {
          detail: `${payload.state}${payload.message ? ` - ${payload.message}` : ''}${toJobSuffix(payload.job_id)}`,
        });
      });
    };

    void bindEvents();

    return () => {
      disposed = true;
      if (progressUnlisten) progressUnlisten();
      if (logUnlisten) logUnlisten();
      if (oauthUnlisten) oauthUnlisten();
    };
  }, [appendEvent, fetchRuntimeInventory]);

  useEffect(() => {
    if (selectedToolServerId && !connectedServers[selectedToolServerId]) {
      setSelectedToolServerId(null);
    }
  }, [connectedServers, selectedToolServerId]);

  const withAction = useCallback(
    async (id: string, setState: React.Dispatch<React.SetStateAction<Set<string>>>, fn: () => Promise<void>) => {
      setState((prev) => addToSet(prev, id));
      try {
        await fn();
      } finally {
        setState((prev) => removeFromSet(prev, id));
      }
    },
    [],
  );

  const handleRepairRuntime = useCallback(
    async (runtimeKind: string) => {
      await withAction(runtimeKind, setRepairingRuntimes, async () => {
        try {
          const response = await invoke<CommandResponse<McpRuntimeRepairResult>>('repair_mcp_runtime', {
            runtimeKind,
          });
          if (response.success && response.data) {
            const isSuccess = response.data.status === 'repaired' || response.data.status === 'already_healthy';
            appendEvent('runtime_repair', isSuccess ? 'success' : 'error', {
              detail: `${runtimeKind}: ${response.data.message}`,
            });
            showToast(response.data.message, isSuccess ? 'success' : 'error');
            await fetchRuntimeInventory(true);
          } else {
            const message = response.error || t('mcp.errors.fetchServers');
            appendEvent('runtime_repair', 'error', { detail: `${runtimeKind}: ${message}` });
            showToast(message, 'error');
          }
        } catch (err) {
          const message = err instanceof Error ? err.message : t('mcp.errors.fetchServers');
          appendEvent('runtime_repair', 'error', { detail: `${runtimeKind}: ${message}` });
          showToast(message, 'error');
        }
      });
    },
    [appendEvent, fetchRuntimeInventory, showToast, t, withAction],
  );

  const handleTest = useCallback(
    async (serverId: string) => {
      await withAction(serverId, setTestingIds, async () => {
        setServerError(serverId, null);
        try {
          const response = await invoke<CommandResponse<HealthCheckResult>>('test_mcp_server', {
            id: serverId,
          });

          if (response.success && response.data) {
            setServers((prev) =>
              prev.map((s) =>
                s.id === serverId
                  ? {
                      ...s,
                      status: response.data!.status,
                      last_checked: response.data!.checked_at,
                      last_error: typeof response.data!.status === 'object' ? response.data!.status.error : null,
                    }
                  : s,
              ),
            );

            const testLabel = response.data.latency_ms != null ? ` (${response.data.latency_ms}ms)` : '';
            if (response.data.status === 'connected') {
              showToast(`${t('mcp.status.connected')}${testLabel}`, 'success');
              const server = servers.find((s) => s.id === serverId);
              appendEvent('test', 'success', {
                server,
                serverId,
                detail: t('mcp.eventDetails.testMetrics', {
                  latency: response.data.latency_ms ?? 'n/a',
                  tools: response.data.tool_count ?? 'n/a',
                }),
              });
            } else {
              showToast(`${t('mcp.status.error', { message: '' })}${testLabel}`.trim(), 'error');
              const server = servers.find((s) => s.id === serverId);
              appendEvent('test', 'error', {
                server,
                serverId,
                detail: response.data.protocol_version || t('mcp.status.error', { message: '' }),
              });
            }
            await fetchServers(true);
          } else {
            const message = response.error || t('mcp.errors.testConnection');
            setServerError(serverId, message);
            showToast(message, 'error');
            const server = servers.find((s) => s.id === serverId);
            appendEvent('test', 'error', { server, serverId, detail: message });
          }
        } catch (err) {
          const message = err instanceof Error ? err.message : t('mcp.errors.testConnection');
          setServerError(serverId, message);
          showToast(message, 'error');
          const server = servers.find((s) => s.id === serverId);
          appendEvent('test', 'error', { server, serverId, detail: message });
        }
      });
    },
    [appendEvent, fetchServers, servers, setServerError, showToast, t, withAction],
  );

  const handleToggle = async (serverId: string, enabled: boolean) => {
    await withAction(serverId, setTogglingIds, async () => {
      setServerError(serverId, null);
      try {
        const response = await invoke<CommandResponse<McpServer>>('toggle_mcp_server', {
          id: serverId,
          enabled,
        });

        if (response.success && response.data) {
          setServers((prev) => prev.map((s) => (s.id === serverId ? response.data! : s)));
          if (!enabled && connectedServerIds.has(serverId)) {
            await invoke<CommandResponse<void>>('disconnect_mcp_server', { id: serverId });
            await fetchConnectedServers();
            await fetchServers(true);
          }
          appendEvent('toggle', 'info', {
            server: response.data,
            serverId,
            detail: t('mcp.eventDetails.enabled', { enabled }),
          });
        } else {
          const message = response.error || t('mcp.errors.toggleServer');
          setServerError(serverId, message);
          showToast(message, 'error');
          const server = servers.find((s) => s.id === serverId);
          appendEvent('toggle', 'error', { server, serverId, detail: message });
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : t('mcp.errors.toggleServer');
        setServerError(serverId, message);
        showToast(message, 'error');
        const server = servers.find((s) => s.id === serverId);
        appendEvent('toggle', 'error', { server, serverId, detail: message });
      }
    });
  };

  const handleConnect = async (serverId: string) => {
    await withAction(serverId, setConnectingIds, async () => {
      setServerError(serverId, null);
      try {
        const response = await invoke<CommandResponse<ConnectedServerInfo>>('connect_mcp_server', { id: serverId });
        if (response.success) {
          await fetchConnectedServers();
          await fetchServers(true);
          showToast(t('mcp.status.connected'), 'success');
          const server = servers.find((s) => s.id === serverId);
          appendEvent('connect', 'success', { server, serverId });
        } else {
          const message = response.error || t('mcp.errors.connectServer');
          setServerError(serverId, message);
          showToast(message, 'error');
          const server = servers.find((s) => s.id === serverId);
          appendEvent('connect', 'error', { server, serverId, detail: message });
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : t('mcp.errors.connectServer');
        setServerError(serverId, message);
        showToast(message, 'error');
        const server = servers.find((s) => s.id === serverId);
        appendEvent('connect', 'error', { server, serverId, detail: message });
      }
    });
  };

  const handleDisconnect = async (serverId: string) => {
    await withAction(serverId, setDisconnectingIds, async () => {
      setServerError(serverId, null);
      try {
        const response = await invoke<CommandResponse<void>>('disconnect_mcp_server', { id: serverId });
        if (response.success) {
          await fetchConnectedServers();
          await fetchServers(true);
          showToast(t('mcp.status.disconnected'), 'info');
          const server = servers.find((s) => s.id === serverId);
          appendEvent('disconnect', 'success', { server, serverId });
        } else {
          const message = response.error || t('mcp.errors.disconnectServer');
          setServerError(serverId, message);
          showToast(message, 'error');
          const server = servers.find((s) => s.id === serverId);
          appendEvent('disconnect', 'error', { server, serverId, detail: message });
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : t('mcp.errors.disconnectServer');
        setServerError(serverId, message);
        showToast(message, 'error');
        const server = servers.find((s) => s.id === serverId);
        appendEvent('disconnect', 'error', { server, serverId, detail: message });
      }
    });
  };

  const handleDelete = async (serverId: string) => {
    const confirmed = confirm(t('mcp.confirmDeleteWithDisconnect'));
    if (!confirmed) return;

    await withAction(serverId, setDeletingIds, async () => {
      setServerError(serverId, null);
      try {
        const response = await invoke<CommandResponse<void>>('remove_mcp_server', {
          id: serverId,
        });

        if (response.success) {
          const server = servers.find((s) => s.id === serverId);
          setServers((prev) => prev.filter((s) => s.id !== serverId));
          setConnectedServers((prev) => {
            const next = { ...prev };
            delete next[serverId];
            return next;
          });
          if (selectedToolServerId === serverId) {
            setSelectedToolServerId(null);
          }
          appendEvent('delete', 'success', { server, serverId });
          showToast(t('common.done'), 'success');
        } else {
          const message = response.error || t('mcp.errors.deleteServer');
          setServerError(serverId, message);
          showToast(message, 'error');
          const server = servers.find((s) => s.id === serverId);
          appendEvent('delete', 'error', { server, serverId, detail: message });
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : t('mcp.errors.deleteServer');
        setServerError(serverId, message);
        showToast(message, 'error');
        const server = servers.find((s) => s.id === serverId);
        appendEvent('delete', 'error', { server, serverId, detail: message });
      }
    });
  };

  const handleServerAdded = (server: McpServer) => {
    setServers((prev) => [...prev, server]);
    setAddDialogOpen(false);
    appendEvent('add', 'success', { server });
    showToast(t('common.done'), 'success');
  };

  const handleServerUpdated = (updated: McpServer) => {
    setServers((prev) => prev.map((s) => (s.id === updated.id ? updated : s)));
    setEditingServer(null);
    appendEvent('update', 'success', { server: updated });
    showToast(t('common.done'), 'success');
  };

  const handleImportComplete = () => {
    void fetchServers(true);
    void fetchConnectedServers();
    appendEvent('import', 'success');
    setImportDialogOpen(false);
    showToast(t('common.done'), 'success');
  };

  const handleCatalogInstalled = useCallback(
    (result: McpInstallResult) => {
      void fetchServers(true);
      void fetchConnectedServers();
      void fetchRuntimeInventory(true);
      appendEvent('add', 'success', {
        serverId: result.server_id || undefined,
        detail: t('mcp.install.installSucceeded'),
      });
      showToast(t('mcp.install.installSucceeded'), 'success');
      setSelectedCatalogItem(null);
    },
    [appendEvent, fetchConnectedServers, fetchRuntimeInventory, fetchServers, showToast, t],
  );

  const handleExport = useCallback(async () => {
    try {
      const response = await invoke<CommandResponse<McpExportPayload>>('export_mcp_servers');
      if (!response.success || !response.data) {
        showToast(response.error || t('mcp.errors.fetchServers'), 'error');
        appendEvent('export', 'error', { detail: response.error || t('mcp.errors.fetchServers') });
        return;
      }

      const filename = `mcp-config-${localTimestampForFilename()}.json`;
      const saved = await saveTextWithDialog(filename, JSON.stringify(response.data, null, 2));
      if (saved) {
        showToast(t('mcp.exportSuccess'), 'success');
        appendEvent('export', 'success');
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : t('mcp.errors.fetchServers');
      showToast(message, 'error');
      appendEvent('export', 'error', { detail: message });
    }
  }, [appendEvent, showToast, t]);

  const handleTestEnabled = useCallback(async () => {
    const enabledIds = servers.filter((s) => s.enabled).map((s) => s.id);
    for (const id of enabledIds) {
      await handleTest(id);
    }
    appendEvent('test_enabled', 'info', {
      detail: t('mcp.eventDetails.count', { count: enabledIds.length }),
    });
  }, [appendEvent, handleTest, servers, t]);

  useEffect(() => {
    const listener = (evt: Event) => {
      const detail = (evt as CustomEvent<{ action?: McpCommandAction }>).detail;
      switch (detail?.action) {
        case 'open-add':
          setAddDialogOpen(true);
          break;
        case 'open-import':
          setImportDialogOpen(true);
          break;
        case 'open-discover':
          setActiveTab('discover');
          break;
        case 'install-recommended':
          setActiveTab('discover');
          setInstallRecommendedNonce((prev) => prev + 1);
          break;
        case 'refresh':
          void fetchServers(true);
          void fetchConnectedServers();
          void fetchRuntimeInventory(true);
          break;
        case 'test-enabled':
          void handleTestEnabled();
          break;
        case 'export':
          void handleExport();
          break;
        default:
          break;
      }
    };

    window.addEventListener(MCP_COMMAND_EVENT, listener as EventListener);
    return () => window.removeEventListener(MCP_COMMAND_EVENT, listener as EventListener);
  }, [fetchConnectedServers, fetchRuntimeInventory, fetchServers, handleExport, handleTestEnabled]);

  return (
    <div className="h-full flex flex-col">
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between mb-2">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">{t('mcp.title')}</h2>

          <div className="flex items-center gap-2">
            <button
              onClick={() => fetchServers()}
              disabled={loading}
              className={clsx(
                'p-2 rounded-md',
                'bg-gray-100 dark:bg-gray-800',
                'text-gray-600 dark:text-gray-400',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'disabled:opacity-50',
                'transition-colors',
              )}
              title={t('mcp.refresh')}
            >
              <ReloadIcon className={clsx('w-4 h-4', loading && 'animate-spin')} />
            </button>

            <button
              onClick={handleExport}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
                'bg-gray-100 dark:bg-gray-800',
                'text-gray-700 dark:text-gray-300',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'text-sm font-medium',
                'transition-colors',
              )}
            >
              <UploadIcon className="w-4 h-4" />
              <span>{t('mcp.export')}</span>
            </button>

            <button
              onClick={() => setImportDialogOpen(true)}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
                'bg-gray-100 dark:bg-gray-800',
                'text-gray-700 dark:text-gray-300',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'text-sm font-medium',
                'transition-colors',
              )}
            >
              <DownloadIcon className="w-4 h-4" />
              <span>{t('mcp.import')}</span>
            </button>

            <button
              onClick={() => setActiveTab('discover')}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
                'bg-gray-100 dark:bg-gray-800',
                'text-gray-700 dark:text-gray-300',
                'hover:bg-gray-200 dark:hover:bg-gray-700',
                'text-sm font-medium',
                'transition-colors',
              )}
            >
              <InfoCircledIcon className="w-4 h-4" />
              <span>{t('mcp.discover.title')}</span>
            </button>

            <button
              onClick={() => setAddDialogOpen(true)}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
                'bg-primary-600 hover:bg-primary-700',
                'text-white text-sm font-medium',
                'transition-colors',
              )}
            >
              <PlusIcon className="w-4 h-4" />
              <span>{t('mcp.addServer')}</span>
            </button>
          </div>
        </div>

        <p className="text-sm text-gray-500 dark:text-gray-400">{t('mcp.description')}</p>
      </div>

      <div className="flex-1 overflow-y-auto p-4">
        <div className="mb-4 inline-flex rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
          <button
            type="button"
            onClick={() => setActiveTab('installed')}
            className={clsx(
              'px-3 py-1.5 text-xs font-medium',
              activeTab === 'installed'
                ? 'bg-primary-600 text-white'
                : 'bg-white dark:bg-gray-900 text-gray-700 dark:text-gray-300',
            )}
          >
            {t('mcp.tabs.installed')}
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('discover')}
            className={clsx(
              'px-3 py-1.5 text-xs font-medium border-l border-gray-200 dark:border-gray-700',
              activeTab === 'discover'
                ? 'bg-primary-600 text-white'
                : 'bg-white dark:bg-gray-900 text-gray-700 dark:text-gray-300',
            )}
          >
            {t('mcp.tabs.discover')}
          </button>
        </div>

        {activeTab === 'discover' ? (
          <DiscoverTab
            onInstallItem={(item) => setSelectedCatalogItem(item)}
            installRecommendedNonce={installRecommendedNonce}
            installedCatalogItems={installedCatalogItems}
            onCatalogEvent={handleCatalogEvent}
          />
        ) : loading && servers.length === 0 ? (
          <div className="space-y-4">
            <ServerSkeleton />
            <ServerSkeleton />
          </div>
        ) : error ? (
          <div className="text-center py-8">
            <p className="text-sm text-red-500 dark:text-red-400">{error}</p>
            <button
              onClick={() => fetchServers()}
              className="mt-2 text-sm text-primary-600 dark:text-primary-400 hover:underline"
            >
              {t('common.retry')}
            </button>
          </div>
        ) : servers.length === 0 ? (
          <div className="text-center py-12">
            <div className="w-16 h-16 mx-auto mb-4 rounded-full bg-gray-100 dark:bg-gray-800 flex items-center justify-center">
              <PlusIcon className="w-8 h-8 text-gray-400" />
            </div>
            <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-2">{t('mcp.noServers')}</h3>
            <p className="text-sm text-gray-500 dark:text-gray-400 mb-4">{t('mcp.noServersDescription')}</p>
            <div className="flex items-center justify-center gap-2">
              <button
                onClick={() => setImportDialogOpen(true)}
                className={clsx(
                  'flex items-center gap-1.5 px-3 py-2 rounded-md',
                  'bg-gray-100 dark:bg-gray-800',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                  'text-sm font-medium',
                  'transition-colors',
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
                  'transition-colors',
                )}
              >
                <PlusIcon className="w-4 h-4" />
                <span>{t('mcp.addManually')}</span>
              </button>
            </div>
          </div>
        ) : (
          <>
            <div className="grid gap-4 grid-cols-1 lg:grid-cols-2 3xl:grid-cols-3">
              {servers.map((server) => {
                return (
                  <div key={server.id} className="space-y-2">
                    <ServerCard
                      server={server}
                      connected={connectedServerIds.has(server.id)}
                      connectedInfo={connectedServers[server.id]}
                      onTest={() => handleTest(server.id)}
                      onToggle={(enabled) => handleToggle(server.id, enabled)}
                      onConnect={() => handleConnect(server.id)}
                      onDisconnect={() => handleDisconnect(server.id)}
                      onEdit={async () => {
                        try {
                          const response = await invoke<CommandResponse<McpServer>>('get_mcp_server_detail', {
                            id: server.id,
                            includeSecrets: true,
                          });
                          if (response.success && response.data) {
                            setEditingServer(response.data);
                          } else {
                            showToast(response.error || t('mcp.errors.fetchServers'), 'error');
                          }
                        } catch (err) {
                          showToast(err instanceof Error ? err.message : t('mcp.errors.fetchServers'), 'error');
                        }
                      }}
                      onDelete={() => handleDelete(server.id)}
                      onViewTools={
                        connectedServers[server.id]
                          ? () => {
                              setSelectedToolServerId(server.id);
                            }
                          : undefined
                      }
                      isConnecting={connectingIds.has(server.id)}
                      isDisconnecting={disconnectingIds.has(server.id)}
                      isTesting={testingIds.has(server.id)}
                      isToggling={togglingIds.has(server.id)}
                      isDeleting={deletingIds.has(server.id)}
                    />
                    {serverErrors[server.id] && (
                      <div className="text-xs rounded-md border border-red-200 bg-red-50 dark:border-red-900/40 dark:bg-red-900/20 text-red-700 dark:text-red-300 px-2 py-1.5">
                        {serverErrors[server.id]}
                      </div>
                    )}
                    {duplicateServerNames.has(server.name) && (
                      <div className="text-xs rounded-md border border-yellow-200 bg-yellow-50 dark:border-yellow-900/40 dark:bg-yellow-900/20 text-yellow-700 dark:text-yellow-300 px-2 py-1.5">
                        {t('mcp.duplicateNameWarning')}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>

            <div className="mt-4 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
              <button
                onClick={() => setShowDiagnostics((v) => !v)}
                className="w-full flex items-center justify-between px-3 py-2 text-sm bg-gray-50 dark:bg-gray-800 hover:bg-gray-100 dark:hover:bg-gray-700"
              >
                <span className="inline-flex items-center gap-2 text-gray-700 dark:text-gray-300">
                  <InfoCircledIcon className="w-4 h-4" />
                  {t('mcp.diagnosticsTitle')}
                </span>
                <span className="text-xs text-gray-500 dark:text-gray-400">
                  {showDiagnostics ? t('mcp.hide') : t('mcp.show')}
                </span>
              </button>
              {showDiagnostics && (
                <div className="p-3 space-y-2 bg-white dark:bg-gray-900">
                  {Object.values(connectedServers).length === 0 ? (
                    <p className="text-xs text-gray-500 dark:text-gray-400">{t('mcp.noConnectedServers')}</p>
                  ) : (
                    Object.values(connectedServers).map((info) => (
                      <div
                        key={info.server_id}
                        className="text-xs rounded border border-gray-200 dark:border-gray-700 p-2"
                      >
                        <p className="font-medium text-gray-900 dark:text-white">{info.server_name}</p>
                        <p className="text-gray-500 dark:text-gray-400">
                          {t('mcp.connectionMeta', {
                            protocol: info.protocol_version || t('mcp.status.unknown'),
                            count: info.tool_names.length,
                          })}
                        </p>
                        {info.tool_names.length > 0 && (
                          <p className="text-gray-600 dark:text-gray-300 mt-1 break-all">
                            {info.tool_names.join(', ')}
                          </p>
                        )}
                      </div>
                    ))
                  )}

                  <div className="pt-2 mt-2 border-t border-gray-200 dark:border-gray-700">
                    <p className="text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">
                      {t('mcp.runtimeInventoryTitle')}
                    </p>
                    {runtimeLoading ? (
                      <p className="text-xs text-gray-500 dark:text-gray-400">{t('mcp.loading')}</p>
                    ) : runtimeInventory.length === 0 ? (
                      <p className="text-xs text-gray-500 dark:text-gray-400">{t('mcp.runtimeInventoryEmpty')}</p>
                    ) : (
                      <div className="space-y-1.5">
                        {runtimeInventory.map((runtime) => (
                          <div
                            key={runtime.runtime}
                            className="text-xs rounded border border-gray-200 dark:border-gray-700 p-2"
                          >
                            <div className="flex items-center justify-between gap-2">
                              <p className="font-medium text-gray-900 dark:text-white">{runtime.runtime}</p>
                              <div className="flex items-center gap-2">
                                <span
                                  className={clsx(
                                    'text-[11px]',
                                    runtime.healthy
                                      ? 'text-emerald-700 dark:text-emerald-300'
                                      : 'text-red-700 dark:text-red-300',
                                  )}
                                >
                                  {runtime.healthy ? t('mcp.runtimeHealthy') : t('mcp.runtimeUnhealthy')}
                                </span>
                                {!runtime.healthy && (
                                  <button
                                    type="button"
                                    onClick={() => handleRepairRuntime(runtime.runtime)}
                                    disabled={repairingRuntimes.has(runtime.runtime)}
                                    className={clsx(
                                      'px-2 py-1 rounded text-[11px] font-medium',
                                      'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300',
                                      'hover:bg-amber-200 dark:hover:bg-amber-900/50 disabled:opacity-50',
                                    )}
                                  >
                                    {repairingRuntimes.has(runtime.runtime)
                                      ? t('mcp.repairingRuntime')
                                      : t('mcp.repairRuntime')}
                                  </button>
                                )}
                              </div>
                            </div>
                            <p className="text-gray-500 dark:text-gray-400">
                              {runtime.version || t('mcp.status.unknown')}
                              {runtime.source ? ` | ${t('mcp.runtimeSource')}: ${runtime.source}` : ''}
                            </p>
                            {runtime.path && (
                              <p className="text-gray-500 dark:text-gray-400 break-all">
                                {t('mcp.runtimePath')}: {runtime.path}
                              </p>
                            )}
                            {runtime.last_error && !runtime.healthy && (
                              <p className="text-red-700 dark:text-red-300 break-all">{runtime.last_error}</p>
                            )}
                          </div>
                        ))}
                      </div>
                    )}
                  </div>

                  <div className="pt-2 mt-2 border-t border-gray-200 dark:border-gray-700">
                    <p className="text-xs font-medium text-gray-700 dark:text-gray-300 mb-1">{t('mcp.recentEvents')}</p>
                    {eventLog.length === 0 ? (
                      <p className="text-xs text-gray-500 dark:text-gray-400">{t('mcp.noRecentEvents')}</p>
                    ) : (
                      <div className="space-y-1.5 max-h-52 overflow-y-auto">
                        {eventLog.map((evt) => (
                          <div key={evt.id} className="text-xs rounded border border-gray-200 dark:border-gray-700 p-2">
                            <p
                              className={clsx(
                                'font-medium',
                                evt.status === 'success' && 'text-green-700 dark:text-green-300',
                                evt.status === 'error' && 'text-red-700 dark:text-red-300',
                                evt.status === 'info' && 'text-gray-700 dark:text-gray-300',
                              )}
                            >
                              {getEventActionLabel(evt.action)}
                              {evt.serverName ? ` - ${evt.serverName}` : ''}
                            </p>
                            {evt.detail && <p className="text-gray-600 dark:text-gray-400 break-all">{evt.detail}</p>}
                            <p className="text-gray-500 dark:text-gray-500">{new Date(evt.at).toLocaleString()}</p>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>
          </>
        )}
      </div>

      {selectedToolServer && (
        <div className="fixed inset-0 z-50">
          <button
            type="button"
            className="absolute inset-0 bg-black/40"
            onClick={() => setSelectedToolServerId(null)}
            aria-label={t('buttons.close')}
          />
          <div className="absolute right-0 top-0 h-full w-full max-w-md bg-white dark:bg-gray-900 border-l border-gray-200 dark:border-gray-700 shadow-xl p-4 flex flex-col">
            <div className="flex items-center justify-between mb-3">
              <div>
                <h3 className="text-sm font-semibold text-gray-900 dark:text-white">{t('mcp.toolsDrawerTitle')}</h3>
                <p className="text-xs text-gray-500 dark:text-gray-400">{selectedToolServer.server_name}</p>
              </div>
              <button
                type="button"
                onClick={() => setSelectedToolServerId(null)}
                className="p-1 rounded-md hover:bg-gray-100 dark:hover:bg-gray-800"
              >
                <Cross2Icon className="w-4 h-4 text-gray-600 dark:text-gray-300" />
              </button>
            </div>

            <div className="text-xs text-gray-600 dark:text-gray-400 mb-3">
              <p>{t('mcp.protocolMeta', { value: selectedToolServer.protocol_version || t('mcp.status.unknown') })}</p>
              {selectedToolServer.connected_at && (
                <p>{t('mcp.connectedAtMeta', { value: selectedToolServer.connected_at })}</p>
              )}
            </div>

            <div className="flex-1 overflow-y-auto space-y-2">
              {selectedToolServer.qualified_tool_names.length === 0 ? (
                <p className="text-xs text-gray-500 dark:text-gray-400">{t('mcp.noTools')}</p>
              ) : (
                selectedToolServer.qualified_tool_names.map((tool) => (
                  <div key={tool} className="rounded border border-gray-200 dark:border-gray-700 px-2 py-1.5">
                    <p className="text-xs font-mono text-gray-800 dark:text-gray-200 break-all">{tool}</p>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      )}

      <AddServerDialog
        open={addDialogOpen}
        onOpenChange={setAddDialogOpen}
        onServerAdded={handleServerAdded}
        onServerUpdated={handleServerUpdated}
      />

      <AddServerDialog
        open={!!editingServer}
        onOpenChange={(open) => {
          if (!open) {
            setEditingServer(null);
          }
        }}
        onServerAdded={handleServerAdded}
        onServerUpdated={handleServerUpdated}
        server={editingServer}
      />

      <ImportDialog
        open={importDialogOpen}
        onOpenChange={setImportDialogOpen}
        onImportComplete={handleImportComplete}
      />

      <InstallCatalogDialog
        open={!!selectedCatalogItem}
        item={selectedCatalogItem}
        onOpenChange={(open) => {
          if (!open) {
            setSelectedCatalogItem(null);
          }
        }}
        onInstalled={handleCatalogInstalled}
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
        <div className="h-7 w-16 bg-gray-100 dark:bg-gray-800 rounded animate-skeleton" />
      </div>
    </div>
  );
}
