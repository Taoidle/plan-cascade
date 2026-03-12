/**
 * RemoteSection Component
 *
 * Settings section for remote session control via Telegram Bot.
 * Includes gateway status, Telegram configuration, active sessions, and audit log.
 */

import { useState, useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { CheckCircledIcon, CrossCircledIcon, TrashIcon, PlusIcon } from '@radix-ui/react-icons';
import { useRemoteStore } from '../../store/remote';
import { useSettingsStore } from '../../store/settings';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import type {
  RemoteSessionType,
  RemoteWorkspaceEntry,
  StreamingMode,
  UpdateTelegramConfigRequest,
} from '../../lib/remoteApi';
import type { WorkflowSessionCatalogItem } from '../../types/workflowKernel';

function parseStreamingMode(mode: StreamingMode): {
  mode: 'WaitForComplete' | 'PeriodicUpdate' | 'LiveEdit';
  intervalSecs: number;
  throttleMs: number;
} {
  if (mode === 'WaitForComplete') {
    return {
      mode,
      intervalSecs: 5,
      throttleMs: 1200,
    };
  }
  if ('PeriodicUpdate' in mode) {
    return {
      mode: 'PeriodicUpdate',
      intervalSecs: mode.PeriodicUpdate.interval_secs,
      throttleMs: 1200,
    };
  }
  return {
    mode: 'LiveEdit',
    intervalSecs: 5,
    throttleMs: mode.LiveEdit.throttle_ms,
  };
}

function buildStreamingMode(
  mode: 'WaitForComplete' | 'PeriodicUpdate' | 'LiveEdit',
  intervalSecs: number,
  throttleMs: number,
): StreamingMode {
  if (mode === 'PeriodicUpdate') {
    return { PeriodicUpdate: { interval_secs: intervalSecs } };
  }
  if (mode === 'LiveEdit') {
    return { LiveEdit: { throttle_ms: throttleMs } };
  }
  return 'WaitForComplete';
}

function normalizeWorkspacePath(path: string): string {
  return path.trim().replace(/\\/g, '/').replace(/\/+$/, '');
}

function deriveWorkspaceLabel(path: string, displayTitle?: string | null): string | null {
  const title = displayTitle?.trim();
  if (title) {
    return title;
  }
  const normalized = normalizeWorkspacePath(path);
  const basename = normalized.split('/').filter(Boolean).slice(-1)[0];
  return basename || null;
}

function formatRemoteSessionType(sessionType: RemoteSessionType | string): string {
  if (typeof sessionType === 'string') {
    return sessionType;
  }
  if ('Standalone' in sessionType) {
    const { provider, model } = sessionType.Standalone;
    return `Standalone(${provider}/${model})`;
  }
  if ('WorkflowRoot' in sessionType) {
    const { active_mode, kernel_session_id } = sessionType.WorkflowRoot;
    return `Workflow(${active_mode}/${kernel_session_id})`;
  }
  return 'Unknown';
}

function mergeImportedRoots(
  existingRoots: RemoteWorkspaceEntry[],
  workspacePath: string,
  displayTitle?: string | null,
): { nextRoots: RemoteWorkspaceEntry[]; added: number } {
  const normalizedPath = normalizeWorkspacePath(workspacePath);
  if (!normalizedPath) {
    return { nextRoots: existingRoots, added: 0 };
  }

  const existingIndex = existingRoots.findIndex((root) => normalizeWorkspacePath(root.path) === normalizedPath);
  if (existingIndex >= 0) {
    const existing = existingRoots[existingIndex];
    if (existing.label?.trim()) {
      return { nextRoots: existingRoots, added: 0 };
    }
    const nextRoots = [...existingRoots];
    nextRoots[existingIndex] = {
      ...existing,
      label: deriveWorkspaceLabel(normalizedPath, displayTitle),
    };
    return { nextRoots, added: 0 };
  }

  return {
    nextRoots: [
      ...existingRoots,
      {
        path: normalizedPath,
        label: deriveWorkspaceLabel(normalizedPath, displayTitle),
        default_provider: null,
        default_model: null,
      },
    ],
    added: 1,
  };
}

function mergeSessionCatalogRoots(
  existingRoots: RemoteWorkspaceEntry[],
  currentWorkspacePath: string,
  sessionCatalog: WorkflowSessionCatalogItem[],
): { nextRoots: RemoteWorkspaceEntry[]; added: number; discovered: number } {
  let nextRoots = existingRoots;
  let added = 0;
  let discovered = 0;

  const candidates = [
    currentWorkspacePath
      ? {
          workspacePath: currentWorkspacePath,
          displayTitle: null,
        }
      : null,
    ...sessionCatalog
      .filter((session) => !!session.workspacePath?.trim())
      .map((session) => ({
        workspacePath: session.workspacePath as string,
        displayTitle: session.displayTitle,
      })),
  ].filter((value): value is { workspacePath: string; displayTitle: string | null } => !!value);

  for (const candidate of candidates) {
    const normalizedPath = normalizeWorkspacePath(candidate.workspacePath);
    if (!normalizedPath) {
      continue;
    }
    discovered += 1;
    const merged = mergeImportedRoots(nextRoots, normalizedPath, candidate.displayTitle);
    nextRoots = merged.nextRoots;
    added += merged.added;
  }

  return { nextRoots, added, discovered };
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function RemoteSection() {
  const { t } = useTranslation('settings');

  const {
    gatewayStatus,
    remoteConfig,
    telegramConfig,
    remoteSessions,
    auditLog,
    saving,
    error,
    fetchGatewayStatus,
    startGateway,
    stopGateway,
    fetchConfig,
    saveConfig,
    fetchTelegramConfig,
    saveTelegramConfig,
    fetchSessions,
    disconnectSession,
    fetchAuditLog,
    clearError,
  } = useRemoteStore();
  const workspacePath = useSettingsStore((state) => state.workspacePath);
  const workflowSessionCatalog = useWorkflowKernelStore((state) => state.sessionCatalog);
  const getSessionCatalogState = useWorkflowKernelStore((state) => state.getSessionCatalogState);

  // Local form state
  const [enabled, setEnabled] = useState(false);
  const [autoStart, setAutoStart] = useState(false);
  const [botToken, setBotToken] = useState('');
  const [hasBotToken, setHasBotToken] = useState(false);
  const [requirePassword, setRequirePassword] = useState(false);
  const [accessPassword, setAccessPassword] = useState('');
  const [hasAccessPassword, setHasAccessPassword] = useState(false);
  const [newChatId, setNewChatId] = useState('');
  const [chatIds, setChatIds] = useState<number[]>([]);
  const [newUserId, setNewUserId] = useState('');
  const [userIds, setUserIds] = useState<number[]>([]);
  const [allowedRoots, setAllowedRoots] = useState<RemoteWorkspaceEntry[]>([]);
  const [newAllowedRootPath, setNewAllowedRootPath] = useState('');
  const [newAllowedRootLabel, setNewAllowedRootLabel] = useState('');
  const [newAllowedRootProvider, setNewAllowedRootProvider] = useState('');
  const [newAllowedRootModel, setNewAllowedRootModel] = useState('');
  const [streamingMode, setStreamingMode] = useState<'WaitForComplete' | 'PeriodicUpdate' | 'LiveEdit'>(
    'WaitForComplete',
  );
  const [periodicIntervalSecs, setPeriodicIntervalSecs] = useState(5);
  const [liveEditThrottleMs, setLiveEditThrottleMs] = useState(1200);
  const [statusMessage, setStatusMessage] = useState<{ type: 'success' | 'warning' | 'error'; text: string } | null>(
    null,
  );

  // Load data on mount
  useEffect(() => {
    fetchGatewayStatus();
    fetchConfig();
    fetchTelegramConfig();
    fetchSessions();
    fetchAuditLog(20);
  }, [fetchGatewayStatus, fetchConfig, fetchTelegramConfig, fetchSessions, fetchAuditLog]);

  // Sync form state with config
  useEffect(() => {
    if (remoteConfig) {
      setEnabled(remoteConfig.enabled);
      setAutoStart(remoteConfig.auto_start);
      setAllowedRoots(remoteConfig.allowed_project_roots ?? []);
    }
  }, [remoteConfig]);

  useEffect(() => {
    if (telegramConfig) {
      setHasBotToken(!!telegramConfig.bot_token && telegramConfig.bot_token !== '');
      setRequirePassword(telegramConfig.require_password);
      setHasAccessPassword(!!telegramConfig.access_password && telegramConfig.access_password !== '');
      setChatIds(telegramConfig.allowed_chat_ids);
      setUserIds(telegramConfig.allowed_user_ids);
      const parsed = parseStreamingMode(telegramConfig.streaming_mode);
      setStreamingMode(parsed.mode);
      setPeriodicIntervalSecs(parsed.intervalSecs);
      setLiveEditThrottleMs(parsed.throttleMs);
    }
  }, [telegramConfig]);

  // Poll status periodically when running
  useEffect(() => {
    if (!gatewayStatus?.running) return;
    const interval = setInterval(() => {
      fetchGatewayStatus();
      fetchSessions();
    }, 10000);
    return () => clearInterval(interval);
  }, [gatewayStatus?.running, fetchGatewayStatus, fetchSessions]);

  const handleSaveGatewayConfig = useCallback(async () => {
    const result = await saveConfig({ enabled, auto_start: autoStart, allowed_project_roots: allowedRoots });
    if (result) {
      setStatusMessage({
        type: result.restart_required ? 'warning' : 'success',
        text: result.restart_required
          ? t('remote.saveRestartRequired', 'Configuration saved. Restart the gateway to apply all changes.')
          : t('remote.saveApplied', 'Configuration applied'),
      });
    }
  }, [allowedRoots, autoStart, enabled, saveConfig, t]);

  const handleSaveTelegramConfig = useCallback(async () => {
    const request: UpdateTelegramConfigRequest = {
      allowed_chat_ids: chatIds,
      allowed_user_ids: userIds,
      require_password: requirePassword,
      streaming_mode: buildStreamingMode(streamingMode, periodicIntervalSecs, liveEditThrottleMs),
    };
    if (botToken && botToken !== '***') {
      request.bot_token = botToken;
    }
    if (accessPassword && accessPassword !== '***') {
      request.access_password = accessPassword;
    }
    const result = await saveTelegramConfig(request);
    if (result) {
      setBotToken('');
      setAccessPassword('');
      setStatusMessage({
        type: result.restart_required ? 'warning' : 'success',
        text: result.restart_required
          ? t('remote.saveRestartRequired', 'Configuration saved. Restart the gateway to apply all changes.')
          : t('remote.saveApplied', 'Configuration applied'),
      });
    }
  }, [
    accessPassword,
    botToken,
    chatIds,
    liveEditThrottleMs,
    periodicIntervalSecs,
    requirePassword,
    saveTelegramConfig,
    streamingMode,
    t,
    userIds,
  ]);

  const handleAddChatId = useCallback(() => {
    const id = parseInt(newChatId, 10);
    if (!isNaN(id) && !chatIds.includes(id)) {
      setChatIds([...chatIds, id]);
      setNewChatId('');
    }
  }, [newChatId, chatIds]);

  const handleRemoveChatId = useCallback(
    (id: number) => {
      setChatIds(chatIds.filter((cid) => cid !== id));
    },
    [chatIds],
  );

  const handleAddUserId = useCallback(() => {
    const id = parseInt(newUserId, 10);
    if (!isNaN(id) && !userIds.includes(id)) {
      setUserIds([...userIds, id]);
      setNewUserId('');
    }
  }, [newUserId, userIds]);

  const handleRemoveUserId = useCallback(
    (id: number) => {
      setUserIds(userIds.filter((uid) => uid !== id));
    },
    [userIds],
  );

  const handleAddAllowedRoot = useCallback(() => {
    const path = newAllowedRootPath.trim();
    if (!path || allowedRoots.some((root) => root.path === path)) {
      return;
    }
    setAllowedRoots([
      ...allowedRoots,
      {
        path,
        label: newAllowedRootLabel.trim() || null,
        default_provider: newAllowedRootProvider.trim() || null,
        default_model: newAllowedRootModel.trim() || null,
      },
    ]);
    setNewAllowedRootPath('');
    setNewAllowedRootLabel('');
    setNewAllowedRootProvider('');
    setNewAllowedRootModel('');
  }, [allowedRoots, newAllowedRootLabel, newAllowedRootModel, newAllowedRootPath, newAllowedRootProvider]);

  const handleRemoveAllowedRoot = useCallback(
    (value: string) => {
      setAllowedRoots(allowedRoots.filter((root) => root.path !== value));
    },
    [allowedRoots],
  );

  const handleImportFromSessions = useCallback(async () => {
    const latestCatalogState = await getSessionCatalogState();
    const latestCatalog = latestCatalogState?.sessions ?? workflowSessionCatalog;
    const { nextRoots, added, discovered } = mergeSessionCatalogRoots(allowedRoots, workspacePath, latestCatalog);
    setAllowedRoots(nextRoots);
    if (discovered === 0) {
      setStatusMessage({
        type: 'warning',
        text: t('remote.config.importNoWorkspaces', 'No open workspaces available to import.'),
      });
      return;
    }
    if (added === 0) {
      setStatusMessage({
        type: 'warning',
        text: t('remote.config.importNoNewWorkspaces', 'All open workspaces are already included.'),
      });
      return;
    }
    setStatusMessage({
      type: 'success',
      text: t('remote.config.importedWorkspaces', {
        defaultValue: 'Imported {{count}} workspace(s) from session management.',
        count: added,
      }),
    });
  }, [allowedRoots, getSessionCatalogState, t, workflowSessionCatalog, workspacePath]);

  return (
    <div className="space-y-8">
      {/* Error Display */}
      {error && (
        <div className="flex items-center gap-2 p-3 rounded-lg bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300">
          <CrossCircledIcon className="w-4 h-4 shrink-0" />
          <span className="text-sm">{error}</span>
          <button onClick={clearError} className="ml-auto text-sm underline">
            {t('remote.dismiss', 'Dismiss')}
          </button>
        </div>
      )}

      {/* Status Message */}
      {statusMessage && (
        <div
          className={clsx(
            'flex items-center gap-2 p-3 rounded-lg text-sm',
            statusMessage.type === 'success'
              ? 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-300'
              : statusMessage.type === 'warning'
                ? 'bg-yellow-50 dark:bg-yellow-900/20 text-yellow-700 dark:text-yellow-300'
                : 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300',
          )}
        >
          {statusMessage.type === 'success' ? (
            <CheckCircledIcon className="w-4 h-4 shrink-0" />
          ) : statusMessage.type === 'warning' ? (
            <CrossCircledIcon className="w-4 h-4 shrink-0" />
          ) : (
            <CrossCircledIcon className="w-4 h-4 shrink-0" />
          )}
          <span>{statusMessage.text}</span>
        </div>
      )}

      {/* Gateway Status Panel */}
      <section>
        <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-4">
          {t('remote.gateway.title', 'Gateway Status')}
        </h3>
        <div className="bg-gray-50 dark:bg-gray-800/50 rounded-lg p-4 space-y-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <div className={clsx('w-3 h-3 rounded-full', gatewayStatus?.running ? 'bg-green-500' : 'bg-gray-400')} />
              <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                {gatewayStatus?.reconnecting
                  ? t('remote.gateway.reconnecting', 'Reconnecting...')
                  : gatewayStatus?.running
                    ? t('remote.gateway.running', 'Running')
                    : t('remote.gateway.stopped', 'Stopped')}
              </span>
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => startGateway()}
                disabled={saving || gatewayStatus?.running || gatewayStatus?.reconnecting}
                className={clsx(
                  'px-3 py-1.5 rounded-md text-sm font-medium',
                  'bg-green-600 text-white hover:bg-green-700',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                )}
              >
                {t('remote.gateway.start', 'Start')}
              </button>
              <button
                onClick={() => stopGateway()}
                disabled={saving || !gatewayStatus?.running}
                className={clsx(
                  'px-3 py-1.5 rounded-md text-sm font-medium',
                  'bg-red-600 text-white hover:bg-red-700',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                )}
              >
                {t('remote.gateway.stop', 'Stop')}
              </button>
            </div>
          </div>
          {gatewayStatus?.error && (
            <div className="rounded-md bg-red-50 dark:bg-red-900/20 px-3 py-2 text-sm text-red-700 dark:text-red-300">
              {gatewayStatus.error}
            </div>
          )}
          {gatewayStatus?.running && (
            <div className="grid grid-cols-3 gap-4 text-sm text-gray-600 dark:text-gray-400">
              <div>
                <span className="block text-xs uppercase tracking-wide">
                  {t('remote.gateway.connectedSince', 'Connected Since')}
                </span>
                <span className="font-mono">
                  {gatewayStatus.connected_since ? new Date(gatewayStatus.connected_since).toLocaleString() : '-'}
                </span>
              </div>
              <div>
                <span className="block text-xs uppercase tracking-wide">
                  {t('remote.gateway.commandsProcessed', 'Commands')}
                </span>
                <span className="font-mono">{gatewayStatus.total_commands_processed}</span>
              </div>
              <div>
                <span className="block text-xs uppercase tracking-wide">
                  {t('remote.gateway.activeSessions', 'Active Sessions')}
                </span>
                <span className="font-mono">{gatewayStatus.active_remote_sessions}</span>
              </div>
            </div>
          )}
          {(gatewayStatus?.reconnecting || gatewayStatus?.last_error_at) && (
            <div className="grid grid-cols-2 gap-4 text-sm text-gray-600 dark:text-gray-400">
              <div>
                <span className="block text-xs uppercase tracking-wide">
                  {t('remote.gateway.reconnectAttempt', 'Reconnect attempt {{current}} of {{max}}', {
                    current: gatewayStatus.reconnect_attempts ?? 0,
                    max: 5,
                  })}
                </span>
                <span className="font-mono">{gatewayStatus.reconnect_attempts ?? 0}</span>
              </div>
              <div>
                <span className="block text-xs uppercase tracking-wide">
                  {t('remote.gateway.lastErrorAt', 'Last Error')}
                </span>
                <span className="font-mono">
                  {gatewayStatus.last_error_at ? new Date(gatewayStatus.last_error_at).toLocaleString() : '-'}
                </span>
              </div>
            </div>
          )}
        </div>
      </section>

      {/* Gateway Configuration */}
      <section>
        <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-4">
          {t('remote.config.title', 'Gateway Configuration')}
        </h3>
        <div className="space-y-4">
          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
              className="w-4 h-4 rounded text-primary-600"
            />
            <span className="text-sm text-gray-700 dark:text-gray-300">
              {t('remote.config.enabled', 'Enable Remote Control')}
            </span>
          </label>
          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={autoStart}
              onChange={(e) => setAutoStart(e.target.checked)}
              className="w-4 h-4 rounded text-primary-600"
            />
            <span className="text-sm text-gray-700 dark:text-gray-300">
              {t('remote.config.autoStart', 'Auto-start on launch')}
            </span>
          </label>
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('remote.config.allowedRoots', 'Allowed Project Roots')}
            </label>
            <div className="flex justify-end mb-2">
              <button
                onClick={() => void handleImportFromSessions()}
                disabled={saving}
                className={clsx(
                  'px-3 py-1.5 rounded-lg text-sm font-medium',
                  'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-200',
                  'hover:bg-gray-200 dark:hover:bg-gray-600',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                )}
              >
                {t('remote.config.importFromSessions', 'Import Open Workspaces')}
              </button>
            </div>
            <div className="flex flex-wrap gap-2 mb-2">
              {allowedRoots.map((root) => (
                <span
                  key={root.path}
                  className="inline-flex items-center gap-1 px-2 py-1 rounded-md bg-gray-100 dark:bg-gray-700 text-sm"
                >
                  <span className="font-medium">
                    {root.label?.trim() || root.path.split('/').filter(Boolean).slice(-1)[0] || root.path}
                  </span>
                  <span className="text-gray-500 dark:text-gray-400">{root.path}</span>
                  {(root.default_provider || root.default_model) && (
                    <span className="text-gray-500 dark:text-gray-400">
                      {root.default_provider || '-'} / {root.default_model || '-'}
                    </span>
                  )}
                  <button
                    onClick={() => handleRemoveAllowedRoot(root.path)}
                    className="text-gray-400 hover:text-red-500"
                  >
                    <CrossCircledIcon className="w-3 h-3" />
                  </button>
                </span>
              ))}
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
              <input
                type="text"
                value={newAllowedRootLabel}
                onChange={(e) => setNewAllowedRootLabel(e.target.value)}
                placeholder={t('remote.config.allowedRootsLabelPlaceholder', 'Workspace label (optional)')}
                className={clsx(
                  'px-3 py-2 rounded-lg text-sm',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
              />
              <input
                type="text"
                value={newAllowedRootPath}
                onChange={(e) => setNewAllowedRootPath(e.target.value)}
                placeholder={t('remote.config.allowedRootsPlaceholder', 'Absolute path...')}
                className={clsx(
                  'px-3 py-2 rounded-lg text-sm',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
                onKeyDown={(e) => e.key === 'Enter' && handleAddAllowedRoot()}
              />
              <input
                type="text"
                value={newAllowedRootProvider}
                onChange={(e) => setNewAllowedRootProvider(e.target.value)}
                placeholder={t('remote.config.allowedRootsProviderPlaceholder', 'Default provider (optional)')}
                className={clsx(
                  'px-3 py-2 rounded-lg text-sm',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
              />
              <input
                type="text"
                value={newAllowedRootModel}
                onChange={(e) => setNewAllowedRootModel(e.target.value)}
                placeholder={t('remote.config.allowedRootsModelPlaceholder', 'Default model (optional)')}
                className={clsx(
                  'px-3 py-2 rounded-lg text-sm',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
              />
            </div>
            <div className="flex justify-end mt-2">
              <button
                onClick={handleAddAllowedRoot}
                aria-label={t('remote.config.addWorkspace', 'Add workspace')}
                className="px-3 py-2 rounded-lg bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600"
              >
                <PlusIcon className="w-4 h-4" />
              </button>
            </div>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
              {t(
                'remote.config.allowedRootsHint',
                'Only these directories can be opened from remote commands. At least one path is required.',
              )}
            </p>
          </div>
          <button
            onClick={handleSaveGatewayConfig}
            disabled={saving}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'bg-primary-600 text-white hover:bg-primary-700',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {saving ? t('remote.saving', 'Saving...') : t('remote.save', 'Save')}
          </button>
        </div>
      </section>

      {/* Telegram Bot Configuration */}
      <section>
        <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-4">
          {t('remote.telegram.title', 'Telegram Bot Configuration')}
        </h3>
        <div className="space-y-4">
          {/* Bot Token */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('remote.telegram.botToken', 'Bot Token')}
            </label>
            <div className="flex gap-2">
              <input
                type="password"
                value={botToken}
                onChange={(e) => setBotToken(e.target.value)}
                placeholder={hasBotToken ? '***' : t('remote.telegram.botTokenPlaceholder', 'Enter bot token...')}
                className={clsx(
                  'flex-1 px-3 py-2 rounded-lg text-sm',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
              />
              {hasBotToken && (
                <span className="flex items-center text-xs text-green-600 dark:text-green-400">
                  <CheckCircledIcon className="w-4 h-4 mr-1" />
                  {t('remote.telegram.tokenSet', 'Set')}
                </span>
              )}
            </div>
          </div>

          {/* Allowed Chat IDs */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('remote.telegram.allowedChatIds', 'Allowed Chat IDs')}
            </label>
            <div className="flex flex-wrap gap-2 mb-2">
              {chatIds.map((id) => (
                <span
                  key={id}
                  className="inline-flex items-center gap-1 px-2 py-1 rounded-md bg-gray-100 dark:bg-gray-700 text-sm"
                >
                  {id}
                  <button onClick={() => handleRemoveChatId(id)} className="text-gray-400 hover:text-red-500">
                    <CrossCircledIcon className="w-3 h-3" />
                  </button>
                </span>
              ))}
            </div>
            <div className="flex gap-2">
              <input
                type="text"
                value={newChatId}
                onChange={(e) => setNewChatId(e.target.value)}
                placeholder={t('remote.telegram.chatIdPlaceholder', 'Chat ID...')}
                className={clsx(
                  'flex-1 px-3 py-2 rounded-lg text-sm',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
                onKeyDown={(e) => e.key === 'Enter' && handleAddChatId()}
              />
              <button
                onClick={handleAddChatId}
                className="px-3 py-2 rounded-lg bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600"
              >
                <PlusIcon className="w-4 h-4" />
              </button>
            </div>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
              {t('remote.telegram.chatIdHint', 'Leave empty to allow all chats.')}
            </p>
          </div>

          {/* Allowed User IDs */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('remote.telegram.allowedUserIds', 'Allowed User IDs')}
            </label>
            <div className="flex flex-wrap gap-2 mb-2">
              {userIds.map((id) => (
                <span
                  key={id}
                  className="inline-flex items-center gap-1 px-2 py-1 rounded-md bg-gray-100 dark:bg-gray-700 text-sm"
                >
                  {id}
                  <button onClick={() => handleRemoveUserId(id)} className="text-gray-400 hover:text-red-500">
                    <CrossCircledIcon className="w-3 h-3" />
                  </button>
                </span>
              ))}
            </div>
            <div className="flex gap-2">
              <input
                type="text"
                value={newUserId}
                onChange={(e) => setNewUserId(e.target.value)}
                placeholder={t('remote.telegram.userIdPlaceholder', 'User ID...')}
                className={clsx(
                  'flex-1 px-3 py-2 rounded-lg text-sm',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
                onKeyDown={(e) => e.key === 'Enter' && handleAddUserId()}
              />
              <button
                onClick={handleAddUserId}
                className="px-3 py-2 rounded-lg bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600"
              >
                <PlusIcon className="w-4 h-4" />
              </button>
            </div>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
              {t('remote.telegram.userIdHint', 'Leave empty to allow all users.')}
            </p>
          </div>

          {/* Password Protection */}
          <div>
            <label className="flex items-center gap-3 mb-2">
              <input
                type="checkbox"
                checked={requirePassword}
                onChange={(e) => setRequirePassword(e.target.checked)}
                className="w-4 h-4 rounded text-primary-600"
              />
              <span className="text-sm text-gray-700 dark:text-gray-300">
                {t('remote.telegram.passwordProtection', 'Require Password')}
              </span>
            </label>
            {requirePassword && (
              <div className="ml-7">
                <input
                  type="password"
                  value={accessPassword}
                  onChange={(e) => setAccessPassword(e.target.value)}
                  placeholder={
                    hasAccessPassword ? '***' : t('remote.telegram.accessPasswordPlaceholder', 'Set access password...')
                  }
                  className={clsx(
                    'w-full px-3 py-2 rounded-lg text-sm',
                    'bg-white dark:bg-gray-800',
                    'border border-gray-300 dark:border-gray-600',
                    'focus:outline-none focus:ring-2 focus:ring-primary-500',
                  )}
                />
              </div>
            )}
          </div>

          {/* Streaming Mode */}
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              {t('remote.telegram.streamingMode', 'Streaming Mode')}
            </label>
            <select
              value={streamingMode}
              onChange={(e) => setStreamingMode(e.target.value as 'WaitForComplete' | 'PeriodicUpdate' | 'LiveEdit')}
              className={clsx(
                'w-full px-3 py-2 rounded-lg text-sm',
                'bg-white dark:bg-gray-800',
                'border border-gray-300 dark:border-gray-600',
                'focus:outline-none focus:ring-2 focus:ring-primary-500',
              )}
            >
              <option value="WaitForComplete">{t('remote.telegram.modeWaitForComplete', 'Wait for Complete')}</option>
              <option value="PeriodicUpdate">{t('remote.telegram.modePeriodicUpdate', 'Periodic Update')}</option>
              <option value="LiveEdit">{t('remote.telegram.modeLiveEdit', 'Live Edit')}</option>
            </select>
          </div>
          {streamingMode === 'PeriodicUpdate' && (
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                {t('remote.telegram.periodicInterval', 'Update Interval (seconds)')}
              </label>
              <input
                type="number"
                min={1}
                value={periodicIntervalSecs}
                onChange={(e) => setPeriodicIntervalSecs(Math.max(1, parseInt(e.target.value || '1', 10)))}
                className={clsx(
                  'w-full px-3 py-2 rounded-lg text-sm',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
              />
            </div>
          )}
          {streamingMode === 'LiveEdit' && (
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                {t('remote.telegram.liveEditThrottle', 'Edit Throttle (ms)')}
              </label>
              <input
                type="number"
                min={100}
                value={liveEditThrottleMs}
                onChange={(e) => setLiveEditThrottleMs(Math.max(100, parseInt(e.target.value || '100', 10)))}
                className={clsx(
                  'w-full px-3 py-2 rounded-lg text-sm',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
              />
            </div>
          )}

          <button
            onClick={handleSaveTelegramConfig}
            disabled={saving}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'bg-primary-600 text-white hover:bg-primary-700',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {saving ? t('remote.saving', 'Saving...') : t('remote.save', 'Save')}
          </button>
        </div>
      </section>

      {/* Active Remote Sessions */}
      <section>
        <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-4">
          {t('remote.sessions.title', 'Active Remote Sessions')}
        </h3>
        {remoteSessions.length === 0 ? (
          <p className="text-sm text-gray-500 dark:text-gray-400">
            {t('remote.sessions.none', 'No active remote sessions.')}
          </p>
        ) : (
          <div className="space-y-2">
            {remoteSessions.map((session) => (
              <div
                key={session.chat_id}
                className="flex items-center justify-between p-3 rounded-lg bg-gray-50 dark:bg-gray-800/50"
              >
                <div>
                  <div className="text-sm font-medium text-gray-700 dark:text-gray-300">Chat {session.chat_id}</div>
                  <div className="text-xs text-gray-500 dark:text-gray-400">
                    Session: {session.local_session_id ?? 'N/A'} | Type: {formatRemoteSessionType(session.session_type)}
                    {session.project_path && <span className="ml-1">| Path: {session.project_path}</span>}
                  </div>
                </div>
                <button
                  onClick={() => disconnectSession(session.chat_id)}
                  disabled={saving}
                  className="p-2 rounded-lg text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20"
                  title={t('remote.sessions.disconnect', 'Disconnect')}
                >
                  <TrashIcon className="w-4 h-4" />
                </button>
              </div>
            ))}
          </div>
        )}
      </section>

      {/* Audit Log */}
      <section>
        <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-4">
          {t('remote.audit.title', 'Audit Log')}
        </h3>
        {!auditLog || auditLog.entries.length === 0 ? (
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('remote.audit.none', 'No audit log entries.')}</p>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-left text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400 border-b border-gray-200 dark:border-gray-700">
                  <th className="pb-2 pr-4">{t('remote.audit.time', 'Time')}</th>
                  <th className="pb-2 pr-4">{t('remote.audit.user', 'User')}</th>
                  <th className="pb-2 pr-4">{t('remote.audit.command', 'Command')}</th>
                  <th className="pb-2 pr-4">{t('remote.audit.status', 'Status')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
                {auditLog.entries.map((entry) => (
                  <tr key={entry.id}>
                    <td className="py-2 pr-4 text-gray-600 dark:text-gray-400 font-mono text-xs">
                      {new Date(entry.created_at).toLocaleString()}
                    </td>
                    <td className="py-2 pr-4 text-gray-700 dark:text-gray-300">
                      {entry.username ?? `User ${entry.user_id}`}
                    </td>
                    <td className="py-2 pr-4 text-gray-700 dark:text-gray-300">{entry.command_type}</td>
                    <td className="py-2 pr-4">
                      <span
                        className={clsx(
                          'inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium',
                          entry.result_status === 'success'
                            ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
                            : entry.result_status === 'unauthorized'
                              ? 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-300'
                              : 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
                        )}
                      >
                        {entry.result_status}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {auditLog.total > auditLog.entries.length && (
              <button
                onClick={() => fetchAuditLog(auditLog.total)}
                className="mt-2 text-sm text-primary-600 dark:text-primary-400 hover:underline"
              >
                {t('remote.audit.viewAll', 'View all entries')} ({auditLog.total})
              </button>
            )}
          </div>
        )}
      </section>
    </div>
  );
}
