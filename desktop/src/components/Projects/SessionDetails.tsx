/**
 * SessionDetails Component
 *
 * Shows detailed view of a session including messages and metadata.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  ArrowLeftIcon,
  PlayIcon,
  ClockIcon,
  ChatBubbleIcon,
  CheckCircledIcon,
  PersonIcon,
  GearIcon,
} from '@radix-ui/react-icons';
import { useProjectsStore } from '../../store/projects';
import { formatRelativeTime } from './utils';

export function SessionDetails() {
  const { t } = useTranslation();
  const { selectedSession, sessionDetails, loading, selectSession, resumeSession } = useProjectsStore();

  if (!selectedSession) {
    return null;
  }

  const handleResume = async () => {
    const result = await resumeSession(selectedSession.file_path);
    if (result?.success) {
      navigator.clipboard.writeText(result.resume_command);
      // TODO: Show toast notification
    }
  };

  const handleBack = () => {
    selectSession(null);
  };

  const getMessageIcon = (type: string) => {
    switch (type) {
      case 'user':
        return <PersonIcon className="w-4 h-4" />;
      case 'assistant':
        return <ChatBubbleIcon className="w-4 h-4" />;
      case 'tool_call':
      case 'tool_result':
        return <GearIcon className="w-4 h-4" />;
      default:
        return <ChatBubbleIcon className="w-4 h-4" />;
    }
  };

  const getMessageLabel = (type: string) => {
    switch (type) {
      case 'user':
        return t('projects.messageType.user');
      case 'assistant':
        return t('projects.messageType.assistant');
      case 'tool_call':
        return t('projects.messageType.toolCall');
      case 'tool_result':
        return t('projects.messageType.toolResult');
      default:
        return type;
    }
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <button
              onClick={handleBack}
              className={clsx(
                'p-1.5 rounded-md',
                'hover:bg-gray-100 dark:hover:bg-gray-800',
                'text-gray-500 dark:text-gray-400',
              )}
            >
              <ArrowLeftIcon className="w-4 h-4" />
            </button>
            <h2 className="text-lg font-semibold text-gray-900 dark:text-white">{t('projects.sessionDetails')}</h2>
          </div>

          <button
            onClick={handleResume}
            className={clsx(
              'flex items-center gap-1.5 px-3 py-1.5 rounded-md',
              'bg-primary-600 hover:bg-primary-700',
              'text-white text-sm font-medium',
              'transition-colors',
            )}
          >
            <PlayIcon className="w-4 h-4" />
            <span>{t('projects.resume')}</span>
          </button>
        </div>

        {/* Session Stats */}
        <div className="flex items-center gap-4 text-xs text-gray-500 dark:text-gray-400">
          <span className="flex items-center gap-1">
            <ClockIcon className="w-3.5 h-3.5" />
            <span>{formatRelativeTime(selectedSession.created_at)}</span>
          </span>
          <span className="flex items-center gap-1">
            <ChatBubbleIcon className="w-3.5 h-3.5" />
            <span>
              {selectedSession.message_count} {t('projects.messages', { count: selectedSession.message_count })}
            </span>
          </span>
          {sessionDetails && sessionDetails.checkpoint_count > 0 && (
            <span className="flex items-center gap-1">
              <CheckCircledIcon className="w-3.5 h-3.5 text-green-500" />
              <span>
                {sessionDetails.checkpoint_count}{' '}
                {t('projects.checkpoints', { count: sessionDetails.checkpoint_count })}
              </span>
            </span>
          )}
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {loading.details ? (
          // Loading state
          <div className="space-y-3">
            {[...Array(5)].map((_, i) => (
              <div key={i} className="animate-pulse">
                <div className="h-4 w-24 bg-gray-200 dark:bg-gray-700 rounded mb-2" />
                <div className="h-10 bg-gray-100 dark:bg-gray-800 rounded" />
              </div>
            ))}
          </div>
        ) : sessionDetails ? (
          // Message list
          sessionDetails.messages.map((message, index) => (
            <div
              key={index}
              className={clsx(
                'p-3 rounded-lg',
                message.message_type === 'user'
                  ? 'bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800'
                  : message.message_type === 'assistant'
                    ? 'bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700'
                    : 'bg-yellow-50 dark:bg-yellow-900/20 border border-yellow-200 dark:border-yellow-800',
              )}
            >
              {/* Message Type Label */}
              <div className="flex items-center gap-1.5 mb-2">
                {getMessageIcon(message.message_type)}
                <span className="text-xs font-medium text-gray-600 dark:text-gray-400">
                  {getMessageLabel(message.message_type)}
                </span>
                {message.timestamp && (
                  <span className="text-xs text-gray-400 dark:text-gray-500 ml-auto">
                    {formatRelativeTime(message.timestamp)}
                  </span>
                )}
              </div>

              {/* Message Content */}
              <p className="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap">{message.content_preview}</p>
            </div>
          ))
        ) : (
          // No details
          <div className="text-center py-8">
            <p className="text-sm text-gray-500 dark:text-gray-400">{t('projects.noDetails')}</p>
          </div>
        )}
      </div>
    </div>
  );
}

export default SessionDetails;
