/**
 * ProjectSelector Component
 *
 * Compact button showing current working directory basename.
 * Click opens native folder picker to change the workspace path.
 * Includes an "Open in File Manager" action for the active workspace.
 */

import { useCallback, type MouseEvent } from 'react';
import { clsx } from 'clsx';
import { Command } from '@tauri-apps/plugin-shell';
import { useSettingsStore } from '../../store/settings';
import { useTranslation } from 'react-i18next';

interface ProjectSelectorProps {
  /** Compact mode for embedding in headers */
  compact?: boolean;
  className?: string;
}

export function ProjectSelector({ compact = false, className }: ProjectSelectorProps) {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const setWorkspacePath = useSettingsStore((s) => s.setWorkspacePath);

  const basename = workspacePath
    ? workspacePath.split(/[/\\]/).filter(Boolean).pop() || workspacePath
    : 'No directory';

  const handleClick = useCallback(async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select working directory',
        defaultPath: workspacePath || undefined,
      });
      if (selected && typeof selected === 'string') {
        setWorkspacePath(selected);
      }
    } catch (err) {
      console.error('Failed to open directory picker:', err);
    }
  }, [workspacePath, setWorkspacePath]);

  const handleOpenFileManager = useCallback(
    (e: MouseEvent<HTMLButtonElement>) => {
      e.stopPropagation();
      if (!workspacePath) return;
      const platform = navigator.platform.toLowerCase();
      let cmdName: string;
      if (platform.includes('mac')) {
        cmdName = 'open-path-macos';
      } else if (platform.includes('win')) {
        cmdName = 'open-path-windows';
      } else {
        cmdName = 'open-path-linux';
      }
      Command.create(cmdName, [workspacePath])
        .execute()
        .catch((err) => {
          console.error('Failed to open file manager:', err);
        });
    },
    [workspacePath]
  );

  const handleClear = useCallback(
    (e: MouseEvent<HTMLButtonElement>) => {
      e.stopPropagation();
      setWorkspacePath('');
    },
    [setWorkspacePath]
  );

  return (
    <div className="flex items-center gap-0.5">
      <button
        onClick={handleClick}
        title={workspacePath || 'Select working directory'}
        className={clsx(
          'flex items-center gap-1.5 rounded-lg transition-colors',
          'text-gray-600 dark:text-gray-400',
          'hover:bg-gray-100 dark:hover:bg-gray-800',
          compact ? 'px-2 py-1 text-xs' : 'px-3 py-1.5 text-sm',
          className
        )}
      >
        <svg
          className={clsx(compact ? 'w-3 h-3' : 'w-4 h-4')}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
          />
        </svg>
        <span className="max-w-[120px] truncate">{basename}</span>
      </button>
      {workspacePath && (
        <button
          onClick={handleOpenFileManager}
          title={t('projectSelector.openInFileManager', 'Open in file manager')}
          className={clsx(
            'flex items-center justify-center rounded-md transition-colors',
            'text-gray-400 dark:text-gray-500',
            'hover:text-gray-600 dark:hover:text-gray-300',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
            compact ? 'w-5 h-5' : 'w-6 h-6'
          )}
        >
          <svg
            className={clsx(compact ? 'w-3 h-3' : 'w-3.5 h-3.5')}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"
            />
          </svg>
        </button>
      )}
      {workspacePath && (
        <button
          onClick={handleClear}
          title="Clear directory"
          className={clsx(
            'flex items-center justify-center rounded-md transition-colors',
            'text-gray-400 dark:text-gray-500',
            'hover:text-gray-600 dark:hover:text-gray-300',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
            compact ? 'w-5 h-5' : 'w-6 h-6'
          )}
        >
          <svg
            className={clsx(compact ? 'w-3 h-3' : 'w-3.5 h-3.5')}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      )}
    </div>
  );
}

export default ProjectSelector;
