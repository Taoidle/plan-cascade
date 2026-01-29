/**
 * Draft Manager Component
 *
 * Handles draft save/load functionality with auto-save
 * and draft management UI.
 */

import { useState, useEffect, useRef } from 'react';
import { clsx } from 'clsx';
import { usePRDStore } from '../../store/prd';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import * as Dialog from '@radix-ui/react-dialog';
import {
  DownloadIcon,
  UploadIcon,
  TrashIcon,
  ChevronDownIcon,
  Cross2Icon,
  ClockIcon,
  CheckIcon,
} from '@radix-ui/react-icons';

export function DraftManager() {
  const { prd, drafts, saveDraft, loadDraft, deleteDraft, autoSave } = usePRDStore();
  const [saveDialogOpen, setSaveDialogOpen] = useState(false);
  const [draftName, setDraftName] = useState('');
  const [lastSaved, setLastSaved] = useState<Date | null>(null);
  const autoSaveIntervalRef = useRef<NodeJS.Timeout | null>(null);

  // Auto-save every 30 seconds when there are stories
  useEffect(() => {
    if (prd.stories.length > 0) {
      autoSaveIntervalRef.current = setInterval(() => {
        autoSave();
        setLastSaved(new Date());
      }, 30000);

      return () => {
        if (autoSaveIntervalRef.current) {
          clearInterval(autoSaveIntervalRef.current);
        }
      };
    }
  }, [prd.stories.length, autoSave]);

  const handleSaveDraft = () => {
    saveDraft(draftName || undefined);
    setDraftName('');
    setSaveDialogOpen(false);
    setLastSaved(new Date());
  };

  const formatTimestamp = (timestamp: number) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const minutes = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (minutes < 1) return 'Just now';
    if (minutes < 60) return `${minutes}m ago`;
    if (hours < 24) return `${hours}h ago`;
    if (days < 7) return `${days}d ago`;
    return date.toLocaleDateString();
  };

  // Filter out auto-save from displayed drafts for the dropdown
  const userDrafts = drafts.filter((d) => d.name !== 'Auto-save');
  const autoSaveDraft = drafts.find((d) => d.name === 'Auto-save');

  return (
    <div className="flex items-center gap-2">
      {/* Auto-save indicator */}
      {lastSaved && (
        <span className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400">
          <CheckIcon className="w-3 h-3 text-green-500" />
          Saved {formatTimestamp(lastSaved.getTime())}
        </span>
      )}

      {/* Save Draft Button */}
      <Dialog.Root open={saveDialogOpen} onOpenChange={setSaveDialogOpen}>
        <Dialog.Trigger asChild>
          <button
            className={clsx(
              'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm',
              'bg-gray-100 dark:bg-gray-700',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-600',
              'transition-colors'
            )}
            title="Save Draft"
          >
            <DownloadIcon className="w-4 h-4" />
            <span>Save</span>
          </button>
        </Dialog.Trigger>

        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 bg-black/50" />
          <Dialog.Content
            className={clsx(
              'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
              'w-full max-w-md p-6 rounded-xl shadow-xl',
              'bg-white dark:bg-gray-800'
            )}
          >
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
              Save Draft
            </Dialog.Title>

            <div className="space-y-4">
              <div>
                <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
                  Draft Name (optional)
                </label>
                <input
                  type="text"
                  value={draftName}
                  onChange={(e) => setDraftName(e.target.value)}
                  placeholder={`Draft ${new Date().toLocaleString()}`}
                  className={clsx(
                    'w-full px-3 py-2 rounded-lg',
                    'bg-gray-50 dark:bg-gray-900',
                    'border border-gray-300 dark:border-gray-600',
                    'focus:outline-none focus:ring-2 focus:ring-primary-500'
                  )}
                  autoFocus
                />
              </div>

              <div className="flex justify-end gap-2">
                <Dialog.Close asChild>
                  <button
                    className={clsx(
                      'px-4 py-2 rounded-lg',
                      'bg-gray-100 dark:bg-gray-700',
                      'text-gray-700 dark:text-gray-300',
                      'hover:bg-gray-200 dark:hover:bg-gray-600'
                    )}
                  >
                    Cancel
                  </button>
                </Dialog.Close>
                <button
                  onClick={handleSaveDraft}
                  className={clsx(
                    'px-4 py-2 rounded-lg',
                    'bg-primary-600 text-white',
                    'hover:bg-primary-700'
                  )}
                >
                  Save Draft
                </button>
              </div>
            </div>

            <Dialog.Close asChild>
              <button
                className="absolute top-4 right-4 p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-700"
                aria-label="Close"
              >
                <Cross2Icon className="w-4 h-4" />
              </button>
            </Dialog.Close>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      {/* Load Draft Dropdown */}
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <button
            className={clsx(
              'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm',
              'bg-gray-100 dark:bg-gray-700',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-600',
              'transition-colors',
              drafts.length === 0 && 'opacity-50 cursor-not-allowed'
            )}
            disabled={drafts.length === 0}
            title="Load Draft"
          >
            <UploadIcon className="w-4 h-4" />
            <span>Load</span>
            <ChevronDownIcon className="w-3 h-3" />
          </button>
        </DropdownMenu.Trigger>

        <DropdownMenu.Portal>
          <DropdownMenu.Content
            className={clsx(
              'min-w-[250px] max-h-[400px] overflow-auto p-1 rounded-lg shadow-lg',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700'
            )}
            sideOffset={5}
            align="end"
          >
            {/* Auto-save section */}
            {autoSaveDraft && (
              <>
                <DropdownMenu.Label className="px-3 py-1.5 text-xs font-medium text-gray-500 dark:text-gray-400">
                  Auto-saved
                </DropdownMenu.Label>
                <DraftItem
                  draft={autoSaveDraft}
                  onLoad={() => loadDraft(autoSaveDraft.id)}
                  onDelete={() => deleteDraft(autoSaveDraft.id)}
                  formatTimestamp={formatTimestamp}
                />
                {userDrafts.length > 0 && <DropdownMenu.Separator className="my-1 h-px bg-gray-200 dark:bg-gray-700" />}
              </>
            )}

            {/* User drafts section */}
            {userDrafts.length > 0 && (
              <>
                <DropdownMenu.Label className="px-3 py-1.5 text-xs font-medium text-gray-500 dark:text-gray-400">
                  Saved Drafts
                </DropdownMenu.Label>
                {userDrafts.map((draft) => (
                  <DraftItem
                    key={draft.id}
                    draft={draft}
                    onLoad={() => loadDraft(draft.id)}
                    onDelete={() => deleteDraft(draft.id)}
                    formatTimestamp={formatTimestamp}
                  />
                ))}
              </>
            )}

            {drafts.length === 0 && (
              <div className="px-3 py-4 text-center text-sm text-gray-500 dark:text-gray-400">
                No saved drafts
              </div>
            )}
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
    </div>
  );
}

