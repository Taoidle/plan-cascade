/**
 * CodebasePanel Component
 *
 * Two-panel layout for codebase index management:
 * - Left: Indexed project list with status, file counts, and actions
 * - Right: Project detail with Overview / Files / Search tabs
 */

import { useState, useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { useCodebaseStore } from '../../store/codebase';
import type { IndexStatusEvent } from '../../lib/codebaseApi';
import { CodebaseDetail } from './CodebaseDetail';

// ---------------------------------------------------------------------------
// DeleteConfirmDialog
// ---------------------------------------------------------------------------

function DeleteConfirmDialog({
  projectPath,
  onConfirm,
  onClose,
}: {
  projectPath: string;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation('codebase');

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div
        className={clsx(
          'w-full max-w-md rounded-xl p-6',
          'bg-white dark:bg-gray-800',
          'border border-gray-200 dark:border-gray-700',
          'shadow-xl',
        )}
      >
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-3">{t('deleteIndex')}</h3>
        <p className="text-sm text-gray-600 dark:text-gray-400 mb-6">{t('deleteConfirm', { path: projectPath })}</p>
        <div className="flex justify-end gap-3">
          <button
            onClick={onClose}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-700',
              'transition-colors',
            )}
          >
            {t('cancel', { ns: 'common' })}
          </button>
          <button
            onClick={onConfirm}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'bg-red-600 hover:bg-red-700',
              'text-white',
              'transition-colors',
            )}
          >
            {t('deleteIndex')}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// StatusDot
// ---------------------------------------------------------------------------

function StatusDot({ status }: { status: string }) {
  const color =
    status === 'indexing'
      ? 'bg-blue-500 animate-pulse'
      : status === 'indexed'
        ? 'bg-green-500'
        : status === 'error'
          ? 'bg-red-500'
          : 'bg-gray-400';

  return <span className={clsx('inline-block w-2 h-2 rounded-full', color)} />;
}

// ---------------------------------------------------------------------------
// CodebasePanel
// ---------------------------------------------------------------------------

