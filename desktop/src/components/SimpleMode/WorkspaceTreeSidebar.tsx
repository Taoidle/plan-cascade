/**
 * WorkspaceTreeSidebar Component
 *
 * Tree-structured sidebar that groups sessions by pinned workspace directories.
 * Provides directory management (pin/unpin), session actions (restore/rename/delete),
 * and a collapsible "Other" group for unmatched sessions.
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import {
  ChevronRightIcon,
  ChevronDownIcon,
  PlusIcon,
  Cross2Icon,
} from '@radix-ui/react-icons';
import { type ExecutionHistoryItem, type SessionSnapshot } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface WorkspaceTreeSidebarProps {
  history: ExecutionHistoryItem[];
  onRestore: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
  onClear: () => void;
  onNewTask: () => void;
  currentTask: string | null;
  /** Background session snapshots keyed by session ID */
  backgroundSessions?: Record<string, SessionSnapshot>;
  /** Called when user clicks a background session to switch to it */
  onSwitchSession?: (id: string) => void;
  /** Called when user clicks the remove button on a background session */
  onRemoveSession?: (id: string) => void;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function normalizeWorkspacePath(path: string | null | undefined): string | null {
  const value = (path || '').trim();
  if (!value) return null;
  return value.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
}

function basename(path: string): string {
  const normalized = path.replace(/\\/g, '/').replace(/\/+$/, '');
  const lastSlash = normalized.lastIndexOf('/');
  return lastSlash >= 0 ? normalized.slice(lastSlash + 1) : normalized;
}

