/**
 * ArtifactActions Component
 *
 * Action buttons for artifact operations: download, delete, copy path.
 */

import { useCallback, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useArtifactsStore } from '../../store/artifacts';
import type { ArtifactMeta } from '../../lib/artifactsApi';

interface ArtifactActionsProps {
  artifact: ArtifactMeta;
  projectId: string;
}

export function ArtifactActions({ artifact, projectId }: ArtifactActionsProps) {
  const { t } = useTranslation('artifacts');
  const { previewContent, deleteArtifact, isDeleting } = useArtifactsStore();
  const [copied, setCopied] = useState(false);

  const handleDownload = useCallback(() => {
    if (!previewContent) return;

    const blob = new Blob([new Uint8Array(previewContent)], {
      type: artifact.content_type || 'application/octet-stream',
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = artifact.name;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }, [previewContent, artifact]);

  const handleCopyPath = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(`${projectId}/${artifact.name}`);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard may not be available
    }
  }, [projectId, artifact.name]);

  return (
    <div className="flex items-center gap-2">
      {/* Download */}
      <button
        onClick={handleDownload}
        disabled={!previewContent}
        className={clsx(
          'p-2 rounded-lg transition-colors',
          'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300',
          'hover:bg-gray-100 dark:hover:bg-gray-800',
          'disabled:opacity-50 disabled:cursor-not-allowed'
        )}
        title={t('actions.download')}
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
        </svg>
      </button>

      {/* Copy Path */}
      <button
        onClick={handleCopyPath}
        className={clsx(
          'p-2 rounded-lg transition-colors',
          copied
            ? 'text-green-500'
            : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300',
          'hover:bg-gray-100 dark:hover:bg-gray-800'
        )}
        title={copied ? t('actions.copied') : t('actions.copyPath')}
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          {copied ? (
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
          ) : (
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3" />
          )}
        </svg>
      </button>
    </div>
  );
}
