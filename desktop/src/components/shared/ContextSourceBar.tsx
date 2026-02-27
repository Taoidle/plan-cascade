/**
 * ContextSourceBar
 *
 * Toolbar toggle buttons for Knowledge, Memory, and Skills context sources.
 * Each source has a toggle button + ChevronDown to open a Popover picker.
 * Placed in ChatToolbar so users can control which domain knowledge is injected
 * into both Chat Mode and Task Mode prompts.
 */

import { useState, useRef, useEffect, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { ChevronUpIcon } from '@radix-ui/react-icons';
import { useContextSourcesStore } from '../../store/contextSources';
import { useSettingsStore } from '../../store/settings';
import { useProjectsStore } from '../../store/projects';
import { ragSyncDocsCollection } from '../../lib/knowledgeApi';
import { KnowledgeSourcePicker } from './KnowledgeSourcePicker';
import { MemorySourcePicker } from './MemorySourcePicker';
import { SkillsSourcePicker } from './SkillsSourcePicker';

// Simple inline SVG icons to avoid dependency on a specific icon library
function BookOpenIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
      <path d="M2 3h4.5a2 2 0 0 1 2 2v8a1.5 1.5 0 0 0-1.5-1.5H2V3z" />
      <path d="M14 3H9.5a2 2 0 0 0-2 2v8A1.5 1.5 0 0 1 9 11.5H14V3z" />
    </svg>
  );
}

function BrainIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
      <path d="M8 2a4 4 0 0 0-4 4c0 1.5.8 2.8 2 3.5V12h4V9.5A4 4 0 0 0 8 2z" />
      <path d="M6 14h4" />
    </svg>
  );
}

function WrenchIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
      <path d="M10.5 2.5a3.5 3.5 0 0 0-4.95 4.95l-3.3 3.3a1 1 0 0 0 0 1.42l1.58 1.58a1 1 0 0 0 1.42 0l3.3-3.3A3.5 3.5 0 0 0 10.5 2.5z" />
    </svg>
  );
}

type PopoverType = 'knowledge' | 'memory' | 'skills' | null;