function timeAgo(timestamp: number): string {
  const seconds = Math.floor((Date.now() - timestamp) / 1000);
  if (seconds < 60) return 'just now';
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

// ---------------------------------------------------------------------------
// Grouping logic
// ---------------------------------------------------------------------------

interface DirectoryGroup {
  path: string;
  normalizedPath: string;
  sessions: ExecutionHistoryItem[];
}

function groupSessionsByDirectories(
  history: ExecutionHistoryItem[],
  pinnedDirectories: string[]
): { directories: DirectoryGroup[]; other: ExecutionHistoryItem[] } {
  const normalizedPinned = pinnedDirectories.map((p) => ({
    path: p,
    normalizedPath: p.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase(),
  }));

  const dirMap = new Map<string, DirectoryGroup>();
  for (const pin of normalizedPinned) {
    dirMap.set(pin.normalizedPath, {
      path: pin.path,
      normalizedPath: pin.normalizedPath,
      sessions: [],
    });
  }

  const other: ExecutionHistoryItem[] = [];

  for (const session of history) {
    const sessionPath = normalizeWorkspacePath(session.workspacePath);
    if (!sessionPath) {
      other.push(session);
      continue;
    }

    let matched = false;
    for (const pin of normalizedPinned) {
      if (sessionPath === pin.normalizedPath || sessionPath.startsWith(pin.normalizedPath + '/')) {
        dirMap.get(pin.normalizedPath)!.sessions.push(session);
        matched = true;
        break;
      }
    }

    if (!matched) {
      other.push(session);
    }
  }

  const directories = normalizedPinned
    .map((pin) => dirMap.get(pin.normalizedPath)!)
    .filter(Boolean);

  return { directories, other };
}

// ---------------------------------------------------------------------------
// SidebarToolbar
// ---------------------------------------------------------------------------

function SidebarToolbar({
  onNewTask,
  onAddDirectory,
}: {
  onNewTask: () => void;
  onAddDirectory: () => void;
}) {
  const { t } = useTranslation('simpleMode');

  return (
    <div className="px-3 py-3 border-b border-gray-200 dark:border-gray-700 space-y-2">
      <button
        onClick={onNewTask}
        className={clsx(
          'w-full px-3 py-2 rounded-lg text-xs font-medium transition-colors',
          'bg-primary-600 text-white hover:bg-primary-700',
          'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1'
        )}
      >
        {t('sidebar.newTask')}
      </button>

      <div className="flex items-center gap-1">
        <button
          onClick={onAddDirectory}
          className={clsx(
            'flex-1 flex items-center justify-center gap-1 px-2 py-1.5 rounded-md text-xs transition-colors',
            'text-gray-600 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-800'
          )}
          title={t('sidebar.addDirectory')}
        >
          <PlusIcon className="w-3.5 h-3.5" />
          <span>{t('sidebar.addDirectory')}</span>
        </button>

        <button
          disabled
          className={clsx(
            'flex-1 flex items-center justify-center gap-1 px-2 py-1.5 rounded-md text-xs transition-colors',
            'text-gray-400 dark:text-gray-600 cursor-not-allowed opacity-50'
          )}
          title={t('sidebar.skills')}
        >
          {t('sidebar.skills')}
        </button>

        <button
          disabled
          className={clsx(
            'flex-1 flex items-center justify-center gap-1 px-2 py-1.5 rounded-md text-xs transition-colors',
            'text-gray-400 dark:text-gray-600 cursor-not-allowed opacity-50'
          )}
          title={t('sidebar.plugins')}
        >
          {t('sidebar.plugins')}
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// SessionItem
// ---------------------------------------------------------------------------

function SessionItem({
  session,
  depth,
  onRestore,
  onDelete,
  onRename,
}: {
  session: ExecutionHistoryItem;
  depth: number;
  onRestore: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
}) {
  const { t } = useTranslation('simpleMode');

  const handleRename = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      const current = session.title || session.taskDescription;
      const next = window.prompt('Rename session', current);
      if (next === null) return;
      onRename(session.id, next);
    },
    [session, onRename]
  );

  const handleDelete = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onDelete(session.id);
    },
    [session.id, onDelete]
  );

  return (
    <div
      className={clsx(
        'group flex items-start gap-2 py-1.5 pr-2 rounded-md cursor-pointer transition-colors',
        'hover:bg-gray-50 dark:hover:bg-gray-800'
      )}
      style={{ paddingLeft: `${depth * 12 + 8}px` }}
      onClick={() => onRestore(session.id)}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') onRestore(session.id);
      }}
    >
      {/* Status dot */}
      <span
        className={clsx(
          'mt-1.5 w-2 h-2 rounded-full shrink-0',
          session.success
            ? 'bg-green-500 dark:bg-green-400'
            : 'bg-red-500 dark:bg-red-400'
        )}
        title={session.success ? 'Success' : 'Failed'}
      />

      {/* Content */}
      <div className="flex-1 min-w-0">
        <p className="text-xs text-gray-900 dark:text-white line-clamp-1">
          {session.title || session.taskDescription}
        </p>
        <p className="text-2xs text-gray-500 dark:text-gray-400 mt-0.5">
          {timeAgo(session.startedAt)}
        </p>
      </div>

      {/* Hover actions */}
      <div className="flex items-center gap-0.5 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
        <button
          className="text-2xs px-1.5 py-0.5 rounded text-gray-500 hover:text-gray-200 hover:bg-gray-700/50"
          onClick={handleRename}
          title={t('sidebar.rename')}
        >
          {t('sidebar.rename')}
        </button>
        <button
          className="p-0.5 rounded text-gray-400 hover:text-red-400 hover:bg-red-900/30"
          onClick={handleDelete}
          title={t('sidebar.deleteSession')}
        >
          <Cross2Icon className="w-3 h-3" />
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// DirectoryNode
// ---------------------------------------------------------------------------

