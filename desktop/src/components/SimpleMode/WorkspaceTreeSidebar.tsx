/**
 * WorkspaceTreeSidebar Component
 *
 * Tree-structured sidebar that groups sessions by pinned workspace directories.
 * Provides directory management (pin/unpin), session actions (restore/rename/delete),
 * and a collapsible "Other" group for unmatched sessions.
 */

import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { ChevronRightIcon, ChevronLeftIcon, PlusIcon, Cross2Icon } from '@radix-ui/react-icons';
import { type ExecutionHistoryItem, type SessionSnapshot } from '../../store/execution';
import { useSettingsStore, type SessionPathSort } from '../../store/settings';
import { useSkillMemoryStore } from '../../store/skillMemory';
import { usePluginStore } from '../../store/plugins';
import { useAgentsStore } from '../../store/agents';
import { usePromptsStore } from '../../store/prompts';
import { useWorkflowKernelStore } from '../../store/workflowKernel';
import type { WorkflowSessionCatalogItem } from '../../types/workflowKernel';
import type { Worktree, WorktreeCleanupPolicy } from '../../types/git';
import { Collapsible } from './Collapsible';
import { buildSessionTreeViewModel, type PathGroup, type SessionTreeItem } from './sessionTreeViewModel';
import { SkillMemoryPanel } from './SkillMemoryPanel';
import { PluginPanel } from './PluginPanel';
import { AgentPanel } from './AgentPanel';
import { PromptPanel } from './PromptPanel';
import { SkillMemoryDialog } from '../SkillMemory/SkillMemoryDialog';
import { PluginDialog } from '../Plugins/PluginDialog';
import { AgentDialog } from '../Agents/AgentDialog';
import { PromptDialog } from '../Prompts/PromptDialog';
import { SkillMemoryToast } from '../SkillMemory/SkillMemoryToast';

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface WorkspaceTreeSidebarProps {
  history: ExecutionHistoryItem[];
  onRestore: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onClear: () => void;
  onClearAllSessions?: () => void;
  onNewTask: (request?: NewSessionRequest) => void;
  currentTask?: string | null;
  /** Background session snapshots keyed by session ID */
  backgroundSessions?: Record<string, SessionSnapshot>;
  /** Called when user clicks a background session to switch to it */
  onSwitchSession?: (id: string) => void;
  /** Called when user clicks the remove button on a background session */
  onRemoveSession?: (id: string) => void;
  /** Parent session ID of the current foreground session (for fork hierarchy display) */
  foregroundParentSessionId?: string | null;
  /** bg session ID representing the foreground in the tree (ghost entry) */
  foregroundBgId?: string | null;
  workflowSessions?: WorkflowSessionCatalogItem[];
  activeWorkflowSessionId?: string | null;
  onSwitchWorkflowSession?: (id: string) => void;
  onRenameWorkflowSession?: (id: string, title: string) => void;
  onArchiveWorkflowSession?: (id: string) => void;
  onRestoreWorkflowSession?: (id: string) => void;
  onDeleteWorkflowSession?: (
    id: string,
    options?: {
      deleteWorktree?: boolean;
    },
  ) => void;
  pathGroups?: PathGroup[];
  activePath?: string | null;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function normalizeWorkspacePath(path: string | null | undefined): string | null {
  const value = (path || '').trim();
  if (!value) return null;
  return value.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
}

function timeAgo(timestamp: number, nowMs: number, t: ReturnType<typeof useTranslation>['t']): string {
  const seconds = Math.floor((nowMs - timestamp) / 1000);
  if (seconds < 60) return t('sidebar.time.justNow', { defaultValue: 'just now' });
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return t('sidebar.time.minutesAgo', { count: minutes, defaultValue: '{{count}}m ago' });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t('sidebar.time.hoursAgo', { count: hours, defaultValue: '{{count}}h ago' });
  const days = Math.floor(hours / 24);
  return t('sidebar.time.daysAgo', { count: days, defaultValue: '{{count}}d ago' });
}

type SessionManageTarget = Pick<SessionTreeItem, 'kind' | 'sourceSessionId' | 'title'>;

type SessionRuntimeMode = 'main' | 'managed_worktree' | 'attach_existing_worktree';

export interface NewSessionRequest {
  workspacePath: string;
  runtimeMode: SessionRuntimeMode;
  taskName?: string;
  targetBranch?: string;
  worktreePath?: string;
  cleanupPolicy?: WorktreeCleanupPolicy;
}

interface WorkflowSessionDeleteMeta {
  hasManagedWorktree: boolean;
  runtimeLabel: string | null;
  autoCleanupDefault: boolean;
}

function NewSessionDialog({
  open,
  workspacePath,
  suggestedTaskName,
  autoCleanupDefault,
  existingWorktrees,
  onOpenChange,
  onConfirm,
}: {
  open: boolean;
  workspacePath: string;
  suggestedTaskName: string;
  autoCleanupDefault: boolean;
  existingWorktrees: Worktree[];
  onOpenChange: (open: boolean) => void;
  onConfirm: (request: NewSessionRequest) => void;
}) {
  const { t } = useTranslation('simpleMode');
  const [runtimeMode, setRuntimeMode] = useState<SessionRuntimeMode>('main');
  const [taskName, setTaskName] = useState(suggestedTaskName);
  const [targetBranch, setTargetBranch] = useState('main');
  const [selectedWorktreePath, setSelectedWorktreePath] = useState('');
  const [cleanupPolicy, setCleanupPolicy] = useState<WorktreeCleanupPolicy>(
    autoCleanupDefault ? 'delete_on_session_delete' : 'manual',
  );

  useEffect(() => {
    if (!open) return;
    setTaskName(suggestedTaskName);
    setTargetBranch('main');
    setCleanupPolicy(autoCleanupDefault ? 'delete_on_session_delete' : 'manual');
    setSelectedWorktreePath(existingWorktrees[0]?.path ?? '');
  }, [autoCleanupDefault, existingWorktrees, open, suggestedTaskName]);

  const canSubmit =
    workspacePath.trim().length > 0 &&
    (runtimeMode !== 'managed_worktree' || taskName.trim().length > 0) &&
    (runtimeMode !== 'attach_existing_worktree' || selectedWorktreePath.trim().length > 0);

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-[90] bg-black/40 backdrop-blur-[1px]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-[100] w-[min(92vw,560px)] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-gray-200 bg-white p-5 shadow-xl dark:border-gray-700 dark:bg-gray-900">
          <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-gray-100">
            {t('sidebar.newSessionDialog.title', { defaultValue: 'Create session runtime' })}
          </Dialog.Title>
          <Dialog.Description className="mt-2 text-sm text-gray-600 dark:text-gray-300">
            {t('sidebar.newSessionDialog.description', {
              defaultValue:
                'Choose whether this session should run in the main workspace, a new managed worktree, or an existing worktree.',
            })}
          </Dialog.Description>

          <div className="mt-4 space-y-4">
            <div>
              <div className="mb-1 text-xs font-medium text-gray-600 dark:text-gray-300">
                {t('sidebar.newSessionDialog.workspace', { defaultValue: 'Workspace root' })}
              </div>
              <div className="rounded-md border border-gray-200 bg-gray-50 px-3 py-2 font-mono text-xs text-gray-700 dark:border-gray-700 dark:bg-gray-950 dark:text-gray-200">
                {workspacePath}
              </div>
            </div>

            <div className="space-y-2">
              <label className="flex items-start gap-3 rounded-lg border border-gray-200 p-3 dark:border-gray-700">
                <input
                  type="radio"
                  name="session-runtime-mode"
                  checked={runtimeMode === 'main'}
                  onChange={() => setRuntimeMode('main')}
                  className="mt-1"
                />
                <div>
                  <div className="text-sm font-medium text-gray-900 dark:text-gray-100">
                    {t('sidebar.newSessionDialog.main', { defaultValue: 'Main workspace' })}
                  </div>
                  <div className="text-xs text-gray-500 dark:text-gray-400">
                    {t('sidebar.newSessionDialog.mainHelp', {
                      defaultValue: 'Run directly in the project root without creating a worktree.',
                    })}
                  </div>
                </div>
              </label>
              <label className="flex items-start gap-3 rounded-lg border border-gray-200 p-3 dark:border-gray-700">
                <input
                  type="radio"
                  name="session-runtime-mode"
                  checked={runtimeMode === 'managed_worktree'}
                  onChange={() => setRuntimeMode('managed_worktree')}
                  className="mt-1"
                />
                <div className="flex-1">
                  <div className="text-sm font-medium text-gray-900 dark:text-gray-100">
                    {t('sidebar.newSessionDialog.managed', { defaultValue: 'New managed worktree' })}
                  </div>
                  <div className="text-xs text-gray-500 dark:text-gray-400">
                    {t('sidebar.newSessionDialog.managedHelp', {
                      defaultValue: 'Create an isolated runtime under ~/.plan-cascade/worktrees for this session.',
                    })}
                  </div>
                  {runtimeMode === 'managed_worktree' ? (
                    <div className="mt-3 grid gap-3">
                      <input
                        type="text"
                        value={taskName}
                        onChange={(event) => setTaskName(event.target.value)}
                        placeholder={t('sidebar.newSessionDialog.taskNamePlaceholder', {
                          defaultValue: 'feature-runtime-isolation',
                        })}
                        className="w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 outline-none transition-colors focus:border-primary-500 focus:ring-2 focus:ring-primary-500/30 dark:border-gray-600 dark:bg-gray-950 dark:text-gray-100"
                      />
                      <input
                        type="text"
                        value={targetBranch}
                        onChange={(event) => setTargetBranch(event.target.value)}
                        placeholder={t('sidebar.newSessionDialog.targetBranchPlaceholder', {
                          defaultValue: 'main',
                        })}
                        className="w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 outline-none transition-colors focus:border-primary-500 focus:ring-2 focus:ring-primary-500/30 dark:border-gray-600 dark:bg-gray-950 dark:text-gray-100"
                      />
                    </div>
                  ) : null}
                </div>
              </label>
              <label className="flex items-start gap-3 rounded-lg border border-gray-200 p-3 dark:border-gray-700">
                <input
                  type="radio"
                  name="session-runtime-mode"
                  checked={runtimeMode === 'attach_existing_worktree'}
                  onChange={() => setRuntimeMode('attach_existing_worktree')}
                  className="mt-1"
                />
                <div className="flex-1">
                  <div className="text-sm font-medium text-gray-900 dark:text-gray-100">
                    {t('sidebar.newSessionDialog.attach', { defaultValue: 'Attach existing worktree' })}
                  </div>
                  <div className="text-xs text-gray-500 dark:text-gray-400">
                    {t('sidebar.newSessionDialog.attachHelp', {
                      defaultValue: 'Bind this new session to an existing managed or legacy worktree.',
                    })}
                  </div>
                  {runtimeMode === 'attach_existing_worktree' ? (
                    existingWorktrees.length > 0 ? (
                      <select
                        value={selectedWorktreePath}
                        onChange={(event) => setSelectedWorktreePath(event.target.value)}
                        className="mt-3 w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 outline-none transition-colors focus:border-primary-500 focus:ring-2 focus:ring-primary-500/30 dark:border-gray-600 dark:bg-gray-950 dark:text-gray-100"
                      >
                        {existingWorktrees.map((worktree) => (
                          <option key={worktree.path} value={worktree.path}>
                            {worktree.branch} - {worktree.path}
                          </option>
                        ))}
                      </select>
                    ) : (
                      <p className="mt-3 text-xs text-amber-600 dark:text-amber-300">
                        {t('sidebar.newSessionDialog.noWorktrees', {
                          defaultValue: 'No existing worktrees were found for this repository.',
                        })}
                      </p>
                    )
                  ) : null}
                </div>
              </label>
            </div>

            {runtimeMode !== 'main' ? (
              <label className="flex items-start gap-3 rounded-lg border border-gray-200 p-3 dark:border-gray-700">
                <input
                  type="checkbox"
                  checked={cleanupPolicy === 'delete_on_session_delete'}
                  onChange={(event) => setCleanupPolicy(event.target.checked ? 'delete_on_session_delete' : 'manual')}
                  className="mt-1"
                />
                <div>
                  <div className="text-sm font-medium text-gray-900 dark:text-gray-100">
                    {t('sidebar.newSessionDialog.cleanupLabel', {
                      defaultValue: 'Delete worktree when deleting the session',
                    })}
                  </div>
                  <div className="text-xs text-gray-500 dark:text-gray-400">
                    {t('sidebar.newSessionDialog.cleanupHelp', {
                      defaultValue:
                        'This only changes the default for this session. You can still override it when deleting the session later.',
                    })}
                  </div>
                </div>
              </label>
            ) : null}
          </div>

          <div className="mt-5 flex items-center justify-end gap-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="rounded-md border border-gray-300 px-3 py-1.5 text-sm text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-800"
            >
              {t('sidebar.cancelAction', { defaultValue: 'Cancel' })}
            </button>
            <button
              type="button"
              disabled={!canSubmit}
              onClick={() =>
                onConfirm({
                  workspacePath,
                  runtimeMode,
                  taskName: taskName.trim(),
                  targetBranch: targetBranch.trim(),
                  worktreePath: selectedWorktreePath.trim(),
                  cleanupPolicy,
                })
              }
              className={clsx(
                'rounded-md px-3 py-1.5 text-sm text-white transition-colors',
                canSubmit ? 'bg-primary-600 hover:bg-primary-700' : 'cursor-not-allowed bg-gray-400 dark:bg-gray-600',
              )}
            >
              {t('sidebar.newSessionDialog.confirm', { defaultValue: 'Create session' })}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

// ---------------------------------------------------------------------------
// Sidebar Header
// ---------------------------------------------------------------------------

type SidebarTabId = 'sessions' | 'skills' | 'plugins' | 'agents' | 'prompts';

interface SidebarTabDef {
  id: SidebarTabId;
  label: string;
  count: number;
}

function SidebarTabs({
  tabs,
  activeTab,
  onTabChange,
}: {
  tabs: SidebarTabDef[];
  activeTab: SidebarTabId;
  onTabChange: (tab: SidebarTabId) => void;
}) {
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const pages = useMemo(() => [tabs.slice(0, 2), tabs.slice(2, 5)], [tabs]);
  const getPageIndexForTab = useCallback(
    (tabId: SidebarTabId) => (tabId === 'sessions' || tabId === 'skills' ? 0 : 1),
    [],
  );
  const [currentPage, setCurrentPage] = useState<number>(() => getPageIndexForTab(activeTab));

  const focusTab = useCallback((index: number) => {
    tabRefs.current[index]?.focus();
  }, []);

  useEffect(() => {
    setCurrentPage(getPageIndexForTab(activeTab));
  }, [activeTab, getPageIndexForTab]);

  const handleTabKeyDown = useCallback(
    (event: React.KeyboardEvent<HTMLButtonElement>, index: number) => {
      let nextIndex: number | null = null;
      if (event.key === 'ArrowRight') {
        nextIndex = (index + 1) % tabs.length;
      } else if (event.key === 'ArrowLeft') {
        nextIndex = (index - 1 + tabs.length) % tabs.length;
      } else if (event.key === 'Home') {
        nextIndex = 0;
      } else if (event.key === 'End') {
        nextIndex = tabs.length - 1;
      }
      if (nextIndex === null) return;
      event.preventDefault();
      onTabChange(tabs[nextIndex].id);
      setCurrentPage(getPageIndexForTab(tabs[nextIndex].id));
      focusTab(nextIndex);
    },
    [focusTab, onTabChange, tabs, getPageIndexForTab],
  );

  const handlePrevPage = useCallback(() => {
    const targetPage = Math.max(0, currentPage - 1);
    setCurrentPage(targetPage);
    const targetTab = pages[targetPage][0];
    if (targetTab && targetTab.id !== activeTab) onTabChange(targetTab.id);
  }, [activeTab, currentPage, onTabChange, pages]);

  const handleNextPage = useCallback(() => {
    const targetPage = Math.min(pages.length - 1, currentPage + 1);
    setCurrentPage(targetPage);
    const targetTab = pages[targetPage][0];
    if (targetTab && targetTab.id !== activeTab) onTabChange(targetTab.id);
  }, [activeTab, currentPage, onTabChange, pages]);

  return (
    <div role="tablist" aria-label="Sidebar tabs" className="rounded-lg bg-gray-100 dark:bg-gray-800/80 p-1">
      <div className="flex items-center gap-1 h-8">
        {currentPage > 0 && (
          <button
            type="button"
            data-testid="sidebar-tab-prev"
            onClick={handlePrevPage}
            className={clsx(
              'h-7 w-7 shrink-0 rounded-md flex items-center justify-center transition-colors',
              'text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700',
            )}
            aria-label="Previous tab page"
            title="Previous"
          >
            <ChevronLeftIcon className="w-3.5 h-3.5" />
          </button>
        )}

        <div className="flex-1 min-w-0 h-8 overflow-hidden">
          <div
            className="h-8 flex transition-transform duration-250 ease-out"
            style={{
              width: `${pages.length * 100}%`,
              transform: `translateX(-${currentPage * (100 / pages.length)}%)`,
            }}
          >
            {pages.map((page, pageIndex) => (
              <div
                key={pageIndex}
                className="h-8 shrink-0 flex items-center gap-1"
                style={{ width: `${100 / pages.length}%` }}
              >
                {page.map((tab, pageItemIndex) => {
                  void pageItemIndex;
                  const index = tabs.findIndex((item) => item.id === tab.id);
                  const selected = activeTab === tab.id;
                  return (
                    <button
                      key={tab.id}
                      id={`sidebar-tab-${tab.id}`}
                      ref={(node) => {
                        tabRefs.current[index] = node;
                      }}
                      data-testid={`sidebar-tab-${tab.id}`}
                      role="tab"
                      aria-selected={selected}
                      aria-controls={`sidebar-panel-${tab.id}`}
                      tabIndex={selected ? 0 : -1}
                      onClick={() => onTabChange(tab.id)}
                      onKeyDown={(event) => handleTabKeyDown(event, index)}
                      className={clsx(
                        'relative flex-1 min-w-0 h-8 px-2 rounded-md text-xs font-medium transition-colors',
                        selected
                          ? 'bg-white dark:bg-gray-700 text-primary-700 dark:text-primary-300 shadow-sm'
                          : 'text-gray-600 dark:text-gray-300 hover:text-gray-800 dark:hover:text-gray-100',
                      )}
                    >
                      <span className="truncate">{tab.label}</span>
                      {tab.count > 0 && (
                        <span
                          className={clsx(
                            'ml-1 inline-flex min-w-[1.1rem] h-[1.1rem] items-center justify-center rounded-full px-1 text-2xs',
                            selected
                              ? 'bg-primary-100 dark:bg-primary-900/40 text-primary-700 dark:text-primary-300'
                              : 'bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300',
                          )}
                        >
                          {tab.count}
                        </span>
                      )}
                    </button>
                  );
                })}
              </div>
            ))}
          </div>
        </div>

        {currentPage < pages.length - 1 && (
          <button
            type="button"
            data-testid="sidebar-tab-next"
            onClick={handleNextPage}
            className={clsx(
              'h-7 w-7 shrink-0 rounded-md flex items-center justify-center transition-colors',
              'text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700',
            )}
            aria-label="Next tab page"
            title="Next"
          >
            <ChevronRightIcon className="w-3.5 h-3.5" />
          </button>
        )}
      </div>
    </div>
  );
}

function SidebarActions({
  activeTab,
  onNewTask,
  onAddDirectory,
  sessionPathSort,
  onSessionPathSortChange,
  showArchivedSessions,
  onToggleShowArchivedSessions,
  onManageSkills,
  onManagePlugins,
  onOpenPluginMarketplace,
  onManageAgents,
  onManagePrompts,
}: {
  activeTab: SidebarTabId;
  onNewTask: () => void;
  onAddDirectory: () => void;
  sessionPathSort: SessionPathSort;
  onSessionPathSortChange: (sort: SessionPathSort) => void;
  showArchivedSessions: boolean;
  onToggleShowArchivedSessions: () => void;
  onManageSkills: () => void;
  onManagePlugins: () => void;
  onOpenPluginMarketplace: () => void;
  onManageAgents: () => void;
  onManagePrompts: () => void;
}) {
  const { t } = useTranslation('simpleMode');
  const [sortMenuOpen, setSortMenuOpen] = useState(false);

  const secondaryBtn = clsx(
    'px-2 py-1.5 rounded-md text-xs font-medium transition-colors',
    'text-gray-600 dark:text-gray-300',
    'hover:bg-gray-100 dark:hover:bg-gray-800',
    'border border-gray-200 dark:border-gray-700',
  );

  const iconBtn = clsx(
    'h-8 w-8 inline-flex items-center justify-center rounded-lg border transition-colors shrink-0',
    'border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-300',
    'hover:bg-gray-100 dark:hover:bg-gray-800',
  );

  if (activeTab === 'sessions') {
    return (
      <div className="flex items-center gap-2">
        <button
          onClick={onNewTask}
          className={clsx(
            'h-8 flex-1 px-3 rounded-lg text-xs font-medium transition-colors inline-flex items-center justify-center',
            'bg-primary-600 text-white hover:bg-primary-700',
            'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1',
          )}
        >
          {t('sidebar.newTask')}
        </button>
        <button
          onClick={onAddDirectory}
          className={iconBtn}
          aria-label={t('sidebar.addDirectory')}
          title={t('sidebar.addDirectory')}
        >
          <svg className="w-4 h-4" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
            <path d="M2 5.5A1.5 1.5 0 013.5 4H8l1.2 1.5H16.5A1.5 1.5 0 0118 7v7.5a1.5 1.5 0 01-1.5 1.5h-13A1.5 1.5 0 012 14.5v-9z" />
            <path d="M10 8a.75.75 0 01.75.75v1.5h1.5a.75.75 0 010 1.5h-1.5v1.5a.75.75 0 01-1.5 0v-1.5h-1.5a.75.75 0 010-1.5h1.5v-1.5A.75.75 0 0110 8z" />
          </svg>
        </button>
        <button
          type="button"
          onClick={onToggleShowArchivedSessions}
          className={clsx(
            iconBtn,
            showArchivedSessions && 'border-primary-300 text-primary-700 dark:text-primary-300 dark:border-primary-700',
          )}
          aria-label={
            showArchivedSessions
              ? t('sidebar.archived.hide', { defaultValue: 'Hide Archived' })
              : t('sidebar.archived.show', { defaultValue: 'Show Archived' })
          }
          title={
            showArchivedSessions
              ? t('sidebar.archived.hide', { defaultValue: 'Hide Archived' })
              : t('sidebar.archived.show', { defaultValue: 'Show Archived' })
          }
        >
          <svg className="w-4 h-4" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
            <path d="M4 4.75A1.75 1.75 0 015.75 3h8.5A1.75 1.75 0 0116 4.75v2.1a1.75 1.75 0 01-.513 1.237l-.487.488v5.675A1.75 1.75 0 0113.25 16h-6.5A1.75 1.75 0 015 14.25V8.575l-.487-.488A1.75 1.75 0 014 6.85v-2.1zm2.5-.25a.75.75 0 000 1.5h7a.75.75 0 000-1.5h-7z" />
          </svg>
        </button>
        <div className="relative">
          <button
            type="button"
            onClick={() => setSortMenuOpen((open) => !open)}
            className={iconBtn}
            aria-label={t('sidebar.sort.label', { defaultValue: 'Sort paths' })}
            title={t('sidebar.sort.label', { defaultValue: 'Sort paths' })}
          >
            <svg className="w-4 h-4" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
              <path d="M4 5.75A.75.75 0 014.75 5h10.5a.75.75 0 010 1.5H4.75A.75.75 0 014 5.75zm2 4A.75.75 0 016.75 9h8.5a.75.75 0 010 1.5h-8.5A.75.75 0 016 9.75zm3 4a.75.75 0 01.75-.75h5.5a.75.75 0 010 1.5h-5.5a.75.75 0 01-.75-.75z" />
            </svg>
          </button>
          {sortMenuOpen && (
            <div className="absolute right-0 top-11 z-10 min-w-[10rem] rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg p-1">
              <button
                type="button"
                onClick={() => {
                  onSessionPathSortChange('recent');
                  setSortMenuOpen(false);
                }}
                className={clsx(
                  'w-full rounded-md px-2 py-1.5 text-left text-xs transition-colors',
                  sessionPathSort === 'recent'
                    ? 'bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300'
                    : 'text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800',
                )}
              >
                {t('sidebar.sort.recent', { defaultValue: 'Sort: Recent' })}
              </button>
              <button
                type="button"
                onClick={() => {
                  onSessionPathSortChange('name');
                  setSortMenuOpen(false);
                }}
                className={clsx(
                  'w-full rounded-md px-2 py-1.5 text-left text-xs transition-colors',
                  sessionPathSort === 'name'
                    ? 'bg-primary-50 dark:bg-primary-900/20 text-primary-700 dark:text-primary-300'
                    : 'text-gray-700 dark:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800',
                )}
              >
                {t('sidebar.sort.name', { defaultValue: 'Sort: Name' })}
              </button>
            </div>
          )}
        </div>
      </div>
    );
  }

  if (activeTab === 'plugins') {
    return (
      <div className="flex items-center gap-2">
        <button onClick={onManagePlugins} className={clsx(secondaryBtn, 'flex-1')}>
          {t('pluginPanel.manageAll')}
        </button>
        <button onClick={onOpenPluginMarketplace} className={clsx(secondaryBtn, 'shrink-0')}>
          {t('pluginPanel.marketplace')}
        </button>
      </div>
    );
  }

  if (activeTab === 'skills') {
    return (
      <button onClick={onManageSkills} className={clsx(secondaryBtn, 'w-full')}>
        {t('skillPanel.manageAll')}
      </button>
    );
  }

  if (activeTab === 'agents') {
    return (
      <button onClick={onManageAgents} className={clsx(secondaryBtn, 'w-full')}>
        {t('agentPanel.manageAll', { defaultValue: 'Manage All...' })}
      </button>
    );
  }

  return (
    <button onClick={onManagePrompts} className={clsx(secondaryBtn, 'w-full')}>
      {t('promptPanel.manageAll', { defaultValue: 'Manage All...' })}
    </button>
  );
}

function statusDotClass(status: SessionTreeItem['status']): string {
  if (status === 'running') return 'bg-amber-500 dark:bg-amber-400';
  if (status === 'attention') return 'bg-red-500 dark:bg-red-400';
  return 'bg-gray-300 dark:bg-gray-600';
}

function statusText(status: SessionTreeItem['status'], t: ReturnType<typeof useTranslation>['t']): string {
  if (status === 'running') return t('sidebar.status.running', { defaultValue: 'Running' });
  if (status === 'attention') return t('sidebar.status.attention', { defaultValue: 'Attention' });
  return t('sidebar.status.idle', { defaultValue: 'Idle' });
}

function modeBadgeLabel(mode: SessionTreeItem['mode']): string | null {
  if (mode === 'chat') return 'C';
  if (mode === 'plan') return 'P';
  if (mode === 'task') return 'T';
  if (mode === 'debug') return 'D';
  return null;
}

function runtimeBadgeLabel(item: SessionTreeItem, t: ReturnType<typeof useTranslation>['t']): string | null {
  if (item.runtimeKind === 'managed_worktree') {
    return t('sidebar.runtime.managed', { defaultValue: 'worktree' });
  }
  if (item.runtimeKind === 'legacy_worktree') {
    return t('sidebar.runtime.legacy', { defaultValue: 'legacy' });
  }
  return null;
}

function SessionTreeRow({
  item,
  nowMs,
  selectionMode,
  isSelected,
  onToggleSelect,
  onActivateLive,
  onRestoreHistory,
  onRestoreArchived,
  onRequestArchive,
  onRequestRename,
  onRequestDelete,
}: {
  item: SessionTreeItem;
  nowMs: number;
  selectionMode: boolean;
  isSelected: boolean;
  onToggleSelect: (target: SessionManageTarget) => void;
  onActivateLive?: (id: string) => void;
  onRestoreHistory: (id: string) => void;
  onRestoreArchived?: (id: string) => void;
  onRequestArchive: (target: SessionManageTarget) => void;
  onRequestRename: (target: SessionManageTarget) => void;
  onRequestDelete: (target: SessionManageTarget) => void;
}) {
  const { t } = useTranslation('simpleMode');
  const selectionTarget = useMemo<SessionManageTarget>(
    () => ({
      kind: item.kind,
      sourceSessionId: item.sourceSessionId,
      title: item.title,
    }),
    [item.kind, item.sourceSessionId, item.title],
  );

  const handleActivate = useCallback(() => {
    if (selectionMode) {
      onToggleSelect(selectionTarget);
      return;
    }
    if (item.kind === 'live') {
      onActivateLive?.(item.sourceSessionId);
      return;
    }
    if (item.kind === 'archived') {
      onRestoreArchived?.(item.sourceSessionId);
      return;
    }
    onRestoreHistory(item.sourceSessionId);
  }, [item, onActivateLive, onRestoreArchived, onRestoreHistory, onToggleSelect, selectionMode, selectionTarget]);

  const handleArchive = useCallback(
    (event: React.MouseEvent) => {
      event.preventDefault();
      event.stopPropagation();
      if (item.kind !== 'live') return;
      onRequestArchive({
        kind: item.kind,
        sourceSessionId: item.sourceSessionId,
        title: item.title,
      });
    },
    [item, onRequestArchive],
  );

  const handleRename = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      onRequestRename({
        kind: item.kind,
        sourceSessionId: item.sourceSessionId,
        title: item.title,
      });
    },
    [item, onRequestRename],
  );

  const handleDelete = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      onRequestDelete({
        kind: item.kind,
        sourceSessionId: item.sourceSessionId,
        title: item.title,
      });
    },
    [item, onRequestDelete],
  );

  return (
    <div
      className={clsx(
        'group relative flex items-center gap-2 py-1.5 pr-2 rounded-md cursor-pointer transition-colors',
        item.isActive
          ? 'bg-primary-50 dark:bg-primary-900/20 border border-primary-200 dark:border-primary-800'
          : 'hover:bg-gray-50 dark:hover:bg-gray-800 border border-transparent',
      )}
      style={{ paddingLeft: 32 }}
      onClick={handleActivate}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') handleActivate();
      }}
      title={item.workspacePath || item.title}
    >
      {selectionMode ? (
        <label
          className="shrink-0 flex items-center"
          onClick={(event) => {
            event.stopPropagation();
          }}
        >
          <input
            type="checkbox"
            className="h-3.5 w-3.5 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
            checked={isSelected}
            onChange={() => onToggleSelect(selectionTarget)}
            aria-label={t('sidebar.selectSession', {
              defaultValue: 'Select session {{title}}',
              title: item.title,
            })}
          />
        </label>
      ) : null}
      <span className={clsx('w-2 h-2 rounded-full shrink-0', statusDotClass(item.status))} />

      <div
        className={clsx(
          'flex-1 min-w-0 transition-[padding] duration-150',
          !selectionMode && 'group-hover:pr-28 group-focus-within:pr-28',
        )}
      >
        <div className="flex items-center gap-1 min-w-0">
          <p className="min-w-0 flex-1 text-xs text-gray-900 dark:text-white truncate">{item.title}</p>
          {item.mode && (
            <span
              className="shrink-0 inline-flex h-4 min-w-4 items-center justify-center rounded bg-gray-100 dark:bg-gray-800 px-1 text-[9px] font-semibold text-gray-500 dark:text-gray-400"
              title={t(`sidebar.mode.${item.mode}`, { defaultValue: item.mode })}
            >
              {modeBadgeLabel(item.mode)}
            </span>
          )}
          {item.kind === 'history' && (
            <span
              className="shrink-0 inline-flex h-4 w-4 items-center justify-center rounded bg-gray-100 dark:bg-gray-800 text-[9px] text-gray-500 dark:text-gray-400"
              title={t('sidebar.badges.history', { defaultValue: 'History' })}
            >
              H
            </span>
          )}
          {item.kind === 'archived' && (
            <span
              className="shrink-0 inline-flex h-4 w-4 items-center justify-center rounded bg-gray-100 dark:bg-gray-800 text-[9px] text-gray-500 dark:text-gray-400"
              title={t('sidebar.badges.archived', { defaultValue: 'Archived' })}
            >
              A
            </span>
          )}
        </div>
        {item.detailChips.length > 0 && (
          <div className="mt-1 flex flex-wrap items-center gap-1">
            {item.detailChips.map((chip) => (
              <span
                key={`${item.id}-${chip}`}
                className="inline-flex max-w-full items-center rounded-full bg-gray-100 px-1.5 py-0.5 text-[10px] font-medium leading-none text-gray-600 dark:bg-gray-800 dark:text-gray-300"
              >
                <span className="truncate">{chip}</span>
              </span>
            ))}
          </div>
        )}
        {item.detailSummary && (
          <p className="mt-1 text-2xs leading-4 text-gray-500 dark:text-gray-400 line-clamp-2">{item.detailSummary}</p>
        )}
        {(item.runtimeKind !== 'main' || item.runtimeBranch || item.runtimePrState) && (
          <div className="mt-1 flex flex-wrap items-center gap-1">
            {runtimeBadgeLabel(item, t) ? (
              <span className="inline-flex max-w-full items-center rounded-full bg-primary-50 px-1.5 py-0.5 text-[10px] font-medium leading-none text-primary-700 dark:bg-primary-900/30 dark:text-primary-300">
                <span className="truncate">{runtimeBadgeLabel(item, t)}</span>
              </span>
            ) : null}
            {item.runtimeBranch ? (
              <span className="inline-flex max-w-full items-center rounded-full bg-gray-100 px-1.5 py-0.5 font-mono text-[10px] font-medium leading-none text-gray-600 dark:bg-gray-800 dark:text-gray-300">
                <span className="truncate">{item.runtimeBranch}</span>
              </span>
            ) : null}
            {item.runtimePrState ? (
              <span className="inline-flex max-w-full items-center rounded-full bg-emerald-50 px-1.5 py-0.5 text-[10px] font-medium leading-none text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300">
                <span className="truncate">
                  {t('sidebar.prState', { defaultValue: 'PR' })}: {item.runtimePrState}
                </span>
              </span>
            ) : null}
          </div>
        )}
        <div className="mt-0.5 flex items-center gap-2 text-2xs text-gray-500 dark:text-gray-400">
          {item.status !== 'idle' ? <span>{statusText(item.status, t)}</span> : null}
          {item.workspaceRootPath && item.runtimePath && item.workspaceRootPath !== item.runtimePath ? (
            <span className="truncate" title={`${item.workspaceRootPath} -> ${item.runtimePath}`}>
              {t('sidebar.runtimePath', { defaultValue: 'runtime' })}: {item.runtimePath}
            </span>
          ) : null}
          <span>{timeAgo(item.updatedAt, nowMs, t)}</span>
        </div>
      </div>

      {!selectionMode && (item.kind === 'history' || item.kind === 'live' || item.kind === 'archived') && (
        <div className="pointer-events-none absolute inset-y-0 right-2 flex items-center">
          <div className="absolute inset-y-0 -left-10 right-0 bg-gradient-to-l from-white via-white/95 to-transparent dark:from-gray-900 dark:via-gray-900/95 dark:to-transparent opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100" />
          <div className="pointer-events-auto relative flex items-center gap-0.5 shrink-0 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
            {item.kind === 'live' && (
              <button
                type="button"
                className="text-2xs px-1.5 py-0.5 rounded text-gray-500 hover:text-gray-200 hover:bg-gray-700/50"
                onClick={handleArchive}
                title={t('sidebar.archiveSession', { defaultValue: 'Archive session' })}
              >
                {t('sidebar.archiveAction', { defaultValue: 'archive' })}
              </button>
            )}
            {item.kind === 'archived' && (
              <button
                type="button"
                className="text-2xs px-1.5 py-0.5 rounded text-gray-500 hover:text-gray-200 hover:bg-gray-700/50"
                onClick={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  onRestoreArchived?.(item.sourceSessionId);
                }}
                title={t('sidebar.restoreSession', { defaultValue: 'Restore session' })}
              >
                {t('sidebar.restoreAction', { defaultValue: 'restore' })}
              </button>
            )}
            <button
              type="button"
              className="text-2xs px-1.5 py-0.5 rounded text-gray-500 hover:text-gray-200 hover:bg-gray-700/50"
              onClick={handleRename}
              title={t('sidebar.rename')}
            >
              {t('sidebar.rename')}
            </button>
            <button
              type="button"
              className="p-0.5 rounded text-gray-400 hover:text-red-400 hover:bg-red-900/30"
              onClick={handleDelete}
              title={t('sidebar.deleteSession', { defaultValue: 'Delete session' })}
            >
              <Cross2Icon className="w-3 h-3" />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function PathGroupNode({
  group,
  nowMs,
  isActive,
  isExpanded,
  selectionMode,
  selectedSessionIds,
  onToggleSelect,
  onToggle,
  onUnpin,
  onNewTaskInPath,
  onActivateLive,
  onRestoreHistory,
  onRestoreArchived,
  onRequestArchive,
  onRequestRename,
  onRequestDelete,
}: {
  group: PathGroup;
  nowMs: number;
  isActive: boolean;
  isExpanded: boolean;
  selectionMode: boolean;
  selectedSessionIds: Set<string>;
  onToggleSelect: (target: SessionManageTarget) => void;
  onToggle: () => void;
  onUnpin?: (() => void) | null;
  onNewTaskInPath: () => void;
  onActivateLive?: (id: string) => void;
  onRestoreHistory: (id: string) => void;
  onRestoreArchived?: (id: string) => void;
  onRequestArchive: (target: SessionManageTarget) => void;
  onRequestRename: (target: SessionManageTarget) => void;
  onRequestDelete: (target: SessionManageTarget) => void;
}) {
  const { t } = useTranslation('simpleMode');
  const hasAttention = group.children.some((child) => child.status === 'attention');

  const handleUnpin = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      onUnpin?.();
    },
    [onUnpin],
  );

  const handleNewTask = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();
      onNewTaskInPath();
    },
    [onNewTaskInPath],
  );

  return (
    <div>
      <div
        className={clsx(
          'group flex items-center gap-1 px-2 py-1.5 rounded-md cursor-pointer transition-colors',
          isActive ? 'bg-primary-50 dark:bg-primary-900/20' : 'hover:bg-gray-50 dark:hover:bg-gray-800',
        )}
        onClick={onToggle}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') onToggle();
        }}
        title={group.path || group.label}
      >
        <ChevronRightIcon
          className={clsx(
            'w-3.5 h-3.5 text-gray-400 shrink-0 transition-transform duration-200',
            isExpanded && 'rotate-90',
          )}
        />
        <svg className="w-3.5 h-3.5 text-gray-500 dark:text-gray-400 shrink-0" fill="currentColor" viewBox="0 0 20 20">
          <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
        </svg>
        <span className="flex-1 min-w-0 text-xs font-medium text-gray-900 dark:text-white truncate">{group.label}</span>
        {(group.hasRunning || hasAttention) && (
          <span
            className={clsx(
              'w-2 h-2 rounded-full shrink-0',
              hasAttention ? 'bg-red-500 dark:bg-red-400' : 'bg-amber-500 dark:bg-amber-400',
            )}
          />
        )}
        <span className="text-2xs px-1.5 py-0.5 rounded-full bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300 shrink-0">
          {group.sessionCount}
        </span>
        <div
          className={clsx(
            'flex items-center gap-0.5 shrink-0 transition-opacity',
            onUnpin ? 'opacity-100' : 'opacity-0 group-hover:opacity-100',
          )}
        >
          <button
            type="button"
            className="p-0.5 rounded text-gray-400 hover:text-primary-600 dark:hover:text-primary-400 hover:bg-primary-50 dark:hover:bg-primary-900/20"
            onClick={handleNewTask}
            title={t('sidebar.newTaskInDir')}
          >
            <PlusIcon className="w-3 h-3" />
          </button>
          {onUnpin && (
            <button
              type="button"
              className="p-0.5 rounded text-gray-400 hover:text-red-400 hover:bg-red-900/30"
              onClick={handleUnpin}
              title={t('sidebar.removeDirectory')}
            >
              <Cross2Icon className="w-3 h-3" />
            </button>
          )}
        </div>
      </div>

      <Collapsible open={isExpanded}>
        <div className="mt-0.5">
          {group.children.map((item) => (
            <SessionTreeRow
              key={item.id}
              item={item}
              nowMs={nowMs}
              selectionMode={selectionMode}
              isSelected={selectedSessionIds.has(item.id)}
              onToggleSelect={onToggleSelect}
              onActivateLive={onActivateLive}
              onRestoreHistory={onRestoreHistory}
              onRestoreArchived={onRestoreArchived}
              onRequestArchive={onRequestArchive}
              onRequestRename={onRequestRename}
              onRequestDelete={onRequestDelete}
            />
          ))}
        </div>
      </Collapsible>
    </div>
  );
}

