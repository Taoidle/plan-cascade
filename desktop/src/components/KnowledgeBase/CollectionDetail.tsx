/**
 * CollectionDetail Component
 *
 * Displays metadata for a selected knowledge collection including
 * chunk count, description, and timestamps.
 */

import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import type { KnowledgeCollection } from '../../lib/knowledgeApi';

interface CollectionDetailProps {
  collection: KnowledgeCollection;
}

export function CollectionDetail({ collection }: CollectionDetailProps) {
  const { t } = useTranslation('knowledge');

  const formatDate = (dateStr: string) => {
    try {
      return new Date(dateStr).toLocaleString();
    } catch {
      return dateStr;
    }
  };

  return (
    <div className="p-6 space-y-6">
      {/* Collection Name */}
      <div>
        <h3 className="text-xl font-semibold text-gray-900 dark:text-white">
          {collection.name}
        </h3>
        {collection.description && (
          <p className="text-sm text-gray-600 dark:text-gray-400 mt-1">
            {collection.description}
          </p>
        )}
      </div>

      {/* Stats Grid */}
      <div className="grid grid-cols-2 lg:grid-cols-3 gap-4">
        <StatCard
          label={t('detail.chunkCount')}
          value={String(collection.chunk_count)}
        />
        <StatCard
          label={t('detail.projectId')}
          value={collection.project_id}
        />
        <StatCard
          label={t('detail.collectionId')}
          value={collection.id}
          truncate
        />
      </div>

      {/* Timestamps */}
      <div className={clsx(
        'rounded-lg p-4',
        'bg-gray-50 dark:bg-gray-900',
        'border border-gray-200 dark:border-gray-700'
      )}>
        <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
          {t('detail.timestamps')}
        </h4>
        <div className="space-y-2">
          <div className="flex justify-between text-sm">
            <span className="text-gray-500 dark:text-gray-400">{t('detail.createdAt')}</span>
            <span className="text-gray-900 dark:text-white font-mono text-xs">
              {formatDate(collection.created_at)}
            </span>
          </div>
          <div className="flex justify-between text-sm">
            <span className="text-gray-500 dark:text-gray-400">{t('detail.updatedAt')}</span>
            <span className="text-gray-900 dark:text-white font-mono text-xs">
              {formatDate(collection.updated_at)}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// StatCard
// ---------------------------------------------------------------------------

interface StatCardProps {
  label: string;
  value: string;
  truncate?: boolean;
}

function StatCard({ label, value, truncate }: StatCardProps) {
  return (
    <div className={clsx(
      'rounded-lg p-4',
      'bg-gray-50 dark:bg-gray-900',
      'border border-gray-200 dark:border-gray-700'
    )}>
      <div className="text-xs text-gray-500 dark:text-gray-400 mb-1">{label}</div>
      <div className={clsx(
        'text-lg font-semibold text-gray-900 dark:text-white',
        truncate && 'truncate text-sm'
      )}>
        {value}
      </div>
    </div>
  );
}
