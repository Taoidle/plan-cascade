/**
 * ConnectionStatus Component
 *
 * Displays the current backend connection status (Tauri IPC).
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { ConnectionStatus as ConnectionStatusType } from '../../lib/claudeCodeClient';

interface ConnectionStatusProps {
  status: ConnectionStatusType;
}

export function ConnectionStatus({ status }: ConnectionStatusProps) {
  const { t } = useTranslation();

  const getStatusInfo = () => {
    switch (status) {
      case 'connected':
        return {
          color: 'bg-green-500',
          text: t('connection.connected'),
          textColor: 'text-green-600 dark:text-green-400',
        };
      case 'connecting':
        return {
          color: 'bg-yellow-500 animate-pulse',
          text: t('connection.connecting'),
          textColor: 'text-yellow-600 dark:text-yellow-400',
        };
      case 'reconnecting':
        return {
          color: 'bg-yellow-500 animate-pulse',
          text: t('connection.reconnecting'),
          textColor: 'text-yellow-600 dark:text-yellow-400',
        };
      case 'disconnected':
      default:
        return {
          color: 'bg-red-500',
          text: t('connection.disconnected'),
          textColor: 'text-red-600 dark:text-red-400',
        };
    }
  };

  const { color, text, textColor } = getStatusInfo();

  return (
    <div className="flex items-center gap-2">
      <div
        className={clsx(
          'w-2 h-2 rounded-full',
          color
        )}
      />
      <span className={clsx('text-xs font-medium', textColor)}>
        {text}
      </span>
    </div>
  );
}

export default ConnectionStatus;
