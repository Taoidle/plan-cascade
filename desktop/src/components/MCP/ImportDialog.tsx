/**
 * ImportDialog Component
 *
 * Dialog for importing MCP servers from Claude Desktop.
 */

import { useState } from 'react';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import * as Dialog from '@radix-ui/react-dialog';
import { Cross2Icon, DownloadIcon, CheckIcon, CrossCircledIcon } from '@radix-ui/react-icons';
import type { ImportResult, CommandResponse } from '../../types/mcp';

interface ImportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImportComplete: () => void;
}

export function ImportDialog({ open, onOpenChange, onImportComplete }: ImportDialogProps) {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<ImportResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleImport = async () => {
    setLoading(true);
    setError(null);
    setResult(null);

    try {
      const response = await invoke<CommandResponse<ImportResult>>('import_from_claude_desktop');

      if (response.success && response.data) {
        setResult(response.data);
      } else {
        setError(response.error || 'Failed to import servers');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to import servers');
    } finally {
      setLoading(false);
    }
  };

  const handleClose = () => {
    if (result && result.added > 0) {
      onImportComplete();
    }
    setResult(null);
    setError(null);
    onOpenChange(false);
  };

  return (
    <Dialog.Root open={open} onOpenChange={handleClose}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 animate-in fade-in-0" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-md max-h-[85vh] overflow-y-auto',
            'bg-white dark:bg-gray-900 rounded-lg shadow-xl',
            'animate-in fade-in-0 zoom-in-95',
            'focus:outline-none'
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              {t('mcp.importTitle')}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button className="p-1 rounded-md hover:bg-gray-100 dark:hover:bg-gray-800">
                <Cross2Icon className="w-5 h-5" />
              </button>
            </Dialog.Close>
          </div>

          {/* Body */}
          <div className="p-4">
            {!result && !error && (
              <>
                <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">
                  {t('mcp.importDescription')}
                </p>

                <div className="p-3 rounded-md bg-gray-100 dark:bg-gray-800 mb-4">
                  <p className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
                    {t('mcp.configPath')}
                  </p>
                  <code className="text-xs text-gray-700 dark:text-gray-300">
                    {getConfigPathForPlatform()}
                  </code>
                </div>

                <button
                  onClick={handleImport}
                  disabled={loading}
                  className={clsx(
                    'w-full flex items-center justify-center gap-2 py-2.5 rounded-md',
                    'bg-primary-600 hover:bg-primary-700',
                    'text-white text-sm font-medium',
                    'disabled:opacity-50',
                    'transition-colors'
                  )}
                >
                  {loading ? (
                    <>
                      <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
                      <span>{t('mcp.importing')}</span>
                    </>
                  ) : (
                    <>
                      <DownloadIcon className="w-4 h-4" />
                      <span>{t('mcp.importNow')}</span>
                    </>
                  )}
                </button>
              </>
            )}

            {result && (
              <div className="space-y-4">
                {/* Summary */}
                <div className="grid grid-cols-3 gap-3">
                  <div className="p-3 rounded-md bg-green-100 dark:bg-green-900/30 text-center">
                    <p className="text-2xl font-bold text-green-700 dark:text-green-300">
                      {result.added}
                    </p>
                    <p className="text-xs text-green-600 dark:text-green-400">
                      {t('mcp.added')}
                    </p>
                  </div>
                  <div className="p-3 rounded-md bg-yellow-100 dark:bg-yellow-900/30 text-center">
                    <p className="text-2xl font-bold text-yellow-700 dark:text-yellow-300">
                      {result.skipped}
                    </p>
                    <p className="text-xs text-yellow-600 dark:text-yellow-400">
                      {t('mcp.skipped')}
                    </p>
                  </div>
                  <div className="p-3 rounded-md bg-red-100 dark:bg-red-900/30 text-center">
                    <p className="text-2xl font-bold text-red-700 dark:text-red-300">
                      {result.failed}
                    </p>
                    <p className="text-xs text-red-600 dark:text-red-400">
                      {t('mcp.failed')}
                    </p>
                  </div>
                </div>

                {/* Added Servers */}
                {result.servers.length > 0 && (
                  <div>
                    <p className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      {t('mcp.importedServers')}
                    </p>
                    <div className="space-y-1">
                      {result.servers.map((name, i) => (
                        <div
                          key={i}
                          className="flex items-center gap-2 text-sm text-gray-600 dark:text-gray-400"
                        >
                          <CheckIcon className="w-4 h-4 text-green-500" />
                          <span>{name}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {/* Errors */}
                {result.errors.length > 0 && (
                  <div>
                    <p className="text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                      {t('mcp.importErrors')}
                    </p>
                    <div className="space-y-1 max-h-32 overflow-y-auto">
                      {result.errors.map((err, i) => (
                        <div
                          key={i}
                          className="flex items-start gap-2 text-xs text-red-600 dark:text-red-400"
                        >
                          <CrossCircledIcon className="w-3.5 h-3.5 mt-0.5 flex-shrink-0" />
                          <span>{err}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}

            {error && (
              <div className="p-4 rounded-md bg-red-100 dark:bg-red-900/30">
                <p className="text-sm text-red-700 dark:text-red-300">{error}</p>
              </div>
            )}
          </div>

          {/* Footer */}
          <div className="flex justify-end p-4 border-t border-gray-200 dark:border-gray-700">
            <button
              onClick={handleClose}
              className={clsx(
                'px-4 py-2 rounded-md',
                'bg-primary-600 hover:bg-primary-700',
                'text-white text-sm font-medium'
              )}
            >
              {result ? t('common.done') : t('common.cancel')}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function getConfigPathForPlatform(): string {
  // Detect platform from user agent
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes('win')) {
    return '%APPDATA%\\Claude\\config.json';
  } else if (ua.includes('mac')) {
    return '~/Library/Application Support/Claude/config.json';
  } else {
    return '~/.config/claude-desktop/config.json';
  }
}

export default ImportDialog;
