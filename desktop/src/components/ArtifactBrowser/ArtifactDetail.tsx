/**
 * ArtifactDetail Component
 *
 * Shows full artifact metadata, version history timeline, and content preview.
 * Supports rendering markdown, displaying images, pretty-printing JSON,
 * and showing download links for binary files.
 */

import { useEffect, useMemo } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useArtifactsStore } from '../../store/artifacts';
import { ArtifactActions } from './ArtifactActions';
import { ArtifactVersionDiff } from './ArtifactVersionDiff';
import type { ArtifactMeta } from '../../lib/artifactsApi';

interface ArtifactDetailProps {
  artifact: ArtifactMeta;
  projectId: string;
  onBack: () => void;
}

export function ArtifactDetail({ artifact, projectId, onBack }: ArtifactDetailProps) {
  const { t } = useTranslation('artifacts');
  const { versionHistory, previewContent, isLoadingVersions, isLoadingPreview, fetchVersionHistory, loadPreview } =
    useArtifactsStore();

  // Fetch version history and preview on artifact change
  useEffect(() => {
    fetchVersionHistory(artifact.name, projectId);
    loadPreview(artifact.name, projectId);
  }, [artifact.name, projectId, fetchVersionHistory, loadPreview]);

  const formatDate = (dateStr: string): string => {
    try {
      return new Date(dateStr).toLocaleString();
    } catch {
      return dateStr;
    }
  };

  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  // Decode preview content as text if applicable
  const previewText = useMemo(() => {
    if (!previewContent) return null;
    const isTextType = ['text/', 'application/json', 'text/markdown', 'text/plain'].some(
      (t) => artifact.content_type.startsWith(t) || artifact.content_type.includes(t),
    );
    if (
      !isTextType &&
      !artifact.content_type.includes('json') &&
      !artifact.name.endsWith('.md') &&
      !artifact.name.endsWith('.txt')
    ) {
      return null;
    }
    try {
      const decoder = new TextDecoder('utf-8');
      return decoder.decode(new Uint8Array(previewContent));
    } catch {
      return null;
    }
  }, [previewContent, artifact.content_type, artifact.name]);

  // Check if content is JSON for pretty-printing
  const prettyJson = useMemo(() => {
    if (!previewText) return null;
    if (!artifact.content_type.includes('json') && !artifact.name.endsWith('.json')) return null;
    try {
      return JSON.stringify(JSON.parse(previewText), null, 2);
    } catch {
      return null;
    }
  }, [previewText, artifact.content_type, artifact.name]);

  return (
    <div className="h-full flex flex-col overflow-hidden">
      {/* Header */}
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
        <div className="flex items-center gap-3">
          {/* Back button (mobile) */}
          <button onClick={onBack} className="md:hidden text-gray-500 hover:text-gray-700 dark:hover:text-gray-300">
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <div>
            <h3 className="text-lg font-semibold text-gray-900 dark:text-white">{artifact.name}</h3>
            <div className="flex items-center gap-3 text-xs text-gray-500 dark:text-gray-400">
              <span>{artifact.content_type}</span>
              <span>v{artifact.current_version}</span>
              <span>{formatSize(artifact.size_bytes)}</span>
            </div>
          </div>
        </div>
        <ArtifactActions artifact={artifact} projectId={projectId} />
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        <div className="p-6 space-y-6">
          {/* Metadata */}
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
            <MetadataCard label={t('detail.id')} value={artifact.id} truncate />
            <MetadataCard label={t('detail.checksum')} value={artifact.checksum} truncate />
            <MetadataCard label={t('detail.createdAt')} value={formatDate(artifact.created_at)} />
            <MetadataCard label={t('detail.updatedAt')} value={formatDate(artifact.updated_at)} />
          </div>

          {/* Content Preview */}
          <div>
            <h4 className="text-sm font-semibold text-gray-900 dark:text-white mb-3">{t('detail.preview')}</h4>
            {isLoadingPreview ? (
              <div className="animate-pulse text-sm text-gray-500 py-4 text-center">{t('detail.loadingPreview')}</div>
            ) : prettyJson ? (
              <pre
                className={clsx(
                  'rounded-lg p-4 text-sm overflow-x-auto',
                  'bg-gray-50 dark:bg-gray-900',
                  'border border-gray-200 dark:border-gray-700',
                  'text-gray-800 dark:text-gray-200 font-mono',
                  'max-h-96',
                )}
              >
                {prettyJson}
              </pre>
            ) : previewText ? (
              <div
                className={clsx(
                  'rounded-lg p-4 text-sm overflow-x-auto',
                  'bg-gray-50 dark:bg-gray-900',
                  'border border-gray-200 dark:border-gray-700',
                  'text-gray-800 dark:text-gray-200 whitespace-pre-wrap',
                  'max-h-96',
                )}
              >
                {previewText}
              </div>
            ) : previewContent ? (
              <div
                className={clsx(
                  'rounded-lg p-4 text-center',
                  'bg-gray-50 dark:bg-gray-900',
                  'border border-gray-200 dark:border-gray-700',
                )}
              >
                <p className="text-sm text-gray-500 dark:text-gray-400">
                  {t('detail.binaryContent', { size: formatSize(previewContent.length) })}
                </p>
                <p className="text-xs text-gray-400 mt-1">{t('detail.useDownload')}</p>
              </div>
            ) : (
              <div className="text-sm text-gray-500 py-4 text-center">{t('detail.noPreview')}</div>
            )}
          </div>

          {/* Version History */}
          <div>
            <h4 className="text-sm font-semibold text-gray-900 dark:text-white mb-3">{t('detail.versionHistory')}</h4>
            {isLoadingVersions ? (
              <div className="animate-pulse text-sm text-gray-500 py-4 text-center">{t('detail.loadingVersions')}</div>
            ) : versionHistory.length === 0 ? (
              <p className="text-sm text-gray-500 dark:text-gray-400 text-center py-4">{t('detail.noVersions')}</p>
            ) : (
              <div className="relative">
                {/* Vertical timeline line */}
                <div className="absolute left-4 top-0 bottom-0 w-0.5 bg-gray-200 dark:bg-gray-700" />

                <div className="space-y-4">
                  {versionHistory.map((version, index) => (
                    <div key={version.id} className="relative flex items-start gap-4 pl-2">
                      {/* Timeline dot */}
                      <div
                        className={clsx(
                          'relative z-10 w-5 h-5 rounded-full border-2 flex items-center justify-center shrink-0',
                          index === 0
                            ? 'border-primary-500 bg-primary-100 dark:bg-primary-900'
                            : 'border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800',
                        )}
                      >
                        <div className={clsx('w-2 h-2 rounded-full', index === 0 ? 'bg-primary-500' : 'bg-gray-400')} />
                      </div>

                      {/* Version info */}
                      <div
                        className={clsx(
                          'flex-1 rounded-lg p-3',
                          'bg-gray-50 dark:bg-gray-900',
                          'border border-gray-200 dark:border-gray-700',
                        )}
                      >
                        <div className="flex items-center justify-between">
                          <span className="text-sm font-medium text-gray-900 dark:text-white">
                            v{version.version}
                            {index === 0 && (
                              <span className="ml-2 text-xs text-primary-600 dark:text-primary-400">
                                ({t('detail.latest')})
                              </span>
                            )}
                          </span>
                          <button
                            onClick={() => loadPreview(artifact.name, projectId, undefined, undefined, version.version)}
                            className="text-xs text-primary-600 hover:text-primary-800 dark:text-primary-400"
                          >
                            {t('detail.viewVersion')}
                          </button>
                        </div>
                        <div className="flex items-center gap-3 mt-1 text-xs text-gray-500 dark:text-gray-400">
                          <span>{formatSize(version.size_bytes)}</span>
                          <span className="font-mono truncate max-w-32" title={version.checksum}>
                            {version.checksum.substring(0, 12)}...
                          </span>
                          <span>{formatDate(version.created_at)}</span>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </div>

          {/* Version Diff (if 2+ versions) */}
          {versionHistory.length >= 2 && (
            <ArtifactVersionDiff artifact={artifact} projectId={projectId} versions={versionHistory} />
          )}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// MetadataCard
// ---------------------------------------------------------------------------

interface MetadataCardProps {
  label: string;
  value: string;
  truncate?: boolean;
}

function MetadataCard({ label, value, truncate }: MetadataCardProps) {
  return (
    <div
      className={clsx('rounded-lg p-3', 'bg-gray-50 dark:bg-gray-900', 'border border-gray-200 dark:border-gray-700')}
    >
      <div className="text-xs text-gray-500 dark:text-gray-400 mb-0.5">{label}</div>
      <div className={clsx('text-sm font-medium text-gray-900 dark:text-white', truncate && 'truncate')} title={value}>
        {value}
      </div>
    </div>
  );
}