function DirectoryNode({
  group,
  isActive,
  isExpanded,
  onToggle,
  onUnpin,
  onNewTaskInDir,
  onRestore,
  onDelete,
  onRename,
}: {
  group: DirectoryGroup;
  isActive: boolean;
  isExpanded: boolean;
  onToggle: () => void;
  onUnpin: () => void;
  onNewTaskInDir: () => void;
  onRestore: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
}) {
  const { t } = useTranslation('simpleMode');

  const handleUnpin = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onUnpin();
    },
    [onUnpin]
  );

  const handleNewTask = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onNewTaskInDir();
    },
    [onNewTaskInDir]
  );

  return (
    <div>
      {/* Directory header row */}
      <div
        className={clsx(
          'group flex items-center gap-1 px-2 py-1.5 rounded-md cursor-pointer transition-colors',
          isActive
            ? 'bg-primary-50 dark:bg-primary-900/20'
            : 'hover:bg-gray-50 dark:hover:bg-gray-800'
        )}
        onClick={onToggle}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') onToggle();
        }}
      >
        {/* Expand/collapse chevron */}
        {isExpanded ? (
          <ChevronDownIcon className="w-3.5 h-3.5 text-gray-400 shrink-0" />
        ) : (
          <ChevronRightIcon className="w-3.5 h-3.5 text-gray-400 shrink-0" />
        )}

        {/* Folder icon */}
        <svg
          className="w-3.5 h-3.5 text-gray-500 dark:text-gray-400 shrink-0"
          fill="currentColor"
          viewBox="0 0 20 20"
        >
          <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
        </svg>

        {/* Directory name */}
        <span className="flex-1 min-w-0 text-xs font-medium text-gray-900 dark:text-white truncate">
          {basename(group.path)}
        </span>

        {/* Session count badge */}
        {group.sessions.length > 0 && (
          <span className="text-2xs px-1.5 py-0.5 rounded-full bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300 shrink-0">
            {group.sessions.length}
          </span>
        )}

        {/* Hover actions: unpin and new task */}
        <div className="flex items-center gap-0.5 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
          <button
            className="p-0.5 rounded text-gray-400 hover:text-primary-600 dark:hover:text-primary-400 hover:bg-primary-50 dark:hover:bg-primary-900/20"
            onClick={handleNewTask}
            title={t('sidebar.newTaskInDir')}
          >
            <PlusIcon className="w-3 h-3" />
          </button>
          <button
            className="p-0.5 rounded text-gray-400 hover:text-red-400 hover:bg-red-900/30"
            onClick={handleUnpin}
            title={t('sidebar.removeDirectory')}
          >
            <Cross2Icon className="w-3 h-3" />
          </button>
        </div>
      </div>

      {/* Child session items */}
      {isExpanded && group.sessions.length > 0 && (
        <div className="mt-0.5">
          {group.sessions.map((session) => (
            <SessionItem
              key={session.id}
              session={session}
              depth={2}
              onRestore={onRestore}
              onDelete={onDelete}
              onRename={onRename}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// OtherSessionsGroup
// ---------------------------------------------------------------------------

function OtherSessionsGroup({
  sessions,
  isExpanded,
  onToggle,
  onRestore,
  onDelete,
  onRename,
}: {
  sessions: ExecutionHistoryItem[];
  isExpanded: boolean;
  onToggle: () => void;
  onRestore: (id: string) => void;
  onDelete: (id: string) => void;
  onRename: (id: string, title: string) => void;
}) {
  const { t } = useTranslation('simpleMode');

  if (sessions.length === 0) return null;

  return (
    <div>
      {/* Group header */}
      <div
        className={clsx(
          'flex items-center gap-1 px-2 py-1.5 rounded-md cursor-pointer transition-colors',
          'hover:bg-gray-50 dark:hover:bg-gray-800'
        )}
        onClick={onToggle}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') onToggle();
        }}
      >
        {isExpanded ? (
          <ChevronDownIcon className="w-3.5 h-3.5 text-gray-400 shrink-0" />
        ) : (
          <ChevronRightIcon className="w-3.5 h-3.5 text-gray-400 shrink-0" />
        )}

        <span className="flex-1 min-w-0 text-xs font-medium text-gray-500 dark:text-gray-400">
          {t('sidebar.otherSessions')}
        </span>

        <span className="text-2xs px-1.5 py-0.5 rounded-full bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300 shrink-0">
          {sessions.length}
        </span>
      </div>

      {/* Sessions */}
      {isExpanded && (
        <div className="mt-0.5">
          {sessions.map((session) => (
            <SessionItem
              key={session.id}
              session={session}
              depth={2}
              onRestore={onRestore}
              onDelete={onDelete}
              onRename={onRename}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// BackgroundSessionItem
// ---------------------------------------------------------------------------

function getStatusDotClasses(status: string): string {
  switch (status) {
    case 'running':
    case 'streaming':
      return 'bg-blue-500 dark:bg-blue-400 animate-pulse';
    case 'completed':
      return 'bg-green-500 dark:bg-green-400';
    case 'failed':
      return 'bg-red-500 dark:bg-red-400';
    case 'paused':
      return 'bg-yellow-500 dark:bg-yellow-400';
    default:
      return 'bg-gray-400 dark:bg-gray-500';
  }
}

function truncateLabel(text: string, maxLen = 50): string {
  if (!text) return '';
  if (text.length <= maxLen) return text;
  return text.slice(0, maxLen) + '...';
}

function BackgroundSessionItem({
  sessionId,
  snapshot,
  onSwitch,
  onRemove,
}: {
  sessionId: string;
  snapshot: SessionSnapshot;
  onSwitch: (id: string) => void;
  onRemove: (id: string) => void;
}) {
  const { t } = useTranslation('simpleMode');

  const label = snapshot.taskDescription
    ? truncateLabel(snapshot.taskDescription)
    : 'Untitled Session';

  const handleRemove = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onRemove(sessionId);
    },
    [sessionId, onRemove]
  );

  return (
    <div
      data-testid={`bg-session-item-${sessionId}`}
      className={clsx(
        'group flex items-start gap-2 py-1.5 px-2 rounded-md cursor-pointer transition-colors',
        'hover:bg-gray-50 dark:hover:bg-gray-800'
      )}
      onClick={() => onSwitch(sessionId)}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') onSwitch(sessionId);
      }}
      title={t('sidebar.switchSession')}
    >
      {/* Status dot */}
      <span
        data-testid={`bg-status-dot-${sessionId}`}
        className={clsx(
          'mt-1.5 w-2 h-2 rounded-full shrink-0',
          getStatusDotClasses(snapshot.status)
        )}
      />

      {/* Content */}
      <div className="flex-1 min-w-0">
        <p className="text-xs text-gray-900 dark:text-white line-clamp-1">
          {label}
        </p>
        {(snapshot.llmModel || snapshot.llmBackend) && (
          <p className="text-[10px] text-gray-400 dark:text-gray-500 line-clamp-1">
            {snapshot.llmModel || snapshot.llmBackend}
          </p>
        )}
      </div>

      {/* Remove button */}
      <button
        data-testid={`bg-remove-btn-${sessionId}`}
        className="p-0.5 rounded text-gray-400 hover:text-red-400 hover:bg-red-900/30 opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
        onClick={handleRemove}
        title={t('sidebar.removeSession')}
      >
        <Cross2Icon className="w-3 h-3" />
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// BackgroundSessionsSection
// ---------------------------------------------------------------------------

function BackgroundSessionsSection({
  backgroundSessions,
  onSwitch,
  onRemove,
}: {
  backgroundSessions: Record<string, SessionSnapshot>;
  onSwitch: (id: string) => void;
  onRemove: (id: string) => void;
}) {
  const { t } = useTranslation('simpleMode');

  const entries = useMemo(
    () => Object.entries(backgroundSessions),
    [backgroundSessions]
  );

  if (entries.length === 0) return null;

  return (
    <div className="mb-2">
      {/* Section header */}
      <div className="flex items-center gap-1 px-2 py-1.5">
        <span className="flex-1 min-w-0 text-xs font-medium text-gray-500 dark:text-gray-400">
          {t('sidebar.activeSessions')}
        </span>
        <span className="text-2xs px-1.5 py-0.5 rounded-full bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-300 shrink-0">
          {entries.length}
        </span>
      </div>

      {/* Session items */}
      <div className="space-y-0.5">
        {entries.map(([id, snapshot]) => (
          <BackgroundSessionItem
            key={id}
            sessionId={id}
            snapshot={snapshot}
            onSwitch={onSwitch}
            onRemove={onRemove}
          />
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// WorkspaceTreeSidebar (main component)
// ---------------------------------------------------------------------------

export function WorkspaceTreeSidebar({
  history,
  onRestore,
  onDelete,
  onRename,
  onClear,
  onNewTask,
  currentTask,
  backgroundSessions = {},
  onSwitchSession,
  onRemoveSession,
}: WorkspaceTreeSidebarProps) {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const pinnedDirectories = useSettingsStore((s) => s.pinnedDirectories);
  const addPinnedDirectory = useSettingsStore((s) => s.addPinnedDirectory);
  const removePinnedDirectory = useSettingsStore((s) => s.removePinnedDirectory);
  const setWorkspacePath = useSettingsStore((s) => s.setWorkspacePath);

  // Expand/collapse state for directory paths and "other" group
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(() => new Set());
  const [otherExpanded, setOtherExpanded] = useState(true);

  // Auto-expand the active directory on mount / workspace change
  useEffect(() => {
    const activeNormalized = normalizeWorkspacePath(workspacePath);
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
  }, [workspacePath, pinnedDirectories]);

  // Group sessions by pinned directories
  const { directories, other } = useMemo(
    () => groupSessionsByDirectories(history, pinnedDirectories),
    [history, pinnedDirectories]
  );

  // Compute active normalized path for highlighting
  const activeNormalized = useMemo(
    () => normalizeWorkspacePath(workspacePath),
    [workspacePath]
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

  // Toggle "Other" group
  const toggleOther = useCallback(() => {
    setOtherExpanded((prev) => !prev);
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
        addPinnedDirectory(selected);
      }
    } catch (err) {
      console.error('Failed to open directory picker:', err);
    }
  }, [addPinnedDirectory, t]);

  // Unpin a directory
  const handleUnpin = useCallback(
    (path: string) => {
      removePinnedDirectory(path);
    },
    [removePinnedDirectory]
  );

  // New task in a specific directory
  const handleNewTaskInDir = useCallback(
    (dirPath: string) => {
      setWorkspacePath(dirPath);
      onNewTask();
    },
    [setWorkspacePath, onNewTask]
  );

  // Restore session and set workspace path to match
  const handleRestore = useCallback(
    (id: string) => {
      const session = history.find((h) => h.id === id);
      if (session?.workspacePath) {
        setWorkspacePath(session.workspacePath);
      }
      onRestore(id);
    },
    [history, setWorkspacePath, onRestore]
  );

  const isEmpty = pinnedDirectories.length === 0 && history.length === 0;

  return (
    <div className="min-h-0 flex flex-col rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
      {/* Toolbar */}
      <SidebarToolbar onNewTask={onNewTask} onAddDirectory={handleAddDirectory} />

      {/* Current task indicator */}
      {currentTask && (
        <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700 text-xs">
          <p className="text-gray-500 dark:text-gray-400">{t('sidebar.current')}</p>
          <p className="text-gray-700 dark:text-gray-200 line-clamp-2">{currentTask}</p>
        </div>
      )}

      {/* Scrollable tree content */}
      <div className="flex-1 min-h-0 overflow-y-auto p-2 space-y-1">
        {/* Background sessions section */}
        {onSwitchSession && onRemoveSession && (
          <BackgroundSessionsSection
            backgroundSessions={backgroundSessions}
            onSwitch={onSwitchSession}
            onRemove={onRemoveSession}
          />
        )}

        {isEmpty ? (
          <div className="h-full flex flex-col items-center justify-center text-center px-4 py-8">
            <svg
              className="w-8 h-8 text-gray-300 dark:text-gray-600 mb-2"
              fill="currentColor"
              viewBox="0 0 20 20"
            >
              <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
            </svg>
            <p className="text-xs text-gray-500 dark:text-gray-400">
              {t('sidebar.noDirectories')}
            </p>
            <p className="text-2xs text-gray-400 dark:text-gray-500 mt-1">
              {t('sidebar.noDirectoriesHint')}
            </p>
          </div>
        ) : (
          <>
            {/* Pinned directory nodes */}
            {directories.map((group) => {
              const isActive = activeNormalized !== null && (
                activeNormalized === group.normalizedPath ||
                activeNormalized.startsWith(group.normalizedPath + '/')
              );
              return (
                <DirectoryNode
                  key={group.normalizedPath}
                  group={group}
                  isActive={isActive}
                  isExpanded={expandedPaths.has(group.normalizedPath)}
                  onToggle={() => toggleDirectory(group.normalizedPath)}
                  onUnpin={() => handleUnpin(group.path)}
                  onNewTaskInDir={() => handleNewTaskInDir(group.path)}
                  onRestore={handleRestore}
                  onDelete={onDelete}
                  onRename={onRename}
                />
              );
            })}

            {/* Other sessions group */}
            <OtherSessionsGroup
              sessions={other}
              isExpanded={otherExpanded}
              onToggle={toggleOther}
              onRestore={handleRestore}
              onDelete={onDelete}
              onRename={onRename}
            />
          </>
        )}
      </div>

      {/* Footer */}
      {history.length > 0 && (
        <div className="px-3 py-2 border-t border-gray-200 dark:border-gray-700">
          <button
            onClick={onClear}
            className={clsx(
              'w-full text-xs px-2 py-1.5 rounded-md transition-colors',
              'text-red-600 dark:text-red-400',
              'hover:bg-red-50 dark:hover:bg-red-900/20'
            )}
          >
            {t('sidebar.clearAll')}
          </button>
        </div>
      )}
    </div>
  );
}

export default WorkspaceTreeSidebar;
