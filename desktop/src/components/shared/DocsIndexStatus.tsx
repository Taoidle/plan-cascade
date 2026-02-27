/**
 * DocsIndexStatus Component
 *
 * Displays the current docs knowledge base indexing status in the bottom status bar.
 * Listens to Tauri "knowledge:ingest-progress" events (filtered by [Docs] prefix)
 * and polls rag_get_docs_status for initial/final state.
 */

import { useEffect, useState, useCallback, useRef } from 'react';
import { clsx } from 'clsx';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useTranslation } from 'react-i18next';
import { useSettingsStore } from '../../store/settings';
import { useProjectsStore } from '../../store/projects';
import { ragGetDocsStatus, ragSyncDocsCollection } from '../../lib/knowledgeApi';

type DocsStatus = 'idle' | 'scanning' | 'indexing' | 'indexed' | 'changes_pending' | 'error';

interface IngestProgressEvent {
  collection_name?: string;
  stage: string;
  progress: number;
  detail: string;
}

interface DocsChangesEvent {
  workspace_path: string;
  changed_files: string[];
}

interface DocsIndexStatusProps {
  compact?: boolean;
  className?: string;
}

const DOCS_PREFIX = '[Docs] ';

export function DocsIndexStatus({ compact = false, className }: DocsIndexStatusProps) {
  const { t } = useTranslation('common');
  const workspacePath = useSettingsStore((s) => s.workspacePath);
  const projectId = useProjectsStore((s) => s.selectedProject?.id ?? 'default');

  const [status, setStatus] = useState<DocsStatus>('idle');
  const [totalDocs, setTotalDocs] = useState(0);
  const [progress, setProgress] = useState(0);
  const [isSyncing, setIsSyncing] = useState(false);

  // Track previous status to detect indexing→indexed transitions
  const prevStatusRef = useRef<DocsStatus>('idle');

  const fetchDocsStatus = useCallback(async () => {
    if (!workspacePath) return;
    try {
      const result = await ragGetDocsStatus(workspacePath, projectId);
      if (result.success && result.data) {
        const data = result.data;
        if (data.status === 'indexed' && data.total_docs > 0) {
          setStatus('indexed');
          setTotalDocs(data.total_docs);
        } else if (data.status === 'changes_pending') {
          setStatus('changes_pending');
          setTotalDocs(data.total_docs);
        } else if (data.status === 'indexing') {
          setStatus('indexing');
        } else if (data.status === 'none') {
          setStatus('idle');
        } else {
          setStatus(data.status as DocsStatus);
        }
      }
    } catch {
      // Silently ignore — backend may not be ready
    }
  }, [workspacePath, projectId]);

  // Fetch initial status with retry backoff
  useEffect(() => {
    if (!workspacePath) {
      setStatus('idle');
      setTotalDocs(0);
      setProgress(0);
      return;
    }

    let cancelled = false;
    let retryTimer: ReturnType<typeof setTimeout> | null = null;
    let retryCount = 0;
    const maxRetries = 4;

    const poll = () => {
      if (cancelled) return;
      ragGetDocsStatus(workspacePath, projectId)
        .then((result) => {
          if (cancelled) return;
          if (result.success && result.data) {
            const data = result.data;
            if (data.status === 'indexed' && data.total_docs > 0) {
              setStatus('indexed');
              setTotalDocs(data.total_docs);
            } else if (data.status === 'changes_pending') {
              setStatus('changes_pending');
              setTotalDocs(data.total_docs);
            } else if (data.status === 'indexing') {
              setStatus('indexing');
            } else if (data.status === 'none' || (data.status === 'indexed' && data.total_docs === 0)) {
              // Still idle or zero docs — retry in case indexing hasn't started yet
              if (retryCount < maxRetries) {
                const delay = Math.min(2000 * Math.pow(2, retryCount), 16000);
                retryCount++;
                retryTimer = setTimeout(poll, delay);
              } else {
                setStatus('idle');
              }
            } else {
              setStatus(data.status as DocsStatus);
            }
          } else if (retryCount < maxRetries) {
            const delay = Math.min(2000 * Math.pow(2, retryCount), 16000);
            retryCount++;
            retryTimer = setTimeout(poll, delay);
          }
        })
        .catch(() => {
          if (!cancelled && retryCount < maxRetries) {
            const delay = Math.min(2000 * Math.pow(2, retryCount), 16000);
            retryCount++;
            retryTimer = setTimeout(poll, delay);
          }
        });
    };
    poll();

    return () => {
      cancelled = true;
      if (retryTimer) clearTimeout(retryTimer);
    };
  }, [workspacePath, projectId]);

  // Listen for ingest-progress events filtered to [Docs] collections
  useEffect(() => {
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;

    listen<IngestProgressEvent>('knowledge:ingest-progress', (event) => {
      if (cancelled) return;
      const payload = event.payload;
      if (!payload.collection_name || !payload.collection_name.startsWith(DOCS_PREFIX)) return;

      setProgress(payload.progress);

      if (payload.stage === 'chunking' && payload.progress === 0) {
        setStatus('scanning');
      } else if (payload.stage === 'chunking' || payload.stage === 'embedding') {
        setStatus('indexing');
      } else if (payload.stage === 'storing' && payload.progress >= 100) {
        // Indexing complete — fetch final status to get doc count
        setStatus('indexed');
        fetchDocsStatus();
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
  }, [fetchDocsStatus]);

  // Listen for docs-changes-detected events
  useEffect(() => {
    if (!workspacePath) return;

    let cancelled = false;
    let unlisten: UnlistenFn | null = null;

    listen<DocsChangesEvent>('knowledge:docs-changes-detected', (event) => {
      if (cancelled) return;
      // Normalize paths for comparison
      const eventPath = event.payload.workspace_path.replace(/\\/g, '/').replace(/\/+$/, '');
      const currentPath = workspacePath.replace(/\\/g, '/').replace(/\/+$/, '');
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

  // When transitioning from indexing → indexed, refresh doc count
  useEffect(() => {
    if (prevStatusRef.current === 'indexing' && status === 'indexed') {
      fetchDocsStatus();
    }
    prevStatusRef.current = status;
  }, [status, fetchDocsStatus]);

  const handleSync = useCallback(async () => {
    if (!workspacePath || isSyncing) return;
    setIsSyncing(true);
    setStatus('indexing');
    setProgress(0);
    try {
      await ragSyncDocsCollection(workspacePath, projectId);
      await fetchDocsStatus();
    } catch {
      setStatus('error');
    } finally {
      setIsSyncing(false);
    }
  }, [workspacePath, projectId, isSyncing, fetchDocsStatus]);

  // idle: render nothing
  if (status === 'idle') {
    return null;
  }

  // scanning/indexing: amber spinner + progress
  if (status === 'scanning' || status === 'indexing') {
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
        <span className={clsx('text-amber-600 dark:text-amber-400', compact ? 'text-xs' : 'text-sm')}>
          {status === 'scanning' ? t('docsIndexing.scanning') : t('docsIndexing.indexing', { progress })}
        </span>
      </div>
    );
  }

  // indexed: green dot + doc count
  if (status === 'indexed') {
    return (
      <div className={clsx('flex items-center gap-1', className)}>
        <div className="w-2 h-2 rounded-full bg-amber-500 flex-shrink-0" />
        <span className={clsx('text-amber-600 dark:text-amber-400', compact ? 'text-xs' : 'text-sm')}>
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
          disabled={isSyncing}
          title={t('docsIndexing.changesPending')}
          className={clsx(
            'text-orange-600 dark:text-orange-400 hover:underline cursor-pointer',
            compact ? 'text-xs' : 'text-sm',
          )}
        >
          {isSyncing ? t('docsIndexing.syncing') : t('docsIndexing.syncDocs')}
        </button>
      </div>
    );
  }

  // error: red dot
  return (
    <div className={clsx('flex items-center gap-1', className)}>
      <div className="w-2 h-2 rounded-full bg-red-500 flex-shrink-0" />
      <span className={clsx('text-red-600 dark:text-red-400', compact ? 'text-xs' : 'text-sm')}>
        {t('docsIndexing.error')}
      </span>
    </div>
  );
}

export default DocsIndexStatus;
