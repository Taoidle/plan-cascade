/**
 * CodebaseDetail Component
 *
 * Right panel with three tabs: Overview, Files, and Search.
 * Shows project index summary, language breakdown, embedding info,
 * file browsing, and semantic search.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { useCodebaseStore } from '../../store/codebase';
import type { IndexStatusEvent } from '../../lib/codebaseApi';
import { classifyComponents } from '../../lib/codebaseApi';
import { CodebaseFileList } from './CodebaseFileList';
import { CodebaseSearch } from './CodebaseSearch';

type Tab = 'overview' | 'files' | 'search';

interface CodebaseDetailProps {
  projectPath: string;
  liveStatus?: IndexStatusEvent;
  onBack: () => void;
}

export function CodebaseDetail({ projectPath, liveStatus, onBack }: CodebaseDetailProps) {
  const { t } = useTranslation('codebase');
  const { projectDetail, detailLoading, reindexProject, loadProjectDetail } = useCodebaseStore();
  const [activeTab, setActiveTab] = useState<Tab>('overview');
  const [classifying, setClassifying] = useState(false);

  const detail = projectDetail;
  const status = liveStatus?.status ?? detail?.status?.status ?? 'idle';

  const tabs: { key: Tab; label: string }[] = [
    { key: 'overview', label: t('overview') },
    { key: 'files', label: t('fileList') },
    { key: 'search', label: t('search') },
  ];

  return (
    <div className="h-full flex flex-col">
      {/* Header with back button and tabs */}
      <div className="border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900">
        <div className="px-4 py-2 flex items-center gap-3">
          <button
            onClick={onBack}
            className="md:hidden text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200"
          >
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <h3 className="text-sm font-medium text-gray-900 dark:text-white truncate flex-1" title={projectPath}>
            {projectPath}
          </h3>
          <StatusBadge status={status} t={t} />
        </div>

        {/* Tabs */}
        <div className="px-4 flex gap-1">
          {tabs.map((tab) => (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              className={clsx(
                'px-3 py-2 text-sm font-medium border-b-2 transition-colors',
                activeTab === tab.key
                  ? 'border-primary-500 text-primary-600 dark:text-primary-400'
                  : 'border-transparent text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
              )}
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-y-auto">
        {detailLoading ? (
          <div className="p-8 text-center">
            <div className="animate-pulse text-sm text-gray-500">Loading...</div>
          </div>
        ) : activeTab === 'overview' ? (
          <OverviewTab
            detail={detail}
            status={status}
            liveStatus={liveStatus}
            onReindex={() => reindexProject(projectPath)}
            classifying={classifying}
            onClassify={async () => {
              setClassifying(true);
              try {
                await classifyComponents(projectPath);
                await loadProjectDetail(projectPath);
              } finally {
                setClassifying(false);
              }
            }}
            t={t}
          />
        ) : activeTab === 'files' ? (
          <CodebaseFileList projectPath={projectPath} />
        ) : (
          <CodebaseSearch projectPath={projectPath} />
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// StatusBadge
// ---------------------------------------------------------------------------

function StatusBadge({ status, t }: { status: string; t: (key: string) => string }) {
  const config: Record<string, { bg: string; text: string; label: string }> = {
    indexing: {
      bg: 'bg-blue-100 dark:bg-blue-900/30',
      text: 'text-blue-700 dark:text-blue-300',
      label: t('indexing'),
    },
    indexed: {
      bg: 'bg-green-100 dark:bg-green-900/30',
      text: 'text-green-700 dark:text-green-300',
      label: t('indexed'),
    },
    error: {
      bg: 'bg-red-100 dark:bg-red-900/30',
      text: 'text-red-700 dark:text-red-300',
      label: t('error'),
    },
  };

  const c = config[status] ?? {
    bg: 'bg-gray-100 dark:bg-gray-800',
    text: 'text-gray-600 dark:text-gray-400',
    label: t('idle'),
  };

  return <span className={clsx('px-2 py-0.5 rounded-full text-xs font-medium', c.bg, c.text)}>{c.label}</span>;
}

// ---------------------------------------------------------------------------
// OverviewTab
// ---------------------------------------------------------------------------

function OverviewTab({
  detail,
  status,
  liveStatus,
  onReindex,
  classifying,
  onClassify,
  t,
}: {
  detail: ReturnType<typeof useCodebaseStore.getState>['projectDetail'];
  status: string;
  liveStatus?: IndexStatusEvent;
  onReindex: () => void;
  classifying: boolean;
  onClassify: () => void;
  t: ReturnType<typeof useTranslation>['t'];
}) {
  if (!detail) {
    return <div className="p-8 text-center text-sm text-gray-500 dark:text-gray-400">{t('noDetail')}</div>;
  }

  const { summary, languages, embedding_metadata } = detail;
  const hasEmbeddings = summary.embedding_chunks > 0;

  return (
    <div className="p-4 space-y-6">
      {/* Stats cards */}
      <div className="grid grid-cols-3 gap-3">
        <StatCard label={t('files')} value={summary.total_files} />
        <StatCard label={t('symbols')} value={summary.total_symbols} />
        <StatCard label={t('embeddings')} value={summary.embedding_chunks} />
      </div>

      {/* Indexing progress */}
      {status === 'indexing' && liveStatus && (
        <div className="bg-blue-50 dark:bg-blue-900/20 rounded-lg p-3">
          <div className="flex items-center gap-2 mb-2">
            <span className="w-2 h-2 rounded-full bg-blue-500 animate-pulse" />
            <span className="text-sm font-medium text-blue-700 dark:text-blue-300">{t('indexing')}</span>
          </div>
          <div className="w-full bg-blue-200 dark:bg-blue-800 rounded-full h-1.5">
            <div
              className="bg-blue-500 h-1.5 rounded-full transition-all"
              style={{
                width: `${liveStatus.total_files > 0 ? (liveStatus.indexed_files / liveStatus.total_files) * 100 : 0}%`,
              }}
            />
          </div>
          <p className="text-xs text-blue-600 dark:text-blue-400 mt-1">
            {liveStatus.indexed_files} / {liveStatus.total_files} {t('files')}
          </p>
        </div>
      )}

      {/* Languages */}
      {languages.length > 0 && (
        <Section title={t('languages')}>
          <div className="flex flex-wrap gap-2">
            {languages.map((lang) => (
              <span
                key={lang.language}
                className={clsx(
                  'px-2 py-1 rounded-md text-xs font-medium',
                  'bg-gray-100 dark:bg-gray-800',
                  'text-gray-700 dark:text-gray-300',
                )}
              >
                {lang.language} <span className="text-gray-500 dark:text-gray-400">({lang.count})</span>
              </span>
            ))}
          </div>
        </Section>
      )}

      {/* Components */}
      {summary.components.length > 0 && (
        <Section title={t('components')}>
          <div className="flex flex-wrap gap-2">
            {summary.components.map((comp) => (
              <span
                key={comp.name}
                className={clsx(
                  'px-2 py-1 rounded-md text-xs font-medium',
                  'bg-purple-50 dark:bg-purple-900/20',
                  'text-purple-700 dark:text-purple-300',
                )}
              >
                {comp.name} <span className="text-purple-500 dark:text-purple-400">({comp.count})</span>
              </span>
            ))}
          </div>
        </Section>
      )}

      {/* Entry Points */}
      {summary.key_entry_points.length > 0 && (
        <Section title={t('entryPoints')}>
          <ul className="space-y-1">
            {summary.key_entry_points.map((ep) => (
              <li key={ep} className="text-xs text-gray-600 dark:text-gray-400 font-mono">
                {ep}
              </li>
            ))}
          </ul>
        </Section>
      )}

      {/* Embedding info */}
      <Section title={t('embeddingProvider')}>
        {embedding_metadata.length > 0 ? (
          <div className="space-y-2">
            {embedding_metadata.map((meta, i) => (
              <div key={i} className="flex items-center gap-4 text-sm">
                <div>
                  <span className="text-gray-500 dark:text-gray-400">{t('embeddingProvider')}:</span>{' '}
                  <span className="text-gray-900 dark:text-white font-medium">{meta.provider_type}</span>
                </div>
                <div>
                  <span className="text-gray-500 dark:text-gray-400">{t('embeddingModel')}:</span>{' '}
                  <span className="text-gray-900 dark:text-white font-medium">{meta.provider_model}</span>
                </div>
                <div>
                  <span className="text-gray-500 dark:text-gray-400">{t('embeddingDimension')}:</span>{' '}
                  <span className="text-gray-900 dark:text-white font-medium">{meta.embedding_dimension}</span>
                </div>
              </div>
            ))}
            <span
              className={clsx(
                'inline-block px-2 py-0.5 rounded text-xs font-medium',
                hasEmbeddings
                  ? 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400',
              )}
            >
              {hasEmbeddings ? t('semanticSearchAvailable') : t('semanticSearchUnavailable')}
            </span>
          </div>
        ) : (
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('semanticSearchUnavailable')}</p>
        )}
      </Section>

      {/* Action buttons */}
      <div className="flex gap-2">
        <button
          onClick={onReindex}
          disabled={status === 'indexing'}
          className={clsx(
            'px-4 py-2 rounded-lg text-sm font-medium',
            'bg-primary-600 hover:bg-primary-700',
            'text-white',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'transition-colors',
          )}
        >
          {status === 'indexing' ? t('reindexing') : t('reindex')}
        </button>
        <button
          onClick={onClassify}
          disabled={classifying || status === 'indexing'}
          className={clsx(
            'px-4 py-2 rounded-lg text-sm font-medium',
            'bg-purple-600 hover:bg-purple-700',
            'text-white',
            'disabled:opacity-50 disabled:cursor-not-allowed',
            'transition-colors',
          )}
        >
          {classifying ? t('classifying') : t('aiClassify')}
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function StatCard({ label, value }: { label: string; value: number }) {
  return (
    <div
      className={clsx(
        'rounded-lg p-3',
        'bg-gray-50 dark:bg-gray-800/50',
        'border border-gray-200 dark:border-gray-700',
      )}
    >
      <div className="text-2xl font-bold text-gray-900 dark:text-white">{value.toLocaleString()}</div>
      <div className="text-xs text-gray-500 dark:text-gray-400 mt-1">{label}</div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <h4 className="text-sm font-medium text-gray-900 dark:text-white mb-2">{title}</h4>
      {children}
    </div>
  );
}
