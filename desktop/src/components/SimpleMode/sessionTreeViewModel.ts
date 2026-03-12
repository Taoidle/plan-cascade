import i18n from '../../i18n';
import { buildDebugStateChips, summarizeDebugCase } from '../../lib/debugLabels';
import type { ExecutionHistoryItem } from '../../store/execution';
import type { SessionPathSort } from '../../store/settings';
import type {
  ChatState,
  ModeSnapshots,
  WorkflowBackgroundState,
  WorkflowMode,
  WorkflowSessionCatalogItem,
} from '../../types/workflowKernel';

export type SessionTreeStatus = 'idle' | 'running' | 'attention';

export interface SessionTreeItem {
  id: string;
  kind: 'live' | 'history' | 'archived';
  title: string;
  workspacePath: string | null;
  workspaceRootPath: string | null;
  runtimePath: string | null;
  runtimeKind: 'main' | 'managed_worktree' | 'legacy_worktree';
  runtimeBranch: string | null;
  runtimePrUrl: string | null;
  runtimePrState: string | null;
  managedWorktreeId: string | null;
  updatedAt: number;
  status: SessionTreeStatus;
  mode: WorkflowMode | null;
  isActive: boolean;
  sourceSessionId: string;
  badgeText: 'Live' | 'History' | 'Archived';
  detailSummary: string | null;
  detailChips: string[];
}

export interface PathGroup {
  path: string | null;
  normalizedPath: string;
  label: string;
  sessionCount: number;
  hasRunning: boolean;
  children: SessionTreeItem[];
}

const DEFAULT_TITLE_SET = new Set(['new chat', 'new plan', 'new task', 'new debug case', 'new session']);
const NO_WORKSPACE_KEY = '__no_workspace__';

function normalizePath(path: string | null | undefined): string | null {
  const value = (path || '').trim();
  if (!value) return null;
  return value.replace(/\\/g, '/').replace(/\/+$/, '').toLowerCase();
}

function basename(path: string): string {
  const normalized = path.replace(/\\/g, '/').replace(/\/+$/, '');
  const lastSlash = normalized.lastIndexOf('/');
  return lastSlash >= 0 ? normalized.slice(lastSlash + 1) : normalized;
}

function summarizeText(content: string): string {
  const normalized = content.trim();
  if (!normalized) {
    return i18n.t('simpleMode:sidebar.placeholderTitles.session', { defaultValue: 'New session' });
  }
  const firstLine =
    normalized
      .split('\n')
      .find((line) => line.trim().length > 0)
      ?.trim() ?? normalized;
  const truncated = firstLine.slice(0, 80);
  return firstLine.length > 80 ? `${truncated}...` : truncated;
}

function localizeDefaultTitle(rawTitle: string): string {
  const normalized = rawTitle.trim().toLowerCase();
  if (normalized === 'new chat') {
    return i18n.t('simpleMode:sidebar.placeholderTitles.chat', { defaultValue: 'New chat' });
  }
  if (normalized === 'new plan') {
    return i18n.t('simpleMode:sidebar.placeholderTitles.plan', { defaultValue: 'New plan' });
  }
  if (normalized === 'new task') {
    return i18n.t('simpleMode:sidebar.placeholderTitles.task', { defaultValue: 'New task' });
  }
  if (normalized === 'new debug case') {
    return i18n.t('simpleMode:sidebar.placeholderTitles.debug', { defaultValue: 'New debug case' });
  }
  return i18n.t('simpleMode:sidebar.placeholderTitles.session', { defaultValue: 'New session' });
}

function extractPlaceholderSummary(snapshot: ModeSnapshots, activeMode: WorkflowMode): string | null {
  const activeChat = snapshot.chat;
  if (activeChat?.lastUserMessage?.trim()) {
    return summarizeText(activeChat.lastUserMessage);
  }

  if (activeMode === 'chat') {
    return null;
  }

  if (activeChat?.lastAssistantMessage?.trim()) {
    return summarizeText(activeChat.lastAssistantMessage);
  }

  return null;
}

export function normalizeSidebarSessionTitle(
  displayTitle: string | null | undefined,
  options?: {
    modeSnapshots?: ModeSnapshots | null;
    activeMode?: WorkflowMode | null;
    fallbackText?: string | null;
  },
): string {
  const rawTitle = (displayTitle || '').trim();
  if (!rawTitle) {
    return options?.fallbackText?.trim() || 'New session';
  }

  if (!DEFAULT_TITLE_SET.has(rawTitle.toLowerCase())) {
    return rawTitle;
  }

  const summary =
    options?.modeSnapshots && options?.activeMode
      ? extractPlaceholderSummary(options.modeSnapshots, options.activeMode)
      : null;
  return summary || localizeDefaultTitle(rawTitle);
}