export function ContextSourceBar() {
  const { t } = useTranslation('simpleMode');
  const {
    knowledgeEnabled,
    memoryEnabled,
    skillsEnabled,
    selectedCollections,
    selectedDocuments,
    selectedMemoryCategories,
    selectedMemoryIds,
    selectedSkillIds,
    toggleKnowledge,
    toggleMemory,
    toggleSkills,
    loadCollections,
    loadMemoryStats,
    loadAvailableSkills,
    autoAssociateForWorkspace,
  } = useContextSourcesStore();

  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const selectedProject = useProjectsStore((s) => s.selectedProject);
  const projectId = selectedProject?.id ?? 'default';

  // Auto-associate knowledge collections when workspace changes
  useEffect(() => {
    if (workspacePath) {
      autoAssociateForWorkspace(workspacePath, projectId);
    }
  }, [workspacePath, projectId, autoAssociateForWorkspace]);

  // Docs change detection
  const [docsChangePending, setDocsChangePending] = useState(false);
  const [isSyncingDocs, setIsSyncingDocs] = useState(false);

  useEffect(() => {
    const unlisten = listen<{ workspace_path: string; changed_files: string[] }>(
      'knowledge:docs-changes-detected',
      (event) => {
        if (!workspacePath) return;
        const normalized = (p: string) => p.replace(/\\/g, '/').replace(/\/+$/, '');
        if (normalized(event.payload.workspace_path) === normalized(workspacePath)) {
          setDocsChangePending(true);
        }
      },
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [workspacePath]);

  const handleSyncDocs = useCallback(async () => {
    if (!workspacePath) return;
    setIsSyncingDocs(true);
    try {
      await ragSyncDocsCollection(workspacePath, projectId);
      setDocsChangePending(false);
    } finally {
      setIsSyncingDocs(false);
    }
  }, [workspacePath, projectId]);

  const [openPopover, setOpenPopover] = useState<PopoverType>(null);
  const popoverRef = useRef<HTMLDivElement>(null);
  const barRef = useRef<HTMLDivElement>(null);

  // Close popover when clicking outside the entire bar + popover area
  useEffect(() => {
    if (!openPopover) return;
    const handler = (e: MouseEvent) => {
      if (
        popoverRef.current &&
        !popoverRef.current.contains(e.target as Node) &&
        barRef.current &&
        !barRef.current.contains(e.target as Node)
      ) {
        setOpenPopover(null);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [openPopover]);

  // --- Knowledge handlers ---
  const handleKnowledgeClick = useCallback(() => {
    if (!knowledgeEnabled) {
      toggleKnowledge(true);
      loadCollections(projectId);
    } else {
      toggleKnowledge(false);
      if (openPopover === 'knowledge') setOpenPopover(null);
    }
  }, [knowledgeEnabled, toggleKnowledge, projectId, loadCollections, openPopover]);

  const handleKnowledgeChevron = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!knowledgeEnabled) {
        toggleKnowledge(true);
        loadCollections(projectId);
      }
      setOpenPopover((prev) => (prev === 'knowledge' ? null : 'knowledge'));
    },
    [knowledgeEnabled, toggleKnowledge, projectId, loadCollections],
  );

  // --- Memory handlers ---
  const handleMemoryClick = useCallback(() => {
    if (!memoryEnabled) {
      toggleMemory(true);
      if (workspacePath) loadMemoryStats(workspacePath);
    } else {
      toggleMemory(false);
      if (openPopover === 'memory') setOpenPopover(null);
    }
  }, [memoryEnabled, toggleMemory, workspacePath, loadMemoryStats, openPopover]);

  const handleMemoryChevron = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!memoryEnabled) {
        toggleMemory(true);
      }
      if (workspacePath) loadMemoryStats(workspacePath);
      setOpenPopover((prev) => (prev === 'memory' ? null : 'memory'));
    },
    [memoryEnabled, toggleMemory, workspacePath, loadMemoryStats],
  );

  // --- Skills handlers ---
  const handleSkillsClick = useCallback(() => {
    if (!skillsEnabled) {
      toggleSkills(true);
      if (workspacePath) loadAvailableSkills(workspacePath);
    } else {
      toggleSkills(false);
      if (openPopover === 'skills') setOpenPopover(null);
    }
  }, [skillsEnabled, toggleSkills, workspacePath, loadAvailableSkills, openPopover]);

  const handleSkillsChevron = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      if (!skillsEnabled) {
        toggleSkills(true);
      }
      if (workspacePath) loadAvailableSkills(workspacePath);
      setOpenPopover((prev) => (prev === 'skills' ? null : 'skills'));
    },
    [skillsEnabled, toggleSkills, workspacePath, loadAvailableSkills],
  );

  // Badge counts
  const knowledgeCount = selectedCollections.length + selectedDocuments.length;
  const memoryCount = selectedMemoryCategories.length + selectedMemoryIds.length;
  const skillsCount = selectedSkillIds.length;

  return (
    <div ref={barRef} className="flex items-center gap-1 relative">
      {/* Knowledge toggle + picker */}
      <div className="relative">
        <button
          onClick={handleKnowledgeClick}
          className={clsx(
            'px-2 py-1 text-xs font-medium rounded-md transition-colors inline-flex items-center gap-1',
            knowledgeEnabled
              ? 'bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300 border border-amber-200 dark:border-amber-700'
              : 'text-gray-400 dark:text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700',
          )}
          title={t('contextSources.knowledge', { defaultValue: 'Knowledge' })}
        >
          <BookOpenIcon className="w-3.5 h-3.5" />
          {t('contextSources.knowledge', { defaultValue: 'Knowledge' })}
          {knowledgeEnabled && knowledgeCount > 0 && (
            <span className="ml-0.5 px-1 py-0 rounded-full bg-amber-200 dark:bg-amber-800 text-2xs">
              {knowledgeCount}
            </span>
          )}
          <span
            role="button"
            tabIndex={-1}
            onClick={handleKnowledgeChevron}
            className="ml-0.5 hover:bg-amber-200/50 dark:hover:bg-amber-800/50 rounded"
          >
            <ChevronUpIcon className="w-3 h-3" />
          </span>
        </button>

        {openPopover === 'knowledge' && (
          <div
            ref={popoverRef}
            className={clsx(
              'absolute bottom-full left-0 mb-1 z-50',
              'w-72 max-h-80 overflow-y-auto',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'rounded-lg shadow-lg',
            )}
          >
            <KnowledgeSourcePicker />
          </div>
        )}
      </div>

      {/* Memory toggle + picker */}
      <div className="relative">
        <button
          onClick={handleMemoryClick}
          className={clsx(
            'px-2 py-1 text-xs font-medium rounded-md transition-colors inline-flex items-center gap-1',
            memoryEnabled
              ? 'bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-300 border border-purple-200 dark:border-purple-700'
              : 'text-gray-400 dark:text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700',
          )}
          title={t('contextSources.memory', { defaultValue: 'Memory' })}
        >
          <BrainIcon className="w-3.5 h-3.5" />
          {t('contextSources.memory', { defaultValue: 'Memory' })}
          {memoryEnabled && memoryCount > 0 && (
            <span className="ml-0.5 px-1 py-0 rounded-full bg-purple-200 dark:bg-purple-800 text-2xs">
              {memoryCount}
            </span>
          )}
          <span
            role="button"
            tabIndex={-1}
            onClick={handleMemoryChevron}
            className="ml-0.5 hover:bg-purple-200/50 dark:hover:bg-purple-800/50 rounded"
          >
            <ChevronUpIcon className="w-3 h-3" />
          </span>
        </button>

        {openPopover === 'memory' && (
          <div
            ref={popoverRef}
            className={clsx(
              'absolute bottom-full left-0 mb-1 z-50',
              'w-72 max-h-80 overflow-y-auto',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'rounded-lg shadow-lg',
            )}
          >
            <MemorySourcePicker />
          </div>
        )}
      </div>

      {/* Skills toggle + picker */}
      <div className="relative">
        <button
          onClick={handleSkillsClick}
          className={clsx(
            'px-2 py-1 text-xs font-medium rounded-md transition-colors inline-flex items-center gap-1',
            skillsEnabled
              ? 'bg-emerald-100 dark:bg-emerald-900/40 text-emerald-700 dark:text-emerald-300 border border-emerald-200 dark:border-emerald-700'
              : 'text-gray-400 dark:text-gray-500 hover:bg-gray-100 dark:hover:bg-gray-700',
          )}
          title={t('contextSources.skills', { defaultValue: 'Skills' })}
        >
          <WrenchIcon className="w-3.5 h-3.5" />
          {t('contextSources.skills', { defaultValue: 'Skills' })}
          {skillsEnabled && skillsCount > 0 && (
            <span className="ml-0.5 px-1 py-0 rounded-full bg-emerald-200 dark:bg-emerald-800 text-2xs">
              {skillsCount}
            </span>
          )}
          <span
            role="button"
            tabIndex={-1}
            onClick={handleSkillsChevron}
            className="ml-0.5 hover:bg-emerald-200/50 dark:hover:bg-emerald-800/50 rounded"
          >
            <ChevronUpIcon className="w-3 h-3" />
          </span>
        </button>

        {openPopover === 'skills' && (
          <div
            ref={popoverRef}
            className={clsx(
              'absolute bottom-full left-0 mb-1 z-50',
              'w-72 max-h-80 overflow-y-auto',
              'bg-white dark:bg-gray-800',
              'border border-gray-200 dark:border-gray-700',
              'rounded-lg shadow-lg',
            )}
          >
            <SkillsSourcePicker />
          </div>
        )}
      </div>

      {/* Docs sync indicator */}
      {docsChangePending && (
        <button
          onClick={handleSyncDocs}
          disabled={isSyncingDocs}
          className={clsx(
            'px-2 py-1 text-xs font-medium rounded-md transition-colors inline-flex items-center gap-1',
            'bg-orange-100 dark:bg-orange-900/40 text-orange-700 dark:text-orange-300',
            'border border-orange-200 dark:border-orange-700',
            'hover:bg-orange-200 dark:hover:bg-orange-900/60',
            'disabled:opacity-50 disabled:cursor-not-allowed',
          )}
          title={t('contextSources.syncDocs', { defaultValue: 'Sync Docs' })}
        >
          <svg className="w-3.5 h-3.5" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5">
            <path d="M2 8a6 6 0 0 1 10.5-4M14 8A6 6 0 0 1 3.5 12" />
            <path d="M12.5 1v3h-3M3.5 15v-3h3" />
          </svg>
          {isSyncingDocs
            ? t('contextSources.syncingDocs', { defaultValue: 'Syncing...' })
            : t('contextSources.syncDocs', { defaultValue: 'Sync Docs' })}
        </button>
      )}
    </div>
  );
}