// ---------------------------------------------------------------------------
// WorkspaceTreeSidebar (main component)
// ---------------------------------------------------------------------------

export const WorkspaceTreeSidebar = memo(function WorkspaceTreeSidebar({
  history,
  onRestore,
  onDelete,
  onRename,
  onClear,
  onClearAllSessions,
  onNewTask,
  currentTask: _currentTask,
  workflowSessions = [],
  activeWorkflowSessionId,
  onSwitchWorkflowSession,
  onRenameWorkflowSession,
  onArchiveWorkflowSession,
  onRestoreWorkflowSession,
  onDeleteWorkflowSession,
  pathGroups,
  activePath,
}: WorkspaceTreeSidebarProps) {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const pinnedDirectories = useSettingsStore((s) => s.pinnedDirectories);
  const removePinnedDirectory = useSettingsStore((s) => s.removePinnedDirectory);
  const setWorkspacePath = useSettingsStore((s) => s.setWorkspacePath);
  const sessionPathSort = useSettingsStore((s) => s.sessionPathSort);
  const setSessionPathSort = useSettingsStore((s) => s.setSessionPathSort);
  const showArchivedSessions = useSettingsStore((s) => s.showArchivedSessions);
  const setShowArchivedSessions = useSettingsStore((s) => s.setShowArchivedSessions);
  const worktreeAutoCleanupOnSessionDelete = useSettingsStore((s) => s.worktreeAutoCleanupOnSessionDelete);
  const listRepoWorktrees = useWorkflowKernelStore((s) => s.listRepoWorktrees);

  const skills = useSkillMemoryStore((s) => s.skills);
  const loadSkills = useSkillMemoryStore((s) => s.loadSkills);
  const loadMemories = useSkillMemoryStore((s) => s.loadMemories);
  const openSkillDialog = useSkillMemoryStore((s) => s.openDialog);

  const plugins = usePluginStore((s) => s.plugins);
  const loadPlugins = usePluginStore((s) => s.loadPlugins);
  const openPluginDialog = usePluginStore((s) => s.openDialog);
  const setPluginDialogTab = usePluginStore((s) => s.setActiveTab);

  const agents = useAgentsStore((s) => s.agents);
  const fetchAgents = useAgentsStore((s) => s.fetchAgents);
  const openAgentDialog = useAgentsStore((s) => s.openDialog);

  const prompts = usePromptsStore((s) => s.prompts);
  const fetchPrompts = usePromptsStore((s) => s.fetchPrompts);
  const openPromptDialog = usePromptsStore((s) => s.openDialog);

  // Count of detected/enabled skills for badge
  const detectedSkillCount = useMemo(() => skills.filter((s) => s.detected || s.enabled).length, [skills]);
  const pluginCount = plugins.length;
  const agentCount = agents.length;
  const promptCount = prompts.length;
  const [activeTab, setActiveTab] = useState<SidebarTabId>('sessions');

  // Expand/collapse state for path groups
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(() => new Set());
  const [relativeTimeNow, setRelativeTimeNow] = useState(() => Date.now());
  const [archiveTarget, setArchiveTarget] = useState<SessionManageTarget | null>(null);
  const [renameTarget, setRenameTarget] = useState<SessionManageTarget | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<SessionManageTarget | null>(null);
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedSessionIds, setSelectedSessionIds] = useState<Set<string>>(() => new Set());
  const [bulkDeleteTargets, setBulkDeleteTargets] = useState<SessionManageTarget[] | null>(null);
  const [bulkArchiveTargets, setBulkArchiveTargets] = useState<SessionManageTarget[] | null>(null);
  const [newSessionDialog, setNewSessionDialog] = useState<{
    open: boolean;
    workspacePath: string;
    existingWorktrees: Worktree[];
  }>({
    open: false,
    workspacePath: '',
    existingWorktrees: [],
  });

  useEffect(() => {
    const timer = window.setInterval(() => {
      setRelativeTimeNow(Date.now());
    }, 60_000);
    return () => {
      window.clearInterval(timer);
    };
  }, []);

  // Startup preload for tab badges/content responsiveness.
  const basePreloadedRef = useRef(false);
  useEffect(() => {
    if (basePreloadedRef.current) return;
    basePreloadedRef.current = true;
    if (plugins.length === 0) void loadPlugins();
    if (agents.length === 0) void fetchAgents();
    if (prompts.length === 0) void fetchPrompts();
  }, [plugins.length, agents.length, prompts.length, loadPlugins, fetchAgents, fetchPrompts]);

  // Keep skills/memories warm for the current workspace; avoid duplicate call in StrictMode.
  const lastPreloadedWorkspaceRef = useRef<string | null>(null);
  useEffect(() => {
    const normalizedWorkspace = normalizeWorkspacePath(workspacePath);
    if (!normalizedWorkspace || !workspacePath) {
      lastPreloadedWorkspaceRef.current = null;
      return;
    }
    if (lastPreloadedWorkspaceRef.current === normalizedWorkspace) return;
    lastPreloadedWorkspaceRef.current = normalizedWorkspace;
    void loadSkills(workspacePath);
    void loadMemories(workspacePath);
  }, [workspacePath, loadSkills, loadMemories]);

  // Auto-expand the active directory on mount / workspace change
  useEffect(() => {
    const activeNormalized = normalizeWorkspacePath(activePath ?? workspacePath);
    if (!activeNormalized) return;

    setExpandedPaths((prev) => {
      // Check if any pinned directory matches the active workspace
      for (const dir of pinnedDirectories) {
        const dirNormalized = dir.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
        if (activeNormalized === dirNormalized || activeNormalized.startsWith(dirNormalized + '/')) {
          if (!prev.has(dirNormalized)) {
            const next = new Set(prev);
            next.add(dirNormalized);
            return next;
          }
          break;
        }
      }
      return prev;
    });
  }, [activePath, workspacePath, pinnedDirectories]);

  const effectivePathGroups = useMemo(
    () =>
      pathGroups ??
      buildSessionTreeViewModel({
        workflowSessions,
        history,
        activeSessionId: activeWorkflowSessionId,
        pinnedDirectories,
        pathSort: sessionPathSort,
        includeArchived: showArchivedSessions,
      }),
    [
      activeWorkflowSessionId,
      history,
      pathGroups,
      pinnedDirectories,
      sessionPathSort,
      showArchivedSessions,
      workflowSessions,
    ],
  );

  const activeNormalized = useMemo(
    () => normalizeWorkspacePath(activePath ?? workspacePath),
    [activePath, workspacePath],
  );

  // Toggle expand/collapse for a directory
  const toggleDirectory = useCallback((normalizedPath: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(normalizedPath)) {
        next.delete(normalizedPath);
      } else {
        next.add(normalizedPath);
      }
      return next;
    });
  }, []);

  // Add directory via Tauri folder picker
  const handleAddDirectory = useCallback(async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('sidebar.addDirectory'),
      });
      if (selected && typeof selected === 'string') {
        setWorkspacePath(selected);
      }
    } catch (err) {
      console.error('Failed to open directory picker:', err);
    }
  }, [setWorkspacePath, t]);

  // Unpin a directory
  const handleUnpin = useCallback(
    (path: string) => {
      removePinnedDirectory(path);
    },
    [removePinnedDirectory],
  );

  const openNewSessionDialog = useCallback(
    async (targetPath?: string | null) => {
      const resolvedPath = (targetPath ?? workspacePath ?? '').trim();
      if (!resolvedPath) {
        onNewTask();
        return;
      }
      let existingWorktrees: Worktree[] = [];
      try {
        existingWorktrees = await listRepoWorktrees(resolvedPath);
      } catch (error) {
        console.warn('Failed to list repo worktrees for session creation dialog:', error);
      }
      setNewSessionDialog({
        open: true,
        workspacePath: resolvedPath,
        existingWorktrees,
      });
    },
    [listRepoWorktrees, onNewTask, workspacePath],
  );

  const handleConfirmNewSession = useCallback(
    (request: NewSessionRequest) => {
      setWorkspacePath(request.workspacePath);
      onNewTask(request);
      setNewSessionDialog((prev) => ({ ...prev, open: false }));
    },
    [onNewTask, setWorkspacePath],
  );

  // Restore session and set workspace path to match.
  // IMPORTANT: call onRestore first so that the current foreground session is
  // persisted with its original workspacePath before we overwrite settings.
  const handleRestore = useCallback(
    (id: string) => {
      onRestore(id);
      const session = history.find((h) => h.id === id);
      if (session?.workspacePath) {
        setWorkspacePath(session.workspacePath);
      }
    },
    [history, setWorkspacePath, onRestore],
  );

  const sessionCount = effectivePathGroups.reduce((total, group) => total + group.sessionCount, 0);
  const hasSessions = sessionCount > 0;
  const isEmpty = pinnedDirectories.length === 0 && !hasSessions;
  const tabs = useMemo<SidebarTabDef[]>(
    () => [
      { id: 'sessions', label: t('sidebar.sessions'), count: sessionCount },
      { id: 'skills', label: t('sidebar.skills'), count: detectedSkillCount },
      { id: 'plugins', label: t('sidebar.plugins'), count: pluginCount },
      { id: 'agents', label: t('sidebar.agents', { defaultValue: 'Agents' }), count: agentCount },
      { id: 'prompts', label: t('sidebar.prompts', { defaultValue: 'Prompts' }), count: promptCount },
    ],
    [t, sessionCount, detectedSkillCount, pluginCount, agentCount, promptCount],
  );

  const handleManageSkills = useCallback(() => {
    openSkillDialog('skills');
  }, [openSkillDialog]);

  const handleManagePlugins = useCallback(() => {
    setPluginDialogTab('installed');
    openPluginDialog();
  }, [setPluginDialogTab, openPluginDialog]);

  const handleOpenPluginMarketplace = useCallback(() => {
    setPluginDialogTab('marketplace');
    openPluginDialog();
  }, [setPluginDialogTab, openPluginDialog]);

  const handleManageAgents = useCallback(() => {
    openAgentDialog();
  }, [openAgentDialog]);

  const handleManagePrompts = useCallback(() => {
    openPromptDialog();
  }, [openPromptDialog]);

  const handleRequestArchive = useCallback((target: SessionManageTarget) => {
    setArchiveTarget(target);
  }, []);

  const handleCloseArchiveDialog = useCallback((open: boolean) => {
    if (open) return;
    setArchiveTarget(null);
    setBulkArchiveTargets(null);
  }, []);

  const handleConfirmArchive = useCallback(() => {
    if (!archiveTarget || archiveTarget.kind !== 'live') return;
    onArchiveWorkflowSession?.(archiveTarget.sourceSessionId);
    setArchiveTarget(null);
  }, [archiveTarget, onArchiveWorkflowSession]);

  const handleConfirmBulkArchive = useCallback(() => {
    const targets = bulkArchiveTargets ?? (archiveTarget ? [archiveTarget] : []);
    for (const target of targets) {
      if (target.kind === 'live') {
        onArchiveWorkflowSession?.(target.sourceSessionId);
      }
    }
    setArchiveTarget(null);
    setBulkArchiveTargets(null);
    setSelectionMode(false);
    setSelectedSessionIds(new Set());
  }, [bulkArchiveTargets, archiveTarget, onArchiveWorkflowSession]);

  const handleRequestRename = useCallback((target: SessionManageTarget) => {
    setRenameTarget(target);
    setRenameValue(target.title);
  }, []);

  const handleCloseRenameDialog = useCallback((open: boolean) => {
    if (open) return;
    setRenameTarget(null);
    setRenameValue('');
  }, []);

  const handleConfirmRename = useCallback(() => {
    if (!renameTarget) return;
    const trimmed = renameValue.trim();
    if (!trimmed) return;

    if (renameTarget.kind === 'history') {
      onRename(renameTarget.sourceSessionId, trimmed);
    } else {
      onRenameWorkflowSession?.(renameTarget.sourceSessionId, trimmed);
    }

    setRenameTarget(null);
    setRenameValue('');
  }, [onRename, onRenameWorkflowSession, renameTarget, renameValue]);

  const handleRequestDelete = useCallback((target: SessionManageTarget) => {
    setDeleteTarget(target);
  }, []);

  const allSessionTargets = useMemo(
    () =>
      effectivePathGroups.flatMap((group) =>
        group.children.map((item) => ({
          id: item.id,
          target: {
            kind: item.kind,
            sourceSessionId: item.sourceSessionId,
            title: item.title,
          } satisfies SessionManageTarget,
        })),
      ),
    [effectivePathGroups],
  );

  const handleRequestBulkArchive = useCallback(() => {
    const archiveableTargets = allSessionTargets
      .filter((item) => selectedSessionIds.has(item.id))
      .map((item) => item.target)
      .filter((target) => target.kind === 'live');
    if (archiveableTargets.length === 0) return;
    setBulkArchiveTargets(archiveableTargets);
  }, [allSessionTargets, selectedSessionIds]);

  useEffect(() => {
    const validIds = new Set(allSessionTargets.map((item) => item.id));
    setSelectedSessionIds((prev) => {
      const next = new Set(Array.from(prev).filter((id) => validIds.has(id)));
      if (next.size === prev.size) return prev;
      return next;
    });
  }, [allSessionTargets]);

  useEffect(() => {
    if (!selectionMode && selectedSessionIds.size > 0) {
      setSelectedSessionIds(new Set());
    }
  }, [selectionMode, selectedSessionIds]);

  const handleToggleSelect = useCallback((target: SessionManageTarget) => {
    setSelectedSessionIds((prev) => {
      const next = new Set(prev);
      const itemId = `${target.kind}:${target.sourceSessionId}`;
      if (next.has(itemId)) {
        next.delete(itemId);
      } else {
        next.add(itemId);
      }
      return next;
    });
  }, []);

  const selectedTargets = useMemo(
    () => allSessionTargets.filter((item) => selectedSessionIds.has(item.id)).map((item) => item.target),
    [allSessionTargets, selectedSessionIds],
  );

  const workflowDeleteMetaBySessionId = useMemo(
    () =>
      Object.fromEntries(
        workflowSessions.map((session) => [
          session.sessionId,
          {
            hasManagedWorktree: session.runtime?.runtimeKind === 'managed_worktree',
            runtimeLabel: session.runtime?.branch ?? session.runtime?.runtimePath ?? null,
            autoCleanupDefault:
              session.runtime?.runtimeKind === 'managed_worktree' && worktreeAutoCleanupOnSessionDelete,
          } satisfies WorkflowSessionDeleteMeta,
        ]),
      ) as Record<string, WorkflowSessionDeleteMeta>,
    [workflowSessions, worktreeAutoCleanupOnSessionDelete],
  );

  const deleteSessionMeta = useMemo(() => {
    if (deleteTarget?.kind !== 'live' && deleteTarget?.kind !== 'archived') return null;
    return workflowDeleteMetaBySessionId[deleteTarget.sourceSessionId] ?? null;
  }, [deleteTarget, workflowDeleteMetaBySessionId]);
  const bulkDeleteManagedCount = useMemo(
    () =>
      (bulkDeleteTargets ?? []).filter(
        (target) =>
          (target.kind === 'live' || target.kind === 'archived') &&
          workflowDeleteMetaBySessionId[target.sourceSessionId]?.hasManagedWorktree,
      ).length,
    [bulkDeleteTargets, workflowDeleteMetaBySessionId],
  );
  const [deleteManagedWorktree, setDeleteManagedWorktree] = useState(worktreeAutoCleanupOnSessionDelete);

  useEffect(() => {
    if (!deleteTarget && !bulkDeleteTargets) return;
    if (bulkDeleteTargets) {
      setDeleteManagedWorktree(
        worktreeAutoCleanupOnSessionDelete &&
          bulkDeleteTargets.some(
            (target) =>
              (target.kind === 'live' || target.kind === 'archived') &&
              workflowDeleteMetaBySessionId[target.sourceSessionId]?.hasManagedWorktree,
          ),
      );
      return;
    }
    setDeleteManagedWorktree(deleteSessionMeta?.autoCleanupDefault ?? false);
  }, [
    bulkDeleteTargets,
    deleteSessionMeta,
    deleteTarget,
    workflowDeleteMetaBySessionId,
    worktreeAutoCleanupOnSessionDelete,
  ]);

  const handleStartSelection = useCallback(() => {
    setSelectionMode(true);
  }, []);

  const handleCancelSelection = useCallback(() => {
    setSelectionMode(false);
    setSelectedSessionIds(new Set());
    setBulkArchiveTargets(null);
    setBulkDeleteTargets(null);
  }, []);

  const handleRequestBulkDelete = useCallback(() => {
    if (selectedTargets.length === 0) return;
    setBulkDeleteTargets(selectedTargets);
  }, [selectedTargets]);

  const handleCloseDeleteDialog = useCallback((open: boolean) => {
    if (open) return;
    setDeleteTarget(null);
    setBulkDeleteTargets(null);
  }, []);

  const handleConfirmDelete = useCallback(() => {
    const targets = bulkDeleteTargets ?? (deleteTarget ? [deleteTarget] : []);
    if (targets.length === 0) return;

    for (const target of targets) {
      if (target.kind === 'history') {
        onDelete(target.sourceSessionId);
      } else {
        const meta = workflowDeleteMetaBySessionId[target.sourceSessionId];
        onDeleteWorkflowSession?.(target.sourceSessionId, {
          deleteWorktree: meta?.hasManagedWorktree ? deleteManagedWorktree : false,
        });
      }
    }

    setDeleteTarget(null);
    setBulkDeleteTargets(null);
    setSelectionMode(false);
    setSelectedSessionIds(new Set());
  }, [
    bulkDeleteTargets,
    deleteManagedWorktree,
    deleteTarget,
    onDelete,
    onDeleteWorkflowSession,
    workflowDeleteMetaBySessionId,
  ]);

  return (
    <div className="h-full min-h-0 flex flex-col rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
      {/* Header: tabs + contextual actions */}
      <div className="px-3 py-3 border-b border-gray-200 dark:border-gray-700 space-y-2">
        <SidebarTabs tabs={tabs} activeTab={activeTab} onTabChange={setActiveTab} />
        <SidebarActions
          activeTab={activeTab}
          onNewTask={() => void openNewSessionDialog(workspacePath)}
          onAddDirectory={handleAddDirectory}
          sessionPathSort={sessionPathSort}
          onSessionPathSortChange={setSessionPathSort}
          showArchivedSessions={showArchivedSessions}
          onToggleShowArchivedSessions={() => setShowArchivedSessions(!showArchivedSessions)}
          onManageSkills={handleManageSkills}
          onManagePlugins={handleManagePlugins}
          onOpenPluginMarketplace={handleOpenPluginMarketplace}
          onManageAgents={handleManageAgents}
          onManagePrompts={handleManagePrompts}
        />
      </div>

      {/* Current task indicator */}
      {activeTab === 'sessions' ? (
        <div
          id="sidebar-panel-sessions"
          role="tabpanel"
          aria-labelledby="sidebar-tab-sessions"
          className="flex-1 min-h-0 flex flex-col"
        >
          <div className="flex-1 min-h-0 overflow-y-auto p-2 space-y-1">
            {isEmpty ? (
              <div className="h-full flex flex-col items-center justify-center text-center px-4 py-8">
                <svg className="w-8 h-8 text-gray-300 dark:text-gray-600 mb-2" fill="currentColor" viewBox="0 0 20 20">
                  <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
                </svg>
                <p className="text-xs text-gray-500 dark:text-gray-400">{t('sidebar.noDirectories')}</p>
                <p className="text-2xs text-gray-400 dark:text-gray-500 mt-1">{t('sidebar.noDirectoriesHint')}</p>
              </div>
            ) : (
              <>
                {effectivePathGroups.map((group) => {
                  const groupPath = group.path ? normalizeWorkspacePath(group.path) : group.normalizedPath;
                  const isActive =
                    activeNormalized !== null &&
                    groupPath !== null &&
                    (activeNormalized === groupPath || activeNormalized.startsWith(`${groupPath}/`));
                  return (
                    <PathGroupNode
                      key={group.normalizedPath}
                      group={group}
                      nowMs={relativeTimeNow}
                      isActive={isActive}
                      isExpanded={expandedPaths.has(group.normalizedPath)}
                      selectionMode={selectionMode}
                      selectedSessionIds={selectedSessionIds}
                      onToggleSelect={handleToggleSelect}
                      onToggle={() => toggleDirectory(group.normalizedPath)}
                      onUnpin={
                        group.path && pinnedDirectories.includes(group.path) ? () => handleUnpin(group.path!) : null
                      }
                      onNewTaskInPath={() => void openNewSessionDialog(group.path ?? workspacePath)}
                      onActivateLive={onSwitchWorkflowSession}
                      onRestoreHistory={handleRestore}
                      onRestoreArchived={onRestoreWorkflowSession}
                      onRequestArchive={handleRequestArchive}
                      onRequestRename={handleRequestRename}
                      onRequestDelete={handleRequestDelete}
                    />
                  );
                })}
              </>
            )}
          </div>
          {(history.length > 0 || workflowSessions.length > 0) && (
            <div className="px-3 py-2 border-t border-gray-200 dark:border-gray-700">
              {selectionMode ? (
                <div className="flex items-center gap-2">
                  <span className="min-w-0 flex-1 text-2xs text-gray-500 dark:text-gray-400">
                    {t('sidebar.selectionCount', {
                      count: selectedTargets.length,
                      defaultValue: `${selectedTargets.length} selected`,
                    })}
                  </span>
                  <button
                    type="button"
                    onClick={handleCancelSelection}
                    className="shrink-0 rounded-md px-2 py-1.5 text-xs text-gray-600 transition-colors hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-800"
                  >
                    {t('sidebar.cancelSelection', { defaultValue: 'Cancel' })}
                  </button>
                  <button
                    type="button"
                    onClick={handleRequestBulkArchive}
                    disabled={selectedTargets.length === 0 || !selectedTargets.some((t) => t.kind === 'live')}
                    className={clsx(
                      'shrink-0 rounded-md px-2 py-1.5 text-xs transition-colors',
                      selectedTargets.length === 0 || !selectedTargets.some((t) => t.kind === 'live')
                        ? 'cursor-not-allowed text-gray-400 dark:text-gray-500'
                        : 'text-amber-600 dark:text-amber-400 hover:bg-amber-50 dark:hover:bg-amber-900/20',
                    )}
                  >
                    {t('sidebar.archiveSelected', { defaultValue: 'Archive Selected' })}
                  </button>
                  <button
                    type="button"
                    onClick={handleRequestBulkDelete}
                    disabled={selectedTargets.length === 0}
                    className={clsx(
                      'shrink-0 rounded-md px-2 py-1.5 text-xs transition-colors',
                      selectedTargets.length === 0
                        ? 'cursor-not-allowed text-gray-400 dark:text-gray-500'
                        : 'text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20',
                    )}
                  >
                    {t('sidebar.deleteSelected', { defaultValue: 'Delete Selected' })}
                  </button>
                </div>
              ) : (
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={handleStartSelection}
                    className="shrink-0 rounded-md px-2 py-1.5 text-xs text-gray-600 transition-colors hover:bg-gray-100 dark:text-gray-300 dark:hover:bg-gray-800"
                  >
                    {t('sidebar.selectSessions', { defaultValue: 'Select' })}
                  </button>
                  <button
                    onClick={onClearAllSessions ?? onClear}
                    className={clsx(
                      'min-w-0 flex-1 text-xs px-2 py-1.5 rounded-md transition-colors',
                      'text-red-600 dark:text-red-400',
                      'hover:bg-red-50 dark:hover:bg-red-900/20',
                    )}
                  >
                    {t('sidebar.clearAllSessions', { defaultValue: t('sidebar.clearAll') })}
                  </button>
                </div>
              )}
            </div>
          )}
        </div>
      ) : activeTab === 'skills' ? (
        <div id="sidebar-panel-skills" role="tabpanel" aria-labelledby="sidebar-tab-skills" className="flex-1 min-h-0">
          <SkillMemoryPanel />
        </div>
      ) : activeTab === 'plugins' ? (
        <div
          id="sidebar-panel-plugins"
          role="tabpanel"
          aria-labelledby="sidebar-tab-plugins"
          className="flex-1 min-h-0"
        >
          <PluginPanel />
        </div>
      ) : activeTab === 'agents' ? (
        <div id="sidebar-panel-agents" role="tabpanel" aria-labelledby="sidebar-tab-agents" className="flex-1 min-h-0">
          <AgentPanel />
        </div>
      ) : (
        <div
          id="sidebar-panel-prompts"
          role="tabpanel"
          aria-labelledby="sidebar-tab-prompts"
          className="flex-1 min-h-0"
        >
          <PromptPanel />
        </div>
      )}

      {/* Skill & Memory Dialog (portal-rendered) */}
      <SkillMemoryDialog />

      {/* Plugin Dialog (portal-rendered) */}
      <PluginDialog />

      {/* Agent Dialog (portal-rendered) */}
      <AgentDialog />

      {/* Prompt Dialog (portal-rendered) */}
      <PromptDialog />

      <NewSessionDialog
        open={newSessionDialog.open}
        workspacePath={newSessionDialog.workspacePath}
        suggestedTaskName=""
        autoCleanupDefault={worktreeAutoCleanupOnSessionDelete}
        existingWorktrees={newSessionDialog.existingWorktrees}
        onOpenChange={(open) => setNewSessionDialog((prev) => ({ ...prev, open }))}
        onConfirm={handleConfirmNewSession}
      />

      <Dialog.Root open={archiveTarget !== null || bulkArchiveTargets !== null} onOpenChange={handleCloseArchiveDialog}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-[90] bg-black/40 backdrop-blur-[1px]" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-[100] w-[min(92vw,460px)] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-gray-200 bg-white p-5 shadow-xl dark:border-gray-700 dark:bg-gray-900">
            <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-gray-100">
              {bulkArchiveTargets
                ? t('sidebar.bulkArchiveSession', { defaultValue: 'Archive sessions' })
                : t('sidebar.archiveSession', { defaultValue: 'Archive session' })}
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-gray-600 dark:text-gray-300">
              {bulkArchiveTargets
                ? t('sidebar.bulkArchiveSessionConfirm', {
                    count: bulkArchiveTargets.length,
                    defaultValue: `Archive ${bulkArchiveTargets.length} selected live sessions?`,
                  })
                : t('sidebar.archiveSessionConfirm', { defaultValue: 'Archive this live session?' })}
            </Dialog.Description>
            <div className="mt-3 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:border-amber-900/60 dark:bg-amber-950/40 dark:text-amber-300">
              {bulkArchiveTargets ? (
                <ul className="max-h-32 overflow-y-auto">
                  {bulkArchiveTargets.map((target) => (
                    <li key={target.sourceSessionId} className="truncate">
                      {target.title}
                    </li>
                  ))}
                </ul>
              ) : (
                (archiveTarget?.title ?? '')
              )}
            </div>
            <div className="mt-5 flex items-center justify-end gap-2">
              <button
                type="button"
                onClick={() => handleCloseArchiveDialog(false)}
                className="rounded-md border border-gray-300 px-3 py-1.5 text-sm text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-800"
              >
                {t('sidebar.cancelAction', { defaultValue: 'Cancel' })}
              </button>
              <button
                type="button"
                onClick={bulkArchiveTargets ? handleConfirmBulkArchive : handleConfirmArchive}
                className="rounded-md bg-primary-600 px-3 py-1.5 text-sm text-white transition-colors hover:bg-primary-700"
              >
                {t('sidebar.archiveConfirmAction', { defaultValue: 'Confirm' })}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      <Dialog.Root open={renameTarget !== null} onOpenChange={handleCloseRenameDialog}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-[90] bg-black/40 backdrop-blur-[1px]" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-[100] w-[min(92vw,460px)] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-gray-200 bg-white p-5 shadow-xl dark:border-gray-700 dark:bg-gray-900">
            <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-gray-100">
              {t('sidebar.renamePrompt', { defaultValue: 'Rename session' })}
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-gray-600 dark:text-gray-300">
              {t('sidebar.renameDialogDescription', {
                defaultValue: 'Enter a new title for this session.',
              })}
            </Dialog.Description>
            <div className="mt-4">
              <label
                className="mb-2 block text-xs font-medium text-gray-700 dark:text-gray-300"
                htmlFor="sidebar-rename-session-input"
              >
                {t('sidebar.renameInputLabel', { defaultValue: 'Session title' })}
              </label>
              <input
                id="sidebar-rename-session-input"
                autoFocus
                type="text"
                value={renameValue}
                onChange={(event) => setRenameValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    event.preventDefault();
                    handleConfirmRename();
                  }
                }}
                className="w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 outline-none transition-colors focus:border-primary-500 focus:ring-2 focus:ring-primary-500/30 dark:border-gray-600 dark:bg-gray-950 dark:text-gray-100"
                placeholder={t('sidebar.renameInputPlaceholder', { defaultValue: 'Enter session title' })}
              />
            </div>
            <div className="mt-5 flex items-center justify-end gap-2">
              <button
                type="button"
                onClick={() => handleCloseRenameDialog(false)}
                className="rounded-md border border-gray-300 px-3 py-1.5 text-sm text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-800"
              >
                {t('sidebar.cancelAction', { defaultValue: 'Cancel' })}
              </button>
              <button
                type="button"
                onClick={handleConfirmRename}
                disabled={renameValue.trim().length === 0}
                className={clsx(
                  'rounded-md px-3 py-1.5 text-sm text-white transition-colors',
                  renameValue.trim().length === 0
                    ? 'cursor-not-allowed bg-gray-400 dark:bg-gray-600'
                    : 'bg-primary-600 hover:bg-primary-700',
                )}
              >
                {t('sidebar.renameConfirm', { defaultValue: 'Confirm' })}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      <Dialog.Root open={deleteTarget !== null || bulkDeleteTargets !== null} onOpenChange={handleCloseDeleteDialog}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-[90] bg-black/40 backdrop-blur-[1px]" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-[100] w-[min(92vw,460px)] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-gray-200 bg-white p-5 shadow-xl dark:border-gray-700 dark:bg-gray-900">
            <Dialog.Title className="text-base font-semibold text-gray-900 dark:text-gray-100">
              {t('sidebar.deleteSession', { defaultValue: 'Delete session' })}
            </Dialog.Title>
            <Dialog.Description className="mt-2 text-sm text-gray-600 dark:text-gray-300">
              {bulkDeleteTargets
                ? t('sidebar.bulkDeleteSessionConfirm', {
                    count: bulkDeleteTargets.length,
                    defaultValue: `Delete ${bulkDeleteTargets.length} selected sessions permanently? This cannot be undone.`,
                  })
                : t('sidebar.deleteSessionConfirm', {
                    defaultValue: 'Delete this session permanently? This cannot be undone.',
                  })}
            </Dialog.Description>
            <div className="mt-3 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-900/60 dark:bg-red-950/40 dark:text-red-300">
              {bulkDeleteTargets ? (
                <div className="space-y-1">
                  {bulkDeleteTargets.map((target) => (
                    <div key={`${target.kind}:${target.sourceSessionId}`}>{target.title}</div>
                  ))}
                </div>
              ) : (
                (deleteTarget?.title ?? '')
              )}
            </div>
            {(deleteSessionMeta?.hasManagedWorktree || bulkDeleteManagedCount > 0) && (
              <label className="mt-4 flex items-start gap-3 rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-sm text-amber-800 dark:border-amber-900/60 dark:bg-amber-950/40 dark:text-amber-300">
                <input
                  type="checkbox"
                  checked={deleteManagedWorktree}
                  onChange={(event) => setDeleteManagedWorktree(event.target.checked)}
                  className="mt-1"
                />
                <span>
                  {bulkDeleteManagedCount > 0
                    ? t('sidebar.deleteManagedWorktreesBulk', {
                        count: bulkDeleteManagedCount,
                        defaultValue: `Also delete ${bulkDeleteManagedCount} managed worktrees under ~/.plan-cascade/worktrees.`,
                      })
                    : t('sidebar.deleteManagedWorktree', {
                        defaultValue: 'Also delete the managed worktree under ~/.plan-cascade/worktrees.',
                      })}
                  {deleteSessionMeta?.runtimeLabel ? (
                    <span className="mt-1 block font-mono text-xs">{deleteSessionMeta.runtimeLabel}</span>
                  ) : null}
                </span>
              </label>
            )}
            <div className="mt-5 flex items-center justify-end gap-2">
              <button
                type="button"
                onClick={() => handleCloseDeleteDialog(false)}
                className="rounded-md border border-gray-300 px-3 py-1.5 text-sm text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-800"
              >
                {t('sidebar.cancelAction', { defaultValue: 'Cancel' })}
              </button>
              <button
                type="button"
                onClick={handleConfirmDelete}
                className="rounded-md bg-red-600 px-3 py-1.5 text-sm text-white transition-colors hover:bg-red-700"
              >
                {t('sidebar.deleteConfirmAction', { defaultValue: 'Delete' })}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>

      {/* Toast Notifications */}
      <SkillMemoryToast />
    </div>
  );
});

export default WorkspaceTreeSidebar;
