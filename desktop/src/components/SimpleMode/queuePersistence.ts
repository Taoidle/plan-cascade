import type { FileAttachmentData, WorkspaceFileReferenceData } from '../../types/attachment';

export type QueuePriority = 'high' | 'normal' | 'low';

export type QueueStatus = 'pending' | 'running' | 'succeeded' | 'failed' | 'blocked' | 'retrying';

export interface QueuedChatMessage {
  id: string;
  sessionId: string;
  prompt: string;
  submitAsFollowUp: boolean;
  mode: 'chat' | 'plan' | 'task';
  attempts: number;
  attachments: FileAttachmentData[];
  references: WorkspaceFileReferenceData[];
  priority: QueuePriority;
  status: QueueStatus;
  enqueueSeq: number;
  createdAt: string;
  lastError: string | null;
}

interface PersistedQueueV1 {
  version: 1;
  workspacePath: string | null;
  queue: Array<{
    id: string;
    prompt: string;
    submitAsFollowUp: boolean;
  }>;
}

interface PersistedQueueV2 {
  version: 2;
  workspacePath: string | null;
  queue: Array<{
    id: string;
    prompt: string;
    submitAsFollowUp: boolean;
    mode: 'chat' | 'plan' | 'task';
    attempts: number;
  }>;
}

interface PersistedQueueV3 {
  version: 3;
  workspacePath: string | null;
  queue: Array<{
    id: string;
    prompt: string;
    submitAsFollowUp: boolean;
    mode: 'chat' | 'plan' | 'task';
    attempts: number;
    attachments: FileAttachmentData[];
  }>;
}

interface PersistedQueueV5 {
  version: 5;
  workspacePath: string | null;
  queue: QueuedChatMessage[];
}

export const SIMPLE_CHAT_QUEUE_STORAGE_KEY = 'plan-cascade-simple-chat-queue-v5';

const LEGACY_STORAGE_KEYS = [
  'plan-cascade-simple-chat-queue-v4',
  'plan-cascade-simple-chat-queue-v3',
  'plan-cascade-simple-chat-queue-v2',
  'plan-cascade-simple-chat-queue-v1',
];

export interface PersistedQueueLoadResult {
  queue: QueuedChatMessage[];
  sourceVersion: number | null;
  sourceKey: string | null;
  migratedFromVersion: number | null;
  crossSessionCount: number;
}

function normalizeQueuePriority(value: unknown): QueuePriority {
  if (value === 'high' || value === 'normal' || value === 'low') return value;
  return 'normal';
}

function normalizeQueueStatus(value: unknown): QueueStatus {
  if (
    value === 'pending' ||
    value === 'running' ||
    value === 'succeeded' ||
    value === 'failed' ||
    value === 'blocked' ||
    value === 'retrying'
  ) {
    return value;
  }
  return 'pending';
}

function ensureIsoDatetime(value: unknown): string {
  if (typeof value !== 'string') return new Date(0).toISOString();
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return new Date(0).toISOString();
  return parsed.toISOString();
}

export function sanitizeQueuedAttachment(value: unknown): FileAttachmentData | null {
  if (!value || typeof value !== 'object') return null;
  const entry = value as Partial<FileAttachmentData>;
  if (
    typeof entry.id !== 'string' ||
    typeof entry.name !== 'string' ||
    typeof entry.path !== 'string' ||
    typeof entry.size !== 'number' ||
    (entry.type !== 'text' && entry.type !== 'image' && entry.type !== 'pdf' && entry.type !== 'unknown')
  ) {
    return null;
  }

  return {
    id: entry.id,
    name: entry.name,
    path: entry.path,
    size: entry.size,
    type: entry.type,
    mimeType: typeof entry.mimeType === 'string' ? entry.mimeType : undefined,
    isWorkspaceFile: typeof entry.isWorkspaceFile === 'boolean' ? entry.isWorkspaceFile : undefined,
    isAccessible: typeof entry.isAccessible === 'boolean' ? entry.isAccessible : undefined,
  };
}

function sanitizeQueuedAttachments(values: unknown[]): FileAttachmentData[] {
  return values
    .map((attachment) => sanitizeQueuedAttachment(attachment))
    .filter((attachment): attachment is FileAttachmentData => attachment !== null);
}