function isChatBusy(phase: ChatState['phase'] | string | null | undefined): boolean {
  return phase === 'submitting' || phase === 'streaming' || phase === 'paused';
}

function deriveDebugDetails(session: WorkflowSessionCatalogItem): {
  detailSummary: string | null;
  detailChips: string[];
} {
  const debug = session.modeSnapshots.debug;
  if (!debug) {
    return { detailSummary: null, detailChips: [] };
  }

  const detailChips = buildDebugStateChips(debug, { max: 4 });
  const detailSummary = summarizeDebugCase(debug, debug.pendingPrompt);

  return {
    detailSummary,
    detailChips,
  };
}

export function deriveSidebarSessionStatus(params: {
  modeSnapshots: ModeSnapshots;
  backgroundState: WorkflowBackgroundState;
  lastError: string | null;
  activeMode: WorkflowMode;
}): SessionTreeStatus {
  const { modeSnapshots, backgroundState, lastError } = params;

  if (
    lastError ||
    backgroundState === 'interrupted' ||
    modeSnapshots.chat?.phase === 'failed' ||
    modeSnapshots.plan?.phase === 'failed' ||
    modeSnapshots.task?.phase === 'failed' ||
    modeSnapshots.debug?.phase === 'failed'
  ) {
    return 'attention';
  }

  if (modeSnapshots.debug?.pendingApproval) {
    return 'attention';
  }

  if (
    backgroundState === 'background_running' ||
    modeSnapshots.plan?.phase === 'executing' ||
    modeSnapshots.task?.phase === 'executing' ||
    modeSnapshots.debug?.phase === 'gathering_signal' ||
    modeSnapshots.debug?.phase === 'hypothesizing' ||
    modeSnapshots.debug?.phase === 'identifying_root_cause' ||
    modeSnapshots.debug?.phase === 'verifying' ||
    modeSnapshots.debug?.phase === 'patching' ||
    isChatBusy(modeSnapshots.chat?.phase)
  ) {
    return 'running';
  }

  return 'idle';
}

function matchPinnedDirectory(
  sessionPath: string | null,
  normalizedPinned: { path: string; normalizedPath: string }[],
): { path: string; normalizedPath: string } | null {
  if (!sessionPath) return null;
  let bestMatch: { path: string; normalizedPath: string } | null = null;
  for (const pin of normalizedPinned) {
    if (sessionPath === pin.normalizedPath || sessionPath.startsWith(`${pin.normalizedPath}/`)) {
      if (!bestMatch || pin.normalizedPath.length > bestMatch.normalizedPath.length) {
        bestMatch = pin;
      }
    }
  }
  return bestMatch;
}

function createPathGroup(path: string | null, normalizedPath: string): PathGroup {
  return {
    path,
    normalizedPath,
    label: path ? basename(path) : i18n.t('simpleMode:sidebar.noWorkspace', { defaultValue: 'No Workspace' }),
    sessionCount: 0,
    hasRunning: false,
    children: [],
  };
}

function toLiveItem(session: WorkflowSessionCatalogItem, activeSessionId: string | null | undefined): SessionTreeItem {
  const kind = session.status === 'archived' ? 'archived' : 'live';
  const debugDetails =
    session.activeMode === 'debug' ? deriveDebugDetails(session) : { detailSummary: null, detailChips: [] };
  const runtime = session.runtime;
  return {
    id: `${kind}:${session.sessionId}`,
    kind,
    title: normalizeSidebarSessionTitle(session.displayTitle, {
      modeSnapshots: session.modeSnapshots,
      activeMode: session.activeMode,
    }),
    workspacePath: session.workspacePath,
    workspaceRootPath: runtime?.rootPath ?? session.workspacePath,
    runtimePath: runtime?.runtimePath ?? session.workspacePath,
    runtimeKind: runtime?.runtimeKind ?? 'main',
    runtimeBranch: runtime?.branch ?? null,
    runtimePrUrl: runtime?.prStatus?.url ?? null,
    runtimePrState: runtime?.prStatus?.state ?? null,
    managedWorktreeId: runtime?.managedWorktreeId ?? null,
    updatedAt: new Date(session.updatedAt).getTime(),
    status: deriveSidebarSessionStatus({
      modeSnapshots: session.modeSnapshots,
      backgroundState: session.backgroundState,
      lastError: session.lastError,
      activeMode: session.activeMode,
    }),
    mode: session.activeMode,
    isActive: activeSessionId === session.sessionId,
    sourceSessionId: session.sessionId,
    badgeText: kind === 'archived' ? 'Archived' : 'Live',
    detailSummary: debugDetails.detailSummary,
    detailChips: debugDetails.detailChips,
  };
}

