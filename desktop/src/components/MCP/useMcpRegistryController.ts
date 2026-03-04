import { useCallback, useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import type {
  ConnectedMcpToolDetail,
  ConnectedServerInfo,
  McpCatalogItem,
  McpInstallLogEvent,
  McpInstallProgressEvent,
  McpInstallResult,
  McpOauthEvent,
  McpRuntimeInfo,
  McpServer,
} from '../../types/mcp';
import { useToast } from '../shared/Toast';
import { localTimestampForFilename, saveTextWithDialog } from '../../lib/exportUtils';
import { mcpApi } from '../../lib/mcpApi';
import { useMcpUiStore, type McpUiIntentAction } from '../../store/mcpUi';

export type McpEventStatus = 'success' | 'error' | 'info';

export interface McpEventRecord {
  id: string;
  at: string;
  action: string;
  status: McpEventStatus;
  serverId?: string;
  serverName?: string;
  detail?: string;
}

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

function statusFromInstallProgress(payload: McpInstallProgressEvent): McpEventStatus {
  if (payload.status === 'success') return 'success';
  if (payload.status === 'failed') return 'error';
  return 'info';
}

function statusFromOauth(payload: McpOauthEvent): McpEventStatus {
  const state = payload.state.toLowerCase();
  if (state.includes('error') || state.includes('failed') || state.includes('denied')) return 'error';
  if (state.includes('success') || state.includes('authorized') || state.includes('connected')) return 'success';
  return 'info';
}

export function useMcpRegistryController() {
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
  const [toolQuery, setToolQuery] = useState('');
  const [toolDetails, setToolDetails] = useState<ConnectedMcpToolDetail[]>([]);
  const [toolDetailsLoading, setToolDetailsLoading] = useState(false);
  const [activeTab, setActiveTab] = useState<'installed' | 'discover'>('installed');
  const [selectedCatalogItem, setSelectedCatalogItem] = useState<McpCatalogItem | null>(null);
  const [installRecommendedNonce, setInstallRecommendedNonce] = useState(0);
  const [exportDialogOpen, setExportDialogOpen] = useState(false);
  const [pendingDeleteServer, setPendingDeleteServer] = useState<McpServer | null>(null);
  const lastMcpIntent = useMcpUiStore((state) => state.lastIntent);
  const clearMcpIntent = useMcpUiStore((state) => state.clearIntent);
  const hasInitializedRef = useRef(false);

  const connectedServerIds = useMemo(() => new Set(Object.keys(connectedServers)), [connectedServers]);
  const selectedToolServer = selectedToolServerId ? connectedServers[selectedToolServerId] : null;
  const filteredToolDetails = useMemo(() => {
    const q = toolQuery.trim().toLowerCase();
    if (!q) return toolDetails;
    return toolDetails.filter((tool) => {
      const blob = `${tool.tool_name} ${tool.qualified_name} ${tool.description}`.toLowerCase();
      return blob.includes(q);
    });
  }, [toolDetails, toolQuery]);

  const installedCatalogItems = useMemo(() => {
    const map: Record<string, { serverId: string; serverName: string; managed: boolean }[]> = {};
    for (const server of servers) {
      if (!server.catalog_item_id) continue;
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
      setEventLog((prev) => [next, ...prev].slice(0, 50));
    },
    [],
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
        setError(null);
      }

      const response = await mcpApi.listServers();
      if (response.success && response.data) {
        setServers(response.data);
      } else {
        const message = response.error || t('mcp.errors.fetchServers');
        if (!silent) {
          setError(message);
        }
        showToast(message, 'error');
      }

      if (!silent) {
        setLoading(false);
      }
    },
    [showToast, t],
  );

  const fetchConnectedServers = useCallback(async () => {
    const response = await mcpApi.listConnectedServers();
    if (response.success && response.data) {
      const next: Record<string, ConnectedServerInfo> = {};
      response.data.forEach((info) => {
        next[info.server_id] = info;
      });
      setConnectedServers(next);
    }
  }, []);

  const fetchRuntimeInventory = useCallback(
    async (silent = false) => {
      if (!silent) {
        setRuntimeLoading(true);
      }
      const response = await mcpApi.listRuntimeInventory();
      if (response.success && response.data) {
        setRuntimeInventory(response.data);
      } else if (!silent) {
        showToast(response.error || t('mcp.errors.fetchServers'), 'error');
      }
      if (!silent) {
        setRuntimeLoading(false);
      }
    },
    [showToast, t],
  );

  const fetchToolDetails = useCallback(
    async (serverId: string) => {
      setToolDetailsLoading(true);
      setToolQuery('');
      const response = await mcpApi.getConnectedServerTools(serverId);
      if (response.success && response.data) {
        setToolDetails(response.data);
      } else {
        const fallback = connectedServers[serverId];
        if (fallback) {
          setToolDetails(
            fallback.qualified_tool_names.map((qualifiedName) => ({
              qualified_name: qualifiedName,
              tool_name: qualifiedName.split(':').slice(2).join(':') || qualifiedName,
              description: '',
              input_schema: {},
              is_parallel_safe: false,
            })),
          );
        } else {
          setToolDetails([]);
        }
      }
      setToolDetailsLoading(false);
    },
    [connectedServers],
  );

  useEffect(() => {
    if (hasInitializedRef.current) {
      return;
    }
    hasInitializedRef.current = true;
    void fetchServers();
    void fetchConnectedServers();
    void fetchRuntimeInventory();
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
        appendEvent('install_progress', statusFromInstallProgress(payload), {
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
        appendEvent('oauth_state', statusFromOauth(payload), {
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
    if (!selectedToolServerId) return;
    if (!connectedServers[selectedToolServerId]) {
      setSelectedToolServerId(null);
      setToolDetails([]);
      return;
    }
    void fetchToolDetails(selectedToolServerId);
  }, [connectedServers, fetchToolDetails, selectedToolServerId]);

  const withAction = useCallback(
    async (id: string, setState: Dispatch<SetStateAction<Set<string>>>, fn: () => Promise<void>) => {
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
        const response = await mcpApi.repairRuntime(runtimeKind);
        if (response.success && response.data) {
          const isSuccess = response.data.status === 'repaired' || response.data.status === 'already_healthy';
          appendEvent('runtime_repair', isSuccess ? 'success' : 'error', {
            detail: `${runtimeKind}: ${response.data.message}`,
          });
          showToast(response.data.message, isSuccess ? 'success' : 'error');
          await fetchRuntimeInventory(true);
          return;
        }
        const message = response.error || t('mcp.errors.fetchServers');
        appendEvent('runtime_repair', 'error', { detail: `${runtimeKind}: ${message}` });
        showToast(message, 'error');
      });
    },
    [appendEvent, fetchRuntimeInventory, showToast, t, withAction],
  );

  const handleTest = useCallback(
    async (serverId: string) => {
      await withAction(serverId, setTestingIds, async () => {
        setServerError(serverId, null);
        const response = await mcpApi.testServer(serverId);
        if (response.success && response.data) {
          const server = servers.find((s) => s.id === serverId);
          const testLabel = response.data.latency_ms != null ? ` (${response.data.latency_ms}ms)` : '';
          if (response.data.status === 'connected') {
            showToast(`${t('mcp.status.connected')}${testLabel}`, 'success');
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
            appendEvent('test', 'error', {
              server,
              serverId,
              detail: response.data.protocol_version || t('mcp.status.error', { message: '' }),
            });
          }
          await fetchServers(true);
          return;
        }
        const message = response.error || t('mcp.errors.testConnection');
        setServerError(serverId, message);
        showToast(message, 'error');
        appendEvent('test', 'error', { serverId, detail: message });
      });
    },
    [appendEvent, fetchServers, servers, setServerError, showToast, t, withAction],
  );

  const handleToggle = useCallback(
    async (serverId: string, enabled: boolean) => {
      await withAction(serverId, setTogglingIds, async () => {
        setServerError(serverId, null);
        const response = await mcpApi.toggleServer(serverId, enabled);
        if (response.success && response.data) {
          setServers((prev) => prev.map((s) => (s.id === serverId ? response.data! : s)));
          if (!enabled && connectedServerIds.has(serverId)) {
            await mcpApi.disconnectServer(serverId);
            await fetchConnectedServers();
            await fetchServers(true);
          }
          appendEvent('toggle', 'info', {
            server: response.data,
            serverId,
            detail: t('mcp.eventDetails.enabled', { enabled }),
          });
          return;
        }
        const message = response.error || t('mcp.errors.toggleServer');
        setServerError(serverId, message);
        showToast(message, 'error');
        appendEvent('toggle', 'error', { serverId, detail: message });
      });
    },
    [appendEvent, connectedServerIds, fetchConnectedServers, fetchServers, setServerError, showToast, t, withAction],
  );

  const handleConnect = useCallback(
    async (serverId: string) => {
      await withAction(serverId, setConnectingIds, async () => {
        setServerError(serverId, null);
        const response = await mcpApi.connectServer(serverId);
        if (response.success) {
          await fetchConnectedServers();
          await fetchServers(true);
          showToast(t('mcp.status.connected'), 'success');
          const server = servers.find((s) => s.id === serverId);
          appendEvent('connect', 'success', { server, serverId });
          return;
        }
        const message = response.error || t('mcp.errors.connectServer');
        setServerError(serverId, message);
        showToast(message, 'error');
        const server = servers.find((s) => s.id === serverId);
        appendEvent('connect', 'error', { server, serverId, detail: message });
      });
    },
    [appendEvent, fetchConnectedServers, fetchServers, servers, setServerError, showToast, t, withAction],
  );

  const handleDisconnect = useCallback(
    async (serverId: string) => {
      await withAction(serverId, setDisconnectingIds, async () => {
        setServerError(serverId, null);
        const response = await mcpApi.disconnectServer(serverId);
        if (response.success) {
          await fetchConnectedServers();
          await fetchServers(true);
          showToast(t('mcp.status.disconnected'), 'info');
          const server = servers.find((s) => s.id === serverId);
          appendEvent('disconnect', 'success', { server, serverId });
          return;
        }
        const message = response.error || t('mcp.errors.disconnectServer');
        setServerError(serverId, message);
        showToast(message, 'error');
        const server = servers.find((s) => s.id === serverId);
        appendEvent('disconnect', 'error', { server, serverId, detail: message });
      });
    },
    [appendEvent, fetchConnectedServers, fetchServers, servers, setServerError, showToast, t, withAction],
  );

  const handleDelete = useCallback(
    async (serverId: string) => {
      await withAction(serverId, setDeletingIds, async () => {
        setServerError(serverId, null);
        const response = await mcpApi.removeServer(serverId);
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
            setToolDetails([]);
            setToolQuery('');
          }
          appendEvent('delete', 'success', { server, serverId });
          showToast(t('common.done'), 'success');
          return;
        }
        const message = response.error || t('mcp.errors.deleteServer');
        setServerError(serverId, message);
        showToast(message, 'error');
        const server = servers.find((s) => s.id === serverId);
        appendEvent('delete', 'error', { server, serverId, detail: message });
      });
    },
    [appendEvent, selectedToolServerId, servers, setServerError, showToast, t, withAction],
  );

  const requestDelete = useCallback(
    (serverId: string) => {
      const server = servers.find((entry) => entry.id === serverId) || null;
      setPendingDeleteServer(server);
    },
    [servers],
  );

  const handleServerAdded = useCallback(
    (server: McpServer) => {
      setServers((prev) => [...prev, server]);
      setAddDialogOpen(false);
      appendEvent('add', 'success', { server });
      showToast(t('common.done'), 'success');
    },
    [appendEvent, showToast, t],
  );

  const handleServerUpdated = useCallback(
    (updated: McpServer) => {
      setServers((prev) => prev.map((s) => (s.id === updated.id ? updated : s)));
      setEditingServer(null);
      appendEvent('update', 'success', { server: updated });
      showToast(t('common.done'), 'success');
    },
    [appendEvent, showToast, t],
  );

  const handleImportComplete = useCallback(() => {
    void fetchServers(true);
    void fetchConnectedServers();
    appendEvent('import', 'success');
    setImportDialogOpen(false);
    showToast(t('common.done'), 'success');
  }, [appendEvent, fetchConnectedServers, fetchServers, showToast, t]);

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
    const response = await mcpApi.exportServers('redacted');
    if (!response.success || !response.data) {
      const message = response.error || t('mcp.errors.fetchServers');
      showToast(message, 'error');
      appendEvent('export', 'error', { detail: message });
      return;
    }

    const filename = `mcp-config-${localTimestampForFilename()}.json`;
    const saved = await saveTextWithDialog(filename, JSON.stringify(response.data, null, 2));
    if (saved) {
      showToast(t('mcp.exportSuccess'), 'success');
      appendEvent('export', 'success', {
        detail: t('mcp.exportRedacted', {
          defaultValue: 'Exported with secrets redacted',
        }),
      });
    }
  }, [appendEvent, showToast, t]);

  const handleTestEnabled = useCallback(async () => {
    const enabledIds = servers.filter((s) => s.enabled).map((s) => s.id);
    const concurrency = 2;
    for (let idx = 0; idx < enabledIds.length; idx += concurrency) {
      const batch = enabledIds.slice(idx, idx + concurrency);
      await Promise.all(batch.map((id) => handleTest(id)));
    }
    appendEvent('test_enabled', 'info', {
      detail: t('mcp.eventDetails.count', { count: enabledIds.length }),
    });
  }, [appendEvent, handleTest, servers, t]);

  const openExportDialog = useCallback(() => {
    setExportDialogOpen(true);
  }, []);

  const openServerEditor = useCallback(
    async (serverId: string) => {
      const response = await mcpApi.getServerDetail(serverId, true);
      if (response.success && response.data) {
        setEditingServer(response.data);
        return;
      }
      showToast(response.error || t('mcp.errors.fetchServers'), 'error');
    },
    [showToast, t],
  );

  useEffect(() => {
    const action = lastMcpIntent?.action as McpUiIntentAction | undefined;
    if (!lastMcpIntent || !action) {
      return;
    }

    switch (action) {
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
        void Promise.all([fetchServers(true), fetchConnectedServers(), fetchRuntimeInventory(true)]);
        break;
      case 'test-enabled':
        void handleTestEnabled();
        break;
      case 'export':
        openExportDialog();
        break;
      default:
        break;
    }

    clearMcpIntent(lastMcpIntent.id);
  }, [
    clearMcpIntent,
    fetchConnectedServers,
    fetchRuntimeInventory,
    fetchServers,
    handleTestEnabled,
    lastMcpIntent,
    openExportDialog,
  ]);

  const refreshAll = useCallback(
    async (silent = true) => {
      await Promise.all([fetchServers(silent), fetchConnectedServers(), fetchRuntimeInventory(silent)]);
    },
    [fetchConnectedServers, fetchRuntimeInventory, fetchServers],
  );

  const handleCatalogEvent = useCallback(
    (event: { action: 'catalog_refresh'; status: McpEventStatus; detail?: string }) => {
      appendEvent(event.action, event.status, { detail: event.detail });
    },
    [appendEvent],
  );

  return {
    t,
    servers,
    loading,
    error,
    serverErrors,
    testingIds,
    connectingIds,
    disconnectingIds,
    togglingIds,
    deletingIds,
    connectedServers,
    connectedServerIds,
    addDialogOpen,
    setAddDialogOpen,
    importDialogOpen,
    setImportDialogOpen,
    editingServer,
    setEditingServer,
    showDiagnostics,
    setShowDiagnostics,
    eventLog,
    runtimeInventory,
    runtimeLoading,
    repairingRuntimes,
    selectedToolServerId,
    setSelectedToolServerId,
    selectedToolServer,
    toolQuery,
    setToolQuery,
    toolDetails,
    filteredToolDetails,
    toolDetailsLoading,
    activeTab,
    setActiveTab,
    selectedCatalogItem,
    setSelectedCatalogItem,
    installRecommendedNonce,
    exportDialogOpen,
    setExportDialogOpen,
    pendingDeleteServer,
    setPendingDeleteServer,
    installedCatalogItems,
    refreshAll,
    handleCatalogEvent,
    appendEvent,
    getEventActionLabel,
    fetchServers,
    handleRepairRuntime,
    handleTest,
    handleToggle,
    handleConnect,
    handleDisconnect,
    handleDelete,
    requestDelete,
    handleServerAdded,
    handleServerUpdated,
    handleImportComplete,
    handleCatalogInstalled,
    handleExport,
    handleTestEnabled,
    openExportDialog,
    openServerEditor,
  };
}
