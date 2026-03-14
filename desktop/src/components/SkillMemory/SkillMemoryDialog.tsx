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
import { useContextSourcesStore } from '../../store/contextSources';
import { useExecutionStore } from '../../store/execution';
import { useContextOpsStore } from '../../store/contextOps';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import { selectKernelChatRuntime } from '../../store/workflowKernelSelectors';
import { SkillRow } from '../SimpleMode/SkillRow';
import { SkillDetail } from './SkillDetail';
import { MemoryDetail } from './MemoryDetail';
import { AddMemoryForm } from './AddMemoryForm';
import { CategoryBadge } from './CategoryBadge';
import { ImportanceBar } from './ImportanceBar';
import { EmptyState } from './EmptyState';
import { debounce } from '../Projects/utils';
import type {
  SkillSummary,
  SkillReviewStatus,
  MemoryEntry,
  MemoryCategory,
  MemoryScope,
  MemoryReviewCandidate,
} from '../../types/skillMemory';
import { MEMORY_CATEGORIES } from '../../types/skillMemory';
import { resolveActiveMemorySessionId } from '../../lib/memorySession';

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

function inferMemoryScope(entry: MemoryEntry): MemoryScope {
  if (entry.scope) return entry.scope;
  if (entry.project_path === '__global__') return 'global';
  if (entry.project_path.startsWith('__session__:')) return 'session';
  return 'project';
}

function reviewStatusLabelFallback(status: SkillReviewStatus | null | undefined): string {
  switch (status) {
    case 'approved':
      return 'Approved';
    case 'rejected':
      return 'Rejected';
    case 'archived':
      return 'Archived';
    case 'pending_review':
    default:
      return 'Pending Review';
  }
}

function reviewStatusTone(status: SkillReviewStatus | null | undefined): string {
  switch (status) {
    case 'approved':
      return 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/20 dark:text-emerald-300';
    case 'rejected':
      return 'bg-red-100 text-red-700 dark:bg-red-900/20 dark:text-red-300';
    case 'archived':
      return 'bg-gray-200 text-gray-700 dark:bg-gray-800 dark:text-gray-300';
    case 'pending_review':
    default:
      return 'bg-amber-100 text-amber-700 dark:bg-amber-900/20 dark:text-amber-300';
  }
}

function whyNotSelectedReasonFallback(reason: string): string {
  switch (reason) {
    case 'disabled':
      return 'Disabled';
    case 'pending_review':
      return 'Pending Review';
    case 'rejected':
      return 'Rejected';
    case 'archived':
      return 'Archived';
    case 'phase_mismatch':
      return 'Phase mismatch';
    case 'not_in_explicit_selection':
      return 'Not in explicit selection';
    case 'filtered_out':
      return 'Filtered out';
    case 'unmatched':
    default:
      return 'Not selected by latest matching pass';
  }
}

function skillRouterFallbackReasonFallback(reason: string): string {
  switch (reason) {
    case 'provider_unavailable':
      return 'Provider unavailable';
    case 'empty_query':
      return 'Empty query';
    case 'timeout':
      return 'Router timed out';
    case 'empty_response':
      return 'Router returned empty response';
    case 'invalid_empty_selection':
      return 'Router returned no valid skills';
    default:
      return reason;
  }
}

function toolPolicyModeLabelFallback(mode: SkillSummary['tool_policy_mode']): string {
  return mode === 'restrictive' ? 'Restrictive' : 'Advisory';
}

function sourceTypeLabel(t: (key: string, options?: { defaultValue?: string }) => string, sourceType: string): string {
  switch (sourceType) {
    case 'local':
      return t('skillPanel.sourceTypes.local', { defaultValue: 'Local' });
    case 'git':
      return t('skillPanel.sourceTypes.git', { defaultValue: 'Git' });
    case 'url':
      return t('skillPanel.sourceTypes.url', { defaultValue: 'URL' });
    default:
      return sourceType;
  }
}

// ============================================================================
// SkillsTab
// ============================================================================

type SkillsViewTab = 'catalog' | 'why' | 'sources';

