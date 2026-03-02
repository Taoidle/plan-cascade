/**
 * CodebaseSearch Component
 *
 * Semantic search input with results list showing file paths,
 * similarity scores, and code chunk previews.
 */

import { useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useCodebaseStore } from '../../store/codebase';
import { openCodebaseFileInEditor } from '../../lib/codebaseApi';

interface CodebaseSearchProps {
  projectPath: string;
}

export function CodebaseSearch({ projectPath }: CodebaseSearchProps) {
  const { t } = useTranslation('codebase');
  const {
    searchResults,
    searchLoading,
    searchProject,
    clearSearch,
    addContextItem,
    contextItems,
    pushContextToMode,
    contextPushLoading,
    lastContextPush,
    setError,
  } = useCodebaseStore();
  const [query, setQuery] = useState('');
  const [mode, setMode] = useState<'hybrid' | 'semantic' | 'symbol' | 'path'>('hybrid');
  const [targetMode, setTargetMode] = useState<'chat' | 'plan' | 'task'>('chat');

  const handleSearch = useCallback(() => {
    if (query.trim()) {
      searchProject(projectPath, query.trim(), 20, [mode]);
    }
  }, [mode, projectPath, query, searchProject]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter') {
        handleSearch();
      }
    },
    [handleSearch],
  );

  const handleClear = useCallback(() => {
    setQuery('');
    clearSearch();
  }, [clearSearch]);

  const formatSimilarity = (sim: number): string => {
    return `${(sim * 100).toFixed(1)}%`;
  };

  const getSimilarityColor = (sim: number): string => {
    if (sim >= 0.8) return 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300';
    if (sim >= 0.5) return 'bg-yellow-100 dark:bg-yellow-900/30 text-yellow-700 dark:text-yellow-300';
    return 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400';
  };

  return (
    <div className="flex flex-col h-full">
      {/* Search input */}
      <div className="px-4 py-3 border-b border-gray-200 dark:border-gray-700">
        <div className="flex gap-2 items-center">
          <select
            value={mode}
            onChange={(e) => setMode(e.target.value as 'hybrid' | 'semantic' | 'symbol' | 'path')}
            className={clsx(
              'px-3 py-2 rounded-lg text-sm',
              'border border-gray-300 dark:border-gray-600',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
            )}
          >
            <option value="hybrid">{t('searchMode.hybrid')}</option>
            <option value="semantic">{t('searchMode.semantic')}</option>
            <option value="symbol">{t('searchMode.symbol')}</option>
            <option value="path">{t('searchMode.path')}</option>
          </select>
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('searchPlaceholder')}
            className={clsx(
              'flex-1 min-w-0 px-3 py-2 rounded-lg text-sm',
              'border border-gray-300 dark:border-gray-600',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'placeholder-gray-400 dark:placeholder-gray-500',
              'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
            )}
          />
          <button
            onClick={handleSearch}
            disabled={!query.trim() || searchLoading}
            className={clsx(
              'px-4 py-2 rounded-lg text-sm font-medium',
              'bg-primary-600 hover:bg-primary-700',
              'text-white',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors',
            )}
          >
            {searchLoading ? t('searching') : t('search')}
          </button>
          <select
            value={targetMode}
            onChange={(e) => setTargetMode(e.target.value as 'chat' | 'plan' | 'task')}
            className={clsx(
              'px-2.5 py-2 rounded-lg text-sm',
              'border border-gray-300 dark:border-gray-600',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
            )}
            title={t('targetModeLabel')}
          >
            <option value="chat">{t('targetMode.chat')}</option>
            <option value="plan">{t('targetMode.plan')}</option>
            <option value="task">{t('targetMode.task')}</option>
          </select>
          <button
            onClick={() => pushContextToMode(targetMode)}
            disabled={contextItems.length === 0 || contextPushLoading}
            className={clsx(
              'px-3 py-2 rounded-lg text-sm',
              'bg-gray-100 hover:bg-gray-200 dark:bg-gray-800 dark:hover:bg-gray-700',
              'text-gray-700 dark:text-gray-300',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
            title={t('pushContextTitle', { mode: t(`targetMode.${targetMode}`) })}
          >
            {contextPushLoading
              ? t('pushingContext')
              : t('pushContext', { mode: t(`targetMode.${targetMode}`), count: contextItems.length })}
          </button>
          {searchResults.length > 0 && (
            <button
              onClick={handleClear}
              className={clsx(
                'px-3 py-2 rounded-lg text-sm',
                'text-gray-600 dark:text-gray-400',
                'hover:bg-gray-100 dark:hover:bg-gray-800',
                'transition-colors',
              )}
            >
              {t('clear', { ns: 'common' })}
            </button>
          )}
        </div>
        {searchResults.length > 0 && (
          <>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-2">
              {t('searchResults', { count: searchResults.length })}
            </p>
            {lastContextPush && (
              <p className="text-xs text-emerald-600 dark:text-emerald-400 mt-1">
                {t('contextPushSuccess', {
                  count: lastContextPush.appendedCount,
                  mode: t(`targetMode.${lastContextPush.targetMode}`),
                })}
              </p>
            )}
          </>
        )}
      </div>

      {/* Results */}
      <div className="flex-1 overflow-y-auto">
        {searchLoading ? (
          <div className="p-8 text-center">
            <div className="animate-pulse text-sm text-gray-500">{t('searching')}</div>
          </div>
        ) : searchResults.length === 0 && query.trim() ? (
          <div className="p-8 text-center text-sm text-gray-500 dark:text-gray-400">{t('noResults')}</div>
        ) : searchResults.length === 0 ? (
          <div className="p-8 text-center text-sm text-gray-500 dark:text-gray-400">{t('searchPlaceholder')}</div>
        ) : (
          <div className="divide-y divide-gray-200 dark:divide-gray-800">
            {searchResults.map((result, idx) => (
              <div
                key={`${result.file_path}-${idx}`}
                className="px-4 py-3 hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
              >
                <div className="flex items-center justify-between mb-1.5">
                  <span
                    className="text-sm font-medium text-gray-900 dark:text-white font-mono truncate flex-1 mr-3"
                    title={result.file_path}
                  >
                    {result.file_path}
                  </span>
                  <span
                    className={clsx(
                      'px-2 py-0.5 rounded text-xs font-medium shrink-0',
                      getSimilarityColor(result.similarity ?? result.score),
                    )}
                  >
                    {t('similarity')} {formatSimilarity(result.similarity ?? result.score)}
                  </span>
                </div>
                {result.snippet ? (
                  <pre
                    className={clsx(
                      'text-xs font-mono p-2 rounded',
                      'bg-gray-100 dark:bg-gray-800',
                      'text-gray-700 dark:text-gray-300',
                      'overflow-x-auto max-h-32',
                      'whitespace-pre-wrap break-words',
                    )}
                  >
                    {result.snippet.length > 500 ? result.snippet.slice(0, 500) + '...' : result.snippet}
                  </pre>
                ) : (
                  <div className="text-xs text-gray-500 dark:text-gray-400">{t('noSnippet')}</div>
                )}
                {result.score_breakdown.length > 0 && (
                  <div className="mt-1.5 flex items-center gap-1.5 flex-wrap">
                    {result.score_breakdown.map((channel) => (
                      <span
                        key={`${result.file_path}-${idx}-${channel.channel}`}
                        className="px-1.5 py-0.5 rounded text-[10px] bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-300"
                      >
                        {channel.channel}
                      </span>
                    ))}
                  </div>
                )}
                <div className="mt-2 flex items-center gap-2">
                  <button
                    onClick={async () => {
                      const opened = await openCodebaseFileInEditor(
                        projectPath,
                        result.file_path,
                        result.line_start ?? 1,
                        1,
                      );
                      if (!opened.success) {
                        setError(opened.error ?? t('openFailed'));
                      }
                    }}
                    className="px-2 py-1 rounded text-xs bg-gray-200 text-gray-700 hover:bg-gray-300 dark:bg-gray-700 dark:text-gray-200 dark:hover:bg-gray-600"
                  >
                    {t('open')}
                  </button>
                  <button
                    onClick={() =>
                      addContextItem({
                        type: 'search_result',
                        project_path: projectPath,
                        file_path: result.file_path,
                        symbol_name: result.symbol_name ?? null,
                        snippet: result.snippet ?? null,
                        line_start: result.line_start ?? null,
                        line_end: result.line_end ?? null,
                        score: result.score,
                        source: 'codebase',
                        metadata: {
                          score_breakdown: result.score_breakdown,
                          query_id: result.query_id ?? null,
                          channels: result.channels ?? null,
                        },
                      })
                    }
                    className="px-2 py-1 rounded text-xs bg-blue-100 text-blue-700 hover:bg-blue-200 dark:bg-blue-900/40 dark:text-blue-300 dark:hover:bg-blue-900/60"
                  >
                    {t('addContext')}
                  </button>
                  <button
                    onClick={async () => {
                      const ref = `${result.file_path}${result.symbol_name ? `#${result.symbol_name}` : ''}`;
                      try {
                        await navigator.clipboard.writeText(ref);
                      } catch {
                        setError(t('copyRefFailed'));
                      }
                    }}
                    className="px-2 py-1 rounded text-xs bg-gray-100 text-gray-700 hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
                  >
                    {t('copyRef')}
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
