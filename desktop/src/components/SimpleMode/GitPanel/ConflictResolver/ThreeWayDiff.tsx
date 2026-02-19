/**
 * ThreeWayDiff Component
 *
 * Three-column conflict resolution UI inspired by VS Code merge editor.
 * - OURS (left): Current branch content, read-only
 * - OUTPUT (center): Editable resolution area
 * - THEIRS (right): Merging branch content, read-only
 *
 * Feature-004: Branches Tab & Merge Conflict Resolution
 * Feature-005: AI-Powered Conflict Resolution
 */

import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import type { CommandResponse } from '../../../../lib/tauri';
import type { ConflictRegion } from '../../../../types/git';
import { useGitAI } from '../../../../hooks/useGitAI';
import { useToast } from '../../../shared/Toast';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ThreeWayDiffProps {
  repoPath: string;
  filePath: string;
  onResolved: () => void;
}

interface RegionResolution {
  regionIndex: number;
  content: string;
  method: 'ours' | 'theirs' | 'both' | 'manual' | 'ai';
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Build the full output content from original file content, conflict regions,
 * and resolution decisions.
 */
function buildResolvedContent(
  fileContent: string,
  regions: ConflictRegion[],
  resolutions: Map<number, RegionResolution>
): string {
  const lines = fileContent.split('\n');
  const result: string[] = [];
  let lineIdx = 0;

  for (let rIdx = 0; rIdx < regions.length; rIdx++) {
    const region = regions[rIdx];
    // Add lines before this conflict region (1-based start_line)
    while (lineIdx < region.start_line - 1 && lineIdx < lines.length) {
      result.push(lines[lineIdx]);
      lineIdx++;
    }

    // Add the resolved content for this region
    const resolution = resolutions.get(rIdx);
    if (resolution) {
      if (resolution.content) {
        result.push(resolution.content);
      }
    } else {
      // Not yet resolved - keep the conflict markers
      result.push(lines[lineIdx] || '');
      lineIdx++;
      while (lineIdx < region.end_line && lineIdx < lines.length) {
        result.push(lines[lineIdx]);
        lineIdx++;
      }
      continue;
    }

    // Skip past the conflict marker lines in the original
    lineIdx = region.end_line;
  }

  // Add remaining lines after the last conflict
  while (lineIdx < lines.length) {
    result.push(lines[lineIdx]);
    lineIdx++;
  }

  return result.join('\n');
}

// ---------------------------------------------------------------------------
// Spinner
// ---------------------------------------------------------------------------

function Spinner({ className }: { className?: string }) {
  return (
    <svg className={clsx('animate-spin', className)} fill="none" viewBox="0 0 24 24">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path
        className="opacity-75"
        fill="currentColor"
        d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
      />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// RegionActions
// ---------------------------------------------------------------------------

function RegionActions({
  regionIndex,
  resolution,
  isAIAvailable,
  isAIResolving,
  onAcceptOurs,
  onAcceptTheirs,
  onAcceptBoth,
  onManualEdit,
  onAIResolve,
}: {
  regionIndex: number;
  region: ConflictRegion;
  resolution: RegionResolution | undefined;
  isAIAvailable: boolean;
  isAIResolving: boolean;
  onAcceptOurs: (idx: number) => void;
  onAcceptTheirs: (idx: number) => void;
  onAcceptBoth: (idx: number) => void;
  onManualEdit: (idx: number) => void;
  onAIResolve: (idx: number) => void;
}) {
  const { t } = useTranslation('git');
  return (
    <div className="flex items-center gap-1 py-1">
      <span className="text-2xs font-medium text-gray-500 dark:text-gray-400 mr-1">
        {t('threeWayDiff.region', { index: regionIndex + 1 })}
        {resolution && (
          <span className={clsx(
            'ml-1',
            resolution.method === 'ai'
              ? 'text-purple-600 dark:text-purple-400'
              : 'text-green-600 dark:text-green-400'
          )}>
            ({resolution.method})
          </span>
        )}
      </span>
      <button
        onClick={() => onAcceptOurs(regionIndex)}
        className={clsx(
          'px-2 py-0.5 text-2xs rounded transition-colors',
          resolution?.method === 'ours'
            ? 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-400 font-medium'
            : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-blue-50 dark:hover:bg-blue-900/20'
        )}
      >
        {t('threeWayDiff.acceptOurs')}
      </button>
      <button
        onClick={() => onAcceptTheirs(regionIndex)}
        className={clsx(
          'px-2 py-0.5 text-2xs rounded transition-colors',
          resolution?.method === 'theirs'
            ? 'bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-400 font-medium'
            : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-purple-50 dark:hover:bg-purple-900/20'
        )}
      >
        {t('threeWayDiff.acceptTheirs')}
      </button>
      <button
        onClick={() => onAcceptBoth(regionIndex)}
        className={clsx(
          'px-2 py-0.5 text-2xs rounded transition-colors',
          resolution?.method === 'both'
            ? 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-400 font-medium'
            : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-amber-50 dark:hover:bg-amber-900/20'
        )}
      >
        {t('threeWayDiff.acceptBoth')}
      </button>
      <button
        onClick={() => onManualEdit(regionIndex)}
        className={clsx(
          'px-2 py-0.5 text-2xs rounded transition-colors',
          resolution?.method === 'manual'
            ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400 font-medium'
            : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-green-50 dark:hover:bg-green-900/20'
        )}
      >
        {t('threeWayDiff.manualEdit')}
      </button>
      {/* AI Resolve button */}
      <button
        onClick={() => onAIResolve(regionIndex)}
        disabled={!isAIAvailable || isAIResolving}
        className={clsx(
          'px-2 py-0.5 text-2xs rounded transition-colors flex items-center gap-1',
          resolution?.method === 'ai'
            ? 'bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-400 font-medium'
            : isAIAvailable && !isAIResolving
              ? 'bg-gray-100 dark:bg-gray-800 text-purple-600 dark:text-purple-400 hover:bg-purple-50 dark:hover:bg-purple-900/20'
              : 'bg-gray-50 dark:bg-gray-900 text-gray-400 dark:text-gray-600 cursor-not-allowed'
        )}
        title={
          !isAIAvailable
            ? t('threeWayDiff.configureLlm')
            : isAIResolving
              ? t('threeWayDiff.resolving')
              : t('threeWayDiff.aiResolve')
        }
      >
        {isAIResolving ? (
          <Spinner className="w-3 h-3" />
        ) : (
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09z"
            />
          </svg>
        )}
        {t('threeWayDiff.aiResolve')}
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// CodePanel
// ---------------------------------------------------------------------------

function CodePanel({
  title,
  titleColor,
  content,
  side,
}: {
  title: string;
  titleColor: string;
  content: string;
  regions: ConflictRegion[];
  side: 'ours' | 'theirs';
}) {
  const lines = content.split('\n');

  return (
    <div className="flex flex-col h-full border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
      <div className={clsx('shrink-0 px-3 py-1.5 text-xs font-semibold border-b border-gray-200 dark:border-gray-700', titleColor)}>
        {title}
      </div>
      <div className="flex-1 overflow-auto font-mono text-xs bg-gray-50 dark:bg-gray-950">
        <pre className="p-2 whitespace-pre-wrap break-words">
          {lines.map((line, idx) => (
            <div
              key={idx}
              className={clsx(
                'px-1',
                side === 'ours' ? 'hover:bg-blue-50/50 dark:hover:bg-blue-900/10' : 'hover:bg-purple-50/50 dark:hover:bg-purple-900/10'
              )}
            >
              <span className="inline-block w-8 text-right text-gray-400 dark:text-gray-600 mr-2 select-none">
                {idx + 1}
              </span>
              <span className="text-gray-800 dark:text-gray-200">{line}</span>
            </div>
          ))}
        </pre>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// OutputPanel
// ---------------------------------------------------------------------------

function OutputPanel({
  content,
  onChange,
  isEditing,
}: {
  content: string;
  onChange: (content: string) => void;
  isEditing: boolean;
}) {
  const { t } = useTranslation('git');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (isEditing && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [isEditing]);

  return (
    <div className="flex flex-col h-full border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
      <div className="shrink-0 px-3 py-1.5 text-xs font-semibold border-b border-gray-200 dark:border-gray-700 bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400">
        {t('threeWayDiff.outputResolved')}
      </div>
      <div className="flex-1 overflow-auto bg-gray-50 dark:bg-gray-950">
        {isEditing ? (
          <textarea
            ref={textareaRef}
            value={content}
            onChange={(e) => onChange(e.target.value)}
            className="w-full h-full p-2 font-mono text-xs bg-transparent text-gray-800 dark:text-gray-200 resize-none focus:outline-none"
            spellCheck={false}
          />
        ) : (
          <pre className="p-2 font-mono text-xs whitespace-pre-wrap break-words">
            {content.split('\n').map((line, idx) => (
              <div key={idx} className="px-1 hover:bg-green-50/50 dark:hover:bg-green-900/10">
                <span className="inline-block w-8 text-right text-gray-400 dark:text-gray-600 mr-2 select-none">
                  {idx + 1}
                </span>
                <span className="text-gray-800 dark:text-gray-200">{line}</span>
              </div>
            ))}
          </pre>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// ThreeWayDiff Component
// ---------------------------------------------------------------------------

export function ThreeWayDiff({ repoPath, filePath, onResolved }: ThreeWayDiffProps) {
  const { t } = useTranslation('git');
  const [fileContent, setFileContent] = useState('');
  const [regions, setRegions] = useState<ConflictRegion[]>([]);
  const [resolutions, setResolutions] = useState<Map<number, RegionResolution>>(new Map());
  const [editingRegion, setEditingRegion] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isResolving, setIsResolving] = useState(false);
  const [isAIResolvingAll, setIsAIResolvingAll] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { isAvailable, resolveConflictAI, resolvingFiles } = useGitAI();
  const { showToast } = useToast();

  const isFileAIResolving = resolvingFiles.has(filePath) || isAIResolvingAll;

  // Load file content and parse conflicts
  useEffect(() => {
    let cancelled = false;

    const load = async () => {
      setIsLoading(true);
      setError(null);
      setResolutions(new Map());
      setEditingRegion(null);

      try {
        const [contentRes, conflictsRes] = await Promise.all([
          invoke<CommandResponse<string>>('git_read_file_content', {
            repoPath,
            filePath,
          }),
          invoke<CommandResponse<ConflictRegion[]>>('git_parse_file_conflicts', {
            repoPath,
            filePath,
          }),
        ]);

        if (cancelled) return;

        if (contentRes.success && contentRes.data !== null) {
          setFileContent(contentRes.data);
        } else {
          setError(contentRes.error || 'Failed to read file');
        }

        if (conflictsRes.success && conflictsRes.data) {
          setRegions(conflictsRes.data);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    };

    load();
    return () => {
      cancelled = true;
    };
  }, [repoPath, filePath]);

  // Build ours/theirs content for display
  const oursContent = useMemo(() => {
    return regions.map((r) => r.ours).join('\n---\n');
  }, [regions]);

  const theirsContent = useMemo(() => {
    return regions.map((r) => r.theirs).join('\n---\n');
  }, [regions]);

  // Build current output content
  const outputContent = useMemo(() => {
    if (regions.length === 0) return fileContent;
    return buildResolvedContent(fileContent, regions, resolutions);
  }, [fileContent, regions, resolutions]);

  const [editableOutput, setEditableOutput] = useState('');

  useEffect(() => {
    setEditableOutput(outputContent);
  }, [outputContent]);

  const allRegionsResolved = regions.length > 0 && resolutions.size === regions.length;

  // Resolution actions
  const handleAcceptOurs = useCallback(
    (idx: number) => {
      setResolutions((prev) => {
        const next = new Map(prev);
        next.set(idx, { regionIndex: idx, content: regions[idx].ours, method: 'ours' });
        return next;
      });
    },
    [regions]
  );

  const handleAcceptTheirs = useCallback(
    (idx: number) => {
      setResolutions((prev) => {
        const next = new Map(prev);
        next.set(idx, { regionIndex: idx, content: regions[idx].theirs, method: 'theirs' });
        return next;
      });
    },
    [regions]
  );

  const handleAcceptBoth = useCallback(
    (idx: number) => {
      const combined =
        regions[idx].ours && regions[idx].theirs
          ? `${regions[idx].ours}\n${regions[idx].theirs}`
          : regions[idx].ours || regions[idx].theirs;
      setResolutions((prev) => {
        const next = new Map(prev);
        next.set(idx, { regionIndex: idx, content: combined, method: 'both' });
        return next;
      });
    },
    [regions]
  );

  const handleManualEdit = useCallback(
    (idx: number) => {
      setEditingRegion(idx);
      const current = resolutions.get(idx);
      if (!current) {
        // Initialize with ours content for editing
        setResolutions((prev) => {
          const next = new Map(prev);
          next.set(idx, { regionIndex: idx, content: regions[idx].ours, method: 'manual' });
          return next;
        });
      }
    },
    [regions, resolutions]
  );

  // AI Resolve for a single region
  const handleAIResolve = useCallback(
    async (idx: number) => {
      if (!isAvailable) return;
      const result = await resolveConflictAI(repoPath, filePath);
      if (result) {
        // The AI resolves the entire file at once. We apply it to the first
        // unresolved region or the requested region.
        setResolutions((prev) => {
          const next = new Map(prev);
          next.set(idx, { regionIndex: idx, content: result, method: 'ai' });
          return next;
        });
        showToast(t('threeWayDiff.aiResolvedRegion', { index: idx + 1 }), 'success');
      } else {
        showToast(t('threeWayDiff.aiResolveFailed'), 'error');
      }
    },
    [isAvailable, resolveConflictAI, repoPath, filePath, showToast]
  );

  // AI Resolve All
  const handleAIResolveAll = useCallback(async () => {
    if (!isAvailable) return;
    setIsAIResolvingAll(true);
    const result = await resolveConflictAI(repoPath, filePath);
    if (result) {
      // Apply AI resolution to all unresolved regions
      setResolutions((prev) => {
        const next = new Map(prev);
        for (let i = 0; i < regions.length; i++) {
          if (!next.has(i)) {
            next.set(i, { regionIndex: i, content: result, method: 'ai' });
          }
        }
        return next;
      });
      showToast(t('threeWayDiff.aiResolvedAll'), 'success');
    } else {
      showToast(t('threeWayDiff.aiResolveAllFailed'), 'error');
    }
    setIsAIResolvingAll(false);
  }, [isAvailable, resolveConflictAI, repoPath, filePath, regions.length, showToast]);

  const handleOutputChange = useCallback(
    (content: string) => {
      setEditableOutput(content);
      if (editingRegion !== null) {
        setResolutions((prev) => {
          const next = new Map(prev);
          next.set(editingRegion, { regionIndex: editingRegion, content, method: 'manual' });
          return next;
        });
      }
    },
    [editingRegion]
  );

  const handleMarkResolved = useCallback(async () => {
    if (!allRegionsResolved) return;
    setIsResolving(true);
    setError(null);

    try {
      // Build the final resolved content
      const resolvedContent = buildResolvedContent(fileContent, regions, resolutions);

      const res = await invoke<CommandResponse<void>>('git_resolve_file_and_stage', {
        repoPath,
        filePath,
        content: resolvedContent,
      });

      if (res.success) {
        onResolved();
      } else {
        setError(res.error || t('threeWayDiff.resolveFailed'));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsResolving(false);
    }
  }, [allRegionsResolved, fileContent, regions, resolutions, repoPath, filePath, onResolved]);

  // Loading state
  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400">
          <div className="animate-spin h-4 w-4 border-2 border-gray-400 border-t-transparent rounded-full" />
          {t('threeWayDiff.loadingFile')}
        </div>
      </div>
    );
  }

  // No conflicts
  if (regions.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-sm text-gray-500 dark:text-gray-400">
        {t('threeWayDiff.noConflictMarkers')}
      </div>
    );
  }

  const unresolvedCount = regions.length - resolutions.size;

  return (
    <div className="flex flex-col h-full">
      {/* Region action bars */}
      <div className="shrink-0 px-3 py-2 border-b border-gray-200 dark:border-gray-700 overflow-x-auto">
        <div className="flex items-center justify-between mb-2">
          <span className="text-xs text-gray-600 dark:text-gray-400">
            {t('threeWayDiff.conflictRegion', { count: regions.length })}
            {unresolvedCount > 0 && ` (${t('threeWayDiff.unresolved', { count: unresolvedCount })})`}
          </span>
          {/* AI Resolve All button */}
          {isAvailable && unresolvedCount > 0 && (
            <button
              onClick={handleAIResolveAll}
              disabled={isAIResolvingAll}
              className={clsx(
                'flex items-center gap-1.5 px-3 py-1 text-xs rounded-md transition-colors',
                isAIResolvingAll
                  ? 'bg-purple-100 dark:bg-purple-900/30 text-purple-500 dark:text-purple-400 cursor-wait'
                  : 'bg-purple-50 dark:bg-purple-900/20 text-purple-600 dark:text-purple-400 hover:bg-purple-100 dark:hover:bg-purple-900/30 border border-purple-200 dark:border-purple-700'
              )}
            >
              {isAIResolvingAll ? (
                <>
                  <svg className="animate-spin w-3.5 h-3.5" fill="none" viewBox="0 0 24 24">
                    <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                    <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                  </svg>
                  {t('threeWayDiff.resolvingAll')}
                </>
              ) : (
                <>
                  <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09z"
                    />
                  </svg>
                  {t('threeWayDiff.aiResolveAll')}
                </>
              )}
            </button>
          )}
        </div>
        <div className="flex flex-wrap gap-2">
          {regions.map((region, idx) => (
            <RegionActions
              key={idx}
              regionIndex={idx}
              region={region}
              resolution={resolutions.get(idx)}
              isAIAvailable={isAvailable}
              isAIResolving={isFileAIResolving}
              onAcceptOurs={handleAcceptOurs}
              onAcceptTheirs={handleAcceptTheirs}
              onAcceptBoth={handleAcceptBoth}
              onManualEdit={handleManualEdit}
              onAIResolve={handleAIResolve}
            />
          ))}
        </div>
      </div>

      {/* Error bar */}
      {error && (
        <div className="shrink-0 px-3 py-2 bg-red-50 dark:bg-red-900/20 text-sm text-red-600 dark:text-red-400 border-b border-red-200 dark:border-red-800">
          {error}
        </div>
      )}

      {/* Three-column diff */}
      <div className="flex-1 min-h-0 flex gap-1 p-2">
        {/* Ours (left) */}
        <div className="flex-1 min-w-0">
          <CodePanel
            title={t('threeWayDiff.oursCurrentBranch')}
            titleColor="bg-blue-50 dark:bg-blue-900/20 text-blue-700 dark:text-blue-400"
            content={oursContent}
            regions={regions}
            side="ours"
          />
        </div>

        {/* Output (center) */}
        <div className="flex-1 min-w-0">
          <OutputPanel
            content={editingRegion !== null ? editableOutput : outputContent}
            onChange={handleOutputChange}
            isEditing={editingRegion !== null}
          />
        </div>

        {/* Theirs (right) */}
        <div className="flex-1 min-w-0">
          <CodePanel
            title={t('threeWayDiff.theirsIncomingBranch')}
            titleColor="bg-purple-50 dark:bg-purple-900/20 text-purple-700 dark:text-purple-400"
            content={theirsContent}
            regions={regions}
            side="theirs"
          />
        </div>
      </div>

      {/* Bottom action bar */}
      <div className="shrink-0 flex items-center justify-between px-3 py-2 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
        <div className="text-sm text-gray-600 dark:text-gray-400">
          <span className="font-mono">{filePath}</span>
          <span className="mx-2">-</span>
          <span>
            {t('threeWayDiff.regionsResolved', { resolved: resolutions.size, total: regions.length })}
          </span>
        </div>
        <button
          onClick={handleMarkResolved}
          disabled={!allRegionsResolved || isResolving}
          className={clsx(
            'px-4 py-1.5 text-sm rounded-lg font-medium text-white transition-colors',
            allRegionsResolved && !isResolving
              ? 'bg-green-600 hover:bg-green-700'
              : 'bg-green-400 cursor-not-allowed'
          )}
        >
          {isResolving ? t('threeWayDiff.resolving') : t('threeWayDiff.markResolved')}
        </button>
      </div>
    </div>
  );
}
