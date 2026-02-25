/**
 * ProjectBrowser Component
 *
 * Main container for browsing projects with sorting and search.
 */

import { useEffect } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import * as Select from '@radix-ui/react-select';
import { ChevronDownIcon, CheckIcon, MagnifyingGlassIcon } from '@radix-ui/react-icons';
import { useProjectsStore } from '../../store/projects';
import type { ProjectSortBy } from '../../types/project';
import { ProjectCard } from './ProjectCard';
import { ProjectSkeleton } from './ProjectSkeleton';

export function ProjectBrowser() {
  const { t } = useTranslation();
  const {
    projects,
    selectedProject,
    sortBy,
    searchQuery,
    loading,
    error,
    fetchProjects,
    selectProject,
    searchProjects,
    setSortBy,
    setSearchQuery,
  } = useProjectsStore();

  // Fetch projects on mount
  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  const handleSearch = (query: string) => {
    setSearchQuery(query);
    if (query.trim()) {
      searchProjects(query);
    } else {
      fetchProjects();
    }
  };

  const sortOptions: { value: ProjectSortBy; label: string }[] = [
    { value: 'recent_activity', label: t('projects.sort.recentActivity') },
    { value: 'name', label: t('projects.sort.name') },
    { value: 'session_count', label: t('projects.sort.sessionCount') },
  ];

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-700">
        <h2 className="text-lg font-semibold text-gray-900 dark:text-white mb-3">{t('projects.title')}</h2>

        {/* Search */}
        <div className="relative mb-3">
          <MagnifyingGlassIcon className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => handleSearch(e.target.value)}
            placeholder={t('projects.searchPlaceholder')}
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

        {/* Sort Dropdown */}
        <Select.Root value={sortBy} onValueChange={(v: string) => setSortBy(v as ProjectSortBy)}>
          <Select.Trigger
            className={clsx(
              'inline-flex items-center gap-2 px-3 py-1.5 rounded-md',
              'bg-gray-100 dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'text-xs text-gray-600 dark:text-gray-400',
              'hover:bg-gray-200 dark:hover:bg-gray-700',
            )}
          >
            <span>{t('projects.sortBy')}:</span>
            <Select.Value />
            <Select.Icon>
              <ChevronDownIcon className="w-3.5 h-3.5" />
            </Select.Icon>
          </Select.Trigger>

          <Select.Portal>
            <Select.Content
              className={clsx(
                'overflow-hidden rounded-md',
                'bg-white dark:bg-gray-800',
                'border border-gray-200 dark:border-gray-700',
                'shadow-lg',
              )}
            >
              <Select.Viewport className="p-1">
                {sortOptions.map((option) => (
                  <Select.Item
                    key={option.value}
                    value={option.value}
                    className={clsx(
                      'flex items-center gap-2 px-3 py-2 rounded-md',
                      'text-xs text-gray-700 dark:text-gray-300',
                      'cursor-pointer outline-none',
                      'data-[highlighted]:bg-gray-100 dark:data-[highlighted]:bg-gray-700',
                    )}
                  >
                    <Select.ItemText>{option.label}</Select.ItemText>
                    <Select.ItemIndicator className="ml-auto">
                      <CheckIcon className="w-3.5 h-3.5" />
                    </Select.ItemIndicator>
                  </Select.Item>
                ))}
              </Select.Viewport>
            </Select.Content>
          </Select.Portal>
        </Select.Root>
      </div>

      {/* Project List */}
      <div className="flex-1 overflow-y-auto p-4 space-y-2">
        {loading.projects ? (
          // Loading skeletons
          <>
            <ProjectSkeleton />
            <ProjectSkeleton />
            <ProjectSkeleton />
          </>
        ) : error ? (
          // Error state
          <div className="text-center py-8">
            <p className="text-sm text-red-500 dark:text-red-400">{error}</p>
            <button
              onClick={() => fetchProjects()}
              className="mt-2 text-sm text-primary-600 dark:text-primary-400 hover:underline"
            >
              {t('common.retry')}
            </button>
          </div>
        ) : projects.length === 0 ? (
          // Empty state
          <div className="text-center py-8">
            <p className="text-sm text-gray-500 dark:text-gray-400">
              {searchQuery ? t('projects.noSearchResults') : t('projects.noProjects')}
            </p>
          </div>
        ) : (
          // Project cards
          projects.map((project) => (
            <ProjectCard
              key={project.id}
              project={project}
              isSelected={selectedProject?.id === project.id}
              onClick={() => selectProject(project)}
            />
          ))
        )}
      </div>
    </div>
  );
}

export default ProjectBrowser;
