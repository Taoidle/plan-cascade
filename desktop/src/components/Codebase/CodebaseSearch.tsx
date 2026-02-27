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

interface CodebaseSearchProps {
  projectPath: string;
}

export function CodebaseSearch({ projectPath }: CodebaseSearchProps) {
  const { t } = useTranslation('codebase');
  const { searchResults, searchLoading, searchProject, clearSearch } = useCodebaseStore();
  const [query, setQuery] = useState('');

  const handleSearch = useCallback(() => {
    if (query.trim()) {
      searchProject(projectPath, query.trim());
    }
  }, [projectPath, query, searchProject]);

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
        <div className="flex gap-2">
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
            {searchLoading ? '...' : t('search')}
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
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-2">
            {t('searchResults', { count: searchResults.length })}
          </p>
        )}
      </div>

      {/* Results */}
      <div className="flex-1 overflow-y-auto">
        {searchLoading ? (
          <div className="p-8 text-center">
            <div className="animate-pulse text-sm text-gray-500">Searching...</div>
          </div>
        ) : searchResults.length === 0 && query.trim() ? (
          <div className="p-8 text-center text-sm text-gray-500 dark:text-gray-400">{t('noResults')}</div>
        ) : searchResults.length === 0 ? (
          <div className="p-8 text-center text-sm text-gray-500 dark:text-gray-400">{t('searchPlaceholder')}</div>
        ) : (
          <div className="divide-y divide-gray-200 dark:divide-gray-800">
            {searchResults.map((result, idx) => (
              <div
                key={`${result.file_path}-${result.chunk_index}-${idx}`}
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
                      getSimilarityColor(result.similarity),
                    )}
                  >
                    {t('similarity')} {formatSimilarity(result.similarity)}
                  </span>
                </div>
                <pre
                  className={clsx(
                    'text-xs font-mono p-2 rounded',
                    'bg-gray-100 dark:bg-gray-800',
                    'text-gray-700 dark:text-gray-300',
                    'overflow-x-auto max-h-32',
                    'whitespace-pre-wrap break-words',
                  )}
                >
                  {result.chunk_text.length > 500 ? result.chunk_text.slice(0, 500) + '...' : result.chunk_text}
                </pre>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
