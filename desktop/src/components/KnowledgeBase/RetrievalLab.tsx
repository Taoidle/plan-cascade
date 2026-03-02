/**
 * RetrievalLab Component
 *
 * Production-oriented retrieval debugging surface:
 * - scoped query execution
 * - optional document filters
 * - query run observability timeline
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { ScopedDocumentRef } from '../../lib/knowledgeApi';
import { useKnowledgeStore } from '../../store/knowledge';

interface RetrievalLabProps {
  projectId: string;
  collectionId: string;
}

const DEFAULT_TOP_K = 10;

function formatDateTime(value: string): string {
  try {
    return new Date(value).toLocaleString();
  } catch {
    return value;
  }
}

function highlightQueryTerms(text: string, query: string): React.ReactNode {
  const q = query.trim();
  if (!q) return text;

  const escaped = q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const regex = new RegExp(`(${escaped})`, 'gi');
  const parts = text.split(regex);

  return parts.map((part, idx) =>
    part.toLowerCase() === q.toLowerCase() ? (
      <mark key={idx} className="bg-yellow-100 dark:bg-yellow-900/40 rounded px-0.5">
        {part}
      </mark>
    ) : (
      <span key={idx}>{part}</span>
    ),
  );
}

export function RetrievalLab({ projectId, collectionId }: RetrievalLabProps) {
  const { t } = useTranslation('knowledge');
  const {
    documents,
    queryResults,
    totalSearched,
    searchQuery,
    isQuerying,
    isLoadingQueryRuns,
    queryRuns,
    fetchDocuments,
    fetchQueryRuns,
    queryCollection,
    setSearchQuery,
    clearQueryResults,
  } = useKnowledgeStore();

  const [topK, setTopK] = useState<number>(DEFAULT_TOP_K);
  const [retrievalProfile, setRetrievalProfile] = useState<string>('balanced');
  const [useDocumentFilter, setUseDocumentFilter] = useState<boolean>(false);
  const [selectedDocumentUids, setSelectedDocumentUids] = useState<string[]>([]);

  useEffect(() => {
    fetchDocuments(collectionId);
    fetchQueryRuns(projectId, collectionId, 20);
  }, [collectionId, projectId, fetchDocuments, fetchQueryRuns]);

  const documentFilters: ScopedDocumentRef[] | undefined = useMemo(() => {
    if (!useDocumentFilter || selectedDocumentUids.length === 0) return undefined;
    return selectedDocumentUids.map((document_uid) => ({
      collection_id: collectionId,
      document_uid,
    }));
  }, [useDocumentFilter, selectedDocumentUids, collectionId]);

  const toggleDocument = useCallback((documentUid: string) => {
    setSelectedDocumentUids((prev) =>
      prev.includes(documentUid) ? prev.filter((id) => id !== documentUid) : [...prev, documentUid],
    );
  }, []);

  const handleQuery = useCallback(async () => {
    if (!searchQuery.trim()) return;
    await queryCollection(projectId, collectionId, searchQuery.trim(), topK, retrievalProfile, documentFilters);
  }, [searchQuery, queryCollection, projectId, collectionId, topK, retrievalProfile, documentFilters]);

  const handleRefreshRuns = useCallback(async () => {
    await fetchQueryRuns(projectId, collectionId, 20);
  }, [fetchQueryRuns, projectId, collectionId]);

  const runStats = useMemo(() => {
    if (queryRuns.length === 0) {
      return {
        avgTotalMs: 0,
        p50TotalMs: 0,
        p95TotalMs: 0,
        avgResults: 0,
      };
    }
    const totals = queryRuns.map((r) => r.total_ms).sort((a, b) => a - b);
    const sum = totals.reduce((acc, v) => acc + v, 0);
    const avgTotalMs = Math.round(sum / totals.length);
    const p50Idx = Math.floor((totals.length - 1) * 0.5);
    const p95Idx = Math.floor((totals.length - 1) * 0.95);
    const avgResults = Math.round(queryRuns.reduce((acc, run) => acc + run.result_count, 0) / queryRuns.length);
    return {
      avgTotalMs,
      p50TotalMs: totals[p50Idx] ?? 0,
      p95TotalMs: totals[p95Idx] ?? 0,
      avgResults,
    };
  }, [queryRuns]);

  return (
    <div className="p-6 space-y-6">
      <div>
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
          {t('retrievalLab.title', { defaultValue: 'Retrieval Lab' })}
        </h3>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
          {t('retrievalLab.subtitle', {
            defaultValue: 'Run scoped retrieval and inspect candidate quality/latency telemetry.',
          })}
        </p>
      </div>

      <div className="space-y-3">
        <div className="flex gap-2">
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                handleQuery();
              }
            }}
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
            onClick={handleQuery}
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

        <div className="flex flex-wrap items-center gap-3">
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
              {[5, 10, 20, 30, 50].map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </label>

          <label className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400">
            <span>{t('retrievalLab.profile', { defaultValue: 'Profile' })}</span>
            <select
              value={retrievalProfile}
              onChange={(e) => setRetrievalProfile(e.target.value)}
              className={clsx(
                'px-2 py-1 rounded-md text-sm',
                'border border-gray-300 dark:border-gray-600',
                'bg-white dark:bg-gray-800',
                'text-gray-900 dark:text-white',
              )}
            >
              <option value="balanced">{t('retrievalLab.profiles.balanced', { defaultValue: 'Balanced' })}</option>
              <option value="precision">{t('retrievalLab.profiles.precision', { defaultValue: 'Precision' })}</option>
              <option value="recall">{t('retrievalLab.profiles.recall', { defaultValue: 'Recall' })}</option>
            </select>
          </label>

          <button
            onClick={handleRefreshRuns}
            disabled={isLoadingQueryRuns}
            className={clsx(
              'text-sm px-3 py-1.5 rounded-md',
              'border border-gray-300 dark:border-gray-600',
              'text-gray-700 dark:text-gray-300',
              'hover:bg-gray-50 dark:hover:bg-gray-800',
              'disabled:opacity-50 disabled:cursor-not-allowed',
            )}
          >
            {isLoadingQueryRuns
              ? t('retrievalLab.loadingRuns', { defaultValue: 'Loading runs...' })
              : t('retrievalLab.refreshRuns', { defaultValue: 'Refresh Runs' })}
          </button>

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

      <div className="rounded-lg border border-gray-200 dark:border-gray-700 p-4 space-y-3">
        <label className="flex items-center gap-2 text-sm text-gray-700 dark:text-gray-300">
          <input
            type="checkbox"
            checked={useDocumentFilter}
            onChange={(e) => setUseDocumentFilter(e.target.checked)}
            className="rounded border-gray-300 dark:border-gray-600"
          />
          <span>{t('retrievalLab.enableDocFilter', { defaultValue: 'Enable document filter' })}</span>
        </label>

        {useDocumentFilter && (
          <div className="space-y-2">
            <div className="flex items-center gap-3 text-xs">
              <button
                onClick={() => setSelectedDocumentUids(documents.map((d) => d.document_uid))}
                className="text-primary-600 dark:text-primary-400 hover:underline"
              >
                {t('retrievalLab.selectAllDocs', { defaultValue: 'Select all' })}
              </button>
              <button
                onClick={() => setSelectedDocumentUids([])}
                className="text-gray-600 dark:text-gray-400 hover:underline"
              >
                {t('retrievalLab.clearDocSelection', { defaultValue: 'Clear selection' })}
              </button>
              <span className="text-gray-500 dark:text-gray-400">
                {t('retrievalLab.selectedDocs', {
                  defaultValue: '{{count}} selected',
                  count: selectedDocumentUids.length,
                })}
              </span>
            </div>
            <div className="max-h-40 overflow-y-auto divide-y divide-gray-100 dark:divide-gray-800 border border-gray-100 dark:border-gray-800 rounded">
              {documents.length === 0 ? (
                <div className="px-3 py-2 text-sm text-gray-500 dark:text-gray-400">{t('documents.noDocuments')}</div>
              ) : (
                documents.map((doc) => (
                  <label key={doc.document_uid} className="flex items-center gap-2 px-3 py-2 text-sm">
                    <input
                      type="checkbox"
                      checked={selectedDocumentUids.includes(doc.document_uid)}
                      onChange={() => toggleDocument(doc.document_uid)}
                      className="rounded border-gray-300 dark:border-gray-600"
                    />
                    <span className="truncate text-gray-700 dark:text-gray-300">{doc.display_name}</span>
                  </label>
                ))
              )}
            </div>
          </div>
        )}
      </div>

      {queryResults.length > 0 && (
        <div className="text-sm text-gray-600 dark:text-gray-400">
          {t('query.resultsFound', { count: queryResults.length, total: totalSearched })}
        </div>
      )}

      <div className="grid grid-cols-1 xl:grid-cols-2 gap-4">
        <div className="space-y-3">
          {isQuerying ? (
            <div className="text-center py-8 text-sm text-gray-500 dark:text-gray-400">{t('query.searching')}</div>
          ) : queryResults.length === 0 ? (
            <div className="rounded-lg border border-dashed border-gray-300 dark:border-gray-700 px-4 py-8 text-center text-sm text-gray-500 dark:text-gray-400">
              {searchQuery
                ? t('query.noResults')
                : t('retrievalLab.emptyResults', { defaultValue: 'Run a query to inspect results.' })}
            </div>
          ) : (
            queryResults.map((result, index) => (
              <div
                key={`${result.collection_id}-${result.document_uid}-${index}`}
                className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-hidden"
              >
                <div className="flex items-center justify-between px-4 py-2 border-b border-gray-100 dark:border-gray-800">
                  <div className="flex items-center gap-3 min-w-0">
                    <span className="text-xs font-mono text-gray-400">#{index + 1}</span>
                    <span className="text-sm font-medium text-gray-700 dark:text-gray-300 truncate">
                      {result.document_id}
                    </span>
                  </div>
                  <span className="text-xs px-2 py-0.5 rounded-full bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300">
                    {(result.score * 100).toFixed(1)}%
                  </span>
                </div>
                <div className="px-4 py-3">
                  <p className="text-sm text-gray-800 dark:text-gray-200 whitespace-pre-wrap leading-relaxed">
                    {highlightQueryTerms(result.chunk_text, searchQuery)}
                  </p>
                </div>
              </div>
            ))
          )}
        </div>

        <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-hidden">
          <div className="px-4 py-3 border-b border-gray-100 dark:border-gray-800 flex items-center justify-between">
            <h4 className="text-sm font-semibold text-gray-900 dark:text-white">
              {t('retrievalLab.runHistory', { defaultValue: 'Query Run History' })}
            </h4>
            <span className="text-xs text-gray-500 dark:text-gray-400">
              {t('retrievalLab.lastRuns', { defaultValue: 'Last {{count}} runs', count: queryRuns.length })}
            </span>
          </div>

          <div className="grid grid-cols-2 gap-2 p-3 border-b border-gray-100 dark:border-gray-800">
            <MetricCard label="Avg" value={`${runStats.avgTotalMs}ms`} />
            <MetricCard label="P50" value={`${runStats.p50TotalMs}ms`} />
            <MetricCard label="P95" value={`${runStats.p95TotalMs}ms`} />
            <MetricCard
              label={t('retrievalLab.resultCount', { defaultValue: 'Results' })}
              value={String(runStats.avgResults)}
            />
          </div>

          {queryRuns.length === 0 ? (
            <div className="px-4 py-8 text-sm text-center text-gray-500 dark:text-gray-400">
              {t('retrievalLab.emptyRuns', { defaultValue: 'No run telemetry yet.' })}
            </div>
          ) : (
            <div className="max-h-[520px] overflow-y-auto divide-y divide-gray-100 dark:divide-gray-800">
              {queryRuns.map((run) => (
                <div key={run.id} className="px-4 py-3 space-y-2">
                  <div className="flex items-center justify-between gap-3">
                    <div className="text-sm font-medium text-gray-900 dark:text-white truncate">{run.query}</div>
                    <div className="flex items-center gap-2">
                      <span className="text-[11px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 uppercase">
                        {run.retrieval_profile}
                      </span>
                      <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">
                        {formatDateTime(run.created_at)}
                      </span>
                    </div>
                  </div>
                  <div className="grid grid-cols-3 gap-2 text-xs text-gray-600 dark:text-gray-400">
                    <div>
                      <span className="text-gray-500 dark:text-gray-500">
                        {t('retrievalLab.totalMs', { defaultValue: 'Total' })}
                      </span>
                      <div className="font-mono text-gray-900 dark:text-gray-200">{run.total_ms}ms</div>
                    </div>
                    <div>
                      <span className="text-gray-500 dark:text-gray-500">
                        {t('retrievalLab.rerankMs', { defaultValue: 'Rerank' })}
                      </span>
                      <div className="font-mono text-gray-900 dark:text-gray-200">{run.rerank_ms}ms</div>
                    </div>
                    <div>
                      <span className="text-gray-500 dark:text-gray-500">
                        {t('retrievalLab.resultCount', { defaultValue: 'Results' })}
                      </span>
                      <div className="font-mono text-gray-900 dark:text-gray-200">{run.result_count}</div>
                    </div>
                  </div>
                  <div className="grid grid-cols-3 gap-2 text-xs text-gray-600 dark:text-gray-400">
                    <div>
                      <span className="text-gray-500 dark:text-gray-500">Vector</span>
                      <div className="font-mono text-gray-900 dark:text-gray-200">{run.vector_candidates}</div>
                    </div>
                    <div>
                      <span className="text-gray-500 dark:text-gray-500">BM25</span>
                      <div className="font-mono text-gray-900 dark:text-gray-200">{run.bm25_candidates}</div>
                    </div>
                    <div>
                      <span className="text-gray-500 dark:text-gray-500">Merged</span>
                      <div className="font-mono text-gray-900 dark:text-gray-200">{run.merged_candidates}</div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default RetrievalLab;

interface MetricCardProps {
  label: string;
  value: string;
}

function MetricCard({ label, value }: MetricCardProps) {
  return (
    <div className="rounded border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 px-2 py-1.5">
      <div className="text-[11px] text-gray-500 dark:text-gray-400">{label}</div>
      <div className="text-sm font-semibold text-gray-900 dark:text-gray-100">{value}</div>
    </div>
  );
}
