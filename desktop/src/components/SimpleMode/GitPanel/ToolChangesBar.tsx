/**
 * ToolChangesBar Component
 *
 * Bottom persistent bar showing tool changes (Write/Edit operations).
 * Refactored from the existing tool changes display in DiffPanel.
 * Includes correlation markers linking tool operations to git status entries.
 *
 * Feature-002, Story-006
 */

import { useState, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import type { StreamLine } from '../../../store/execution';
import { useGitStore } from '../../../store/git';

// ============================================================================
// Types
// ============================================================================

interface ToolChangesBarProps {
  streamingOutput: StreamLine[];
}

interface ToolChange {
  id: number;
  toolName: string;
  filePath: string;
  preview: string;
  /** Whether this tool change corresponds to a file in the git status. */
  hasGitChange: boolean;
}

// ============================================================================
// Helpers
// ============================================================================

/**
 * Extract tool changes (Write/Edit operations) from streaming output lines.
 * Matches the extraction logic from the original DiffPanel.
 */
function extractToolChanges(lines: StreamLine[], changedPaths: Set<string>): ToolChange[] {
  const changes: ToolChange[] = [];
  const seen = new Set<string>();

  for (const line of lines) {
    if (line.type !== 'tool' && line.type !== 'tool_result') continue;

    const content = line.content;

    // Match tool start lines like: [tool:Write] /path/to/file.ts ...
    const toolStartMatch = content.match(/\[tool:(Write|Edit)\]\s+(.+)/i);
    if (toolStartMatch) {
      const toolName = toolStartMatch[1];
      const rest = toolStartMatch[2].trim();
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
          hasGitChange: changedPaths.has(filePath),
        });
      }
    }
  }

  return changes;
}

// ============================================================================
// Component
// ============================================================================

export function ToolChangesBar({ streamingOutput }: ToolChangesBarProps) {
  const { t } = useTranslation('git');
  const [expanded, setExpanded] = useState(false);
  const status = useGitStore((s) => s.status);

  // Build a set of all changed file paths from git status
  const changedPaths = useMemo(() => {
    const paths = new Set<string>();
    if (status) {
      for (const f of status.staged) paths.add(f.path);
      for (const f of status.unstaged) paths.add(f.path);
      for (const f of status.untracked) paths.add(f.path);
    }
    return paths;
  }, [status]);

  const toolChanges = useMemo(() => extractToolChanges(streamingOutput, changedPaths), [streamingOutput, changedPaths]);

  if (toolChanges.length === 0) {
    return null;
  }

  return (
    <div className="shrink-0 border-t border-gray-200 dark:border-gray-700">
      {/* Header toggle */}
      <button
        onClick={() => setExpanded((v) => !v)}
        className="w-full flex items-center justify-between px-3 py-1.5 text-2xs font-medium text-gray-500 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
      >
        <span className="flex items-center gap-1.5">
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"
            />
          </svg>
          {t('toolChanges.title')}
          <span className="text-gray-400 dark:text-gray-500">({toolChanges.length})</span>
        </span>
        <svg
          className={clsx('w-3 h-3 transition-transform', expanded && 'rotate-180')}
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
        </svg>
      </button>

      {/* Expanded list */}
      {expanded && (
        <div className="max-h-40 overflow-y-auto px-3 pb-2 space-y-1.5">
          {toolChanges.map((change) => (
            <div
              key={change.id}
              className="flex items-start gap-2 px-2 py-1.5 rounded border border-gray-100 dark:border-gray-800 bg-gray-50 dark:bg-gray-800/50"
            >
              {/* Tool badge */}
              <span
                className={clsx(
                  'text-2xs font-medium px-1.5 py-0.5 rounded shrink-0 mt-0.5',
                  change.toolName === 'Write'
                    ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400'
                    : 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-400',
                )}
              >
                {change.toolName}
              </span>

              <div className="flex-1 min-w-0">
                <div className="flex items-center gap-1.5">
                  <code className="text-2xs text-gray-600 dark:text-gray-400 truncate">{change.filePath}</code>
                  {/* Git correlation marker */}
                  {change.hasGitChange && (
                    <span
                      className="shrink-0 w-1.5 h-1.5 rounded-full bg-green-500"
                      title={t('toolChanges.fileInGitStatus')}
                    />
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default ToolChangesBar;
