/**
 * ExplorationCard
 *
 * Displays the project exploration results including tech stack,
 * components, key files, patterns, and optional LLM summary.
 * Uses violet color scheme to distinguish from other workflow cards.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { ChevronRightIcon } from '@radix-ui/react-icons';
import type { ExplorationCardData } from '../../../types/workflowCard';
import { Collapsible } from '../Collapsible';

export function ExplorationCard({ data }: { data: ExplorationCardData }) {
  const { t } = useTranslation('simpleMode');
  const [expanded, setExpanded] = useState(false);

  const hasContent = data.techStack.languages.length > 0 || data.components.length > 0 || data.keyFiles.length > 0;

  if (!hasContent && !data.llmSummary) return null;

  const summaryBadgeClass =
    data.summaryQuality === 'complete'
      ? 'bg-green-200 dark:bg-green-800 text-green-700 dark:text-green-300'
      : data.summaryQuality === 'partial'
        ? 'bg-amber-200 dark:bg-amber-800 text-amber-700 dark:text-amber-300'
        : 'bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300';
  const summaryBadgeLabel =
    data.summaryQuality === 'complete'
      ? t('workflow.exploration.summaryComplete', { defaultValue: 'Complete' })
      : data.summaryQuality === 'partial'
        ? t('workflow.exploration.summaryPartial', { defaultValue: 'Partial' })
        : t('workflow.exploration.summaryEmpty', { defaultValue: 'No AI summary' });

  return (
    <div className="rounded-lg border border-violet-200 dark:border-violet-800 bg-violet-50 dark:bg-violet-900/20 overflow-hidden">
      {/* Header */}
      <div className="px-3 py-2 bg-violet-100/50 dark:bg-violet-900/30 border-b border-violet-200 dark:border-violet-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="text-xs font-semibold text-violet-700 dark:text-violet-300 uppercase tracking-wide">
              {t('workflow.exploration.title')}
            </span>
            <span
              className={clsx(
                'text-2xs px-1.5 py-0.5 rounded',
                data.usedLlmExploration
                  ? 'bg-violet-200 dark:bg-violet-800 text-violet-600 dark:text-violet-400'
                  : 'bg-gray-200 dark:bg-gray-700 text-gray-600 dark:text-gray-400',
              )}
            >
              {data.usedLlmExploration ? t('workflow.exploration.aiAssisted') : t('workflow.exploration.deterministic')}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <span className="text-2xs text-violet-500 dark:text-violet-400">
              {t('workflow.exploration.duration', { ms: data.durationMs })}
            </span>
            <button
              onClick={() => setExpanded((v) => !v)}
              className="text-2xs text-violet-600 dark:text-violet-400 hover:text-violet-800 dark:hover:text-violet-200 transition-colors"
            >
              <ChevronRightIcon
                className={clsx('w-3.5 h-3.5 transition-transform duration-200', expanded && 'rotate-90')}
              />
            </button>
          </div>
        </div>
      </div>

      <div className="px-3 py-2 space-y-2">
        {/* Tech Stack pills */}
        {(data.techStack.languages.length > 0 || data.techStack.frameworks.length > 0) && (
          <div>
            <span className="text-2xs font-medium text-violet-600 dark:text-violet-400">
              {t('workflow.exploration.techStack')}
            </span>
            <div className="flex flex-wrap gap-1 mt-0.5">
              {data.techStack.languages.map((lang) => (
                <span
                  key={`lang-${lang}`}
                  className="text-2xs px-1.5 py-0.5 rounded bg-violet-100 dark:bg-violet-900/40 text-violet-600 dark:text-violet-400"
                >
                  {lang}
                </span>
              ))}
              {data.techStack.frameworks.map((fw) => (
                <span
                  key={`fw-${fw}`}
                  className="text-2xs px-1.5 py-0.5 rounded bg-violet-200 dark:bg-violet-800/50 text-violet-700 dark:text-violet-300"
                >
                  {fw}
                </span>
              ))}
              {data.techStack.buildTools.map((tool) => (
                <span
                  key={`bt-${tool}`}
                  className="text-2xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400"
                >
                  {tool}
                </span>
              ))}
            </div>
          </div>
        )}

        {/* Components summary */}
        {data.components.length > 0 && (
          <div>
            <span className="text-2xs font-medium text-violet-600 dark:text-violet-400">
              {t('workflow.exploration.components')}
            </span>
            <div className="mt-0.5 grid grid-cols-2 gap-1">
              {data.components.slice(0, expanded ? undefined : 4).map((comp) => (
                <div
                  key={comp.name}
                  className="text-2xs px-1.5 py-0.5 rounded bg-violet-50 dark:bg-violet-900/20 border border-violet-100 dark:border-violet-800"
                >
                  <span className="text-violet-700 dark:text-violet-300 font-medium">{comp.name}</span>
                  <span className="text-violet-500 dark:text-violet-400 ml-1">
                    {t('workflow.exploration.fileCount', { count: comp.fileCount })}
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Expanded details */}
        <Collapsible open={expanded}>
          <div className="space-y-2 pt-1 border-t border-violet-200 dark:border-violet-800">
            {/* Key Files */}
            {data.keyFiles.length > 0 && (
              <div>
                <span className="text-2xs font-medium text-violet-600 dark:text-violet-400">
                  {t('workflow.exploration.keyFiles')}
                </span>
                <div className="mt-0.5 space-y-0.5">
                  {data.keyFiles.map((file) => (
                    <div key={file.path} className="flex items-center gap-1 text-2xs">
                      <span className="text-violet-700 dark:text-violet-300 font-mono truncate">{file.path}</span>
                      <span className="text-violet-400 dark:text-violet-500 shrink-0">[{file.fileType}]</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Patterns */}
            {data.patterns.length > 0 && (
              <div>
                <span className="text-2xs font-medium text-violet-600 dark:text-violet-400">
                  {t('workflow.exploration.patterns')}
                </span>
                <ul className="mt-0.5 space-y-0.5">
                  {data.patterns.map((pattern, i) => (
                    <li key={i} className="text-2xs text-violet-600 dark:text-violet-400">
                      • {pattern}
                    </li>
                  ))}
                </ul>
              </div>
            )}

            {/* LLM Summary */}
            {(data.llmSummary || data.summaryQuality !== 'complete') && (
              <div>
                <div className="flex items-center gap-1.5">
                  <span className="text-2xs font-medium text-violet-600 dark:text-violet-400">
                    {t('workflow.exploration.llmSummary')}
                  </span>
                  <span className={clsx('text-2xs px-1.5 py-0.5 rounded', summaryBadgeClass)}>{summaryBadgeLabel}</span>
                </div>
                {data.llmSummary ? (
                  <div className="mt-0.5 text-2xs text-violet-700/80 dark:text-violet-300/80 whitespace-pre-wrap">
                    {data.llmSummary}
                  </div>
                ) : (
                  <div className="mt-0.5 text-2xs text-violet-600/80 dark:text-violet-300/80">
                    {t('workflow.exploration.summaryUnavailable', {
                      defaultValue: 'AI summary unavailable, using deterministic exploration results.',
                    })}
                  </div>
                )}
                {data.summaryNotes && (
                  <div className="mt-0.5 text-2xs text-violet-500/80 dark:text-violet-400/80">{data.summaryNotes}</div>
                )}
                {!data.llmSummary && data.summarySource === 'fallback_synthesized' && (
                  <div className="mt-0.5 text-2xs text-violet-500/80 dark:text-violet-400/80">
                    {t('workflow.exploration.summarySynthesized', {
                      defaultValue: 'Summary synthesized from deterministic exploration data.',
                    })}
                  </div>
                )}
              </div>
            )}
          </div>
        </Collapsible>
      </div>
    </div>
  );
}
