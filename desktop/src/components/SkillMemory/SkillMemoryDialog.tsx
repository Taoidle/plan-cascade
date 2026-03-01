/**
 * SkillMemoryDialog Component
 *
 * Full management modal with two tabs:
 * - Skills: source filter, search, grouped skill list with toggles
 * - Memory: category filter, search, paginated list with importance bars, edit/delete
 *
 * Uses Radix UI Dialog and Tabs primitives.
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import * as Dialog from '@radix-ui/react-dialog';
import * as Tabs from '@radix-ui/react-tabs';
import { Cross2Icon, MagnifyingGlassIcon, PlusIcon, ReloadIcon, TrashIcon } from '@radix-ui/react-icons';
import { useSkillMemoryStore, type SkillSourceFilter, type MemoryCategoryFilter } from '../../store/skillMemory';
import { useSettingsStore } from '../../store/settings';
import { useExecutionStore } from '../../store/execution';
import { SkillRow } from '../SimpleMode/SkillRow';
import { SkillDetail } from './SkillDetail';
import { MemoryDetail } from './MemoryDetail';
import { AddMemoryForm } from './AddMemoryForm';
import { CategoryBadge } from './CategoryBadge';
import { ImportanceBar } from './ImportanceBar';
import { EmptyState } from './EmptyState';
import { debounce } from '../Projects/utils';
import type { SkillSummary, MemoryEntry, MemoryCategory, MemoryScope } from '../../types/skillMemory';
import { MEMORY_CATEGORIES } from '../../types/skillMemory';

// ============================================================================
// Source filter options
// ============================================================================

function sourceLabelFallback(source: SkillSourceFilter): string {
  switch (source) {
    case 'all':
      return 'All';
    case 'builtin':
      return 'Built-in';
    case 'external':
      return 'External';
    case 'project_local':
      return 'Project';
    case 'generated':
      return 'Generated';
    case 'user':
      return 'User';
    default:
      return source;
  }
}

function sourceGroupFallback(source: string): string {
  switch (source) {
    case 'builtin':
      return 'Built-in';
    case 'external':
      return 'External';
    case 'project_local':
      return 'Project';
    case 'generated':
      return 'Generated';
    case 'user':
      return 'User';
    default:
      return source.replace(/_/g, ' ');
  }
}

function memoryCategoryFallback(category: MemoryCategory | string): string {
  switch (category) {
    case 'preference':
      return 'Preference';
    case 'convention':
      return 'Convention';
    case 'pattern':
      return 'Pattern';
    case 'correction':
      return 'Correction';
    case 'fact':
      return 'Fact';
    default:
      return category;
  }
}

function memoryScopeFallback(scope: MemoryScope): string {
  switch (scope) {
    case 'project':
      return 'Project';
    case 'global':
      return 'Global';
    case 'session':
      return 'Session';
    default:
      return scope;
  }
}

// ============================================================================
// SkillsTab
// ============================================================================

function SkillsTab() {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const skills = useSkillMemoryStore((s) => s.skills);
  const skillsLoading = useSkillMemoryStore((s) => s.skillsLoading);
  const skillSearchQuery = useSkillMemoryStore((s) => s.skillSearchQuery);
  const skillSourceFilter = useSkillMemoryStore((s) => s.skillSourceFilter);
  const setSkillSearchQuery = useSkillMemoryStore((s) => s.setSkillSearchQuery);
  const setSkillSourceFilter = useSkillMemoryStore((s) => s.setSkillSourceFilter);
  const toggleSkill = useSkillMemoryStore((s) => s.toggleSkill);
  const refreshSkillIndex = useSkillMemoryStore((s) => s.refreshSkillIndex);
  const loadSkillDetail = useSkillMemoryStore((s) => s.loadSkillDetail);
  const skillDetail = useSkillMemoryStore((s) => s.skillDetail);

  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null);
  const sourceFilters: { value: SkillSourceFilter; label: string }[] = useMemo(
    () => [
      { value: 'all', label: t('skillPanel.sourceFilters.all', { defaultValue: sourceLabelFallback('all') }) },
      {
        value: 'builtin',
        label: t('skillPanel.sourceFilters.builtin', { defaultValue: sourceLabelFallback('builtin') }),
      },
      {
        value: 'external',
        label: t('skillPanel.sourceFilters.external', { defaultValue: sourceLabelFallback('external') }),
      },
      {
        value: 'project_local',
        label: t('skillPanel.sourceFilters.project_local', { defaultValue: sourceLabelFallback('project_local') }),
      },
      {
        value: 'generated',
        label: t('skillPanel.sourceFilters.generated', { defaultValue: sourceLabelFallback('generated') }),
      },
      { value: 'user', label: t('skillPanel.sourceFilters.user', { defaultValue: sourceLabelFallback('user') }) },
    ],
    [t],
  );

  const getSourceGroupLabel = useCallback(
    (sourceType: string) =>
      t(`skillPanel.sourceGroups.${sourceType}`, {
        defaultValue: sourceGroupFallback(sourceType),
      }),
    [t],
  );

  // Filter skills by source and search query
  const filteredSkills = useMemo(() => {
    let result = skills;

    // Apply source filter
    if (skillSourceFilter !== 'all') {
      result = result.filter((s) => s.source.type === skillSourceFilter);
    }

    // Apply search query
    if (skillSearchQuery.trim()) {
      const q = skillSearchQuery.toLowerCase();
      result = result.filter(
        (s) =>
          s.name.toLowerCase().includes(q) ||
          s.description.toLowerCase().includes(q) ||
          s.tags.some((tag) => tag.toLowerCase().includes(q)),
      );
    }

    return result;
  }, [skills, skillSourceFilter, skillSearchQuery]);

  // Group by source type
  const groupedSkills = useMemo(() => {
    const groups: Record<string, SkillSummary[]> = {};
    for (const skill of filteredSkills) {
      const key = skill.source.type;
      if (!groups[key]) groups[key] = [];
      groups[key].push(skill);
    }
    return groups;
  }, [filteredSkills]);

  const handleToggle = useCallback(
    (id: string, enabled: boolean) => {
      toggleSkill(id, enabled);
    },
    [toggleSkill],
  );

  const handleSkillClick = useCallback(
    (skill: SkillSummary) => {
      setSelectedSkillId(skill.id);
      if (workspacePath) {
        loadSkillDetail(workspacePath, skill.id);
      }
    },
    [workspacePath, loadSkillDetail],
  );

  const handleRefresh = useCallback(() => {
    if (workspacePath) {
      refreshSkillIndex(workspacePath);
    }
  }, [workspacePath, refreshSkillIndex]);

  // If a skill detail is open, show it
  if (selectedSkillId && skillDetail) {
    return <SkillDetail skill={skillDetail} onClose={() => setSelectedSkillId(null)} />;
  }

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar: search + filter + refresh */}
      <div className="p-3 border-b border-gray-200 dark:border-gray-700 space-y-2">
        {/* Search */}
        <div className="relative">
          <MagnifyingGlassIcon className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
          <input
            type="text"
            value={skillSearchQuery}
            onChange={(e) => setSkillSearchQuery(e.target.value)}
            placeholder={t('skillPanel.searchSkills')}
            className={clsx(
              'w-full pl-8 pr-3 py-1.5 rounded-md text-xs',
              'bg-gray-50 dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'text-gray-700 dark:text-gray-300',
              'placeholder:text-gray-400 dark:placeholder:text-gray-500',
              'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent',
            )}
          />
        </div>

        {/* Source filter tabs + refresh */}
        <div className="flex items-center gap-1 flex-wrap">
          {sourceFilters.map((filter) => (
            <button
              key={filter.value}
              onClick={() => setSkillSourceFilter(filter.value)}
              className={clsx(
                'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                skillSourceFilter === filter.value
                  ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                  : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
              )}
            >
              {filter.label}
            </button>
          ))}
          <button
            onClick={handleRefresh}
            className={clsx(
              'ml-auto p-1 rounded-md transition-colors',
              'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
            title={t('skillPanel.refresh')}
          >
            <ReloadIcon className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Skills list */}
      <div className="flex-1 overflow-y-auto p-2">
        {skillsLoading && filteredSkills.length === 0 ? (
          <div className="flex items-center justify-center py-8">
            <span className="text-xs text-gray-400">{t('skillPanel.loading')}</span>
          </div>
        ) : filteredSkills.length === 0 ? (
          <EmptyState title={t('skillPanel.noSkillsFound')} description={t('skillPanel.noSkillsFoundHint')} />
        ) : (
          <div className="space-y-3">
            {Object.entries(groupedSkills).map(([sourceType, groupSkills]) => (
              <div key={sourceType}>
                <div className="px-2 py-1">
                  <span className="text-2xs font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
                    {getSourceGroupLabel(sourceType)}
                  </span>
                  <span className="text-2xs text-gray-400 dark:text-gray-500 ml-1">({groupSkills.length})</span>
                </div>
                {groupSkills.map((skill) => (
                  <SkillRow key={skill.id} skill={skill} onToggle={handleToggle} onClick={handleSkillClick} />
                ))}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// MemoryTab
// ============================================================================

function MemoryTab() {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const taskId = useExecutionStore((s) => s.taskId);
  const standaloneSessionId = useExecutionStore((s) => s.standaloneSessionId);
  const foregroundOriginSessionId = useExecutionStore((s) => s.foregroundOriginSessionId);
  const memories = useSkillMemoryStore((s) => s.memories);
  const memoriesLoading = useSkillMemoryStore((s) => s.memoriesLoading);
  const memorySearchQuery = useSkillMemoryStore((s) => s.memorySearchQuery);
  const memoryCategoryFilter = useSkillMemoryStore((s) => s.memoryCategoryFilter);
  const memoryScope = useSkillMemoryStore((s) => s.memoryScope);
  const memoryHasMore = useSkillMemoryStore((s) => s.memoryHasMore);
  const setMemorySearchQuery = useSkillMemoryStore((s) => s.setMemorySearchQuery);
  const setMemoryCategoryFilter = useSkillMemoryStore((s) => s.setMemoryCategoryFilter);
  const setMemoryScope = useSkillMemoryStore((s) => s.setMemoryScope);
  const setMemorySessionId = useSkillMemoryStore((s) => s.setMemorySessionId);
  const loadMemories = useSkillMemoryStore((s) => s.loadMemories);
  const loadMoreMemories = useSkillMemoryStore((s) => s.loadMoreMemories);
  const searchMemories = useSkillMemoryStore((s) => s.searchMemories);
  const updateMemory = useSkillMemoryStore((s) => s.updateMemory);
  const deleteMemory = useSkillMemoryStore((s) => s.deleteMemory);
  const addMemory = useSkillMemoryStore((s) => s.addMemory);
  const clearMemories = useSkillMemoryStore((s) => s.clearMemories);
  const memoryStats = useSkillMemoryStore((s) => s.memoryStats);
  const loadMemoryStats = useSkillMemoryStore((s) => s.loadMemoryStats);

  const [selectedMemory, setSelectedMemory] = useState<MemoryEntry | null>(null);
  const [showAddForm, setShowAddForm] = useState(false);
  const activeSessionId = useMemo(() => {
    if (foregroundOriginSessionId?.trim()) return foregroundOriginSessionId.trim();
    if (taskId?.trim()) return `claude:${taskId.trim()}`;
    if (standaloneSessionId?.trim()) return `standalone:${standaloneSessionId.trim()}`;
    return null;
  }, [foregroundOriginSessionId, taskId, standaloneSessionId]);

  const memoryScopeOptions = useMemo(
    () =>
      (['project', 'global', 'session'] as MemoryScope[]).map((scope) => ({
        value: scope,
        label: t(`skillPanel.memoryScopes.${scope}`, { defaultValue: memoryScopeFallback(scope) }),
      })),
    [t],
  );
  const getCategoryLabel = useCallback(
    (category: MemoryCategory | string) =>
      t(`skillPanel.memoryCategories.${category}`, {
        defaultValue: memoryCategoryFallback(category),
      }),
    [t],
  );

  useEffect(() => {
    setMemorySessionId(activeSessionId);
  }, [activeSessionId, setMemorySessionId]);

  useEffect(() => {
    if (memoryScope === 'session' && !activeSessionId) {
      setMemoryScope('project');
    }
  }, [memoryScope, activeSessionId, setMemoryScope]);

  // Reload when category filter changes
  useEffect(() => {
    if (workspacePath) {
      if (memorySearchQuery.trim()) {
        searchMemories(workspacePath, memorySearchQuery);
      } else {
        loadMemories(workspacePath);
      }
    }
  }, [memoryCategoryFilter, memoryScope, activeSessionId]); // eslint-disable-line react-hooks/exhaustive-deps

  const debouncedSearch = useMemo(
    () =>
      debounce((query: string) => {
        if (workspacePath) {
          if (query.trim()) {
            searchMemories(workspacePath, query);
          } else {
            loadMemories(workspacePath);
          }
        }
      }, 300),
    [workspacePath, searchMemories, loadMemories],
  );

  const handleSearch = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const query = e.target.value;
      setMemorySearchQuery(query);
      debouncedSearch(query);
    },
    [setMemorySearchQuery, debouncedSearch],
  );

  const handleLoadMore = useCallback(() => {
    if (workspacePath) {
      loadMoreMemories(workspacePath);
    }
  }, [workspacePath, loadMoreMemories]);

  const handleCategoryFilter = useCallback(
    (filter: MemoryCategoryFilter) => {
      setMemoryCategoryFilter(filter);
    },
    [setMemoryCategoryFilter],
  );

  const handleScopeFilter = useCallback(
    (scope: MemoryScope) => {
      if (scope === 'session' && !activeSessionId) return;
      setMemoryScope(scope);
    },
    [activeSessionId, setMemoryScope],
  );

  const handleAddMemory = useCallback(
    async (category: MemoryCategory, content: string, keywords: string[], importance: number) => {
      if (workspacePath) {
        await addMemory(workspacePath, category, content, keywords, importance);
        setShowAddForm(false);
        loadMemoryStats(workspacePath);
      }
    },
    [workspacePath, addMemory, loadMemoryStats],
  );

  const handleClearAll = useCallback(() => {
    const confirmKey =
      memoryScope === 'global'
        ? 'skillPanel.clearAllConfirmGlobal'
        : memoryScope === 'session'
          ? 'skillPanel.clearAllConfirmSession'
          : 'skillPanel.clearAllConfirm';
    if (workspacePath && window.confirm(t(confirmKey, { defaultValue: t('skillPanel.clearAllConfirm') }))) {
      clearMemories(workspacePath);
    }
  }, [workspacePath, clearMemories, t, memoryScope]);

  // If add form is open, show it
  if (showAddForm) {
    return <AddMemoryForm onSave={handleAddMemory} onCancel={() => setShowAddForm(false)} />;
  }

  // If a memory is selected, show detail view
  if (selectedMemory) {
    return (
      <MemoryDetail
        memory={selectedMemory}
        onClose={() => setSelectedMemory(null)}
        onUpdate={(id, updates) => {
          updateMemory(id, updates);
          setSelectedMemory(null);
        }}
        onDelete={(id) => {
          deleteMemory(id);
          setSelectedMemory(null);
        }}
      />
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar: scope + search + category filter */}
      <div className="p-3 border-b border-gray-200 dark:border-gray-700 space-y-2">
        {/* Scope filter */}
        <div className="flex items-center gap-1 flex-wrap">
          {memoryScopeOptions.map((option) => {
            const isDisabled = option.value === 'session' && !activeSessionId;
            return (
              <button
                key={option.value}
                onClick={() => handleScopeFilter(option.value)}
                disabled={isDisabled}
                className={clsx(
                  'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                  memoryScope === option.value
                    ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                    : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
                  isDisabled && 'opacity-50 cursor-not-allowed hover:bg-transparent dark:hover:bg-transparent',
                )}
              >
                {option.label}
              </button>
            );
          })}
        </div>

        {/* Search */}
        <div className="relative">
          <MagnifyingGlassIcon className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
          <input
            type="text"
            value={memorySearchQuery}
            onChange={handleSearch}
            placeholder={t('skillPanel.searchMemories')}
            className={clsx(
              'w-full pl-8 pr-3 py-1.5 rounded-md text-xs',
              'bg-gray-50 dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'text-gray-700 dark:text-gray-300',
              'placeholder:text-gray-400 dark:placeholder:text-gray-500',
              'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent',
            )}
          />
        </div>

        {/* Category filter + action buttons */}
        <div className="flex items-center gap-1 flex-wrap">
          <button
            onClick={() => handleCategoryFilter('all')}
            className={clsx(
              'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
              memoryCategoryFilter === 'all'
                ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
          >
            {t('skillPanel.filterAll')}
          </button>
          {MEMORY_CATEGORIES.map((cat) => (
            <button
              key={cat}
              onClick={() => handleCategoryFilter(cat)}
              className={clsx(
                'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                memoryCategoryFilter === cat
                  ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                  : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
              )}
            >
              {getCategoryLabel(cat)}
            </button>
          ))}
          <button
            onClick={() => setShowAddForm(true)}
            className={clsx(
              'ml-auto p-1 rounded-md transition-colors',
              'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
            title={t('skillPanel.addMemory')}
          >
            <PlusIcon className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={handleClearAll}
            disabled={memories.length === 0}
            className={clsx(
              'p-1 rounded-md transition-colors',
              memories.length === 0
                ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                : 'text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20',
            )}
            title={t('skillPanel.clearAll')}
          >
            <TrashIcon className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Stats bar */}
      {memoryStats && (
        <div className="px-3 py-1.5 border-b border-gray-200 dark:border-gray-700 flex items-center gap-2 text-2xs text-gray-500 dark:text-gray-400">
          <span>{t('skillPanel.totalMemories', { count: memoryStats.total_count })}</span>
          <span className="text-gray-300 dark:text-gray-600">|</span>
          <span>{t('skillPanel.avgImportance', { pct: (memoryStats.avg_importance * 100).toFixed(0) })}</span>
          {Object.entries(memoryStats.category_counts).map(([cat, count]) => (
            <span key={cat} className="text-gray-400 dark:text-gray-500">
              {getCategoryLabel(cat)}: {count as number}
            </span>
          ))}
        </div>
      )}

      {/* Memory list */}
      <div className="flex-1 overflow-y-auto">
        {memoriesLoading && memories.length === 0 ? (
          <div className="flex items-center justify-center py-8">
            <span className="text-xs text-gray-400">{t('skillPanel.loading')}</span>
          </div>
        ) : memories.length === 0 ? (
          <EmptyState
            title={t('skillPanel.noMemoriesFound')}
            description={t('skillPanel.noMemoriesFoundHint')}
            action={{ label: t('skillPanel.addMemory'), onClick: () => setShowAddForm(true) }}
          />
        ) : (
          <div className="divide-y divide-gray-100 dark:divide-gray-800">
            {memories.map((memory) => (
              <button
                key={memory.id}
                data-testid={`memory-list-item-${memory.id}`}
                onClick={() => setSelectedMemory(memory)}
                className={clsx(
                  'w-full text-left px-4 py-3 transition-colors',
                  'hover:bg-gray-50 dark:hover:bg-gray-800/50',
                )}
              >
                <div className="flex items-center gap-2 mb-1">
                  <CategoryBadge category={memory.category} compact />
                  <ImportanceBar value={memory.importance} className="flex-1 max-w-[6rem]" />
                  <span className="text-2xs text-gray-400 dark:text-gray-500 shrink-0">
                    {new Date(memory.updated_at).toLocaleDateString()}
                  </span>
                </div>
                <p className="text-xs text-gray-700 dark:text-gray-300 line-clamp-2">{memory.content}</p>
                {memory.keywords.length > 0 && (
                  <div className="flex gap-1 mt-1 flex-wrap">
                    {memory.keywords.slice(0, 3).map((kw) => (
                      <span
                        key={kw}
                        className="text-2xs px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400"
                      >
                        {kw}
                      </span>
                    ))}
                    {memory.keywords.length > 3 && (
                      <span className="text-2xs text-gray-400 dark:text-gray-500">+{memory.keywords.length - 3}</span>
                    )}
                  </div>
                )}
              </button>
            ))}
          </div>
        )}

        {/* Load more */}
        {memoryHasMore && memories.length > 0 && (
          <div className="p-3 text-center">
            <button
              onClick={handleLoadMore}
              className={clsx(
                'px-4 py-1.5 rounded-md text-xs font-medium transition-colors',
                'text-primary-600 dark:text-primary-400',
                'hover:bg-primary-50 dark:hover:bg-primary-900/20',
              )}
            >
              {t('skillPanel.loadMore')}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================================
// SkillMemoryDialog
// ============================================================================

export function SkillMemoryDialog() {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const dialogOpen = useSkillMemoryStore((s) => s.dialogOpen);
  const activeTab = useSkillMemoryStore((s) => s.activeTab);
  const closeDialog = useSkillMemoryStore((s) => s.closeDialog);
  const setActiveTab = useSkillMemoryStore((s) => s.setActiveTab);
  const loadSkills = useSkillMemoryStore((s) => s.loadSkills);
  const loadMemories = useSkillMemoryStore((s) => s.loadMemories);
  const loadMemoryStats = useSkillMemoryStore((s) => s.loadMemoryStats);
  const runMaintenance = useSkillMemoryStore((s) => s.runMaintenance);

  // Load data when dialog opens
  useEffect(() => {
    if (dialogOpen && workspacePath) {
      loadSkills(workspacePath);
      loadMemories(workspacePath);
      loadMemoryStats(workspacePath);
      runMaintenance(workspacePath);
    }
  }, [dialogOpen, workspacePath, loadSkills, loadMemories, loadMemoryStats, runMaintenance]);

  return (
    <Dialog.Root open={dialogOpen} onOpenChange={(open) => !open && closeDialog()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 z-50 animate-[fadeIn_0.15s]" />
        <Dialog.Content
          data-testid="skill-memory-dialog"
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-50',
            'w-[680px] max-w-[90vw] h-[560px] max-h-[85vh]',
            'bg-white dark:bg-gray-900 rounded-xl shadow-2xl',
            'border border-gray-200 dark:border-gray-700',
            'flex flex-col overflow-hidden',
            'animate-[contentShow_0.2s]',
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
            <Dialog.Title className="text-sm font-semibold text-gray-900 dark:text-white">
              {t('skillPanel.dialogTitle')}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button
                className={clsx(
                  'p-1.5 rounded-md',
                  'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                  'hover:bg-gray-100 dark:hover:bg-gray-800',
                )}
              >
                <Cross2Icon className="w-4 h-4" />
              </button>
            </Dialog.Close>
          </div>

          {/* Tabs */}
          <Tabs.Root
            value={activeTab}
            onValueChange={(value) => setActiveTab(value as 'skills' | 'memory')}
            className="flex-1 flex flex-col min-h-0"
          >
            <Tabs.List className="flex border-b border-gray-200 dark:border-gray-700 px-4">
              <Tabs.Trigger
                value="skills"
                className={clsx(
                  'px-4 py-2.5 text-xs font-medium border-b-2 transition-colors -mb-px',
                  activeTab === 'skills'
                    ? 'border-primary-600 text-primary-600 dark:text-primary-400'
                    : 'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300',
                )}
              >
                {t('skillPanel.skillsTab')}
              </Tabs.Trigger>
              <Tabs.Trigger
                value="memory"
                className={clsx(
                  'px-4 py-2.5 text-xs font-medium border-b-2 transition-colors -mb-px',
                  activeTab === 'memory'
                    ? 'border-primary-600 text-primary-600 dark:text-primary-400'
                    : 'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300',
                )}
              >
                {t('skillPanel.memoryTab')}
              </Tabs.Trigger>
            </Tabs.List>

            <Tabs.Content value="skills" className="flex-1 min-h-0">
              <SkillsTab />
            </Tabs.Content>

            <Tabs.Content value="memory" className="flex-1 min-h-0">
              <MemoryTab />
            </Tabs.Content>
          </Tabs.Root>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default SkillMemoryDialog;