function SkillsTab() {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const latestEnvelope = useContextOpsStore((s) => s.latestEnvelope);
  const skills = useSkillMemoryStore((s) => s.skills);
  const skillsLoading = useSkillMemoryStore((s) => s.skillsLoading);
  const skillSourcesState = useSkillMemoryStore((s) => s.skillSources);
  const skillSourcesLoading = useSkillMemoryStore((s) => s.skillSourcesLoading);
  const skillSearchQuery = useSkillMemoryStore((s) => s.skillSearchQuery);
  const skillSourceFilter = useSkillMemoryStore((s) => s.skillSourceFilter);
  const setSkillSearchQuery = useSkillMemoryStore((s) => s.setSkillSearchQuery);
  const setSkillSourceFilter = useSkillMemoryStore((s) => s.setSkillSourceFilter);
  const toggleSkill = useSkillMemoryStore((s) => s.toggleSkill);
  const toggleGeneratedSkill = useSkillMemoryStore((s) => s.toggleGeneratedSkill);
  const reviewGeneratedSkill = useSkillMemoryStore((s) => s.reviewGeneratedSkill);
  const reviewGeneratedSkills = useSkillMemoryStore((s) => s.reviewGeneratedSkills);
  const refreshSkillIndex = useSkillMemoryStore((s) => s.refreshSkillIndex);
  const loadSkillDetail = useSkillMemoryStore((s) => s.loadSkillDetail);
  const loadSkillSources = useSkillMemoryStore((s) => s.loadSkillSources);
  const installSkillSource = useSkillMemoryStore((s) => s.installSkillSource);
  const setSkillSourceEnabled = useSkillMemoryStore((s) => s.setSkillSourceEnabled);
  const refreshSkillSource = useSkillMemoryStore((s) => s.refreshSkillSource);
  const removeSkillSource = useSkillMemoryStore((s) => s.removeSkillSource);
  const importGeneratedSkill = useSkillMemoryStore((s) => s.importGeneratedSkill);
  const skillDetail = useSkillMemoryStore((s) => s.skillDetail);
  const selectedSkillIds = useContextSourcesStore((s) => s.selectedSkillIds);
  const invokedSkillIds = useContextSourcesStore((s) => s.invokedSkillIds);

  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null);
  const [skillsView, setSkillsView] = useState<SkillsViewTab>('catalog');
  const [sourceInstallValue, setSourceInstallValue] = useState('');
  const [sourceInstallName, setSourceInstallName] = useState('');
  const diagnostics = latestEnvelope?.diagnostics;
  const skillSources = useMemo(
    () => (latestEnvelope?.sources ?? []).filter((source) => source.kind === 'skills'),
    [latestEnvelope],
  );
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
  const skillsViewTabs: { value: SkillsViewTab; label: string }[] = useMemo(
    () => [
      {
        value: 'catalog',
        label: t('skillPanel.skillsViews.catalog', { defaultValue: 'Skills' }),
      },
      {
        value: 'why',
        label: t('skillPanel.skillsViews.why', { defaultValue: 'Why these skills?' }),
      },
      {
        value: 'sources',
        label: t('skillPanel.skillsViews.sources', { defaultValue: 'Skill Sources' }),
      },
    ],
    [t],
  );

  useEffect(() => {
    if (workspacePath) {
      void loadSkillSources(workspacePath);
    }
  }, [loadSkillSources, workspacePath]);

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
  const pendingGeneratedSkillIds = useMemo(
    () =>
      filteredSkills
        .filter((skill) => skill.source.type === 'generated' && skill.review_status === 'pending_review')
        .map((skill) => skill.id),
    [filteredSkills],
  );

  const handleToggle = useCallback(
    (id: string, enabled: boolean, sourceType?: SkillSummary['source']['type']) => {
      if (sourceType === 'generated') {
        toggleGeneratedSkill(id, enabled);
        return;
      }
      toggleSkill(id, enabled);
    },
    [toggleGeneratedSkill, toggleSkill],
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
      void loadSkillSources(workspacePath);
    }
  }, [workspacePath, refreshSkillIndex, loadSkillSources]);

  const handleInstallSource = useCallback(() => {
    if (!workspacePath || !sourceInstallValue.trim()) return;
    void installSkillSource(workspacePath, sourceInstallValue, sourceInstallName || null).then(() => {
      setSourceInstallValue('');
      setSourceInstallName('');
    });
  }, [installSkillSource, sourceInstallName, sourceInstallValue, workspacePath]);

  const handleImportGenerated = useCallback(() => {
    if (!workspacePath) return;
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const json = await file.text();
      const imported = await importGeneratedSkill(workspacePath, json, 'rename');
      if (imported) {
        setSelectedSkillId(imported.id);
      }
    };
    input.click();
  }, [importGeneratedSkill, workspacePath]);

  const unselectedSkillReasons = useMemo(() => {
    if (diagnostics?.why_not_selected_skills?.length) {
      return diagnostics.why_not_selected_skills.slice(0, 12).map((entry) => ({
        id: entry.skill_id,
        name: entry.skill_name,
        path: entry.path,
        source_type: entry.source_type,
        reason: t(`skillPanel.whyNotSelectedReasons.${entry.reason}`, {
          defaultValue: whyNotSelectedReasonFallback(entry.reason),
        }),
      }));
    }
    const effectiveIds = new Set(diagnostics?.effective_skill_ids ?? []);
    return skills
      .filter((skill) => !effectiveIds.has(skill.id))
      .map((skill) => {
        let reason = t('skillPanel.whyNotSelected.unmatched', { defaultValue: 'Not selected by latest matching pass' });
        if (!skill.enabled) {
          reason = t('skillPanel.whyNotSelected.disabled', { defaultValue: 'Disabled' });
        } else if (skill.review_status && skill.review_status !== 'approved') {
          reason = t(`skillPanel.reviewStatus.${skill.review_status}`, {
            defaultValue: reviewStatusLabelFallback(skill.review_status),
          });
        } else if (
          (selectedSkillIds.length > 0 || invokedSkillIds.length > 0) &&
          !selectedSkillIds.includes(skill.id) &&
          !invokedSkillIds.includes(skill.id)
        ) {
          reason = t('skillPanel.whyNotSelected.notPinnedOrExplicit', {
            defaultValue: 'Not in explicit or pinned selection',
          });
        } else if (
          selectedSkillIds.length > 0 &&
          !selectedSkillIds.includes(skill.id) &&
          diagnostics?.selection_origin === 'explicit'
        ) {
          reason = t('skillPanel.whyNotSelected.notExplicitlySelected', {
            defaultValue: 'Not explicitly selected',
          });
        }
        return { id: skill.id, name: skill.name, path: skill.path, source_type: skill.source.type, reason };
      })
      .slice(0, 8);
  }, [
    diagnostics?.effective_skill_ids,
    diagnostics?.selection_origin,
    diagnostics?.why_not_selected_skills,
    invokedSkillIds,
    selectedSkillIds,
    skills,
    t,
  ]);

  // If a skill detail is open, show it
  if (selectedSkillId && skillDetail) {
    return <SkillDetail skill={skillDetail} onClose={() => setSelectedSkillId(null)} projectPath={workspacePath} />;
  }

  return (
    <div className="flex flex-col h-full">
      <div className="border-b border-gray-200 px-3 py-2 dark:border-gray-700">
        <div className="flex items-center gap-1 flex-wrap">
          {skillsViewTabs.map((tab) => (
            <button
              key={tab.value}
              type="button"
              onClick={() => setSkillsView(tab.value)}
              className={clsx(
                'px-2.5 py-1 rounded-md text-2xs font-medium transition-colors',
                skillsView === tab.value
                  ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                  : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
              )}
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {skillsView === 'catalog' && (
        <>
          <div className="p-3 border-b border-gray-200 dark:border-gray-700 space-y-2">
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
              <button
                onClick={handleImportGenerated}
                className={clsx(
                  'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                  'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
                )}
              >
                {t('skillPanel.importGenerated', { defaultValue: 'Import Generated' })}
              </button>
            </div>
          </div>

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
                      {sourceType === 'generated' && pendingGeneratedSkillIds.length > 0 && (
                        <span className="ml-2 inline-flex items-center gap-1">
                          <button
                            type="button"
                            onClick={() => void reviewGeneratedSkills(pendingGeneratedSkillIds, 'approved')}
                            className="rounded px-1.5 py-0.5 text-2xs font-medium text-emerald-700 hover:bg-emerald-50 dark:text-emerald-300 dark:hover:bg-emerald-900/20"
                          >
                            {t('skillPanel.approvePendingGenerated', { defaultValue: 'Approve Pending' })}
                          </button>
                          <button
                            type="button"
                            onClick={() => void reviewGeneratedSkills(pendingGeneratedSkillIds, 'rejected')}
                            className="rounded px-1.5 py-0.5 text-2xs font-medium text-red-700 hover:bg-red-50 dark:text-red-300 dark:hover:bg-red-900/20"
                          >
                            {t('skillPanel.rejectPendingGenerated', { defaultValue: 'Reject Pending' })}
                          </button>
                        </span>
                      )}
                    </div>
                    {groupSkills.map((skill) => (
                      <div key={skill.id} className="space-y-1">
                        <SkillRow
                          skill={skill}
                          onToggle={(id, enabled) => handleToggle(id, enabled, skill.source.type)}
                          onClick={handleSkillClick}
                        />
                        {(skill.source.type === 'generated' || skill.review_status) && (
                          <div className="ml-7 flex flex-wrap items-center gap-1.5 pb-1">
                            <span
                              className={clsx(
                                'inline-flex items-center rounded-full px-1.5 py-0.5 text-2xs font-medium',
                                reviewStatusTone(skill.review_status),
                              )}
                            >
                              {t(`skillPanel.reviewStatus.${skill.review_status ?? 'pending_review'}`, {
                                defaultValue: reviewStatusLabelFallback(skill.review_status),
                              })}
                            </span>
                            {skill.source.type === 'generated' && (
                              <>
                                <button
                                  type="button"
                                  onClick={() => void reviewGeneratedSkill(skill.id, 'approved')}
                                  className="rounded px-1.5 py-0.5 text-2xs font-medium text-emerald-700 hover:bg-emerald-50 dark:text-emerald-300 dark:hover:bg-emerald-900/20"
                                >
                                  {t('skillPanel.reviewActions.approve', { defaultValue: 'Approve' })}
                                </button>
                                <button
                                  type="button"
                                  onClick={() => void reviewGeneratedSkill(skill.id, 'rejected')}
                                  className="rounded px-1.5 py-0.5 text-2xs font-medium text-red-700 hover:bg-red-50 dark:text-red-300 dark:hover:bg-red-900/20"
                                >
                                  {t('skillPanel.reviewActions.reject', { defaultValue: 'Reject' })}
                                </button>
                                {skill.review_status === 'archived' ? (
                                  <button
                                    type="button"
                                    onClick={() => void reviewGeneratedSkill(skill.id, 'pending_review')}
                                    className="rounded px-1.5 py-0.5 text-2xs font-medium text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-800"
                                  >
                                    {t('skillPanel.reviewActions.restore', { defaultValue: 'Restore' })}
                                  </button>
                                ) : (
                                  <button
                                    type="button"
                                    onClick={() => void reviewGeneratedSkill(skill.id, 'archived')}
                                    className="rounded px-1.5 py-0.5 text-2xs font-medium text-gray-700 hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-800"
                                  >
                                    {t('skillPanel.reviewActions.archive', { defaultValue: 'Archive' })}
                                  </button>
                                )}
                              </>
                            )}
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            )}
          </div>
        </>
      )}

      {skillsView === 'why' && (
        <div className="flex-1 overflow-y-auto p-3">
          <div className="rounded-md border border-sky-200 dark:border-sky-900 bg-sky-50/60 dark:bg-sky-900/10 p-3 text-2xs text-sky-800 dark:text-sky-200 space-y-2">
            <div>
              {t('skillPanel.selectionReason', { defaultValue: 'Selection reason' })}:{' '}
              {diagnostics?.selection_reason
                ? t(`skillPanel.selectionReasons.${diagnostics.selection_reason}`, {
                    defaultValue: diagnostics.selection_reason,
                  })
                : t('skillPanel.none', { defaultValue: 'none' })}
            </div>
            <div>
              {t('skillPanel.selectionOrigin', { defaultValue: 'Selection origin' })}:{' '}
              {diagnostics?.selection_origin
                ? t(`skillPanel.selectionOrigins.${diagnostics.selection_origin}`, {
                    defaultValue: diagnostics.selection_origin,
                  })
                : t('skillPanel.none', { defaultValue: 'none' })}
            </div>
            {(diagnostics?.skill_router_used || diagnostics?.skill_router_fallback_reason) && (
              <>
                <div>
                  {t('skillPanel.routerStatus', { defaultValue: 'Router' })}:{' '}
                  {t(`skillPanel.routerStrategies.${diagnostics?.skill_router_strategy ?? 'hybrid'}`, {
                    defaultValue: diagnostics?.skill_router_strategy ?? 'hybrid',
                  })}
                </div>
                <div>
                  {t('skillPanel.routerConfidence', { defaultValue: 'Router confidence' })}:{' '}
                  {typeof diagnostics?.skill_router_confidence === 'number'
                    ? diagnostics.skill_router_confidence.toFixed(2)
                    : t('skillPanel.none', { defaultValue: 'none' })}
                </div>
                <div>
                  {t('skillPanel.routerReason', { defaultValue: 'Router reason' })}:{' '}
                  {diagnostics?.skill_router_reason || t('skillPanel.none', { defaultValue: 'none' })}
                </div>
                <div>
                  {t('skillPanel.routerFallback', { defaultValue: 'Fallback' })}:{' '}
                  {diagnostics?.skill_router_fallback_reason
                    ? t(`skillPanel.routerFallbackReasons.${diagnostics.skill_router_fallback_reason}`, {
                        defaultValue: skillRouterFallbackReasonFallback(diagnostics.skill_router_fallback_reason),
                      })
                    : t('skillPanel.none', { defaultValue: 'none' })}
                </div>
              </>
            )}
            <div>
              {t('skillPanel.selectedSkills', { defaultValue: 'Selected skills' })}:{' '}
              {diagnostics?.selected_skills?.length
                ? diagnostics.selected_skills.join(', ')
                : t('skillPanel.none', { defaultValue: 'none' })}
            </div>
            {skills
              .filter((skill) => diagnostics?.effective_skill_ids?.includes(skill.id))
              .map((skill) => (
                <div key={skill.id}>
                  {skill.name}:{' '}
                  {t(`skillPanel.toolPolicyModes.${skill.tool_policy_mode}`, {
                    defaultValue: toolPolicyModeLabelFallback(skill.tool_policy_mode),
                  })}
                </div>
              ))}
            <div>
              {t('skillPanel.blockedTools', { defaultValue: 'Blocked tools' })}:{' '}
              {diagnostics?.blocked_tools?.length
                ? diagnostics.blocked_tools.join(', ')
                : t('skillPanel.none', { defaultValue: 'none' })}
            </div>
            {skillSources.length > 0 && (
              <div>
                {t('skillPanel.sourceReason', { defaultValue: 'Source reason' })}:{' '}
                {skillSources.map((s) => s.reason).join(', ')}
              </div>
            )}
            <div>
              {t('skillPanel.hierarchyMatches', { defaultValue: 'Hierarchy matches' })}:{' '}
              {diagnostics?.hierarchy_matches?.length
                ? diagnostics.hierarchy_matches.join(', ')
                : t('skillPanel.none', { defaultValue: 'none' })}
            </div>
            <div>
              {t('skillPanel.commandPinnedSkills', { defaultValue: 'Command pinned skills' })}:{' '}
              {invokedSkillIds.length ? invokedSkillIds.join(', ') : t('skillPanel.none', { defaultValue: 'none' })}
            </div>
          </div>

          {unselectedSkillReasons.length > 0 && (
            <div className="mt-3 rounded-md border border-sky-100 bg-white/70 p-3 dark:border-sky-950 dark:bg-gray-900/40">
              <div className="text-xs font-semibold text-gray-700 dark:text-gray-200">
                {t('skillPanel.whyNotSelectedTitle', { defaultValue: 'Why not selected' })}
              </div>
              <div className="mt-2 space-y-2">
                {unselectedSkillReasons.map((item) => (
                  <div
                    key={item.id}
                    className="rounded border border-gray-200 dark:border-gray-700 px-2 py-1.5 text-2xs"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate text-gray-700 dark:text-gray-200">{item.name}</span>
                      <span className="text-gray-500 dark:text-gray-400">{item.reason}</span>
                    </div>
                    {'path' in item && item.path ? (
                      <div className="mt-0.5 truncate text-[10px] text-gray-400 dark:text-gray-500">
                        {'source_type' in item ? item.source_type : ''} · {item.path}
                      </div>
                    ) : null}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {skillsView === 'sources' && (
        <div className="flex-1 overflow-y-auto p-3">
          <div className="rounded-md border border-gray-200 dark:border-gray-700 bg-gray-50/80 dark:bg-gray-800/40 p-3 space-y-3">
            <div className="flex items-center justify-between gap-2">
              <div>
                <div className="text-xs font-semibold text-gray-700 dark:text-gray-200">
                  {t('skillPanel.skillSourcesTitle', { defaultValue: 'Skill Sources' })}
                </div>
                <div className="text-2xs text-gray-500 dark:text-gray-400">
                  {t('skillPanel.skillSourcesHint', {
                    defaultValue: 'Manage installed local, Git, and URL-backed skill sources.',
                  })}
                </div>
              </div>
              {skillSourcesLoading && (
                <span className="text-2xs text-gray-400 dark:text-gray-500">
                  {t('skillPanel.loading', { defaultValue: 'Loading...' })}
                </span>
              )}
            </div>
            <div className="grid grid-cols-1 gap-2 sm:grid-cols-[1fr_180px_auto]">
              <input
                value={sourceInstallValue}
                onChange={(event) => setSourceInstallValue(event.target.value)}
                placeholder={t('skillPanel.skillSourceInput', {
                  defaultValue: 'Paste a local path, Git URL, github:owner/repo, or raw SKILL.md URL',
                })}
                className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-2xs text-gray-700 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-200"
              />
              <input
                value={sourceInstallName}
                onChange={(event) => setSourceInstallName(event.target.value)}
                placeholder={t('skillPanel.skillSourceName', { defaultValue: 'Optional source name' })}
                className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-2xs text-gray-700 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-200"
              />
              <button
                type="button"
                onClick={handleInstallSource}
                className="rounded-md bg-primary-600 px-3 py-1.5 text-2xs font-medium text-white hover:bg-primary-700"
              >
                {t('skillPanel.installSkillSource', { defaultValue: 'Install Source' })}
              </button>
            </div>
            {skillSourcesState.length > 0 ? (
              <div className="space-y-2">
                {skillSourcesState.map((source) => (
                  <div
                    key={source.name}
                    className="flex flex-wrap items-center gap-2 rounded-md border border-gray-200 dark:border-gray-700 px-2 py-1.5 text-2xs text-gray-600 dark:text-gray-300"
                  >
                    <span className="font-medium">{source.name}</span>
                    <span className="rounded bg-gray-100 px-1.5 py-0.5 dark:bg-gray-700">
                      {sourceTypeLabel(t, source.source_type)}
                    </span>
                    <span>
                      {t('skillPanel.sourceSkillCount', {
                        count: source.skill_count,
                        defaultValue: '{{count}} skills',
                      })}
                    </span>
                    <span
                      className={
                        source.installed
                          ? 'text-emerald-600 dark:text-emerald-300'
                          : 'text-amber-600 dark:text-amber-300'
                      }
                    >
                      {source.installed
                        ? t('skillPanel.sourceInstalled', { defaultValue: 'installed' })
                        : t('skillPanel.sourceMissing', { defaultValue: 'missing' })}
                    </span>
                    <button
                      type="button"
                      onClick={() =>
                        workspacePath && void setSkillSourceEnabled(workspacePath, source.name, !source.enabled)
                      }
                      className={clsx(
                        'rounded border px-1.5 py-0.5 text-2xs',
                        source.enabled
                          ? 'border-emerald-200 text-emerald-700 hover:bg-emerald-50 dark:border-emerald-800 dark:text-emerald-300 dark:hover:bg-emerald-900/20'
                          : 'border-gray-200 text-gray-500 hover:bg-gray-50 dark:border-gray-700 dark:text-gray-300 dark:hover:bg-gray-800',
                      )}
                      title={
                        source.enabled
                          ? t('skillPanel.disableSkillSource', { defaultValue: 'Disable source' })
                          : t('skillPanel.enableSkillSource', { defaultValue: 'Enable source' })
                      }
                    >
                      {source.enabled
                        ? t('skillPanel.sourceEnabled', { defaultValue: 'enabled' })
                        : t('skillPanel.sourceDisabled', { defaultValue: 'disabled' })}
                    </button>
                    <span className="truncate text-gray-400 dark:text-gray-500">
                      {source.repository || source.url || source.path}
                    </span>
                    {workspacePath && (
                      <>
                        <button
                          type="button"
                          onClick={() => void refreshSkillSource(workspacePath, source.name)}
                          className="rounded border border-gray-200 px-1.5 py-0.5 text-2xs text-gray-500 hover:bg-gray-50 dark:border-gray-700 dark:text-gray-300 dark:hover:bg-gray-800"
                          title={t('skillPanel.refreshSkillSource', { defaultValue: 'Refresh source' })}
                        >
                          <ReloadIcon className="h-3 w-3" />
                        </button>
                        <button
                          type="button"
                          onClick={() => void removeSkillSource(workspacePath, source.name)}
                          className="rounded border border-red-200 px-1.5 py-0.5 text-2xs text-red-600 hover:bg-red-50 dark:border-red-800 dark:text-red-300 dark:hover:bg-red-900/20"
                          title={t('skillPanel.removeSkillSource', { defaultValue: 'Remove source' })}
                        >
                          <TrashIcon className="h-3 w-3" />
                        </button>
                      </>
                    )}
                  </div>
                ))}
              </div>
            ) : (
              <p className="text-2xs text-gray-400 dark:text-gray-500">
                {t('skillPanel.noSkillSources', { defaultValue: 'No external skill sources configured yet.' })}
              </p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// ============================================================================
// MemoryTab
// ============================================================================

function MemoryTab() {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const latestEnvelope = useContextOpsStore((s) => s.latestEnvelope);
  const rootSessionId = useWorkflowKernelStore((s) => s.sessionId);
  const kernelChatBindingSessionId = useWorkflowKernelStore((s) => selectKernelChatRuntime(s.session).bindingSessionId);
  const taskId = useExecutionStore((s) => s.taskId);
  const standaloneSessionId = useExecutionStore((s) => s.standaloneSessionId);
  const foregroundOriginSessionId = useExecutionStore((s) => s.foregroundOriginSessionId);
  const memories = useSkillMemoryStore((s) => s.memories);
  const memoriesLoading = useSkillMemoryStore((s) => s.memoriesLoading);
  const memorySearchQuery = useSkillMemoryStore((s) => s.memorySearchQuery);
  const memoryCategoryFilter = useSkillMemoryStore((s) => s.memoryCategoryFilter);
  const memoryScope = useSkillMemoryStore((s) => s.memoryScope);
  const memoryViewMode = useSkillMemoryStore((s) => s.memoryViewMode);
  const memoryHasMore = useSkillMemoryStore((s) => s.memoryHasMore);
  const setMemorySearchQuery = useSkillMemoryStore((s) => s.setMemorySearchQuery);
  const setMemoryCategoryFilter = useSkillMemoryStore((s) => s.setMemoryCategoryFilter);
  const setMemoryScope = useSkillMemoryStore((s) => s.setMemoryScope);
  const setMemorySessionId = useSkillMemoryStore((s) => s.setMemorySessionId);
  const setMemoryViewMode = useSkillMemoryStore((s) => s.setMemoryViewMode);
  const loadMemories = useSkillMemoryStore((s) => s.loadMemories);
  const loadMoreMemories = useSkillMemoryStore((s) => s.loadMoreMemories);
  const searchMemories = useSkillMemoryStore((s) => s.searchMemories);
  const updateMemory = useSkillMemoryStore((s) => s.updateMemory);
  const deleteMemory = useSkillMemoryStore((s) => s.deleteMemory);
  const addMemory = useSkillMemoryStore((s) => s.addMemory);
  const clearMemories = useSkillMemoryStore((s) => s.clearMemories);
  const memoryStats = useSkillMemoryStore((s) => s.memoryStats);
  const loadMemoryStats = useSkillMemoryStore((s) => s.loadMemoryStats);
  const pendingMemoryCandidates = useSkillMemoryStore((s) => s.pendingMemoryCandidates);
  const pendingMemoryCandidatesLoading = useSkillMemoryStore((s) => s.pendingMemoryCandidatesLoading);
  const loadPendingMemoryCandidates = useSkillMemoryStore((s) => s.loadPendingMemoryCandidates);
  const reviewPendingMemoryCandidates = useSkillMemoryStore((s) => s.reviewPendingMemoryCandidates);
  const setMemoryStatus = useSkillMemoryStore((s) => s.setMemoryStatus);
  const restoreDeletedMemories = useSkillMemoryStore((s) => s.restoreDeletedMemories);
  const purgeMemories = useSkillMemoryStore((s) => s.purgeMemories);
  const runMaintenance = useSkillMemoryStore((s) => s.runMaintenance);
  const memoryPipelineSnapshot = useSkillMemoryStore((s) =>
    rootSessionId ? (s.memoryPipelineByRootSession[rootSessionId] ?? null) : null,
  );

  const [selectedMemory, setSelectedMemory] = useState<MemoryEntry | null>(null);
  const [showAddForm, setShowAddForm] = useState(false);
  const [selectedPendingIds, setSelectedPendingIds] = useState<Set<string>>(new Set());
  const [selectedPersistedIds, setSelectedPersistedIds] = useState<Set<string>>(new Set());
  const [showWhyMemory, setShowWhyMemory] = useState(false);
  const diagnostics = latestEnvelope?.diagnostics;
  const memorySources = useMemo(
    () => (latestEnvelope?.sources ?? []).filter((source) => source.kind === 'memory'),
    [latestEnvelope],
  );
  const activeSessionId = useMemo(() => {
    return resolveActiveMemorySessionId({
      foregroundOriginSessionId,
      bindingSessionId: kernelChatBindingSessionId,
      taskId,
      standaloneSessionId,
    });
  }, [foregroundOriginSessionId, kernelChatBindingSessionId, taskId, standaloneSessionId]);
  const persistedStatusCounts = memoryStats?.status_counts ?? {};
  const allPersistedCount =
    (persistedStatusCounts.active ?? 0) +
    (persistedStatusCounts.rejected ?? 0) +
    (persistedStatusCounts.archived ?? 0) +
    (persistedStatusCounts.deleted ?? 0);
  const currentPersistedSelectionCount = selectedPersistedIds.size;
  const isPersistedSelectionView =
    memoryViewMode === 'rejected' || memoryViewMode === 'archived' || memoryViewMode === 'deleted';

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
      if (memoryViewMode === 'pending') {
        void loadPendingMemoryCandidates(workspacePath);
      } else {
        if (memorySearchQuery.trim()) {
          void searchMemories(workspacePath, memorySearchQuery);
        } else {
          void loadMemories(workspacePath);
        }
        void loadMemoryStats(workspacePath);
      }
      void loadPendingMemoryCandidates(workspacePath);
    }
  }, [memoryCategoryFilter, memoryScope, activeSessionId, memoryViewMode]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    setSelectedPendingIds(new Set());
    setSelectedPersistedIds(new Set());
  }, [memoryScope, pendingMemoryCandidates.length, memoryViewMode]);

  const pipelineStatusLabel = useMemo(() => {
    if (!memoryPipelineSnapshot) return null;
    if (memoryPipelineSnapshot.requiresReviewModel) {
      return t('skillPanel.pipeline.status.requiresReviewModel', { defaultValue: 'Review model required' });
    }
    if (memoryPipelineSnapshot.phase === 'extracting') {
      return t('skillPanel.pipeline.status.extracting', { defaultValue: 'Extracting memories' });
    }
    if (memoryPipelineSnapshot.phase === 'reviewing') {
      return t('skillPanel.pipeline.status.reviewing', { defaultValue: 'Reviewing memories' });
    }
    if (memoryPipelineSnapshot.phase === 'error') {
      return t('skillPanel.pipeline.status.error', { defaultValue: 'Pipeline error' });
    }
    if (memoryPipelineSnapshot.pendingCount > 0) {
      return t('skillPanel.pipeline.status.pending', {
        count: memoryPipelineSnapshot.pendingCount,
        defaultValue: '{{count}} pending review',
      });
    }
    if (memoryPipelineSnapshot.injectedCount > 0) {
      return t('skillPanel.pipeline.status.injected', {
        count: memoryPipelineSnapshot.injectedCount,
        defaultValue: '{{count}} injected',
      });
    }
    if (memoryPipelineSnapshot.extractedCount === 0 && memoryPipelineSnapshot.lastRunAt) {
      return t('skillPanel.pipeline.status.empty', { defaultValue: 'No memories extracted' });
    }
    return t('skillPanel.pipeline.status.ready', { defaultValue: 'Pipeline ready' });
  }, [memoryPipelineSnapshot, t]);

  const activeMemoryIndex = useMemo(() => {
    const grouped = new Map<string, MemoryEntry[]>();
    for (const entry of memories) {
      const scope = inferMemoryScope(entry);
      const key = `${scope}:${entry.category}`;
      const prev = grouped.get(key) || [];
      prev.push(entry);
      grouped.set(key, prev);
    }
    return grouped;
  }, [memories]);

  const conflictReference = useCallback(
    (candidate: MemoryReviewCandidate): MemoryEntry | null => {
      const key = `${candidate.scope}:${candidate.category}`;
      const candidates = activeMemoryIndex.get(key);
      if (!candidates || candidates.length === 0) return null;
      const normalizedPending = candidate.content.toLowerCase();
      const best = [...candidates].sort((a, b) => {
        const aHit = normalizedPending.includes(a.content.toLowerCase()) ? 1 : 0;
        const bHit = normalizedPending.includes(b.content.toLowerCase()) ? 1 : 0;
        return bHit - aHit;
      });
      return best[0] ?? null;
    },
    [activeMemoryIndex],
  );

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

  const handleTogglePendingSelection = useCallback((memoryId: string) => {
    setSelectedPendingIds((prev) => {
      const next = new Set(prev);
      if (next.has(memoryId)) {
        next.delete(memoryId);
      } else {
        next.add(memoryId);
      }
      return next;
    });
  }, []);

  const handleTogglePersistedSelection = useCallback((memoryId: string) => {
    setSelectedPersistedIds((prev) => {
      const next = new Set(prev);
      if (next.has(memoryId)) {
        next.delete(memoryId);
      } else {
        next.add(memoryId);
      }
      return next;
    });
  }, []);

  const handleToggleSelectAllPersisted = useCallback(() => {
    if (currentPersistedSelectionCount >= memories.length) {
      setSelectedPersistedIds(new Set());
      return;
    }
    setSelectedPersistedIds(new Set(memories.map((memory) => memory.id)));
  }, [currentPersistedSelectionCount, memories]);

  const handleToggleSelectAllPending = useCallback(() => {
    if (selectedPendingIds.size >= pendingMemoryCandidates.length) {
      setSelectedPendingIds(new Set());
      return;
    }
    setSelectedPendingIds(new Set(pendingMemoryCandidates.map((candidate) => candidate.id)));
  }, [pendingMemoryCandidates, selectedPendingIds.size]);

  const handleReviewPending = useCallback(
    async (ids: string[], decision: 'approve' | 'reject' | 'archive' | 'restore') => {
      if (!workspacePath || ids.length === 0) return;
      await reviewPendingMemoryCandidates(workspacePath, ids, decision);
      setSelectedPendingIds((prev) => {
        const next = new Set(prev);
        ids.forEach((id) => next.delete(id));
        return next;
      });
      setSelectedPersistedIds((prev) => {
        const next = new Set(prev);
        ids.forEach((id) => next.delete(id));
        return next;
      });
    },
    [workspacePath, reviewPendingMemoryCandidates],
  );

  const handleRunMaintenance = useCallback(async () => {
    if (!workspacePath) return;
    await runMaintenance(workspacePath);
    await Promise.all([
      loadMemories(workspacePath),
      loadMemoryStats(workspacePath),
      loadPendingMemoryCandidates(workspacePath),
    ]);
  }, [workspacePath, runMaintenance, loadMemories, loadMemoryStats, loadPendingMemoryCandidates]);

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
        onSetStatus={(id, targetStatus) => {
          if (!workspacePath) return;
          void setMemoryStatus(workspacePath, [id], targetStatus);
          setSelectedMemory(null);
        }}
        onRestoreDeleted={(id) => {
          if (!workspacePath) return;
          void restoreDeletedMemories(workspacePath, [id]);
          setSelectedMemory(null);
        }}
        onPurge={(id) => {
          if (!workspacePath) return;
          void purgeMemories(workspacePath, [id]);
          setSelectedMemory(null);
        }}
        onReviewDecision={(id, decision) => {
          if (!workspacePath) return;
          void handleReviewPending([id], decision);
          setSelectedMemory(null);
        }}
      />
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Toolbar: scope + mode + filters */}
      <div className="p-3 border-b border-gray-200 dark:border-gray-700 space-y-2">
        {memoryPipelineSnapshot && (
          <div className="rounded-md border border-sky-200 dark:border-sky-900 bg-sky-50/70 dark:bg-sky-900/10 p-2 text-2xs text-sky-800 dark:text-sky-200">
            <div className="flex items-center justify-between gap-2">
              <span className="font-medium">
                {t('skillPanel.pipeline.title', { defaultValue: 'Automatic memory pipeline' })}
              </span>
              <span>{pipelineStatusLabel}</span>
            </div>
            <div className="mt-1 flex flex-wrap gap-x-3 gap-y-1 text-sky-700 dark:text-sky-300">
              <span>
                {t('skillPanel.pipeline.extracted', {
                  count: memoryPipelineSnapshot.extractedCount,
                  defaultValue: 'Extracted {{count}}',
                })}
              </span>
              <span>
                {t('skillPanel.pipeline.approved', {
                  count: memoryPipelineSnapshot.approvedCount,
                  defaultValue: 'Approved {{count}}',
                })}
              </span>
              <span>
                {t('skillPanel.pipeline.rejected', {
                  count: memoryPipelineSnapshot.rejectedCount,
                  defaultValue: 'Rejected {{count}}',
                })}
              </span>
              <span>
                {t('skillPanel.pipeline.pending', {
                  count: memoryPipelineSnapshot.pendingCount,
                  defaultValue: 'Pending {{count}}',
                })}
              </span>
              <span>
                {t('skillPanel.pipeline.scopeSummary', {
                  global: memoryPipelineSnapshot.resolvedScopes.global,
                  project: memoryPipelineSnapshot.resolvedScopes.project,
                  session: memoryPipelineSnapshot.resolvedScopes.session,
                  defaultValue: 'G {{global}} / P {{project}} / S {{session}}',
                })}
              </span>
            </div>
            <div className="mt-1 text-sky-700 dark:text-sky-300">
              {t('skillPanel.pipeline.reviewSource', { defaultValue: 'Review source' })}:{' '}
              {memoryPipelineSnapshot.reviewSource
                ? t(`skillPanel.pipeline.reviewSources.${memoryPipelineSnapshot.reviewSource}`, {
                    defaultValue: memoryPipelineSnapshot.reviewSource,
                  })
                : t('skillPanel.none', { defaultValue: 'none' })}
            </div>
          </div>
        )}

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

        {/* View mode */}
        <div className="flex items-center gap-1 flex-wrap">
          <button
            onClick={() => setMemoryViewMode('all')}
            className={clsx(
              'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
              memoryViewMode === 'all'
                ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
          >
            {t('skillPanel.allMemories', { defaultValue: 'All' })} ({allPersistedCount})
          </button>
          <button
            onClick={() => setMemoryViewMode('active')}
            className={clsx(
              'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
              memoryViewMode === 'active'
                ? 'bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300'
                : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
          >
            {t('skillPanel.activeMemories', { defaultValue: 'Active' })}
          </button>
          <button
            onClick={() => setMemoryViewMode('pending')}
            className={clsx(
              'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
              memoryViewMode === 'pending'
                ? 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300'
                : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
          >
            {t('skillPanel.pendingReview', { defaultValue: 'Pending Review' })} ({pendingMemoryCandidates.length})
          </button>
          <button
            onClick={() => setMemoryViewMode('rejected')}
            className={clsx(
              'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
              memoryViewMode === 'rejected'
                ? 'bg-rose-100 dark:bg-rose-900/30 text-rose-700 dark:text-rose-300'
                : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
          >
            {t('skillPanel.rejectedMemories', { defaultValue: 'Rejected' })}
          </button>
          <button
            onClick={() => setMemoryViewMode('archived')}
            className={clsx(
              'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
              memoryViewMode === 'archived'
                ? 'bg-slate-100 dark:bg-slate-900/30 text-slate-700 dark:text-slate-300'
                : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
          >
            {t('skillPanel.archivedMemories', { defaultValue: 'Archived' })} ({persistedStatusCounts.archived ?? 0})
          </button>
          <button
            onClick={() => setMemoryViewMode('deleted')}
            className={clsx(
              'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
              memoryViewMode === 'deleted'
                ? 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300'
                : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
          >
            {t('skillPanel.deletedMemories', { defaultValue: 'Recycle Bin' })} ({persistedStatusCounts.deleted ?? 0})
          </button>
          <button
            onClick={() => setShowWhyMemory((value) => !value)}
            className={clsx(
              'ml-auto px-2 py-1 rounded-md text-2xs font-medium transition-colors',
              'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
            title={t('skillPanel.whyMemoryTitle', { defaultValue: 'Why these memories' })}
          >
            {showWhyMemory
              ? t('skillPanel.hideWhyMemory', { defaultValue: 'Hide Why' })
              : t('skillPanel.showWhyMemory', { defaultValue: 'Why Memory?' })}
          </button>
        </div>

        {showWhyMemory && (
          <div className="rounded-md border border-amber-200 dark:border-amber-900 bg-amber-50/60 dark:bg-amber-900/10 p-2 text-2xs text-amber-800 dark:text-amber-200 space-y-1">
            <div>
              {t('skillPanel.selectionReason', { defaultValue: 'Selection reason' })}:{' '}
              {diagnostics?.selection_reason || t('skillPanel.none', { defaultValue: 'none' })}
            </div>
            <div>
              {t('skillPanel.selectionOrigin', { defaultValue: 'Selection origin' })}:{' '}
              {diagnostics?.selection_origin || t('skillPanel.none', { defaultValue: 'none' })}
            </div>
            <div>
              {t('skillPanel.effectiveStatuses', { defaultValue: 'Effective statuses' })}:{' '}
              {diagnostics?.effective_statuses?.length
                ? diagnostics.effective_statuses.join(', ')
                : t('skillPanel.none', { defaultValue: 'none' })}
            </div>
            <div>
              {t('skillPanel.candidateCount', { defaultValue: 'Candidates' })}:{' '}
              {diagnostics?.memory_candidates_count ?? 0}
            </div>
            <div>
              {t('skillPanel.degradedReason', { defaultValue: 'Degraded reason' })}:{' '}
              {diagnostics?.degraded_reason || t('skillPanel.none', { defaultValue: 'none' })}
            </div>
            {memorySources.length > 0 && (
              <div>
                {t('skillPanel.sourceReason', { defaultValue: 'Source reason' })}:{' '}
                {memorySources.map((s) => s.reason).join(', ')}
              </div>
            )}
          </div>
        )}

        {/* Search */}
        <div className="relative">
          <MagnifyingGlassIcon className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
          <input
            type="text"
            value={memorySearchQuery}
            onChange={handleSearch}
            disabled={memoryViewMode === 'pending'}
            placeholder={t('skillPanel.searchMemories', { defaultValue: 'Search memories...' })}
            className={clsx(
              'w-full pl-8 pr-3 py-1.5 rounded-md text-xs',
              'bg-gray-50 dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'text-gray-700 dark:text-gray-300',
              'placeholder:text-gray-400 dark:placeholder:text-gray-500',
              'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent',
              memoryViewMode === 'pending' && 'opacity-60 cursor-not-allowed',
            )}
          />
        </div>

        {/* Category filter + action buttons */}
        {memoryViewMode !== 'pending' ? (
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
            {memoryViewMode === 'active' ? (
              <>
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
              </>
            ) : memoryViewMode === 'all' ? (
              <span className="ml-auto text-2xs text-gray-500 dark:text-gray-400">
                {t('skillPanel.pendingJumpHint', {
                  count: pendingMemoryCandidates.length,
                  defaultValue: '{{count}} pending candidates are managed in the Pending tab.',
                })}
              </span>
            ) : (
              <>
                <button
                  onClick={handleToggleSelectAllPersisted}
                  disabled={memories.length === 0}
                  className={clsx(
                    'ml-auto px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                    currentPersistedSelectionCount >= memories.length && memories.length > 0
                      ? 'bg-rose-100 dark:bg-rose-900/30 text-rose-700 dark:text-rose-300'
                      : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
                  )}
                >
                  {t('skillPanel.selectAll', { defaultValue: 'Select All' })}
                </button>
                {memoryViewMode === 'rejected' && (
                  <>
                    <button
                      onClick={() => void handleReviewPending(Array.from(selectedPersistedIds), 'restore')}
                      disabled={currentPersistedSelectionCount === 0}
                      className={clsx(
                        'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                        currentPersistedSelectionCount === 0
                          ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                          : 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300',
                      )}
                    >
                      {t('skillPanel.restoreSelected', { defaultValue: 'Restore Selected' })}
                    </button>
                    <button
                      onClick={() => void handleReviewPending(Array.from(selectedPersistedIds), 'approve')}
                      disabled={currentPersistedSelectionCount === 0}
                      className={clsx(
                        'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                        currentPersistedSelectionCount === 0
                          ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                          : 'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300',
                      )}
                    >
                      {t('skillPanel.approveSelected', { defaultValue: 'Approve Selected' })}
                    </button>
                    <button
                      onClick={() => void handleReviewPending(Array.from(selectedPersistedIds), 'archive')}
                      disabled={currentPersistedSelectionCount === 0}
                      className={clsx(
                        'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                        currentPersistedSelectionCount === 0
                          ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                          : 'bg-slate-100 dark:bg-slate-900/30 text-slate-700 dark:text-slate-300',
                      )}
                    >
                      {t('skillPanel.archiveSelected', { defaultValue: 'Archive Selected' })}
                    </button>
                  </>
                )}
                {memoryViewMode === 'archived' && (
                  <>
                    <button
                      onClick={() =>
                        workspacePath && void setMemoryStatus(workspacePath, Array.from(selectedPersistedIds), 'active')
                      }
                      disabled={currentPersistedSelectionCount === 0}
                      className={clsx(
                        'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                        currentPersistedSelectionCount === 0
                          ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                          : 'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300',
                      )}
                    >
                      {t('skillPanel.restoreSelected', { defaultValue: 'Restore Selected' })}
                    </button>
                    <button
                      onClick={() =>
                        workspacePath &&
                        void setMemoryStatus(workspacePath, Array.from(selectedPersistedIds), 'deleted')
                      }
                      disabled={currentPersistedSelectionCount === 0}
                      className={clsx(
                        'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                        currentPersistedSelectionCount === 0
                          ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                          : 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
                      )}
                    >
                      {t('skillPanel.moveToRecycleBin', { defaultValue: 'Move to Recycle Bin' })}
                    </button>
                  </>
                )}
                {memoryViewMode === 'deleted' && (
                  <>
                    <button
                      onClick={() =>
                        workspacePath && void restoreDeletedMemories(workspacePath, Array.from(selectedPersistedIds))
                      }
                      disabled={currentPersistedSelectionCount === 0}
                      className={clsx(
                        'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                        currentPersistedSelectionCount === 0
                          ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                          : 'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300',
                      )}
                    >
                      {t('skillPanel.restoreSelected', { defaultValue: 'Restore Selected' })}
                    </button>
                    <button
                      onClick={() =>
                        workspacePath && void purgeMemories(workspacePath, Array.from(selectedPersistedIds))
                      }
                      disabled={currentPersistedSelectionCount === 0}
                      className={clsx(
                        'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                        currentPersistedSelectionCount === 0
                          ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                          : 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
                      )}
                    >
                      {t('skillPanel.purgeSelected', { defaultValue: 'Delete Permanently' })}
                    </button>
                  </>
                )}
              </>
            )}
            <button
              onClick={() => void handleRunMaintenance()}
              className={clsx(
                'p-1 rounded-md transition-colors',
                'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                'hover:bg-gray-100 dark:hover:bg-gray-800',
              )}
              title={t('skillPanel.runMaintenance', { defaultValue: 'Run maintenance' })}
            >
              <ReloadIcon className="w-3.5 h-3.5" />
            </button>
          </div>
        ) : (
          <div className="flex items-center gap-1 flex-wrap">
            <button
              onClick={handleToggleSelectAllPending}
              disabled={pendingMemoryCandidates.length === 0}
              className={clsx(
                'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                selectedPendingIds.size >= pendingMemoryCandidates.length && pendingMemoryCandidates.length > 0
                  ? 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300'
                  : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
              )}
            >
              {t('skillPanel.selectAll', { defaultValue: 'Select All' })}
            </button>
            <button
              onClick={() => void handleReviewPending(Array.from(selectedPendingIds), 'approve')}
              disabled={selectedPendingIds.size === 0}
              className={clsx(
                'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                selectedPendingIds.size === 0
                  ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                  : 'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300',
              )}
            >
              {t('skillPanel.approveSelected', { defaultValue: 'Approve Selected' })}
            </button>
            <button
              onClick={() => void handleReviewPending(Array.from(selectedPendingIds), 'reject')}
              disabled={selectedPendingIds.size === 0}
              className={clsx(
                'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                selectedPendingIds.size === 0
                  ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                  : 'bg-rose-100 dark:bg-rose-900/30 text-rose-700 dark:text-rose-300',
              )}
            >
              {t('skillPanel.rejectSelected', { defaultValue: 'Reject Selected' })}
            </button>
            <button
              onClick={() => void handleReviewPending(Array.from(selectedPendingIds), 'archive')}
              disabled={selectedPendingIds.size === 0}
              className={clsx(
                'px-2 py-1 rounded-md text-2xs font-medium transition-colors',
                selectedPendingIds.size === 0
                  ? 'text-gray-300 dark:text-gray-600 cursor-not-allowed'
                  : 'bg-slate-100 dark:bg-slate-900/30 text-slate-700 dark:text-slate-300',
              )}
            >
              {t('skillPanel.archiveSelected', { defaultValue: 'Archive Selected' })}
            </button>
            <button
              onClick={() => workspacePath && void loadPendingMemoryCandidates(workspacePath)}
              className={clsx(
                'ml-auto p-1 rounded-md transition-colors',
                'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                'hover:bg-gray-100 dark:hover:bg-gray-800',
              )}
              title={t('skillPanel.refresh', { defaultValue: 'Refresh' })}
            >
              <ReloadIcon className="w-3.5 h-3.5" />
            </button>
          </div>
        )}
      </div>

      {/* Stats bar */}
      {memoryViewMode !== 'pending' && memoryStats && (
        <div className="px-3 py-1.5 border-b border-gray-200 dark:border-gray-700 flex items-center gap-2 text-2xs text-gray-500 dark:text-gray-400">
          <span>{t('skillPanel.totalMemories', { count: memoryStats.total_count })}</span>
          <span className="text-gray-300 dark:text-gray-600">|</span>
          <span>{t('skillPanel.avgImportance', { pct: (memoryStats.avg_importance * 100).toFixed(0) })}</span>
          {memoryViewMode === 'all' && (
            <>
              <span className="text-gray-300 dark:text-gray-600">|</span>
              <span>
                {t('skillPanel.activeMemories', { defaultValue: 'Active' })}: {memoryStats.status_counts.active ?? 0}
              </span>
              <span>
                {t('skillPanel.rejectedMemories', { defaultValue: 'Rejected' })}:{' '}
                {memoryStats.status_counts.rejected ?? 0}
              </span>
              <span>
                {t('skillPanel.archivedMemories', { defaultValue: 'Archived' })}:{' '}
                {memoryStats.status_counts.archived ?? 0}
              </span>
              <span>
                {t('skillPanel.deletedMemories', { defaultValue: 'Recycle Bin' })}:{' '}
                {memoryStats.status_counts.deleted ?? 0}
              </span>
            </>
          )}
          {Object.entries(memoryStats.category_counts).map(([cat, count]) => (
            <span key={cat} className="text-gray-400 dark:text-gray-500">
              {getCategoryLabel(cat)}: {count as number}
            </span>
          ))}
        </div>
      )}

      {memoryViewMode === 'pending' && (
        <div className="px-3 py-1.5 border-b border-gray-200 dark:border-gray-700 flex items-center gap-2 text-2xs text-gray-500 dark:text-gray-400">
          <span>
            {t('skillPanel.pendingReviewCount', { defaultValue: 'Pending' })}: {pendingMemoryCandidates.length}
          </span>
          <span className="text-gray-300 dark:text-gray-600">|</span>
          <span>
            {t('skillPanel.selectedCount', { defaultValue: 'Selected' })}: {selectedPendingIds.size}
          </span>
          <span className="text-gray-300 dark:text-gray-600">|</span>
          <span>
            {t('skillPanel.conflictCount', { defaultValue: 'Conflicts' })}:{' '}
            {pendingMemoryCandidates.filter((candidate) => candidate.conflict_flag).length}
          </span>
        </div>
      )}

      {isPersistedSelectionView && (
        <div className="px-3 py-1.5 border-b border-gray-200 dark:border-gray-700 flex items-center gap-2 text-2xs text-gray-500 dark:text-gray-400">
          <span>
            {memoryViewMode === 'rejected'
              ? t('skillPanel.rejectedMemories', { defaultValue: 'Rejected' })
              : memoryViewMode === 'archived'
                ? t('skillPanel.archivedMemories', { defaultValue: 'Archived' })
                : t('skillPanel.deletedMemories', { defaultValue: 'Recycle Bin' })}
            : {memories.length}
          </span>
          <span className="text-gray-300 dark:text-gray-600">|</span>
          <span>
            {t('skillPanel.selectedCount', { defaultValue: 'Selected' })}: {currentPersistedSelectionCount}
          </span>
        </div>
      )}

      {/* Memory list */}
      <div className="flex-1 overflow-y-auto">
        {memoryViewMode !== 'pending' && memoriesLoading && memories.length === 0 ? (
          <div className="flex items-center justify-center py-8">
            <span className="text-xs text-gray-400">{t('skillPanel.loading')}</span>
          </div>
        ) : (memoryViewMode === 'active' || memoryViewMode === 'all') && memories.length === 0 ? (
          <EmptyState
            title={
              memoryViewMode === 'all'
                ? t('skillPanel.noPersistedMemories', { defaultValue: 'No persisted memories' })
                : t('skillPanel.noMemoriesFound')
            }
            description={
              memoryViewMode === 'all'
                ? t('skillPanel.noPersistedMemoriesHint', {
                    defaultValue: 'Archived, rejected, and deleted memories will appear here with active memories.',
                  })
                : t('skillPanel.noMemoriesFoundHint')
            }
            action={
              memoryViewMode === 'active'
                ? { label: t('skillPanel.addMemory'), onClick: () => setShowAddForm(true) }
                : undefined
            }
          />
        ) : memoryViewMode === 'rejected' && memories.length === 0 ? (
          <EmptyState
            title={t('skillPanel.noRejectedMemories', { defaultValue: 'No rejected memories' })}
            description={t('skillPanel.noRejectedMemoriesHint', {
              defaultValue: 'Rejected memories will stay here until you restore or archive them.',
            })}
          />
        ) : memoryViewMode === 'archived' && memories.length === 0 ? (
          <EmptyState
            title={t('skillPanel.noArchivedMemories', { defaultValue: 'No archived memories' })}
            description={t('skillPanel.noArchivedMemoriesHint', {
              defaultValue: 'Archived memories can be restored to active or moved to the recycle bin.',
            })}
          />
        ) : memoryViewMode === 'deleted' && memories.length === 0 ? (
          <EmptyState
            title={t('skillPanel.noDeletedMemories', { defaultValue: 'Recycle bin is empty' })}
            description={t('skillPanel.noDeletedMemoriesHint', {
              defaultValue: 'Deleted memories stay here until you restore or permanently delete them.',
            })}
          />
        ) : memoryViewMode === 'active' || memoryViewMode === 'all' ? (
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
                  {memoryViewMode === 'all' && memory.status && memory.status !== 'active' && (
                    <span
                      className={clsx(
                        'text-2xs px-1 py-0.5 rounded shrink-0',
                        memory.status === 'rejected'
                          ? 'bg-rose-100 text-rose-700 dark:bg-rose-900/30 dark:text-rose-300'
                          : memory.status === 'archived'
                            ? 'bg-slate-100 text-slate-700 dark:bg-slate-900/30 dark:text-slate-300'
                            : 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300',
                      )}
                    >
                      {memory.status === 'rejected'
                        ? t('skillPanel.rejectedMemories', { defaultValue: 'Rejected' })
                        : memory.status === 'archived'
                          ? t('skillPanel.archivedMemories', { defaultValue: 'Archived' })
                          : t('skillPanel.deletedMemories', { defaultValue: 'Recycle Bin' })}
                    </span>
                  )}
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
        ) : isPersistedSelectionView ? (
          <div className="divide-y divide-gray-100 dark:divide-gray-800">
            {memories.map((memory) => {
              const checked = selectedPersistedIds.has(memory.id);
              return (
                <div key={memory.id} className="px-4 py-3">
                  <div className="flex items-start gap-2">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => handleTogglePersistedSelection(memory.id)}
                      className="mt-0.5 w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-primary-600 focus:ring-primary-500"
                    />
                    <button className="min-w-0 flex-1 text-left" onClick={() => setSelectedMemory(memory)}>
                      <div className="flex items-center gap-1.5 mb-1 flex-wrap">
                        <CategoryBadge category={memory.category} compact />
                        <span
                          className={clsx(
                            'text-2xs px-1 py-0.5 rounded',
                            memoryViewMode === 'rejected'
                              ? 'bg-rose-100 text-rose-700 dark:bg-rose-900/30 dark:text-rose-300'
                              : memoryViewMode === 'archived'
                                ? 'bg-slate-100 text-slate-700 dark:bg-slate-900/30 dark:text-slate-300'
                                : 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300',
                          )}
                        >
                          {memoryViewMode === 'rejected'
                            ? t('skillPanel.rejectedMemories', { defaultValue: 'Rejected' })
                            : memoryViewMode === 'archived'
                              ? t('skillPanel.archivedMemories', { defaultValue: 'Archived' })
                              : t('skillPanel.deletedMemories', { defaultValue: 'Recycle Bin' })}
                        </span>
                        <span className="ml-auto text-2xs text-gray-400 dark:text-gray-500 shrink-0">
                          {new Date(memory.updated_at).toLocaleDateString()}
                        </span>
                      </div>
                      <p className="text-xs text-gray-700 dark:text-gray-300 line-clamp-2">{memory.content}</p>
                    </button>
                    <div className="flex flex-col gap-1">
                      {memoryViewMode === 'rejected' && (
                        <>
                          <button
                            onClick={() => void handleReviewPending([memory.id], 'restore')}
                            className="px-2 py-0.5 rounded text-2xs font-medium bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300"
                          >
                            {t('skillPanel.restore', { defaultValue: 'Restore to Review' })}
                          </button>
                          <button
                            onClick={() => void handleReviewPending([memory.id], 'approve')}
                            className="px-2 py-0.5 rounded text-2xs font-medium bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300"
                          >
                            {t('skillPanel.approve', { defaultValue: 'Approve' })}
                          </button>
                          <button
                            onClick={() => void handleReviewPending([memory.id], 'archive')}
                            className="px-2 py-0.5 rounded text-2xs font-medium bg-slate-100 text-slate-700 dark:bg-slate-900/30 dark:text-slate-300"
                          >
                            {t('skillPanel.archive', { defaultValue: 'Archive' })}
                          </button>
                        </>
                      )}
                      {memoryViewMode === 'archived' && (
                        <>
                          <button
                            onClick={() => workspacePath && void setMemoryStatus(workspacePath, [memory.id], 'active')}
                            className="px-2 py-0.5 rounded text-2xs font-medium bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300"
                          >
                            {t('skillPanel.restoreSelected', { defaultValue: 'Restore Selected' })}
                          </button>
                          <button
                            onClick={() => workspacePath && void setMemoryStatus(workspacePath, [memory.id], 'deleted')}
                            className="px-2 py-0.5 rounded text-2xs font-medium bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300"
                          >
                            {t('skillPanel.moveToRecycleBin', { defaultValue: 'Move to Recycle Bin' })}
                          </button>
                        </>
                      )}
                      {memoryViewMode === 'deleted' && (
                        <>
                          <button
                            onClick={() => workspacePath && void restoreDeletedMemories(workspacePath, [memory.id])}
                            className="px-2 py-0.5 rounded text-2xs font-medium bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300"
                          >
                            {t('skillPanel.restoreSelected', { defaultValue: 'Restore Selected' })}
                          </button>
                          <button
                            onClick={() => workspacePath && void purgeMemories(workspacePath, [memory.id])}
                            className="px-2 py-0.5 rounded text-2xs font-medium bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300"
                          >
                            {t('skillPanel.purgeSelected', { defaultValue: 'Delete Permanently' })}
                          </button>
                        </>
                      )}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        ) : pendingMemoryCandidatesLoading && pendingMemoryCandidates.length === 0 ? (
          <div className="flex items-center justify-center py-8">
            <span className="text-xs text-gray-400">{t('skillPanel.loading')}</span>
          </div>
        ) : pendingMemoryCandidates.length === 0 ? (
          <EmptyState
            title={t('skillPanel.noPendingReview', { defaultValue: 'No pending review memories' })}
            description={t('skillPanel.noPendingReviewHint', {
              defaultValue: 'New inferred memories will appear here.',
            })}
          />
        ) : (
          <div className="divide-y divide-gray-100 dark:divide-gray-800">
            {pendingMemoryCandidates.map((candidate) => {
              const checked = selectedPendingIds.has(candidate.id);
              const reference = conflictReference(candidate);
              return (
                <div key={candidate.id} className="px-4 py-3">
                  <div className="flex items-start gap-2">
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => handleTogglePendingSelection(candidate.id)}
                      className="mt-0.5 w-3.5 h-3.5 rounded border-gray-300 dark:border-gray-600 text-primary-600 focus:ring-primary-500"
                    />
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-1.5 mb-1 flex-wrap">
                        <CategoryBadge category={candidate.category} compact />
                        <span className="text-2xs px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300">
                          {candidate.scope}
                        </span>
                        <span
                          className={clsx(
                            'text-2xs px-1 py-0.5 rounded',
                            candidate.risk_tier === 'high'
                              ? 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-300'
                              : candidate.risk_tier === 'medium'
                                ? 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300'
                                : 'bg-sky-100 text-sky-700 dark:bg-sky-900/30 dark:text-sky-300',
                          )}
                        >
                          {candidate.risk_tier}
                        </span>
                        {candidate.conflict_flag && (
                          <span className="text-2xs px-1 py-0.5 rounded bg-rose-100 text-rose-700 dark:bg-rose-900/30 dark:text-rose-300">
                            {t('skillPanel.conflictFlag', { defaultValue: 'Conflict' })}
                          </span>
                        )}
                        <span className="ml-auto text-2xs text-gray-400 dark:text-gray-500 shrink-0">
                          {new Date(candidate.updated_at).toLocaleDateString()}
                        </span>
                      </div>

                      <p className="text-xs text-gray-700 dark:text-gray-300">{candidate.content}</p>

                      {candidate.keywords.length > 0 && (
                        <div className="flex gap-1 mt-1.5 flex-wrap">
                          {candidate.keywords.slice(0, 4).map((kw) => (
                            <span
                              key={kw}
                              className="text-2xs px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400"
                            >
                              {kw}
                            </span>
                          ))}
                        </div>
                      )}

                      {candidate.conflict_flag && (
                        <div className="mt-2 grid grid-cols-1 gap-2">
                          <div className="rounded-md border border-amber-200 dark:border-amber-800 bg-amber-50/70 dark:bg-amber-900/10 p-2">
                            <p className="text-2xs font-medium text-amber-700 dark:text-amber-300 mb-0.5">
                              {t('skillPanel.pendingCandidate', { defaultValue: 'Candidate' })}
                            </p>
                            <p className="text-2xs text-amber-700/90 dark:text-amber-200">{candidate.content}</p>
                          </div>
                          <div className="rounded-md border border-sky-200 dark:border-sky-800 bg-sky-50/70 dark:bg-sky-900/10 p-2">
                            <p className="text-2xs font-medium text-sky-700 dark:text-sky-300 mb-0.5">
                              {t('skillPanel.activeReference', { defaultValue: 'Active Reference' })}
                            </p>
                            <p className="text-2xs text-sky-700/90 dark:text-sky-200">
                              {reference
                                ? reference.content
                                : t('skillPanel.noActiveReference', {
                                    defaultValue: 'No active reference loaded in this scope/category.',
                                  })}
                            </p>
                          </div>
                        </div>
                      )}
                    </div>

                    <div className="flex flex-col gap-1">
                      <button
                        onClick={() => void handleReviewPending([candidate.id], 'approve')}
                        className="px-2 py-0.5 rounded text-2xs font-medium bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300"
                      >
                        {t('skillPanel.approve', { defaultValue: 'Approve' })}
                      </button>
                      <button
                        onClick={() => void handleReviewPending([candidate.id], 'reject')}
                        className="px-2 py-0.5 rounded text-2xs font-medium bg-rose-100 text-rose-700 dark:bg-rose-900/30 dark:text-rose-300"
                      >
                        {t('skillPanel.reject', { defaultValue: 'Reject' })}
                      </button>
                      <button
                        onClick={() => void handleReviewPending([candidate.id], 'archive')}
                        className="px-2 py-0.5 rounded text-2xs font-medium bg-slate-100 text-slate-700 dark:bg-slate-900/30 dark:text-slate-300"
                      >
                        {t('skillPanel.archive', { defaultValue: 'Archive' })}
                      </button>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {/* Load more */}
        {memoryViewMode !== 'pending' && memoryHasMore && memories.length > 0 && (
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
  const loadPendingMemoryCandidates = useSkillMemoryStore((s) => s.loadPendingMemoryCandidates);

  // Load data when dialog opens
  useEffect(() => {
    if (dialogOpen && workspacePath) {
      loadSkills(workspacePath);
      loadMemories(workspacePath);
      loadMemoryStats(workspacePath);
      loadPendingMemoryCandidates(workspacePath);
    }
  }, [dialogOpen, workspacePath, loadSkills, loadMemories, loadMemoryStats, loadPendingMemoryCandidates]);

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