function toHistoryItem(item: ExecutionHistoryItem): SessionTreeItem {
  return {
    id: `history:${item.id}`,
    kind: 'history',
    title: normalizeSidebarSessionTitle(item.title ?? item.taskDescription, {
      fallbackText: item.taskDescription,
    }),
    workspacePath: item.workspacePath ?? null,
    workspaceRootPath: item.workspacePath ?? null,
    runtimePath: item.workspacePath ?? null,
    runtimeKind: 'main',
    runtimeBranch: null,
    runtimePrUrl: null,
    runtimePrState: null,
    managedWorktreeId: null,
    updatedAt: item.completedAt ?? item.startedAt,
    status: item.error ? 'attention' : 'idle',
    mode: item.sessionId?.startsWith('claude:') ? 'chat' : null,
    isActive: false,
    sourceSessionId: item.id,
    badgeText: 'History',
    detailSummary: null,
    detailChips: [],
  };
}

function buildDedupeKey(title: string, workspacePath: string | null): string {
  const normalizedTitle = title.trim().toLowerCase();
  const normalizedWorkspace = normalizePath(workspacePath) ?? NO_WORKSPACE_KEY;
  return `${normalizedWorkspace}::${normalizedTitle}`;
}

export function buildSessionTreeViewModel(params: {
  workflowSessions?: WorkflowSessionCatalogItem[];
  history?: ExecutionHistoryItem[];
  activeSessionId?: string | null;
  pinnedDirectories?: string[];
  pathSort?: SessionPathSort;
  includeArchived?: boolean;
}): PathGroup[] {
  const workflowSessions = params.workflowSessions ?? [];
  const history = params.history ?? [];
  const pathSort = params.pathSort ?? 'recent';
  const includeArchived = params.includeArchived ?? false;
  const normalizedPinned = (params.pinnedDirectories ?? [])
    .map((path) => {
      const normalizedPath = normalizePath(path);
      return normalizedPath ? { path, normalizedPath } : null;
    })
    .filter((item): item is { path: string; normalizedPath: string } => Boolean(item));

  const groups = new Map<string, PathGroup>();
  for (const pin of normalizedPinned) {
    groups.set(pin.normalizedPath, createPathGroup(pin.path, pin.normalizedPath));
  }

  const workflowItems = workflowSessions
    .map((session) => toLiveItem(session, params.activeSessionId))
    .filter((item) => includeArchived || item.kind !== 'archived');

  const workflowKeys = new Set(workflowItems.map((item) => buildDedupeKey(item.title, item.workspaceRootPath)));
  const historyItems = history
    .map(toHistoryItem)
    .filter((item) => !workflowKeys.has(buildDedupeKey(item.title, item.workspaceRootPath)));

  const allItems: SessionTreeItem[] = [...workflowItems, ...historyItems];

  for (const item of allItems) {
    const normalizedWorkspacePath = normalizePath(item.workspaceRootPath);
    const matchedPin = matchPinnedDirectory(normalizedWorkspacePath, normalizedPinned);
    const groupKey = matchedPin?.normalizedPath ?? normalizedWorkspacePath ?? NO_WORKSPACE_KEY;
    const existing = groups.get(groupKey);
    const group =
      existing ??
      createPathGroup(
        matchedPin?.path ?? item.workspaceRootPath ?? null,
        matchedPin?.normalizedPath ?? normalizedWorkspacePath ?? NO_WORKSPACE_KEY,
      );

    group.children.push(item);
    group.sessionCount += 1;
    group.hasRunning = group.hasRunning || item.status === 'running';
    groups.set(groupKey, group);
  }

  for (const group of groups.values()) {
    group.children.sort((left, right) => right.updatedAt - left.updatedAt);
  }

  const sortedGroups = [...groups.values()];
  sortedGroups.sort((left, right) => {
    if (pathSort === 'name') {
      return left.label.localeCompare(right.label, undefined, { sensitivity: 'base' });
    }
    const leftUpdatedAt = left.children[0]?.updatedAt ?? 0;
    const rightUpdatedAt = right.children[0]?.updatedAt ?? 0;
    return rightUpdatedAt - leftUpdatedAt;
  });

  return sortedGroups;
}