export function sanitizeQueuedReference(value: unknown): WorkspaceFileReferenceData | null {
  if (!value || typeof value !== 'object') return null;
  const entry = value as Partial<WorkspaceFileReferenceData>;
  if (
    typeof entry.id !== 'string' ||
    typeof entry.name !== 'string' ||
    typeof entry.relativePath !== 'string' ||
    typeof entry.absolutePath !== 'string' ||
    typeof entry.mentionText !== 'string'
  ) {
    return null;
  }

  return {
    id: entry.id,
    name: entry.name,
    relativePath: entry.relativePath,
    absolutePath: entry.absolutePath,
    mentionText: entry.mentionText,
  };
}

function sanitizeQueuedReferences(values: unknown[]): WorkspaceFileReferenceData[] {
  return values
    .map((reference) => sanitizeQueuedReference(reference))
    .filter((reference): reference is WorkspaceFileReferenceData => reference !== null);
}

function isQueuedChatMessage(value: unknown): value is QueuedChatMessage {
  if (!value || typeof value !== 'object') return false;
  const candidate = value as Partial<QueuedChatMessage>;
  const attachments = Array.isArray(candidate.attachments) ? candidate.attachments : null;
  const references = Array.isArray(candidate.references) ? candidate.references : null;

  return (
    typeof candidate.id === 'string' &&
    typeof candidate.sessionId === 'string' &&
    typeof candidate.prompt === 'string' &&
    typeof candidate.submitAsFollowUp === 'boolean' &&
    (candidate.mode === 'chat' || candidate.mode === 'plan' || candidate.mode === 'task') &&
    typeof candidate.attempts === 'number' &&
    attachments !== null &&
    attachments.every((attachment) => sanitizeQueuedAttachment(attachment) !== null) &&
    references !== null &&
    references.every((reference) => sanitizeQueuedReference(reference) !== null) &&
    typeof candidate.enqueueSeq === 'number'
  );
}

function normalizeLegacyEntry(
  entry: {
    id: string;
    prompt: string;
    submitAsFollowUp: boolean;
    mode: 'chat' | 'plan' | 'task';
    attempts: number;
    attachments?: FileAttachmentData[];
    references?: WorkspaceFileReferenceData[];
  },
  index: number,
  sessionId: string,
): QueuedChatMessage {
  return {
    id: entry.id,
    sessionId,
    prompt: entry.prompt,
    submitAsFollowUp: entry.submitAsFollowUp,
    mode: entry.mode,
    attempts: entry.attempts,
    attachments: entry.attachments ?? [],
    references: entry.references ?? [],
    priority: 'normal',
    status: 'pending',
    enqueueSeq: index,
    createdAt: new Date().toISOString(),
    lastError: null,
  };
}

export function snapshotQueueAttachments(attachments: FileAttachmentData[]): {
  attachments: FileAttachmentData[];
  droppedCount: number;
} {
  const snapshots: FileAttachmentData[] = [];
  let droppedCount = 0;

  for (const attachment of attachments) {
    const candidate = {
      id: attachment.id,
      name: attachment.name,
      path: attachment.path,
      size: attachment.size,
      type: attachment.type,
      mimeType: attachment.mimeType,
      isWorkspaceFile: attachment.isWorkspaceFile,
      isAccessible: attachment.isAccessible,
    };

    try {
      JSON.stringify(candidate);
    } catch {
      droppedCount += 1;
      continue;
    }

    const sanitized = sanitizeQueuedAttachment(candidate);
    if (!sanitized) {
      droppedCount += 1;
      continue;
    }
    snapshots.push(sanitized);
  }

  return { attachments: snapshots, droppedCount };
}

export function snapshotQueueReferences(references: WorkspaceFileReferenceData[]): WorkspaceFileReferenceData[] {
  return references
    .map((reference) => sanitizeQueuedReference(reference))
    .filter((reference): reference is WorkspaceFileReferenceData => reference !== null);
}

function clearQueueStorage(storage: Storage): void {
  storage.removeItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
  for (const key of LEGACY_STORAGE_KEYS) {
    storage.removeItem(key);
  }
}

