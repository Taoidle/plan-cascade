/**
 * PluginMarketplaceCard Component
 *
 * Displays a single plugin from the marketplace registry.
 * Shows install/installed status, metadata, and action buttons.
 */

import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { DownloadIcon, StarIcon, ReloadIcon, TrashIcon } from '@radix-ui/react-icons';
import type { MarketplacePlugin } from '../../types/plugin';

interface PluginMarketplaceCardProps {
  plugin: MarketplacePlugin;
  onInstall: () => void;
  onUninstall: () => void;
  installing: boolean;
  uninstalling: boolean;
}

export function PluginMarketplaceCard({
  plugin,
  onInstall,
  onUninstall,
  installing,
  uninstalling,
}: PluginMarketplaceCardProps) {
  const { t } = useTranslation('settings');

  return (
    <div
      className={clsx(
        'p-4 rounded-lg border transition-colors',
        'bg-white dark:bg-gray-800 border-gray-200 dark:border-gray-700',
        'hover:border-gray-300 dark:hover:border-gray-600'
      )}
    >
      {/* Header: name + version */}
      <div className="flex items-start justify-between mb-2">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h3 className="font-medium text-sm text-gray-900 dark:text-white truncate">
              {plugin.name}
            </h3>
            <span className="text-2xs text-gray-400 dark:text-gray-500 shrink-0">
              v{plugin.version}
            </span>
          </div>
          {plugin.author && (
            <p className="text-2xs text-gray-400 dark:text-gray-500 mt-0.5">
              {plugin.author}
              {plugin.license && ` \u00B7 ${plugin.license}`}
            </p>
          )}
        </div>

        {/* Status / Action */}
        <div className="shrink-0 ml-2">
          {plugin.installed ? (
            <span
              className={clsx(
                'inline-flex items-center px-2 py-0.5 rounded text-2xs font-medium',
                'bg-green-100 dark:bg-green-900/50 text-green-700 dark:text-green-300'
              )}
            >
              {t('plugins.installed')}
            </span>
          ) : (
            <button
              onClick={onInstall}
              disabled={installing}
              className={clsx(
                'inline-flex items-center gap-1 px-2.5 py-1 rounded-md text-xs font-medium',
                'bg-primary-600 hover:bg-primary-700 text-white',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'transition-colors'
              )}
            >
              {installing ? (
                <ReloadIcon className="w-3 h-3 animate-spin" />
              ) : (
                <DownloadIcon className="w-3 h-3" />
              )}
              {t('plugins.install')}
            </button>
          )}
        </div>
      </div>

      {/* Description */}
      <p className="text-xs text-gray-600 dark:text-gray-300 line-clamp-2 mb-2">
        {plugin.description || t('plugins.noDescription')}
      </p>

      {/* Keywords */}
      {plugin.keywords.length > 0 && (
        <div className="flex flex-wrap gap-1 mb-2">
          {plugin.keywords.slice(0, 5).map((kw) => (
            <span
              key={kw}
              className={clsx(
                'px-1.5 py-0.5 text-2xs rounded-full',
                'bg-gray-100 dark:bg-gray-700',
                'text-gray-500 dark:text-gray-400'
              )}
            >
              {kw}
            </span>
          ))}
        </div>
      )}

      {/* Footer: stats + uninstall */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3 text-2xs text-gray-400 dark:text-gray-500">
          {plugin.stars > 0 && (
            <span className="flex items-center gap-0.5">
              <StarIcon className="w-3 h-3" />
              {plugin.stars}
            </span>
          )}
          {plugin.downloads > 0 && (
            <span className="flex items-center gap-0.5">
              <DownloadIcon className="w-3 h-3" />
              {plugin.downloads}
            </span>
          )}
        </div>

        {/* Uninstall button (only for installed project_local plugins) */}
        {plugin.installed && (
          <button
            onClick={onUninstall}
            disabled={uninstalling}
            className={clsx(
              'inline-flex items-center gap-1 px-2 py-0.5 rounded text-2xs',
              'text-red-600 dark:text-red-400',
              'hover:bg-red-50 dark:hover:bg-red-900/20',
              'disabled:opacity-50 disabled:cursor-not-allowed',
              'transition-colors'
            )}
          >
            {uninstalling ? (
              <ReloadIcon className="w-3 h-3 animate-spin" />
            ) : (
              <TrashIcon className="w-3 h-3" />
            )}
            {t('plugins.uninstall')}
          </button>
        )}
      </div>
    </div>
  );
}

export default PluginMarketplaceCard;
