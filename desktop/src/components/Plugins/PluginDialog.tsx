/**
 * PluginDialog Component
 *
 * Full management dialog for plugins. Shows tabs for "Installed" (existing
 * list with toggle/detail) and "Marketplace" (registry browsing).
 * Includes "Install from URL" button and uses Radix UI Dialog.
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import * as Dialog from '@radix-ui/react-dialog';
import {
  Cross2Icon,
  MagnifyingGlassIcon,
  ReloadIcon,
  ChevronLeftIcon,
  ChevronRightIcon,
  DownloadIcon,
} from '@radix-ui/react-icons';
import { usePluginStore } from '../../store/plugins';
import { getPluginSourceLabel } from '../../types/plugin';
import type { PluginInfo, PluginDetail } from '../../types/plugin';
import { MarketplaceView } from './MarketplaceView';
import { InstallFromUrlDialog } from './InstallFromUrlDialog';
import { AddMarketplaceDialog } from './AddMarketplaceDialog';

// ============================================================================
// PluginListItem
// ============================================================================

function PluginListItem({
  plugin,
  onToggle,
  onSelect,
}: {
  plugin: PluginInfo;
  onToggle: (name: string, enabled: boolean) => void;
  onSelect: (name: string) => void;
}) {
  return (
    <div
      className={clsx(
        'flex items-center justify-between p-3 rounded-lg',
        'border border-gray-200 dark:border-gray-700',
        'hover:bg-gray-50 dark:hover:bg-gray-800/50',
        'transition-colors',
      )}
    >
      <div className="flex-1 min-w-0 mr-3">
        <div className="flex items-center gap-2">
          <span className="font-medium text-sm text-gray-900 dark:text-white truncate">{plugin.name}</span>
          <span className="text-xs text-gray-500 dark:text-gray-400">v{plugin.version}</span>
          <span
            className={clsx(
              'px-1.5 py-0.5 text-2xs rounded',
              'bg-gray-100 dark:bg-gray-700',
              'text-gray-600 dark:text-gray-300',
            )}
          >
            {getPluginSourceLabel(plugin.source)}
          </span>
        </div>
        <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5 truncate">
          {plugin.description || 'No description'}
        </p>
        <div className="flex items-center gap-3 mt-1 text-2xs text-gray-400 dark:text-gray-500">
          {plugin.skill_count > 0 && <span>{plugin.skill_count} skills</span>}
          {plugin.command_count > 0 && <span>{plugin.command_count} commands</span>}
          {plugin.hook_count > 0 && <span>{plugin.hook_count} hooks</span>}
          {plugin.has_instructions && <span>instructions</span>}
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
            plugin.enabled ? 'bg-primary-600' : 'bg-gray-300 dark:bg-gray-600',
          )}
          role="switch"
          aria-checked={plugin.enabled}
        >
          <span
            className={clsx(
              'inline-block h-3.5 w-3.5 rounded-full bg-white',
              'transform transition-transform',
              plugin.enabled ? 'translate-x-4.5' : 'translate-x-0.5',
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
            'focus:outline-none focus:ring-2 focus:ring-primary-500',
          )}
        >
          <ChevronRightIcon className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}

// ============================================================================
// PluginDetailView
// ============================================================================

function PluginDetailView({ detail, onBack }: { detail: PluginDetail; onBack: () => void }) {
  const { plugin } = detail;

  return (
    <div className="flex flex-col h-full overflow-y-auto p-4 space-y-4">
      {/* Back button + Header */}
      <div className="flex items-center gap-2">
        <button
          onClick={onBack}
          className={clsx(
            'p-1.5 rounded-md',
            'hover:bg-gray-200 dark:hover:bg-gray-700',
            'text-gray-500 dark:text-gray-400',
          )}
        >
          <ChevronLeftIcon className="w-4 h-4" />
        </button>
        <div>
          <h3 className="text-base font-semibold text-gray-900 dark:text-white">{plugin.manifest.name}</h3>
          <p className="text-xs text-gray-500 dark:text-gray-400">
            v{plugin.manifest.version}
            {plugin.manifest.author && ` by ${plugin.manifest.author}`}
            {plugin.manifest.license && ` (${plugin.manifest.license})`}
          </p>
        </div>
      </div>

      {/* Description */}
      {plugin.manifest.description && (
        <p className="text-xs text-gray-600 dark:text-gray-300">{plugin.manifest.description}</p>
      )}

      {/* Keywords */}
      {plugin.manifest.keywords.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {plugin.manifest.keywords.map((kw) => (
            <span
              key={kw}
              className="px-2 py-0.5 text-2xs rounded-full bg-primary-100 dark:bg-primary-900 text-primary-700 dark:text-primary-300"
            >
              {kw}
            </span>
          ))}
        </div>
      )}

      {/* Skills */}
      {plugin.skills.length > 0 && (
        <DetailSection title={`Skills (${plugin.skills.length})`}>
          {plugin.skills.map((skill) => (
            <div key={skill.name} className="p-2 rounded border border-gray-200 dark:border-gray-700">
              <div className="flex items-center gap-2">
                <span className="font-medium text-xs text-gray-900 dark:text-white">{skill.name}</span>
                {skill.user_invocable && (
                  <span className="text-2xs px-1.5 py-0.5 rounded bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300">
                    invocable
                  </span>
                )}
              </div>
              <p className="text-2xs text-gray-500 dark:text-gray-400 mt-0.5">{skill.description}</p>
            </div>
          ))}
        </DetailSection>
      )}

      {/* Commands */}
      {plugin.commands.length > 0 && (
        <DetailSection title={`Commands (${plugin.commands.length})`}>
          {plugin.commands.map((cmd) => (
            <div key={cmd.name} className="p-2 rounded border border-gray-200 dark:border-gray-700">
              <span className="font-medium text-xs text-gray-900 dark:text-white">/{cmd.name}</span>
              <p className="text-2xs text-gray-500 dark:text-gray-400 mt-0.5">{cmd.description}</p>
            </div>
          ))}
        </DetailSection>
      )}

      {/* Hooks */}
      {plugin.hooks.length > 0 && (
        <DetailSection title={`Hooks (${plugin.hooks.length})`}>
          {plugin.hooks.map((hook, i) => (
            <div key={`${hook.event}-${i}`} className="p-2 rounded border border-gray-200 dark:border-gray-700">
              <div className="flex items-center gap-2">
                <span className="font-medium text-xs text-gray-900 dark:text-white">{hook.event}</span>
                <span
                  className={clsx(
                    'text-2xs px-1.5 py-0.5 rounded',
                    hook.hook_type === 'command'
                      ? 'bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300'
                      : 'bg-purple-100 dark:bg-purple-900 text-purple-700 dark:text-purple-300',
                  )}
                >
                  {hook.hook_type}
                </span>
              </div>
              {hook.matcher && (
                <p className="text-2xs text-gray-400 mt-0.5">
                  matcher: <code className="bg-gray-100 dark:bg-gray-800 px-1 rounded">{hook.matcher}</code>
                </p>
              )}
              <p className="text-2xs text-gray-500 dark:text-gray-400 mt-0.5 font-mono truncate">{hook.command}</p>
            </div>
          ))}
        </DetailSection>
      )}

      {/* Instructions preview */}
      {plugin.instructions && (
        <DetailSection title="Instructions">
          <pre
            className={clsx(
              'text-2xs text-gray-600 dark:text-gray-300',
              'bg-gray-50 dark:bg-gray-800 rounded p-3',
              'overflow-auto max-h-40',
              'whitespace-pre-wrap',
            )}
          >
            {plugin.instructions.length > 500 ? `${plugin.instructions.slice(0, 500)}...` : plugin.instructions}
          </pre>
        </DetailSection>
      )}

      {/* Root path */}
      <p className="text-2xs text-gray-400 dark:text-gray-500">Location: {detail.root_path}</p>
    </div>
  );
}

function DetailSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <h4 className="text-xs font-medium text-gray-700 dark:text-gray-300 mb-2">{title}</h4>
      <div className="space-y-2">{children}</div>
    </div>
  );
}

// ============================================================================
// PluginDialog
// ============================================================================

export function PluginDialog() {
  const { t } = useTranslation('settings');
  const { t: tSimple } = useTranslation('simpleMode');

  const plugins = usePluginStore((s) => s.plugins);
  const selectedPlugin = usePluginStore((s) => s.selectedPlugin);
  const loading = usePluginStore((s) => s.loading);
  const detailLoading = usePluginStore((s) => s.detailLoading);
  const refreshing = usePluginStore((s) => s.refreshing);
  const dialogOpen = usePluginStore((s) => s.dialogOpen);
  const activeTab = usePluginStore((s) => s.activeTab);
  const closeDialog = usePluginStore((s) => s.closeDialog);
  const loadPlugins = usePluginStore((s) => s.loadPlugins);
  const togglePlugin = usePluginStore((s) => s.togglePlugin);
  const refresh = usePluginStore((s) => s.refresh);
  const loadPluginDetail = usePluginStore((s) => s.loadPluginDetail);
  const clearSelectedPlugin = usePluginStore((s) => s.clearSelectedPlugin);
  const setActiveTab = usePluginStore((s) => s.setActiveTab);
  const openInstallDialog = usePluginStore((s) => s.openInstallDialog);

  const [searchQuery, setSearchQuery] = useState('');

  // Load data when dialog opens
  useEffect(() => {
    if (dialogOpen) {
      loadPlugins();
    }
  }, [dialogOpen, loadPlugins]);

  // Reset search on close
  useEffect(() => {
    if (!dialogOpen) {
      setSearchQuery('');
    }
  }, [dialogOpen]);

  const filteredPlugins = useMemo(() => {
    if (!searchQuery.trim()) return plugins;
    const q = searchQuery.toLowerCase();
    return plugins.filter(
      (p) =>
        p.name.toLowerCase().includes(q) ||
        p.description.toLowerCase().includes(q) ||
        (p.author && p.author.toLowerCase().includes(q)),
    );
  }, [plugins, searchQuery]);

  const handleToggle = useCallback(
    (name: string, enabled: boolean) => {
      togglePlugin(name, enabled);
    },
    [togglePlugin],
  );

  const handleSelect = useCallback(
    (name: string) => {
      loadPluginDetail(name);
    },
    [loadPluginDetail],
  );

  const handleRefresh = useCallback(() => {
    refresh();
  }, [refresh]);

  // Detail view
  if (selectedPlugin && !detailLoading) {
    return (
      <Dialog.Root open={dialogOpen} onOpenChange={(open) => !open && closeDialog()}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 bg-black/50 z-50 animate-[fadeIn_0.15s]" />
          <Dialog.Content
            data-testid="plugin-dialog"
            className={clsx(
              'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-50',
              'w-[680px] max-w-[90vw] h-[560px] max-h-[85vh]',
              'bg-white dark:bg-gray-900 rounded-xl shadow-2xl',
              'border border-gray-200 dark:border-gray-700',
              'flex flex-col overflow-hidden',
              'animate-[contentShow_0.2s]',
            )}
          >
            {/* Header */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
              <Dialog.Title className="text-sm font-semibold text-gray-900 dark:text-white">
                {tSimple('pluginPanel.title')}
              </Dialog.Title>
              <Dialog.Close asChild>
                <button
                  className={clsx(
                    'p-1.5 rounded-md',
                    'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                    'hover:bg-gray-100 dark:hover:bg-gray-800',
                  )}
                >
                  <Cross2Icon className="w-4 h-4" />
                </button>
              </Dialog.Close>
            </div>
            <PluginDetailView detail={selectedPlugin} onBack={clearSelectedPlugin} />
          </Dialog.Content>
        </Dialog.Portal>
        <InstallFromUrlDialog />
      </Dialog.Root>
    );
  }

  return (
    <Dialog.Root open={dialogOpen} onOpenChange={(open) => !open && closeDialog()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 z-50 animate-[fadeIn_0.15s]" />
        <Dialog.Content
          data-testid="plugin-dialog"
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-50',
            'w-[680px] max-w-[90vw] h-[560px] max-h-[85vh]',
            'bg-white dark:bg-gray-900 rounded-xl shadow-2xl',
            'border border-gray-200 dark:border-gray-700',
            'flex flex-col overflow-hidden',
            'animate-[contentShow_0.2s]',
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700">
            <Dialog.Title className="text-sm font-semibold text-gray-900 dark:text-white">
              {tSimple('pluginPanel.title')}
            </Dialog.Title>
            <div className="flex items-center gap-2">
              {/* Install from URL button */}
              <button
                onClick={openInstallDialog}
                className={clsx(
                  'inline-flex items-center gap-1 px-2 py-1 rounded-md text-xs',
                  'text-gray-500 dark:text-gray-400',
                  'hover:bg-gray-100 dark:hover:bg-gray-800',
                  'transition-colors',
                )}
                title={t('plugins.installFromUrl')}
              >
                <DownloadIcon className="w-3.5 h-3.5" />
                <span className="hidden sm:inline">{t('plugins.installFromUrl')}</span>
              </button>
              <Dialog.Close asChild>
                <button
                  className={clsx(
                    'p-1.5 rounded-md',
                    'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                    'hover:bg-gray-100 dark:hover:bg-gray-800',
                  )}
                >
                  <Cross2Icon className="w-4 h-4" />
                </button>
              </Dialog.Close>
            </div>
          </div>

          {/* Tabs */}
          <div className="flex border-b border-gray-200 dark:border-gray-700">
            <button
              onClick={() => setActiveTab('installed')}
              className={clsx(
                'flex-1 px-4 py-2 text-xs font-medium transition-colors',
                activeTab === 'installed'
                  ? 'text-primary-600 dark:text-primary-400 border-b-2 border-primary-600 dark:border-primary-400'
                  : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
              )}
            >
              {t('plugins.installedTab')} ({plugins.length})
            </button>
            <button
              onClick={() => setActiveTab('marketplace')}
              className={clsx(
                'flex-1 px-4 py-2 text-xs font-medium transition-colors',
                activeTab === 'marketplace'
                  ? 'text-primary-600 dark:text-primary-400 border-b-2 border-primary-600 dark:border-primary-400'
                  : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
              )}
            >
              {t('plugins.marketplace')}
            </button>
          </div>

          {/* Tab Content */}
          {activeTab === 'installed' ? (
            <>
              {/* Toolbar: search + refresh */}
              <div className="p-3 border-b border-gray-200 dark:border-gray-700 flex items-center gap-2">
                <div className="relative flex-1">
                  <MagnifyingGlassIcon className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
                  <input
                    type="text"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                    placeholder="Search plugins..."
                    className={clsx(
                      'w-full pl-8 pr-3 py-1.5 rounded-md text-xs',
                      'bg-gray-50 dark:bg-gray-800',
                      'border border-gray-200 dark:border-gray-700',
                      'text-gray-700 dark:text-gray-300',
                      'placeholder:text-gray-400 dark:placeholder:text-gray-500',
                      'focus:outline-none focus:ring-2 focus:ring-primary-500 focus:border-transparent',
                    )}
                  />
                </div>
                <button
                  onClick={handleRefresh}
                  disabled={refreshing}
                  className={clsx(
                    'p-1.5 rounded-md transition-colors',
                    'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
                    'hover:bg-gray-100 dark:hover:bg-gray-800',
                    'disabled:opacity-50',
                  )}
                  title="Refresh"
                >
                  <ReloadIcon className={clsx('w-3.5 h-3.5', refreshing && 'animate-spin')} />
                </button>
              </div>

              {/* Plugin list */}
              <div className="flex-1 overflow-y-auto p-3">
                {loading && filteredPlugins.length === 0 ? (
                  <div className="flex items-center justify-center py-8">
                    <span className="text-xs text-gray-400">{tSimple('pluginPanel.loading')}</span>
                  </div>
                ) : filteredPlugins.length === 0 ? (
                  <div className="flex flex-col items-center justify-center py-8 text-center">
                    <p className="text-xs text-gray-500 dark:text-gray-400">{tSimple('pluginPanel.noPlugins')}</p>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {filteredPlugins.map((plugin) => (
                      <PluginListItem
                        key={plugin.name}
                        plugin={plugin}
                        onToggle={handleToggle}
                        onSelect={handleSelect}
                      />
                    ))}
                  </div>
                )}
              </div>

              {/* Footer summary */}
              {!loading && plugins.length > 0 && (
                <div className="px-4 py-2 border-t border-gray-200 dark:border-gray-700">
                  <p className="text-2xs text-gray-400 dark:text-gray-500">
                    {plugins.length} plugin(s), {plugins.filter((p) => p.enabled).length} enabled
                  </p>
                </div>
              )}
            </>
          ) : (
            <MarketplaceView />
          )}
        </Dialog.Content>
      </Dialog.Portal>
      <InstallFromUrlDialog />
      <AddMarketplaceDialog />
    </Dialog.Root>
  );
}

export default PluginDialog;
