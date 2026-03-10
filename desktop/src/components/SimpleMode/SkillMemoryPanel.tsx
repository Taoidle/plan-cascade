/**
 * SkillMemoryPanel Component
 *
 * Collapsible sidebar panel with three sections:
 * 1. Auto-Detected Skills (with toggles)
 * 2. Project Skills (with toggles)
 * 3. Memories (with category badges)
 *
 * Includes a "Manage All..." button to open the full dialog.
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { ChevronRightIcon, GearIcon } from '@radix-ui/react-icons';
import { useSkillMemoryStore } from '../../store/skillMemory';
import { useSettingsStore } from '../../store/settings';
import { useContextSourcesStore } from '../../store/contextSources';
import { useContextOpsStore } from '../../store/contextOps';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import { SkillRow } from './SkillRow';
import { MemoryRow } from './MemoryRow';
import type { SkillSummary } from '../../types/skillMemory';
import { Collapsible } from './Collapsible';

// ============================================================================
// CollapsibleSection
// ============================================================================

function CollapsibleSection({
  title,
  count,
  defaultExpanded = true,
  children,
}: {
  title: string;
  count: number;
  defaultExpanded?: boolean;
  children: React.ReactNode;
}) {
  const [expanded, setExpanded] = useState(defaultExpanded);

  return (
    <div>
      <button
        onClick={() => setExpanded((prev) => !prev)}
        className={clsx(
          'w-full flex items-center gap-1 px-2 py-1.5 rounded-md text-xs font-medium transition-colors',
          'text-gray-600 dark:text-gray-400',
          'hover:bg-gray-50 dark:hover:bg-gray-800',
        )}
      >
        <ChevronRightIcon
          className={clsx('w-3.5 h-3.5 shrink-0 transition-transform duration-200', expanded && 'rotate-90')}
        />
        <span className="flex-1 text-left">{title}</span>
        {count > 0 && (
          <span className="text-2xs px-1.5 py-0.5 rounded-full bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300 shrink-0">
            {count}
          </span>
        )}
      </button>
      <Collapsible open={expanded}>
        <div className="mt-0.5">{children}</div>
      </Collapsible>
    </div>
  );
}

// ============================================================================
// SkillMemoryPanel
// ============================================================================

export function SkillMemoryPanel() {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const rootSessionId = useWorkflowKernelStore((s) => s.sessionId);
  const invokedSkillIds = useContextSourcesStore((s) => s.invokedSkillIds);
  const latestEnvelope = useContextOpsStore((s) => s.latestEnvelope);

  const skills = useSkillMemoryStore((s) => s.skills);
  const skillsLoading = useSkillMemoryStore((s) => s.skillsLoading);
  const memories = useSkillMemoryStore((s) => s.memories);
  const memoriesLoading = useSkillMemoryStore((s) => s.memoriesLoading);
  const loadSkills = useSkillMemoryStore((s) => s.loadSkills);
  const loadMemories = useSkillMemoryStore((s) => s.loadMemories);
  const toggleSkill = useSkillMemoryStore((s) => s.toggleSkill);
  const toggleGeneratedSkill = useSkillMemoryStore((s) => s.toggleGeneratedSkill);
  const openDialog = useSkillMemoryStore((s) => s.openDialog);
  const memoryPipelineSnapshot = useSkillMemoryStore((s) =>
    rootSessionId ? (s.memoryPipelineByRootSession[rootSessionId] ?? null) : null,
  );

  // Fallback loading for direct panel usage (sidebar preloads on mount).
  useEffect(() => {
    if (!workspacePath) return;
    if (skills.length === 0) void loadSkills(workspacePath);
    if (memories.length === 0) void loadMemories(workspacePath);
  }, [workspacePath, skills.length, memories.length, loadSkills, loadMemories]);

  // Separate auto-detected skills from others
  const { activeSkills, detectedSkills, projectSkills, generatedSkills } = useMemo(() => {
    const effectiveIds = new Set(latestEnvelope?.diagnostics?.effective_skill_ids ?? []);
    const detected: SkillSummary[] = [];
    const project: SkillSummary[] = [];
    const active: SkillSummary[] = [];
    const generated: SkillSummary[] = [];
    for (const skill of skills) {
      if (effectiveIds.has(skill.id)) {
        active.push(skill);
      }
      if (skill.detected) {
        detected.push(skill);
      } else if (skill.source.type === 'generated') {
        generated.push(skill);
      } else {
        project.push(skill);
      }
    }
    return { activeSkills: active, detectedSkills: detected, projectSkills: project, generatedSkills: generated };
  }, [latestEnvelope?.diagnostics?.effective_skill_ids, skills]);

  const handleToggle = useCallback(
    (id: string, enabled: boolean) => {
      const skill = skills.find((entry) => entry.id === id);
      if (skill?.source.type === 'generated') {
        toggleGeneratedSkill(id, enabled);
        return;
      }
      toggleSkill(id, enabled);
    },
    [skills, toggleGeneratedSkill, toggleSkill],
  );

  const handleManageAll = useCallback(() => {
    openDialog();
  }, [openDialog]);

  const handleSkillClick = useCallback(() => {
    openDialog('skills');
  }, [openDialog]);

  const handleMemoryClick = useCallback(() => {
    openDialog('memory', { memoryViewMode: 'all' });
  }, [openDialog]);

  const currentSkillSummary = latestEnvelope?.diagnostics;

  return (
    <div data-testid="skill-memory-panel" className="h-full min-h-0 flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-gray-200 dark:border-gray-700">
        <div className="min-w-0">
          <span className="text-xs font-medium text-gray-700 dark:text-gray-300">{t('skillPanel.title')}</span>
          <p className="text-2xs text-gray-400 dark:text-gray-500">
            {memoryPipelineSnapshot
              ? memoryPipelineSnapshot.pendingCount > 0
                ? t('skillPanel.panelSummary.pending', {
                    count: memoryPipelineSnapshot.pendingCount,
                    defaultValue: '{{count}} memories pending review',
                  })
                : memoryPipelineSnapshot.injectedCount > 0
                  ? t('skillPanel.panelSummary.injected', {
                      count: memoryPipelineSnapshot.injectedCount,
                      defaultValue: '{{count}} memories injected',
                    })
                  : t('skillPanel.panelSummary.idle', {
                      defaultValue: 'Memory pipeline ready',
                    })
              : t('skillPanel.globalEnableHint', { defaultValue: 'Global skill enable/disable' })}
          </p>
        </div>
        <button
          onClick={handleManageAll}
          className={clsx(
            'p-1 rounded-md transition-colors',
            'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
          )}
          title={t('skillPanel.manageAll')}
        >
          <GearIcon className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Content */}
      <div className="px-2 py-2 space-y-1 flex-1 min-h-0 overflow-y-auto">
        {/* Loading state */}
        {(skillsLoading || memoriesLoading) && skills.length === 0 && memories.length === 0 && (
          <div className="text-center py-4">
            <span className="text-xs text-gray-400 dark:text-gray-500">{t('skillPanel.loading')}</span>
          </div>
        )}

        <CollapsibleSection
          title={t('skillPanel.currentSkillStack', { defaultValue: 'Active in current session' })}
          count={activeSkills.length}
        >
          {activeSkills.length > 0 ? (
            <>
              {activeSkills.map((skill) => (
                <SkillRow key={skill.id} skill={skill} onToggle={handleToggle} onClick={handleSkillClick} />
              ))}
              <div className="px-2 py-1.5 space-y-1 text-2xs text-gray-500 dark:text-gray-400">
                {currentSkillSummary?.selection_origin && (
                  <div>
                    {t('skillPanel.selectionOrigin', { defaultValue: 'Selection origin' })}:{' '}
                    {currentSkillSummary.selection_origin}
                  </div>
                )}
                {currentSkillSummary?.selection_reason && (
                  <div>
                    {t('skillPanel.selectionReason', { defaultValue: 'Selection reason' })}:{' '}
                    {currentSkillSummary.selection_reason}
                  </div>
                )}
                {currentSkillSummary?.blocked_tools?.length ? (
                  <div className="text-amber-700 dark:text-amber-300">
                    {t('skillPanel.blockedTools', { defaultValue: 'Blocked tools' })}:{' '}
                    {currentSkillSummary.blocked_tools.join(', ')}
                  </div>
                ) : null}
              </div>
            </>
          ) : (
            <p className="text-2xs text-gray-400 dark:text-gray-500 px-2 py-1">
              {t('skillPanel.noActiveSkills', {
                defaultValue: 'No effective skills in the latest run yet.',
              })}
            </p>
          )}
        </CollapsibleSection>

        {/* Auto-Detected Skills */}
        <CollapsibleSection title={t('skillPanel.detectedSkills')} count={detectedSkills.length}>
          {detectedSkills.length > 0 ? (
            detectedSkills.map((skill) => (
              <SkillRow key={skill.id} skill={skill} onToggle={handleToggle} onClick={handleSkillClick} />
            ))
          ) : (
            <p className="text-2xs text-gray-400 dark:text-gray-500 px-2 py-1">{t('skillPanel.noDetectedSkills')}</p>
          )}
        </CollapsibleSection>

        {/* Project Skills */}
        <CollapsibleSection title={t('skillPanel.projectSkills')} count={projectSkills.length} defaultExpanded={false}>
          {projectSkills.length > 0 ? (
            projectSkills.map((skill) => (
              <SkillRow key={skill.id} skill={skill} onToggle={handleToggle} onClick={handleSkillClick} />
            ))
          ) : (
            <p className="text-2xs text-gray-400 dark:text-gray-500 px-2 py-1">{t('skillPanel.noProjectSkills')}</p>
          )}
        </CollapsibleSection>

        <CollapsibleSection
          title={t('skillPanel.generatedReviewedSkills', { defaultValue: 'Generated & reviewed' })}
          count={generatedSkills.length}
          defaultExpanded={false}
        >
          {generatedSkills.length > 0 ? (
            generatedSkills.map((skill) => (
              <div key={skill.id} className="px-2 py-1">
                <SkillRow skill={skill} onToggle={handleToggle} onClick={handleSkillClick} />
                <div className="ml-7 flex items-center gap-1.5 py-0.5 text-2xs text-gray-400 dark:text-gray-500">
                  <span>
                    {t(`skillPanel.reviewStatus.${skill.review_status ?? 'pending_review'}`, {
                      defaultValue: skill.review_status ?? 'pending_review',
                    })}
                  </span>
                  {invokedSkillIds.includes(skill.id) && (
                    <span className="rounded bg-sky-100 px-1 py-0.5 text-sky-700 dark:bg-sky-900/20 dark:text-sky-300">
                      {t('skillPanel.pinned', { defaultValue: 'Pinned' })}
                    </span>
                  )}
                </div>
              </div>
            ))
          ) : (
            <p className="text-2xs text-gray-400 dark:text-gray-500 px-2 py-1">
              {t('skillPanel.noGeneratedSkills', { defaultValue: 'No generated skills' })}
            </p>
          )}
        </CollapsibleSection>

        {/* Memories */}
        <CollapsibleSection title={t('skillPanel.memories')} count={memories.length} defaultExpanded={false}>
          {memories.length > 0 ? (
            memories
              .slice(0, 5)
              .map((memory) => <MemoryRow key={memory.id} memory={memory} onClick={handleMemoryClick} />)
          ) : (
            <p className="text-2xs text-gray-400 dark:text-gray-500 px-2 py-1">{t('skillPanel.noMemories')}</p>
          )}
          {memories.length > 5 && (
            <button
              onClick={() => openDialog('memory', { memoryViewMode: 'all' })}
              className="w-full text-2xs text-primary-600 dark:text-primary-400 hover:underline px-2 py-1"
            >
              {t('skillPanel.viewAll', { count: memories.length })}
            </button>
          )}
        </CollapsibleSection>
      </div>

      {/* Manage All button */}
      <div className="px-3 py-2 border-t border-gray-200 dark:border-gray-700">
        <button
          onClick={handleManageAll}
          className={clsx(
            'w-full px-2 py-1.5 rounded-md text-xs font-medium transition-colors',
            'text-primary-600 dark:text-primary-400',
            'hover:bg-primary-50 dark:hover:bg-primary-900/20',
            'border border-primary-200 dark:border-primary-800',
          )}
        >
          {t('skillPanel.manageAll')}
        </button>
      </div>
    </div>
  );
}

export default SkillMemoryPanel;
