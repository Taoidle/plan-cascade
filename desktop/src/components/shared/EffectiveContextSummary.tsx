import { clsx } from 'clsx';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../../store/settings';
import { useContextSourcesStore } from '../../store/contextSources';
import { useContextSelectionStore } from '../../store/contextSelection';
import { useContextOpsStore } from '../../store/contextOps';
import { useSkillMemoryStore } from '../../store/skillMemory';

export function EffectiveContextSummary({ className }: { className?: string }) {
  const { t } = useTranslation('simpleMode');
  const unified = useSettingsStore((s) => s.simpleContextUnifiedStore);
  const openDialog = useSkillMemoryStore((s) => s.openDialog);
  const diagnostics = useContextOpsStore((s) => s.latestEnvelope?.diagnostics);
  const [expanded, setExpanded] = useState(false);

  const legacyKnowledgeEnabled = useContextSourcesStore((s) => s.knowledgeEnabled);
  const legacySelectedCollections = useContextSourcesStore((s) => s.selectedCollections);
  const legacySelectedDocuments = useContextSourcesStore((s) => s.selectedDocuments);
  const legacyMemoryEnabled = useContextSourcesStore((s) => s.memoryEnabled);
  const legacySelectedMemoryScopes = useContextSourcesStore((s) => s.selectedMemoryScopes);
  const legacyMemorySelectionMode = useContextSourcesStore((s) => s.memorySelectionMode);
  const legacyExcludedMemoryIds = useContextSourcesStore((s) => s.excludedMemoryIds);
  const legacyIncludedMemoryIds = useContextSourcesStore((s) => s.includedMemoryIds);
  const legacySkillsEnabled = useContextSourcesStore((s) => s.skillsEnabled);
  const legacySelectedSkillIds = useContextSourcesStore((s) => s.selectedSkillIds);

  const unifiedKnowledge = useContextSelectionStore((s) => s.knowledge);
  const unifiedMemory = useContextSelectionStore((s) => s.memory);
  const unifiedSkills = useContextSelectionStore((s) => s.skills);

  const sourceLabel = unified
    ? t('contextSummary.source.unified', { defaultValue: 'unified' })
    : t('contextSummary.source.legacy', { defaultValue: 'legacy' });

  const knowledgeCount = unified
    ? unifiedKnowledge.selectedCollections.length + unifiedKnowledge.selectedDocuments.length
    : legacySelectedCollections.length + legacySelectedDocuments.length;

  const memoryScopeCount = unified ? unifiedMemory.selectedScopes.length : legacySelectedMemoryScopes.length;
  const memoryMode = unified
    ? unifiedMemory.selectionMode
    : legacyMemorySelectionMode === 'only_selected'
      ? 'only_selected'
      : 'auto_exclude';
  const memoryModeLabel =
    memoryMode === 'only_selected'
      ? t('contextSummary.memoryMode.only_selected', { defaultValue: 'only selected' })
      : t('contextSummary.memoryMode.auto_exclude', { defaultValue: 'auto exclude' });
  const memoryItemCount = unified
    ? memoryMode === 'only_selected'
      ? unifiedMemory.includedMemoryIds.length
      : unifiedMemory.excludedMemoryIds.length
    : legacyMemorySelectionMode === 'only_selected'
      ? legacyIncludedMemoryIds.length
      : legacyExcludedMemoryIds.length;

  const skillsCount = unified ? unifiedSkills.selectedSkillIds.length : legacySelectedSkillIds.length;

  const knowledgeEnabled = unified ? unifiedKnowledge.enabled : legacyKnowledgeEnabled;
  const memoryEnabled = unified ? unifiedMemory.enabled : legacyMemoryEnabled;
  const skillsEnabled = unified ? unifiedSkills.enabled : legacySkillsEnabled;

  const chipClass =
    'inline-flex items-center gap-1 rounded-md border border-gray-200 dark:border-gray-700 px-2 py-1 text-2xs text-gray-600 dark:text-gray-300';

  if (!expanded) {
    return (
      <div
        className={clsx('flex items-center justify-end', className)}
        data-testid="effective-context-summary-collapsed"
      >
        <button
          onClick={() => setExpanded(true)}
          className="text-2xs text-gray-500 dark:text-gray-400 hover:text-primary-600 dark:hover:text-primary-400"
        >
          {t('contextSummary.show', { defaultValue: 'Show effective context' })}
        </button>
      </div>
    );
  }

  return (
    <div
      className={clsx(
        'rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50/80 dark:bg-gray-800/40 px-3 py-2',
        className,
      )}
      data-testid="effective-context-summary"
    >
      <div className="flex items-center justify-between gap-2">
        <p className="text-2xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">
          {t('contextSummary.title', { defaultValue: 'Effective Context' })}
        </p>
        <div className="flex items-center gap-2">
          <span className="text-2xs text-gray-400 dark:text-gray-500">{sourceLabel}</span>
          <button
            onClick={() => setExpanded(false)}
            className="text-2xs text-gray-500 dark:text-gray-400 hover:underline"
          >
            {t('contextSummary.hide', { defaultValue: 'Hide' })}
          </button>
          <button
            onClick={() => openDialog('skills')}
            className="text-2xs text-primary-600 dark:text-primary-400 hover:underline"
          >
            {t('contextSummary.why', { defaultValue: 'Why' })}
          </button>
        </div>
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
                defaultValue: '{{count}} selected',
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
      </div>
    </div>
  );
}

export default EffectiveContextSummary;
