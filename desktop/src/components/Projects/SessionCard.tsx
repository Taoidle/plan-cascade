/**
 * SessionCard Component
 *
 * Displays a single session with preview, timestamp, and resume button.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ClockIcon, ChatBubbleIcon, PlayIcon } from '@radix-ui/react-icons';
import type { Session } from '../../types/project';
import { formatRelativeTime, truncateText } from './utils';

interface SessionCardProps {
  session: Session;
  isSelected: boolean;
  onClick: () => void;
  onResume: () => void;
}

export function SessionCard({ session, isSelected, onClick, onResume }: SessionCardProps) {
  const { t } = useTranslation();

  const handleResume = (e: React.MouseEvent) => {
    e.stopPropagation();
    onResume();
  };

  return (
    <div
      onClick={onClick}
      className={clsx(
        'w-full text-left p-4 rounded-lg transition-colors cursor-pointer',
        'border',
        isSelected
          ? 'bg-primary-50 dark:bg-primary-900/30 border-primary-300 dark:border-primary-700'
          : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600',
      )}
    >
      {/* First Message Preview */}
      <p
        className={clsx(
          'text-sm mb-2 line-clamp-2',
          isSelected ? 'text-primary-700 dark:text-primary-300' : 'text-gray-900 dark:text-white',
        )}
      >
        {session.first_message_preview ? truncateText(session.first_message_preview, 100) : t('projects.noPreview')}
      </p>

      {/* Stats Row */}
      <div className="flex items-center gap-4 text-xs text-gray-500 dark:text-gray-400">
        {/* Message Count */}
        <span className="flex items-center gap-1">
          <ChatBubbleIcon className="w-3.5 h-3.5" />
          <span>
            {session.message_count} {t('projects.messages', { count: session.message_count })}
          </span>
        </span>

        {/* Timestamp */}
        <span className="flex items-center gap-1">
          <ClockIcon className="w-3.5 h-3.5" />
          <span>{formatRelativeTime(session.created_at)}</span>
        </span>

        {/* Resume Button */}
        <button
          onClick={handleResume}
          className={clsx(
            'ml-auto flex items-center gap-1 px-2.5 py-1 rounded-md',
            'bg-primary-100 dark:bg-primary-900/50',
            'text-primary-700 dark:text-primary-300',
            'hover:bg-primary-200 dark:hover:bg-primary-800',
            'transition-colors text-xs font-medium',
          )}
        >
          <PlayIcon className="w-3 h-3" />
          <span>{t('projects.resume')}</span>
        </button>
      </div>
    </div>
  );
}

export default SessionCard;
