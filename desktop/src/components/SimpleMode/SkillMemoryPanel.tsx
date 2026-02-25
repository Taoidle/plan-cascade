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
import {
  ChevronRightIcon,
  GearIcon,
} from '@radix-ui/react-icons';
import { useSkillMemoryStore } from '../../store/skillMemory';
import { useSettingsStore } from '../../store/settings';
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
          'hover:bg-gray-50 dark:hover:bg-gray-800'
        )}
      >
        <ChevronRightIcon className={clsx('w-3.5 h-3.5 shrink-0 transition-transform duration-200', expanded && 'rotate-90')} />
        <span className="flex-1 text-left">{title}</span>
        {count > 0 && (
          <span className="text-2xs px-1.5 py-0.5 rounded-full bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300 shrink-0">
            {count}
          </span>
        )}
      </button>
      <Collapsible open={expanded}><div className="mt-0.5">{children}</div></Collapsible>
    </div>
  );
}

// ============================================================================
// SkillMemoryPanel
// ============================================================================

export function SkillMemoryPanel() {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);

  const skills = useSkillMemoryStore((s) => s.skills);
  const skillsLoading = useSkillMemoryStore((s) => s.skillsLoading);
  const memories = useSkillMemoryStore((s) => s.memories);
  const memoriesLoading = useSkillMemoryStore((s) => s.memoriesLoading);
  const panelOpen = useSkillMemoryStore((s) => s.panelOpen);
  const loadSkills = useSkillMemoryStore((s) => s.loadSkills);
  const loadMemories = useSkillMemoryStore((s) => s.loadMemories);
  const toggleSkill = useSkillMemoryStore((s) => s.toggleSkill);
  const openDialog = useSkillMemoryStore((s) => s.openDialog);

  // Load data when panel opens
  useEffect(() => {
    if (panelOpen && workspacePath) {
      loadSkills(workspacePath);
      loadMemories(workspacePath);
    }
  }, [panelOpen, workspacePath, loadSkills, loadMemories]);

  // Separate auto-detected skills from others
  const { detectedSkills, projectSkills } = useMemo(() => {
    const detected: SkillSummary[] = [];
    const project: SkillSummary[] = [];
    for (const skill of skills) {
      if (skill.detected) {
        detected.push(skill);
      } else {
        project.push(skill);
      }
    }
    return { detectedSkills: detected, projectSkills: project };
  }, [skills]);

  const handleToggle = useCallback(
    (id: string, enabled: boolean) => {
      toggleSkill(id, enabled);
    },
    [toggleSkill]
  );

  const handleManageAll = useCallback(() => {
    openDialog();
  }, [openDialog]);

  const handleSkillClick = useCallback(
    () => {
      openDialog('skills');
    },
    [openDialog]
  );

  const handleMemoryClick = useCallback(
    () => {
      openDialog('memory');
    },
    [openDialog]
  );

  return (
    <Collapsible open={panelOpen}>
    <div
      data-testid="skill-memory-panel"
      className="border-t border-gray-200 dark:border-gray-700"
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2">
        <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
          {t('skillPanel.title')}
        </span>
        <button
          onClick={handleManageAll}
          className={clsx(
            'p-1 rounded-md transition-colors',
            'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
            'hover:bg-gray-100 dark:hover:bg-gray-800'
          )}
          title={t('skillPanel.manageAll')}
        >
          <GearIcon className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Content */}
      <div className="px-2 pb-2 space-y-1 max-h-[300px] overflow-y-auto">
        {/* Loading state */}
        {(skillsLoading || memoriesLoading) && skills.length === 0 && memories.length === 0 && (
          <div className="text-center py-4">
            <span className="text-xs text-gray-400 dark:text-gray-500">
              {t('skillPanel.loading')}
            </span>
          </div>
        )}

        {/* Auto-Detected Skills */}
        <CollapsibleSection
          title={t('skillPanel.detectedSkills')}
          count={detectedSkills.length}
        >
          {detectedSkills.length > 0 ? (
            detectedSkills.map((skill) => (
              <SkillRow
                key={skill.id}
                skill={skill}
                onToggle={handleToggle}
                onClick={handleSkillClick}
              />
            ))
          ) : (
            <p className="text-2xs text-gray-400 dark:text-gray-500 px-2 py-1">
              {t('skillPanel.noDetectedSkills')}
            </p>
          )}
        </CollapsibleSection>

        {/* Project Skills */}
        <CollapsibleSection
          title={t('skillPanel.projectSkills')}
          count={projectSkills.length}
          defaultExpanded={false}
        >
          {projectSkills.length > 0 ? (
            projectSkills.map((skill) => (
              <SkillRow
                key={skill.id}
                skill={skill}
                onToggle={handleToggle}
                onClick={handleSkillClick}
              />
            ))
          ) : (
            <p className="text-2xs text-gray-400 dark:text-gray-500 px-2 py-1">
              {t('skillPanel.noProjectSkills')}
            </p>
          )}
        </CollapsibleSection>

        {/* Memories */}
        <CollapsibleSection
          title={t('skillPanel.memories')}
          count={memories.length}
          defaultExpanded={false}
        >
          {memories.length > 0 ? (
            memories.slice(0, 5).map((memory) => (
              <MemoryRow
                key={memory.id}
                memory={memory}
                onClick={handleMemoryClick}
              />
            ))
          ) : (
            <p className="text-2xs text-gray-400 dark:text-gray-500 px-2 py-1">
              {t('skillPanel.noMemories')}
            </p>
          )}
          {memories.length > 5 && (
            <button
              onClick={() => openDialog('memory')}
              className="w-full text-2xs text-primary-600 dark:text-primary-400 hover:underline px-2 py-1"
            >
              {t('skillPanel.viewAll', { count: memories.length })}
            </button>
          )}
        </CollapsibleSection>
      </div>

      {/* Manage All button */}
      <div className="px-3 pb-2">
        <button
          onClick={handleManageAll}
          className={clsx(
            'w-full px-2 py-1.5 rounded-md text-xs font-medium transition-colors',
            'text-primary-600 dark:text-primary-400',
            'hover:bg-primary-50 dark:hover:bg-primary-900/20',
            'border border-primary-200 dark:border-primary-800'
          )}
        >
          {t('skillPanel.manageAll')}
        </button>
      </div>
    </div>
    </Collapsible>
  );
}

export default SkillMemoryPanel;
