import { clsx } from 'clsx';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useContextSourcesStore } from '../../store/contextSources';
import { useContextOpsStore } from '../../store/contextOps';
import { useSkillMemoryStore } from '../../store/skillMemory';

export function EffectiveContextSummary({
  className,
  dropdownDirection = 'up',
}: {
  className?: string;
  dropdownDirection?: 'up' | 'down';
}) {
  const { t } = useTranslation('simpleMode');
  const openDialog = useSkillMemoryStore((s) => s.openDialog);
  const latestEnvelope = useContextOpsStore((s) => s.latestEnvelope);
  const diagnostics = latestEnvelope?.diagnostics;
  const [expanded, setExpanded] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const knowledgeEnabled = useContextSourcesStore((s) => s.knowledgeEnabled);
  const selectedCollections = useContextSourcesStore((s) => s.selectedCollections);
  const selectedDocuments = useContextSourcesStore((s) => s.selectedDocuments);
  const memoryEnabled = useContextSourcesStore((s) => s.memoryEnabled);
  const selectedMemoryScopes = useContextSourcesStore((s) => s.selectedMemoryScopes);
  const memorySelectionMode = useContextSourcesStore((s) => s.memorySelectionMode);
  const excludedMemoryIds = useContextSourcesStore((s) => s.excludedMemoryIds);
  const includedMemoryIds = useContextSourcesStore((s) => s.includedMemoryIds);
  const skillsEnabled = useContextSourcesStore((s) => s.skillsEnabled);
  const selectedSkillIds = useContextSourcesStore((s) => s.selectedSkillIds);
  const invokedSkillIds = useContextSourcesStore((s) => s.invokedSkillIds);
  const skills = useSkillMemoryStore((s) => s.skills);

  const knowledgeCount = selectedCollections.length + selectedDocuments.length;
  const memoryScopeCount = selectedMemoryScopes.length;
  const memoryMode = memorySelectionMode === 'only_selected' ? 'only_selected' : 'auto_exclude';
  const memoryModeLabel =
    memoryMode === 'only_selected'
      ? t('contextSummary.memoryMode.only_selected', { defaultValue: 'only selected' })
      : t('contextSummary.memoryMode.auto_exclude', { defaultValue: 'auto exclude' });
  const memoryItemCount = memorySelectionMode === 'only_selected' ? includedMemoryIds.length : excludedMemoryIds.length;

  const skillsCount = selectedSkillIds.length + invokedSkillIds.length;
  const currentSkillStack = useMemo(() => {
    if (!diagnostics?.effective_skill_ids?.length) return [];
    const byId = new Map(skills.map((skill) => [skill.id, skill]));
    return diagnostics.effective_skill_ids
      .map((id) => byId.get(id))
      .filter((skill): skill is NonNullable<typeof skill> => Boolean(skill));
  }, [diagnostics?.effective_skill_ids, skills]);

  const chipClass =
    'inline-flex items-center gap-1 rounded-md border border-gray-200 dark:border-gray-700 px-2 py-1 text-2xs text-gray-600 dark:text-gray-300';
  const effectiveContextCount = useMemo(() => {
    if (!latestEnvelope) return 0;

    const sourceKindsById = new Map((latestEnvelope.sources ?? []).map((source) => [source.id, source.kind] as const));
    const hasKnowledgeBlock = (latestEnvelope.blocks ?? []).some(
      (block) => sourceKindsById.get(block.source_id) === 'knowledge',
    );
    const hasMemoryBlock = (latestEnvelope.blocks ?? []).some(
      (block) => sourceKindsById.get(block.source_id) === 'memory',
    );
    const knowledgeCount = hasKnowledgeBlock ? 1 : 0;
    const memoryCount = Math.max(diagnostics?.effective_memory_ids?.length ?? 0, hasMemoryBlock ? 1 : 0);
    const skillCount = diagnostics?.effective_skill_ids?.length ?? 0;
    return knowledgeCount + memoryCount + skillCount;
  }, [diagnostics?.effective_memory_ids, diagnostics?.effective_skill_ids, latestEnvelope]);

  useEffect(() => {
    if (!expanded) return;
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setExpanded(false);
      }
    };
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setExpanded(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEscape);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [expanded]);

  return (
    <div className={clsx('relative', className)} ref={containerRef}>
      <button
        onClick={() => setExpanded((value) => !value)}
        className={clsx(
          'flex items-center gap-1 px-2 py-1 rounded text-[11px] font-medium transition-colors',
          'text-gray-600 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
          'border border-gray-200 dark:border-gray-700',
        )}
        data-testid={expanded ? 'effective-context-summary-expanded' : 'effective-context-summary-collapsed'}
      >
        <span>{t('contextSummary.buttonLabel', { defaultValue: 'Context' })}</span>
        <span className="rounded bg-gray-100 px-1 py-0.5 text-[10px] text-gray-500 dark:bg-gray-800 dark:text-gray-300">
          {effectiveContextCount}
        </span>
      </button>

      {expanded && (
        <div
          className={clsx(
            'absolute right-0 w-[360px] max-w-[calc(100vw-2rem)] rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg z-50 px-3 py-2',
            dropdownDirection === 'up' ? 'bottom-full mb-1' : 'top-full mt-1',
            'animate-in fade-in-0 zoom-in-95 duration-150',
          )}
          data-testid="effective-context-summary"
        >
          <div className="flex items-center justify-between gap-2">
            <p className="text-2xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">
              {t('contextSummary.panelTitle', { defaultValue: 'Active Context' })}
            </p>
            <button
              onClick={() => openDialog('skills')}
              className="text-2xs text-primary-600 dark:text-primary-400 hover:underline"
            >
              {t('contextSummary.openPanel', { defaultValue: 'Inspect' })}
            </button>
          </div>

          <div className="mt-2 flex flex-wrap gap-1.5">
            <span className={chipClass}>
              K:{' '}
              {knowledgeEnabled
                ? t('contextSummary.knowledgeCount', {
                    defaultValue: '{{count}} selected',
                    count: knowledgeCount,
                  })
                : t('contextSummary.off', { defaultValue: 'off' })}
            </span>
            <span className={chipClass}>
              M:{' '}
              {memoryEnabled
                ? t('contextSummary.memoryDetail', {
                    defaultValue: '{{mode}} · {{scopeCount}} scopes · {{itemCount}} items',
                    mode: memoryModeLabel,
                    scopeCount: memoryScopeCount,
                    itemCount: memoryItemCount,
                  })
                : t('contextSummary.off', { defaultValue: 'off' })}
            </span>
            <span className={chipClass}>
              S:{' '}
              {skillsEnabled
                ? t('contextSummary.skillsCount', {
                    defaultValue: '{{count}} selected/pinned',
                    count: skillsCount,
                  })
                : t('contextSummary.off', { defaultValue: 'off' })}
            </span>
            {diagnostics?.selection_origin && (
              <span className={chipClass}>
                {t('contextSummary.origin', {
                  defaultValue: 'origin: {{origin}}',
                  origin: t(`contextSummary.originValues.${diagnostics.selection_origin}`, {
                    defaultValue: diagnostics.selection_origin,
                  }),
                })}
              </span>
            )}
            {diagnostics?.blocked_tools?.length ? (
              <span
                className={clsx(chipClass, 'border-amber-200 text-amber-700 dark:border-amber-800 dark:text-amber-300')}
              >
                {t('contextSummary.blockedTools', {
                  defaultValue: '{{count}} blocked tools',
                  count: diagnostics.blocked_tools.length,
                })}
              </span>
            ) : null}
          </div>

          {(currentSkillStack.length > 0 || invokedSkillIds.length > 0 || diagnostics?.selection_reason) && (
            <div className="mt-3 space-y-2 border-t border-gray-200 dark:border-gray-700 pt-2">
              <div className="flex items-center justify-between gap-2">
                <p className="text-2xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">
                  {t('contextSummary.currentSkillStack', { defaultValue: 'Current Skill Stack' })}
                </p>
                <button
                  onClick={() => openDialog('skills')}
                  className="text-2xs text-primary-600 dark:text-primary-400 hover:underline"
                >
                  {t('contextSummary.inspectSkills', { defaultValue: 'Inspect' })}
                </button>
              </div>
              {currentSkillStack.length > 0 ? (
                <div className="flex flex-wrap gap-1.5">
                  {currentSkillStack.map((skill) => (
                    <span key={skill.id} className={chipClass}>
                      {skill.name}
                      {invokedSkillIds.includes(skill.id) && (
                        <span className="rounded bg-sky-100 px-1 py-0.5 text-2xs text-sky-700 dark:bg-sky-900/20 dark:text-sky-300">
                          {t('contextSummary.pinned', { defaultValue: 'pinned' })}
                        </span>
                      )}
                    </span>
                  ))}
                </div>
              ) : (
                <p className="text-2xs text-gray-500 dark:text-gray-400">
                  {t('contextSummary.noEffectiveSkills', {
                    defaultValue: 'No effective skills in the latest context run.',
                  })}
                </p>
              )}
              {diagnostics?.selection_reason && (
                <p className="text-2xs text-gray-500 dark:text-gray-400">
                  {t('contextSummary.selectionReason', {
                    defaultValue: 'reason: {{reason}}',
                    reason: t(`contextSummary.selectionReasons.${diagnostics.selection_reason}`, {
                      defaultValue: diagnostics.selection_reason,
                    }),
                  })}
                </p>
              )}
              {(diagnostics?.skill_router_used || diagnostics?.skill_router_fallback_reason) && (
                <p className="text-2xs text-gray-500 dark:text-gray-400">
                  {t('contextSummary.routerStatus', {
                    defaultValue: 'router: {{strategy}}',
                    strategy: t(`contextSummary.routerStrategies.${diagnostics?.skill_router_strategy ?? 'hybrid'}`, {
                      defaultValue: diagnostics?.skill_router_strategy ?? 'hybrid',
                    }),
                  })}
                  {typeof diagnostics?.skill_router_confidence === 'number'
                    ? ` · ${t('contextSummary.routerConfidence', {
                        defaultValue: 'confidence {{confidence}}',
                        confidence: diagnostics.skill_router_confidence.toFixed(2),
                      })}`
                    : ''}
                  {diagnostics?.skill_router_fallback_reason
                    ? ` · ${t('contextSummary.routerFallback', {
                        defaultValue: 'fallback: {{reason}}',
                        reason: t(`contextSummary.routerFallbackReasons.${diagnostics.skill_router_fallback_reason}`, {
                          defaultValue: diagnostics.skill_router_fallback_reason,
                        }),
                      })}`
                    : ''}
                </p>
              )}
              {diagnostics?.skill_router_reason && (
                <p className="text-2xs text-gray-500 dark:text-gray-400">
                  {t('contextSummary.routerReason', {
                    defaultValue: 'router reason: {{reason}}',
                    reason: diagnostics.skill_router_reason,
                  })}
                </p>
              )}
              {diagnostics?.blocked_tools?.length ? (
                <p className="text-2xs text-amber-700 dark:text-amber-300">
                  {t('contextSummary.blockedToolsList', {
                    defaultValue: 'blocked: {{tools}}',
                    tools: diagnostics.blocked_tools.join(', '),
                  })}
                </p>
              ) : null}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default EffectiveContextSummary;
