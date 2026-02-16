/**
 * SkillDetail Component
 *
 * Full detail view for a skill within the management dialog.
 * Shows name, description, source, tags, body (markdown), and metadata.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { Cross2Icon } from '@radix-ui/react-icons';
import { SkillSourceBadge } from './SkillSourceBadge';
import type { SkillDocument } from '../../types/skillMemory';

interface SkillDetailProps {
  skill: SkillDocument;
  onClose: () => void;
  className?: string;
}

export function SkillDetail({ skill, onClose, className }: SkillDetailProps) {
  const { t } = useTranslation('simpleMode');

  return (
    <div
      data-testid="skill-detail"
      className={clsx('flex flex-col h-full', className)}
    >
      {/* Header */}
      <div className="flex items-start justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 mb-1">
            <h3 className="text-sm font-semibold text-gray-900 dark:text-white truncate">
              {skill.name}
            </h3>
            <SkillSourceBadge source={skill.source} />
          </div>
          <p className="text-xs text-gray-500 dark:text-gray-400">{skill.description}</p>
        </div>
        <button
          onClick={onClose}
          className={clsx(
            'p-1 rounded-md shrink-0 ml-2',
            'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
            'hover:bg-gray-100 dark:hover:bg-gray-800'
          )}
          title={t('skillPanel.close')}
        >
          <Cross2Icon className="w-4 h-4" />
        </button>
      </div>

      {/* Metadata */}
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 space-y-2">
        {/* Tags */}
        {skill.tags.length > 0 && (
          <div className="flex items-center gap-1 flex-wrap">
            <span className="text-2xs text-gray-500 dark:text-gray-400 shrink-0">
              {t('skillPanel.tags')}:
            </span>
            {skill.tags.map((tag) => (
              <span
                key={tag}
                className="text-2xs px-1.5 py-0.5 rounded-full bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300"
              >
                {tag}
              </span>
            ))}
          </div>
        )}

        {/* Version */}
        {skill.version && (
          <div className="flex items-center gap-1">
            <span className="text-2xs text-gray-500 dark:text-gray-400">
              {t('skillPanel.version')}:
            </span>
            <span className="text-2xs text-gray-700 dark:text-gray-300">{skill.version}</span>
          </div>
        )}

        {/* Priority */}
        <div className="flex items-center gap-1">
          <span className="text-2xs text-gray-500 dark:text-gray-400">
            {t('skillPanel.priority')}:
          </span>
          <span className="text-2xs text-gray-700 dark:text-gray-300">{skill.priority}</span>
        </div>

        {/* Injection phases */}
        <div className="flex items-center gap-1 flex-wrap">
          <span className="text-2xs text-gray-500 dark:text-gray-400 shrink-0">
            {t('skillPanel.phases')}:
          </span>
          {skill.inject_into.map((phase) => (
            <span
              key={phase}
              className="text-2xs px-1.5 py-0.5 rounded bg-blue-50 dark:bg-blue-900/20 text-blue-600 dark:text-blue-400"
            >
              {phase}
            </span>
          ))}
        </div>
      </div>

      {/* Body */}
      <div className="flex-1 overflow-y-auto p-4">
        <pre className="text-xs text-gray-700 dark:text-gray-300 whitespace-pre-wrap font-mono leading-relaxed">
          {skill.body}
        </pre>
      </div>
    </div>
  );
}

export default SkillDetail;
