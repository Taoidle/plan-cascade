/**
 * MemorySourcePicker
 *
 * Popover content for selecting Memory categories and individual memory items.
 * Tree view: Category → Memory Items with lazy-loading.
 * Supports backend semantic search that flattens results.
 */

import { useState, useCallback, useEffect, useRef } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronDownIcon, ChevronRightIcon, MagnifyingGlassIcon } from '@radix-ui/react-icons';
import { useContextSourcesStore } from '../../store/contextSources';
import { useSettingsStore } from '../../store/settings';
import { MEMORY_CATEGORIES, type MemoryCategory } from '../../types/skillMemory';

/** Color classes for each memory category */
const CATEGORY_COLORS: Record<string, string> = {
  preference: 'bg-blue-400',
  convention: 'bg-yellow-400',
  pattern: 'bg-green-400',
  correction: 'bg-red-400',
  fact: 'bg-purple-400',
};

export function MemorySourcePicker() {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const {
    selectedMemoryCategories,
    selectedMemoryIds,
    availableMemoryStats,
    categoryMemories,
    isLoadingMemoryStats,
    isLoadingCategoryMemories,
    memoryPickerSearchQuery,
    memorySearchResults,
    isSearchingMemories,
    toggleMemoryCategory,
    toggleMemoryItem,
    loadMemoryStats,
    loadCategoryMemories,
    searchMemoriesForPicker,
    clearMemorySearch,
  } = useContextSourcesStore();

  const [expandedCategories, setExpandedCategories] = useState<Set<string>>(new Set());
  const [localSearchQuery, setLocalSearchQuery] = useState(memoryPickerSearchQuery);
  const searchTimerRef = useRef<ReturnType<typeof setTimeout>>();

  // Load memory stats on mount
  useEffect(() => {
    if (workspacePath && !availableMemoryStats && !isLoadingMemoryStats) {
      loadMemoryStats(workspacePath);
    }
  }, [workspacePath, availableMemoryStats, isLoadingMemoryStats, loadMemoryStats]);

  // Debounced search
  useEffect(() => {
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current);

    if (!localSearchQuery.trim()) {
      clearMemorySearch();
      return;
    }

    searchTimerRef.current = setTimeout(() => {
      if (workspacePath && localSearchQuery.trim()) {
        searchMemoriesForPicker(workspacePath, localSearchQuery.trim());
      }
    }, 300);

    return () => {
      if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    };
  }, [localSearchQuery, workspacePath, searchMemoriesForPicker, clearMemorySearch]);

  const toggleExpand = useCallback(
    (category: string) => {
      setExpandedCategories((prev) => {
        const next = new Set(prev);
        if (next.has(category)) {
          next.delete(category);
        } else {
          next.add(category);
          // Lazy-load memories for this category
          if (!categoryMemories[category] && workspacePath) {
            loadCategoryMemories(workspacePath, category);
          }
        }
        return next;
      });
    },
    [categoryMemories, workspacePath, loadCategoryMemories],
  );

  const getCategoryCheckState = (category: string): 'checked' | 'unchecked' | 'indeterminate' => {
    const isCatSelected = selectedMemoryCategories.includes(category);
    const memories = categoryMemories[category] || [];

    if (memories.length === 0) {
      return isCatSelected ? 'checked' : 'unchecked';
    }

    const memIds = memories.map((m) => m.id);
    const selectedCount = memIds.filter((id) => selectedMemoryIds.includes(id)).length;

    if (selectedCount === 0 && !isCatSelected) return 'unchecked';
    if (selectedCount === memIds.length) return 'checked';
    return 'indeterminate';
  };

  const categoryLabel = (cat: MemoryCategory) => {
    const key = `contextSources.memoryPicker.categories.${cat}` as const;
    const defaults: Record<MemoryCategory, string> = {
      preference: 'Preferences',
      convention: 'Conventions',
      pattern: 'Patterns',
      correction: 'Corrections',
      fact: 'Facts',
    };
    return t(key, { defaultValue: defaults[cat] });
  };

  const isSearchMode = !!localSearchQuery.trim();

  if (isLoadingMemoryStats) {
    return (
      <div className="p-3 text-xs text-gray-500 dark:text-gray-400">
        {t('contextSources.knowledgePicker.loading', { defaultValue: 'Loading...' })}
      </div>
    );
  }

  const totalCount = availableMemoryStats?.total_count ?? 0;
  if (totalCount === 0 && !isLoadingMemoryStats) {
    return (
      <div className="p-3 text-xs text-gray-500 dark:text-gray-400">
        {t('contextSources.memoryPicker.noMemories', { defaultValue: 'No memories available' })}
      </div>
    );
  }

  return (
    <div className="py-1">
      <div className="px-3 py-1.5 text-xs font-semibold text-gray-600 dark:text-gray-300 border-b border-gray-100 dark:border-gray-700">
        {t('contextSources.memoryPicker.title', { defaultValue: 'Memory Sources' })}
      </div>

      {/* Search input */}
      <div className="px-2 py-1.5 border-b border-gray-100 dark:border-gray-700">
        <div className="relative">
          <MagnifyingGlassIcon className="absolute left-2 top-1/2 -translate-y-1/2 w-3 h-3 text-gray-400" />
          <input
            type="text"
            value={localSearchQuery}
            onChange={(e) => setLocalSearchQuery(e.target.value)}
            placeholder={t('contextSources.memoryPicker.searchPlaceholder', {
              defaultValue: 'Search memories...',
            })}
            className={clsx(
              'w-full pl-6 pr-2 py-1 text-2xs rounded',
              'bg-gray-50 dark:bg-gray-750 border border-gray-200 dark:border-gray-600',
              'text-gray-700 dark:text-gray-300 placeholder-gray-400',
              'focus:outline-none focus:ring-1 focus:ring-purple-400',
            )}
          />
        </div>
      </div>

      <div className="max-h-64 overflow-y-auto">
        {isSearchMode ? (
          // Search results (flat list)
          <>
            {isSearchingMemories && (
              <div className="px-3 py-2 text-2xs text-gray-400">
                {t('contextSources.knowledgePicker.loading', { defaultValue: 'Loading...' })}
              </div>
            )}
            {!isSearchingMemories && memorySearchResults && memorySearchResults.length === 0 && (
              <div className="px-3 py-2 text-2xs text-gray-400">
                {t('contextSources.memoryPicker.noResults', { defaultValue: 'No matching memories' })}
              </div>
            )}
            {memorySearchResults?.map((entry) => (
              <div
                key={entry.id}
                className={clsx(
                  'flex items-center gap-1.5 px-2 py-1.5',
                  'hover:bg-gray-50 dark:hover:bg-gray-750',
                  'cursor-pointer select-none',
                )}
                onClick={() => toggleMemoryItem(entry.id)}
              >
                <input
                  type="checkbox"
                  checked={selectedMemoryIds.includes(entry.id)}
                  onChange={() => toggleMemoryItem(entry.id)}
                  className="w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-purple-600 focus:ring-purple-500"
                />
                <span
                  className={clsx(
                    'w-1.5 h-1.5 rounded-full flex-shrink-0',
                    CATEGORY_COLORS[entry.category] || 'bg-gray-400',
                  )}
                />
                <span className="flex-1 text-2xs text-gray-600 dark:text-gray-400 truncate">
                  {entry.content.slice(0, 80)}
                  {entry.content.length > 80 ? '...' : ''}
                </span>
              </div>
            ))}
          </>
        ) : (
          // Category tree view
          MEMORY_CATEGORIES.map((category) => {
            const isExpanded = expandedCategories.has(category);
            const checkState = getCategoryCheckState(category);
            const memories = categoryMemories[category] || [];
            const loading = isLoadingCategoryMemories[category];
            const count = availableMemoryStats?.category_counts[category] ?? 0;

            if (count === 0) return null;

            return (
              <div key={category}>
                {/* Category row */}
                <div
                  className={clsx(
                    'flex items-center gap-1.5 px-2 py-1.5',
                    'hover:bg-gray-50 dark:hover:bg-gray-750',
                    'cursor-pointer select-none',
                  )}
                >
                  <button
                    onClick={() => toggleExpand(category)}
                    className="p-0.5 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
                  >
                    {isExpanded ? <ChevronDownIcon className="w-3 h-3" /> : <ChevronRightIcon className="w-3 h-3" />}
                  </button>

                  <input
                    type="checkbox"
                    checked={checkState === 'checked'}
                    ref={(el) => {
                      if (el) el.indeterminate = checkState === 'indeterminate';
                    }}
                    onChange={() => toggleMemoryCategory(category)}
                    className="w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-purple-600 focus:ring-purple-500"
                  />

                  <span
                    className={clsx('w-2 h-2 rounded-full flex-shrink-0', CATEGORY_COLORS[category] || 'bg-gray-400')}
                  />

                  <span
                    className="flex-1 text-xs text-gray-700 dark:text-gray-300 cursor-pointer"
                    onClick={() => toggleExpand(category)}
                  >
                    {categoryLabel(category)}
                  </span>

                  <span className="text-2xs text-gray-400 dark:text-gray-500">{count}</span>
                </div>

                {/* Memory items (expanded) */}
                {isExpanded && (
                  <div className="ml-5 border-l border-gray-100 dark:border-gray-700">
                    {loading && (
                      <div className="px-3 py-1 text-2xs text-gray-400">
                        {t('contextSources.knowledgePicker.loading', { defaultValue: 'Loading...' })}
                      </div>
                    )}
                    {!loading && memories.length === 0 && (
                      <div className="px-3 py-1 text-2xs text-gray-400">
                        {t('contextSources.memoryPicker.noMemories', { defaultValue: 'No memories available' })}
                      </div>
                    )}
                    {memories.map((entry) => (
                      <div
                        key={entry.id}
                        className={clsx(
                          'flex items-center gap-1.5 px-2 py-1',
                          'hover:bg-gray-50 dark:hover:bg-gray-750',
                          'cursor-pointer select-none',
                        )}
                        onClick={() => toggleMemoryItem(entry.id)}
                      >
                        <input
                          type="checkbox"
                          checked={selectedMemoryIds.includes(entry.id)}
                          onChange={() => toggleMemoryItem(entry.id)}
                          className="w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-purple-600 focus:ring-purple-500"
                        />
                        <span className="flex-1 text-2xs text-gray-600 dark:text-gray-400 truncate">
                          {entry.content.slice(0, 60)}
                          {entry.content.length > 60 ? '...' : ''}
                        </span>
                        {entry.importance >= 0.7 && (
                          <span className="text-2xs text-amber-500" title="High importance">
                            &#9733;
                          </span>
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
