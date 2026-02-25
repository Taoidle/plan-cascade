/**
 * ArtifactBrowserPanel Component
 *
 * Main container for browsing and managing artifacts with two-panel layout:
 * - Left: Artifact list with scope filter and search
 * - Right: Artifact detail, version history, and actions
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useArtifactsStore, type ScopeFilter } from '../../store/artifacts';
import { useProjectsStore } from '../../store/projects';
import { ArtifactDetail } from './ArtifactDetail';
import type { ArtifactMeta } from '../../lib/artifactsApi';

// ---------------------------------------------------------------------------
// ArtifactBrowserPanel
// ---------------------------------------------------------------------------

export function ArtifactBrowserPanel() {
  const { t } = useTranslation('artifacts');
  const {
    artifacts,
    selectedArtifact,
    scopeFilter,
    searchText,
    isLoading,
    isDeleting,
    error,
    fetchArtifacts,
    selectArtifact,
    deleteArtifact,
    setScopeFilter,
    setSearchText,
    clearError,
  } = useArtifactsStore();

  const { selectedProject } = useProjectsStore();
  const projectId = selectedProject?.id ?? 'default';

  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  // Fetch artifacts on mount and project change
  useEffect(() => {
    fetchArtifacts(projectId);
  }, [projectId, fetchArtifacts]);

  // Filter artifacts by search text
  const filteredArtifacts = useMemo(() => {
    if (!searchText.trim()) return artifacts;
    const lower = searchText.toLowerCase();
    return artifacts.filter((a) => a.name.toLowerCase().includes(lower));
  }, [artifacts, searchText]);

  const handleDelete = useCallback(async () => {
    if (!deleteTarget) return;
    const ok = await deleteArtifact(deleteTarget, projectId);
    if (ok) {
      setDeleteTarget(null);
    }
  }, [deleteTarget, projectId, deleteArtifact]);

  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const formatDate = (dateStr: string): string => {
    try {
      return new Date(dateStr).toLocaleDateString();
    } catch {
      return dateStr;
    }
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
            {t('dismiss')}
          </button>
        </div>
      )}

      <div className="flex-1 flex overflow-hidden">
        {/* Left Panel - Artifact List */}
        <div
          className={clsx(
            'h-full border-r border-gray-200 dark:border-gray-700',
            'bg-gray-50 dark:bg-gray-900',
            selectedArtifact ? 'hidden md:block md:w-1/3 lg:w-1/4' : 'w-full md:w-1/3 lg:w-1/4',
          )}
        >
          <div className="h-full flex flex-col">
            {/* Header */}
            <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 space-y-3">
              <div className="flex items-center justify-between">
                <h2 className="text-sm font-semibold text-gray-900 dark:text-white">{t('title')}</h2>
                <span className="text-xs text-gray-500 dark:text-gray-400">
                  {t('count', { count: filteredArtifacts.length })}
                </span>
              </div>

              {/* Scope filter */}
              <div className="flex gap-1">
                {(['project', 'session', 'user'] as ScopeFilter[]).map((scope) => (
                  <button
                    key={scope}
                    onClick={() => setScopeFilter(scope)}
                    className={clsx(
                      'px-3 py-1 rounded-md text-xs font-medium transition-colors',
                      scopeFilter === scope
                        ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                        : 'text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-800',
                    )}
                  >
                    {t(`scope.${scope}`)}
                  </button>
                ))}
              </div>

              {/* Search */}
              <input
                type="text"
                value={searchText}
                onChange={(e) => setSearchText(e.target.value)}
                placeholder={t('searchPlaceholder')}
                className={clsx(
                  'w-full px-3 py-1.5 rounded-lg text-sm',
                  'border border-gray-300 dark:border-gray-600',
                  'bg-white dark:bg-gray-800',
                  'text-gray-900 dark:text-white',
                  'placeholder:text-gray-400',
                )}
              />
            </div>

            {/* Artifact list */}
            <div className="flex-1 overflow-y-auto">
              {isLoading ? (
                <div className="px-4 py-8 text-center">
                  <div className="animate-pulse text-sm text-gray-500">{t('loading')}</div>
                </div>
              ) : filteredArtifacts.length === 0 ? (
                <div className="px-4 py-8 text-center">
                  <p className="text-sm text-gray-500 dark:text-gray-400">{t('noArtifacts')}</p>
                  <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">{t('noArtifactsHint')}</p>
                </div>
              ) : (
                <div className="divide-y divide-gray-200 dark:divide-gray-700">
                  {filteredArtifacts.map((artifact) => (
                    <button
                      key={artifact.id}
                      onClick={() => selectArtifact(artifact)}
                      className={clsx(
                        'w-full text-left px-4 py-3',
                        'hover:bg-gray-100 dark:hover:bg-gray-800',
                        'transition-colors',
                        selectedArtifact?.id === artifact.id &&
                          'bg-primary-50 dark:bg-primary-900/20 border-l-2 border-primary-500',
                      )}
                    >
                      <div className="flex items-center justify-between">
                        <span className="text-sm font-medium text-gray-900 dark:text-white truncate">
                          {artifact.name}
                        </span>
                        <button
                          onClick={(e) => {
                            e.stopPropagation();
                            setDeleteTarget(artifact.name);
                          }}
                          className="text-gray-400 hover:text-red-500 dark:hover:text-red-400 p-1"
                          title={t('delete')}
                        >
                          <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path
                              strokeLinecap="round"
                              strokeLinejoin="round"
                              strokeWidth={2}
                              d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                            />
                          </svg>
                        </button>
                      </div>
                      <div className="flex items-center gap-3 mt-1 text-xs text-gray-500 dark:text-gray-400">
                        <span>{artifact.content_type}</span>
                        <span>v{artifact.current_version}</span>
                        <span>{formatSize(artifact.size_bytes)}</span>
                        <span>{formatDate(artifact.created_at)}</span>
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Right Panel - Artifact Detail */}
        <div
          className={clsx(
            'h-full flex-1',
            'bg-white dark:bg-gray-950',
            selectedArtifact ? 'w-full md:w-2/3 lg:w-3/4' : 'hidden md:flex md:items-center md:justify-center',
          )}
        >
          {selectedArtifact ? (
            <ArtifactDetail artifact={selectedArtifact} projectId={projectId} onBack={() => selectArtifact(null)} />
          ) : (
            <div className="text-center px-4">
              <svg
                className="mx-auto w-12 h-12 text-gray-300 dark:text-gray-600 mb-3"
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={1.5}
                  d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4"
                />
              </svg>
              <p className="text-sm text-gray-500 dark:text-gray-400">{t('selectArtifact')}</p>
            </div>
          )}
        </div>
      </div>

      {/* Delete Confirmation */}
      {deleteTarget && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div
            className={clsx(
              'w-full max-w-sm rounded-xl p-6',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'shadow-xl',
            )}
          >
            <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-2">{t('deleteArtifact')}</h3>
            <p className="text-sm text-gray-600 dark:text-gray-400 mb-6">
              {t('deleteConfirm', { name: deleteTarget })}
            </p>
            <div className="flex justify-end gap-3">
              <button
                onClick={() => setDeleteTarget(null)}
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
                onClick={handleDelete}
                disabled={isDeleting}
                className={clsx(
                  'px-4 py-2 rounded-lg text-sm font-medium',
                  'bg-red-600 hover:bg-red-700',
                  'text-white',
                  'disabled:opacity-50 disabled:cursor-not-allowed',
                  'transition-colors',
                )}
              >
                {isDeleting ? t('deleting') : t('delete')}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
