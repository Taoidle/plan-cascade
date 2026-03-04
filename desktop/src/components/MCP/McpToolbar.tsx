import { clsx } from 'clsx';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import {
  CheckIcon,
  DotsHorizontalIcon,
  DownloadIcon,
  InfoCircledIcon,
  PlusIcon,
  ReloadIcon,
  UploadIcon,
} from '@radix-ui/react-icons';
import { useTranslation } from 'react-i18next';

interface McpToolbarProps {
  loading: boolean;
  onRefresh: () => void;
  onExport: () => void;
  onImport: () => void;
  onDiscover: () => void;
  onAdd: () => void;
  onTestEnabled: () => void;
}

export function McpToolbar({
  loading,
  onRefresh,
  onExport,
  onImport,
  onDiscover,
  onAdd,
  onTestEnabled,
}: McpToolbarProps) {
  const { t } = useTranslation();
  const secondaryButtonClass = clsx(
    'inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium',
    'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300',
    'hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors',
  );

  return (
    <div className="p-4 border-b border-gray-200 dark:border-gray-700">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">{t('mcp.title')}</h2>
          <p className="text-sm text-gray-500 dark:text-gray-400">{t('mcp.description')}</p>
        </div>

        <div className="flex items-center justify-end gap-2 flex-wrap">
          <button
            type="button"
            onClick={onRefresh}
            disabled={loading}
            className={clsx(
              'p-2 rounded-md bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400',
              'hover:bg-gray-200 dark:hover:bg-gray-700 disabled:opacity-50 transition-colors',
            )}
            title={t('mcp.refresh')}
            aria-label={t('mcp.refresh')}
          >
            <ReloadIcon className={clsx('w-4 h-4', loading && 'animate-spin')} />
          </button>

          <button
            type="button"
            onClick={onAdd}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium text-white bg-primary-600 hover:bg-primary-700"
          >
            <PlusIcon className="w-4 h-4" />
            {t('mcp.addServer')}
          </button>

          <div className="hidden md:flex items-center gap-2">
            <button type="button" onClick={onExport} className={secondaryButtonClass}>
              <UploadIcon className="w-4 h-4" />
              {t('mcp.export')}
            </button>
            <button type="button" onClick={onImport} className={secondaryButtonClass}>
              <DownloadIcon className="w-4 h-4" />
              {t('mcp.import')}
            </button>
            <button type="button" onClick={onDiscover} className={secondaryButtonClass}>
              <InfoCircledIcon className="w-4 h-4" />
              {t('mcp.discover.title')}
            </button>
            <button type="button" onClick={onTestEnabled} className={secondaryButtonClass}>
              <CheckIcon className="w-4 h-4" />
              {t('mcp.eventActions.testEnabled')}
            </button>
          </div>

          <div className="md:hidden">
            <DropdownMenu.Root>
              <DropdownMenu.Trigger asChild>
                <button
                  type="button"
                  className={clsx(
                    'inline-flex items-center justify-center rounded-md p-2',
                    'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300',
                    'hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors',
                  )}
                  aria-label={t('common.moreActions', { defaultValue: 'More actions' })}
                >
                  <DotsHorizontalIcon className="w-4 h-4" />
                </button>
              </DropdownMenu.Trigger>
              <DropdownMenu.Portal>
                <DropdownMenu.Content
                  sideOffset={8}
                  align="end"
                  className={clsx(
                    'z-50 min-w-44 rounded-md border border-gray-200 dark:border-gray-700',
                    'bg-white dark:bg-gray-900 shadow-lg p-1',
                  )}
                >
                  <DropdownMenu.Item
                    onSelect={onExport}
                    className="px-3 py-2 text-sm rounded cursor-pointer outline-none text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
                  >
                    {t('mcp.export')}
                  </DropdownMenu.Item>
                  <DropdownMenu.Item
                    onSelect={onImport}
                    className="px-3 py-2 text-sm rounded cursor-pointer outline-none text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
                  >
                    {t('mcp.import')}
                  </DropdownMenu.Item>
                  <DropdownMenu.Item
                    onSelect={onDiscover}
                    className="px-3 py-2 text-sm rounded cursor-pointer outline-none text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
                  >
                    {t('mcp.discover.title')}
                  </DropdownMenu.Item>
                  <DropdownMenu.Item
                    onSelect={onTestEnabled}
                    className="px-3 py-2 text-sm rounded cursor-pointer outline-none text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
                  >
                    {t('mcp.eventActions.testEnabled')}
                  </DropdownMenu.Item>
                </DropdownMenu.Content>
              </DropdownMenu.Portal>
            </DropdownMenu.Root>
          </div>
        </div>
      </div>
    </div>
  );
}
