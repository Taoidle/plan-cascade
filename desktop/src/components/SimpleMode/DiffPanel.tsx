/**
 * DiffPanel Component
 *
 * Shows Git Changes and Tool Changes in SimpleMode's right panel.
 * Git Changes: Runs `git diff` via Tauri shell plugin and renders parsed output.
 * Tool Changes: Extracts Write/Edit tool operations from streamingOutput.
 *
 * Story-005: Integrate Diffs Panel into SimpleMode
 */

import { useState, useCallback, useMemo, useEffect } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { Command } from '@tauri-apps/plugin-shell';
import { parseUnifiedDiff, type FileDiff } from '../../lib/diffParser';
import { EnhancedDiffViewer } from '../ClaudeCodeMode/EnhancedDiffViewer';
import type { StreamLine } from '../../store/execution';

// ============================================================================
// Constants
// ============================================================================

/** Maximum number of file diffs to display for performance. */
const MAX_FILES_DISPLAYED = 50;

// ============================================================================
// Types
// ============================================================================

interface DiffPanelProps {
  /** Full streaming output from the current session. */
  streamingOutput: StreamLine[];
  /** Current workspace directory path, or null if none selected. */
  workspacePath: string | null;
}

interface ToolChange {
  /** Identifier for the tool change (based on line id). */
  id: number;
  /** The tool name (e.g., Write, Edit). */
  toolName: string;
  /** File path affected by the tool. */
  filePath: string;
  /** Preview of the operation content. */
  preview: string;
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Reconstruct old and new content from a FileDiff's hunks for rendering
 * with EnhancedDiffViewer, which expects oldContent/newContent strings.
 */
function fileDiffToContents(fileDiff: FileDiff): { oldContent: string; newContent: string } {
  const oldLines: string[] = [];
  const newLines: string[] = [];

  for (const hunk of fileDiff.hunks) {
    for (const line of hunk.lines) {
      if (line.type === 'context') {
        oldLines.push(line.content);
        newLines.push(line.content);
      } else if (line.type === 'removed') {
        oldLines.push(line.content);
      } else if (line.type === 'added') {
        newLines.push(line.content);
      }
    }
  }

  return {
    oldContent: oldLines.join('\n'),
    newContent: newLines.join('\n'),
  };
}

/**
 * Extract tool changes (Write/Edit operations) from streaming output lines.
 * Looks for tool-type lines whose content references Write or Edit operations.
 */
function extractToolChanges(lines: StreamLine[]): ToolChange[] {
  const changes: ToolChange[] = [];
  const seen = new Set<string>();

  for (const line of lines) {
    if (line.type !== 'tool' && line.type !== 'tool_result') continue;

    const content = line.content;

    // Match tool start lines like: [tool:Write] /path/to/file.ts ...
    // or [tool:Edit] /path/to/file.ts ...
    const toolStartMatch = content.match(/\[tool:(Write|Edit)\]\s+(.+)/i);
    if (toolStartMatch) {
      const toolName = toolStartMatch[1];
      const rest = toolStartMatch[2].trim();
      // Extract file path: first non-space segment that looks like a path
      const pathMatch = rest.match(/^(\S+)/);
      const filePath = pathMatch ? pathMatch[1] : rest;

      const key = `${toolName}:${filePath}`;
      if (!seen.has(key)) {
        seen.add(key);
        changes.push({
          id: line.id,
          toolName,
          filePath,
          preview: rest.length > 120 ? rest.slice(0, 120) + '...' : rest,
        });
      }
      continue;
    }

    // Match tool_result lines that reference Write/Edit
    const resultMatch = content.match(/\[tool_result:\S*\]\s*(.*)/i);
    if (resultMatch) {
      // Tool results don't contain enough info to extract new changes;
      // they confirm completions. Skip for dedup purposes.
      continue;
    }

    // Also check for simple [tool] Write/Edit started patterns
    const simpleMatch = content.match(/\[tool\]\s+(Write|Edit)\s+started/i);
    if (simpleMatch) {
      // These don't have file paths, skip.
      continue;
    }
  }

  return changes;
}

// ============================================================================
// Sub-components
// ============================================================================

function SectionHeader({ title, count, defaultOpen = true }: { title: string; count?: number; defaultOpen?: boolean }) {
  const [open, setOpen] = useState(defaultOpen);

  return {
    open,
    header: (
      <button
        onClick={() => setOpen((v) => !v)}
        className="w-full flex items-center justify-between px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
      >
        <div className="flex items-center gap-2">
          <svg
            className={clsx('w-3 h-3 transition-transform', open && 'rotate-90')}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
          </svg>
          <span>{title}</span>
        </div>
        {typeof count === 'number' && <span className="text-xs text-gray-500 dark:text-gray-400">{count}</span>}
      </button>
    ),
  };
}

function GitChangesSection({
  fileDiffs,
  isLoading,
  error,
}: {
  fileDiffs: FileDiff[];
  isLoading: boolean;
  error: string | null;
}) {
  const { t } = useTranslation('simpleMode');
  const displayDiffs = fileDiffs.slice(0, MAX_FILES_DISPLAYED);
  const section = SectionHeader({
    title: t('diffPanel.gitChanges', { defaultValue: 'Git Changes' }),
    count: fileDiffs.length,
  });

  return (
    <div className="border-b border-gray-200 dark:border-gray-700">
      {section.header}
      {section.open && (
        <div className="px-3 pb-3">
          {isLoading && (
            <div className="flex items-center gap-2 py-4 justify-center text-sm text-gray-500 dark:text-gray-400">
              <div className="animate-spin h-4 w-4 border-2 border-gray-400 border-t-transparent rounded-full" />
              <span>Loading...</span>
            </div>
          )}
          {error && <div className="py-3 text-center text-sm text-gray-500 dark:text-gray-400">{error}</div>}
          {!isLoading && !error && displayDiffs.length === 0 && (
            <div className="py-4 text-center text-sm text-gray-500 dark:text-gray-400">
              {t('diffPanel.noChanges', { defaultValue: 'No changes detected' })}
            </div>
          )}
          {!isLoading && !error && displayDiffs.length > 0 && (
            <div className="space-y-3">
              {displayDiffs.map((fileDiff, idx) => {
                const { oldContent, newContent } = fileDiffToContents(fileDiff);
                return (
                  <div key={`${fileDiff.filePath}-${idx}`}>
                    <EnhancedDiffViewer
                      oldContent={oldContent}
                      newContent={newContent}
                      filePath={fileDiff.filePath}
                      maxHeight={350}
                    />
                  </div>
                );
              })}
              {fileDiffs.length > MAX_FILES_DISPLAYED && (
                <div className="text-xs text-center text-gray-500 dark:text-gray-400 py-2">
                  {t('diffPanel.filesChanged', {
                    count: fileDiffs.length,
                    defaultValue: '{{count}} file(s) changed',
                  })}{' '}
                  (showing first {MAX_FILES_DISPLAYED})
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function ToolChangesSection({ changes }: { changes: ToolChange[] }) {
  const { t } = useTranslation('simpleMode');
  const section = SectionHeader({
    title: t('diffPanel.toolChanges', { defaultValue: 'Tool Changes' }),
    count: changes.length,
    defaultOpen: true,
  });

  return (
    <div>
      {section.header}
      {section.open && (
        <div className="px-3 pb-3">
          {changes.length === 0 && (
            <div className="py-4 text-center text-sm text-gray-500 dark:text-gray-400">
              {t('diffPanel.noChanges', { defaultValue: 'No changes detected' })}
            </div>
          )}
          {changes.length > 0 && (
            <div className="space-y-2">
              {changes.map((change) => (
                <div
                  key={change.id}
                  className="px-3 py-2 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50"
                >
                  <div className="flex items-center gap-2 mb-1">
                    <span
                      className={clsx(
                        'text-2xs font-medium px-1.5 py-0.5 rounded',
                        change.toolName === 'Write'
                          ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400'
                          : 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-400',
                      )}
                    >
                      {change.toolName}
                    </span>
                    <code className="text-xs text-gray-600 dark:text-gray-400 truncate flex-1">{change.filePath}</code>
                  </div>
                  <p className="text-2xs text-gray-500 dark:text-gray-400 truncate">{change.preview}</p>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// DiffPanel Component
// ============================================================================

export function DiffPanel({ streamingOutput, workspacePath }: DiffPanelProps) {
  const { t } = useTranslation('simpleMode');

  // Git diff state
  const [gitDiffs, setGitDiffs] = useState<FileDiff[]>([]);
  const [gitLoading, setGitLoading] = useState(false);
  const [gitError, setGitError] = useState<string | null>(null);

  // Fetch git diff
  const fetchGitDiff = useCallback(async () => {
    if (!workspacePath) {
      setGitError(t('diffPanel.notGitRepo', { defaultValue: 'Not a git repository' }));
      setGitDiffs([]);
      return;
    }

    setGitLoading(true);
    setGitError(null);

    try {
      const command = Command.create('git', ['diff'], { cwd: workspacePath });
      const output = await command.execute();

      if (output.code !== 0) {
        // Check if the error indicates this is not a git repository
        const stderr = output.stderr || '';
        if (stderr.includes('not a git repository') || stderr.includes('Not a git repository')) {
          setGitError(t('diffPanel.notGitRepo', { defaultValue: 'Not a git repository' }));
        } else {
          setGitError(stderr || 'git diff failed');
        }
        setGitDiffs([]);
        return;
      }

      const parsed = parseUnifiedDiff(output.stdout || '');
      setGitDiffs(parsed);
      setGitError(null);
    } catch (err) {
      // Handle non-git directories or shell errors gracefully
      const message = err instanceof Error ? err.message : String(err);
      if (message.includes('not a git repository') || message.includes('Not a git repository')) {
        setGitError(t('diffPanel.notGitRepo', { defaultValue: 'Not a git repository' }));
      } else {
        setGitError(t('diffPanel.notGitRepo', { defaultValue: 'Not a git repository' }));
      }
      setGitDiffs([]);
    } finally {
      setGitLoading(false);
    }
  }, [workspacePath, t]);

  // Auto-fetch on mount and when workspacePath changes
  useEffect(() => {
    fetchGitDiff();
  }, [fetchGitDiff]);

  // Extract tool changes from streaming output
  const toolChanges = useMemo(() => extractToolChanges(streamingOutput), [streamingOutput]);

  return (
    <div className="min-h-0 flex flex-col rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-hidden">
      {/* Header */}
      <div className="shrink-0 flex items-center justify-between px-3 py-2 border-b border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800/50">
        <h3 className="text-sm font-medium text-gray-800 dark:text-gray-200">
          {t('diffPanel.title', { defaultValue: 'Changes' })}
        </h3>
        <button
          onClick={fetchGitDiff}
          disabled={gitLoading}
          className={clsx(
            'text-xs px-2 py-1 rounded transition-colors',
            'text-gray-600 dark:text-gray-400',
            'hover:bg-gray-100 dark:hover:bg-gray-700',
            gitLoading && 'opacity-50 cursor-not-allowed',
          )}
          title={t('diffPanel.refresh', { defaultValue: 'Refresh' })}
        >
          <svg
            className={clsx('w-3.5 h-3.5', gitLoading && 'animate-spin')}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
            />
          </svg>
        </button>
      </div>

      {/* Scrollable content */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        <GitChangesSection fileDiffs={gitDiffs} isLoading={gitLoading} error={gitError} />
        <ToolChangesSection changes={toolChanges} />
      </div>
    </div>
  );
}

export default DiffPanel;

// Export helpers for testing
export { extractToolChanges, fileDiffToContents };
