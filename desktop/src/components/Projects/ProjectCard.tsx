/**
 * ProjectCard Component
 *
 * Displays a single project with name, path, session count, and last activity.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { FileIcon, ClockIcon, ChatBubbleIcon } from '@radix-ui/react-icons';
import type { Project } from '../../types/project';
import { formatRelativeTime, truncatePath } from './utils';

interface ProjectCardProps {
  project: Project;
  isSelected: boolean;
  onClick: () => void;
}

export function ProjectCard({ project, isSelected, onClick }: ProjectCardProps) {
  const { t } = useTranslation();

  return (
    <button
      onClick={onClick}
      className={clsx(
        'w-full text-left p-4 rounded-lg transition-colors',
        'border',
        isSelected
          ? 'bg-primary-50 dark:bg-primary-900/30 border-primary-300 dark:border-primary-700'
          : 'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
      )}
    >
      {/* Project Name */}
      <h3 className={clsx(
        'font-semibold text-sm mb-1',
        isSelected
          ? 'text-primary-700 dark:text-primary-300'
          : 'text-gray-900 dark:text-white'
      )}>
        {project.name}
      </h3>

      {/* Project Path */}
      <p className="text-xs text-gray-500 dark:text-gray-400 mb-3 truncate" title={project.path}>
        {truncatePath(project.path, 40)}
      </p>

      {/* Stats Row */}
      <div className="flex items-center gap-4 text-xs text-gray-500 dark:text-gray-400">
        {/* Session Count */}
        <span className="flex items-center gap-1">
          <FileIcon className="w-3.5 h-3.5" />
          <span>{project.session_count} {t('projects.sessions', { count: project.session_count })}</span>
        </span>

        {/* Message Count */}
        <span className="flex items-center gap-1">
          <ChatBubbleIcon className="w-3.5 h-3.5" />
          <span>{project.message_count}</span>
        </span>

        {/* Last Activity */}
        <span className="flex items-center gap-1 ml-auto">
          <ClockIcon className="w-3.5 h-3.5" />
          <span>{formatRelativeTime(project.last_activity)}</span>
        </span>
      </div>
    </button>
  );
}

export default ProjectCard;
