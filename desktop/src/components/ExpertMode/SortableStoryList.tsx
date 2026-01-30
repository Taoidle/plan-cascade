/**
 * Sortable Story List Component
 *
 * Drag-and-drop enabled story list using @dnd-kit/sortable.
 * Provides visual feedback during drag and keyboard accessibility.
 */

import { useState, useMemo } from 'react';
import { clsx } from 'clsx';
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  DragEndEvent,
  DragStartEvent,
  DragOverlay,
} from '@dnd-kit/core';
import {
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { usePRDStore, PRDStory, StoryStatus } from '../../store/prd';
import {
  DragHandleDots2Icon,
  ChevronDownIcon,
  ChevronRightIcon,
  Pencil1Icon,
  TrashIcon,
  CheckIcon,
  Cross2Icon,
} from '@radix-ui/react-icons';

interface SortableStoryCardProps {
  story: PRDStory;
  index: number;
  isActive?: boolean;
}

function SortableStoryCard({ story, index, isActive }: SortableStoryCardProps) {
  const { updateStory, deleteStory } = usePRDStore();
  const [isEditing, setIsEditing] = useState(false);
  const [isExpanded, setIsExpanded] = useState(false);
  const [editedTitle, setEditedTitle] = useState(story.title);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: story.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  const statusColors: Record<StoryStatus, string> = {
    pending: 'bg-gray-400',
    in_progress: 'bg-blue-500 animate-pulse',
    completed: 'bg-green-500',
    failed: 'bg-red-500',
  };

  const handleSave = () => {
    updateStory(story.id, { title: editedTitle });
    setIsEditing(false);
  };

  const handleCancel = () => {
    setEditedTitle(story.title);
    setIsEditing(false);
  };

  const handleDelete = () => {
    if (showDeleteConfirm) {
      deleteStory(story.id);
      setShowDeleteConfirm(false);
    } else {
      setShowDeleteConfirm(true);
    }
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={clsx(
        'group rounded-lg border transition-all',
        'bg-white dark:bg-gray-800',
        isDragging
          ? 'border-primary-500 shadow-lg opacity-50'
          : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600',
        isActive && 'ring-2 ring-primary-500'
      )}
    >
      <div className="p-4">
        <div className="flex items-center gap-3">
          {/* Drag handle */}
          <button
            {...attributes}
            {...listeners}
            className={clsx(
              'p-1 rounded cursor-grab active:cursor-grabbing',
              'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-700',
              'focus:outline-none focus:ring-2 focus:ring-primary-500'
            )}
            title="Drag to reorder"
          >
            <DragHandleDots2Icon className="w-5 h-5" />
          </button>

          {/* Order number */}
          <span className="flex items-center justify-center w-7 h-7 rounded-full bg-gray-100 dark:bg-gray-700 text-sm font-medium text-gray-600 dark:text-gray-400">
            {index + 1}
          </span>

          {/* Status indicator */}
          <span
            className={clsx(
              'w-2.5 h-2.5 rounded-full shrink-0',
              statusColors[story.status]
            )}
          />

          {/* Title */}
          {isEditing ? (
            <input
              type="text"
              value={editedTitle}
              onChange={(e) => setEditedTitle(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleSave();
                if (e.key === 'Escape') handleCancel();
              }}
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
              className="flex-1 flex items-center gap-2 text-left min-w-0"
            >
              {isExpanded ? (
                <ChevronDownIcon className="w-4 h-4 text-gray-400 shrink-0" />
              ) : (
                <ChevronRightIcon className="w-4 h-4 text-gray-400 shrink-0" />
              )}
              <span className="font-medium text-gray-900 dark:text-white truncate">
                {story.title}
              </span>
            </button>
          )}

          {/* Actions */}
          <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
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
                  <div className="flex items-center gap-1">
                    <button
                      onClick={handleDelete}
                      className="px-2 py-1 text-xs rounded bg-red-600 text-white hover:bg-red-700"
                    >
                      Confirm
                    </button>
                    <button
                      onClick={() => setShowDeleteConfirm(false)}
                      className="px-2 py-1 text-xs rounded bg-gray-200 dark:bg-gray-700"
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
          <div className="mt-4 ml-14 space-y-3">
            <p className="text-sm text-gray-600 dark:text-gray-400">
              {story.description || 'No description'}
            </p>
            {story.dependencies.length > 0 && (
              <div className="flex items-center gap-2 flex-wrap">
                <span className="text-xs text-gray-500">Blocked by:</span>
                {story.dependencies.map((dep) => (
                  <span
                    key={dep}
                    className="px-2 py-0.5 text-xs rounded-full bg-amber-100 dark:bg-amber-900 text-amber-700 dark:text-amber-300"
                  >
                    {dep}
                  </span>
                ))}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

// Drag overlay card for smooth dragging animation
function DragOverlayCard({ story, index }: { story: PRDStory; index: number }) {
  const statusColors: Record<StoryStatus, string> = {
    pending: 'bg-gray-400',
    in_progress: 'bg-blue-500',
    completed: 'bg-green-500',
    failed: 'bg-red-500',
  };

  return (
    <div
      className={clsx(
        'rounded-lg border-2 border-primary-500 shadow-2xl',
        'bg-white dark:bg-gray-800'
      )}
    >
      <div className="p-4 flex items-center gap-3">
        <DragHandleDots2Icon className="w-5 h-5 text-primary-500" />
        <span className="flex items-center justify-center w-7 h-7 rounded-full bg-primary-100 dark:bg-primary-900 text-sm font-medium text-primary-600 dark:text-primary-400">
          {index + 1}
        </span>
        <span className={clsx('w-2.5 h-2.5 rounded-full', statusColors[story.status])} />
        <span className="font-medium text-gray-900 dark:text-white">
          {story.title}
        </span>
      </div>
    </div>
  );
}

export function SortableStoryList() {
  const { prd, reorderStories } = usePRDStore();
  const [activeId, setActiveId] = useState<string | null>(null);

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  const storyIds = useMemo(() => prd.stories.map((s) => s.id), [prd.stories]);
  const activeStory = activeId ? prd.stories.find((s) => s.id === activeId) : null;
  const activeIndex = activeId ? prd.stories.findIndex((s) => s.id === activeId) : -1;

  const handleDragStart = (event: DragStartEvent) => {
    setActiveId(event.active.id as string);
  };

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;
    setActiveId(null);

    if (over && active.id !== over.id) {
      const oldIndex = prd.stories.findIndex((s) => s.id === active.id);
      const newIndex = prd.stories.findIndex((s) => s.id === over.id);
      reorderStories(oldIndex, newIndex);
    }
  };

  if (prd.stories.length === 0) {
    return null;
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      onDragStart={handleDragStart}
      onDragEnd={handleDragEnd}
    >
      <SortableContext items={storyIds} strategy={verticalListSortingStrategy}>
        <div className="space-y-2">
          {prd.stories.map((story, index) => (
            <SortableStoryCard
              key={story.id}
              story={story}
              index={index}
              isActive={story.id === activeId}
            />
          ))}
        </div>
      </SortableContext>

      <DragOverlay>
        {activeStory && (
          <DragOverlayCard story={activeStory} index={activeIndex} />
        )}
      </DragOverlay>
    </DndContext>
  );
}

export default SortableStoryList;
