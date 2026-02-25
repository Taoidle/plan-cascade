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
import { ChevronRightIcon, PlusIcon, Cross2Icon } from '@radix-ui/react-icons';
import { type ExecutionHistoryItem, type SessionSnapshot } from '../../store/execution';
import { useSettingsStore } from '../../store/settings';
import { useSkillMemoryStore } from '../../store/skillMemory';
import { usePluginStore } from '../../store/plugins';
import { useAgentsStore } from '../../store/agents';
import { usePromptsStore } from '../../store/prompts';
import { Collapsible } from './Collapsible';
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
  onNewTask: () => void;
  currentTask: string | null;
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
  /** Background sessions belonging to this directory (keyed by ID) */
  backgroundSessions: Record<string, SessionSnapshot>;
}

/** Match a path against pinned directories, returning the matching normalized path or null */
function matchPinnedDirectory(
  sessionPath: string | null,
  normalizedPinned: { normalizedPath: string }[],
): string | null {
  if (!sessionPath) return null;
  for (const pin of normalizedPinned) {
    if (sessionPath === pin.normalizedPath || sessionPath.startsWith(pin.normalizedPath + '/')) {
      return pin.normalizedPath;
    }
  }
  return null;
}

function groupSessionsByDirectories(
  history: ExecutionHistoryItem[],
  pinnedDirectories: string[],
  backgroundSessions: Record<string, SessionSnapshot> = {},
): {
  directories: DirectoryGroup[];
  other: ExecutionHistoryItem[];
  unmatchedBgSessions: Record<string, SessionSnapshot>;
} {
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
      backgroundSessions: {},
    });
  }

  const other: ExecutionHistoryItem[] = [];

  // Group history sessions
  for (const session of history) {
    const sessionPath = normalizeWorkspacePath(session.workspacePath);
    const match = matchPinnedDirectory(sessionPath, normalizedPinned);
    if (match) {
      dirMap.get(match)!.sessions.push(session);
    } else {
      other.push(session);
    }
  }

  // Group background sessions by workspace path
  const unmatchedBgSessions: Record<string, SessionSnapshot> = {};
  for (const [id, snapshot] of Object.entries(backgroundSessions)) {
    const bgPath = normalizeWorkspacePath(snapshot.workspacePath);
    const match = matchPinnedDirectory(bgPath, normalizedPinned);
    if (match) {
      dirMap.get(match)!.backgroundSessions[id] = snapshot;
    } else {
      unmatchedBgSessions[id] = snapshot;
    }
  }

  const directories = normalizedPinned.map((pin) => dirMap.get(pin.normalizedPath)!).filter(Boolean);

  return { directories, other, unmatchedBgSessions };
}

// ---------------------------------------------------------------------------
// SidebarToolbar
// ---------------------------------------------------------------------------

