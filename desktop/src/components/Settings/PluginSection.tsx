/**
 * PluginSection Component
 *
 * Settings section for plugin management. Shows discovered plugins
 * with toggle switches, and provides detail views for inspecting
 * plugin skills, commands, hooks, and instructions.
 */

import { useEffect, useState, useCallback } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  ReloadIcon,
  ChevronRightIcon,
  ChevronLeftIcon,
  InfoCircledIcon,
} from '@radix-ui/react-icons';
import { usePluginStore } from '../../store/plugins';
import { getPluginSourceLabel } from '../../types/plugin';
import type { PluginInfo, PluginDetail } from '../../types/plugin';

// ---------------------------------------------------------------------------
// Plugin List View
// ---------------------------------------------------------------------------

function PluginListItem({
  plugin,
  onToggle,
  onSelect,
}: {
  plugin: PluginInfo;
  onToggle: (name: string, enabled: boolean) => void;
  onSelect: (name: string) => void;
}) {
  const { t } = useTranslation('settings');

  return (
    <div
      className={clsx(
        'flex items-center justify-between p-3 rounded-lg',
        'border border-gray-200 dark:border-gray-700',
        'hover:bg-gray-50 dark:hover:bg-gray-800/50',
        'transition-colors'
      )}
    >
      <div className="flex-1 min-w-0 mr-3">
        <div className="flex items-center gap-2">
          <span className="font-medium text-gray-900 dark:text-white truncate">
            {plugin.name}
          </span>
          <span className="text-xs text-gray-500 dark:text-gray-400">
            v{plugin.version}
          </span>
          <span
            className={clsx(
              'px-1.5 py-0.5 text-xs rounded',
              'bg-gray-100 dark:bg-gray-700',
              'text-gray-600 dark:text-gray-300'
            )}
          >
            {getPluginSourceLabel(plugin.source)}
          </span>
        </div>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-0.5 truncate">
          {plugin.description || t('plugins.noDescription')}
        </p>
        <div className="flex items-center gap-3 mt-1 text-xs text-gray-400 dark:text-gray-500">
          {plugin.skill_count > 0 && (
            <span>{t('plugins.skillCount', { count: plugin.skill_count })}</span>
          )}
          {plugin.command_count > 0 && (
            <span>{t('plugins.commandCount', { count: plugin.command_count })}</span>
          )}
          {plugin.hook_count > 0 && (
            <span>{t('plugins.hookCount', { count: plugin.hook_count })}</span>
          )}
          {plugin.has_instructions && <span>{t('plugins.instructions')}</span>}
        </div>
      </div>

      <div className="flex items-center gap-2 shrink-0">
        {/* Toggle switch */}
        <button
          onClick={(e) => {
            e.stopPropagation();
            onToggle(plugin.name, !plugin.enabled);
          }}
          className={clsx(
            'relative inline-flex h-5 w-9 items-center rounded-full',
            'transition-colors focus:outline-none focus:ring-2 focus:ring-primary-500',
            plugin.enabled
              ? 'bg-primary-600'
              : 'bg-gray-300 dark:bg-gray-600'
          )}
          role="switch"
          aria-checked={plugin.enabled}
          aria-label={t('plugins.toggle', { name: plugin.name })}
        >
          <span
            className={clsx(
              'inline-block h-3.5 w-3.5 rounded-full bg-white',
              'transform transition-transform',
              plugin.enabled ? 'translate-x-4.5' : 'translate-x-0.5'
            )}
          />
        </button>

        {/* Detail button */}
        <button
          onClick={() => onSelect(plugin.name)}
          className={clsx(
            'p-1.5 rounded-md',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'text-gray-500 dark:text-gray-400',
            'focus:outline-none focus:ring-2 focus:ring-primary-500'
          )}
          aria-label={t('plugins.viewDetails', { name: plugin.name })}
        >
          <ChevronRightIcon className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Plugin Detail View
// ---------------------------------------------------------------------------

function PluginDetailView({
  detail,
  onBack,
}: {
  detail: PluginDetail;
  onBack: () => void;
}) {
  const { t } = useTranslation('settings');
  const { plugin } = detail;

  return (
    <div className="space-y-4">
      {/* Back button + Header */}
      <div className="flex items-center gap-2">
        <button
          onClick={onBack}
          className={clsx(
            'p-1.5 rounded-md',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'text-gray-500 dark:text-gray-400'
          )}
        >
          <ChevronLeftIcon className="w-4 h-4" />
        </button>
        <div>
          <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
            {plugin.manifest.name}
          </h3>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            v{plugin.manifest.version}
            {plugin.manifest.author && ` by ${plugin.manifest.author}`}
            {plugin.manifest.license && ` (${plugin.manifest.license})`}
          </p>
        </div>
      </div>

      {/* Description */}
      {plugin.manifest.description && (
        <p className="text-sm text-gray-600 dark:text-gray-300">
          {plugin.manifest.description}
        </p>
      )}

      {/* Keywords */}
      {plugin.manifest.keywords.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {plugin.manifest.keywords.map((kw) => (
            <span
              key={kw}
              className={clsx(
                'px-2 py-0.5 text-xs rounded-full',
                'bg-primary-100 dark:bg-primary-900',
                'text-primary-700 dark:text-primary-300'
              )}
            >
              {kw}
            </span>
          ))}
        </div>
      )}

      {/* Skills */}
      {plugin.skills.length > 0 && (
        <DetailSection title={`${t('plugins.skills')} (${plugin.skills.length})`}>
          {plugin.skills.map((skill) => (
            <div
              key={skill.name}
              className="p-2 rounded border border-gray-200 dark:border-gray-700"
            >
              <div className="flex items-center gap-2">
                <span className="font-medium text-sm text-gray-900 dark:text-white">
                  {skill.name}
                </span>
                {skill.user_invocable && (
                  <span className="text-xs px-1.5 py-0.5 rounded bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300">
                    {t('plugins.invocable')}
                  </span>
                )}
              </div>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                {skill.description}
              </p>
              {skill.allowed_tools.length > 0 && (
                <p className="text-xs text-gray-400 mt-0.5">
                  {t('plugins.tools')} {skill.allowed_tools.join(', ')}
                </p>
              )}
            </div>
          ))}
        </DetailSection>
      )}

      {/* Commands */}
      {plugin.commands.length > 0 && (
        <DetailSection title={`${t('plugins.commands')} (${plugin.commands.length})`}>
          {plugin.commands.map((cmd) => (
            <div
              key={cmd.name}
              className="p-2 rounded border border-gray-200 dark:border-gray-700"
            >
              <span className="font-medium text-sm text-gray-900 dark:text-white">
                /{cmd.name}
              </span>
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                {cmd.description}
              </p>
            </div>
          ))}
        </DetailSection>
      )}

      {/* Hooks */}
      {plugin.hooks.length > 0 && (
        <DetailSection title={`${t('plugins.hooks')} (${plugin.hooks.length})`}>
          {plugin.hooks.map((hook, i) => (
            <div
              key={`${hook.event}-${i}`}
              className="p-2 rounded border border-gray-200 dark:border-gray-700"
            >
              <div className="flex items-center gap-2">
                <span className="font-medium text-sm text-gray-900 dark:text-white">
                  {hook.event}
                </span>
                <span
                  className={clsx(
                    'text-xs px-1.5 py-0.5 rounded',
                    hook.hook_type === 'command'
                      ? 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300'
                      : 'bg-purple-100 dark:bg-purple-900 text-purple-700 dark:text-purple-300'
                  )}
                >
                  {hook.hook_type}
                </span>
                {hook.async_hook && (
                  <span className="text-xs px-1.5 py-0.5 rounded bg-yellow-100 dark:bg-yellow-900 text-yellow-700 dark:text-yellow-300">
                    {t('plugins.async')}
                  </span>
                )}
              </div>
              {hook.matcher && (
                <p className="text-xs text-gray-400 mt-0.5">
                  {t('plugins.matcher')} <code className="bg-gray-100 dark:bg-gray-800 px-1 rounded">{hook.matcher}</code>
                </p>
              )}
              <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5 font-mono truncate">
                {hook.command}
              </p>
            </div>
          ))}
        </DetailSection>
      )}

      {/* Instructions preview */}
      {plugin.instructions && (
        <DetailSection title={t('plugins.instructions')}>
          <pre
            className={clsx(
              'text-xs text-gray-600 dark:text-gray-300',
              'bg-gray-50 dark:bg-gray-800 rounded p-3',
              'overflow-auto max-h-40',
              'whitespace-pre-wrap'
            )}
          >
            {plugin.instructions.length > 500
              ? `${plugin.instructions.slice(0, 500)}...`
              : plugin.instructions}
          </pre>
        </DetailSection>
      )}

      {/* Permissions */}
      {(plugin.permissions.allow.length > 0 ||
        plugin.permissions.deny.length > 0) && (
        <DetailSection title={t('plugins.permissions')}>
          {plugin.permissions.allow.length > 0 && (
            <p className="text-xs text-gray-500">
              <span className="text-green-600 dark:text-green-400">{t('plugins.allow')}</span>{' '}
              {plugin.permissions.allow.join(', ')}
            </p>
          )}
          {plugin.permissions.deny.length > 0 && (
            <p className="text-xs text-gray-500">
              <span className="text-red-600 dark:text-red-400">{t('plugins.deny')}</span>{' '}
              {plugin.permissions.deny.join(', ')}
            </p>
          )}
        </DetailSection>
      )}

      {/* Root path */}
      <p className="text-xs text-gray-400 dark:text-gray-500">
        {t('plugins.location')} {detail.root_path}
      </p>
    </div>
  );
}

function DetailSection({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <h4 className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
        {title}
      </h4>
      <div className="space-y-2">{children}</div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main Section
// ---------------------------------------------------------------------------

export function PluginSection() {
  const { t } = useTranslation('settings');
  const {
    plugins,
    selectedPlugin,
    loading,
    detailLoading,
    refreshing,
    error,
    toastMessage,
    toastType,
    loadPlugins,
    togglePlugin,
    refresh,
    loadPluginDetail,
    clearSelectedPlugin,
    clearToast,
  } = usePluginStore();

  const [_initialized, setInitialized] = useState(false);

  useEffect(() => {
    if (!_initialized) {
      loadPlugins();
      setInitialized(true);
    }
  }, [_initialized, loadPlugins]);

  // Auto-clear toast
  useEffect(() => {
    if (toastMessage) {
      const timer = setTimeout(clearToast, 3000);
      return () => clearTimeout(timer);
    }
  }, [toastMessage, clearToast]);

  const handleToggle = useCallback(
    (name: string, enabled: boolean) => {
      togglePlugin(name, enabled);
    },
    [togglePlugin]
  );

  const handleSelect = useCallback(
    (name: string) => {
      loadPluginDetail(name);
    },
    [loadPluginDetail]
  );

  // Detail view
  if (selectedPlugin) {
    if (detailLoading) {
      return (
        <div className="flex items-center justify-center py-12">
          <ReloadIcon className="w-5 h-5 animate-spin text-gray-400" />
        </div>
      );
    }
    return <PluginDetailView detail={selectedPlugin} onBack={clearSelectedPlugin} />;
  }

  // List view
  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-lg font-semibold text-gray-900 dark:text-white">
            {t('plugins.title')}
          </h3>
          <p className="text-sm text-gray-500 dark:text-gray-400">
            {t('plugins.description')}
          </p>
        </div>
        <button
          onClick={refresh}
          disabled={refreshing}
          className={clsx(
            'flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm',
            'bg-gray-100 dark:bg-gray-800',
            'text-gray-700 dark:text-gray-300',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'disabled:opacity-50',
            'focus:outline-none focus:ring-2 focus:ring-primary-500'
          )}
        >
          <ReloadIcon className={clsx('w-3.5 h-3.5', refreshing && 'animate-spin')} />
          {t('plugins.refresh')}
        </button>
      </div>

      {/* Toast */}
      {toastMessage && (
        <div
          className={clsx(
            'px-3 py-2 rounded-lg text-sm',
            toastType === 'success' && 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300',
            toastType === 'error' && 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300',
            toastType === 'info' && 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300'
          )}
        >
          {toastMessage}
        </div>
      )}

      {/* Error */}
      {error && (
        <div className="px-3 py-2 rounded-lg text-sm bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300">
          {error}
        </div>
      )}

      {/* Loading */}
      {loading && (
        <div className="flex items-center justify-center py-8">
          <ReloadIcon className="w-5 h-5 animate-spin text-gray-400" />
        </div>
      )}

      {/* Empty state */}
      {!loading && plugins.length === 0 && (
        <div className="text-center py-8">
          <InfoCircledIcon className="w-8 h-8 mx-auto text-gray-300 dark:text-gray-600 mb-2" />
          <p className="text-sm text-gray-500 dark:text-gray-400">
            {t('plugins.empty.title')}
          </p>
          <p className="text-xs text-gray-400 dark:text-gray-500 mt-1">
            {t('plugins.empty.description')}
          </p>
        </div>
      )}

      {/* Plugin list */}
      {!loading && plugins.length > 0 && (
        <div className="space-y-2">
          {plugins.map((plugin) => (
            <PluginListItem
              key={plugin.name}
              plugin={plugin}
              onToggle={handleToggle}
              onSelect={handleSelect}
            />
          ))}
        </div>
      )}

      {/* Summary */}
      {!loading && plugins.length > 0 && (
        <p className="text-xs text-gray-400 dark:text-gray-500">
          {t('plugins.summary', { count: plugins.length, enabled: plugins.filter((p) => p.enabled).length })}
        </p>
      )}
    </div>
  );
}

export default PluginSection;
