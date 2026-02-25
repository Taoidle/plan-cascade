/**
 * KnowledgeQuery Component
 *
 * Search input with results display showing matched chunks
 * with relevance scores and source document links.
 */

import { useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useKnowledgeStore } from '../../store/knowledge';

interface KnowledgeQueryProps {
  projectId: string;
  collectionName: string;
}

export function KnowledgeQuery({ projectId, collectionName }: KnowledgeQueryProps) {
  const { t } = useTranslation('knowledge');
  const { queryResults, totalSearched, searchQuery, isQuerying, queryCollection, setSearchQuery, clearQueryResults } =
    useKnowledgeStore();

  const [topK, setTopK] = useState(10);

  const handleSearch = useCallback(async () => {
    if (!searchQuery.trim()) return;
    await queryCollection(projectId, collectionName, searchQuery.trim(), topK);
  }, [projectId, collectionName, searchQuery, topK, queryCollection]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter') {
        handleSearch();
      }
    },
    [handleSearch],
  );

  const formatScore = (score: number): string => {
    return (score * 100).toFixed(1) + '%';
  };

  const getScoreColor = (score: number): string => {
    if (score >= 0.8) return 'text-green-600 dark:text-green-400 bg-green-50 dark:bg-green-900/20';
    if (score >= 0.5) return 'text-yellow-600 dark:text-yellow-400 bg-yellow-50 dark:bg-yellow-900/20';
    return 'text-gray-600 dark:text-gray-400 bg-gray-50 dark:bg-gray-900/20';
  };

  return (
    <div className="p-6 space-y-6">
      <div>
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white">{t('query.title')}</h3>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">{t('query.subtitle')}</p>
      </div>

      {/* Search input */}
      <div className="space-y-3">
        <div className="flex gap-2">
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('query.placeholder')}
            className={clsx(
              'flex-1 px-4 py-2.5 rounded-lg',
              'border border-gray-300 dark:border-gray-600',
              'bg-white dark:bg-gray-800',
              'text-gray-900 dark:text-white',
              'focus:ring-2 focus:ring-primary-500 focus:border-transparent',
              'text-sm placeholder:text-gray-400',
            )}
          />
          <button
            onClick={handleSearch}
            disabled={!searchQuery.trim() || isQuerying}
            className={clsx(
              'px-5 py-2.5 rounded-lg text-sm font-medium',
              'bg-primary-600 hover:bg-primary-700',
              'text-white',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors shrink-0',
            )}
          >
            {isQuerying ? t('query.searching') : t('query.search')}
          </button>
        </div>

        {/* Options */}
        <div className="flex items-center gap-4">
          <label className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400">
            <span>{t('query.topK')}</span>
            <select
              value={topK}
              onChange={(e) => setTopK(Number(e.target.value))}
              className={clsx(
                'px-2 py-1 rounded-md text-sm',
                'border border-gray-300 dark:border-gray-600',
                'bg-white dark:bg-gray-800',
                'text-gray-900 dark:text-white',
              )}
            >
              {[5, 10, 20, 50].map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </label>
          {queryResults.length > 0 && (
            <button
              onClick={clearQueryResults}
              className="text-sm text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
            >
              {t('query.clearResults')}
            </button>
          )}
        </div>
      </div>

      {/* Results summary */}
      {queryResults.length > 0 && (
        <div className="text-sm text-gray-600 dark:text-gray-400">
          {t('query.resultsFound', { count: queryResults.length, total: totalSearched })}
        </div>
      )}

      {/* Results list */}
      {isQuerying ? (
        <div className="text-center py-8">
          <div className="animate-pulse text-sm text-gray-500">{t('query.searching')}</div>
        </div>
      ) : queryResults.length > 0 ? (
        <div className="space-y-3">
          {queryResults.map((result, index) => (
            <div
              key={index}
              className={clsx(
                'rounded-lg border border-gray-200 dark:border-gray-700',
                'bg-white dark:bg-gray-900',
                'overflow-hidden',
              )}
            >
              {/* Result header */}
              <div className="flex items-center justify-between px-4 py-2 border-b border-gray-100 dark:border-gray-800">
                <div className="flex items-center gap-3">
                  <span className="text-xs font-mono text-gray-400">#{index + 1}</span>
                  <span className="text-sm font-medium text-gray-700 dark:text-gray-300">{result.document_id}</span>
                  <span className="text-xs text-gray-400 dark:text-gray-500">{result.collection_name}</span>
                </div>
                <span className={clsx('text-xs font-medium px-2 py-0.5 rounded-full', getScoreColor(result.score))}>
                  {formatScore(result.score)}
                </span>
              </div>

              {/* Result content */}
              <div className="px-4 py-3">
                <p className="text-sm text-gray-800 dark:text-gray-200 whitespace-pre-wrap leading-relaxed">
                  {result.chunk_text}
                </p>
              </div>
            </div>
          ))}
        </div>
      ) : searchQuery && !isQuerying ? (
        <div className="text-center py-8">
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('query.noResults')}</p>
        </div>
      ) : null}
    </div>
  );
}
