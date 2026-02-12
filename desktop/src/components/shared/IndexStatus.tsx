/**
 * IndexStatus Component
 *
 * Displays the current codebase indexing status with real-time progress updates.
 * Listens to Tauri "index-progress" events and provides a re-index button.
 */

import { useEffect, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../../store/settings';

interface IndexStatusEvent {
  project_path: string;
  status: 'idle' | 'indexing' | 'indexed' | 'error';
  indexed_files: number;
  total_files: number;
  error_message?: string | null;
}

interface CommandResponse<T> {
  success: boolean;
  data: T | null;
  error: string | null;
}

interface IndexStatusProps {
  /** Compact mode for embedding in headers */
  compact?: boolean;
  className?: string;
}

export function IndexStatus({ compact = false, className }: IndexStatusProps) {
  const { t } = useTranslation('common');
  const workspacePath = useSettingsStore((s) => s.workspacePath);

  const [status, setStatus] = useState<IndexStatusEvent['status']>('idle');
  const [indexedFiles, setIndexedFiles] = useState(0);
  const [totalFiles, setTotalFiles] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const applyEvent = useCallback((evt: IndexStatusEvent) => {
    setStatus(evt.status);
    setIndexedFiles(evt.indexed_files);
    setTotalFiles(evt.total_files);
    setErrorMessage(evt.error_message ?? null);
  }, []);

  // Fetch initial status and listen for real-time updates
  useEffect(() => {
    if (!workspacePath) {
      setStatus('idle');
      setIndexedFiles(0);
      setTotalFiles(0);
      setErrorMessage(null);
      return;
    }

    let cancelled = false;

    // Fetch initial status
    invoke<CommandResponse<IndexStatusEvent>>('get_index_status', {
      projectPath: workspacePath,
    })
      .then((response) => {
        if (!cancelled && response.success && response.data) {
          applyEvent(response.data);
        }
      })
      .catch(() => {
        // Silently ignore - backend may not be ready yet
      });

    // Listen for real-time progress events
    let unlisten: UnlistenFn | null = null;
    listen<IndexStatusEvent>('index-progress', (event) => {
      if (!cancelled) {
        applyEvent(event.payload);
      }
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch(() => {
        // Silently ignore listener setup failure
      });

    return () => {
      cancelled = true;
      if (unlisten) {
        unlisten();
      }
    };
  }, [workspacePath, applyEvent]);

  const handleReindex = useCallback(() => {
    invoke<CommandResponse<boolean>>('trigger_reindex', {
      projectPath: workspacePath || undefined,
    }).catch(() => {
      // Silently ignore - will show error via event if it fails
    });
  }, [workspacePath]);

  // idle state: render nothing
  if (status === 'idle') {
    return null;
  }

  // indexing state: animated spinner + progress
  if (status === 'indexing') {
    return (
      <div className={clsx('flex items-center gap-1.5', className)}>
        <svg
          className={clsx(
            'animate-spin text-blue-500 dark:text-blue-400',
            compact ? 'w-3 h-3' : 'w-4 h-4'
          )}
          fill="none"
          viewBox="0 0 24 24"
        >
          <circle
            className="opacity-25"
            cx="12"
            cy="12"
            r="10"
            stroke="currentColor"
            strokeWidth="4"
          />
          <path
            className="opacity-75"
            fill="currentColor"
            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
          />
        </svg>
        <span
          className={clsx(
            'text-blue-600 dark:text-blue-400',
            compact ? 'text-xs' : 'text-sm'
          )}
        >
          {compact
            ? `${indexedFiles}/${totalFiles}`
            : t('indexing.indexingProgress', {
                current: indexedFiles,
                total: totalFiles,
              })}
        </span>
      </div>
    );
  }

  // indexed state: green dot + file count + re-index button
  if (status === 'indexed') {
    return (
      <div className={clsx('flex items-center gap-1.5', className)}>
        <div className="w-2 h-2 rounded-full bg-green-500" />
        <span
          className={clsx(
            'text-green-600 dark:text-green-400',
            compact ? 'text-xs' : 'text-sm'
          )}
        >
          {t('indexing.readyFiles', { count: indexedFiles })}
        </span>
        <button
          onClick={handleReindex}
          title={t('indexing.reindexTooltip')}
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
              d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
            />
          </svg>
        </button>
      </div>
    );
  }

  // error state: red dot + error text + retry button
  return (
    <div className={clsx('flex items-center gap-1.5', className)}>
      <div className="w-2 h-2 rounded-full bg-red-500" />
      <span
        className={clsx(
          'text-red-600 dark:text-red-400',
          compact ? 'text-xs' : 'text-sm'
        )}
        title={errorMessage || undefined}
      >
        {t('indexing.error')}
      </span>
      <button
        onClick={handleReindex}
        title={t('indexing.reindexTooltip')}
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
            d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
          />
        </svg>
      </button>
    </div>
  );
}

export default IndexStatus;
