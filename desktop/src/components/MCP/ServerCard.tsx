/**
 * ServerCard Component
 *
 * Displays a single MCP server with status, type, and action buttons.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import * as Switch from '@radix-ui/react-switch';
import { PlayIcon, Pencil1Icon, TrashIcon, GearIcon, Link2Icon, ListBulletIcon } from '@radix-ui/react-icons';
import type { McpServer, McpServerStatus, ConnectedServerInfo } from '../../types/mcp';
import { isStatusError } from '../../types/mcp';

interface ServerCardProps {
  server: McpServer;
  connected?: boolean;
  connectedInfo?: ConnectedServerInfo;
  onTest: () => void;
  onToggle: (enabled: boolean) => void;
  onConnect: () => void;
  onDisconnect: () => void;
  onEdit: () => void;
  onDelete: () => void;
  onViewTools?: () => void;
  isConnecting?: boolean;
  isDisconnecting?: boolean;
  isTesting?: boolean;
  isToggling?: boolean;
  isDeleting?: boolean;
}

export function ServerCard({
  server,
  connected = false,
  connectedInfo,
  onTest,
  onToggle,
  onConnect,
  onDisconnect,
  onEdit,
  onDelete,
  onViewTools,
  isConnecting = false,
  isDisconnecting = false,
  isTesting = false,
  isToggling = false,
  isDeleting = false,
}: ServerCardProps) {
  const { t } = useTranslation();

  const statusColor = getServerStatusColor(server.status, server.enabled);
  const statusLabel = getStatusDisplay(server.status, t);
  const isConnectBusy = isConnecting || isDisconnecting;
  const isAnyBusy = isConnecting || isDisconnecting || isTesting || isToggling || isDeleting;

  return (
    <div
      className={clsx(
        'p-4 rounded-lg border transition-colors',
        server.enabled
          ? 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700'
          : 'bg-gray-50 dark:bg-gray-900 border-gray-200 dark:border-gray-800 opacity-75',
      )}
    >
      {/* Header Row */}
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-3">
          {/* Status Indicator */}
          <div
            className={clsx('w-2.5 h-2.5 rounded-full', statusColor, server.status === 'connected' && 'animate-pulse')}
          />

          {/* Server Name */}
          <div>
            <h3 className="font-semibold text-gray-900 dark:text-white">{server.name}</h3>
            <div className="flex items-center gap-2 mt-0.5">
              {/* Type Badge */}
              <span
                className={clsx(
                  'px-1.5 py-0.5 rounded text-xs font-medium',
                  server.server_type === 'stdio'
                    ? 'bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300'
                    : 'bg-emerald-100 dark:bg-emerald-900/50 text-emerald-700 dark:text-emerald-300',
                )}
              >
                {server.server_type === 'stdio' ? t('mcp.serverTypeStdio') : t('mcp.serverTypeStreamHttp')}
              </span>
              {/* Status Label */}
              <span className={clsx('text-xs', getStatusTextColor(server.status))}>{statusLabel}</span>
            </div>
          </div>
        </div>

        {/* Enable/Disable Toggle */}
        <Switch.Root
          checked={server.enabled}
          onCheckedChange={onToggle}
          disabled={isToggling || isDeleting}
          className={clsx(
            'w-9 h-5 rounded-full relative',
            'bg-gray-300 dark:bg-gray-600',
            'data-[state=checked]:bg-primary-600',
            'transition-colors',
            'disabled:opacity-50',
            'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-2',
            'dark:focus:ring-offset-gray-800',
          )}
        >
          <Switch.Thumb
            className={clsx(
              'block w-4 h-4 rounded-full bg-white',
              'transition-transform',
              'translate-x-0.5',
              'data-[state=checked]:translate-x-[18px]',
            )}
          />
        </Switch.Root>
      </div>

      {/* Server Info */}
      <div className="mb-3">
        {server.server_type === 'stdio' && server.command && (
          <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
            <GearIcon className="w-3.5 h-3.5" />
            <code className="truncate max-w-xs">
              {server.command} {server.args.join(' ')}
            </code>
          </div>
        )}
        {server.server_type === 'stream_http' && server.url && (
          <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
            <Link2Icon className="w-3.5 h-3.5" />
            <code className="truncate max-w-xs">{server.url}</code>
          </div>
        )}
        {connectedInfo && (
          <div className="mt-1 text-[11px] text-gray-500 dark:text-gray-400 space-y-0.5">
            <p>
              protocol={connectedInfo.protocol_version || 'unknown'} tools={connectedInfo.tool_names.length}
            </p>
            {connectedInfo.connected_at && <p>connected_at={connectedInfo.connected_at}</p>}
            {connectedInfo.last_error && (
              <p className="text-red-600 dark:text-red-400">last_error={connectedInfo.last_error}</p>
            )}
          </div>
        )}
      </div>

      {/* Action Buttons */}
      <div className="flex items-center gap-2">
        <button
          onClick={connected ? onDisconnect : onConnect}
          disabled={isConnectBusy || isDeleting || !server.enabled}
          className={clsx(
            'flex items-center gap-1 px-2.5 py-1.5 rounded-md',
            connected
              ? 'bg-amber-100 dark:bg-amber-900/50 text-amber-700 dark:text-amber-300 hover:bg-amber-200 dark:hover:bg-amber-800'
              : 'bg-green-100 dark:bg-green-900/50 text-green-700 dark:text-green-300 hover:bg-green-200 dark:hover:bg-green-800',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'text-xs font-medium transition-colors',
          )}
        >
          <span>
            {connected
              ? isDisconnecting
                ? t('mcp.disconnecting', 'Disconnecting...')
                : t('mcp.disconnect')
              : isConnecting
                ? t('mcp.connecting', 'Connecting...')
                : t('mcp.connect')}
          </span>
        </button>

        <button
          onClick={onTest}
          disabled={isTesting || isAnyBusy || !server.enabled}
          className={clsx(
            'flex items-center gap-1 px-2.5 py-1.5 rounded-md',
            'bg-primary-100 dark:bg-primary-900/50',
            'text-primary-700 dark:text-primary-300',
            'hover:bg-primary-200 dark:hover:bg-primary-800',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'text-xs font-medium transition-colors',
          )}
        >
          <PlayIcon className="w-3 h-3" />
          <span>{isTesting ? t('mcp.testing', 'Testing...') : t('mcp.test')}</span>
        </button>

        {connectedInfo && onViewTools && (
          <button
            onClick={onViewTools}
            disabled={isAnyBusy}
            className={clsx(
              'flex items-center gap-1 px-2.5 py-1.5 rounded-md',
              'bg-indigo-100 dark:bg-indigo-900/50',
              'text-indigo-700 dark:text-indigo-300',
              'hover:bg-indigo-200 dark:hover:bg-indigo-800',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'text-xs font-medium transition-colors',
            )}
          >
            <ListBulletIcon className="w-3 h-3" />
            <span>{t('mcp.viewTools', 'View Tools')}</span>
          </button>
        )}

        <button
          onClick={onEdit}
          disabled={isAnyBusy}
          className={clsx(
            'flex items-center gap-1 px-2.5 py-1.5 rounded-md',
            'bg-gray-100 dark:bg-gray-700',
            'text-gray-700 dark:text-gray-300',
            'hover:bg-gray-200 dark:hover:bg-gray-600',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'text-xs font-medium transition-colors',
          )}
        >
          <Pencil1Icon className="w-3 h-3" />
          <span>{t('mcp.edit')}</span>
        </button>

        <button
          onClick={onDelete}
          disabled={isAnyBusy}
          className={clsx(
            'flex items-center gap-1 px-2.5 py-1.5 rounded-md',
            'bg-red-100 dark:bg-red-900/50',
            'text-red-700 dark:text-red-300',
            'hover:bg-red-200 dark:hover:bg-red-800',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'text-xs font-medium transition-colors ml-auto',
          )}
        >
          <TrashIcon className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
}

function getStatusDisplay(status: McpServerStatus, t: TFunction): string {
  if (status === 'connected') return t('mcp.status.connected');
  if (status === 'disconnected') return t('mcp.status.disconnected');
  if (status === 'unknown') return t('mcp.status.unknown');
  if (isStatusError(status)) return t('mcp.status.error', { message: status.error });
  return t('mcp.status.unknown');
}

function getServerStatusColor(status: McpServerStatus, enabled: boolean): string {
  if (!enabled) return 'bg-gray-400';
  if (status === 'connected') return 'bg-green-500';
  if (status === 'disconnected') return 'bg-gray-400';
  if (status === 'unknown') return 'bg-yellow-500';
  if (isStatusError(status)) return 'bg-red-500';
  return 'bg-gray-400';
}

function getStatusTextColor(status: McpServerStatus): string {
  if (status === 'connected') return 'text-green-600 dark:text-green-400';
  if (status === 'disconnected') return 'text-gray-500 dark:text-gray-400';
  if (status === 'unknown') return 'text-yellow-600 dark:text-yellow-400';
  if (isStatusError(status)) return 'text-red-600 dark:text-red-400';
  return 'text-gray-500';
}

export default ServerCard;
