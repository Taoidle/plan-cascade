/**
 * MarketplaceView Component
 *
 * Browse and search plugins from configured marketplace sources.
 * Displays plugin cards in a responsive grid with category and marketplace filtering.
 */

import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { MagnifyingGlassIcon, ReloadIcon, PlusIcon } from '@radix-ui/react-icons';
import { usePluginStore } from '../../store/plugins';
import { PluginMarketplaceCard } from './PluginMarketplaceCard';

export function MarketplaceView() {
  const { t } = useTranslation('settings');

  const marketplacePlugins = usePluginStore((s) => s.marketplacePlugins);
  const marketplaceLoading = usePluginStore((s) => s.marketplaceLoading);
  const marketplaceError = usePluginStore((s) => s.marketplaceError);
  const installing = usePluginStore((s) => s.installing);
  const uninstalling = usePluginStore((s) => s.uninstalling);
  const loadMarketplace = usePluginStore((s) => s.loadMarketplace);
  const installFromMarketplace = usePluginStore((s) => s.installFromMarketplace);
  const uninstallPlugin = usePluginStore((s) => s.uninstallPlugin);
  const openAddMarketplaceDialog = usePluginStore((s) => s.openAddMarketplaceDialog);

  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [selectedMarketplace, setSelectedMarketplace] = useState<string | null>(null);

  // Load marketplace on mount
  useEffect(() => {
    if (marketplacePlugins.length === 0 && !marketplaceLoading) {
      loadMarketplace();
    }
  }, [marketplacePlugins.length, marketplaceLoading, loadMarketplace]);

  // Extract unique categories from plugins
  const categories = useMemo(() => {
    const cats = new Set<string>();
    marketplacePlugins.forEach((p) => {
      if (p.category) cats.add(p.category);
    });
    return Array.from(cats).sort();
  }, [marketplacePlugins]);

  // Extract unique marketplace sources
  const marketplaceSources = useMemo(() => {
    const sources = new Set<string>();
    marketplacePlugins.forEach((p) => {
      if (p.marketplace_name) sources.add(p.marketplace_name);
    });
    return Array.from(sources).sort();
  }, [marketplacePlugins]);

  // Filter plugins
  const filteredPlugins = useMemo(() => {
    let result = marketplacePlugins;

    if (selectedCategory) {
      result = result.filter((p) => p.category === selectedCategory);
    }

    if (selectedMarketplace) {
      result = result.filter((p) => p.marketplace_name === selectedMarketplace);
    }

    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      result = result.filter(
        (p) =>
          p.name.toLowerCase().includes(q) ||
          p.description.toLowerCase().includes(q) ||
          (p.author && p.author.toLowerCase().includes(q)) ||
          p.keywords.some((k) => k.toLowerCase().includes(q))
      );
    }

    return result;
  }, [marketplacePlugins, searchQuery, selectedCategory, selectedMarketplace]);

  // Loading state
  if (marketplaceLoading && marketplacePlugins.length === 0) {
    return (
      <div className="p-4 space-y-4">
        <div className="grid gap-4 grid-cols-1 lg:grid-cols-2">
          {[1, 2, 3, 4].map((i) => (
            <div
              key={i}
              className={clsx(
                'p-4 rounded-lg border border-gray-200 dark:border-gray-700',
                'animate-pulse'
              )}
            >
              <div className="h-4 w-2/3 bg-gray-200 dark:bg-gray-700 rounded mb-3" />
              <div className="h-3 w-full bg-gray-100 dark:bg-gray-800 rounded mb-2" />
              <div className="h-3 w-1/2 bg-gray-100 dark:bg-gray-800 rounded" />
            </div>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Search & Actions */}
      <div className="p-3 space-y-2">
        <div className="flex items-center gap-2">
          <div className="relative flex-1">
            <MagnifyingGlassIcon className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={t('plugins.searchMarketplace')}
              className={clsx(
                'w-full pl-8 pr-3 py-1.5 rounded-md text-xs',
                'bg-gray-50 dark:bg-gray-800',
                'border border-gray-200 dark:border-gray-700',
                'text-gray-700 dark:text-gray-300',
                'placeholder:text-gray-400 dark:placeholder:text-gray-500',
                'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent'
              )}
            />
          </div>
          <button
            onClick={openAddMarketplaceDialog}
            className={clsx(
              'inline-flex items-center gap-1 px-2 py-1.5 rounded-md text-xs',
              'text-gray-500 dark:text-gray-400',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
              'transition-colors'
            )}
            title={t('plugins.addMarketplace')}
          >
            <PlusIcon className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">{t('plugins.addMarketplace')}</span>
          </button>
          <button
            onClick={() => loadMarketplace()}
            disabled={marketplaceLoading}
            className={clsx(
              'p-1.5 rounded-md transition-colors',
              'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
              'hover:bg-gray-100 dark:hover:bg-gray-800',
              'disabled:opacity-50'
            )}
            title={t('plugins.refresh')}
          >
            <ReloadIcon className={clsx('w-3.5 h-3.5', marketplaceLoading && 'animate-spin')} />
          </button>
        </div>

        {/* Marketplace source filter (only if multiple sources) */}
        {marketplaceSources.length > 1 && (
          <div className="flex flex-wrap gap-1">
            <button
              onClick={() => setSelectedMarketplace(null)}
              className={clsx(
                'px-2 py-0.5 rounded-full text-2xs font-medium transition-colors',
                !selectedMarketplace
                  ? 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
              )}
            >
              {t('plugins.allCategories')}
            </button>
            {marketplaceSources.map((source) => (
              <button
                key={source}
                onClick={() => setSelectedMarketplace(source === selectedMarketplace ? null : source)}
                className={clsx(
                  'px-2 py-0.5 rounded-full text-2xs font-medium transition-colors',
                  source === selectedMarketplace
                    ? 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300'
                    : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
                )}
              >
                {source === 'claude-plugins-official'
                  ? t('plugins.officialMarketplace')
                  : source}
              </button>
            ))}
          </div>
        )}

        {/* Category filter */}
        {categories.length > 0 && (
          <div className="flex flex-wrap gap-1">
            <button
              onClick={() => setSelectedCategory(null)}
              className={clsx(
                'px-2 py-0.5 rounded-full text-2xs font-medium transition-colors',
                !selectedCategory
                  ? 'bg-primary-100 dark:bg-primary-900 text-primary-700 dark:text-primary-300'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
              )}
            >
              {t('plugins.allCategories')}
            </button>
            {categories.map((cat) => (
              <button
                key={cat}
                onClick={() => setSelectedCategory(cat === selectedCategory ? null : cat)}
                className={clsx(
                  'px-2 py-0.5 rounded-full text-2xs font-medium transition-colors capitalize',
                  cat === selectedCategory
                    ? 'bg-primary-100 dark:bg-primary-900 text-primary-700 dark:text-primary-300'
                    : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'
                )}
              >
                {cat}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Error banner */}
      {marketplaceError && (
        <div className="mx-3 mb-2 px-3 py-2 rounded-lg text-xs bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300">
          {marketplaceError}
        </div>
      )}

      {/* Plugin grid */}
      <div className="flex-1 overflow-y-auto p-3">
        {filteredPlugins.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-8 text-center">
            <MagnifyingGlassIcon className="w-8 h-8 text-gray-300 dark:text-gray-600 mb-2" />
            <p className="text-xs text-gray-500 dark:text-gray-400">
              {t('plugins.noMarketplaceResults')}
            </p>
          </div>
        ) : (
          <div className="grid gap-3 grid-cols-1 lg:grid-cols-2">
            {filteredPlugins.map((plugin) => (
              <PluginMarketplaceCard
                key={`${plugin.marketplace_name}:${plugin.name}`}
                plugin={plugin}
                onInstall={() => installFromMarketplace(plugin.name, plugin.marketplace_name)}
                onUninstall={() => uninstallPlugin(plugin.name)}
                installing={installing}
                uninstalling={uninstalling === plugin.name}
              />
            ))}
          </div>
        )}
      </div>

      {/* Footer: marketplace count */}
      {marketplacePlugins.length > 0 && (
        <div className="px-3 py-2 border-t border-gray-200 dark:border-gray-700">
          <p className="text-2xs text-gray-400 dark:text-gray-500">
            {filteredPlugins.length} / {marketplacePlugins.length} {t('plugins.marketplace').toLowerCase()}
          </p>
        </div>
      )}
    </div>
  );
}

export default MarketplaceView;
