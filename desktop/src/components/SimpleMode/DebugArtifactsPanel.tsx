import { useEffect, useMemo, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { listDebugArtifacts, loadDebugArtifact } from '../../lib/debugArtifactsApi';
import type { DebugArtifactContent, DebugArtifactDescriptor } from '../../types/debugMode';

interface DebugArtifactsPanelProps {
  sessionId: string | null;
}

function formatDate(value: string): string {
  try {
    return new Date(value).toLocaleString();
  } catch {
    return value;
  }
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function DebugArtifactsPanel({ sessionId }: DebugArtifactsPanelProps) {
  const { t } = useTranslation('simpleMode');
  const [artifacts, setArtifacts] = useState<DebugArtifactDescriptor[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [selectedContent, setSelectedContent] = useState<DebugArtifactContent | null>(null);
  const [isLoadingList, setIsLoadingList] = useState(false);
  const [isLoadingContent, setIsLoadingContent] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    async function run() {
      if (!sessionId) {
        setArtifacts([]);
        setSelectedPath(null);
        setSelectedContent(null);
        return;
      }
      setIsLoadingList(true);
      setError(null);
      const result = await listDebugArtifacts(sessionId);
      if (cancelled) return;
      if (!result.success || !result.data) {
        setArtifacts([]);
        setSelectedPath(null);
        setSelectedContent(null);
        setError(
          result.error ||
            t('rightPanel.debugArtifacts.loadFailed', { defaultValue: 'Failed to load debug artifacts.' }),
        );
        setIsLoadingList(false);
        return;
      }
      const nextArtifacts = result.data;
      setArtifacts(nextArtifacts);
      setSelectedPath(
        (current) =>
          nextArtifacts.find((artifact) => artifact.path === current)?.path || nextArtifacts[0]?.path || null,
      );
      setIsLoadingList(false);
    }
    void run();
    return () => {
      cancelled = true;
    };
  }, [sessionId, t]);

  useEffect(() => {
    let cancelled = false;
    async function run() {
      if (!sessionId || !selectedPath) {
        setSelectedContent(null);
        return;
      }
      setIsLoadingContent(true);
      const result = await loadDebugArtifact(sessionId, selectedPath);
      if (cancelled) return;
      if (!result.success || !result.data) {
        setSelectedContent(null);
        setError(
          result.error ||
            t('rightPanel.debugArtifacts.previewFailed', { defaultValue: 'Failed to load artifact preview.' }),
        );
        setIsLoadingContent(false);
        return;
      }
      setSelectedContent(result.data);
      setError(null);
      setIsLoadingContent(false);
    }
    void run();
    return () => {
      cancelled = true;
    };
  }, [sessionId, selectedPath, t]);

  const previewText = useMemo(() => {
    if (!selectedContent) return null;
    const contentType = selectedContent.artifact.contentType;
    const isTextLike =
      contentType.startsWith('text/') ||
      contentType.includes('json') ||
      contentType === 'application/json' ||
      selectedContent.artifact.fileName.endsWith('.md') ||
      selectedContent.artifact.fileName.endsWith('.txt');
    if (!isTextLike) return null;
    try {
      return new TextDecoder('utf-8').decode(new Uint8Array(selectedContent.data));
    } catch {
      return null;
    }
  }, [selectedContent]);

  const prettyJson = useMemo(() => {
    if (!previewText || !selectedContent) return null;
    if (
      !selectedContent.artifact.contentType.includes('json') &&
      !selectedContent.artifact.fileName.endsWith('.json')
    ) {
      return null;
    }
    try {
      return JSON.stringify(JSON.parse(previewText), null, 2);
    } catch {
      return null;
    }
  }, [previewText, selectedContent]);

  if (!sessionId) {
    return (
      <div className="h-full flex items-center justify-center px-6 text-sm text-gray-500 dark:text-gray-400">
        {t('rightPanel.debugArtifacts.noSession', { defaultValue: 'Start a Debug case to inspect artifacts.' })}
      </div>
    );
  }

  return (
    <div className="h-full min-h-0 flex">
      <div className="w-72 shrink-0 border-r border-gray-200 dark:border-gray-700 overflow-y-auto">
        <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700">
          <p className="text-xs font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400">
            {t('rightPanel.debugArtifacts.title', { defaultValue: 'Debug artifacts' })}
          </p>
          <p className="text-2xs text-gray-500 dark:text-gray-400">
            {artifacts.length}{' '}
            {t('rightPanel.debugArtifacts.count', {
              defaultValue: 'artifacts',
            })}
          </p>
        </div>
        {isLoadingList ? (
          <div className="px-3 py-4 text-xs text-gray-500 dark:text-gray-400">
            {t('rightPanel.debugArtifacts.loading', { defaultValue: 'Loading artifacts...' })}
          </div>
        ) : artifacts.length === 0 ? (
          <div className="px-3 py-4 text-xs text-gray-500 dark:text-gray-400">
            {t('rightPanel.debugArtifacts.empty', { defaultValue: 'No debug artifacts are available yet.' })}
          </div>
        ) : (
          <div className="divide-y divide-gray-200 dark:divide-gray-700">
            {artifacts.map((artifact) => {
              const selected = artifact.path === selectedPath;
              return (
                <button
                  key={artifact.path}
                  onClick={() => setSelectedPath(artifact.path)}
                  className={clsx(
                    'w-full text-left px-3 py-3 transition-colors',
                    selected ? 'bg-primary-50 dark:bg-primary-900/20' : 'hover:bg-gray-50 dark:hover:bg-gray-800/70',
                  )}
                >
                  <p className="text-xs font-medium text-gray-900 dark:text-white">{artifact.fileName}</p>
                  <p className="mt-1 text-2xs text-gray-500 dark:text-gray-400">{artifact.kind}</p>
                  <div className="mt-1 flex items-center gap-2 text-2xs text-gray-500 dark:text-gray-400">
                    <span>{formatSize(artifact.sizeBytes)}</span>
                    <span>{formatDate(artifact.updatedAt)}</span>
                  </div>
                </button>
              );
            })}
          </div>
        )}
      </div>

      <div className="min-w-0 flex-1 overflow-y-auto">
        {error ? (
          <div className="px-4 py-4 text-sm text-rose-600 dark:text-rose-300">{error}</div>
        ) : isLoadingContent ? (
          <div className="px-4 py-4 text-sm text-gray-500 dark:text-gray-400">
            {t('rightPanel.debugArtifacts.loadingPreview', { defaultValue: 'Loading preview...' })}
          </div>
        ) : !selectedContent ? (
          <div className="px-4 py-4 text-sm text-gray-500 dark:text-gray-400">
            {t('rightPanel.debugArtifacts.selectArtifact', { defaultValue: 'Select an artifact to inspect it.' })}
          </div>
        ) : (
          <div className="p-4 space-y-4">
            <div className="space-y-1">
              <h3 className="text-sm font-semibold text-gray-900 dark:text-white">
                {selectedContent.artifact.fileName}
              </h3>
              <div className="flex flex-wrap items-center gap-3 text-xs text-gray-500 dark:text-gray-400">
                <span>{selectedContent.artifact.kind}</span>
                <span>{selectedContent.artifact.contentType}</span>
                <span>{formatSize(selectedContent.artifact.sizeBytes)}</span>
                <span>{formatDate(selectedContent.artifact.updatedAt)}</span>
              </div>
              <p className="text-xs text-gray-500 dark:text-gray-400 break-all">{selectedContent.artifact.path}</p>
            </div>

            {prettyJson ? (
              <pre className="max-h-[70vh] overflow-auto rounded-lg border border-gray-200 bg-gray-50 p-4 text-xs text-gray-800 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-200">
                {prettyJson}
              </pre>
            ) : previewText ? (
              <pre className="max-h-[70vh] overflow-auto rounded-lg border border-gray-200 bg-gray-50 p-4 text-xs whitespace-pre-wrap text-gray-800 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-200">
                {previewText}
              </pre>
            ) : (
              <div className="rounded-lg border border-gray-200 bg-gray-50 p-4 text-sm text-gray-500 dark:border-gray-700 dark:bg-gray-900 dark:text-gray-400">
                {t('rightPanel.debugArtifacts.binary', {
                  defaultValue: 'This artifact is binary or not previewable as text.',
                })}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
