/**
 * ExportDialog Component
 *
 * Dialog for exporting conversations in various formats (JSON, Markdown, HTML).
 */

import { useState } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { clsx } from 'clsx';
import { useTranslation } from 'react-i18next';
import {
  Cross2Icon,
  DownloadIcon,
  FileTextIcon,
  CodeIcon,
  FileIcon,
  CheckIcon,
  CopyIcon,
} from '@radix-ui/react-icons';
import { useClaudeCodeStore } from '../../store/claudeCode';

// ============================================================================
// ExportDialog Component
// ============================================================================

interface ExportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type ExportFormat = 'json' | 'markdown' | 'html';

export function ExportDialog({ open, onOpenChange }: ExportDialogProps) {
  const { t } = useTranslation('claudeCode');
  const { exportConversation, messages } = useClaudeCodeStore();
  const [selectedFormat, setSelectedFormat] = useState<ExportFormat>('markdown');
  const [copied, setCopied] = useState(false);
  const [downloaded, setDownloaded] = useState(false);

  const isEmpty = messages.length === 0;

  const handleCopy = () => {
    const content = exportConversation(selectedFormat);
    navigator.clipboard.writeText(content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleDownload = () => {
    const content = exportConversation(selectedFormat);
    const mimeTypes = {
      json: 'application/json',
      markdown: 'text/markdown',
      html: 'text/html',
    };
    const extensions = {
      json: 'json',
      markdown: 'md',
      html: 'html',
    };

    const blob = new Blob([content], { type: mimeTypes[selectedFormat] });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `claude-code-conversation.${extensions[selectedFormat]}`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    setDownloaded(true);
    setTimeout(() => setDownloaded(false), 2000);
  };

  const formats: { id: ExportFormat; name: string; icon: typeof FileTextIcon; description: string }[] = [
    {
      id: 'json',
      name: t('export.formats.json.name'),
      icon: CodeIcon,
      description: t('export.formats.json.description'),
    },
    {
      id: 'markdown',
      name: t('export.formats.markdown.name'),
      icon: FileTextIcon,
      description: t('export.formats.markdown.description'),
    },
    {
      id: 'html',
      name: t('export.formats.html.name'),
      icon: FileIcon,
      description: t('export.formats.html.description'),
    },
  ];

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0" />
        <Dialog.Content
          className={clsx(
            'fixed left-[50%] top-[50%] translate-x-[-50%] translate-y-[-50%]',
            'w-full max-w-md rounded-lg',
            'bg-white dark:bg-gray-900',
            'border border-gray-200 dark:border-gray-700',
            'shadow-xl',
            'focus:outline-none',
            'data-[state=open]:animate-in data-[state=closed]:animate-out',
            'data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0',
            'data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95',
            'data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%]',
            'data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%]'
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200 dark:border-gray-700">
            <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-white">
              {t('export.title')}
            </Dialog.Title>
            <Dialog.Close asChild>
              <button
                className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-500"
                aria-label="Close"
              >
                <Cross2Icon className="w-5 h-5" />
              </button>
            </Dialog.Close>
          </div>

          {/* Content */}
          <div className="p-6 space-y-6">
            {isEmpty ? (
              <div className="text-center py-8 text-gray-500 dark:text-gray-400">
                <FileTextIcon className="w-12 h-12 mx-auto mb-3 opacity-50" />
                <p>{t('export.empty.title')}</p>
                <p className="text-sm mt-1">{t('export.empty.description')}</p>
              </div>
            ) : (
              <>
                {/* Format Selection */}
                <div>
                  <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">
                    {t('export.formatLabel')}
                  </label>
                  <div className="space-y-2">
                    {formats.map((format) => (
                      <button
                        key={format.id}
                        onClick={() => setSelectedFormat(format.id)}
                        className={clsx(
                          'w-full flex items-center gap-3 p-3 rounded-lg border transition-colors text-left',
                          selectedFormat === format.id
                            ? 'border-primary-500 bg-primary-50 dark:bg-primary-900/20'
                            : 'border-gray-200 dark:border-gray-700 hover:border-gray-300 dark:hover:border-gray-600'
                        )}
                      >
                        <div
                          className={clsx(
                            'p-2 rounded',
                            selectedFormat === format.id
                              ? 'bg-primary-100 dark:bg-primary-900/50'
                              : 'bg-gray-100 dark:bg-gray-800'
                          )}
                        >
                          <format.icon
                            className={clsx(
                              'w-5 h-5',
                              selectedFormat === format.id
                                ? 'text-primary-600 dark:text-primary-400'
                                : 'text-gray-500 dark:text-gray-400'
                            )}
                          />
                        </div>
                        <div className="flex-1">
                          <div
                            className={clsx(
                              'font-medium',
                              selectedFormat === format.id
                                ? 'text-primary-700 dark:text-primary-300'
                                : 'text-gray-900 dark:text-white'
                            )}
                          >
                            {format.name}
                          </div>
                          <div className="text-xs text-gray-500 dark:text-gray-400">
                            {format.description}
                          </div>
                        </div>
                        {selectedFormat === format.id && (
                          <CheckIcon className="w-5 h-5 text-primary-600 dark:text-primary-400" />
                        )}
                      </button>
                    ))}
                  </div>
                </div>

                {/* Message Count */}
                <div className="flex items-center justify-between text-sm text-gray-500 dark:text-gray-400 bg-gray-50 dark:bg-gray-800 p-3 rounded-lg">
                  <span>{t('export.messagesCount')}</span>
                  <span className="font-medium text-gray-900 dark:text-white">{messages.length}</span>
                </div>
              </>
            )}
          </div>

          {/* Actions */}
          {!isEmpty && (
            <div className="flex items-center justify-end gap-3 px-6 py-4 border-t border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-800">
              <button
                onClick={handleCopy}
                className={clsx(
                  'flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors',
                  'bg-gray-100 dark:bg-gray-700 text-gray-700 dark:text-gray-300',
                  'hover:bg-gray-200 dark:hover:bg-gray-600'
                )}
              >
                {copied ? (
                  <>
                    <CheckIcon className="w-4 h-4" />
                    {t('export.copied')}
                  </>
                ) : (
                  <>
                    <CopyIcon className="w-4 h-4" />
                    {t('export.copy')}
                  </>
                )}
              </button>
              <button
                onClick={handleDownload}
                className={clsx(
                  'flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium transition-colors',
                  'bg-primary-600 text-white hover:bg-primary-700'
                )}
              >
                {downloaded ? (
                  <>
                    <CheckIcon className="w-4 h-4" />
                    {t('export.downloaded')}
                  </>
                ) : (
                  <>
                    <DownloadIcon className="w-4 h-4" />
                    {t('export.download')}
                  </>
                )}
              </button>
            </div>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export default ExportDialog;
