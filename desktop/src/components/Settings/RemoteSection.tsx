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
import type { UpdateTelegramConfigRequest } from '../../lib/remoteApi';

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
  const [streamingMode, setStreamingMode] = useState<string>('WaitForComplete');
  const [statusMessage, setStatusMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

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
    }
  }, [remoteConfig]);

  useEffect(() => {
    if (telegramConfig) {
      setHasBotToken(!!telegramConfig.bot_token && telegramConfig.bot_token !== '');
      setRequirePassword(telegramConfig.require_password);
      setHasAccessPassword(!!telegramConfig.access_password && telegramConfig.access_password !== '');
      setChatIds(telegramConfig.allowed_chat_ids);
      setUserIds(telegramConfig.allowed_user_ids);
      if (typeof telegramConfig.streaming_mode === 'string') {
        setStreamingMode(telegramConfig.streaming_mode);
      } else if (telegramConfig.streaming_mode && 'PeriodicUpdate' in telegramConfig.streaming_mode) {
        setStreamingMode('PeriodicUpdate');
      } else if (telegramConfig.streaming_mode && 'LiveEdit' in telegramConfig.streaming_mode) {
        setStreamingMode('LiveEdit');
      }
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
    const success = await saveConfig({ enabled, auto_start: autoStart });
    if (success) {
      setStatusMessage({ type: 'success', text: t('remote.saveSuccess', 'Configuration saved') });
    }
  }, [enabled, autoStart, saveConfig, t]);

  const handleSaveTelegramConfig = useCallback(async () => {
    const request: UpdateTelegramConfigRequest = {
      allowed_chat_ids: chatIds,
      allowed_user_ids: userIds,
      require_password: requirePassword,
    };
    if (botToken && botToken !== '***') {
      request.bot_token = botToken;
    }
    if (accessPassword && accessPassword !== '***') {
      request.access_password = accessPassword;
    }
    const success = await saveTelegramConfig(request);
    if (success) {
      setBotToken('');
      setAccessPassword('');
      setStatusMessage({ type: 'success', text: t('remote.saveSuccess', 'Configuration saved') });
    }
  }, [chatIds, userIds, requirePassword, botToken, accessPassword, saveTelegramConfig, t]);

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
              : 'bg-red-50 dark:bg-red-900/20 text-red-700 dark:text-red-300',
          )}
        >
          {statusMessage.type === 'success' ? (
            <CheckCircledIcon className="w-4 h-4 shrink-0" />
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
                {gatewayStatus?.running
                  ? t('remote.gateway.running', 'Running')
                  : t('remote.gateway.stopped', 'Stopped')}
              </span>
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => startGateway()}
                disabled={saving || gatewayStatus?.running}
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
              onChange={(e) => setStreamingMode(e.target.value)}
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
                    Session: {session.local_session_id ?? 'N/A'} | Type: {session.session_type}
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
