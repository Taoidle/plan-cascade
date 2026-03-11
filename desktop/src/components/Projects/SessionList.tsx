/**
 * SessionList Component
 *
 * Displays session history for a selected project.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ArrowLeftIcon, MagnifyingGlassIcon } from '@radix-ui/react-icons';
import { useState, useCallback, useMemo } from 'react';
import { useProjectsStore } from '../../store/projects';
import type { Session } from '../../store';
import { buildDebugStateChips, summarizeDebugCase } from '../../lib/debugLabels';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import { useModeStore } from '../../store/mode';
import { SessionCard } from './SessionCard';
import { SessionSkeleton } from './SessionSkeleton';
import { SessionDetails } from './SessionDetails';
import { debounce } from './utils';

function normalizeWorkspacePath(path: string | null | undefined): string | null {
  const value = (path || '').trim();
  if (!value) return null;
  return value.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
}

export function SessionList() {
  const { t } = useTranslation();
  const [sessionSearch, setSessionSearch] = useState('');
  const store = useProjectsStore();
  const {
    selectedProject,
    selectedSession,
    loading,
    error,
    selectProject,
    selectSession,
    resumeSession,
    searchSessions,
    fetchSessions,
  } = store;
  const workflowSessionCatalog = useWorkflowKernelStore((s) => s.sessionCatalog);
  const activateWorkflowSession = useWorkflowKernelStore((s) => s.activateSession);
  const setMode = useModeStore((s) => s.setMode);
  // Explicitly type sessions to help TypeScript
  const sessions: Session[] = store.sessions;
  const projectDebugSessions = useMemo(() => {
    const projectPath = normalizeWorkspacePath(selectedProject?.path);
    if (!projectPath) return [];
    return workflowSessionCatalog.filter((session) => {
      if (session.activeMode !== 'debug') return false;
      const workspacePath = normalizeWorkspacePath(session.workspacePath);
      if (!workspacePath) return false;
      return workspacePath === projectPath || workspacePath.startsWith(`${projectPath}/`);
    });
  }, [selectedProject?.path, workflowSessionCatalog]);

  // Debounced search
  const debouncedSearch = useMemo(
    () =>
      debounce((query: string) => {
        if (selectedProject) {
          if (query.trim()) {
            searchSessions(selectedProject.path, query);
          } else {
            fetchSessions(selectedProject.path);
          }
        }
      }, 300),
    [selectedProject, searchSessions, fetchSessions],
  );

  const handleSearch = (query: string) => {
    setSessionSearch(query);
    debouncedSearch(query);
  };

  const handleResume = async (sessionPath: string) => {
    const result = await resumeSession(sessionPath);
    if (result?.success) {
      // Copy command to clipboard or show it
      navigator.clipboard.writeText(result.resume_command);
      // TODO: Show toast notification
    }
  };

  const handleOpenDebugCase = useCallback(
    async (sessionId: string) => {
      const activated = await activateWorkflowSession(sessionId);
      if (!activated) return;
      setMode('simple');
    },
    [activateWorkflowSession, setMode],
  );

  // Render sessions list
  const renderSessions = () => {
    if (loading.sessions || error || sessions.length === 0) return null;
    return sessions.map((s) => {
      const sessionItem = s as Session;
      return (
        <SessionCard
          key={sessionItem.id}
          session={sessionItem}
          isSelected={selectedSession !== null && selectedSession.id === sessionItem.id}
          onClick={() => selectSession(sessionItem)}
          onResume={() => handleResume(sessionItem.file_path)}
        />
      );
    });
  };

  if (!selectedProject) {
    return (
      <div className="h-full flex items-center justify-center">
        <p className="text-sm text-gray-500 dark:text-gray-400">{t('projects.selectProject')}</p>
      </div>
    );
  }

  // Show session details if a session is selected
  if (selectedSession) {
    return <SessionDetails />;
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        {/* Back button and project name */}
        <div className="flex items-center gap-2 mb-3">
          <button
            onClick={() => selectProject(null)}
            className={clsx(
              'p-1.5 rounded-md',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
              'text-gray-500 dark:text-gray-400',
            )}
          >
            <ArrowLeftIcon className="w-4 h-4" />
          </button>
          <div>
            <h2 className="text-lg font-semibold text-gray-900 dark:text-white">{selectedProject.name}</h2>
            <p className="text-xs text-gray-500 dark:text-gray-400">
              {sessions.length} {t('projects.sessions', { count: sessions.length })}
            </p>
          </div>
        </div>

        {/* Session Search */}
        <div className="relative">
          <MagnifyingGlassIcon className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
          <input
            type="text"
            value={sessionSearch}
            onChange={(e) => handleSearch(e.target.value)}
            placeholder={t('projects.searchSessions')}
            className={clsx(
              'w-full pl-9 pr-3 py-2 rounded-lg',
              'bg-gray-100 dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'text-sm text-gray-900 dark:text-white',
              'placeholder-gray-500 dark:placeholder-gray-400',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
            )}
          />
        </div>
      </div>

      {/* Session List */}
      <div className="flex-1 overflow-y-auto p-4 space-y-2">
        {projectDebugSessions.length > 0 && (
          <div className="mb-4 rounded-xl border border-amber-200 bg-amber-50/80 p-3 dark:border-amber-900/60 dark:bg-amber-950/30">
            <div className="mb-2 flex items-center justify-between gap-2">
              <div>
                <h3 className="text-sm font-semibold text-amber-900 dark:text-amber-200">
                  {t('projects.debugCasesTitle', { defaultValue: 'Debug Cases' })}
                </h3>
                <p className="text-xs text-amber-700 dark:text-amber-300">
                  {t('projects.debugCasesDescription', {
                    defaultValue: 'Active or interrupted Debug mode sessions for this workspace.',
                  })}
                </p>
              </div>
              <span className="inline-flex items-center justify-center rounded-full bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-700 dark:bg-amber-900/60 dark:text-amber-200">
                {projectDebugSessions.length}
              </span>
            </div>
            <div className="space-y-2">
              {projectDebugSessions.map((session) => {
                const debug = session.modeSnapshots.debug;
                const chips = buildDebugStateChips(debug, { max: 5 });
                const summary = summarizeDebugCase(debug, session.lastError);
                return (
                  <button
                    key={session.sessionId}
                    type="button"
                    onClick={() => void handleOpenDebugCase(session.sessionId)}
                    className="w-full rounded-lg border border-amber-200 bg-white/80 px-3 py-2 text-left transition-colors hover:bg-white dark:border-amber-900/70 dark:bg-gray-900/60 dark:hover:bg-gray-900"
                  >
                    <div className="flex items-center gap-2">
                      <p className="min-w-0 flex-1 truncate text-sm font-medium text-gray-900 dark:text-white">
                        {session.displayTitle}
                      </p>
                      <span className="rounded-full bg-amber-100 px-2 py-0.5 text-[10px] font-medium text-amber-700 dark:bg-amber-900/60 dark:text-amber-200">
                        D
                      </span>
                    </div>
                    {chips.length > 0 ? (
                      <div className="mt-1 flex flex-wrap items-center gap-1.5">
                        {chips.map((chip) => (
                          <span
                            key={`${session.sessionId}:${chip}`}
                            className="rounded-full bg-gray-100 px-2 py-0.5 text-[10px] font-medium text-gray-600 dark:bg-gray-800 dark:text-gray-300"
                          >
                            {chip}
                          </span>
                        ))}
                      </div>
                    ) : null}
                    {summary ? (
                      <p className="mt-1 line-clamp-2 text-xs text-gray-600 dark:text-gray-400">{summary}</p>
                    ) : null}
                    <p className="mt-1 text-[11px] font-medium text-amber-700 dark:text-amber-300">
                      {t('projects.openDebugCase', { defaultValue: 'Open in Simple' })}
                    </p>
                  </button>
                );
              })}
            </div>
          </div>
        )}
        {loading.sessions && (
          <>
            <SessionSkeleton />
            <SessionSkeleton />
            <SessionSkeleton />
          </>
        )}

        {!loading.sessions && error && (
          <div className="text-center py-8">
            <p className="text-sm text-red-500 dark:text-red-400">{error}</p>
          </div>
        )}

        {!loading.sessions && !error && sessions.length === 0 && (
          <div className="text-center py-8">
            <p className="text-sm text-gray-500 dark:text-gray-400">
              {sessionSearch ? t('projects.noSearchResults') : t('projects.noSessions')}
            </p>
          </div>
        )}

        {renderSessions()}
      </div>
    </div>
  );
}

export default SessionList;
