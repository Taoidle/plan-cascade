/**
 * AddMemoryForm Component
 *
 * Form for manually adding a new memory entry.
 * Reuses styling patterns from MemoryDetail edit mode.
 */

import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { MemoryCategory } from '../../types/skillMemory';
import { MEMORY_CATEGORIES } from '../../types/skillMemory';

interface AddMemoryFormProps {
  onSave: (category: MemoryCategory, content: string, keywords: string[], importance: number) => void;
  onCancel: () => void;
}

export function AddMemoryForm({ onSave, onCancel }: AddMemoryFormProps) {
  const { t } = useTranslation('simpleMode');
  const [category, setCategory] = useState<MemoryCategory>('fact');
  const [content, setContent] = useState('');
  const [keywords, setKeywords] = useState<string[]>([]);
  const [keywordInput, setKeywordInput] = useState('');
  const [importance, setImportance] = useState(0.5);

  const handleSave = useCallback(() => {
    if (!content.trim()) return;
    onSave(category, content.trim(), keywords, importance);
  }, [category, content, keywords, importance, onSave]);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
        <span className="text-sm font-semibold text-gray-900 dark:text-white">{t('skillPanel.addMemory')}</span>
      </div>

      {/* Form */}
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {/* Category selector */}
        <div>
          <label className="text-2xs font-medium text-gray-500 dark:text-gray-400 block mb-1">
            {t('skillPanel.category')}
          </label>
          <select
            value={category}
            onChange={(e) => setCategory(e.target.value as MemoryCategory)}
            className={clsx(
              'w-full px-2 py-1.5 rounded-md text-xs',
              'bg-white dark:bg-gray-800',
              'border border-gray-300 dark:border-gray-600',
              'text-gray-700 dark:text-gray-300',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
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
            {t('skillPanel.importance')}: {(importance * 100).toFixed(0)}%
          </label>
          <input
            type="range"
            min={0}
            max={100}
            value={importance * 100}
            onChange={(e) => setImportance(Number(e.target.value) / 100)}
            className="w-full h-1.5 accent-primary-600"
          />
        </div>

        {/* Keywords tag input */}
        <div>
          <label className="text-2xs font-medium text-gray-500 dark:text-gray-400 block mb-1">
            {t('skillPanel.keywords')}
          </label>
          <div className="flex flex-wrap gap-1 mb-1.5">
            {keywords.map((kw) => (
              <span
                key={kw}
                className="inline-flex items-center gap-0.5 text-2xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300"
              >
                {kw}
                <button
                  type="button"
                  onClick={() => setKeywords((prev) => prev.filter((k) => k !== kw))}
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
                if (newKw && !keywords.includes(newKw)) {
                  setKeywords((prev) => [...prev, newKw]);
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
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
            )}
          />
        </div>

        {/* Content textarea */}
        <div>
          <label className="text-2xs font-medium text-gray-500 dark:text-gray-400 block mb-1">
            {t('skillPanel.content')}
          </label>
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            rows={6}
            className={clsx(
              'w-full px-3 py-2 rounded-md text-xs',
              'bg-white dark:bg-gray-800',
              'border border-gray-300 dark:border-gray-600',
              'text-gray-700 dark:text-gray-300',
              'focus:outline-none focus:ring-2 focus:ring-primary-500',
              'resize-none',
            )}
          />
        </div>

        {/* Save/Cancel */}
        <div className="flex gap-2">
          <button
            onClick={handleSave}
            disabled={!content.trim()}
            className={clsx(
              'px-3 py-1.5 rounded-md text-xs font-medium',
              content.trim()
                ? 'bg-primary-600 text-white hover:bg-primary-700'
                : 'bg-gray-300 dark:bg-gray-600 text-gray-500 dark:text-gray-400 cursor-not-allowed',
            )}
          >
            {t('skillPanel.save')}
          </button>
          <button
            onClick={onCancel}
            className={clsx(
              'px-3 py-1.5 rounded-md text-xs font-medium',
              'text-gray-600 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
          >
            {t('skillPanel.cancel')}
          </button>
        </div>
      </div>
    </div>
  );
}

export default AddMemoryForm;
