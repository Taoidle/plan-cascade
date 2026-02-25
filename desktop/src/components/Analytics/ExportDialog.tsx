/**
 * Export Dialog Component
 *
 * Dialog for exporting usage data in CSV or JSON format.
 */

import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import * as Dialog from '@radix-ui/react-dialog';
import { Cross2Icon } from '@radix-ui/react-icons';
import { clsx } from 'clsx';
import { useAnalyticsStore, type ExportFormat } from '../../store/analytics';

interface ExportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ExportDialog({ open, onOpenChange }: ExportDialogProps) {
  const { t } = useTranslation('analytics');
  const [format, setFormat] = useState<ExportFormat>('csv');
  const [includeSummary, setIncludeSummary] = useState(true);
  const [exportType, setExportType] = useState<'all' | 'byModel' | 'byProject'>('all');

  const { exportData, exportByModel, exportByProject, isExporting } = useAnalyticsStore();

  const handleExport = async () => {
    let data: string | null = null;
    let filename = '';

    switch (exportType) {
      case 'all':
        const result = await exportData(format, includeSummary);
        if (result) {
          data = result.data;
          filename = result.suggested_filename;
        }
        break;
      case 'byModel':
        data = await exportByModel(format);
        filename = `usage_by_model.${format}`;
        break;
      case 'byProject':
        data = await exportByProject(format);
        filename = `usage_by_project.${format}`;
        break;
    }

    if (data) {
      // Create and download file
      const blob = new Blob([data], {
        type: format === 'csv' ? 'text/csv;charset=utf-8;' : 'application/json',
      });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = filename;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);

      onOpenChange(false);
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm" />
        <Dialog.Content
          className={clsx(
            'fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2',
            'w-full max-w-md',
            'bg-white dark:bg-gray-900 rounded-xl shadow-xl',
            'p-6',
            'focus:outline-none',
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between mb-6">
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              {t('exportDialog.title', 'Export Usage Data')}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button
                className={clsx(
                  'p-2 rounded-lg',
                  'hover:bg-gray-100 dark:hover:bg-gray-800',
                  'focus:outline-none focus:ring-2 focus:ring-primary-500',
                )}
                aria-label="Close"
              >
                <Cross2Icon className="w-4 h-4 text-gray-500" />
              </button>
            </Dialog.Close>
          </div>

          {/* Content */}
          <div className="space-y-6">
            {/* Export Type */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                {t('exportDialog.exportType', 'Export Type')}
              </label>
              <div className="space-y-2">
                {[
                  { value: 'all', label: t('exportDialog.types.all', 'All Records') },
                  { value: 'byModel', label: t('exportDialog.types.byModel', 'Aggregated by Model') },
                  { value: 'byProject', label: t('exportDialog.types.byProject', 'Aggregated by Project') },
                ].map((option) => (
                  <label
                    key={option.value}
                    className={clsx(
                      'flex items-center gap-3 p-3 rounded-lg cursor-pointer',
                      'border transition-colors',
                      exportType === option.value
                        ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                        : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800',
                    )}
                  >
                    <input
                      type="radio"
                      name="exportType"
                      value={option.value}
                      checked={exportType === option.value}
                      onChange={(e) => setExportType(e.target.value as typeof exportType)}
                      className="sr-only"
                    />
                    <span
                      className={clsx(
                        'w-4 h-4 rounded-full border-2 flex items-center justify-center',
                        exportType === option.value ? 'border-primary-500' : 'border-gray-300 dark:border-gray-600',
                      )}
                    >
                      {exportType === option.value && <span className="w-2 h-2 rounded-full bg-primary-500" />}
                    </span>
                    <span className="text-sm text-gray-900 dark:text-white">{option.label}</span>
                  </label>
                ))}
              </div>
            </div>

            {/* Format */}
            <div>
              <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                {t('exportDialog.format', 'Format')}
              </label>
              <div className="flex gap-4">
                {[
                  { value: 'csv', label: 'CSV' },
                  { value: 'json', label: 'JSON' },
                ].map((option) => (
                  <label
                    key={option.value}
                    className={clsx(
                      'flex-1 flex items-center justify-center gap-2 p-3 rounded-lg cursor-pointer',
                      'border transition-colors',
                      format === option.value
                        ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                        : 'border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800',
                    )}
                  >
                    <input
                      type="radio"
                      name="format"
                      value={option.value}
                      checked={format === option.value}
                      onChange={(e) => setFormat(e.target.value as ExportFormat)}
                      className="sr-only"
                    />
                    <span className="text-sm font-medium text-gray-900 dark:text-white">{option.label}</span>
                  </label>
                ))}
              </div>
            </div>

            {/* Include Summary */}
            {exportType === 'all' && (
              <label className="flex items-center gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  checked={includeSummary}
                  onChange={(e) => setIncludeSummary(e.target.checked)}
                  className={clsx(
                    'w-4 h-4 rounded',
                    'border-gray-300 dark:border-gray-600',
                    'text-primary-600 focus:ring-primary-500',
                  )}
                />
                <span className="text-sm text-gray-700 dark:text-gray-300">
                  {t('exportDialog.includeSummary', 'Include summary statistics')}
                </span>
              </label>
            )}
          </div>

          {/* Footer */}
          <div className="flex justify-end gap-3 mt-8">
            <Dialog.Close asChild>
              <button
                className={clsx(
                  'px-4 py-2 rounded-lg',
                  'bg-gray-100 dark:bg-gray-800',
                  'text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-200 dark:hover:bg-gray-700',
                )}
              >
                {t('exportDialog.cancel', 'Cancel')}
              </button>
            </Dialog.Close>
            <button
              onClick={handleExport}
              disabled={isExporting}
              className={clsx(
                'px-4 py-2 rounded-lg',
                'bg-primary-600 text-white',
                'hover:bg-primary-700',
                'disabled:opacity-50 disabled:cursor-not-allowed',
                'flex items-center gap-2',
              )}
            >
              {isExporting && (
                <svg className="w-4 h-4 animate-spin" viewBox="0 0 24 24">
                  <circle
                    className="opacity-25"
                    cx="12"
                    cy="12"
                    r="10"
                    stroke="currentColor"
                    strokeWidth="4"
                    fill="none"
                  />
                  <path
                    className="opacity-75"
                    fill="currentColor"
                    d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                  />
                </svg>
              )}
              {isExporting ? t('exportDialog.exporting', 'Exporting...') : t('exportDialog.export', 'Export')}
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default ExportDialog;
