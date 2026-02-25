/**
 * MemoryDetail Component
 *
 * Detail/edit view for a memory entry within the management dialog.
 * Shows content, category, importance, keywords, and timestamps.
 * Supports editing and deleting.
 */

import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { Cross2Icon, Pencil1Icon, TrashIcon } from '@radix-ui/react-icons';
import { CategoryBadge } from './CategoryBadge';
import { ImportanceBar } from './ImportanceBar';
import type { MemoryEntry, MemoryCategory } from '../../types/skillMemory';
import { MEMORY_CATEGORIES } from '../../types/skillMemory';

interface MemoryDetailProps {
  memory: MemoryEntry;
  onClose: () => void;
  onUpdate: (
    id: string,
    updates: {
      content?: string;
      category?: MemoryCategory;
      importance?: number;
      keywords?: string[];
    }
  ) => void;
  onDelete: (id: string) => void;
  className?: string;
}

export function MemoryDetail({
  memory,
  onClose,
  onUpdate,
  onDelete,
  className,
}: MemoryDetailProps) {
  const { t } = useTranslation('simpleMode');
  const [editing, setEditing] = useState(false);
  const [editContent, setEditContent] = useState(memory.content);
  const [editCategory, setEditCategory] = useState<MemoryCategory>(memory.category);
  const [editImportance, setEditImportance] = useState(memory.importance);
  const [editKeywords, setEditKeywords] = useState<string[]>(memory.keywords);
  const [keywordInput, setKeywordInput] = useState('');
  const [confirmDelete, setConfirmDelete] = useState(false);

  const handleSave = useCallback(() => {
    const updates: Parameters<typeof onUpdate>[1] = {};
    if (editContent !== memory.content) updates.content = editContent;
    if (editCategory !== memory.category) updates.category = editCategory;
    if (editImportance !== memory.importance) updates.importance = editImportance;
    if (JSON.stringify(editKeywords) !== JSON.stringify(memory.keywords)) updates.keywords = editKeywords;

    if (Object.keys(updates).length > 0) {
      onUpdate(memory.id, updates);
    }
    setEditing(false);
  }, [memory, editContent, editCategory, editImportance, editKeywords, onUpdate]);

  const handleDelete = useCallback(() => {
    if (confirmDelete) {
      onDelete(memory.id);
      onClose();
    } else {
      setConfirmDelete(true);
      // Auto-reset after 3 seconds
      setTimeout(() => setConfirmDelete(false), 3000);
    }
  }, [confirmDelete, memory.id, onDelete, onClose]);

  const handleCancel = useCallback(() => {
    setEditing(false);
    setEditContent(memory.content);
    setEditCategory(memory.category);
    setEditImportance(memory.importance);
    setEditKeywords(memory.keywords);
    setKeywordInput('');
  }, [memory]);

  return (
    <div
      data-testid="memory-detail"
      className={clsx('flex flex-col h-full', className)}
    >
      {/* Header */}
      <div className="flex items-start justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2 mb-1">
            <CategoryBadge category={editing ? editCategory : memory.category} />
            <ImportanceBar
              value={editing ? editImportance : memory.importance}
              showLabel
              className="flex-1"
            />
          </div>
          <p className="text-2xs text-gray-500 dark:text-gray-400">
            {t('skillPanel.createdAt')}: {new Date(memory.created_at).toLocaleDateString()}
          </p>
        </div>
        <div className="flex items-center gap-1 shrink-0 ml-2">
          {!editing && (
            <button
              onClick={() => setEditing(true)}
              className="p-1 rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
              title={t('skillPanel.edit')}
            >
              <Pencil1Icon className="w-3.5 h-3.5" />
            </button>
          )}
          <button
            onClick={handleDelete}
            className={clsx(
              'p-1 rounded-md',
              confirmDelete
                ? 'text-red-600 bg-red-50 dark:bg-red-900/20'
                : 'text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20'
            )}
            title={confirmDelete ? t('skillPanel.confirmDelete') : t('skillPanel.delete')}
          >
            <TrashIcon className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={onClose}
            className="p-1 rounded-md text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
            title={t('skillPanel.close')}
          >
            <Cross2Icon className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {editing ? (
          <>
            {/* Category selector */}
            <div>
              <label className="text-2xs font-medium text-gray-500 dark:text-gray-400 block mb-1">
                {t('skillPanel.category')}
              </label>
              <select
                value={editCategory}
                onChange={(e) => setEditCategory(e.target.value as MemoryCategory)}
                className={clsx(
                  'w-full px-2 py-1.5 rounded-md text-xs',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'text-gray-700 dark:text-gray-300',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500'
                )}
              >
                {MEMORY_CATEGORIES.map((cat) => (
                  <option key={cat} value={cat}>
                    {cat.charAt(0).toUpperCase() + cat.slice(1)}
                  </option>
                ))}
              </select>
            </div>

            {/* Importance slider */}
            <div>
              <label className="text-2xs font-medium text-gray-500 dark:text-gray-400 block mb-1">
                {t('skillPanel.importance')}: {(editImportance * 100).toFixed(0)}%
              </label>
              <input
                type="range"
                min={0}
                max={100}
                value={editImportance * 100}
                onChange={(e) => setEditImportance(Number(e.target.value) / 100)}
                className="w-full h-1.5 accent-primary-600"
              />
            </div>

            {/* Keywords tag input */}
            <div>
              <label className="text-2xs font-medium text-gray-500 dark:text-gray-400 block mb-1">
                {t('skillPanel.keywords')}
              </label>
              <div className="flex flex-wrap gap-1 mb-1.5">
                {editKeywords.map((kw) => (
                  <span
                    key={kw}
                    className="inline-flex items-center gap-0.5 text-2xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300"
                  >
                    {kw}
                    <button
                      type="button"
                      onClick={() => setEditKeywords((prev) => prev.filter((k) => k !== kw))}
                      className="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
                    >
                      &times;
                    </button>
                  </span>
                ))}
              </div>
              <input
                type="text"
                value={keywordInput}
                onChange={(e) => setKeywordInput(e.target.value)}
                onKeyDown={(e) => {
                  if ((e.key === 'Enter' || e.key === ',') && keywordInput.trim()) {
                    e.preventDefault();
                    const newKw = keywordInput.trim().replace(/,+$/, '');
                    if (newKw && !editKeywords.includes(newKw)) {
                      setEditKeywords((prev) => [...prev, newKw]);
                    }
                    setKeywordInput('');
                  }
                }}
                placeholder={t('skillPanel.keywordsPlaceholder')}
                className={clsx(
                  'w-full px-2 py-1.5 rounded-md text-xs',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'text-gray-700 dark:text-gray-300',
                  'placeholder:text-gray-400 dark:placeholder:text-gray-500',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500'
                )}
              />
            </div>

            {/* Content textarea */}
            <div>
              <label className="text-2xs font-medium text-gray-500 dark:text-gray-400 block mb-1">
                {t('skillPanel.content')}
              </label>
              <textarea
                value={editContent}
                onChange={(e) => setEditContent(e.target.value)}
                rows={6}
                className={clsx(
                  'w-full px-3 py-2 rounded-md text-xs',
                  'bg-white dark:bg-gray-800',
                  'border border-gray-300 dark:border-gray-600',
                  'text-gray-700 dark:text-gray-300',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                  'resize-none'
                )}
              />
            </div>

            {/* Save/Cancel */}
            <div className="flex gap-2">
              <button
                onClick={handleSave}
                className={clsx(
                  'px-3 py-1.5 rounded-md text-xs font-medium',
                  'bg-primary-600 text-white hover:bg-primary-700'
                )}
              >
                {t('skillPanel.save')}
              </button>
              <button
                onClick={handleCancel}
                className={clsx(
                  'px-3 py-1.5 rounded-md text-xs font-medium',
                  'text-gray-600 dark:text-gray-400',
                  'hover:bg-gray-100 dark:hover:bg-gray-800'
                )}
              >
                {t('skillPanel.cancel')}
              </button>
            </div>
          </>
        ) : (
          <>
            {/* Read-only content */}
            <p className="text-sm text-gray-700 dark:text-gray-300 whitespace-pre-wrap leading-relaxed">
              {memory.content}
            </p>

            {/* Keywords */}
            {memory.keywords.length > 0 && (
              <div className="flex items-center gap-1 flex-wrap">
                <span className="text-2xs text-gray-500 dark:text-gray-400 shrink-0">
                  {t('skillPanel.keywords')}:
                </span>
                {memory.keywords.map((kw) => (
                  <span
                    key={kw}
                    className="text-2xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300"
                  >
                    {kw}
                  </span>
                ))}
              </div>
            )}

            {/* Timestamps */}
            <div className="text-2xs text-gray-400 dark:text-gray-500 space-y-0.5">
              <p>
                {t('skillPanel.updatedAt')}: {new Date(memory.updated_at).toLocaleString()}
              </p>
              <p>
                {t('skillPanel.accessCount')}: {memory.access_count}
              </p>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

export default MemoryDetail;
