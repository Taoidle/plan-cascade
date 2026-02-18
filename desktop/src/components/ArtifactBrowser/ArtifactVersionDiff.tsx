/**
 * ArtifactVersionDiff Component
 *
 * Shows side-by-side diff for text-based artifacts when two versions
 * are selected.
 */

import { useState, useEffect, useMemo, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { artifactLoad } from '../../lib/artifactsApi';
import type { ArtifactMeta, ArtifactVersion } from '../../lib/artifactsApi';

interface ArtifactVersionDiffProps {
  artifact: ArtifactMeta;
  projectId: string;
  versions: ArtifactVersion[];
}

export function ArtifactVersionDiff({ artifact, projectId, versions }: ArtifactVersionDiffProps) {
  const { t } = useTranslation('artifacts');
  const [leftVersion, setLeftVersion] = useState<number | null>(null);
  const [rightVersion, setRightVersion] = useState<number | null>(null);
  const [leftText, setLeftText] = useState<string | null>(null);
  const [rightText, setRightText] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  // Auto-select the two latest versions
  useEffect(() => {
    if (versions.length >= 2) {
      setLeftVersion(versions[1].version);
      setRightVersion(versions[0].version);
    }
  }, [versions]);

  // Load text content when versions change
  const loadVersionText = useCallback(async (version: number): Promise<string | null> => {
    const result = await artifactLoad(artifact.name, projectId, null, null, version);
    if (result.success && result.data) {
      try {
        const decoder = new TextDecoder('utf-8');
        return decoder.decode(new Uint8Array(result.data));
      } catch {
        return null;
      }
    }
    return null;
  }, [artifact.name, projectId]);

  useEffect(() => {
    if (leftVersion === null || rightVersion === null) return;
    setIsLoading(true);
    Promise.all([
      loadVersionText(leftVersion),
      loadVersionText(rightVersion),
    ]).then(([left, right]) => {
      setLeftText(left);
      setRightText(right);
      setIsLoading(false);
    });
  }, [leftVersion, rightVersion, loadVersionText]);

  // Simple line diff computation
  const diffLines = useMemo(() => {
    if (!leftText || !rightText) return null;
    const leftLines = leftText.split('\n');
    const rightLines = rightText.split('\n');
    const maxLen = Math.max(leftLines.length, rightLines.length);
    const result: Array<{
      type: 'same' | 'added' | 'removed' | 'modified';
      left: string;
      right: string;
      lineNum: number;
    }> = [];

    for (let i = 0; i < maxLen; i++) {
      const left = i < leftLines.length ? leftLines[i] : '';
      const right = i < rightLines.length ? rightLines[i] : '';
      if (left === right) {
        result.push({ type: 'same', left, right, lineNum: i + 1 });
      } else if (i >= leftLines.length) {
        result.push({ type: 'added', left: '', right, lineNum: i + 1 });
      } else if (i >= rightLines.length) {
        result.push({ type: 'removed', left, right: '', lineNum: i + 1 });
      } else {
        result.push({ type: 'modified', left, right, lineNum: i + 1 });
      }
    }
    return result;
  }, [leftText, rightText]);

  const stats = useMemo(() => {
    if (!diffLines) return { added: 0, removed: 0, modified: 0 };
    return {
      added: diffLines.filter((l) => l.type === 'added').length,
      removed: diffLines.filter((l) => l.type === 'removed').length,
      modified: diffLines.filter((l) => l.type === 'modified').length,
    };
  }, [diffLines]);

  return (
    <div>
      <h4 className="text-sm font-semibold text-gray-900 dark:text-white mb-3">
        {t('diff.title')}
      </h4>

      {/* Version selectors */}
      <div className="flex items-center gap-4 mb-4">
        <div className="flex items-center gap-2">
          <label className="text-xs text-gray-500">{t('diff.from')}</label>
          <select
            value={leftVersion ?? ''}
            onChange={(e) => setLeftVersion(Number(e.target.value))}
            className={clsx(
              'px-2 py-1 rounded-md text-sm',
              'border border-gray-300 dark:border-gray-600',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white'
            )}
          >
            {versions.map((v) => (
              <option key={v.version} value={v.version}>v{v.version}</option>
            ))}
          </select>
        </div>
        <span className="text-gray-400">vs</span>
        <div className="flex items-center gap-2">
          <label className="text-xs text-gray-500">{t('diff.to')}</label>
          <select
            value={rightVersion ?? ''}
            onChange={(e) => setRightVersion(Number(e.target.value))}
            className={clsx(
              'px-2 py-1 rounded-md text-sm',
              'border border-gray-300 dark:border-gray-600',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white'
            )}
          >
            {versions.map((v) => (
              <option key={v.version} value={v.version}>v{v.version}</option>
            ))}
          </select>
        </div>
      </div>

      {/* Diff stats */}
      {diffLines && (
        <div className="flex items-center gap-4 mb-3 text-xs">
          <span className="text-green-600 dark:text-green-400">+{stats.added} {t('diff.added')}</span>
          <span className="text-red-600 dark:text-red-400">-{stats.removed} {t('diff.removed')}</span>
          <span className="text-yellow-600 dark:text-yellow-400">~{stats.modified} {t('diff.modified')}</span>
        </div>
      )}

      {/* Diff content */}
      {isLoading ? (
        <div className="animate-pulse text-sm text-gray-500 py-4 text-center">
          {t('diff.loading')}
        </div>
      ) : !leftText || !rightText ? (
        <div className="text-sm text-gray-500 py-4 text-center">
          {t('diff.binaryNotSupported')}
        </div>
      ) : diffLines ? (
        <div className={clsx(
          'rounded-lg border border-gray-200 dark:border-gray-700',
          'overflow-x-auto max-h-80 font-mono text-xs'
        )}>
          <table className="w-full">
            <tbody>
              {diffLines.map((line, i) => (
                <tr
                  key={i}
                  className={clsx(
                    line.type === 'same' && '',
                    line.type === 'added' && 'bg-green-50 dark:bg-green-900/20',
                    line.type === 'removed' && 'bg-red-50 dark:bg-red-900/20',
                    line.type === 'modified' && 'bg-yellow-50 dark:bg-yellow-900/20'
                  )}
                >
                  <td className="px-2 py-0.5 text-gray-400 select-none text-right w-8 border-r border-gray-200 dark:border-gray-700">
                    {line.lineNum}
                  </td>
                  <td className="px-2 py-0.5 w-1/2 border-r border-gray-200 dark:border-gray-700">
                    <span className={clsx(
                      line.type === 'removed' && 'text-red-700 dark:text-red-300',
                      line.type === 'modified' && 'text-yellow-700 dark:text-yellow-300',
                      line.type === 'same' && 'text-gray-700 dark:text-gray-300'
                    )}>
                      {line.left}
                    </span>
                  </td>
                  <td className="px-2 py-0.5 w-1/2">
                    <span className={clsx(
                      line.type === 'added' && 'text-green-700 dark:text-green-300',
                      line.type === 'modified' && 'text-yellow-700 dark:text-yellow-300',
                      line.type === 'same' && 'text-gray-700 dark:text-gray-300'
                    )}>
                      {line.right}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </div>
  );
}
