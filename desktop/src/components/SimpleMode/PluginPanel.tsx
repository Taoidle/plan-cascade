/**
 * PluginPanel Component
 *
 * Collapsible sidebar panel showing discovered plugins with toggle switches.
 * Includes a "Manage All..." button to open the full PluginDialog.
 */

import { useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { clsx } from 'clsx';
import { GearIcon } from '@radix-ui/react-icons';
import { usePluginStore } from '../../store/plugins';
import { getPluginSourceLabel } from '../../types/plugin';

// ============================================================================
// PluginPanel
// ============================================================================

export function PluginPanel() {
  const { t } = useTranslation('simpleMode');

  const plugins = usePluginStore((s) => s.plugins);
  const loading = usePluginStore((s) => s.loading);
  const panelOpen = usePluginStore((s) => s.panelOpen);
  const loadPlugins = usePluginStore((s) => s.loadPlugins);
  const togglePlugin = usePluginStore((s) => s.togglePlugin);
  const openDialog = usePluginStore((s) => s.openDialog);

  // Load data when panel opens
  useEffect(() => {
    if (panelOpen && plugins.length === 0) {
      loadPlugins();
    }
  }, [panelOpen, plugins.length, loadPlugins]);

  const handleToggle = useCallback(
    (name: string, enabled: boolean) => {
      togglePlugin(name, enabled);
    },
    [togglePlugin]
  );

  const handleManageAll = useCallback(() => {
    openDialog();
  }, [openDialog]);

  if (!panelOpen) return null;

  return (
    <div
      data-testid="plugin-panel"
      className="border-t border-gray-200 dark:border-gray-700"
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2">
        <span className="text-xs font-medium text-gray-700 dark:text-gray-300">
          {t('pluginPanel.title')}
        </span>
        <button
          onClick={handleManageAll}
          className={clsx(
            'p-1 rounded-md transition-colors',
            'text-gray-400 hover:text-gray-600 dark:hover:text-gray-300',
            'hover:bg-gray-100 dark:hover:bg-gray-800'
          )}
          title={t('pluginPanel.manageAll')}
        >
          <GearIcon className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Content */}
      <div className="px-2 pb-2 space-y-1 max-h-[300px] overflow-y-auto">
        {/* Loading state */}
        {loading && plugins.length === 0 && (
          <div className="text-center py-4">
            <span className="text-xs text-gray-400 dark:text-gray-500">
              {t('pluginPanel.loading')}
            </span>
          </div>
        )}

        {/* Empty state */}
        {!loading && plugins.length === 0 && (
          <div className="text-center py-4">
            <span className="text-xs text-gray-400 dark:text-gray-500">
              {t('pluginPanel.noPlugins')}
            </span>
          </div>
        )}

        {/* Plugin list */}
        {plugins.map((plugin) => (
          <div
            key={plugin.name}
            className={clsx(
              'flex items-center gap-2 px-2 py-1.5 rounded-md',
              'hover:bg-gray-50 dark:hover:bg-gray-800',
              'transition-colors'
            )}
          >
            {/* Toggle */}
            <button
              onClick={() => handleToggle(plugin.name, !plugin.enabled)}
              className={clsx(
                'relative inline-flex h-4 w-7 items-center rounded-full shrink-0',
                'transition-colors focus:outline-none focus:ring-2 focus:ring-primary-500',
                plugin.enabled
                  ? 'bg-primary-600'
                  : 'bg-gray-300 dark:bg-gray-600'
              )}
              role="switch"
              aria-checked={plugin.enabled}
            >
              <span
                className={clsx(
                  'inline-block h-3 w-3 rounded-full bg-white',
                  'transform transition-transform',
                  plugin.enabled ? 'translate-x-3.5' : 'translate-x-0.5'
                )}
              />
            </button>

            {/* Info */}
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-1">
                <span className="text-xs text-gray-900 dark:text-white truncate">
                  {plugin.name}
                </span>
                <span className="text-2xs text-gray-400 dark:text-gray-500 shrink-0">
                  v{plugin.version}
                </span>
              </div>
              <div className="flex items-center gap-1.5 text-2xs text-gray-400 dark:text-gray-500">
                <span className="px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-700 text-gray-500 dark:text-gray-400">
                  {getPluginSourceLabel(plugin.source)}
                </span>
                {plugin.skill_count > 0 && (
                  <span>{plugin.skill_count} skills</span>
                )}
                {plugin.command_count > 0 && (
                  <span>{plugin.command_count} cmds</span>
                )}
                {plugin.hook_count > 0 && (
                  <span>{plugin.hook_count} hooks</span>
                )}
              </div>
            </div>
          </div>
        ))}
      </div>

      {/* Manage All button */}
      <div className="px-3 pb-2">
        <button
          onClick={handleManageAll}
          className={clsx(
            'w-full px-2 py-1.5 rounded-md text-xs font-medium transition-colors',
            'text-primary-600 dark:text-primary-400',
            'hover:bg-primary-50 dark:hover:bg-primary-900/20',
            'border border-primary-200 dark:border-primary-800'
          )}
        >
          {t('pluginPanel.manageAll')}
        </button>
      </div>
    </div>
  );
}

export default PluginPanel;
