/**
 * SessionList Component
 *
 * Displays session history for a selected project.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ArrowLeftIcon, MagnifyingGlassIcon } from '@radix-ui/react-icons';
import { useState, useCallback } from 'react';
import { useProjectsStore } from '../../store/projects';
import type { Session } from '../../store';
import { SessionCard } from './SessionCard';
import { SessionSkeleton } from './SessionSkeleton';
import { SessionDetails } from './SessionDetails';
import { debounce } from './utils';

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
  // Explicitly type sessions to help TypeScript
  const sessions: Session[] = store.sessions;

  // Debounced search
  const debouncedSearch = useCallback(
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
