/**
 * Dependency Editor Component
 *
 * UI for setting blockedBy relationships between stories
 * with visual indicators and circular dependency detection.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { usePRDStore, PRDStory } from '../../store/prd';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import {
  PlusIcon,
  Cross2Icon,
  Link2Icon,
  ExclamationTriangleIcon,
  ChevronDownIcon,
} from '@radix-ui/react-icons';

interface DependencyEditorProps {
  story: PRDStory;
}

export function DependencyEditor({ story }: DependencyEditorProps) {
  const { prd, addDependency, removeDependency, hasCircularDependency } = usePRDStore();
  const [showWarning, setShowWarning] = useState<string | null>(null);

  // Get available stories that can be dependencies (not self, not already a dependency)
  const availableStories = prd.stories.filter(
    (s) => s.id !== story.id && !story.dependencies.includes(s.id)
  );

  // Get story title by ID
  const getStoryTitle = (id: string) => {
    const s = prd.stories.find((story) => story.id === id);
    return s?.title || id;
  };

  const handleAddDependency = (dependsOnId: string) => {
    if (hasCircularDependency(story.id, dependsOnId)) {
      setShowWarning(`Adding this dependency would create a circular reference`);
      setTimeout(() => setShowWarning(null), 3000);
      return;
    }
    addDependency(story.id, dependsOnId);
  };

  const handleRemoveDependency = (dependsOnId: string) => {
    removeDependency(story.id, dependsOnId);
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <Link2Icon className="w-4 h-4 text-gray-400" />
        <span className="text-xs font-medium text-gray-500 dark:text-gray-400">
          Blocked By
        </span>
        {story.dependencies.length > 0 && (
          <span className="px-1.5 py-0.5 text-xs rounded-full bg-amber-100 dark:bg-amber-900 text-amber-700 dark:text-amber-300">
            {story.dependencies.length}
          </span>
        )}
      </div>

      {/* Warning message */}
      {showWarning && (
        <div className="flex items-center gap-2 p-2 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800">
          <ExclamationTriangleIcon className="w-4 h-4 text-red-500" />
          <span className="text-xs text-red-600 dark:text-red-400">
            {showWarning}
          </span>
        </div>
      )}

      {/* Current dependencies */}
      <div className="flex flex-wrap gap-2">
        {story.dependencies.map((depId) => (
          <span
            key={depId}
            className={clsx(
              'inline-flex items-center gap-1 px-2 py-1 rounded-lg text-sm',
              'bg-gray-100 dark:bg-gray-700',
              'text-gray-700 dark:text-gray-300'
            )}
          >
            <span className="truncate max-w-[150px]">{getStoryTitle(depId)}</span>
            <button
              onClick={() => handleRemoveDependency(depId)}
              className="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-600 text-gray-400 hover:text-red-500"
              title="Remove dependency"
            >
              <Cross2Icon className="w-3 h-3" />
            </button>
          </span>
        ))}

        {/* Add dependency dropdown */}
        {availableStories.length > 0 && (
          <DropdownMenu.Root>
            <DropdownMenu.Trigger asChild>
              <button
                className={clsx(
                  'inline-flex items-center gap-1 px-2 py-1 rounded-lg text-sm',
                  'border border-dashed border-gray-300 dark:border-gray-600',
                  'text-gray-500 dark:text-gray-400',
                  'hover:border-primary-500 hover:text-primary-600 dark:hover:text-primary-400',
                  'transition-colors'
                )}
              >
                <PlusIcon className="w-3 h-3" />
                <span>Add</span>
                <ChevronDownIcon className="w-3 h-3" />
              </button>
            </DropdownMenu.Trigger>

            <DropdownMenu.Portal>
              <DropdownMenu.Content
                className={clsx(
                  'min-w-[200px] max-h-[300px] overflow-auto p-1 rounded-lg shadow-lg',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-200 dark:border-gray-700'
                )}
                sideOffset={5}
              >
                {availableStories.map((s) => (
                  <DropdownMenu.Item
                    key={s.id}
                    onClick={() => handleAddDependency(s.id)}
                    className={clsx(
                      'flex items-center gap-2 px-3 py-2 rounded-md cursor-pointer',
                      'text-sm text-gray-700 dark:text-gray-300',
                      'hover:bg-gray-100 dark:hover:bg-gray-700',
                      'focus:outline-none focus:bg-gray-100 dark:focus:bg-gray-700'
                    )}
                  >
                    <span className="flex items-center justify-center w-5 h-5 rounded bg-gray-200 dark:bg-gray-600 text-xs">
                      {prd.stories.findIndex((st) => st.id === s.id) + 1}
                    </span>
                    <span className="truncate">{s.title}</span>
                  </DropdownMenu.Item>
                ))}
              </DropdownMenu.Content>
            </DropdownMenu.Portal>
          </DropdownMenu.Root>
        )}

        {/* Empty state */}
        {story.dependencies.length === 0 && availableStories.length === 0 && (
          <span className="text-xs text-gray-400">No dependencies available</span>
        )}
      </div>
    </div>
  );
}

/**
 * Compact Dependency Badge
 *
 * Shows dependency count with expandable list.
 */
interface DependencyBadgeProps {
  story: PRDStory;
  compact?: boolean;
}

export function DependencyBadge({ story, compact = false }: DependencyBadgeProps) {
  const { prd } = usePRDStore();

  if (story.dependencies.length === 0) {
    return null;
  }

  const getStoryTitle = (id: string) => {
    const s = prd.stories.find((story) => story.id === id);
    return s?.title || id;
  };

  if (compact) {
    return (
      <span
        className="px-1.5 py-0.5 text-xs rounded-full bg-amber-100 dark:bg-amber-900 text-amber-700 dark:text-amber-300"
        title={story.dependencies.map(getStoryTitle).join(', ')}
      >
        {story.dependencies.length} dep{story.dependencies.length !== 1 ? 's' : ''}
      </span>
    );
  }

  return (
    <div className="flex items-center gap-1 flex-wrap">
      <Link2Icon className="w-3 h-3 text-gray-400" />
      {story.dependencies.map((depId) => (
        <span
          key={depId}
          className="px-1.5 py-0.5 text-xs rounded-full bg-amber-100 dark:bg-amber-900 text-amber-700 dark:text-amber-300"
        >
          {getStoryTitle(depId)}
        </span>
      ))}
    </div>
  );
}

export default DependencyEditor;