export function CodebasePanel() {
  const { t } = useTranslation('codebase');
  const {
    projects,
    selectedProjectPath,
    loading,
    error,
    loadProjects,
    selectProject,
    deleteProject,
    reindexProject,
    clearError,
  } = useCodebaseStore();

  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  // Track live statuses from index-progress events
  const [liveStatuses, setLiveStatuses] = useState<Record<string, IndexStatusEvent>>({});

  // Load projects on mount
  useEffect(() => {
    loadProjects();
  }, [loadProjects]);

  // Listen to index-progress events for real-time status updates
  useEffect(() => {
    const unlisten = listen<IndexStatusEvent>('index-progress', (event) => {
      setLiveStatuses((prev) => ({
        ...prev,
        [event.payload.project_path]: event.payload,
      }));
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleDelete = useCallback(() => {
    if (!deleteTarget) return;
    deleteProject(deleteTarget);
    setDeleteTarget(null);
  }, [deleteTarget, deleteProject]);

  const getStatusForProject = (projectPath: string): string => {
    return liveStatuses[projectPath]?.status ?? 'idle';
  };

  const formatPath = (path: string): string => {
    const parts = path.split('/');
    return parts.length > 2 ? `.../${parts.slice(-2).join('/')}` : path;
  };

  return (
    <div className="h-full flex flex-col">
      {/* Error banner */}
      {error && (
        <div
          className={clsx(
            'px-4 py-2 flex items-center justify-between',
            'bg-red-50 dark:bg-red-900/20',
            'border-b border-red-200 dark:border-red-800',
          )}
        >
          <span className="text-sm text-red-700 dark:text-red-300">{error}</span>
          <button onClick={clearError} className="text-sm text-red-600 hover:text-red-800 dark:text-red-400">
            &times;
          </button>
        </div>
      )}

      <div className="flex-1 flex overflow-hidden">
        {/* Left Panel - Project List */}
        <div
          className={clsx(
            'h-full border-r border-gray-200 dark:border-gray-700',
            'bg-gray-50 dark:bg-gray-900',
            selectedProjectPath ? 'hidden md:block md:w-72 lg:w-80' : 'w-full md:w-72 lg:w-80',
          )}
        >
          <div className="h-full flex flex-col">
            {/* Header */}
            <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700">
              <h2 className="text-sm font-semibold text-gray-900 dark:text-white">{t('title')}</h2>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-1">
                {t('projectCount', { count: projects.length })}
              </p>
            </div>

            {/* Project list */}
            <div className="flex-1 overflow-y-auto">
              {loading ? (
                <div className="px-4 py-8 text-center">
                  <div className="animate-pulse text-sm text-gray-500">Loading...</div>
                </div>
              ) : projects.length === 0 ? (
                <div className="px-4 py-8 text-center">
                  <p className="text-sm text-gray-500 dark:text-gray-400">{t('noProjects')}</p>
                  <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">{t('noProjectsHint')}</p>
                </div>
              ) : (
                <div className="divide-y divide-gray-200 dark:divide-gray-700">
                  {projects.map((project) => {
                    const status = getStatusForProject(project.project_path);
                    const isSelected = selectedProjectPath === project.project_path;

                    return (
                      <div
                        key={project.project_path}
                        role="button"
                        tabIndex={0}
                        onClick={() => selectProject(project.project_path)}
                        onKeyDown={(e) => {
                          if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault();
                            selectProject(project.project_path);
                          }
                        }}
                        className={clsx(
                          'w-full text-left px-4 py-3 cursor-pointer',
                          'hover:bg-gray-100 dark:hover:bg-gray-800',
                          'transition-colors',
                          isSelected && 'bg-primary-50 dark:bg-primary-900/20 border-l-2 border-primary-500',
                        )}
                      >
                        <div className="flex items-center justify-between mb-1">
                          <span
                            className="text-sm font-medium text-gray-900 dark:text-white truncate flex-1 mr-2"
                            title={project.project_path}
                          >
                            {formatPath(project.project_path)}
                          </span>
                          <div className="flex items-center gap-2 shrink-0">
                            <StatusDot status={status} />
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                reindexProject(project.project_path);
                              }}
                              className="text-gray-400 hover:text-primary-500 dark:hover:text-primary-400 p-1"
                              title={t('reindex')}
                            >
                              <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                  strokeWidth={2}
                                  d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                                />
                              </svg>
                            </button>
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                setDeleteTarget(project.project_path);
                              }}
                              className="text-gray-400 hover:text-red-500 dark:hover:text-red-400 p-1"
                              title={t('deleteIndex')}
                            >
                              <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path
                                  strokeLinecap="round"
                                  strokeLinejoin="round"
                                  strokeWidth={2}
                                  d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                                />
                              </svg>
                            </button>
                          </div>
                        </div>
                        <div className="flex items-center gap-3 text-xs text-gray-500 dark:text-gray-400">
                          <span>{t('fileCount', { count: project.file_count })}</span>
                          {project.last_indexed_at && (
                            <span>{new Date(project.last_indexed_at).toLocaleDateString()}</span>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Right Panel - Detail */}
        <div className="flex-1 h-full overflow-hidden">
          {selectedProjectPath ? (
            <CodebaseDetail
              projectPath={selectedProjectPath}
              liveStatus={liveStatuses[selectedProjectPath]}
              onBack={() => selectProject(null)}
            />
          ) : (
            <div className="h-full flex items-center justify-center">
              <p className="text-sm text-gray-500 dark:text-gray-400">{t('noDetail')}</p>
            </div>
          )}
        </div>
      </div>

      {/* Delete confirmation dialog */}
      {deleteTarget && (
        <DeleteConfirmDialog
          projectPath={deleteTarget}
          onConfirm={handleDelete}
          onClose={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}