function SidebarToolbar({
  onNewTask,
  onAddDirectory,
  onSkillsClick,
  onPluginsClick,
  onAgentsClick,
  onPromptsClick,
  skillCount,
  pluginCount,
  agentCount,
  promptCount,
}: {
  onNewTask: () => void;
  onAddDirectory: () => void;
  onSkillsClick: () => void;
  onPluginsClick: () => void;
  onAgentsClick: () => void;
  onPromptsClick: () => void;
  skillCount: number;
  pluginCount: number;
  agentCount: number;
  promptCount: number;
}) {
  const { t } = useTranslation('simpleMode');

  return (
    <div className="px-3 py-3 border-b border-gray-200 dark:border-gray-700 space-y-2">
      <button
        onClick={onNewTask}
        className={clsx(
          'w-full px-3 py-2 rounded-lg text-xs font-medium transition-colors',
          'bg-primary-600 text-white hover:bg-primary-700',
          'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:ring-offset-1',
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
            'hover:bg-gray-100 dark:hover:bg-gray-800',
          )}
          title={t('sidebar.addDirectory')}
        >
          <PlusIcon className="w-3.5 h-3.5" />
          <span>{t('sidebar.addDirectory')}</span>
        </button>

        <button
          data-testid="skills-button"
          onClick={onSkillsClick}
          className={clsx(
            'flex-1 flex items-center justify-center gap-1 px-2 py-1.5 rounded-md text-xs transition-colors',
            'text-gray-600 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
          )}
          title={t('sidebar.skills')}
        >
          {t('sidebar.skills')}
          {skillCount > 0 && (
            <span className="text-2xs px-1 py-0.5 rounded-full bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300 min-w-[1.2rem] text-center">
              {skillCount}
            </span>
          )}
        </button>

        <button
          data-testid="plugins-button"
          onClick={onPluginsClick}
          className={clsx(
            'flex-1 flex items-center justify-center gap-1 px-2 py-1.5 rounded-md text-xs transition-colors',
            'text-gray-600 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
          )}
          title={t('sidebar.plugins')}
        >
          {t('sidebar.plugins')}
          {pluginCount > 0 && (
            <span className="text-2xs px-1 py-0.5 rounded-full bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300 min-w-[1.2rem] text-center">
              {pluginCount}
            </span>
          )}
        </button>
      </div>

      <div className="flex items-center gap-1">
        <button
          data-testid="agents-button"
          onClick={onAgentsClick}
          className={clsx(
            'flex-1 flex items-center justify-center gap-1 px-2 py-1.5 rounded-md text-xs transition-colors',
            'text-gray-600 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
          )}
          title={t('sidebar.agents', { defaultValue: 'Agents' })}
        >
          {t('sidebar.agents', { defaultValue: 'Agents' })}
          {agentCount > 0 && (
            <span className="text-2xs px-1 py-0.5 rounded-full bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300 min-w-[1.2rem] text-center">
              {agentCount}
            </span>
          )}
        </button>

        <button
          data-testid="prompts-button"
          onClick={onPromptsClick}
          className={clsx(
            'flex-1 flex items-center justify-center gap-1 px-2 py-1.5 rounded-md text-xs transition-colors',
            'text-gray-600 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-800',
          )}
          title={t('sidebar.prompts', { defaultValue: 'Prompts' })}
        >
          {t('sidebar.prompts', { defaultValue: 'Prompts' })}
          {promptCount > 0 && (
            <span className="text-2xs px-1 py-0.5 rounded-full bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300 min-w-[1.2rem] text-center">
              {promptCount}
            </span>
          )}
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
    [session, onRename],
  );

  const handleDelete = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onDelete(session.id);
    },
    [session.id, onDelete],
  );

  return (
    <div
      className={clsx(
        'group flex items-start gap-2 py-1.5 pr-2 rounded-md cursor-pointer transition-colors',
        'hover:bg-gray-50 dark:hover:bg-gray-800',
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
          session.success ? 'bg-green-500 dark:bg-green-400' : 'bg-red-500 dark:bg-red-400',
        )}
        title={session.success ? 'Success' : 'Failed'}
      />

      {/* Content */}
      <div className="flex-1 min-w-0">
        <p className="text-xs text-gray-900 dark:text-white line-clamp-1">{session.title || session.taskDescription}</p>
        <p className="text-2xs text-gray-500 dark:text-gray-400 mt-0.5">{timeAgo(session.startedAt)}</p>
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
  onSwitchSession,
  onRemoveSession,
  foregroundParentSessionId,
  foregroundBgId,
  currentSessionDescription,
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
  onSwitchSession?: (id: string) => void;
  onRemoveSession?: (id: string) => void;
  foregroundParentSessionId?: string | null;
  foregroundBgId?: string | null;
  currentSessionDescription?: string;
}) {
  const { t } = useTranslation('simpleMode');

  const handleUnpin = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onUnpin();
    },
    [onUnpin],
  );

  const handleNewTask = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onNewTaskInDir();
    },
    [onNewTaskInDir],
  );

  // Build tree of background sessions for this directory
  const bgTree = useMemo(() => buildSessionTree(group.backgroundSessions), [group.backgroundSessions]);

  const bgCount = Object.keys(group.backgroundSessions).length;
  const totalCount = group.sessions.length + bgCount;

  return (
    <div>
      {/* Directory header row */}
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
      >
        {/* Expand/collapse chevron */}
        <ChevronRightIcon
          className={clsx(
            'w-3.5 h-3.5 text-gray-400 shrink-0 transition-transform duration-200',
            isExpanded && 'rotate-90',
          )}
        />

        {/* Folder icon */}
        <svg className="w-3.5 h-3.5 text-gray-500 dark:text-gray-400 shrink-0" fill="currentColor" viewBox="0 0 20 20">
          <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
        </svg>

        {/* Directory name */}
        <span className="flex-1 min-w-0 text-xs font-medium text-gray-900 dark:text-white truncate">
          {basename(group.path)}
        </span>

        {/* Session count badge */}
        {totalCount > 0 && (
          <span className="text-2xs px-1.5 py-0.5 rounded-full bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300 shrink-0">
            {totalCount}
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

      {/* Child items: background sessions (with fork tree) + history sessions */}
      {totalCount > 0 && (
        <Collapsible open={isExpanded}>
          <div className="mt-0.5">
            {/* Background sessions with fork hierarchy */}
            {onSwitchSession &&
              onRemoveSession &&
              bgTree.map((node) => (
                <BackgroundSessionTreeItem
                  key={node.id}
                  node={node}
                  depth={2}
                  onSwitch={onSwitchSession}
                  onRemove={onRemoveSession}
                  foregroundParentSessionId={foregroundParentSessionId}
                  foregroundBgId={foregroundBgId}
                  currentSessionDescription={currentSessionDescription}
                />
              ))}

            {/* History sessions */}
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
        </Collapsible>
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
          'hover:bg-gray-50 dark:hover:bg-gray-800',
        )}
        onClick={onToggle}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') onToggle();
        }}
      >
        <ChevronRightIcon
          className={clsx(
            'w-3.5 h-3.5 text-gray-400 shrink-0 transition-transform duration-200',
            isExpanded && 'rotate-90',
          )}
        />

        <span className="flex-1 min-w-0 text-xs font-medium text-gray-500 dark:text-gray-400">
          {t('sidebar.otherSessions')}
        </span>

        <span className="text-2xs px-1.5 py-0.5 rounded-full bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-300 shrink-0">
          {sessions.length}
        </span>
      </div>

      {/* Sessions */}
      <Collapsible open={isExpanded}>
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
      </Collapsible>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Session tree helpers
// ---------------------------------------------------------------------------

interface SessionTreeNode {
  id: string;
  snapshot: SessionSnapshot;
  children: SessionTreeNode[];
}

function buildSessionTree(sessions: Record<string, SessionSnapshot>): SessionTreeNode[] {
  const nodeMap = new Map<string, SessionTreeNode>();

  // Create nodes for all sessions
  for (const [id, snapshot] of Object.entries(sessions)) {
    nodeMap.set(id, { id, snapshot, children: [] });
  }

  const roots: SessionTreeNode[] = [];

  for (const [id, node] of nodeMap) {
    const parentId = node.snapshot.parentSessionId;
    // Safety: skip self-referencing parents and missing parents
    if (parentId && parentId !== id && nodeMap.has(parentId)) {
      nodeMap.get(parentId)!.children.push(node);
    } else {
      roots.push(node);
    }
  }

  return roots;
}

// ---------------------------------------------------------------------------
// Background session helpers
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

// ---------------------------------------------------------------------------
// BackgroundSessionTreeItem (recursive)
// ---------------------------------------------------------------------------

function BackgroundSessionTreeItem({
  node,
  depth,
  onSwitch,
  onRemove,
  foregroundParentSessionId,
  foregroundBgId,
  currentSessionDescription,
}: {
  node: SessionTreeNode;
  depth: number;
  onSwitch: (id: string) => void;
  onRemove: (id: string) => void;
  foregroundParentSessionId?: string | null;
  foregroundBgId?: string | null;
  currentSessionDescription?: string;
}) {
  const { t } = useTranslation('simpleMode');
  const [expanded, setExpanded] = useState(true);

  const isGhost = foregroundBgId === node.id;

  const label =
    isGhost && currentSessionDescription
      ? truncateLabel(currentSessionDescription)
      : node.snapshot.taskDescription
        ? truncateLabel(node.snapshot.taskDescription)
        : 'Untitled Session';

  const handleRemove = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onRemove(node.id);
    },
    [node.id, onRemove],
  );

  const handleToggle = useCallback((e: React.MouseEvent) => {
    e.stopPropagation();
    setExpanded((prev) => !prev);
  }, []);

  // With ghost, the ghost node itself represents the foreground — no separate (current) child indicator
  const showCurrentFork = !foregroundBgId && foregroundParentSessionId === node.id;
  const hasChildren = node.children.length > 0 || showCurrentFork;

  return (
    <div>
      <div
        data-testid={`bg-session-item-${node.id}`}
        className={clsx(
          'group flex items-start gap-1 py-1.5 pr-2 rounded-md transition-colors',
          isGhost
            ? 'bg-primary-50 dark:bg-primary-900/20 cursor-default'
            : 'cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800',
        )}
        style={{ paddingLeft: `${depth * 12 + 8}px` }}
        onClick={isGhost ? undefined : () => onSwitch(node.id)}
        role="button"
        tabIndex={isGhost ? -1 : 0}
        onKeyDown={
          isGhost
            ? undefined
            : (e) => {
                if (e.key === 'Enter' || e.key === ' ') onSwitch(node.id);
              }
        }
        title={isGhost ? undefined : t('sidebar.switchSession')}
      >
        {/* Expand/collapse chevron */}
        {hasChildren ? (
          <button className="p-0.5 shrink-0 text-gray-400" onClick={handleToggle} tabIndex={-1}>
            <ChevronRightIcon className={clsx('w-3 h-3 transition-transform duration-200', expanded && 'rotate-90')} />
          </button>
        ) : (
          <span className="w-4 shrink-0" />
        )}

        {/* Status dot */}
        <span
          data-testid={`bg-status-dot-${node.id}`}
          className={clsx(
            'mt-1 w-2 h-2 rounded-full shrink-0',
            isGhost ? 'bg-primary-500 dark:bg-primary-400' : getStatusDotClasses(node.snapshot.status),
          )}
        />

        {/* Content */}
        <div className="flex-1 min-w-0">
          <p
            className={clsx(
              'text-xs line-clamp-1',
              isGhost ? 'text-primary-700 dark:text-primary-300 font-medium' : 'text-gray-900 dark:text-white',
            )}
          >
            {label}
          </p>
          {isGhost ? (
            <p className="text-[10px] text-primary-500 dark:text-primary-400">{t('sidebar.currentFork')}</p>
          ) : (
            (node.snapshot.llmModel || node.snapshot.llmBackend) && (
              <p className="text-[10px] text-gray-400 dark:text-gray-500 line-clamp-1">
                {node.snapshot.llmModel || node.snapshot.llmBackend}
              </p>
            )
          )}
        </div>

        {/* Remove button (hidden for ghost) */}
        {!isGhost && (
          <button
            data-testid={`bg-remove-btn-${node.id}`}
            className="p-0.5 rounded text-gray-400 hover:text-red-400 hover:bg-red-900/30 opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
            onClick={handleRemove}
            title={t('sidebar.removeSession')}
          >
            <Cross2Icon className="w-3 h-3" />
          </button>
        )}
      </div>

      {/* Children */}
      {expanded && hasChildren && (
        <div>
          {/* Current foreground as highlighted child (only when no ghost) */}
          {showCurrentFork && (
            <div
              data-testid="current-fork-indicator"
              className="flex items-center gap-1 py-1 rounded-md"
              style={{ paddingLeft: `${(depth + 1) * 12 + 8}px` }}
            >
              <span className="w-4 shrink-0" />
              <span className="mt-0.5 w-2 h-2 rounded-full shrink-0 bg-primary-500 dark:bg-primary-400" />
              <span className="text-xs text-primary-600 dark:text-primary-400 font-medium line-clamp-1">
                {currentSessionDescription ? truncateLabel(currentSessionDescription) : t('sidebar.currentFork')}
              </span>
              <span className="text-[10px] text-primary-500 dark:text-primary-400 shrink-0">
                {t('sidebar.currentFork')}
              </span>
            </div>
          )}

          {/* Recursive child nodes */}
          {node.children.map((child) => (
            <BackgroundSessionTreeItem
              key={child.id}
              node={child}
              depth={depth + 1}
              onSwitch={onSwitch}
              onRemove={onRemove}
              foregroundParentSessionId={foregroundParentSessionId}
              foregroundBgId={foregroundBgId}
              currentSessionDescription={currentSessionDescription}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// BackgroundSessionsSection (only for sessions not matched to any directory)
// ---------------------------------------------------------------------------

function BackgroundSessionsSection({
  backgroundSessions,
  onSwitch,
  onRemove,
  foregroundParentSessionId,
  foregroundBgId,
  currentSessionDescription,
}: {
  backgroundSessions: Record<string, SessionSnapshot>;
  onSwitch: (id: string) => void;
  onRemove: (id: string) => void;
  foregroundParentSessionId?: string | null;
  foregroundBgId?: string | null;
  currentSessionDescription?: string;
}) {
  const { t } = useTranslation('simpleMode');

  const entries = useMemo(() => Object.entries(backgroundSessions), [backgroundSessions]);

  const tree = useMemo(() => buildSessionTree(backgroundSessions), [backgroundSessions]);

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

      {/* Session tree */}
      <div className="space-y-0.5">
        {tree.map((node) => (
          <BackgroundSessionTreeItem
            key={node.id}
            node={node}
            depth={0}
            onSwitch={onSwitch}
            onRemove={onRemove}
            foregroundParentSessionId={foregroundParentSessionId}
            foregroundBgId={foregroundBgId}
            currentSessionDescription={currentSessionDescription}
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
  foregroundParentSessionId,
  foregroundBgId,
}: WorkspaceTreeSidebarProps) {
  const { t } = useTranslation('simpleMode');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const pinnedDirectories = useSettingsStore((s) => s.pinnedDirectories);
  const addPinnedDirectory = useSettingsStore((s) => s.addPinnedDirectory);
  const removePinnedDirectory = useSettingsStore((s) => s.removePinnedDirectory);
  const setWorkspacePath = useSettingsStore((s) => s.setWorkspacePath);
  const skills = useSkillMemoryStore((s) => s.skills);
  const togglePanel = useSkillMemoryStore((s) => s.togglePanel);
  const plugins = usePluginStore((s) => s.plugins);
  const togglePluginPanel = usePluginStore((s) => s.togglePanel);
  const agentCount = useAgentsStore((s) => s.agents.length);
  const toggleAgentPanel = useAgentsStore((s) => s.togglePanel);
  const promptCount = usePromptsStore((s) => s.prompts.length);
  const togglePromptPanel = usePromptsStore((s) => s.togglePanel);

  // Count of detected/enabled skills for badge
  const detectedSkillCount = useMemo(() => skills.filter((s) => s.detected || s.enabled).length, [skills]);

  // Count of plugins for badge
  const pluginCount = useMemo(() => plugins.length, [plugins]);

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

  // Hide history rows that are already represented by active background snapshots
  const visibleHistory = useMemo(() => {
    const activeOriginHistoryIds = new Set<string>();
    const activeSessionIds = new Set<string>();
    for (const snapshot of Object.values(backgroundSessions)) {
      if (snapshot.originHistoryId) activeOriginHistoryIds.add(snapshot.originHistoryId);
      if (snapshot.taskId) {
        activeSessionIds.add(`claude:${snapshot.taskId}`);
      } else if (snapshot.standaloneSessionId) {
        activeSessionIds.add(`standalone:${snapshot.standaloneSessionId}`);
      }
    }
    if (activeOriginHistoryIds.size === 0 && activeSessionIds.size === 0) return history;
    return history.filter(
      (item) => !activeOriginHistoryIds.has(item.id) && !(item.sessionId && activeSessionIds.has(item.sessionId)),
    );
  }, [history, backgroundSessions]);

  // Group sessions (history + background) by pinned directories
  const { directories, other, unmatchedBgSessions } = useMemo(
    () => groupSessionsByDirectories(visibleHistory, pinnedDirectories, backgroundSessions),
    [visibleHistory, pinnedDirectories, backgroundSessions],
  );

  // Compute active normalized path for highlighting
  const activeNormalized = useMemo(() => normalizeWorkspacePath(workspacePath), [workspacePath]);

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
    [removePinnedDirectory],
  );

  // New task in a specific directory
  const handleNewTaskInDir = useCallback(
    (dirPath: string) => {
      setWorkspacePath(dirPath);
      onNewTask();
    },
    [setWorkspacePath, onNewTask],
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
    [history, setWorkspacePath, onRestore],
  );

  const hasBgSessions = Object.keys(backgroundSessions).length > 0;
  const isEmpty = pinnedDirectories.length === 0 && visibleHistory.length === 0 && !hasBgSessions;

  return (
    <div className="h-full min-h-0 flex flex-col rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
      {/* Toolbar */}
      <SidebarToolbar
        onNewTask={onNewTask}
        onAddDirectory={handleAddDirectory}
        onSkillsClick={togglePanel}
        onPluginsClick={togglePluginPanel}
        onAgentsClick={toggleAgentPanel}
        onPromptsClick={togglePromptPanel}
        skillCount={detectedSkillCount}
        pluginCount={pluginCount}
        agentCount={agentCount}
        promptCount={promptCount}
      />

      {/* Current task indicator */}
      {currentTask && (
        <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700 text-xs">
          <p className="text-gray-500 dark:text-gray-400">{t('sidebar.current')}</p>
          <p className="text-gray-700 dark:text-gray-200 line-clamp-2">{currentTask}</p>
        </div>
      )}

      {/* Scrollable tree content */}
      <div className="flex-1 min-h-0 overflow-y-auto p-2 space-y-1">
        {/* Background sessions not matched to any pinned directory */}
        {onSwitchSession && onRemoveSession && (
          <BackgroundSessionsSection
            backgroundSessions={unmatchedBgSessions}
            onSwitch={onSwitchSession}
            onRemove={onRemoveSession}
            foregroundParentSessionId={foregroundParentSessionId}
            foregroundBgId={foregroundBgId}
            currentSessionDescription={currentTask || undefined}
          />
        )}

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
            {/* Pinned directory nodes */}
            {directories.map((group) => {
              const isActive =
                activeNormalized !== null &&
                (activeNormalized === group.normalizedPath || activeNormalized.startsWith(group.normalizedPath + '/'));
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
                  onSwitchSession={onSwitchSession}
                  onRemoveSession={onRemoveSession}
                  foregroundParentSessionId={foregroundParentSessionId}
                  foregroundBgId={foregroundBgId}
                  currentSessionDescription={currentTask || undefined}
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

      {/* Skill & Memory Panel */}
      <SkillMemoryPanel />

      {/* Plugin Panel */}
      <PluginPanel />

      {/* Agent Panel */}
      <AgentPanel />

      {/* Prompt Panel */}
      <PromptPanel />

      {/* Footer */}
      {history.length > 0 && (
        <div className="px-3 py-2 border-t border-gray-200 dark:border-gray-700">
          <button
            onClick={onClear}
            className={clsx(
              'w-full text-xs px-2 py-1.5 rounded-md transition-colors',
              'text-red-600 dark:text-red-400',
              'hover:bg-red-50 dark:hover:bg-red-900/20',
            )}
          >
            {t('sidebar.clearAll')}
          </button>
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

      {/* Toast Notifications */}
      <SkillMemoryToast />
    </div>
  );
}

export default WorkspaceTreeSidebar;
