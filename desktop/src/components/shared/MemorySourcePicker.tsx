/**
 * MemorySourcePicker
 *
 * Popover content for selecting Memory categories and individual memory items.
 * Tree view: Category → Memory Items with lazy-loading.
 * Supports backend semantic search that flattens results.
 */

import { useState, useCallback, useEffect, useMemo, useRef } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronDownIcon, ChevronRightIcon, MagnifyingGlassIcon } from '@radix-ui/react-icons';
import { useContextSourcesStore } from '../../store/contextSources';
import { useSettingsStore } from '../../store/settings';
import { MEMORY_CATEGORIES, type MemoryCategory, type MemoryScope } from '../../types/skillMemory';

/** Color classes for each memory category */
const CATEGORY_COLORS: Record<string, string> = {
  preference: 'bg-blue-400',
  convention: 'bg-yellow-400',
  pattern: 'bg-green-400',
  correction: 'bg-red-400',
  fact: 'bg-purple-400',
};

const SCOPE_LABELS: Record<MemoryScope, string> = {
  global: 'Global',
  project: 'Project',
  session: 'Session',
};

const SCOPE_STYLES: Record<MemoryScope, string> = {
  global: 'bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300',
  project: 'bg-sky-100 text-sky-700 dark:bg-sky-900/40 dark:text-sky-300',
  session: 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-300',
};

function inferScope(projectPath: string): MemoryScope {
  if (projectPath === '__global__') return 'global';
  if (projectPath.startsWith('__session__:')) return 'session';
  return 'project';
}

export function MemorySourcePicker() {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const {
    memorySelectionMode,
    selectedMemoryScopes,
    memorySessionId,
    selectedMemoryCategories,
    selectedMemoryIds,
    includedMemoryIds,
    excludedMemoryIds,
    availableMemoryStats,
    categoryMemories,
    isLoadingMemoryStats,
    isLoadingCategoryMemories,
    memoryPickerSearchQuery,
    memorySearchResults,
    isSearchingMemories,
    toggleMemoryScope,
    setMemorySelectionMode,
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
  const scopesKey = useMemo(() => selectedMemoryScopes.join('|'), [selectedMemoryScopes]);
  const effectiveExcludedMemoryIds = useMemo(
    () => (excludedMemoryIds.length > 0 ? excludedMemoryIds : selectedMemoryIds),
    [excludedMemoryIds, selectedMemoryIds],
  );
  const isMemoryChecked = useCallback(
    (memoryId: string) => {
      if (memorySelectionMode === 'only_selected') {
        return includedMemoryIds.includes(memoryId);
      }
      return !effectiveExcludedMemoryIds.includes(memoryId);
    },
    [memorySelectionMode, includedMemoryIds, effectiveExcludedMemoryIds],
  );

  // Load memory stats on mount
  useEffect(() => {
    if (workspacePath) {
      loadMemoryStats(workspacePath);
    }
  }, [workspacePath, scopesKey, memorySessionId, loadMemoryStats]);

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
    if (selectedMemoryCategories.length === 0) return 'checked';
    return selectedMemoryCategories.includes(category) ? 'checked' : 'unchecked';
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

  const totalCount = availableMemoryStats?.total_count ?? 0;
  const noMemoriesInSelectedScopes = !isLoadingMemoryStats && totalCount === 0;

  return (
    <div className="py-1">
      <div className="px-3 py-1.5 text-xs font-semibold text-gray-600 dark:text-gray-300 border-b border-gray-100 dark:border-gray-700">
        {t('contextSources.memoryPicker.title', { defaultValue: 'Memory Sources' })}
      </div>

      {/* Scope toggles */}
      <div className="px-2 py-1.5 border-b border-gray-100 dark:border-gray-700 flex items-center gap-1 flex-wrap">
        {(['global', 'project', 'session'] as MemoryScope[]).map((scope) => {
          const isSelected = selectedMemoryScopes.includes(scope);
          const disabled = scope === 'session' && !memorySessionId;
          return (
            <button
              key={scope}
              onClick={() => toggleMemoryScope(scope)}
              disabled={disabled}
              className={clsx(
                'px-2 py-0.5 rounded text-2xs font-medium transition-colors',
                isSelected
                  ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300'
                  : 'bg-gray-100 text-gray-500 dark:bg-gray-700 dark:text-gray-400',
                disabled && 'opacity-50 cursor-not-allowed',
              )}
            >
              {t(`contextSources.memoryPicker.scopes.${scope}` as const, {
                defaultValue: SCOPE_LABELS[scope],
              })}
            </button>
          );
        })}
      </div>

      {/* Selection mode */}
      <div className="px-2 py-1.5 border-b border-gray-100 dark:border-gray-700 flex items-center gap-1">
        <button
          onClick={() => setMemorySelectionMode('auto')}
          className={clsx(
            'px-2 py-0.5 rounded text-2xs font-medium transition-colors',
            memorySelectionMode === 'auto'
              ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300'
              : 'bg-gray-100 text-gray-500 dark:bg-gray-700 dark:text-gray-400',
          )}
        >
          {t('contextSources.memoryPicker.selectionModes.auto', { defaultValue: 'Auto + Exclude' })}
        </button>
        <button
          onClick={() => setMemorySelectionMode('only_selected')}
          className={clsx(
            'px-2 py-0.5 rounded text-2xs font-medium transition-colors',
            memorySelectionMode === 'only_selected'
              ? 'bg-purple-100 text-purple-700 dark:bg-purple-900/40 dark:text-purple-300'
              : 'bg-gray-100 text-gray-500 dark:bg-gray-700 dark:text-gray-400',
          )}
        >
          {t('contextSources.memoryPicker.selectionModes.onlySelected', { defaultValue: 'Only Selected' })}
        </button>
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
        {isLoadingMemoryStats ? (
          <div className="px-3 py-2 text-2xs text-gray-400">
            {t('contextSources.knowledgePicker.loading', { defaultValue: 'Loading...' })}
          </div>
        ) : noMemoriesInSelectedScopes ? (
          <div className="px-3 py-2 text-2xs text-gray-400">
            {t('contextSources.memoryPicker.noMemories', { defaultValue: 'No memories available' })}
          </div>
        ) : isSearchMode ? (
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
                  checked={isMemoryChecked(entry.id)}
                  onChange={() => toggleMemoryItem(entry.id)}
                  className="w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-purple-600 focus:ring-purple-500"
                />
                <span
                  className={clsx(
                    'w-1.5 h-1.5 rounded-full flex-shrink-0',
                    CATEGORY_COLORS[entry.category] || 'bg-gray-400',
                  )}
                />
                <span
                  className={clsx(
                    'text-2xs px-1 py-0.5 rounded flex-shrink-0',
                    SCOPE_STYLES[inferScope(entry.project_path)],
                  )}
                >
                  {SCOPE_LABELS[inferScope(entry.project_path)]}
                </span>
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
                          checked={isMemoryChecked(entry.id)}
                          onChange={() => toggleMemoryItem(entry.id)}
                          className="w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-purple-600 focus:ring-purple-500"
                        />
                        <span
                          className={clsx(
                            'text-2xs px-1 py-0.5 rounded flex-shrink-0',
                            SCOPE_STYLES[inferScope(entry.project_path)],
                          )}
                        >
                          {SCOPE_LABELS[inferScope(entry.project_path)]}
                        </span>
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
