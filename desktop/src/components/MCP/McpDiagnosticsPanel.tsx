import { clsx } from 'clsx';
import { InfoCircledIcon } from '@radix-ui/react-icons';
import { useTranslation } from 'react-i18next';
import type { ConnectedServerInfo, McpRuntimeInfo } from '../../types/mcp';
import type { McpEventRecord } from './useMcpRegistryController';

interface McpDiagnosticsPanelProps {
  showDiagnostics: boolean;
  onToggle: () => void;
  connectedServers: Record<string, ConnectedServerInfo>;
  runtimeLoading: boolean;
  runtimeInventory: McpRuntimeInfo[];
  repairingRuntimes: Set<string>;
  onRepairRuntime: (runtimeKind: string) => void;
  eventLog: McpEventRecord[];
  getEventActionLabel: (action: string) => string;
}

export function McpDiagnosticsPanel({
  showDiagnostics,
  onToggle,
  connectedServers,
  runtimeLoading,
  runtimeInventory,
  repairingRuntimes,
  onRepairRuntime,
  eventLog,
  getEventActionLabel,
}: McpDiagnosticsPanelProps) {
  const { t } = useTranslation();

  return (
    <div className="mt-4 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
      <button
        type="button"
        onClick={onToggle}
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
        <div className="p-3 space-y-3 bg-white dark:bg-gray-900">
          {Object.values(connectedServers).length === 0 ? (
            <p className="text-xs text-gray-500 dark:text-gray-400">{t('mcp.noConnectedServers')}</p>
          ) : (
            Object.values(connectedServers).map((info) => (
              <div key={info.server_id} className="text-xs rounded border border-gray-200 dark:border-gray-700 p-2">
                <p className="font-medium text-gray-900 dark:text-white">{info.server_name}</p>
                <p className="text-gray-500 dark:text-gray-400">
                  {t('mcp.connectionMeta', {
                    protocol: info.protocol_version || t('mcp.status.unknown'),
                    count: info.tool_names.length,
                  })}
                </p>
                {info.tool_names.length > 0 && (
                  <p className="text-gray-600 dark:text-gray-300 mt-1 break-all">{info.tool_names.join(', ')}</p>
                )}
              </div>
            ))
          )}

          <div className="pt-2 border-t border-gray-200 dark:border-gray-700">
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
                            onClick={() => onRepairRuntime(runtime.runtime)}
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

          <div className="pt-2 border-t border-gray-200 dark:border-gray-700">
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
  );
}