interface DraftItemProps {
  draft: {
    id: string;
    name: string;
    timestamp: number;
    prd: { stories: unknown[] };
  };
  onLoad: () => void;
  onDelete: () => void;
  formatTimestamp: (timestamp: number) => string;
}

function DraftItem({ draft, onLoad, onDelete, formatTimestamp }: DraftItemProps) {
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  return (
    <div
      className={clsx(
        'flex items-center gap-2 px-3 py-2 rounded-md',
        'hover:bg-gray-100 dark:hover:bg-gray-700'
      )}
    >
      <button
        onClick={onLoad}
        className="flex-1 flex items-center gap-2 text-left"
      >
        <ClockIcon className="w-4 h-4 text-gray-400 shrink-0" />
        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium text-gray-900 dark:text-white truncate">
            {draft.name}
          </p>
          <p className="text-xs text-gray-500 dark:text-gray-400">
            {formatTimestamp(draft.timestamp)} &middot; {(draft.prd as { stories: unknown[] }).stories?.length || 0} stories
          </p>
        </div>
      </button>

      {showDeleteConfirm ? (
        <div className="flex items-center gap-1">
          <button
            onClick={() => {
              onDelete();
              setShowDeleteConfirm(false);
            }}
            className="px-2 py-1 text-xs rounded bg-red-600 text-white hover:bg-red-700"
          >
            Delete
          </button>
          <button
            onClick={() => setShowDeleteConfirm(false)}
            className="px-2 py-1 text-xs rounded bg-gray-200 dark:bg-gray-600"
          >
            Cancel
          </button>
        </div>
      ) : (
        <button
          onClick={() => setShowDeleteConfirm(true)}
          className="p-1 rounded text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20"
          title="Delete draft"
        >
          <TrashIcon className="w-4 h-4" />
        </button>
      )}
    </div>
  );
}

export default DraftManager;
