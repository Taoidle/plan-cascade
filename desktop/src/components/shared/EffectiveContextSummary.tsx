import { clsx } from 'clsx';
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

  const legacy = useContextSourcesStore((s) => ({
    knowledgeEnabled: s.knowledgeEnabled,
    selectedCollections: s.selectedCollections,
    selectedDocuments: s.selectedDocuments,
    memoryEnabled: s.memoryEnabled,
    selectedMemoryScopes: s.selectedMemoryScopes,
    memorySelectionMode: s.memorySelectionMode,
    excludedMemoryIds: s.excludedMemoryIds,
    includedMemoryIds: s.includedMemoryIds,
    skillsEnabled: s.skillsEnabled,
    selectedSkillIds: s.selectedSkillIds,
  }));

  const unifiedSelection = useContextSelectionStore((s) => ({
    knowledge: s.knowledge,
    memory: s.memory,
    skills: s.skills,
  }));

  const sourceLabel = unified ? 'unified' : 'legacy';

  const knowledgeCount = unified
    ? unifiedSelection.knowledge.selectedCollections.length + unifiedSelection.knowledge.selectedDocuments.length
    : legacy.selectedCollections.length + legacy.selectedDocuments.length;

  const memoryScopeCount = unified ? unifiedSelection.memory.selectedScopes.length : legacy.selectedMemoryScopes.length;
  const memoryMode = unified
    ? unifiedSelection.memory.selectionMode
    : legacy.memorySelectionMode === 'only_selected'
      ? 'only_selected'
      : 'auto_exclude';
  const memoryItemCount = unified
    ? memoryMode === 'only_selected'
      ? unifiedSelection.memory.includedMemoryIds.length
      : unifiedSelection.memory.excludedMemoryIds.length
    : legacy.memorySelectionMode === 'only_selected'
      ? legacy.includedMemoryIds.length
      : legacy.excludedMemoryIds.length;

  const skillsCount = unified ? unifiedSelection.skills.selectedSkillIds.length : legacy.selectedSkillIds.length;

  const knowledgeEnabled = unified ? unifiedSelection.knowledge.enabled : legacy.knowledgeEnabled;
  const memoryEnabled = unified ? unifiedSelection.memory.enabled : legacy.memoryEnabled;
  const skillsEnabled = unified ? unifiedSelection.skills.enabled : legacy.skillsEnabled;

  const chipClass =
    'inline-flex items-center gap-1 rounded-md border border-gray-200 dark:border-gray-700 px-2 py-1 text-2xs text-gray-600 dark:text-gray-300';

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
            ? `${memoryMode} · ${memoryScopeCount} scopes · ${memoryItemCount} items`
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
        {diagnostics?.selection_origin && <span className={chipClass}>origin: {diagnostics.selection_origin}</span>}
      </div>
    </div>
  );
}

export default EffectiveContextSummary;
