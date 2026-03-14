/**
 * DocsIndexStatus Component
 *
 * Displays the current docs knowledge base indexing status in the bottom status bar.
 * Listens to Tauri runtime events and polls rag_get_docs_status.
 */

import { useEffect, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../../store/settings';
import { useProjectsStore } from '../../store/projects';
import { ragGetDocsStatus, ragRebuildDocsCollection, ragSyncDocsCollection } from '../../lib/knowledgeApi';

type DocsStatus =
  | 'idle'
  | 'none'
  | 'queued'
  | 'scanning'
  | 'indexing'
  | 'indexed'
  | 'changes_pending'
  | 'retry_waiting'
  | 'error';

interface IngestProgressEvent {
  project_id?: string;
  collection_id?: string;
  collection_name?: string;
  job_id?: string;
  stage: string;
  progress: number;
  detail: string;
}

interface DocsRuntimeStatusEvent {
  workspace_path?: string;
  project_id?: string;
  status?: string;
  last_error?: string | null;
  next_retry_at?: string | null;
}

interface DocsChangesEvent {
  workspace_path: string;
  changed_files: string[];
}

interface DocsIndexStatusProps {
  compact?: boolean;
  className?: string;
  workspacePathOverride?: string | null;
}

const DOCS_PREFIX = '[Docs] ';

function normalizePath(path: string): string {
  return path.replace(/\\/g, '/').replace(/\/+$/, '');
}

function normalizeDocsStatus(raw?: string): DocsStatus {
  switch (raw) {
    case 'none':
    case 'queued':
    case 'indexing':
    case 'indexed':
    case 'changes_pending':
    case 'retry_waiting':
    case 'error':
      return raw;
    default:
      return 'none';
  }
}

export function DocsIndexStatus({ compact = false, className, workspacePathOverride }: DocsIndexStatusProps) {
  const { t } = useTranslation('common');
  const settingsWorkspacePath = useSettingsStore((s) => s.workspacePath);
  const workspacePath = workspacePathOverride !== undefined ? workspacePathOverride : settingsWorkspacePath;
  const projectId = useProjectsStore((s) => s.selectedProject?.id ?? 'default');

  const [status, setStatus] = useState<DocsStatus>('idle');
  const [totalDocs, setTotalDocs] = useState(0);
  const [progress, setProgress] = useState(0);
  const [lastError, setLastError] = useState<string | null>(null);
  const [nextRetryAt, setNextRetryAt] = useState<string | null>(null);
  const [isMutating, setIsMutating] = useState(false);

  const fetchDocsStatus = useCallback(async () => {
    if (!workspacePath) {
      setStatus('idle');
      setTotalDocs(0);
      setProgress(0);
      setLastError(null);
      setNextRetryAt(null);
      return;
    }

    try {
      const result = await ragGetDocsStatus(workspacePath, projectId);
      if (result.success && result.data) {
        const data = result.data;
        setStatus(normalizeDocsStatus(data.status));
        setTotalDocs(data.total_docs);
        setLastError(data.last_error ?? null);
        setNextRetryAt(data.next_retry_at ?? null);
      }
    } catch {
      // Silently ignore — backend may not be ready
    }
  }, [workspacePath, projectId]);

  // Initial fetch + lightweight polling
  useEffect(() => {
    void fetchDocsStatus();
    if (!workspacePath) return;

    const timer = setInterval(() => {
      void fetchDocsStatus();
    }, 15000);

    return () => {
      clearInterval(timer);
    };
  }, [workspacePath, fetchDocsStatus]);

  // Listen for docs ingest progress (job scoped; filtered by project + docs prefix).
  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;

    listen<IngestProgressEvent>('knowledge:ingest-progress', (event) => {
      if (cancelled) return;
      const payload = event.payload;
      if (payload.project_id && payload.project_id !== projectId) return;
      if (!payload.collection_name || !payload.collection_name.startsWith(DOCS_PREFIX)) return;

      const normalizedProgress = Math.max(0, Math.min(100, payload.progress ?? 0));
      setProgress(normalizedProgress);

      if (payload.stage === 'queued') {
        setStatus('queued');
      } else if (payload.stage === 'chunking' && normalizedProgress === 0) {
        setStatus('scanning');
      } else if (payload.stage === 'chunking' || payload.stage === 'embedding' || payload.stage === 'storing') {
        setStatus('indexing');
      }

      if (payload.stage === 'storing' && normalizedProgress >= 100) {
        setStatus('indexed');
        void fetchDocsStatus();
      }
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [projectId, fetchDocsStatus]);

  // Listen for docs runtime status updates.
  useEffect(() => {
    if (!workspacePath) return;

    let cancelled = false;
    let unlisten: UnlistenFn | null = null;
    const currentWorkspace = normalizePath(workspacePath);

    listen<DocsRuntimeStatusEvent>('knowledge:docs-status', (event) => {
      if (cancelled) return;
      const payload = event.payload;
      const payloadWorkspace = normalizePath(payload.workspace_path ?? '');
      if (payloadWorkspace && payloadWorkspace !== currentWorkspace) return;
      if (payload.project_id && payload.project_id !== projectId) return;

      setStatus(normalizeDocsStatus(payload.status));
      setLastError(payload.last_error ?? null);
      setNextRetryAt(payload.next_retry_at ?? null);

      if (payload.status === 'indexed' || payload.status === 'changes_pending') {
        void fetchDocsStatus();
      }
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [workspacePath, projectId, fetchDocsStatus]);

  // Listen for docs-changes-detected events
  useEffect(() => {
    if (!workspacePath) return;

    let cancelled = false;
    let unlisten: UnlistenFn | null = null;
    const currentPath = normalizePath(workspacePath);

    listen<DocsChangesEvent>('knowledge:docs-changes-detected', (event) => {
      if (cancelled) return;
      const eventPath = normalizePath(event.payload.workspace_path);
      if (eventPath === currentPath && event.payload.changed_files.length > 0) {
        setStatus('changes_pending');
      }
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [workspacePath]);

  const handleSync = useCallback(async () => {
    if (!workspacePath || isMutating) return;
    setIsMutating(true);
    setStatus('indexing');
    setProgress(0);
    try {
      await ragSyncDocsCollection(workspacePath, projectId);
      await fetchDocsStatus();
    } catch (error) {
      setStatus('error');
      setLastError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsMutating(false);
    }
  }, [workspacePath, projectId, isMutating, fetchDocsStatus]);

  const handleRebuild = useCallback(async () => {
    if (!workspacePath || isMutating) return;
    setIsMutating(true);
    setStatus('indexing');
    setProgress(0);
    try {
      await ragRebuildDocsCollection(workspacePath, projectId, 'safe_swap');
      await fetchDocsStatus();
    } catch (error) {
      setStatus('error');
      setLastError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsMutating(false);
    }
  }, [workspacePath, projectId, isMutating, fetchDocsStatus]);

  // idle: render nothing
  if (status === 'idle') {
    return null;
  }

  // queued/scanning/indexing: spinner + progress
  if (status === 'queued' || status === 'scanning' || status === 'indexing') {
    const text =
      status === 'queued'
        ? t('docsIndexing.queued')
        : status === 'scanning'
          ? t('docsIndexing.scanning')
          : t('docsIndexing.indexing', { progress });

    return (
      <div className={clsx('flex items-center gap-1', className)}>
        <svg
          className={clsx('animate-spin text-amber-500 dark:text-amber-400', compact ? 'w-3 h-3' : 'w-4 h-4')}
          fill="none"
          viewBox="0 0 24 24"
        >
          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
        </svg>
        <span className={clsx('text-amber-600 dark:text-amber-400', compact ? 'text-xs' : 'text-sm')}>{text}</span>
      </div>
    );
  }

  // indexed: dot + doc count (including 0 docs)
  if (status === 'indexed') {
    return (
      <div className={clsx('flex items-center gap-1', className)}>
        <div className="w-2 h-2 rounded-full bg-green-500 flex-shrink-0" />
        <span className={clsx('text-green-600 dark:text-green-400', compact ? 'text-xs' : 'text-sm')}>
          {t('docsIndexing.indexed', { count: totalDocs })}
        </span>
      </div>
    );
  }

  // changes_pending: orange dot + sync button
  if (status === 'changes_pending') {
    return (
      <div className={clsx('flex items-center gap-1', className)}>
        <div className="w-2 h-2 rounded-full bg-orange-500 flex-shrink-0" />
        <button
          onClick={handleSync}
          disabled={isMutating}
          title={t('docsIndexing.changesPending')}
          className={clsx(
            'text-orange-600 dark:text-orange-400 hover:underline cursor-pointer',
            compact ? 'text-xs' : 'text-sm',
          )}
        >
          {isMutating ? t('docsIndexing.syncing') : t('docsIndexing.syncDocs')}
        </button>
      </div>
    );
  }

  // none: gray state + rebuild
  if (status === 'none') {
    return (
      <div className={clsx('flex items-center gap-1', className)}>
        <div className="w-2 h-2 rounded-full bg-gray-400 flex-shrink-0" />
        <button
          onClick={handleRebuild}
          disabled={isMutating}
          className={clsx(
            'text-gray-600 dark:text-gray-400 hover:underline cursor-pointer',
            compact ? 'text-xs' : 'text-sm',
          )}
        >
          {isMutating ? t('docsIndexing.rebuilding') : t('docsIndexing.rebuild')}
        </button>
      </div>
    );
  }

  // retry_waiting: warning + manual retry
  if (status === 'retry_waiting') {
    const title =
      nextRetryAt && nextRetryAt.trim().length > 0
        ? `${t('docsIndexing.retryWaiting')} (${nextRetryAt})`
        : t('docsIndexing.retryWaiting');

    return (
      <div className={clsx('flex items-center gap-1', className)}>
        <div className="w-2 h-2 rounded-full bg-orange-500 flex-shrink-0" />
        <button
          onClick={handleRebuild}
          disabled={isMutating}
          title={title}
          className={clsx(
            'text-orange-600 dark:text-orange-400 hover:underline cursor-pointer',
            compact ? 'text-xs' : 'text-sm',
          )}
        >
          {isMutating ? t('docsIndexing.rebuilding') : t('docsIndexing.retryNow')}
        </button>
      </div>
    );
  }

  // error: red dot + rebuild
  return (
    <div className={clsx('flex items-center gap-1', className)}>
      <div className="w-2 h-2 rounded-full bg-red-500 flex-shrink-0" />
      <button
        onClick={handleRebuild}
        disabled={isMutating}
        title={lastError ?? t('docsIndexing.error')}
        className={clsx(
          'text-red-600 dark:text-red-400 hover:underline cursor-pointer',
          compact ? 'text-xs' : 'text-sm',
        )}
      >
        {isMutating ? t('docsIndexing.rebuilding') : t('docsIndexing.rebuild')}
      </button>
    </div>
  );
}

export default DocsIndexStatus;
