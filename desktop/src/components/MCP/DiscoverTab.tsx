import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { clsx } from 'clsx';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { ReloadIcon, RocketIcon, ExternalLinkIcon } from '@radix-ui/react-icons';
import type {
  CommandResponse,
  McpCatalogItem,
  McpCatalogListResponse,
  McpCatalogRefreshResult,
  McpCatalogTrustLevel,
} from '../../types/mcp';

type CatalogEventStatus = 'success' | 'error' | 'info';

interface DiscoverTabProps {
  onInstallItem: (item: McpCatalogItem) => void;
  installRecommendedNonce?: number;
  installedCatalogItems?: Record<string, { serverId: string; serverName: string; managed: boolean }[]>;
  onCatalogEvent?: (event: { action: 'catalog_refresh'; status: CatalogEventStatus; detail?: string }) => void;
}

export function DiscoverTab({
  onInstallItem,
  installRecommendedNonce = 0,
  installedCatalogItems,
  onCatalogEvent,
}: DiscoverTabProps) {
  const { t } = useTranslation();
  const [items, setItems] = useState<McpCatalogItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshNotice, setRefreshNotice] = useState<string | null>(null);
  const [catalogMeta, setCatalogMeta] = useState<Pick<
    McpCatalogListResponse,
    'source' | 'fetched_at' | 'signature_valid'
  > | null>(null);
  const [query, setQuery] = useState('');
  const [trustFilter, setTrustFilter] = useState<'all' | McpCatalogTrustLevel>('all');
  const onCatalogEventRef = useRef(onCatalogEvent);
  const hasFetchedInitiallyRef = useRef(false);

  useEffect(() => {
    onCatalogEventRef.current = onCatalogEvent;
  }, [onCatalogEvent]);

  const sourceToLabel = useCallback(
    (source?: string | null) => {
      if (source === 'remote') {
        return t('mcp.discover.source.remote');
      }
      return t('mcp.discover.source.builtin');
    },
    [t],
  );

  const fetchCatalog = useCallback(
    async (force = false) => {
      setLoading(true);
      setError(null);
      if (force) {
        setRefreshNotice(null);
      }
      try {
        if (force) {
          const refresh = await invoke<CommandResponse<McpCatalogRefreshResult>>('refresh_mcp_catalog', {
            force: true,
          });
          if (!refresh.success) {
            throw new Error(refresh.error || t('mcp.errors.fetchCatalog'));
          }
          if (refresh.data?.error) {
            setRefreshNotice(refresh.data.error);
            onCatalogEventRef.current?.({
              action: 'catalog_refresh',
              status: 'info',
              detail: t('mcp.discover.refreshFallback', { reason: refresh.data.error }),
            });
          }
          if (refresh.data) {
            const refreshDetail = [
              t('mcp.discover.catalogSource', { source: sourceToLabel(refresh.data.source) }),
              refresh.data.signature_valid ? t('mcp.discover.signatureValid') : t('mcp.discover.signatureInvalid'),
            ].join(' | ');
            onCatalogEventRef.current?.({
              action: 'catalog_refresh',
              status: refresh.data.signature_valid ? 'success' : 'error',
              detail: refreshDetail,
            });
          }
        }
        const response = await invoke<CommandResponse<McpCatalogListResponse>>('list_mcp_catalog');
        if (response.success && response.data) {
          setItems(response.data.items);
          setCatalogMeta({
            source: response.data.source,
            fetched_at: response.data.fetched_at ?? null,
            signature_valid: response.data.signature_valid,
          });
          if (!response.data.signature_valid) {
            onCatalogEventRef.current?.({
              action: 'catalog_refresh',
              status: 'error',
              detail: t('mcp.discover.signatureInvalid'),
            });
          }
        } else {
          const message = response.error || t('mcp.errors.fetchCatalog');
          setError(message);
          onCatalogEventRef.current?.({
            action: 'catalog_refresh',
            status: 'error',
            detail: message,
          });
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : t('mcp.errors.fetchCatalog');
        setError(message);
        onCatalogEventRef.current?.({
          action: 'catalog_refresh',
          status: 'error',
          detail: message,
        });
      } finally {
        setLoading(false);
      }
    },
    [sourceToLabel, t],
  );

  useEffect(() => {
    if (hasFetchedInitiallyRef.current) {
      return;
    }
    hasFetchedInitiallyRef.current = true;
    void fetchCatalog();
  }, [fetchCatalog]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return items.filter((item) => {
      if (trustFilter !== 'all' && item.trust_level !== trustFilter) return false;
      if (!q) return true;
      return (
        item.name.toLowerCase().includes(q) ||
        item.vendor.toLowerCase().includes(q) ||
        item.id.toLowerCase().includes(q) ||
        item.tags.some((tag) => tag.toLowerCase().includes(q))
      );
    });
  }, [items, query, trustFilter]);

  const sourceLabel = useMemo(
    () => sourceToLabel(catalogMeta?.source || 'builtin'),
    [catalogMeta?.source, sourceToLabel],
  );

  const fetchedAtLabel = useMemo(() => {
    if (!catalogMeta?.fetched_at) return null;
    try {
      return new Date(catalogMeta.fetched_at).toLocaleString();
    } catch {
      return catalogMeta.fetched_at;
    }
  }, [catalogMeta?.fetched_at]);

  const installBlocked = catalogMeta?.signature_valid === false;

  useEffect(() => {
    if (installRecommendedNonce === 0) return;
    if (installBlocked) return;
    const preferred = items.find((item) => item.trust_level === 'official') || items[0];
    if (preferred) {
      onInstallItem(preferred);
    }
  }, [installBlocked, installRecommendedNonce, items, onInstallItem]);

  return (
    <div className="space-y-4">
      <div className="flex flex-col lg:flex-row lg:items-center lg:justify-between gap-3">
        <div className="flex items-center gap-2">
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t('mcp.discover.searchPlaceholder')}
            className={clsx(
              'w-full lg:w-80 px-3 py-2 rounded-md',
              'bg-gray-100 dark:bg-gray-800 border border-gray-200 dark:border-gray-700',
              'text-sm text-gray-900 dark:text-white',
            )}
          />
          <button
            type="button"
            onClick={() => fetchCatalog(true)}
            disabled={loading}
            className={clsx(
              'inline-flex items-center gap-1.5 px-3 py-2 rounded-md text-sm font-medium',
              'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300',
              'hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-50',
            )}
          >
            <ReloadIcon className={clsx('w-4 h-4', loading && 'animate-spin')} />
            {t('mcp.refresh')}
          </button>
        </div>

        <div className="flex items-center gap-2">
          {(['all', 'official', 'verified', 'community'] as const).map((level) => (
            <button
              key={level}
              type="button"
              onClick={() => setTrustFilter(level)}
              className={clsx(
                'px-3 py-1.5 rounded-full text-xs font-medium border',
                trustFilter === level
                  ? 'bg-primary-600 text-white border-primary-600'
                  : 'bg-white dark:bg-gray-900 text-gray-700 dark:text-gray-300 border-gray-200 dark:border-gray-700',
              )}
            >
              {t(`mcp.discover.trust.${level}`)}
            </button>
          ))}
        </div>
      </div>

      {error && (
        <div className="text-sm rounded-md border border-red-200 bg-red-50 dark:border-red-900/40 dark:bg-red-900/20 text-red-700 dark:text-red-300 px-3 py-2">
          {error}
        </div>
      )}

      {catalogMeta && (
        <div
          className={clsx(
            'text-xs rounded-md border px-3 py-2 flex flex-wrap items-center gap-x-3 gap-y-1',
            catalogMeta.signature_valid
              ? 'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-900/40 dark:bg-emerald-900/20 dark:text-emerald-300'
              : 'border-red-200 bg-red-50 text-red-700 dark:border-red-900/40 dark:bg-red-900/20 dark:text-red-300',
          )}
        >
          <span>{t('mcp.discover.catalogSource', { source: sourceLabel })}</span>
          {fetchedAtLabel && <span>{t('mcp.discover.fetchedAt', { value: fetchedAtLabel })}</span>}
          <span>
            {catalogMeta.signature_valid ? t('mcp.discover.signatureValid') : t('mcp.discover.signatureInvalid')}
          </span>
        </div>
      )}

      {refreshNotice && (
        <div className="text-xs rounded-md border border-amber-200 bg-amber-50 dark:border-amber-900/40 dark:bg-amber-900/20 text-amber-700 dark:text-amber-300 px-3 py-2">
          {t('mcp.discover.refreshFallback', { reason: refreshNotice })}
        </div>
      )}

      {loading && items.length === 0 ? (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {Array.from({ length: 4 }).map((_, index) => (
            <div
              key={index}
              className="h-40 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 animate-skeleton"
            />
          ))}
        </div>
      ) : filtered.length === 0 ? (
        <div className="text-sm text-gray-500 dark:text-gray-400 py-8 text-center">{t('mcp.discover.empty')}</div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 3xl:grid-cols-3 gap-4">
          {filtered.map((item) => {
            const installedEntries = installedCatalogItems?.[item.id] || [];
            const isInstalled = installedEntries.length > 0;
            const isManaged = installedEntries.some((entry) => entry.managed);
            const installedNames = installedEntries.map((entry) => entry.serverName).join(', ');
            return (
              <div
                key={item.id}
                className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-4 flex flex-col"
              >
                <div className="flex items-start justify-between gap-2">
                  <div>
                    <h3 className="text-sm font-semibold text-gray-900 dark:text-white">{item.name}</h3>
                    <p className="text-xs text-gray-500 dark:text-gray-400">{item.vendor}</p>
                  </div>
                  <span
                    className={clsx('px-2 py-0.5 rounded-full text-[11px] font-medium', trustClass(item.trust_level))}
                  >
                    {t(`mcp.discover.trust.${item.trust_level}`)}
                  </span>
                </div>

                <div className="mt-2 flex flex-wrap gap-1">
                  {item.tags.map((tag) => (
                    <span
                      key={tag}
                      className="text-[11px] px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300"
                    >
                      {tag}
                    </span>
                  ))}
                </div>

                <p className="mt-2 text-xs text-gray-500 dark:text-gray-400">
                  {t('mcp.discover.strategyCount', { count: item.strategies.length })}
                </p>
                {isInstalled && (
                  <p className="mt-1 text-xs text-emerald-700 dark:text-emerald-300 break-all" title={installedNames}>
                    {t('mcp.discover.installed', { count: installedEntries.length })} -{' '}
                    {isManaged ? t('mcp.discover.managedInstall') : t('mcp.discover.manualInstall')}
                  </p>
                )}

                <div className="mt-auto pt-3 flex items-center justify-between">
                  {item.docs_url ? (
                    <a
                      href={item.docs_url}
                      target="_blank"
                      rel="noreferrer"
                      className="inline-flex items-center gap-1 text-xs text-primary-600 dark:text-primary-400 hover:underline"
                    >
                      <ExternalLinkIcon className="w-3 h-3" />
                      {t('mcp.discover.docs')}
                    </a>
                  ) : (
                    <span />
                  )}
                  <button
                    type="button"
                    disabled={installBlocked}
                    onClick={() => onInstallItem(item)}
                    className={clsx(
                      'inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium text-white',
                      'bg-primary-600 hover:bg-primary-700 disabled:opacity-50 disabled:cursor-not-allowed',
                    )}
                  >
                    <RocketIcon className="w-3 h-3" />
                    {isInstalled ? t('mcp.discover.installAnother') : t('mcp.discover.install')}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function trustClass(level: McpCatalogTrustLevel): string {
  if (level === 'official') return 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-300';
  if (level === 'verified') return 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300';
  return 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-300';
}