export function loadPersistedSimpleChatQueue(
  storage: Storage,
  currentWorkspacePath: string,
  maxEntries: number,
  fallbackSessionId = '',
): QueuedChatMessage[] {
  return loadPersistedSimpleChatQueueWithMeta(storage, currentWorkspacePath, maxEntries, fallbackSessionId).queue;
}

export function loadPersistedSimpleChatQueueWithMeta(
  storage: Storage,
  currentWorkspacePath: string,
  maxEntries: number,
  fallbackSessionId = '',
): PersistedQueueLoadResult {
  const empty: PersistedQueueLoadResult = {
    queue: [],
    sourceVersion: null,
    sourceKey: null,
    migratedFromVersion: null,
    crossSessionCount: 0,
  };

  try {
    const raw = storage.getItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY);
    const rawLegacyEntry = LEGACY_STORAGE_KEYS.map((key) => ({
      key,
      value: storage.getItem(key),
    })).find((entry) => typeof entry.value === 'string');
    const rawLegacy = rawLegacyEntry?.value ?? null;
    if (!raw && !rawLegacy) return empty;

    const parsed = JSON.parse(raw ?? rawLegacy ?? '{}') as {
      version?: number;
      workspacePath?: string | null;
      queue?: unknown[];
    };
    const queue = Array.isArray(parsed.queue) ? parsed.queue : [];

    let normalizedQueue: QueuedChatMessage[] = [];

    if (parsed.version === 5) {
      normalizedQueue = queue.filter(isQueuedChatMessage).map((entry) => ({
        ...entry,
        priority: normalizeQueuePriority(entry.priority),
        status: normalizeQueueStatus(entry.status),
        enqueueSeq: Number.isFinite(entry.enqueueSeq) ? entry.enqueueSeq : 0,
        createdAt: ensureIsoDatetime(entry.createdAt),
        lastError: typeof entry.lastError === 'string' ? entry.lastError : null,
        attachments: sanitizeQueuedAttachments(entry.attachments),
        references: sanitizeQueuedReferences(entry.references),
      }));
    } else if (parsed.version === 4) {
      normalizedQueue = queue
        .filter(
          (entry): entry is Omit<QueuedChatMessage, 'references'> =>
            !!entry &&
            typeof entry === 'object' &&
            typeof (entry as { id?: unknown }).id === 'string' &&
            typeof (entry as { sessionId?: unknown }).sessionId === 'string' &&
            typeof (entry as { prompt?: unknown }).prompt === 'string' &&
            typeof (entry as { submitAsFollowUp?: unknown }).submitAsFollowUp === 'boolean' &&
            (((entry as { mode?: unknown }).mode as string) === 'chat' ||
              ((entry as { mode?: unknown }).mode as string) === 'task' ||
              ((entry as { mode?: unknown }).mode as string) === 'plan') &&
            typeof (entry as { attempts?: unknown }).attempts === 'number' &&
            Array.isArray((entry as { attachments?: unknown }).attachments) &&
            typeof (entry as { enqueueSeq?: unknown }).enqueueSeq === 'number',
        )
        .map((entry) => ({
          ...entry,
          priority: normalizeQueuePriority(entry.priority),
          status: normalizeQueueStatus(entry.status),
          enqueueSeq: Number.isFinite(entry.enqueueSeq) ? entry.enqueueSeq : 0,
          createdAt: ensureIsoDatetime(entry.createdAt),
          lastError: typeof entry.lastError === 'string' ? entry.lastError : null,
          attachments: sanitizeQueuedAttachments(entry.attachments),
          references: [],
        }));
    } else if (parsed.version === 3) {
      normalizedQueue = queue
        .filter(
          (entry): entry is PersistedQueueV3['queue'][number] =>
            !!entry &&
            typeof entry === 'object' &&
            typeof (entry as { id?: unknown }).id === 'string' &&
            typeof (entry as { prompt?: unknown }).prompt === 'string' &&
            typeof (entry as { submitAsFollowUp?: unknown }).submitAsFollowUp === 'boolean' &&
            (((entry as { mode?: unknown }).mode as string) === 'chat' ||
              ((entry as { mode?: unknown }).mode as string) === 'task' ||
              ((entry as { mode?: unknown }).mode as string) === 'plan') &&
            typeof (entry as { attempts?: unknown }).attempts === 'number' &&
            Array.isArray((entry as { attachments?: unknown }).attachments),
        )
        .map((entry, index) =>
          normalizeLegacyEntry(
            {
              id: entry.id,
              prompt: entry.prompt,
              submitAsFollowUp: entry.submitAsFollowUp,
              mode: entry.mode,
              attempts: entry.attempts,
              attachments: sanitizeQueuedAttachments(entry.attachments),
              references: [],
            },
            index,
            fallbackSessionId,
          ),
        );
    } else if (parsed.version === 2) {
      normalizedQueue = queue
        .filter(
          (entry): entry is PersistedQueueV2['queue'][number] =>
            !!entry &&
            typeof entry === 'object' &&
            typeof (entry as { id?: unknown }).id === 'string' &&
            typeof (entry as { prompt?: unknown }).prompt === 'string' &&
            typeof (entry as { submitAsFollowUp?: unknown }).submitAsFollowUp === 'boolean' &&
            (((entry as { mode?: unknown }).mode as string) === 'chat' ||
              ((entry as { mode?: unknown }).mode as string) === 'task' ||
              ((entry as { mode?: unknown }).mode as string) === 'plan') &&
            typeof (entry as { attempts?: unknown }).attempts === 'number',
        )
        .map((entry, index) =>
          normalizeLegacyEntry(
            {
              id: entry.id,
              prompt: entry.prompt,
              submitAsFollowUp: entry.submitAsFollowUp,
              mode: entry.mode,
              attempts: entry.attempts,
              attachments: [],
              references: [],
            },
            index,
            fallbackSessionId,
          ),
        );
    } else if (parsed.version === 1) {
      normalizedQueue = queue
        .filter(
          (entry): entry is PersistedQueueV1['queue'][number] =>
            !!entry &&
            typeof entry === 'object' &&
            typeof (entry as { id?: unknown }).id === 'string' &&
            typeof (entry as { prompt?: unknown }).prompt === 'string' &&
            typeof (entry as { submitAsFollowUp?: unknown }).submitAsFollowUp === 'boolean',
        )
        .map((entry, index) =>
          normalizeLegacyEntry(
            {
              id: entry.id,
              prompt: entry.prompt,
              submitAsFollowUp: entry.submitAsFollowUp,
              mode: 'chat',
              attempts: 0,
              attachments: [],
              references: [],
            },
            index,
            fallbackSessionId,
          ),
        );
    }

    const persistedWorkspace = typeof parsed.workspacePath === 'string' ? parsed.workspacePath : null;
    if (persistedWorkspace && currentWorkspacePath && persistedWorkspace !== currentWorkspacePath) {
      clearQueueStorage(storage);
      return empty;
    }

    if (normalizedQueue.length === 0) {
      clearQueueStorage(storage);
      return empty;
    }

    const sliced = normalizedQueue.slice(0, maxEntries);
    const sourceVersion = typeof parsed.version === 'number' ? parsed.version : null;
    const migratedFromVersion = sourceVersion && sourceVersion >= 1 && sourceVersion < 5 ? sourceVersion : null;
    const crossSessionCount = fallbackSessionId
      ? sliced.filter((item) => item.sessionId !== fallbackSessionId).length
      : 0;

    return {
      queue: sliced,
      sourceVersion,
      sourceKey: raw ? SIMPLE_CHAT_QUEUE_STORAGE_KEY : (rawLegacyEntry?.key ?? null),
      migratedFromVersion,
      crossSessionCount,
    };
  } catch {
    clearQueueStorage(storage);
    return empty;
  }
}

export function persistSimpleChatQueue(storage: Storage, queue: QueuedChatMessage[], workspacePath: string): boolean {
  if (queue.length === 0) {
    clearQueueStorage(storage);
    return true;
  }

  const payload: PersistedQueueV5 = {
    version: 5,
    workspacePath: workspacePath || null,
    queue,
  };

  try {
    storage.setItem(SIMPLE_CHAT_QUEUE_STORAGE_KEY, JSON.stringify(payload));
    for (const key of LEGACY_STORAGE_KEYS) {
      storage.removeItem(key);
    }
    return true;
  } catch {
    return false;
  }
}

export function clearPersistedSimpleChatQueue(storage: Storage): void {
  try {
    clearQueueStorage(storage);
  } catch {
    // Ignore storage failures.
  }
}
