/**
 * Story List Component
 *
 * Displays stories in a list with inline editing capabilities
 * and CRUD operations.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { usePRDStore, PRDStory, StoryStatus } from '../../store/prd';
import {
  Pencil1Icon,
  TrashIcon,
  PlusIcon,
  CheckIcon,
  Cross2Icon,
  ChevronDownIcon,
  ChevronRightIcon,
} from '@radix-ui/react-icons';

interface StoryCardProps {
  story: PRDStory;
  index: number;
  onEdit?: (story: PRDStory) => void;
  onDelete: (id: string) => void;
}

function StoryCard({ story, index, onDelete }: StoryCardProps) {
  const { updateStory } = usePRDStore();
  const [isEditing, setIsEditing] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [editedTitle, setEditedTitle] = useState(story.title);
  const [editedDescription, setEditedDescription] = useState(story.description);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const statusColors: Record<StoryStatus, string> = {
    pending: 'bg-gray-400',
    in_progress: 'bg-blue-500 animate-pulse',
    completed: 'bg-green-500',
    failed: 'bg-red-500',
  };

  const statusLabels: Record<StoryStatus, string> = {
    pending: 'Pending',
    in_progress: 'In Progress',
    completed: 'Completed',
    failed: 'Failed',
  };

  const handleSave = () => {
    updateStory(story.id, {
      title: editedTitle,
      description: editedDescription,
    });
    setIsEditing(false);
  };

  const handleCancel = () => {
    setEditedTitle(story.title);
    setEditedDescription(story.description);
    setIsEditing(false);
  };

  const handleDelete = () => {
    if (showDeleteConfirm) {
      onDelete(story.id);
      setShowDeleteConfirm(false);
    } else {
      setShowDeleteConfirm(true);
    }
  };

  return (
    <div
      className={clsx(
        'group rounded-lg border transition-all',
        'bg-white dark:bg-gray-800',
        'border-gray-200 dark:border-gray-700',
        'hover:border-gray-300 dark:hover:border-gray-600'
      )}
    >
      <div className="p-4">
        <div className="flex items-center gap-3">
          {/* Order number */}
          <span className="flex items-center justify-center w-8 h-8 rounded-full bg-gray-100 dark:bg-gray-700 text-sm font-medium text-gray-600 dark:text-gray-400">
            {index + 1}
          </span>

          {/* Status indicator */}
          <span
            className={clsx(
              'w-2.5 h-2.5 rounded-full',
              statusColors[story.status]
            )}
            title={statusLabels[story.status]}
          />

          {/* Title */}
          {isEditing ? (
            <input
              type="text"
              value={editedTitle}
              onChange={(e) => setEditedTitle(e.target.value)}
              className={clsx(
                'flex-1 px-2 py-1 rounded',
                'bg-gray-50 dark:bg-gray-900',
                'text-gray-900 dark:text-white font-medium',
                'border border-primary-500',
                'focus:outline-none focus:ring-2 focus:ring-primary-500'
              )}
              autoFocus
            />
          ) : (
            <button
              onClick={() => setIsExpanded(!isExpanded)}
              className="flex-1 flex items-center gap-2 text-left"
            >
              {isExpanded ? (
                <ChevronDownIcon className="w-4 h-4 text-gray-400" />
              ) : (
                <ChevronRightIcon className="w-4 h-4 text-gray-400" />
              )}
              <span className="font-medium text-gray-900 dark:text-white">
                {story.title}
              </span>
            </button>
          )}

          {/* Actions */}
          <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
            {isEditing ? (
              <>
                <button
                  onClick={handleSave}
                  className="p-1.5 rounded hover:bg-green-100 dark:hover:bg-green-900 text-green-600 dark:text-green-400"
                  title="Save"
                >
                  <CheckIcon className="w-4 h-4" />
                </button>
                <button
                  onClick={handleCancel}
                  className="p-1.5 rounded hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-600 dark:text-gray-400"
                  title="Cancel"
                >
                  <Cross2Icon className="w-4 h-4" />
                </button>
              </>
            ) : (
              <>
                <button
                  onClick={() => setIsEditing(true)}
                  className="p-1.5 rounded hover:bg-gray-100 dark:hover:bg-gray-700 text-gray-600 dark:text-gray-400"
                  title="Edit"
                >
                  <Pencil1Icon className="w-4 h-4" />
                </button>
                {showDeleteConfirm ? (
                  <div className="flex items-center gap-1 animate-in fade-in">
                    <button
                      onClick={handleDelete}
                      className="px-2 py-1 text-xs rounded bg-red-600 text-white hover:bg-red-700"
                    >
                      Confirm
                    </button>
                    <button
                      onClick={() => setShowDeleteConfirm(false)}
                      className="px-2 py-1 text-xs rounded bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300"
                    >
                      Cancel
                    </button>
                  </div>
                ) : (
                  <button
                    onClick={handleDelete}
                    className="p-1.5 rounded hover:bg-red-100 dark:hover:bg-red-900 text-red-600 dark:text-red-400"
                    title="Delete"
                  >
                    <TrashIcon className="w-4 h-4" />
                  </button>
                )}
              </>
            )}
          </div>
        </div>

        {/* Expanded content */}
        {isExpanded && (
          <div className="mt-4 ml-11 space-y-4">
            {/* Description */}
            <div>
              <label className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                Description
              </label>
              {isEditing ? (
                <textarea
                  value={editedDescription}
                  onChange={(e) => setEditedDescription(e.target.value)}
                  rows={3}
                  className={clsx(
                    'w-full px-3 py-2 rounded-lg',
                    'bg-gray-50 dark:bg-gray-900',
                    'text-gray-900 dark:text-white text-sm',
                    'border border-primary-500',
                    'focus:outline-none focus:ring-2 focus:ring-primary-500'
                  )}
                />
              ) : (
                <p className="text-sm text-gray-600 dark:text-gray-400">
                  {story.description || 'No description'}
                </p>
              )}
            </div>

            {/* Acceptance Criteria */}
            {story.acceptance_criteria && story.acceptance_criteria.length > 0 && (
              <div>
                <label className="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                  Acceptance Criteria
                </label>
                <ul className="space-y-1">
                  {story.acceptance_criteria.map((criterion, i) => (
                    <li
                      key={i}
                      className="flex items-start gap-2 text-sm text-gray-600 dark:text-gray-400"
                    >
                      <CheckIcon className="w-4 h-4 mt-0.5 text-green-500 shrink-0" />
                      <span>{criterion}</span>
                    </li>
                  ))}
                </ul>
              </div>
            )}

            {/* Dependencies badge */}
            {story.dependencies && story.dependencies.length > 0 && (
              <div className="flex items-center gap-2">
                <span className="text-xs font-medium text-gray-500 dark:text-gray-400">
                  Blocked by:
                </span>
                <div className="flex flex-wrap gap-1">
                  {story.dependencies.map((depId) => (
                    <span
                      key={depId}
                      className="px-2 py-0.5 text-xs rounded-full bg-amber-100 dark:bg-amber-900 text-amber-700 dark:text-amber-300"
                    >
                      {depId}
                    </span>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

interface AddStoryFormProps {
  onAdd: (title: string, description: string) => void;
  onCancel: () => void;
}

function AddStoryForm({ onAdd, onCancel }: AddStoryFormProps) {
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (title.trim()) {
      onAdd(title.trim(), description.trim());
      setTitle('');
      setDescription('');
    }
  };

  return (
    <form
      onSubmit={handleSubmit}
      className={clsx(
        'p-4 rounded-lg border-2 border-dashed',
        'border-primary-300 dark:border-primary-700',
        'bg-primary-50 dark:bg-primary-900/20'
      )}
    >
      <div className="space-y-3">
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            Story Title
          </label>
          <input
            type="text"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Enter story title..."
            className={clsx(
              'w-full px-3 py-2 rounded-lg',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'border border-gray-200 dark:border-gray-700',
              'placeholder-gray-400 dark:placeholder-gray-500',
              'focus:outline-none focus:ring-2 focus:ring-primary-500'
            )}
            autoFocus
          />
        </div>
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            Description (optional)
          </label>
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Enter story description..."
            rows={2}
            className={clsx(
              'w-full px-3 py-2 rounded-lg',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'border border-gray-200 dark:border-gray-700',
              'placeholder-gray-400 dark:placeholder-gray-500',
              'focus:outline-none focus:ring-2 focus:ring-primary-500'
            )}
          />
        </div>
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            className={clsx(
              'px-4 py-2 rounded-lg',
              'bg-gray-100 dark:bg-gray-700',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-600'
            )}
          >
            Cancel
          </button>
          <button
            type="submit"
            disabled={!title.trim()}
            className={clsx(
              'px-4 py-2 rounded-lg',
              'bg-primary-600 text-white',
              'hover:bg-primary-700',
              'disabled:opacity-50 disabled:cursor-not-allowed'
            )}
          >
            Add Story
          </button>
        </div>
      </div>
    </form>
  );
}

export function StoryList() {
  const { prd, addStory, deleteStory } = usePRDStore();
  const [isAddingStory, setIsAddingStory] = useState(false);

  const handleAddStory = (title: string, description: string) => {
    addStory({
      title,
      description,
      acceptance_criteria: [],
      status: 'pending',
      dependencies: [],
      agent: 'claude-code',
    });
    setIsAddingStory(false);
  };

  if (prd.stories.length === 0 && !isAddingStory) {
    return (
      <div className="flex flex-col items-center justify-center py-12 text-center">
        <div className="w-16 h-16 mb-4 rounded-full bg-gray-100 dark:bg-gray-800 flex items-center justify-center">
          <PlusIcon className="w-8 h-8 text-gray-400" />
        </div>
        <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-2">
          No stories yet
        </h3>
        <p className="text-gray-500 dark:text-gray-400 mb-4 max-w-sm">
          Generate a PRD from your requirements or add stories manually.
        </p>
        <button
          onClick={() => setIsAddingStory(true)}
          className={clsx(
            'flex items-center gap-2 px-4 py-2 rounded-lg',
            'bg-primary-600 text-white',
            'hover:bg-primary-700'
          )}
        >
          <PlusIcon className="w-4 h-4" />
          Add Story
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {/* Story cards */}
      {prd.stories.map((story, index) => (
        <StoryCard
          key={story.id}
          story={story}
          index={index}
          onEdit={() => {}}
          onDelete={deleteStory}
        />
      ))}

      {/* Add story form or button */}
      {isAddingStory ? (
        <AddStoryForm
          onAdd={handleAddStory}
          onCancel={() => setIsAddingStory(false)}
        />
      ) : (
        <button
          onClick={() => setIsAddingStory(true)}
          className={clsx(
            'w-full flex items-center justify-center gap-2 p-4 rounded-lg',
            'border-2 border-dashed border-gray-300 dark:border-gray-600',
            'text-gray-500 dark:text-gray-400',
            'hover:border-primary-500 hover:text-primary-600 dark:hover:text-primary-400',
            'transition-colors'
          )}
        >
          <PlusIcon className="w-4 h-4" />
          Add Story
        </button>
      )}
    </div>
  );
}

export default StoryList;
