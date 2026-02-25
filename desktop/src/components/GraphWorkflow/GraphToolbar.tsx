/**
 * GraphToolbar Component
 *
 * Toolbar with buttons for adding nodes, edges, and deleting selections.
 */

import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';

interface GraphToolbarProps {
  onAddNode: (type: string) => void;
  onAddEdge: (type: 'direct' | 'conditional') => void;
  onDeleteSelected: () => void;
  hasSelection: boolean;
}

export function GraphToolbar({ onAddNode, onAddEdge, onDeleteSelected, hasSelection }: GraphToolbarProps) {
  const { t } = useTranslation('expertMode');
  return (
    <div
      className={clsx(
        'flex items-center gap-2 px-6 py-2',
        'border-b border-gray-200 dark:border-gray-700',
        'bg-white dark:bg-gray-900',
      )}
    >
      {/* Add Node buttons */}
      <span className="text-xs text-gray-500 dark:text-gray-400 mr-1">{t('graphWorkflow.toolbar.nodes')}</span>
      <ToolbarButton label={t('graphWorkflow.toolbar.addLlm')} color="blue" onClick={() => onAddNode('llm')} />
      <ToolbarButton
        label={t('graphWorkflow.toolbar.addSequential')}
        color="green"
        onClick={() => onAddNode('sequential')}
      />
      <ToolbarButton
        label={t('graphWorkflow.toolbar.addParallel')}
        color="purple"
        onClick={() => onAddNode('parallel')}
      />

      <div className="w-px h-5 bg-gray-300 dark:bg-gray-600 mx-1" />

      {/* Add Edge buttons */}
      <span className="text-xs text-gray-500 dark:text-gray-400 mr-1">{t('graphWorkflow.toolbar.edges')}</span>
      <ToolbarButton label={t('graphWorkflow.toolbar.direct')} color="gray" onClick={() => onAddEdge('direct')} />
      <ToolbarButton
        label={t('graphWorkflow.toolbar.conditional')}
        color="amber"
        onClick={() => onAddEdge('conditional')}
      />

      <div className="w-px h-5 bg-gray-300 dark:bg-gray-600 mx-1" />

      {/* Delete */}
      <button
        onClick={onDeleteSelected}
        disabled={!hasSelection}
        className={clsx(
          'px-3 py-1 text-xs font-medium rounded transition-colors',
          'border border-red-300 dark:border-red-700',
          'text-red-600 dark:text-red-400',
          'hover:bg-red-50 dark:hover:bg-red-900/20',
          'disabled:opacity-30 disabled:cursor-not-allowed',
        )}
      >
        {t('graphWorkflow.toolbar.delete')}
      </button>
    </div>
  );
}

interface ToolbarButtonProps {
  label: string;
  color: string;
  onClick: () => void;
}

function ToolbarButton({ label, color, onClick }: ToolbarButtonProps) {
  const colorClasses: Record<string, string> = {
    blue: 'border-blue-300 dark:border-blue-700 text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/20',
    green:
      'border-green-300 dark:border-green-700 text-green-600 dark:text-green-400 hover:bg-green-50 dark:hover:bg-green-900/20',
    purple:
      'border-purple-300 dark:border-purple-700 text-purple-600 dark:text-purple-400 hover:bg-purple-50 dark:hover:bg-purple-900/20',
    amber:
      'border-amber-300 dark:border-amber-700 text-amber-600 dark:text-amber-400 hover:bg-amber-50 dark:hover:bg-amber-900/20',
    gray: 'border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800',
  };

  return (
    <button
      onClick={onClick}
      className={clsx(
        'px-2 py-1 text-xs font-medium rounded border transition-colors',
        colorClasses[color] ?? colorClasses.gray,
      )}
    >
      {label}
    </button>
  );
}
